//! OASIS-RT — Audio Perception (Hearing)
//!
//! Transforms raw PCM audio into latent space forces.
//! No speech recognition. No ML. Just signal statistics:
//! RMS energy, onset detection, pitch, spectral entropy, voice band.
//!
//! Dimensions 50-55 of the 128-dim latent vector.

use crate::vec::*;
#[cfg(not(feature = "std"))]
use alloc::{vec::Vec, string::String, vec, format};
#[cfg(not(feature = "std"))]
use crate::fmath::F64Ext;

/// Audio analysis result — all values normalized [0, 1]
pub struct AudioPercept {
    /// RMS energy (volume)
    pub energy: f64,
    /// Volume delta since last sample (onset detection)
    pub delta: f64,
    /// Dominant frequency bin (0=low, 1=high)
    pub pitch: f64,
    /// Secondary frequency component
    pub pitch2: f64,
    /// Spectral entropy (1=noise, 0=pure tone)
    pub spectral_entropy: f64,
    /// Voice band ratio (energy 300-3000Hz / total)
    pub voice_ratio: f64,
}

/// Analyze a PCM i16 buffer. Sample rate expected: 16000Hz.
pub fn analyze_pcm(samples: &[i16], prev_energy: f64) -> AudioPercept {
    if samples.is_empty() {
        return AudioPercept { energy: 0.0, delta: 0.0, pitch: 0.0, pitch2: 0.0, spectral_entropy: 0.0, voice_ratio: 0.0 };
    }

    // RMS energy
    let n = samples.len() as f64;
    let sum_sq: f64 = samples.iter().map(|&s| (s as f64).powi(2)).sum();
    let rms = (sum_sq / n).sqrt() / 32768.0; // Normalize to [0, 1]
    let energy = rms.min(1.0);

    // Delta (onset detection)
    let delta = (energy - prev_energy).abs().min(1.0);

    // DFT on first 1024 samples (inline, no crate)
    let fft_size = samples.len().min(1024);
    let mut magnitudes = [0.0_f64; 512]; // Only need first half (Nyquist)
    let bins = fft_size / 2;

    for k in 0..bins.min(512) {
        let mut re = 0.0_f64;
        let mut im = 0.0_f64;
        let freq = 2.0 * core::f64::consts::PI * k as f64 / fft_size as f64;
        for (i, &s) in samples[..fft_size].iter().enumerate() {
            let angle = freq * i as f64;
            re += s as f64 * angle.cos();
            im -= s as f64 * angle.sin();
        }
        magnitudes[k] = (re * re + im * im).sqrt();
    }

    // Pitch: find top 2 frequency bins (skip DC bin 0)
    let mut max1_bin = 1usize;
    let mut max1_mag = 0.0_f64;
    let mut max2_bin = 2usize;
    let mut max2_mag = 0.0_f64;
    for k in 1..bins.min(512) {
        if magnitudes[k] > max1_mag {
            max2_bin = max1_bin;
            max2_mag = max1_mag;
            max1_bin = k;
            max1_mag = magnitudes[k];
        } else if magnitudes[k] > max2_mag {
            max2_bin = k;
            max2_mag = magnitudes[k];
        }
    }
    let pitch = max1_bin as f64 / bins.max(1) as f64; // [0, 1]
    let pitch2 = max2_bin as f64 / bins.max(1) as f64;

    // Spectral entropy (Shannon on magnitude distribution)
    let total_mag: f64 = magnitudes[1..bins.min(512)].iter().sum();
    let spectral_entropy = if total_mag > 1e-10 {
        let mut h = 0.0;
        for k in 1..bins.min(512) {
            let p = magnitudes[k] / total_mag;
            if p > 1e-12 {
                h -= p * p.ln();
            }
        }
        let max_h = (bins.max(2) as f64 - 1.0).ln();
        if max_h > 0.0 {
            (h / max_h).min(1.0)
        } else {
            0.0
        }
    } else {
        0.0
    };

    // Voice band ratio: energy in 300-3000Hz vs total
    // At 16kHz sample rate: bin k = k * 16000 / fft_size Hz
    // 300Hz → bin = 300 * 1024 / 16000 ≈ 19
    // 3000Hz → bin = 3000 * 1024 / 16000 ≈ 192
    let hz_per_bin = 16000.0 / fft_size as f64;
    let low_bin = (300.0 / hz_per_bin) as usize;
    let high_bin = (3000.0 / hz_per_bin).min(bins as f64 - 1.0) as usize;
    let voice_energy: f64 = magnitudes[low_bin..=high_bin.min(511)].iter().sum();
    let voice_ratio = if total_mag > 1e-10 { (voice_energy / total_mag).min(1.0) } else { 0.0 };

    AudioPercept { energy, delta, pitch, pitch2, spectral_entropy, voice_ratio }
}

/// Project audio percept into the latent vector (dimensions 50-55)
pub fn project_audio(percept: &AudioPercept, force: &mut V) {
    force[50] = percept.energy;
    force[51] = percept.delta;
    force[52] = percept.pitch;
    force[53] = percept.pitch2;
    force[54] = percept.spectral_entropy;
    force[55] = percept.voice_ratio;
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sine_wave(freq: f64, sample_rate: f64, n: usize) -> Vec<i16> {
        (0..n).map(|i| (((2.0 * std::f64::consts::PI * freq * i as f64 / sample_rate).sin()) * 16000.0) as i16).collect()
    }

    #[test]
    fn silence_has_zero_energy() {
        let samples = vec![0i16; 1024];
        let p = analyze_pcm(&samples, 0.0);
        assert!(p.energy < 0.01, "silence energy should be ~0, got {}", p.energy);
    }

    #[test]
    fn loud_signal_has_high_energy() {
        let samples: Vec<i16> = (0..1024).map(|_| 20000i16).collect();
        let p = analyze_pcm(&samples, 0.0);
        assert!(p.energy > 0.3, "loud signal should have high energy, got {}", p.energy);
    }

    #[test]
    fn onset_detection_fires_on_change() {
        let p = analyze_pcm(&vec![20000i16; 1024], 0.0);
        assert!(p.delta > 0.3, "onset should fire on 0→loud, got {}", p.delta);
    }

    #[test]
    fn pure_tone_has_lower_entropy_than_noise() {
        let tone = sine_wave(440.0, 16000.0, 1024);
        let noise: Vec<i16> = (0..1024)
            .map(|i| {
                if i % 3 == 0 {
                    10000
                } else if i % 3 == 1 {
                    -8000
                } else {
                    3000
                }
            })
            .collect();
        let pt = analyze_pcm(&tone, 0.0);
        let pn = analyze_pcm(&noise, 0.0);
        assert!(pt.spectral_entropy < pn.spectral_entropy, "tone ({:.4}) should be < noise ({:.4})", pt.spectral_entropy, pn.spectral_entropy);
    }

    #[test]
    fn noise_has_high_spectral_entropy() {
        // Pseudo-noise: alternating values
        let samples: Vec<i16> = (0..1024)
            .map(|i| {
                if i % 3 == 0 {
                    10000
                } else if i % 3 == 1 {
                    -8000
                } else {
                    3000
                }
            })
            .collect();
        let p = analyze_pcm(&samples, 0.0);
        assert!(p.spectral_entropy > 0.3, "noise should have higher spectral entropy, got {}", p.spectral_entropy);
    }

    #[test]
    fn voice_band_detected_for_speech_freq() {
        // 1000Hz tone — right in the voice band (300-3000Hz)
        let samples = sine_wave(1000.0, 16000.0, 1024);
        let p = analyze_pcm(&samples, 0.0);
        assert!(p.voice_ratio > 0.5, "1kHz tone should have high voice ratio, got {}", p.voice_ratio);
    }

    #[test]
    fn low_freq_outside_voice_band() {
        // 100Hz tone — below voice band
        let samples = sine_wave(100.0, 16000.0, 1024);
        let p = analyze_pcm(&samples, 0.0);
        assert!(p.voice_ratio < 0.5, "100Hz should have low voice ratio, got {}", p.voice_ratio);
    }

    #[test]
    fn pitch_finds_dominant_frequency() {
        let samples = sine_wave(2000.0, 16000.0, 1024);
        let p = analyze_pcm(&samples, 0.0);
        // 2000Hz at 16kHz = bin 128 out of 512 = 0.25
        assert!((p.pitch - 0.25).abs() < 0.05, "pitch should be ~0.25 for 2kHz, got {}", p.pitch);
    }

    #[test]
    fn projection_fills_correct_dims() {
        let p = AudioPercept { energy: 0.5, delta: 0.3, pitch: 0.25, pitch2: 0.1, spectral_entropy: 0.7, voice_ratio: 0.6 };
        let mut force = vz();
        project_audio(&p, &mut force);
        assert!((force[50] - 0.5).abs() < 1e-10);
        assert!((force[51] - 0.3).abs() < 1e-10);
        assert!((force[55] - 0.6).abs() < 1e-10);
        // Other dims untouched
        assert!(force[10].abs() < 1e-10);
    }
}
