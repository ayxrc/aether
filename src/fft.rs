// Visualizer and shit
use crate::state::{SharedState, NUM_BARS};
use rustfft::{num_complex::Complex, FftPlanner};
use std::collections::VecDeque;
use std::f32::consts::PI;
use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::Duration;

pub const FFT_SIZE: usize = 2048;

pub fn spawn_fft_worker(
    sample_ring: Arc<Mutex<VecDeque<f32>>>,
    shared_state: Arc<Mutex<SharedState>>,
    sample_rate_atomic: Arc<AtomicU32>,
) {
    thread::Builder::new()
        .name("aether-fft-worker".into())
        .spawn(move || {
            let mut planner = FftPlanner::<f32>::new();
            let fft = planner.plan_fft_forward(FFT_SIZE);

            let hann_window: Vec<f32> = (0..FFT_SIZE)
                .map(|i| 0.5 * (1.0 - (2.0 * PI * i as f32 / (FFT_SIZE - 1) as f32).cos()))
                .collect();

            let mut complex_buf = vec![Complex { re: 0.0, im: 0.0 }; FFT_SIZE];
            let mut smoothed_bars = [0.0f32; NUM_BARS];
            let mut peak_bars = [0.0f32; NUM_BARS];
            let mut smoothed_bass = 0.0f32;
            let mut prev_kick_energy = 0.0f32;

            loop {
                thread::sleep(Duration::from_millis(8)); // ~120 Hz

                let (is_playing, smoothing, gain) = {
                    if let Ok(state) = shared_state.lock() {
                        (
                            state.is_playing,
                            state.config.viz_smoothing.clamp(0.1, 0.96),
                            state.config.viz_gain.clamp(0.2, 3.0),
                        )
                    } else {
                        (false, 0.82, 1.15)
                    }
                };

                if !is_playing {
                    let mut active = false;
                    for b in 0..NUM_BARS {
                        smoothed_bars[b] *= 0.84;
                        peak_bars[b] *= 0.92;
                        if smoothed_bars[b] < 0.002 {
                            smoothed_bars[b] = 0.0;
                        } else {
                            active = true;
                        }
                    }
                    smoothed_bass *= 0.80;
                    if let Ok(mut state) = shared_state.lock() {
                        state.spectrum = smoothed_bars;
                        state.peaks = peak_bars;
                        state.bass_impulse = smoothed_bass;
                    }
                    if !active {
                        thread::sleep(Duration::from_millis(24));
                    }
                    continue;
                }

                let samples_ready = {
                    if let Ok(ring) = sample_ring.lock() {
                        if ring.len() >= FFT_SIZE {
                            let skip = ring.len() - FFT_SIZE;
                            for (i, &sample) in ring.iter().skip(skip).enumerate() {
                                complex_buf[i] = Complex {
                                    re: sample * hann_window[i],
                                    im: 0.0,
                                };
                            }
                            true
                        } else {
                            false
                        }
                    } else {
                        false
                    }
                };

                if !samples_ready {
                    continue;
                }

                fft.process(&mut complex_buf);

                let sr = sample_rate_atomic.load(Ordering::Relaxed).max(8000) as f32;
                let half_bins = FFT_SIZE / 2;
                let min_freq = 36.0f32;
                let max_freq = 16_500.0f32.min(sr * 0.45);
                let norm_factor = (FFT_SIZE as f32).sqrt();

                let mut raw_bars = [0.0f32; NUM_BARS];

                for b in 0..NUM_BARS {
                    let t0 = b as f32 / NUM_BARS as f32;
                    let t1 = (b + 1) as f32 / NUM_BARS as f32;

                    let f0 = min_freq * (max_freq / min_freq).powf(t0);
                    let f1 = min_freq * (max_freq / min_freq).powf(t1);

                    let mut idx0 = ((f0 * FFT_SIZE as f32) / sr).floor() as usize;
                    let mut idx1 = ((f1 * FFT_SIZE as f32) / sr).ceil() as usize;

                    idx0 = idx0.clamp(1, half_bins.saturating_sub(1));
                    idx1 = idx1.clamp(idx0 + 1, half_bins);

                    let mut peak_mag = 0.0f32;
                    for bin in &complex_buf[idx0..idx1] {
                        let mag = bin.norm() / norm_factor;
                        if mag > peak_mag {
                            peak_mag = mag;
                        }
                    }

                    let tilt = 1.0 + (b as f32 / NUM_BARS as f32) * 2.4;
                    let db = 20.0 * (peak_mag * tilt * 3.8 * gain + 1e-6).log10();
                    let normalized = ((db + 44.0) / 44.0).clamp(0.0, 1.0);

                    raw_bars[b] = normalized.powf(1.35);
                }

                // BFFT BOOM BOOM FUCKAAAAA
                for b in 0..NUM_BARS {
                    if raw_bars[b] > smoothed_bars[b] {
                        smoothed_bars[b] = smoothed_bars[b] * 0.22 + raw_bars[b] * 0.78;
                    } else {
                        smoothed_bars[b] =
                            smoothed_bars[b] * smoothing + raw_bars[b] * (1.0 - smoothing);
                    }

                    if smoothed_bars[b] >= peak_bars[b] {
                        peak_bars[b] = smoothed_bars[b];
                    } else {
                        peak_bars[b] = (peak_bars[b] - 0.008).max(0.0);
                    }
                }

                // Kick detection, cuz normal FFT too boring :p
                let kick_energy =
                    raw_bars[0] * 0.40 + raw_bars[1] * 0.40 + raw_bars[2] * 0.20;
                let flux_onset = (kick_energy - prev_kick_energy).max(0.0);
                prev_kick_energy = kick_energy;

                // boom boom on offset haha
                let target_bounce = (kick_energy * 0.85 + flux_onset * 2.4).clamp(0.0, 1.5);

                if target_bounce > smoothed_bass {
                    // Like 0.2 ms latency attack so that cone gets punched harder
                    smoothed_bass = smoothed_bass * 0.06 + target_bounce * 0.94;
                } else {
                    // Suspension??? (idk my smooth brain was too bored)
                    smoothed_bass = smoothed_bass * 0.72 + target_bounce * 0.28;
                }

                if let Ok(mut state) = shared_state.lock() {
                    state.spectrum = smoothed_bars;
                    state.peaks = peak_bars;
                    state.bass_impulse = smoothed_bass;
                }
            }
        })
        .expect("Failed to spawn FFT worker thread");
}
