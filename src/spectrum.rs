use rustfft::{Fft, FftPlanner, num_complex::Complex};
use std::sync::Arc;

pub const SIZE: usize = 1024;
pub const RATE: usize = 22050;

pub fn bar_count(width: usize) -> usize {
    (width.saturating_sub(16) / 16).clamp(5, 30)
}

pub struct Spectrum {
    fft: Arc<dyn Fft<f64>>,
    buffer: Vec<Complex<f64>>,
    scratch: Vec<Complex<f64>>,
    window: Vec<f64>,
}
impl Default for Spectrum {
    fn default() -> Self {
        let fft = FftPlanner::<f64>::new().plan_fft_forward(SIZE);
        let scratch = vec![Complex::default(); fft.get_inplace_scratch_len()];
        Self {
            fft,
            scratch,
            buffer: vec![Complex::default(); SIZE],
            window: (0..SIZE)
                .map(|i| {
                    0.5 - 0.5 * (2.0 * std::f64::consts::PI * i as f64 / (SIZE - 1) as f64).cos()
                })
                .collect(),
        }
    }
}
impl Spectrum {
    pub fn analyze(&mut self, samples: &[f64], volume: f64, counts: &[usize]) -> Vec<Vec<f64>> {
        assert_eq!(samples.len(), SIZE);
        for (i, sample) in samples.iter().enumerate() {
            self.buffer[i] = Complex::new(sample * self.window[i], 0.0);
        }
        self.fft
            .process_with_scratch(&mut self.buffer, &mut self.scratch);
        let scale = (volume / 100.0).powi(3).max(1e-6);
        counts
            .iter()
            .map(|&count| {
                let bounds: Vec<f64> = if count == 5 {
                    vec![40., 160., 500., 1600., 5000., 10000.]
                } else {
                    (0..=count)
                        .map(|i| 40.0 * 250.0_f64.powf(i as f64 / count as f64))
                        .collect()
                };
                bounds
                    .windows(2)
                    .map(|range| {
                        let start = (range[0] * SIZE as f64 / RATE as f64).ceil() as usize;
                        let end = ((range[1] * SIZE as f64 / RATE as f64).ceil() as usize)
                            .max(start + 1)
                            .min(SIZE / 2);
                        let energy: f64 = self.buffer[start.max(1)..end]
                            .iter()
                            .map(|v| v.norm_sqr())
                            .sum();
                        let rms = energy.sqrt() * (2.0_f64 / 0.375).sqrt() / SIZE as f64 / scale;
                        ((20.0 * rms.max(1e-12).log10() + 72.0) / 57.0).clamp(0.0, 1.0)
                    })
                    .collect()
            })
            .collect()
    }
}
