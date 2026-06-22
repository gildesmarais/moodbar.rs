// Rust guideline compliant 2026-06-22

use crate::options::{GenerateOptions, NormalizeMode};

pub(crate) fn aggregate_frames_flat(
    frames_flat: &[f64],
    channel_count: usize,
    frames_per_color: usize,
) -> Vec<f64> {
    if frames_flat.is_empty() || frames_per_color <= 1 {
        return frames_flat.to_vec();
    }

    let chunk_size = frames_per_color * channel_count;
    let capacity = frames_flat.len().div_ceil(chunk_size) * channel_count;
    let mut out = Vec::with_capacity(capacity);

    for chunk in frames_flat.chunks(chunk_size) {
        let mut acc = vec![0.0f64; channel_count];
        let num_frames = chunk.len() / channel_count;
        if num_frames == 0 {
            break;
        }
        for frame in chunk.chunks_exact(channel_count) {
            for (i, &v) in frame.iter().enumerate() {
                acc[i] += v;
            }
        }
        let denom = num_frames as f64;
        for v in &mut acc {
            *v /= denom;
        }
        out.extend_from_slice(&acc);
    }
    out
}

pub(crate) fn normalize_frames_flat(
    frames_flat: &mut [f64],
    channel_count: usize,
    options: &GenerateOptions,
) {
    if frames_flat.is_empty() {
        return;
    }

    let floor = options.deterministic_floor;
    let mut per_channel_max = vec![floor; channel_count];
    for chunk in frames_flat.chunks_exact(channel_count) {
        for (i, &v) in chunk.iter().enumerate() {
            per_channel_max[i] = per_channel_max[i].max(v);
        }
    }
    let global_max = per_channel_max.iter().copied().fold(floor, f64::max);

    for chunk in frames_flat.chunks_exact_mut(channel_count) {
        for (i, v) in chunk.iter_mut().enumerate() {
            let denom = match options.normalize_mode {
                NormalizeMode::PerChannelPeak => per_channel_max[i],
                NormalizeMode::GlobalPeak => global_max,
            };
            *v = (*v / denom).clamp(0.0, 1.0);
        }
    }
}
