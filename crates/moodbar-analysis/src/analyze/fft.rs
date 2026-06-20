pub(crate) fn hann_window(size: usize) -> Vec<f32> {
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

pub(crate) fn band_index(freq_hz: f64, edges_hz: &[f32]) -> usize {
    for (i, edge) in edges_hz.iter().enumerate() {
        if freq_hz < *edge as f64 {
            return i;
        }
    }
    edges_hz.len()
}

pub(crate) fn build_bin_to_band(fft_size: usize, nyquist: f64, edges_hz: &[f32]) -> Vec<usize> {
    (0..fft_size / 2)
        .map(|k| {
            let freq = (k as f64 / (fft_size as f64 / 2.0)) * nyquist;
            band_index(freq, edges_hz)
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

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
}
