// Rust guideline compliant 2026-06-22

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
    options
        .validate()
        .map_err(|e| MoodbarDecodeError::InvalidOptions(e.to_string()))?;

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
    options
        .validate()
        .map_err(|e| MoodbarDecodeError::InvalidOptions(e.to_string()))?;

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
    let estimated_samples = track.codec_params.n_frames.unwrap_or(0) as usize;
    let mut samples = Vec::<f32>::with_capacity(estimated_samples);
    let mut saw_samples = false;
    let mut diagnostics = DecodeDiagnostics::default();
    let mut sample_buf: Option<SampleBuffer<f32>> = None;

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

                if sample_buf.is_none()
                    || sample_buf.as_ref().unwrap().capacity() < decoded.capacity()
                {
                    sample_buf = Some(SampleBuffer::<f32>::new(decoded.capacity() as u64, spec));
                }

                let buf = sample_buf.as_mut().unwrap();
                buf.copy_interleaved_ref(decoded);
                let interleaved = buf.samples();

                if channels == 0 {
                    diagnostics.zero_channel_packets += 1;
                    continue;
                }

                if channels == 1 {
                    samples.extend_from_slice(interleaved);
                    if !interleaved.is_empty() {
                        saw_samples = true;
                    }
                } else if channels == 2 {
                    for pair in interleaved.chunks_exact(2) {
                        samples.push((pair[0] + pair[1]) * 0.5);
                    }
                    if !interleaved.is_empty() {
                        saw_samples = true;
                    }
                } else {
                    let max_channels = channels.min(2);
                    for frame in interleaved.chunks(channels) {
                        if frame.len() != channels {
                            diagnostics.truncated_frames += 1;
                            continue;
                        }
                        let sum = frame[..max_channels].iter().copied().sum::<f32>();
                        samples.push(sum / max_channels as f32);
                        saw_samples = true;
                    }
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
