// Rust guideline compliant 2026-06-22

use num_complex::Complex32;
use rustfft::FftPlanner;

use crate::analyze::color::{frame_to_rgb, hsv_to_rgb, scale_to_u8};
use crate::analyze::fft::{build_bin_to_band, hann_window};
use crate::analyze::normalize::{aggregate_frames_flat, normalize_frames_flat};
use crate::options::{DetectionMode, GenerateOptions};
use crate::types::{AnalysisDiagnostics, MoodbarAnalysis};

pub(crate) struct FrameAnalyzer<'a> {
    options: &'a GenerateOptions,
    fft_size: usize,
    pub(crate) hop_size: usize,
    channel_count: usize,
    hann: Vec<f32>,
    fft: std::sync::Arc<dyn rustfft::Fft<f32>>,
    prev_mag: Vec<f32>,
    bin_to_band: Vec<usize>,
    pending: Vec<f32>,
    pending_start: usize,
    fft_buf: Vec<Complex32>,
    fft_scratch: Vec<Complex32>,
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
        let scratch_len = fft.get_inplace_scratch_len();

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
            fft_scratch: vec![Complex32::new(0.0, 0.0); scratch_len],
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

        let aggregated = aggregate_frames_flat(
            &self.frames_flat,
            self.channel_count,
            self.options.frames_per_color.max(1),
        );
        let mut normalized = aggregated;
        normalize_frames_flat(&mut normalized, self.channel_count, self.options);

        let band_colors = resolve_band_colors(self.channel_count, self.options);
        let band_colors_f64: Vec<[f64; 3]> = band_colors
            .iter()
            .map(|c| {
                [
                    c[0] as f64 / 255.0,
                    c[1] as f64 / 255.0,
                    c[2] as f64 / 255.0,
                ]
            })
            .collect();

        let colors = if self.options.theme == crate::options::Theme::Classic
            && self.options.custom_colors.is_none()
        {
            normalized
                .chunks_exact(self.channel_count)
                .map(|frame| {
                    let (r, g, b) = frame_to_rgb(frame);
                    [scale_to_u8(r), scale_to_u8(g), scale_to_u8(b)]
                })
                .collect()
        } else {
            normalized
                .chunks_exact(self.channel_count)
                .map(|frame| blend_frame_colors(frame, &band_colors_f64))
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
        let pending_slice = &self.pending[start..start + self.fft_size];
        for ((c, &p), &h) in self.fft_buf.iter_mut().zip(pending_slice).zip(&self.hann) {
            c.re = p * h;
            c.im = 0.0;
        }
        self.finish_fft_into_frame();
    }

    fn analyze_window_from_pending_padded(&mut self, copy_len: usize) {
        self.frame_scratch.fill(0.0);
        let pending_slice = &self.pending[self.pending_start..self.pending_start + copy_len];
        for ((i, c), &h) in self.fft_buf.iter_mut().enumerate().zip(&self.hann) {
            let sample = if i < copy_len { pending_slice[i] } else { 0.0 };
            c.re = sample * h;
            c.im = 0.0;
        }
        self.finish_fft_into_frame();
    }

    fn finish_fft_into_frame(&mut self) {
        self.fft
            .process_with_scratch(&mut self.fft_buf, &mut self.fft_scratch);

        let half_size = self.fft_size / 2;
        let bins = &self.fft_buf[..half_size];
        let bin_to_band = &self.bin_to_band[..half_size];
        let prev_mag = &mut self.prev_mag[..half_size];

        for ((&c, &idx), p_mag) in bins.iter().zip(bin_to_band).zip(prev_mag) {
            let re = c.re;
            let im = c.im;
            let mag = (re * re + im * im).sqrt();
            let signal = match self.options.detection_mode {
                DetectionMode::SpectralEnergy => mag,
                DetectionMode::SpectralFlux => {
                    let flux = (mag - *p_mag).max(0.0f32);
                    *p_mag = mag;
                    flux
                }
            };
            // SAFETY: `bin_to_band` is pre-validated during construction in `build_bin_to_band`
            // to only contain indices in the range `0..channel_count`. Since `self.frame_scratch`
            // has length `channel_count`, index `idx` is guaranteed to be in bounds.
            unsafe {
                *self.frame_scratch.get_unchecked_mut(idx) += signal as f64;
            }
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

fn blend_frame_colors(frame: &[f64], band_colors_f64: &[[f64; 3]]) -> [u8; 3] {
    let mut r_sum = 0.0;
    let mut g_sum = 0.0;
    let mut b_sum = 0.0;
    for (i, &energy) in frame.iter().enumerate() {
        if let Some(&color) = band_colors_f64.get(i) {
            let energy = energy.clamp(0.0, 1.0);
            r_sum += energy * color[0];
            g_sum += energy * color[1];
            b_sum += energy * color[2];
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
        for (x, y) in batch.frames.iter().zip(streamed.frames.iter()) {
            assert!((x - y).abs() < 1e-9);
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

        let max_per_channel = |frames: &Vec<f64>, idx: usize| {
            frames
                .chunks_exact(3)
                .map(|f| f[idx])
                .fold(0.0f64, f64::max)
        };
        let max_overall = |frames: &Vec<f64>| frames.iter().copied().fold(0.0f64, f64::max);
        assert!((max_per_channel(&per_channel.frames, 0) - 1.0).abs() < 1e-9);
        assert!((max_per_channel(&per_channel.frames, 2) - 1.0).abs() < 1e-9);
        assert!((max_overall(&global.frames) - 1.0).abs() < 1e-9);
        assert!(
            max_per_channel(&global.frames, 0) < 1.0 || max_per_channel(&global.frames, 2) < 1.0
        );
    }

    #[test]
    #[ignore = "dev-only throughput benchmark"]
    fn bench_analysis_throughput() {
        let sample_rate = 44_100;
        let pcm = sine(440.0, sample_rate, 60.0); // 60 seconds of audio
        let options = GenerateOptions::default();

        let start = std::time::Instant::now();
        let result = crate::analyze_pcm_mono(sample_rate, &pcm, &options);
        let elapsed = start.elapsed();
        eprintln!(
            "Analysis throughput: 60s audio processed in {:?}, frames: {}",
            elapsed,
            result.frames.len() / result.channel_count
        );
    }
}
