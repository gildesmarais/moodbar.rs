use num_complex::Complex32;
use rustfft::FftPlanner;

use crate::analyze::color::{frame_to_rgb, hsv_to_rgb, scale_to_u8};
use crate::analyze::fft::{build_bin_to_band, hann_window};
use crate::analyze::normalize::{aggregate_frames, normalize_frames};
use crate::options::{DetectionMode, GenerateOptions};
use crate::types::{AnalysisDiagnostics, MoodbarAnalysis};

pub(crate) struct FrameAnalyzer<'a> {
    options: &'a GenerateOptions,
    fft_size: usize,
    pub(crate) hop_size: usize,
    channel_count: usize,
    hann: Vec<f32>,
    fft: std::sync::Arc<dyn rustfft::Fft<f32>>,
    prev_mag: Vec<f64>,
    bin_to_band: Vec<usize>,
    pending: Vec<f32>,
    pending_start: usize,
    fft_buf: Vec<Complex32>,
    frame_scratch: Vec<f64>,
    frames_flat: Vec<f64>,
    frame_count: usize,
}

impl<'a> FrameAnalyzer<'a> {
    pub(crate) fn new(
        sample_rate: u32,
        options: &'a GenerateOptions,
        total_samples: Option<usize>,
    ) -> Self {
        let fft_size = options.fft_size;
        let mut hop_size = fft_size / 2;
        if let (Some(total), Some(target)) = (total_samples, options.max_target_frames) {
            let dynamic_hop = total / target.max(1);
            hop_size = hop_size.max(dynamic_hop);
        }
        let band_edges_hz = options.effective_band_edges();
        let channel_count = band_edges_hz.len() + 1;

        let mut planner = FftPlanner::<f32>::new();
        let fft = planner.plan_fft_forward(fft_size);

        Self {
            options,
            fft_size,
            hop_size,
            bin_to_band: build_bin_to_band(
                fft_size,
                options.effective_nyquist_hz(sample_rate),
                &band_edges_hz,
            ),
            channel_count,
            hann: hann_window(fft_size),
            fft,
            prev_mag: vec![0.0; fft_size / 2],
            pending: Vec::with_capacity(fft_size * 2),
            pending_start: 0,
            fft_buf: vec![Complex32::new(0.0, 0.0); fft_size],
            frame_scratch: vec![0.0; channel_count],
            frames_flat: Vec::new(),
            frame_count: 0,
        }
    }

    pub(crate) fn feed_mono_samples(&mut self, samples: &[f32]) {
        if !samples.is_empty() {
            self.pending.extend_from_slice(samples);
        }

        while self.pending.len().saturating_sub(self.pending_start) >= self.fft_size {
            let start = self.pending_start;
            self.analyze_window_from_pending(start);
            self.pending_start += self.hop_size.max(1);
            self.compact_pending_if_needed();
        }
    }

    pub(crate) fn finish(mut self) -> MoodbarAnalysis {
        if self.frame_count == 0 && !self.pending.is_empty() {
            let available = self.pending.len().saturating_sub(self.pending_start);
            let copy_len = available.min(self.fft_size);
            self.analyze_window_from_pending_padded(copy_len);
        }

        let frames = self
            .frames_flat
            .chunks(self.channel_count)
            .map(|chunk| chunk.to_vec())
            .collect::<Vec<_>>();
        let aggregated = aggregate_frames(&frames, self.options.frames_per_color.max(1));
        let normalized = normalize_frames(&aggregated, self.options);

        let band_colors = resolve_band_colors(self.channel_count, self.options);
        let colors = if self.options.theme == crate::options::Theme::Classic
            && self.options.custom_colors.is_none()
        {
            normalized
                .iter()
                .map(|frame| {
                    let (r, g, b) = frame_to_rgb(frame);
                    [scale_to_u8(r), scale_to_u8(g), scale_to_u8(b)]
                })
                .collect()
        } else {
            normalized
                .iter()
                .map(|frame| blend_frame_colors(frame, &band_colors))
                .collect()
        };

        MoodbarAnalysis {
            channel_count: self.channel_count,
            frames: normalized,
            colors,
            diagnostics: AnalysisDiagnostics::default(),
            band_colors,
        }
    }

    fn analyze_window_from_pending(&mut self, start: usize) {
        self.frame_scratch.fill(0.0);
        for (i, c) in self.fft_buf.iter_mut().enumerate().take(self.fft_size) {
            c.re = self.pending[start + i] * self.hann[i];
            c.im = 0.0;
        }
        self.finish_fft_into_frame();
    }

    fn analyze_window_from_pending_padded(&mut self, copy_len: usize) {
        self.frame_scratch.fill(0.0);
        for (i, c) in self.fft_buf.iter_mut().enumerate().take(self.fft_size) {
            let sample = if i < copy_len {
                self.pending[self.pending_start + i]
            } else {
                0.0
            };
            c.re = sample * self.hann[i];
            c.im = 0.0;
        }
        self.finish_fft_into_frame();
    }

    fn finish_fft_into_frame(&mut self) {
        self.fft.process(&mut self.fft_buf);

        for (k, c) in self.fft_buf.iter().take(self.fft_size / 2).enumerate() {
            let mag = (c.re as f64).hypot(c.im as f64);
            let signal = match self.options.detection_mode {
                DetectionMode::SpectralEnergy => mag,
                DetectionMode::SpectralFlux => {
                    let flux = (mag - self.prev_mag[k]).max(0.0);
                    self.prev_mag[k] = mag;
                    flux
                }
            };
            let idx = self.bin_to_band[k];
            self.frame_scratch[idx] += signal;
        }
        self.frames_flat.extend_from_slice(&self.frame_scratch);
        self.frame_count += 1;
    }

    fn compact_pending_if_needed(&mut self) {
        let threshold = self.fft_size * 8;
        if self.pending_start > threshold {
            self.pending.drain(0..self.pending_start);
            self.pending_start = 0;
        }
    }
}

fn resolve_band_colors(channel_count: usize, options: &GenerateOptions) -> Vec<[u8; 3]> {
    let mut colors = Vec::new();
    if let Some(ref custom) = options.custom_colors {
        colors.extend_from_slice(custom);
    } else {
        match options.theme {
            crate::options::Theme::Classic => {
                colors.push([255, 0, 0]);
                colors.push([0, 255, 0]);
                colors.push([0, 0, 255]);
            }
            crate::options::Theme::Cool => {
                colors.push([220, 20, 180]);
                colors.push([240, 120, 0]);
                colors.push([0, 160, 240]);
            }
            crate::options::Theme::Light => {
                colors.push([240, 128, 128]);
                colors.push([144, 238, 144]);
                colors.push([173, 216, 230]);
            }
        }
    }

    while colors.len() < channel_count {
        let i = colors.len();
        let color = if channel_count <= 3 {
            match i {
                0 => [255, 0, 0],
                1 => [0, 255, 0],
                2 => [0, 0, 255],
                _ => [0, 0, 0],
            }
        } else {
            let (r, g, b) = hsv_to_rgb(i as f64 / channel_count as f64, 0.85, 1.0);
            [scale_to_u8(r), scale_to_u8(g), scale_to_u8(b)]
        };
        colors.push(color);
    }

    colors.truncate(channel_count);
    colors
}

fn blend_frame_colors(frame: &[f64], band_colors: &[[u8; 3]]) -> [u8; 3] {
    let mut r_sum = 0.0;
    let mut g_sum = 0.0;
    let mut b_sum = 0.0;
    for (i, &energy) in frame.iter().enumerate() {
        if let Some(&color) = band_colors.get(i) {
            let energy = energy.clamp(0.0, 1.0);
            r_sum += energy * (color[0] as f64 / 255.0);
            g_sum += energy * (color[1] as f64 / 255.0);
            b_sum += energy * (color[2] as f64 / 255.0);
        }
    }
    [scale_to_u8(r_sum), scale_to_u8(g_sum), scale_to_u8(b_sum)]
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::options::NormalizeMode;

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
    fn streaming_and_batch_pcm_paths_match() {
        let sample_rate = 44_100;
        let mut pcm = Vec::new();
        pcm.extend(sine(120.0, sample_rate, 0.4));
        pcm.extend(sine(900.0, sample_rate, 0.4));
        pcm.extend(sine(3300.0, sample_rate, 0.4));

        let options = GenerateOptions::default();
        let batch = crate::analyze_pcm_mono(sample_rate, &pcm, &options);

        let mut stream = FrameAnalyzer::new(sample_rate, &options, None);
        for chunk in pcm.chunks(257) {
            stream.feed_mono_samples(chunk);
        }
        let streamed = stream.finish();

        assert_eq!(batch.channel_count, streamed.channel_count);
        assert_eq!(batch.frames.len(), streamed.frames.len());
        for (a, b) in batch.frames.iter().zip(streamed.frames.iter()) {
            for (x, y) in a.iter().zip(b.iter()) {
                assert!((x - y).abs() < 1e-9);
            }
        }
    }

    #[test]
    fn global_peak_normalization_caps_all_channels_uniformly() {
        let sample_rate = 44_100;
        let mut pcm = Vec::new();
        pcm.extend(sine(100.0, sample_rate, 0.3));
        pcm.extend(sine(3000.0, sample_rate, 0.3));

        let per_channel = crate::analyze_pcm_mono(
            sample_rate,
            &pcm,
            &GenerateOptions {
                normalize_mode: NormalizeMode::PerChannelPeak,
                ..GenerateOptions::default()
            },
        );
        let global = crate::analyze_pcm_mono(
            sample_rate,
            &pcm,
            &GenerateOptions {
                normalize_mode: NormalizeMode::GlobalPeak,
                ..GenerateOptions::default()
            },
        );

        let max_per_channel = |frames: &Vec<Vec<f64>>, idx: usize| {
            frames.iter().map(|f| f[idx]).fold(0.0f64, f64::max)
        };
        let max_overall = |frames: &Vec<Vec<f64>>| {
            frames
                .iter()
                .flat_map(|f| f.iter().copied())
                .fold(0.0f64, f64::max)
        };
        assert!((max_per_channel(&per_channel.frames, 0) - 1.0).abs() < 1e-9);
        assert!((max_per_channel(&per_channel.frames, 2) - 1.0).abs() < 1e-9);
        assert!((max_overall(&global.frames) - 1.0).abs() < 1e-9);
        assert!(
            max_per_channel(&global.frames, 0) < 1.0 || max_per_channel(&global.frames, 2) < 1.0
        );
    }
}
