use std::fs::File;
use std::io::Cursor;
use std::path::Path;

use moodbar_analysis::{
    analysis_to_raw_rgb_bytes, analyze_pcm_mono, GenerateOptions, MoodbarAnalysis,
};
use symphonia::core::audio::SampleBuffer;
use symphonia::core::codecs::DecoderOptions;
use symphonia::core::errors::Error as SymphoniaError;
use symphonia::core::formats::FormatOptions;
use symphonia::core::io::MediaSourceStream;
use symphonia::core::meta::MetadataOptions;
use symphonia::core::probe::Hint;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum MoodbarDecodeError {
    #[error("no playable audio track found")]
    NoAudioTrack,
    #[error("decoded stream has no samples")]
    EmptyAudio,
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),
    #[error("decode error: {0}")]
    Decode(#[from] SymphoniaError),
    #[error("invalid options: {0}")]
    InvalidOptions(String),
}

#[derive(Debug, Clone, Default)]
pub struct DecodeDiagnostics {
    pub decode_errors: usize,
    pub zero_channel_packets: usize,
    pub truncated_frames: usize,
}

pub fn analyze_path(
    path: &Path,
    options: &GenerateOptions,
) -> Result<MoodbarAnalysis, MoodbarDecodeError> {
    validate_options(options)?;

    let file = File::open(path)?;
    let mss = MediaSourceStream::new(Box::new(file), Default::default());

    let mut hint = Hint::new();
    if let Some(ext) = path.extension().and_then(|e| e.to_str()) {
        hint.with_extension(ext);
    }

    analyze_media_source(mss, hint, options)
}

pub fn analyze_bytes(
    bytes: &[u8],
    extension: Option<&str>,
    options: &GenerateOptions,
) -> Result<MoodbarAnalysis, MoodbarDecodeError> {
    validate_options(options)?;

    let cursor = Cursor::new(bytes.to_vec());
    let mss = MediaSourceStream::new(Box::new(cursor), Default::default());
    let mut hint = Hint::new();
    if let Some(ext) = extension {
        if !ext.is_empty() {
            hint.with_extension(ext);
        }
    }

    analyze_media_source(mss, hint, options)
}

pub fn generate_moodbar_from_path(
    path: &Path,
    options: &GenerateOptions,
) -> Result<Vec<u8>, MoodbarDecodeError> {
    let analysis = analyze_path(path, options)?;
    Ok(analysis_to_raw_rgb_bytes(&analysis))
}

pub fn generate_moodbar_from_bytes(
    bytes: &[u8],
    extension: Option<&str>,
    options: &GenerateOptions,
) -> Result<Vec<u8>, MoodbarDecodeError> {
    let analysis = analyze_bytes(bytes, extension, options)?;
    Ok(analysis_to_raw_rgb_bytes(&analysis))
}

fn analyze_media_source(
    mss: MediaSourceStream,
    hint: Hint,
    options: &GenerateOptions,
) -> Result<MoodbarAnalysis, MoodbarDecodeError> {
    let probed = symphonia::default::get_probe().format(
        &hint,
        mss,
        &FormatOptions::default(),
        &MetadataOptions::default(),
    )?;

    let mut format = probed.format;
    let track = format
        .tracks()
        .iter()
        .find(|t| t.codec_params.codec != symphonia::core::codecs::CODEC_TYPE_NULL)
        .ok_or(MoodbarDecodeError::NoAudioTrack)?;

    let sample_rate = track
        .codec_params
        .sample_rate
        .ok_or(MoodbarDecodeError::NoAudioTrack)?;
    let track_id = track.id;

    let mut decoder =
        symphonia::default::get_codecs().make(&track.codec_params, &DecoderOptions::default())?;
    let mut samples = Vec::<f32>::new();
    let mut saw_samples = false;
    let mut diagnostics = DecodeDiagnostics::default();

    loop {
        let packet = match format.next_packet() {
            Ok(packet) => packet,
            Err(SymphoniaError::IoError(err))
                if err.kind() == std::io::ErrorKind::UnexpectedEof =>
            {
                break;
            }
            Err(err) => return Err(err.into()),
        };

        if packet.track_id() != track_id {
            continue;
        }

        match decoder.decode(&packet) {
            Ok(decoded) => {
                let spec = *decoded.spec();
                let channels = spec.channels.count();
                let mut sample_buf = SampleBuffer::<f32>::new(decoded.capacity() as u64, spec);
                sample_buf.copy_interleaved_ref(decoded);
                let interleaved = sample_buf.samples();

                if channels == 0 {
                    diagnostics.zero_channel_packets += 1;
                    continue;
                }

                for frame in interleaved.chunks(channels) {
                    if frame.len() != channels {
                        diagnostics.truncated_frames += 1;
                        continue;
                    }
                    let sum = frame.iter().copied().sum::<f32>();
                    samples.push(sum / channels as f32);
                    saw_samples = true;
                }
            }
            Err(SymphoniaError::DecodeError(_)) => {
                diagnostics.decode_errors += 1;
                continue;
            }
            Err(SymphoniaError::IoError(err))
                if err.kind() == std::io::ErrorKind::UnexpectedEof =>
            {
                break;
            }
            Err(err) => return Err(err.into()),
        }
    }

    if !saw_samples {
        return Err(MoodbarDecodeError::EmptyAudio);
    }

    let mut analysis = analyze_pcm_mono(sample_rate, &samples, options);
    analysis.diagnostics.decode_errors = diagnostics.decode_errors;
    analysis.diagnostics.zero_channel_packets = diagnostics.zero_channel_packets;
    analysis.diagnostics.truncated_frames = diagnostics.truncated_frames;
    Ok(analysis)
}

fn validate_options(options: &GenerateOptions) -> Result<(), MoodbarDecodeError> {
    if !options.fft_size.is_power_of_two() || options.fft_size < 64 {
        return Err(MoodbarDecodeError::InvalidOptions(
            "fft_size must be a power of two and >= 64".to_string(),
        ));
    }
    if !(options.deterministic_floor.is_finite() && options.deterministic_floor > 0.0) {
        return Err(MoodbarDecodeError::InvalidOptions(
            "deterministic_floor must be finite and > 0".to_string(),
        ));
    }
    if options.frames_per_color == 0 {
        return Err(MoodbarDecodeError::InvalidOptions(
            "frames_per_color must be >= 1".to_string(),
        ));
    }

    let edges = if options.band_edges_hz.is_empty() {
        vec![options.low_cut_hz, options.mid_cut_hz]
    } else {
        options.band_edges_hz.clone()
    };

    if edges.is_empty() {
        return Err(MoodbarDecodeError::InvalidOptions(
            "at least one band edge is required".to_string(),
        ));
    }
    for pair in edges.windows(2) {
        if pair[0] >= pair[1] {
            return Err(MoodbarDecodeError::InvalidOptions(
                "band edges must be strictly increasing".to_string(),
            ));
        }
    }
    Ok(())
}
