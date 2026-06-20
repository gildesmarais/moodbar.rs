use crate::options::{GenerateOptions, NormalizeMode};

pub(crate) fn aggregate_frames(frames: &[Vec<f64>], frames_per_color: usize) -> Vec<Vec<f64>> {
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

pub(crate) fn normalize_frames(frames: &[Vec<f64>], options: &GenerateOptions) -> Vec<Vec<f64>> {
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
