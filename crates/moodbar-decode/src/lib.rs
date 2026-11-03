// Rust guideline compliant 2026-06-22

//! Symphonia-based audio decoding pipeline for moodbar analysis.
//!
//! Streams audio packets directly into `FrameAnalyzer` with SIMD-accelerated downmixing
//! and bounded memory usage.

use std::fs::File;
use std::io::Cursor;
use std::path::Path;

use moodbar_analysis::{
    analysis_to_raw_rgb_bytes, FrameAnalyzer, GenerateOptions, MoodbarAnalysis, MoodbarError,
};
use symphonia::core::audio::{AudioBufferRef, SampleBuffer};
use symphonia::core::codecs::DecoderOptions;
use symphonia::core::errors::Error as SymphoniaError;
use symphonia::core::formats::FormatOptions;
use symphonia::core::io::MediaSourceStream;
use symphonia::core::meta::MetadataOptions;
use symphonia::core::probe::Hint;
use thiserror::Error;

/// Error types returned by audio decoding and analysis operations.
#[derive(Debug, Error)]
pub enum MoodbarDecodeError {
    /// No playable audio track found in media container.
    #[error("no playable audio track found")]
    NoAudioTrack,
    /// Decoded audio stream contained zero valid audio samples.
    #[error("decoded stream has no samples")]
    EmptyAudio,
    /// Underlying input/output failure.
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),
    /// Codec or container decoding failure.
    #[error("decode error: {0}")]
    Decode(#[from] SymphoniaError),
    /// Analysis rejected the options (or failed) after decode setup.
    #[error(transparent)]
    Analysis(#[from] MoodbarError),
}

/// Non-fatal diagnostics tracked across audio packet decode.
#[derive(Debug, Clone, Default)]
pub struct DecodeDiagnostics {
    /// Count of packet decode errors recovered during stream processing.
    pub decode_errors: usize,
    /// Count of audio packets reporting zero channels.
    pub zero_channel_packets: usize,
    /// Count of incomplete multi-channel frames truncated at packet end.
    pub truncated_frames: usize,
}

/// Decodes and analyzes media from a filesystem path into moodbar frames.
pub fn analyze_path(
    path: &Path,
    options: &GenerateOptions,
) -> Result<MoodbarAnalysis, MoodbarDecodeError> {
    let file = File::open(path)?;
    let mss = MediaSourceStream::new(Box::new(file), Default::default());

    let mut hint = Hint::new();
    if let Some(ext) = path.extension().and_then(|e| e.to_str()) {
        hint.with_extension(ext);
    }

    analyze_media_source(mss, hint, options)
}

/// Decodes and analyzes in-memory audio bytes into moodbar frames.
pub fn analyze_bytes(
    bytes: &[u8],
    extension: Option<&str>,
    options: &GenerateOptions,
) -> Result<MoodbarAnalysis, MoodbarDecodeError> {
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

/// Convenience API to decode audio from a path directly to raw RGB bytes.
pub fn generate_moodbar_from_path(
    path: &Path,
    options: &GenerateOptions,
) -> Result<Vec<u8>, MoodbarDecodeError> {
    let analysis = analyze_path(path, options)?;
    Ok(analysis_to_raw_rgb_bytes(&analysis))
}

/// Convenience API to decode in-memory audio directly to raw RGB bytes.
pub fn generate_moodbar_from_bytes(
    bytes: &[u8],
    extension: Option<&str>,
    options: &GenerateOptions,
) -> Result<Vec<u8>, MoodbarDecodeError> {
    let analysis = analyze_bytes(bytes, extension, options)?;
    Ok(analysis_to_raw_rgb_bytes(&analysis))
}

#[cfg(target_arch = "aarch64")]
#[inline]
unsafe fn downmix_stereo_neon(interleaved: &[f32], out: &mut Vec<f32>) -> usize {
    use core::arch::aarch64::*;
    let num_pairs = interleaved.len() / 2;
    let chunks_4 = num_pairs / 4;
    let mut ptr = interleaved.as_ptr();
    let half = vdupq_n_f32(0.5);

    let start_len = out.len();
    out.reserve(num_pairs);
    let mut out_ptr = out.as_mut_ptr().add(start_len);

    for _ in 0..chunks_4 {
        let loaded = vld2q_f32(ptr);
        let sum = vaddq_f32(loaded.0, loaded.1);
        let mono = vmulq_f32(sum, half);
        vst1q_f32(out_ptr, mono);
        ptr = ptr.add(8);
        out_ptr = out_ptr.add(4);
    }
    out.set_len(start_len + chunks_4 * 4);
    chunks_4 * 8
}

#[inline]
fn downmix_stereo_to_mono(interleaved: &[f32], out: &mut Vec<f32>) {
    #[cfg(target_arch = "aarch64")]
    let processed = {
        // SAFETY: Pointer offsets and lengths are strictly bounded by `interleaved.len() / 8 * 8`.
        unsafe { downmix_stereo_neon(interleaved, out) }
    };
    #[cfg(not(target_arch = "aarch64"))]
    let processed = 0;

    for pair in interleaved[processed..].chunks_exact(2) {
        out.push((pair[0] + pair[1]) * 0.5);
    }
}

#[inline]
fn downmix_multichannel_to_mono(
    interleaved: &[f32],
    channels: usize,
    max_channels: usize,
    out: &mut Vec<f32>,
    diagnostics: &mut DecodeDiagnostics,
) {
    let inv_channels = 1.0 / max_channels as f32;
    for frame in interleaved.chunks(channels) {
        if frame.len() != channels {
            diagnostics.truncated_frames += 1;
            continue;
        }
        let sum: f32 = frame[..max_channels].iter().copied().sum();
        out.push(sum * inv_channels);
    }
}

/// Interleaves `decoded` into `slot`, reallocating when the packet holds more samples than fit.
///
/// `SampleBuffer::capacity` counts samples (frames x channels), while
/// `AudioBufferRef::capacity` counts frames per channel.
fn interleave<'a>(slot: &'a mut Option<SampleBuffer<f32>>, decoded: AudioBufferRef) -> &'a [f32] {
    let needed = decoded.capacity() * decoded.spec().channels.count();
    let buf = match slot.take() {
        Some(buf) if buf.capacity() >= needed => slot.insert(buf),
        _ => slot.insert(SampleBuffer::new(
            decoded.capacity() as u64,
            *decoded.spec(),
        )),
    };
    buf.copy_interleaved_ref(decoded);
    buf.samples()
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
    let estimated_samples = track.codec_params.n_frames.map(|n| n as usize);
    let mut analyzer = FrameAnalyzer::new(sample_rate, options, estimated_samples)?;
    let mut saw_samples = false;
    let mut diagnostics = DecodeDiagnostics::default();
    let mut sample_buf: Option<SampleBuffer<f32>> = None;
    let mut mono_scratch = Vec::<f32>::new();

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
                let channels = decoded.spec().channels.count();
                let interleaved = interleave(&mut sample_buf, decoded);

                if channels == 0 {
                    diagnostics.zero_channel_packets += 1;
                    continue;
                }

                mono_scratch.clear();
                if channels == 1 {
                    mono_scratch.extend_from_slice(interleaved);
                } else if channels == 2 {
                    downmix_stereo_to_mono(interleaved, &mut mono_scratch);
                } else {
                    let max_channels = channels.min(2);
                    downmix_multichannel_to_mono(
                        interleaved,
                        channels,
                        max_channels,
                        &mut mono_scratch,
                        &mut diagnostics,
                    );
                }

                if !mono_scratch.is_empty() {
                    saw_samples = true;
                    analyzer.feed_mono_samples(&mono_scratch);
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

    let mut analysis = analyzer.finish();
    analysis.diagnostics.decode_errors = diagnostics.decode_errors;
    analysis.diagnostics.zero_channel_packets = diagnostics.zero_channel_packets;
    analysis.diagnostics.truncated_frames = diagnostics.truncated_frames;
    Ok(analysis)
}

#[cfg(test)]
mod tests {
    use super::*;
    use symphonia::core::audio::{AsAudioBufferRef, AudioBuffer, Channels, Signal, SignalSpec};

    fn stereo_packet(frames: usize) -> AudioBuffer<f32> {
        let spec = SignalSpec::new(44_100, Channels::FRONT_LEFT | Channels::FRONT_RIGHT);
        let mut buf = AudioBuffer::new(frames as u64, spec);
        buf.render_reserved(Some(frames));
        buf
    }

    /// Minimal RIFF/WAVE container holding 16-bit PCM.
    fn wav_bytes(sample_rate: u32, channels: u16, samples: &[i16]) -> Vec<u8> {
        let data_len = (samples.len() * 2) as u32;
        let block_align = channels * 2;
        let mut out = Vec::with_capacity(44 + data_len as usize);
        out.extend_from_slice(b"RIFF");
        out.extend_from_slice(&(36 + data_len).to_le_bytes());
        out.extend_from_slice(b"WAVEfmt ");
        out.extend_from_slice(&16u32.to_le_bytes());
        out.extend_from_slice(&1u16.to_le_bytes());
        out.extend_from_slice(&channels.to_le_bytes());
        out.extend_from_slice(&sample_rate.to_le_bytes());
        out.extend_from_slice(&(sample_rate * u32::from(block_align)).to_le_bytes());
        out.extend_from_slice(&block_align.to_le_bytes());
        out.extend_from_slice(&16u16.to_le_bytes());
        out.extend_from_slice(b"data");
        out.extend_from_slice(&data_len.to_le_bytes());
        samples
            .iter()
            .for_each(|s| out.extend_from_slice(&s.to_le_bytes()));
        out
    }

    #[test]
    fn stereo_downmix_matches_scalar_reference() {
        for len in [0, 1, 2, 3, 7, 8, 9, 15, 17, 33, 1025] {
            let interleaved: Vec<f32> = (0..len).map(|i| (i as f32 * 0.37).sin()).collect();
            let expected: Vec<f32> = interleaved
                .chunks_exact(2)
                .map(|p| (p[0] + p[1]) * 0.5)
                .collect();
            let mut out = vec![42.0];
            downmix_stereo_to_mono(&interleaved, &mut out);
            assert_eq!(out[0], 42.0, "len {len}: existing samples must be kept");
            assert_eq!(out[1..], expected[..], "len {len}");
        }
    }

    #[test]
    fn multichannel_downmix_averages_front_pair_and_counts_truncated_frames() {
        let mut interleaved: Vec<f32> = (0..3)
            .flat_map(|f| [f as f32, f as f32 + 1.0, 9.0, 9.0, 9.0, 9.0])
            .collect();
        interleaved.extend_from_slice(&[1.0, 1.0, 1.0]);
        let mut out = Vec::new();
        let mut diagnostics = DecodeDiagnostics::default();
        downmix_multichannel_to_mono(&interleaved, 6, 2, &mut out, &mut diagnostics);
        assert_eq!(out, [0.5, 1.5, 2.5]);
        assert_eq!(diagnostics.truncated_frames, 1);
    }

    #[test]
    fn decodes_generated_wav_within_frame_cap() {
        let sample_rate = 22_050;
        let samples: Vec<i16> = (0..sample_rate * 3)
            .flat_map(|i| {
                let t = i as f32 / sample_rate as f32;
                let v = ((2.0 * std::f32::consts::PI * 440.0 * t).sin() * 16_000.0) as i16;
                [v, v / 2]
            })
            .collect();
        let bytes = wav_bytes(sample_rate, 2, &samples);
        for cap in [10, 25] {
            let options = GenerateOptions {
                max_target_frames: Some(cap),
                ..GenerateOptions::default()
            };
            let analysis = analyze_bytes(&bytes, Some("wav"), &options).unwrap();
            assert!(!analysis.colors.is_empty());
            assert!(analysis.colors.len() <= cap, "cap {cap}");
            assert_eq!(analysis.diagnostics.decode_errors, 0);
        }
    }

    #[test]
    fn interleave_grows_buffer_for_larger_multichannel_packet() {
        let mut slot = None;
        assert_eq!(
            interleave(&mut slot, stereo_packet(100).as_audio_buffer_ref()).len(),
            200
        );
        // 150 frames > 100 frames but < 200 samples: the old frames-vs-samples check kept the
        // 200-sample buffer and tripped symphonia's capacity assert.
        assert_eq!(
            interleave(&mut slot, stereo_packet(150).as_audio_buffer_ref()).len(),
            300
        );
        assert_eq!(
            interleave(&mut slot, stereo_packet(50).as_audio_buffer_ref()).len(),
            100
        );
    }
}
