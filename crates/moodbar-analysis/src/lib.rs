#[cfg(feature = "png")]
use image::{ImageBuffer, ImageEncoder, Rgba};
use num_complex::Complex32;
use rustfft::FftPlanner;

use thiserror::Error;

/// Tunable DSP options used by raw and SVG rendering paths.
#[derive(Debug, Clone)]
pub struct GenerateOptions {
    pub fft_size: usize,
    pub low_cut_hz: f32,
    pub mid_cut_hz: f32,
    pub normalize_mode: NormalizeMode,
    pub deterministic_floor: f64,
    pub detection_mode: DetectionMode,
    pub frames_per_color: usize,
    pub band_edges_hz: Vec<f32>,
    pub max_target_frames: Option<usize>,
    pub playback_rate: Option<f32>,
}

impl Default for GenerateOptions {
    fn default() -> Self {
        Self {
            fft_size: 2048,
            low_cut_hz: 500.0,
            mid_cut_hz: 2000.0,
            normalize_mode: NormalizeMode::PerChannelPeak,
            deterministic_floor: 1e-12,
            detection_mode: DetectionMode::SpectralEnergy,
            frames_per_color: 1,
            band_edges_hz: vec![500.0, 2000.0],
            max_target_frames: Some(2000),
            playback_rate: None,
        }
    }
}

impl GenerateOptions {
    fn effective_band_edges(&self) -> Vec<f32> {
        if self.band_edges_hz.is_empty() {
            vec![self.low_cut_hz, self.mid_cut_hz]
        } else {
            self.band_edges_hz.clone()
        }
    }

    fn effective_nyquist_hz(&self, sample_rate: u32) -> f64 {
        let rate = self.playback_rate.unwrap_or(1.0) as f64;
        sample_rate as f64 / 2.0 * rate
    }
}

/// Band normalization strategy.
#[derive(Debug, Clone, Copy)]
pub enum NormalizeMode {
    PerChannelPeak,
    GlobalPeak,
}

/// Signal extraction strategy per FFT bin.
#[derive(Debug, Clone, Copy)]
pub enum DetectionMode {
    SpectralEnergy,
    SpectralFlux,
}

/// Non-fatal decoder diagnostics collected during analysis.
#[derive(Debug, Clone, Default)]
pub struct AnalysisDiagnostics {
    pub decode_errors: usize,
    pub zero_channel_packets: usize,
    pub truncated_frames: usize,
}

/// Renderer-agnostic analysis output.
#[derive(Debug, Clone)]
pub struct MoodbarAnalysis {
    pub channel_count: usize,
    pub frames: Vec<Vec<f64>>,
    pub colors: Vec<[u8; 3]>,
    pub diagnostics: AnalysisDiagnostics,
}

impl MoodbarAnalysis {
    /// Returns the sequence of colors as a slice of RGB values.
    pub fn colors(&self) -> &[[u8; 3]] {
        &self.colors
    }
}

/// SVG output shape presets.
#[derive(Debug, Clone, Copy)]
pub enum SvgShape {
    Strip,
    Waveform,
    SplitStacked,
    SplitWaveform,
    SplitLanes,
    SplitCentrifugal,
    SplitOverlapping,
}

/// SVG rendering options.
#[derive(Debug, Clone)]
pub struct SvgOptions {
    pub width: u32,
    pub height: u32,
    pub shape: SvgShape,
    pub background: &'static str,
    pub max_gradient_stops: usize,
}

/// PNG rendering options.
#[cfg(feature = "png")]
#[derive(Debug, Clone)]
pub struct PngOptions {
    pub width: u32,
    pub height: u32,
    pub shape: SvgShape,
}

#[cfg(feature = "png")]
impl Default for PngOptions {
    fn default() -> Self {
        Self {
            width: 1200,
            height: 96,
            shape: SvgShape::Strip,
        }
    }
}

impl Default for SvgOptions {
    fn default() -> Self {
        Self {
            width: 1200,
            height: 96,
            shape: SvgShape::Strip,
            background: "transparent",
            max_gradient_stops: 512,
        }
    }
}

/// Errors returned by analysis/decoding APIs.
#[derive(Debug, Error)]
pub enum MoodbarError {
    #[cfg(feature = "png")]
    #[error("image error: {0}")]
    Image(#[from] image::ImageError),
    #[error("invalid options: {0}")]
    InvalidOptions(String),
}

/// Analyze already-decoded mono PCM samples.
pub fn analyze_pcm_mono(
    sample_rate: u32,
    samples: &[f32],
    options: &GenerateOptions,
) -> MoodbarAnalysis {
    let mut analyzer = FrameAnalyzer::new(sample_rate, options, Some(samples.len()));
    analyzer.feed_mono_samples(samples);
    analyzer.finish()
}

struct FrameAnalyzer<'a> {
    options: &'a GenerateOptions,
    fft_size: usize,
    hop_size: usize,
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
    fn new(sample_rate: u32, options: &'a GenerateOptions, total_samples: Option<usize>) -> Self {
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

    fn feed_mono_samples(&mut self, samples: &[f32]) {
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

    fn finish(mut self) -> MoodbarAnalysis {
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
        let colors = normalized
            .iter()
            .map(|frame| {
                let (r, g, b) = frame_to_rgb(frame);
                [scale_to_u8(r), scale_to_u8(g), scale_to_u8(b)]
            })
            .collect();
        MoodbarAnalysis {
            channel_count: self.channel_count,
            frames: normalized,
            colors,
            diagnostics: AnalysisDiagnostics::default(),
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

pub fn analysis_to_raw_rgb_bytes(analysis: &MoodbarAnalysis) -> Vec<u8> {
    let mut out = Vec::<u8>::with_capacity(analysis.colors.len() * 3);
    for color in &analysis.colors {
        out.push(color[0]);
        out.push(color[1]);
        out.push(color[2]);
    }
    out
}

const SVG_WAVEFORM_FILL_OPACITY: f64 = 0.78;
const SVG_WAVEFORM_STROKE_OPACITY: f64 = 0.95;
const SVG_WAVEFORM_STROKE_WIDTH: f64 = 1.60;
const SVG_ESTIMATED_BYTES_PER_FRAME: usize = 96;

/// Render analyzed frames as SVG output.
pub fn render_svg(analysis: &MoodbarAnalysis, options: &SvgOptions) -> String {
    let width = options.width.max(1);
    let height = options.height.max(1);
    let count = analysis.frames.len().max(1) as f64;
    let step = width as f64 / count;

    let mut s = String::with_capacity(
        analysis
            .frames
            .len()
            .saturating_mul(SVG_ESTIMATED_BYTES_PER_FRAME),
    );
    s.push_str(&format!(
        "<svg xmlns=\"http://www.w3.org/2000/svg\" viewBox=\"0 0 {width} {height}\" width=\"{width}\" height=\"{height}\">"
    ));
    s.push_str(&format!(
        "<rect x=\"0\" y=\"0\" width=\"{width}\" height=\"{height}\" fill=\"{}\"/>",
        options.background
    ));

    // Build a mood-derived gradient so non-strip shapes still preserve timeline color semantics.
    let gradient_id = "mood-gradient";
    s.push_str(&format!(
        "<defs><linearGradient id=\"{gradient_id}\" x1=\"0\" y1=\"0\" x2=\"{width}\" y2=\"0\" gradientUnits=\"userSpaceOnUse\">"
    ));
    for i in gradient_stop_indices(analysis.frames.len(), options.max_gradient_stops.max(2)) {
        let frame = &analysis.frames[i];
        let denom = (analysis.frames.len().saturating_sub(1)).max(1) as f64;
        let offset = (i as f64 / denom) * 100.0;
        let (r, g, b) = frame_to_svg_rgb(frame);
        s.push_str(&format!(
            "<stop offset=\"{offset:.3}%\" stop-color=\"rgb({},{},{})\"/>",
            r, g, b
        ));
    }
    s.push_str("</linearGradient></defs>");

    match options.shape {
        SvgShape::Strip => {
            for (i, frame) in analysis.frames.iter().enumerate() {
                let x = i as f64 * step;
                let (r, g, b) = frame_to_rgb(frame);
                s.push_str(&format!(
                    "<rect x=\"{:.6}\" y=\"0\" width=\"{:.6}\" height=\"{}\" fill=\"rgb({},{},{})\"/>",
                    x,
                    step + 0.5,
                    height,
                    scale_to_u8(r),
                    scale_to_u8(g),
                    scale_to_u8(b)
                ));
            }
        }
        SvgShape::Waveform => {
            let mid = height as f64 / 2.0;
            let mut d = String::new();
            for (i, frame) in analysis.frames.iter().enumerate() {
                let x = i as f64 * step;
                let energy =
                    (frame.iter().sum::<f64>() / frame.len().max(1) as f64).clamp(0.0, 1.0);
                let amp = energy * mid * 0.95;
                let y = mid - amp;
                if i == 0 {
                    d.push_str(&format!("M {:.6} {:.6}", x, y));
                } else {
                    d.push_str(&format!(" L {:.6} {:.6}", x, y));
                }
            }
            for i in (0..analysis.frames.len()).rev() {
                let x = i as f64 * step;
                let frame = &analysis.frames[i];
                let energy =
                    (frame.iter().sum::<f64>() / frame.len().max(1) as f64).clamp(0.0, 1.0);
                let amp = energy * mid * 0.95;
                let y = mid + amp;
                d.push_str(&format!(" L {:.6} {:.6}", x, y));
            }
            d.push_str(" Z");
            s.push_str(&format!(
                "<path d=\"{}\" fill=\"url(#{})\" fill-opacity=\"{:.2}\" stroke=\"url(#{})\" stroke-opacity=\"{:.2}\" stroke-width=\"{:.2}\" vector-effect=\"non-scaling-stroke\" stroke-linecap=\"round\" stroke-linejoin=\"round\" shape-rendering=\"geometricPrecision\"/>",
                d,
                gradient_id,
                SVG_WAVEFORM_FILL_OPACITY,
                gradient_id,
                SVG_WAVEFORM_STROKE_OPACITY,
                SVG_WAVEFORM_STROKE_WIDTH
            ));
        }
        SvgShape::SplitStacked => {
            let h_seg = height as f64 / 3.0;
            for (i, frame) in analysis.frames.iter().enumerate() {
                let x = i as f64 * step;

                let b = frame.first().copied().unwrap_or(0.0).clamp(0.0, 1.0);
                let g = frame.get(1).copied().unwrap_or(0.0).clamp(0.0, 1.0);
                let t = frame.get(2).copied().unwrap_or(0.0).clamp(0.0, 1.0);

                let h_b = h_seg * b;
                let h_g = h_seg * g;
                let h_t = h_seg * t;

                if h_b > 0.0 {
                    let y_b = height as f64 - h_b;
                    let r = (220.0 * b).round() as u8;
                    let gr = (20.0 * b).round() as u8;
                    let bl = (180.0 * b).round() as u8;
                    s.push_str(&format!(
                        "<rect x=\"{:.6}\" y=\"{:.6}\" width=\"{:.6}\" height=\"{:.6}\" fill=\"rgb({},{},{})\"/>",
                        x, y_b, step + 0.5, h_b, r, gr, bl
                    ));
                }

                if h_g > 0.0 {
                    let y_g = height as f64 - h_b - h_g;
                    let r = (240.0 * g).round() as u8;
                    let gr = (120.0 * g).round() as u8;
                    let bl = (0.0 * g).round() as u8;
                    s.push_str(&format!(
                        "<rect x=\"{:.6}\" y=\"{:.6}\" width=\"{:.6}\" height=\"{:.6}\" fill=\"rgb({},{},{})\"/>",
                        x, y_g, step + 0.5, h_g, r, gr, bl
                    ));
                }

                if h_t > 0.0 {
                    let y_t = height as f64 - h_b - h_g - h_t;
                    let r = (0.0 * t).round() as u8;
                    let gr = (160.0 * t).round() as u8;
                    let bl = (240.0 * t).round() as u8;
                    s.push_str(&format!(
                        "<rect x=\"{:.6}\" y=\"{:.6}\" width=\"{:.6}\" height=\"{:.6}\" fill=\"rgb({},{},{})\"/>",
                        x, y_t, step + 0.5, h_t, r, gr, bl
                    ));
                }
            }
        }
        SvgShape::SplitWaveform => {
            let mid = height as f64 / 2.0;
            let h_seg = height as f64 / 3.0;
            for (i, frame) in analysis.frames.iter().enumerate() {
                let x = i as f64 * step;

                let b = frame.first().copied().unwrap_or(0.0).clamp(0.0, 1.0);
                let g = frame.get(1).copied().unwrap_or(0.0).clamp(0.0, 1.0);
                let t = frame.get(2).copied().unwrap_or(0.0).clamp(0.0, 1.0);

                let h_b = h_seg * b;
                let h_g = h_seg * g;
                let h_t = h_seg * t;

                let y_b = mid - h_b / 2.0;

                if h_b > 0.0 {
                    let r = (220.0 * b).round() as u8;
                    let gr = (20.0 * b).round() as u8;
                    let bl = (180.0 * b).round() as u8;
                    s.push_str(&format!(
                        "<rect x=\"{:.6}\" y=\"{:.6}\" width=\"{:.6}\" height=\"{:.6}\" fill=\"rgb({},{},{})\"/>",
                        x, y_b, step + 0.5, h_b, r, gr, bl
                    ));
                }

                if h_g > 0.0 {
                    let h_g_half = h_g / 2.0;
                    let r = (240.0 * g).round() as u8;
                    let gr = (120.0 * g).round() as u8;
                    let bl = (0.0 * g).round() as u8;
                    s.push_str(&format!(
                        "<rect x=\"{:.6}\" y=\"{:.6}\" width=\"{:.6}\" height=\"{:.6}\" fill=\"rgb({},{},{})\"/>",
                        x, y_b - h_g_half, step + 0.5, h_g_half, r, gr, bl
                    ));
                    s.push_str(&format!(
                        "<rect x=\"{:.6}\" y=\"{:.6}\" width=\"{:.6}\" height=\"{:.6}\" fill=\"rgb({},{},{})\"/>",
                        x, y_b + h_b, step + 0.5, h_g_half, r, gr, bl
                    ));
                }

                if h_t > 0.0 {
                    let h_t_half = h_t / 2.0;
                    let h_g_half = h_g / 2.0;
                    let r = (0.0 * t).round() as u8;
                    let gr = (160.0 * t).round() as u8;
                    let bl = (240.0 * t).round() as u8;
                    s.push_str(&format!(
                        "<rect x=\"{:.6}\" y=\"{:.6}\" width=\"{:.6}\" height=\"{:.6}\" fill=\"rgb({},{},{})\"/>",
                        x, y_b - h_g_half - h_t_half, step + 0.5, h_t_half, r, gr, bl
                    ));
                    s.push_str(&format!(
                        "<rect x=\"{:.6}\" y=\"{:.6}\" width=\"{:.6}\" height=\"{:.6}\" fill=\"rgb({},{},{})\"/>",
                        x, y_b + h_b + h_g_half, step + 0.5, h_t_half, r, gr, bl
                    ));
                }
            }
        }
        SvgShape::SplitLanes => {
            let g_val = (height as f64 * 0.05).max(1.0).round();
            let h_lane = (height as f64 - 2.0 * g_val) / 3.0;

            let y_t_bottom = h_lane;
            let y_g_bottom = 2.0 * h_lane + g_val;
            let y_b_bottom = height as f64;

            for (i, frame) in analysis.frames.iter().enumerate() {
                let x = i as f64 * step;

                let b = frame.first().copied().unwrap_or(0.0).clamp(0.0, 1.0);
                let g = frame.get(1).copied().unwrap_or(0.0).clamp(0.0, 1.0);
                let t = frame.get(2).copied().unwrap_or(0.0).clamp(0.0, 1.0);

                let h_b = h_lane * b;
                let h_g = h_lane * g;
                let h_t = h_lane * t;

                if h_b > 0.0 {
                    let r = (220.0 * b).round() as u8;
                    let gr = (20.0 * b).round() as u8;
                    let bl = (180.0 * b).round() as u8;
                    s.push_str(&format!(
                        "<rect x=\"{:.6}\" y=\"{:.6}\" width=\"{:.6}\" height=\"{:.6}\" fill=\"rgb({},{},{})\"/>",
                        x, y_b_bottom - h_b, step + 0.5, h_b, r, gr, bl
                    ));
                }

                if h_g > 0.0 {
                    let r = (240.0 * g).round() as u8;
                    let gr = (120.0 * g).round() as u8;
                    let bl = (0.0 * g).round() as u8;
                    s.push_str(&format!(
                        "<rect x=\"{:.6}\" y=\"{:.6}\" width=\"{:.6}\" height=\"{:.6}\" fill=\"rgb({},{},{})\"/>",
                        x, y_g_bottom - h_g, step + 0.5, h_g, r, gr, bl
                    ));
                }

                if h_t > 0.0 {
                    let r = (0.0 * t).round() as u8;
                    let gr = (160.0 * t).round() as u8;
                    let bl = (240.0 * t).round() as u8;
                    s.push_str(&format!(
                        "<rect x=\"{:.6}\" y=\"{:.6}\" width=\"{:.6}\" height=\"{:.6}\" fill=\"rgb({},{},{})\"/>",
                        x, y_t_bottom - h_t, step + 0.5, h_t, r, gr, bl
                    ));
                }
            }
        }
        SvgShape::SplitCentrifugal => {
            let mid = height as f64 / 2.0;
            let h_seg = height as f64 / 3.0;
            for (i, frame) in analysis.frames.iter().enumerate() {
                let x = i as f64 * step;

                let b = frame.first().copied().unwrap_or(0.0).clamp(0.0, 1.0);
                let g = frame.get(1).copied().unwrap_or(0.0).clamp(0.0, 1.0);
                let t = frame.get(2).copied().unwrap_or(0.0).clamp(0.0, 1.0);

                let h_b = h_seg * b;
                let h_g = h_seg * g;
                let h_t = h_seg * t;

                let y_t = mid - h_t / 2.0;

                if h_t > 0.0 {
                    let r = (0.0 * t).round() as u8;
                    let gr = (160.0 * t).round() as u8;
                    let bl = (240.0 * t).round() as u8;
                    s.push_str(&format!(
                        "<rect x=\"{:.6}\" y=\"{:.6}\" width=\"{:.6}\" height=\"{:.6}\" fill=\"rgb({},{},{})\"/>",
                        x, y_t, step + 0.5, h_t, r, gr, bl
                    ));
                }

                if h_g > 0.0 {
                    let h_g_half = h_g / 2.0;
                    let h_t_half = h_t / 2.0;
                    let r = (240.0 * g).round() as u8;
                    let gr = (120.0 * g).round() as u8;
                    let bl = (0.0 * g).round() as u8;
                    s.push_str(&format!(
                        "<rect x=\"{:.6}\" y=\"{:.6}\" width=\"{:.6}\" height=\"{:.6}\" fill=\"rgb({},{},{})\"/>",
                        x, mid - h_t_half - h_g_half, step + 0.5, h_g_half, r, gr, bl
                    ));
                    s.push_str(&format!(
                        "<rect x=\"{:.6}\" y=\"{:.6}\" width=\"{:.6}\" height=\"{:.6}\" fill=\"rgb({},{},{})\"/>",
                        x, mid + h_t_half, step + 0.5, h_g_half, r, gr, bl
                    ));
                }

                if h_b > 0.0 {
                    let h_b_half = h_b / 2.0;
                    let h_g_half = h_g / 2.0;
                    let h_t_half = h_t / 2.0;
                    let r = (220.0 * b).round() as u8;
                    let gr = (20.0 * b).round() as u8;
                    let bl = (180.0 * b).round() as u8;
                    s.push_str(&format!(
                        "<rect x=\"{:.6}\" y=\"{:.6}\" width=\"{:.6}\" height=\"{:.6}\" fill=\"rgb({},{},{})\"/>",
                        x, mid - h_t_half - h_g_half - h_b_half, step + 0.5, h_b_half, r, gr, bl
                    ));
                    s.push_str(&format!(
                        "<rect x=\"{:.6}\" y=\"{:.6}\" width=\"{:.6}\" height=\"{:.6}\" fill=\"rgb({},{},{})\"/>",
                        x, mid + h_t_half + h_g_half, step + 0.5, h_b_half, r, gr, bl
                    ));
                }
            }
        }
        SvgShape::SplitOverlapping => {
            for (i, frame) in analysis.frames.iter().enumerate() {
                let x = i as f64 * step;

                let b = frame.first().copied().unwrap_or(0.0).clamp(0.0, 1.0);
                let g = frame.get(1).copied().unwrap_or(0.0).clamp(0.0, 1.0);
                let t = frame.get(2).copied().unwrap_or(0.0).clamp(0.0, 1.0);

                let h_b = height as f64 * b;
                let h_g = height as f64 * g;
                let h_t = height as f64 * t;

                if h_b > 0.0 {
                    let r = (220.0 * b).round() as u8;
                    let gr = (20.0 * b).round() as u8;
                    let bl = (180.0 * b).round() as u8;
                    s.push_str(&format!(
                        "<rect x=\"{:.6}\" y=\"{:.6}\" width=\"{:.6}\" height=\"{:.6}\" fill=\"rgb({},{},{})\" fill-opacity=\"0.40\"/>",
                        x, height as f64 - h_b, step + 0.5, h_b, r, gr, bl
                    ));
                }

                if h_g > 0.0 {
                    let r = (240.0 * g).round() as u8;
                    let gr = (120.0 * g).round() as u8;
                    let bl = (0.0 * g).round() as u8;
                    s.push_str(&format!(
                        "<rect x=\"{:.6}\" y=\"{:.6}\" width=\"{:.6}\" height=\"{:.6}\" fill=\"rgb({},{},{})\" fill-opacity=\"0.40\"/>",
                        x, height as f64 - h_g, step + 0.5, h_g, r, gr, bl
                    ));
                }

                if h_t > 0.0 {
                    let r = (0.0 * t).round() as u8;
                    let gr = (160.0 * t).round() as u8;
                    let bl = (240.0 * t).round() as u8;
                    s.push_str(&format!(
                        "<rect x=\"{:.6}\" y=\"{:.6}\" width=\"{:.6}\" height=\"{:.6}\" fill=\"rgb({},{},{})\" fill-opacity=\"0.40\"/>",
                        x, height as f64 - h_t, step + 0.5, h_t, r, gr, bl
                    ));
                }
            }
        }
    }

    s.push_str("</svg>");
    s
}

/// Render analyzed frames as PNG bytes.
#[cfg(feature = "png")]
pub fn render_png(
    analysis: &MoodbarAnalysis,
    options: &PngOptions,
) -> Result<Vec<u8>, MoodbarError> {
    let width = options.width.max(1);
    let height = options.height.max(1);
    let mut img: ImageBuffer<Rgba<u8>, Vec<u8>> =
        ImageBuffer::from_pixel(width, height, Rgba([0, 0, 0, 0]));

    if analysis.frames.is_empty() {
        let mut out = Vec::new();
        let encoder = image::codecs::png::PngEncoder::new(&mut out);
        encoder.write_image(img.as_raw(), width, height, image::ColorType::Rgba8.into())?;
        return Ok(out);
    }

    let len = analysis.frames.len();
    let frame_at_x = |x: u32| -> usize {
        (((x as f64 / width as f64) * len as f64).floor() as usize).min(len - 1)
    };

    match options.shape {
        SvgShape::Strip => {
            for x in 0..width {
                let idx = frame_at_x(x);
                let (r, g, b) = frame_to_svg_rgb(&analysis.frames[idx]);
                for y in 0..height {
                    img.put_pixel(x, y, Rgba([r, g, b, 255]));
                }
            }
        }
        SvgShape::Waveform => {
            let mid = height as f64 / 2.0;
            for x in 0..width {
                let idx = frame_at_x(x);
                let frame = &analysis.frames[idx];
                let energy =
                    (frame.iter().sum::<f64>() / frame.len().max(1) as f64).clamp(0.0, 1.0);
                let amp = energy * mid * 0.95;
                let y_top = (mid - amp).max(0.0).floor() as u32;
                let y_bottom = (mid + amp).min((height - 1) as f64).ceil() as u32;
                let (r, g, b) = frame_to_svg_rgb(frame);
                for y in y_top..=y_bottom {
                    let alpha = if y == y_top || y == y_bottom {
                        255
                    } else {
                        210
                    };
                    img.put_pixel(x, y, Rgba([r, g, b, alpha]));
                }
            }
        }
        SvgShape::SplitStacked => {
            let h_seg = height as f64 / 3.0;
            for x in 0..width {
                let idx = frame_at_x(x);
                let frame = &analysis.frames[idx];
                let b = frame.first().copied().unwrap_or(0.0).clamp(0.0, 1.0);
                let g = frame.get(1).copied().unwrap_or(0.0).clamp(0.0, 1.0);
                let t = frame.get(2).copied().unwrap_or(0.0).clamp(0.0, 1.0);

                let h_b = h_seg * b;
                let h_g = h_seg * g;
                let h_t = h_seg * t;

                let y_bass_start = height as f64 - h_b;
                let y_mid_start = y_bass_start - h_g;
                let y_treble_start = y_mid_start - h_t;

                let r_b = (220.0 * b).round() as u8;
                let g_b = (20.0 * b).round() as u8;
                let b_b = (180.0 * b).round() as u8;

                let r_g = (240.0 * g).round() as u8;
                let g_g = (120.0 * g).round() as u8;
                let b_g = (0.0 * g).round() as u8;

                let r_t = (0.0 * t).round() as u8;
                let g_t = (160.0 * t).round() as u8;
                let b_t = (240.0 * t).round() as u8;

                for y in 0..height {
                    let y_f = y as f64;
                    if y_f >= y_bass_start {
                        img.put_pixel(x, y, Rgba([r_b, g_b, b_b, 255]));
                    } else if y_f >= y_mid_start {
                        img.put_pixel(x, y, Rgba([r_g, g_g, b_g, 255]));
                    } else if y_f >= y_treble_start {
                        img.put_pixel(x, y, Rgba([r_t, g_t, b_t, 255]));
                    }
                }
            }
        }
        SvgShape::SplitWaveform => {
            let mid = height as f64 / 2.0;
            let h_seg = height as f64 / 3.0;
            for x in 0..width {
                let idx = frame_at_x(x);
                let frame = &analysis.frames[idx];
                let b = frame.first().copied().unwrap_or(0.0).clamp(0.0, 1.0);
                let g = frame.get(1).copied().unwrap_or(0.0).clamp(0.0, 1.0);
                let t = frame.get(2).copied().unwrap_or(0.0).clamp(0.0, 1.0);

                let h_b = h_seg * b;
                let h_g = h_seg * g;
                let h_t = h_seg * t;

                let y_b_start = mid - h_b / 2.0;
                let y_b_end = mid + h_b / 2.0;

                let h_g_half = h_g / 2.0;
                let h_t_half = h_t / 2.0;

                let y_g_top_start = y_b_start - h_g_half;
                let y_t_top_start = y_g_top_start - h_t_half;

                let y_g_bot_end = y_b_end + h_g_half;
                let y_t_bot_end = y_g_bot_end + h_t_half;

                let r_b = (220.0 * b).round() as u8;
                let g_b = (20.0 * b).round() as u8;
                let b_b = (180.0 * b).round() as u8;

                let r_g = (240.0 * g).round() as u8;
                let g_g = (120.0 * g).round() as u8;
                let b_g = (0.0 * g).round() as u8;

                let r_t = (0.0 * t).round() as u8;
                let g_t = (160.0 * t).round() as u8;
                let b_t = (240.0 * t).round() as u8;

                for y in 0..height {
                    let y_f = y as f64;
                    if y_f >= y_b_start && y_f < y_b_end {
                        img.put_pixel(x, y, Rgba([r_b, g_b, b_b, 255]));
                    } else if (y_f >= y_g_top_start && y_f < y_b_start)
                        || (y_f >= y_b_end && y_f < y_g_bot_end)
                    {
                        img.put_pixel(x, y, Rgba([r_g, g_g, b_g, 255]));
                    } else if (y_f >= y_t_top_start && y_f < y_g_top_start)
                        || (y_f >= y_g_bot_end && y_f < y_t_bot_end)
                    {
                        img.put_pixel(x, y, Rgba([r_t, g_t, b_t, 255]));
                    }
                }
            }
        }
        SvgShape::SplitLanes => {
            let g_val = (height as f64 * 0.05).max(1.0).round();
            let h_lane = (height as f64 - 2.0 * g_val) / 3.0;

            let y_t_bottom = h_lane;
            let y_g_bottom = 2.0 * h_lane + g_val;
            let y_b_bottom = height as f64;

            for x in 0..width {
                let idx = frame_at_x(x);
                let frame = &analysis.frames[idx];
                let b = frame.first().copied().unwrap_or(0.0).clamp(0.0, 1.0);
                let g = frame.get(1).copied().unwrap_or(0.0).clamp(0.0, 1.0);
                let t = frame.get(2).copied().unwrap_or(0.0).clamp(0.0, 1.0);

                let h_b = h_lane * b;
                let h_g = h_lane * g;
                let h_t = h_lane * t;

                let y_b_start = y_b_bottom - h_b;
                let y_g_start = y_g_bottom - h_g;
                let y_t_start = y_t_bottom - h_t;

                let r_b = (220.0 * b).round() as u8;
                let g_b = (20.0 * b).round() as u8;
                let b_b = (180.0 * b).round() as u8;

                let r_g = (240.0 * g).round() as u8;
                let g_g = (120.0 * g).round() as u8;
                let b_g = (0.0 * g).round() as u8;

                let r_t = (0.0 * t).round() as u8;
                let g_t = (160.0 * t).round() as u8;
                let b_t = (240.0 * t).round() as u8;

                for y in 0..height {
                    let y_f = y as f64;
                    if y_f >= y_b_start && y_f < y_b_bottom {
                        img.put_pixel(x, y, Rgba([r_b, g_b, b_b, 255]));
                    } else if y_f >= y_g_start && y_f < y_g_bottom {
                        img.put_pixel(x, y, Rgba([r_g, g_g, b_g, 255]));
                    } else if y_f >= y_t_start && y_f < y_t_bottom {
                        img.put_pixel(x, y, Rgba([r_t, g_t, b_t, 255]));
                    }
                }
            }
        }
        SvgShape::SplitCentrifugal => {
            let mid = height as f64 / 2.0;
            let h_seg = height as f64 / 3.0;
            for x in 0..width {
                let idx = frame_at_x(x);
                let frame = &analysis.frames[idx];
                let b = frame.first().copied().unwrap_or(0.0).clamp(0.0, 1.0);
                let g = frame.get(1).copied().unwrap_or(0.0).clamp(0.0, 1.0);
                let t = frame.get(2).copied().unwrap_or(0.0).clamp(0.0, 1.0);

                let h_b = h_seg * b;
                let h_g = h_seg * g;
                let h_t = h_seg * t;

                let y_t_start = mid - h_t / 2.0;
                let y_t_end = mid + h_t / 2.0;

                let h_g_half = h_g / 2.0;
                let h_b_half = h_b / 2.0;
                let h_t_half = h_t / 2.0;

                let y_g_top_start = mid - h_t_half - h_g_half;
                let y_g_bot_end = mid + h_t_half + h_g_half;

                let y_b_top_start = y_g_top_start - h_b_half;
                let y_b_bot_end = y_g_bot_end + h_b_half;

                let r_b = (220.0 * b).round() as u8;
                let g_b = (20.0 * b).round() as u8;
                let b_b = (180.0 * b).round() as u8;

                let r_g = (240.0 * g).round() as u8;
                let g_g = (120.0 * g).round() as u8;
                let b_g = (0.0 * g).round() as u8;

                let r_t = (0.0 * t).round() as u8;
                let g_t = (160.0 * t).round() as u8;
                let b_t = (240.0 * t).round() as u8;

                for y in 0..height {
                    let y_f = y as f64;
                    if y_f >= y_t_start && y_f < y_t_end {
                        img.put_pixel(x, y, Rgba([r_t, g_t, b_t, 255]));
                    } else if (y_f >= y_g_top_start && y_f < y_t_start)
                        || (y_f >= y_t_end && y_f < y_g_bot_end)
                    {
                        img.put_pixel(x, y, Rgba([r_g, g_g, b_g, 255]));
                    } else if (y_f >= y_b_top_start && y_f < y_g_top_start)
                        || (y_f >= y_g_bot_end && y_f < y_b_bot_end)
                    {
                        img.put_pixel(x, y, Rgba([r_b, g_b, b_b, 255]));
                    }
                }
            }
        }
        SvgShape::SplitOverlapping => {
            let blend_px = |bg: Rgba<u8>, fg: Rgba<u8>| -> Rgba<u8> {
                let alpha_fg = fg.0[3] as f32 / 255.0;
                let alpha_bg = bg.0[3] as f32 / 255.0;
                let out_alpha = alpha_fg + alpha_bg * (1.0 - alpha_fg);
                if out_alpha <= 0.0 {
                    return Rgba([0, 0, 0, 0]);
                }
                let blend = |c_fg: u8, c_bg: u8| -> u8 {
                    let f = c_fg as f32 / 255.0;
                    let b = c_bg as f32 / 255.0;
                    let out = (f * alpha_fg + b * alpha_bg * (1.0 - alpha_fg)) / out_alpha;
                    (out * 255.0).round() as u8
                };
                Rgba([
                    blend(fg.0[0], bg.0[0]),
                    blend(fg.0[1], bg.0[1]),
                    blend(fg.0[2], bg.0[2]),
                    (out_alpha * 255.0).round() as u8,
                ])
            };

            for x in 0..width {
                let idx = frame_at_x(x);
                let frame = &analysis.frames[idx];
                let b = frame.first().copied().unwrap_or(0.0).clamp(0.0, 1.0);
                let g = frame.get(1).copied().unwrap_or(0.0).clamp(0.0, 1.0);
                let t = frame.get(2).copied().unwrap_or(0.0).clamp(0.0, 1.0);

                let h_b = height as f64 * b;
                let h_g = height as f64 * g;
                let h_t = height as f64 * t;

                let y_b_start = height as f64 - h_b;
                let y_g_start = height as f64 - h_g;
                let y_t_start = height as f64 - h_t;

                let r_b = (220.0 * b).round() as u8;
                let g_b = (20.0 * b).round() as u8;
                let b_b = (180.0 * b).round() as u8;

                let r_g = (240.0 * g).round() as u8;
                let g_g = (120.0 * g).round() as u8;
                let b_g = (0.0 * g).round() as u8;

                let r_t = (0.0 * t).round() as u8;
                let g_t = (160.0 * t).round() as u8;
                let b_t = (240.0 * t).round() as u8;

                for y in 0..height {
                    let y_f = y as f64;
                    let mut current = Rgba([0, 0, 0, 0]);

                    if h_b > 0.0 && y_f >= y_b_start {
                        let fg = Rgba([r_b, g_b, b_b, 102]);
                        current = blend_px(current, fg);
                    }

                    if h_g > 0.0 && y_f >= y_g_start {
                        let fg = Rgba([r_g, g_g, b_g, 102]);
                        current = blend_px(current, fg);
                    }

                    if h_t > 0.0 && y_f >= y_t_start {
                        let fg = Rgba([r_t, g_t, b_t, 102]);
                        current = blend_px(current, fg);
                    }

                    if current.0[3] > 0 {
                        img.put_pixel(x, y, current);
                    }
                }
            }
        }
    }

    let mut out = Vec::new();
    let encoder = image::codecs::png::PngEncoder::new(&mut out);
    encoder.write_image(img.as_raw(), width, height, image::ColorType::Rgba8.into())?;
    Ok(out)
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

fn band_index(freq_hz: f64, edges_hz: &[f32]) -> usize {
    for (i, edge) in edges_hz.iter().enumerate() {
        if freq_hz < *edge as f64 {
            return i;
        }
    }
    edges_hz.len()
}

fn build_bin_to_band(fft_size: usize, nyquist: f64, edges_hz: &[f32]) -> Vec<usize> {
    (0..fft_size / 2)
        .map(|k| {
            let freq = (k as f64 / (fft_size as f64 / 2.0)) * nyquist;
            band_index(freq, edges_hz)
        })
        .collect()
}

fn gradient_stop_indices(frame_count: usize, max_stops: usize) -> Vec<usize> {
    if frame_count == 0 {
        return Vec::new();
    }
    if frame_count <= max_stops {
        return (0..frame_count).collect();
    }

    let mut out = Vec::with_capacity(max_stops);
    let denom = (max_stops - 1) as f64;
    let max_idx = (frame_count - 1) as f64;
    for i in 0..max_stops {
        let idx = ((i as f64 / denom) * max_idx).round() as usize;
        if out.last().copied() != Some(idx) {
            out.push(idx);
        }
    }
    if out.last().copied() != Some(frame_count - 1) {
        out.push(frame_count - 1);
    }
    out
}

fn aggregate_frames(frames: &[Vec<f64>], frames_per_color: usize) -> Vec<Vec<f64>> {
    if frames.is_empty() || frames_per_color <= 1 {
        return frames.to_vec();
    }

    let channels = frames[0].len();
    let mut out = Vec::new();
    for chunk in frames.chunks(frames_per_color) {
        let mut acc = vec![0.0f64; channels];
        for frame in chunk {
            for (i, v) in frame.iter().enumerate() {
                acc[i] += *v;
            }
        }
        let denom = chunk.len() as f64;
        for v in &mut acc {
            *v /= denom;
        }
        out.push(acc);
    }
    out
}

fn normalize_frames(frames: &[Vec<f64>], options: &GenerateOptions) -> Vec<Vec<f64>> {
    if frames.is_empty() {
        return Vec::new();
    }

    let channels = frames[0].len();
    let floor = options.deterministic_floor;
    let mut per_channel_max = vec![floor; channels];
    for frame in frames {
        for (i, v) in frame.iter().enumerate() {
            per_channel_max[i] = per_channel_max[i].max(*v);
        }
    }
    let global_max = per_channel_max.iter().copied().fold(floor, f64::max);

    frames
        .iter()
        .map(|frame| {
            frame
                .iter()
                .enumerate()
                .map(|(i, v)| {
                    let denom = match options.normalize_mode {
                        NormalizeMode::PerChannelPeak => per_channel_max[i],
                        NormalizeMode::GlobalPeak => global_max,
                    };
                    (*v / denom).clamp(0.0, 1.0)
                })
                .collect::<Vec<_>>()
        })
        .collect()
}

fn frame_to_rgb(frame: &[f64]) -> (f64, f64, f64) {
    if frame.is_empty() {
        return (0.0, 0.0, 0.0);
    }
    if frame.len() == 1 {
        let v = frame[0];
        return (v, v, v);
    }
    if frame.len() == 2 {
        return (frame[0], frame[1], 0.0);
    }
    if frame.len() == 3 {
        return (frame[0], frame[1], frame[2]);
    }

    // For >3 channels, map dominant channel to hue while preserving total intensity.
    let mut max_idx = 0usize;
    let mut max_val = frame[0];
    let mut sum = 0.0f64;
    for (i, v) in frame.iter().enumerate() {
        sum += *v;
        if *v > max_val {
            max_idx = i;
            max_val = *v;
        }
    }
    let intensity = (sum / frame.len() as f64).clamp(0.0, 1.0);
    let hue = max_idx as f64 / frame.len() as f64;
    hsv_to_rgb(hue, 0.85, intensity)
}

fn frame_to_svg_rgb(frame: &[f64]) -> (u8, u8, u8) {
    let (r, g, b) = frame_to_rgb(frame);
    let peak = r.max(g).max(b);
    if peak <= 1e-12 {
        return (0, 0, 0);
    }

    // Keep hue from channel ratios but increase chroma for clearer default SVG rendering.
    let sr = (r / peak).clamp(0.0, 1.0);
    let sg = (g / peak).clamp(0.0, 1.0);
    let sb = (b / peak).clamp(0.0, 1.0);
    let brightness = (0.30 + 0.70 * peak).clamp(0.0, 1.0);

    (
        scale_to_u8(sr * brightness),
        scale_to_u8(sg * brightness),
        scale_to_u8(sb * brightness),
    )
}

fn hsv_to_rgb(h: f64, s: f64, v: f64) -> (f64, f64, f64) {
    let h = (h.fract() * 6.0).max(0.0);
    let c = v * s;
    let x = c * (1.0 - ((h % 2.0) - 1.0).abs());
    let (r1, g1, b1) = if h < 1.0 {
        (c, x, 0.0)
    } else if h < 2.0 {
        (x, c, 0.0)
    } else if h < 3.0 {
        (0.0, c, x)
    } else if h < 4.0 {
        (0.0, x, c)
    } else if h < 5.0 {
        (x, 0.0, c)
    } else {
        (c, 0.0, x)
    };
    let m = v - c;
    (r1 + m, g1 + m, b1 + m)
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
        let analysis = analyze_pcm_mono(sample_rate, &pcm, &options);
        let bytes = analysis_to_raw_rgb_bytes(&analysis);
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

    #[test]
    fn supports_more_than_three_bands() {
        let sample_rate = 44_100;
        let pcm = sine(400.0, sample_rate, 0.4);
        let options = GenerateOptions {
            band_edges_hz: vec![120.0, 500.0, 1200.0, 4000.0],
            ..GenerateOptions::default()
        };

        let analysis = analyze_pcm_mono(sample_rate, &pcm, &options);
        assert_eq!(analysis.channel_count, 5);
        assert!(!analysis.frames.is_empty());
    }

    #[test]
    fn frames_per_color_reduces_output_density() {
        let sample_rate = 44_100;
        let pcm = sine(400.0, sample_rate, 1.0);

        let baseline = analyze_pcm_mono(sample_rate, &pcm, &GenerateOptions::default());
        let dense = analyze_pcm_mono(
            sample_rate,
            &pcm,
            &GenerateOptions {
                frames_per_color: 1000,
                ..GenerateOptions::default()
            },
        );

        assert!(baseline.frames.len() > dense.frames.len());
        assert_eq!(dense.frames.len(), 1);
    }

    #[test]
    fn spectral_flux_reduces_steady_state_energy() {
        let sample_rate = 44_100;
        let pcm = sine(440.0, sample_rate, 1.0);

        let energy = analyze_pcm_mono(
            sample_rate,
            &pcm,
            &GenerateOptions {
                detection_mode: DetectionMode::SpectralEnergy,
                ..GenerateOptions::default()
            },
        );
        let flux = analyze_pcm_mono(
            sample_rate,
            &pcm,
            &GenerateOptions {
                detection_mode: DetectionMode::SpectralFlux,
                ..GenerateOptions::default()
            },
        );

        let energy_sum: f64 = energy.frames.iter().flatten().sum();
        let flux_sum: f64 = flux.frames.iter().flatten().sum();
        assert!(flux_sum < energy_sum);
    }

    #[test]
    fn global_peak_normalization_caps_all_channels_uniformly() {
        let sample_rate = 44_100;
        let mut pcm = Vec::new();
        pcm.extend(sine(100.0, sample_rate, 0.3));
        pcm.extend(sine(3000.0, sample_rate, 0.3));

        let per_channel = analyze_pcm_mono(
            sample_rate,
            &pcm,
            &GenerateOptions {
                normalize_mode: NormalizeMode::PerChannelPeak,
                ..GenerateOptions::default()
            },
        );
        let global = analyze_pcm_mono(
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

    #[test]
    fn test_playback_rate_frequency_shift() {
        let sample_rate = 44_100;
        // Near the upper mid/high boundary so rate shifts change band allocation.
        let pcm = sine(1900.0, sample_rate, 1.0);
        let stable = GenerateOptions {
            max_target_frames: None,
            ..GenerateOptions::default()
        };

        let baseline = analyze_pcm_mono(sample_rate, &pcm, &stable);
        let faster = analyze_pcm_mono(
            sample_rate,
            &pcm,
            &GenerateOptions {
                playback_rate: Some(1.10),
                ..stable.clone()
            },
        );
        let slower = analyze_pcm_mono(
            sample_rate,
            &pcm,
            &GenerateOptions {
                playback_rate: Some(0.90),
                ..stable
            },
        );

        let total_band_energy = |analysis: &MoodbarAnalysis, band: usize| -> f64 {
            analysis.frames.iter().map(|frame| frame[band]).sum()
        };

        let high_baseline = total_band_energy(&baseline, 2);
        let high_faster = total_band_energy(&faster, 2);
        let low_baseline = total_band_energy(&baseline, 0);
        let low_slower = total_band_energy(&slower, 0);

        assert!(
            high_faster > high_baseline,
            "speed-up should shift energy toward high bands: {high_faster} vs {high_baseline}"
        );
        assert!(
            low_slower > low_baseline,
            "slow-down should shift energy toward low bands: {low_slower} vs {low_baseline}"
        );
    }

    #[test]
    fn streaming_and_batch_pcm_paths_match() {
        let sample_rate = 44_100;
        let mut pcm = Vec::new();
        pcm.extend(sine(120.0, sample_rate, 0.4));
        pcm.extend(sine(900.0, sample_rate, 0.4));
        pcm.extend(sine(3300.0, sample_rate, 0.4));

        let options = GenerateOptions::default();
        let batch = analyze_pcm_mono(sample_rate, &pcm, &options);

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
    fn svg_strip_and_waveform_render() {
        let analysis = MoodbarAnalysis {
            channel_count: 3,
            frames: vec![
                vec![1.0, 0.0, 0.0],
                vec![0.0, 1.0, 0.2],
                vec![0.0, 0.1, 1.0],
            ],
            colors: vec![[255, 0, 0], [0, 255, 51], [0, 25, 255]],
            diagnostics: AnalysisDiagnostics::default(),
        };

        let strip = render_svg(
            &analysis,
            &SvgOptions {
                shape: SvgShape::Strip,
                ..SvgOptions::default()
            },
        );
        assert!(strip.contains("<svg"));
        assert!(strip.contains("<rect"));
        assert!(strip.contains("<linearGradient"));

        let waveform = render_svg(
            &analysis,
            &SvgOptions {
                shape: SvgShape::Waveform,
                ..SvgOptions::default()
            },
        );
        assert!(waveform.contains("<path"));
        assert!(waveform.contains("url(#mood-gradient)"));

        let split = render_svg(
            &analysis,
            &SvgOptions {
                shape: SvgShape::SplitStacked,
                ..SvgOptions::default()
            },
        );
        assert!(split.contains("<svg"));
        assert!(split.contains("<rect"));
        assert!(split.matches("<rect ").count() > 1);

        for shape in &[
            SvgShape::SplitWaveform,
            SvgShape::SplitLanes,
            SvgShape::SplitCentrifugal,
            SvgShape::SplitOverlapping,
        ] {
            let split_variant = render_svg(
                &analysis,
                &SvgOptions {
                    shape: *shape,
                    ..SvgOptions::default()
                },
            );
            assert!(split_variant.contains("<svg"));
            assert!(split_variant.contains("<rect"));
            assert!(split_variant.matches("<rect ").count() > 1);
        }
    }

    #[test]
    fn precomputed_bin_mapping_matches_direct_band_indexing() {
        let fft_size = 2048;
        let nyquist = 22_050.0;
        let edges = vec![120.0, 500.0, 1200.0, 3200.0, 8500.0];
        let map = build_bin_to_band(fft_size, nyquist, &edges);

        assert_eq!(map.len(), fft_size / 2);
        for (k, mapped) in map.iter().enumerate() {
            let freq = (k as f64 / (fft_size as f64 / 2.0)) * nyquist;
            let direct = band_index(freq, &edges);
            assert_eq!(*mapped, direct);
        }
    }

    #[test]
    fn svg_gradient_stop_count_is_capped() {
        let frames = (0..5000)
            .map(|i| {
                let t = i as f64 / 5000.0;
                vec![t, 1.0 - t, (0.5 + 0.5 * (t * 10.0).sin()).clamp(0.0, 1.0)]
            })
            .collect::<Vec<_>>();
        let analysis = MoodbarAnalysis {
            channel_count: 3,
            frames,
            colors: Vec::new(),
            diagnostics: AnalysisDiagnostics::default(),
        };
        let svg = render_svg(
            &analysis,
            &SvgOptions {
                max_gradient_stops: 256,
                ..SvgOptions::default()
            },
        );
        let stop_count = svg.matches("<stop ").count();
        assert!(stop_count <= 256);
        assert!(stop_count > 1);
    }

    #[cfg(feature = "png")]
    #[test]
    fn png_render_produces_valid_png_signature() {
        let analysis = MoodbarAnalysis {
            channel_count: 3,
            frames: vec![
                vec![1.0, 0.0, 0.0],
                vec![0.0, 1.0, 0.0],
                vec![0.0, 0.0, 1.0],
            ],
            colors: vec![[255, 0, 0], [0, 255, 0], [0, 0, 255]],
            diagnostics: AnalysisDiagnostics::default(),
        };
        let png = render_png(
            &analysis,
            &PngOptions {
                width: 64,
                height: 24,
                shape: SvgShape::Waveform,
            },
        )
        .expect("render png");
        assert!(png.starts_with(b"\x89PNG\r\n\x1a\n"));

        let png_split = render_png(
            &analysis,
            &PngOptions {
                width: 64,
                height: 24,
                shape: SvgShape::SplitStacked,
            },
        )
        .expect("render png split");
        assert!(png_split.starts_with(b"\x89PNG\r\n\x1a\n"));

        for shape in &[
            SvgShape::SplitWaveform,
            SvgShape::SplitLanes,
            SvgShape::SplitCentrifugal,
            SvgShape::SplitOverlapping,
        ] {
            let png_variant = render_png(
                &analysis,
                &PngOptions {
                    width: 64,
                    height: 24,
                    shape: *shape,
                },
            )
            .expect("render png variant");
            assert!(png_variant.starts_with(b"\x89PNG\r\n\x1a\n"));
        }
    }
}
