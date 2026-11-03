// Rust guideline compliant 2026-06-22

use num_complex::Complex32;
use rustfft::FftPlanner;

use crate::analyze::color::{frame_to_rgb, hsv_to_rgb, scale_to_u8};
use crate::analyze::fft::{build_bin_to_band, hann_window};
use crate::analyze::normalize::{aggregate_frames_flat, normalize_frames_flat};
use crate::options::{DetectionMode, GenerateOptions};
use crate::types::{AnalysisDiagnostics, MoodbarAnalysis, MoodbarError};

/// Incremental stream analyzer that accumulates mono PCM audio frames.
///
/// Pre-allocates FFT and scratch buffers to process audio chunks without per-frame
/// heap allocations.
pub struct FrameAnalyzer<'a> {
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
    /// `Some(samples fed so far)` when no length estimate was given and `max_target_frames`
    /// must be enforced in `finish` instead of through the hop size.
    unestimated_fed: Option<usize>,
}

impl<'a> FrameAnalyzer<'a> {
    /// Creates a new streaming analyzer instance for a given sample rate and options.
    ///
    /// # Arguments
    ///
    /// * `sample_rate` - Audio sampling frequency in Hz.
    /// * `options` - Configured analysis parameters.
    /// * `total_samples` - Optional expected total sample count for dynamic hop calculation.
    ///   When `None`, `finish` decimates the frames to honor `max_target_frames` instead.
    ///
    /// # Errors
    ///
    /// Returns `MoodbarError::InvalidOptions` when `sample_rate` or `options` are out of range.
    pub fn new(
        sample_rate: u32,
        options: &'a GenerateOptions,
        total_samples: Option<usize>,
    ) -> Result<Self, MoodbarError> {
        options.validate(sample_rate)?;
        let fft_size = options.fft_size;
        let hop_size = match (total_samples, options.max_target_frames) {
            (Some(total), Some(target)) => dynamic_hop(fft_size, total, target),
            _ => fft_size / 2,
        };
        let band_edges_hz = options.effective_band_edges();
        let channel_count = band_edges_hz.len() + 1;

        let mut planner = FftPlanner::<f32>::new();
        let fft = planner.plan_fft_forward(fft_size);
        let scratch_len = fft.get_inplace_scratch_len();

        Ok(Self {
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
            unestimated_fed: (total_samples.is_none() && options.max_target_frames.is_some())
                .then_some(0),
        })
    }

    /// Returns the hop size in samples between FFT windows. Without a `total_samples` estimate
    /// this is the base hop (`fft_size / 2`); `finish` then decimates to `max_target_frames`.
    pub fn hop_size(&self) -> usize {
        self.hop_size
    }

    /// Returns the number of frequency band channels analyzed per frame.
    pub fn channel_count(&self) -> usize {
        self.channel_count
    }

    /// Feeds incremental mono PCM samples into the streaming analyzer.
    pub fn feed_mono_samples(&mut self, samples: &[f32]) {
        if let Some(fed) = &mut self.unestimated_fed {
            *fed += samples.len();
        }
        self.pending.extend_from_slice(samples);

        while self.pending.len().saturating_sub(self.pending_start) >= self.fft_size {
            let start = self.pending_start;
            self.analyze_window_from_pending(start);
            self.pending_start += self.hop_size;
            self.compact_pending_if_needed();
        }
    }

    /// Finalizes the stream and aggregates/normalizes output moodbar frames.
    pub fn finish(mut self) -> MoodbarAnalysis {
        if self.frame_count == 0 && !self.pending.is_empty() {
            let available = self.pending.len().saturating_sub(self.pending_start);
            let copy_len = available.min(self.fft_size);
            self.analyze_window_from_pending_padded(copy_len);
        }
        if let (Some(total), Some(target)) = (self.unestimated_fed, self.options.max_target_frames)
        {
            self.decimate_to_dynamic_hop(total, target);
        }

        let aggregated = aggregate_frames_flat(
            &self.frames_flat,
            self.channel_count,
            self.options.frames_per_color,
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

    /// Keeps, for each window the known-length dynamic hop would have analyzed, the base-hop
    /// frame nearest to it. Spectral energy matches the known-length output exactly whenever
    /// the dynamic hop is a multiple of the base hop; spectral flux stays base-hop flux.
    fn decimate_to_dynamic_hop(&mut self, total: usize, target: usize) {
        let base = self.hop_size;
        let hop = dynamic_hop(self.fft_size, total, target);
        if hop == base || total < self.fft_size {
            return;
        }
        let kept = ((total - self.fft_size) / hop + 1).min(target.max(1));
        let ch = self.channel_count;
        for k in 0..kept {
            let j = ((k * hop + base / 2) / base).min(self.frame_count - 1);
            self.frames_flat.copy_within(j * ch..(j + 1) * ch, k * ch);
        }
        self.frames_flat.truncate(kept * ch);
        self.frame_count = kept;
    }

    fn compact_pending_if_needed(&mut self) {
        // A hop wider than the window can step past buffered samples; carry the overshoot
        // into the next chunk so window starts stay exact multiples of the hop.
        if self.pending_start >= self.pending.len() {
            self.pending_start -= self.pending.len();
            self.pending.clear();
            return;
        }
        let threshold = self.fft_size * 8;
        if self.pending_start > threshold {
            self.pending.drain(0..self.pending_start);
            self.pending_start = 0;
        }
    }
}

/// Hop that spreads `total` samples over at most `target` windows, never below half a window.
fn dynamic_hop(fft_size: usize, total: usize, target: usize) -> usize {
    (fft_size / 2).max(total / target.max(1))
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
        let batch = crate::analyze_pcm_mono(sample_rate, &pcm, &options).unwrap();

        let mut stream = FrameAnalyzer::new(sample_rate, &options, None).unwrap();
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

    fn stream_unestimated(pcm: &[f32], options: &GenerateOptions) -> MoodbarAnalysis {
        let mut stream = FrameAnalyzer::new(44_100, options, None).unwrap();
        for chunk in pcm.chunks(1_000) {
            stream.feed_mono_samples(chunk);
        }
        stream.finish()
    }

    #[test]
    fn unknown_length_stream_honors_max_target_frames() {
        let pcm = sine(440.0, 44_100, 7.0);
        for target in [1, 50, 199, 2000] {
            let options = GenerateOptions {
                max_target_frames: Some(target),
                ..GenerateOptions::default()
            };
            let known = crate::analyze_pcm_mono(44_100, &pcm, &options).unwrap();
            let streamed = stream_unestimated(&pcm, &options);
            assert!(streamed.colors.len() <= target, "target {target}");
            assert_eq!(streamed.colors.len(), known.colors.len(), "target {target}");
        }
    }

    #[test]
    fn chunked_stream_with_hop_wider_than_window_matches_batch() {
        let pcm = sine(440.0, 44_100, 7.0);
        let options = GenerateOptions {
            max_target_frames: Some(10),
            ..GenerateOptions::default()
        };
        let batch = crate::analyze_pcm_mono(44_100, &pcm, &options).unwrap();
        let mut stream = FrameAnalyzer::new(44_100, &options, Some(pcm.len())).unwrap();
        assert!(stream.hop_size() > options.fft_size);
        for chunk in pcm.chunks(1_152) {
            stream.feed_mono_samples(chunk);
        }
        assert_eq!(batch.frames, stream.finish().frames);
    }

    #[test]
    fn unknown_length_matches_known_length_when_dynamic_hop_is_base_multiple() {
        // 40 windows over 40 * 4096 samples: dynamic hop 4096 = 4 x base hop 1024.
        let mut pcm = sine(150.0, 44_100, 1.0);
        pcm.extend(sine(2500.0, 44_100, 1.0));
        pcm.extend(sine(9000.0, 44_100, 2.0));
        pcm.truncate(40 * 4096);
        let options = GenerateOptions {
            max_target_frames: Some(40),
            ..GenerateOptions::default()
        };
        let known = crate::analyze_pcm_mono(44_100, &pcm, &options).unwrap();
        let streamed = stream_unestimated(&pcm, &options);
        assert_eq!(known.frames, streamed.frames);
        assert_eq!(known.colors, streamed.colors);
    }

    #[test]
    fn known_length_keeps_dynamic_hop_and_window_count() {
        let pcm = sine(440.0, 44_100, 7.0);
        let options = GenerateOptions {
            max_target_frames: Some(50),
            ..GenerateOptions::default()
        };
        let analyzer = FrameAnalyzer::new(44_100, &options, Some(pcm.len())).unwrap();
        assert_eq!(analyzer.hop_size(), pcm.len() / 50);
        let analysis = crate::analyze_pcm_mono(44_100, &pcm, &options).unwrap();
        assert_eq!(
            analysis.colors.len(),
            (pcm.len() - options.fft_size) / (pcm.len() / 50) + 1
        );
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
        )
        .unwrap();
        let global = crate::analyze_pcm_mono(
            sample_rate,
            &pcm,
            &GenerateOptions {
                normalize_mode: NormalizeMode::GlobalPeak,
                ..GenerateOptions::default()
            },
        )
        .unwrap();

        let max_per_channel = |frames: &Vec<f64>, idx: usize| {
            let (triples, _) = frames.as_chunks::<3>();
            triples.iter().map(|f| f[idx]).fold(0.0f64, f64::max)
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
    fn new_rejects_invalid_options_instead_of_panicking() {
        let options = GenerateOptions {
            fft_size: 0,
            ..GenerateOptions::default()
        };
        assert!(matches!(
            FrameAnalyzer::new(44_100, &options, None),
            Err(MoodbarError::InvalidOptions(_))
        ));
    }

    #[test]
    #[ignore = "dev-only throughput benchmark"]
    fn bench_analysis_throughput() {
        let sample_rate = 44_100;
        let pcm = sine(440.0, sample_rate, 60.0); // 60 seconds of audio
        let options = GenerateOptions::default();

        let start = std::time::Instant::now();
        let result = crate::analyze_pcm_mono(sample_rate, &pcm, &options).unwrap();
        let elapsed = start.elapsed();
        eprintln!(
            "Analysis throughput: 60s audio processed in {:?}, frames: {}",
            elapsed,
            result.frames.len() / result.channel_count
        );
    }
}
