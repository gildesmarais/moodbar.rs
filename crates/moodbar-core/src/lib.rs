use std::fs::File;
use std::path::Path;

use num_complex::Complex32;
use rustfft::FftPlanner;
use symphonia::core::audio::SampleBuffer;
use symphonia::core::codecs::DecoderOptions;
use symphonia::core::errors::Error as SymphoniaError;
use symphonia::core::formats::FormatOptions;
use symphonia::core::io::MediaSourceStream;
use symphonia::core::meta::MetadataOptions;
use symphonia::core::probe::Hint;
use thiserror::Error;

#[derive(Debug, Clone)]
pub struct GenerateOptions {
    pub fft_size: usize,
    pub low_cut_hz: f32,
    pub mid_cut_hz: f32,
    pub normalize_mode: NormalizeMode,
    pub deterministic_floor: f64,
}

impl Default for GenerateOptions {
    fn default() -> Self {
        Self {
            fft_size: 2048,
            low_cut_hz: 500.0,
            mid_cut_hz: 2000.0,
            normalize_mode: NormalizeMode::PerChannelPeak,
            deterministic_floor: 1e-12,
        }
    }
}

#[derive(Debug, Clone, Copy)]
pub enum NormalizeMode {
    PerChannelPeak,
    GlobalPeak,
}

#[derive(Debug, Error)]
pub enum MoodbarError {
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

pub fn generate_moodbar_from_path(
    path: &Path,
    options: &GenerateOptions,
) -> Result<Vec<u8>, MoodbarError> {
    validate_options(options)?;

    let file = File::open(path)?;
    let mss = MediaSourceStream::new(Box::new(file), Default::default());

    let mut hint = Hint::new();
    if let Some(ext) = path.extension().and_then(|e| e.to_str()) {
        hint.with_extension(ext);
    }

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
        .ok_or(MoodbarError::NoAudioTrack)?;

    let sample_rate = track
        .codec_params
        .sample_rate
        .ok_or(MoodbarError::NoAudioTrack)?;
    let track_id = track.id;

    let mut decoder =
        symphonia::default::get_codecs().make(&track.codec_params, &DecoderOptions::default())?;

    let mut mono_samples = Vec::<f32>::new();

    loop {
        let packet = match format.next_packet() {
            Ok(packet) => packet,
            Err(SymphoniaError::IoError(err))
                if err.kind() == std::io::ErrorKind::UnexpectedEof =>
            {
                break
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
                    continue;
                }

                for frame in interleaved.chunks_exact(channels) {
                    let sum = frame.iter().copied().sum::<f32>();
                    mono_samples.push(sum / channels as f32);
                }
            }
            Err(SymphoniaError::DecodeError(_)) => continue,
            Err(SymphoniaError::IoError(err))
                if err.kind() == std::io::ErrorKind::UnexpectedEof =>
            {
                break
            }
            Err(err) => return Err(err.into()),
        }
    }

    if mono_samples.is_empty() {
        return Err(MoodbarError::EmptyAudio);
    }

    Ok(generate_moodbar_from_pcm_mono(
        sample_rate,
        &mono_samples,
        options,
    ))
}

pub fn generate_moodbar_from_pcm_mono(
    sample_rate: u32,
    samples: &[f32],
    options: &GenerateOptions,
) -> Vec<u8> {
    let fft_size = options.fft_size;
    let hop_size = fft_size / 2;
    let hann = hann_window(fft_size);

    let fft = {
        let mut planner = FftPlanner::<f32>::new();
        planner.plan_fft_forward(fft_size)
    };

    let nyquist = sample_rate as f64 / 2.0;
    let mut frames = Vec::<[f64; 3]>::new();

    let mut cursor = 0;
    while cursor < samples.len() {
        let mut buf = vec![Complex32::new(0.0, 0.0); fft_size];
        for i in 0..fft_size {
            let sample = samples.get(cursor + i).copied().unwrap_or(0.0);
            buf[i].re = sample * hann[i];
        }

        fft.process(&mut buf);

        let mut low = 0.0f64;
        let mut mid = 0.0f64;
        let mut high = 0.0f64;

        for (k, c) in buf.iter().take(fft_size / 2).enumerate() {
            let freq = (k as f64 / (fft_size as f64 / 2.0)) * nyquist;
            let mag = (c.re as f64).hypot(c.im as f64);

            if freq < options.low_cut_hz as f64 {
                low += mag;
            } else if freq < options.mid_cut_hz as f64 {
                mid += mag;
            } else {
                high += mag;
            }
        }

        frames.push([low, mid, high]);
        cursor += hop_size.max(1);
    }

    normalize_frames_to_rgb(&frames, options)
}

fn validate_options(options: &GenerateOptions) -> Result<(), MoodbarError> {
    if !options.fft_size.is_power_of_two() || options.fft_size < 64 {
        return Err(MoodbarError::InvalidOptions(
            "fft_size must be a power of two and >= 64".to_string(),
        ));
    }
    if !(0.0 < options.low_cut_hz && options.low_cut_hz < options.mid_cut_hz) {
        return Err(MoodbarError::InvalidOptions(
            "require 0 < low_cut_hz < mid_cut_hz".to_string(),
        ));
    }
    if !(options.deterministic_floor.is_finite() && options.deterministic_floor > 0.0) {
        return Err(MoodbarError::InvalidOptions(
            "deterministic_floor must be finite and > 0".to_string(),
        ));
    }
    Ok(())
}

fn hann_window(size: usize) -> Vec<f32> {
    if size == 1 {
        return vec![1.0];
    }
    (0..size)
        .map(|i| {
            let phase = (2.0 * std::f32::consts::PI * i as f32) / (size as f32 - 1.0);
            0.5 * (1.0 - phase.cos())
        })
        .collect()
}

fn normalize_frames_to_rgb(frames: &[[f64; 3]], options: &GenerateOptions) -> Vec<u8> {
    if frames.is_empty() {
        return Vec::new();
    }

    let floor = options.deterministic_floor;
    let max_r = frames
        .iter()
        .map(|f| f[0])
        .fold(0.0f64, f64::max)
        .max(floor);
    let max_g = frames
        .iter()
        .map(|f| f[1])
        .fold(0.0f64, f64::max)
        .max(floor);
    let max_b = frames
        .iter()
        .map(|f| f[2])
        .fold(0.0f64, f64::max)
        .max(floor);
    let global = max_r.max(max_g).max(max_b).max(floor);

    let mut out = Vec::<u8>::with_capacity(frames.len() * 3);
    for frame in frames {
        let (dr, dg, db) = match options.normalize_mode {
            NormalizeMode::PerChannelPeak => (max_r, max_g, max_b),
            NormalizeMode::GlobalPeak => (global, global, global),
        };
        out.push(scale_to_u8(frame[0] / dr));
        out.push(scale_to_u8(frame[1] / dg));
        out.push(scale_to_u8(frame[2] / db));
    }
    out
}

fn scale_to_u8(v: f64) -> u8 {
    (v.clamp(0.0, 1.0) * 255.0).round() as u8
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sine(freq_hz: f32, sample_rate: u32, seconds: f32) -> Vec<f32> {
        let len = (sample_rate as f32 * seconds) as usize;
        (0..len)
            .map(|i| {
                let t = i as f32 / sample_rate as f32;
                (2.0 * std::f32::consts::PI * freq_hz * t).sin()
            })
            .collect()
    }

    #[test]
    fn low_mid_high_have_distinct_dominant_channels() {
        let sample_rate = 44_100;
        let mut pcm = Vec::new();
        pcm.extend(sine(100.0, sample_rate, 0.5));
        pcm.extend(sine(1000.0, sample_rate, 0.5));
        pcm.extend(sine(5000.0, sample_rate, 0.5));

        let options = GenerateOptions::default();
        let bytes = generate_moodbar_from_pcm_mono(sample_rate, &pcm, &options);
        let frame_count = bytes.len() / 3;
        assert!(frame_count > 10);

        let segment = frame_count / 3;
        let avg = |start: usize, end: usize| -> [f32; 3] {
            let mut sum = [0.0f32; 3];
            let count = (end - start) as f32;
            for i in start..end {
                sum[0] += bytes[i * 3] as f32;
                sum[1] += bytes[i * 3 + 1] as f32;
                sum[2] += bytes[i * 3 + 2] as f32;
            }
            [sum[0] / count, sum[1] / count, sum[2] / count]
        };

        let low = avg(0, segment);
        let mid = avg(segment, segment * 2);
        let high = avg(segment * 2, frame_count);

        assert!(low[0] > low[1] && low[0] > low[2]);
        assert!(mid[1] > mid[0] && mid[1] > mid[2]);
        assert!(high[2] > high[0] && high[2] > high[1]);
    }
}
