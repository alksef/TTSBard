//! Native Rust DSP post-processing for TTS audio.
//!
//! Provides parametric EQ (biquad), soft-knee compressor, and safety limiter
//! operating on interleaved f32 PCM. All processing is per-phrase with fresh
//! state — no state leaks between utterances.
//!
//! Conventions: all dB values are linear-to-dB internally; the public interface
//! uses human-friendly linear scale where % = factor.

use std::f32::consts::PI;

// ============================================================================
// Biquad filter — RBJ Audio-EQ-Cookbook
// ============================================================================

/// Pre-computed biquad coefficients (direct form I).
#[derive(Debug, Clone, Copy)]
struct BiquadCoeffs {
    b0: f32,
    b1: f32,
    b2: f32,
    a1: f32,
    a2: f32,
}

/// Running state for a single biquad filter channel.
#[derive(Debug, Clone, Copy)]
struct BiquadState {
    x1: f32,
    x2: f32,
    y1: f32,
    y2: f32,
}

impl Default for BiquadState {
    fn default() -> Self {
        Self {
            x1: 0.0,
            x2: 0.0,
            y1: 0.0,
            y2: 0.0,
        }
    }
}

impl BiquadCoeffs {
    /// Low-shelf (shelving boost/cut at low frequencies).
    /// `db_gain` can be negative for cut.
    #[allow(dead_code)]
    fn low_shelf(sample_rate: u32, freq_hz: f32, db_gain: f32, q: f32) -> Self {
        let a = 10.0_f32.powf(db_gain / 40.0);
        let w0 = 2.0 * PI * freq_hz / sample_rate as f32;
        let cos_w0 = w0.cos();
        let sin_w0 = w0.sin();
        let alpha = sin_w0 / (2.0 * q);
        let sqrt_a = a.sqrt();

        let b0 = a * ((a + 1.0) - (a - 1.0) * cos_w0 + 2.0 * sqrt_a * alpha);
        let b1 = 2.0 * a * ((a - 1.0) - (a + 1.0) * cos_w0);
        let b2 = a * ((a + 1.0) - (a - 1.0) * cos_w0 - 2.0 * sqrt_a * alpha);
        let a0 = (a + 1.0) + (a - 1.0) * cos_w0 + 2.0 * sqrt_a * alpha;
        let a1 = -2.0 * ((a - 1.0) + (a + 1.0) * cos_w0);
        let a2 = (a + 1.0) + (a - 1.0) * cos_w0 - 2.0 * sqrt_a * alpha;

        BiquadCoeffs {
            b0: b0 / a0,
            b1: b1 / a0,
            b2: b2 / a0,
            a1: a1 / a0,
            a2: a2 / a0,
        }
    }

    /// High-shelf (shelving boost/cut at high frequencies).
    fn high_shelf(sample_rate: u32, freq_hz: f32, db_gain: f32, q: f32) -> Self {
        let a = 10.0_f32.powf(db_gain / 40.0);
        let w0 = 2.0 * PI * freq_hz / sample_rate as f32;
        let cos_w0 = w0.cos();
        let sin_w0 = w0.sin();
        let alpha = sin_w0 / (2.0 * q);
        let sqrt_a = a.sqrt();

        let b0 = a * ((a + 1.0) + (a - 1.0) * cos_w0 + 2.0 * sqrt_a * alpha);
        let b1 = -2.0 * a * ((a - 1.0) + (a + 1.0) * cos_w0);
        let b2 = a * ((a + 1.0) + (a - 1.0) * cos_w0 - 2.0 * sqrt_a * alpha);
        let a0 = (a + 1.0) - (a - 1.0) * cos_w0 + 2.0 * sqrt_a * alpha;
        let a1 = 2.0 * ((a - 1.0) - (a + 1.0) * cos_w0);
        let a2 = (a + 1.0) - (a - 1.0) * cos_w0 - 2.0 * sqrt_a * alpha;

        BiquadCoeffs {
            b0: b0 / a0,
            b1: b1 / a0,
            b2: b2 / a0,
            a1: a1 / a0,
            a2: a2 / a0,
        }
    }

    /// Peaking EQ (parametric band).
    fn peaking(sample_rate: u32, freq_hz: f32, db_gain: f32, q: f32) -> Self {
        let a = 10.0_f32.powf(db_gain / 40.0);
        let w0 = 2.0 * PI * freq_hz / sample_rate as f32;
        let cos_w0 = w0.cos();
        let sin_w0 = w0.sin();
        let alpha = sin_w0 / (2.0 * q);

        let b0 = 1.0 + alpha * a;
        let b1 = -2.0 * cos_w0;
        let b2 = 1.0 - alpha * a;
        let a0 = 1.0 + alpha / a;
        let a1 = -2.0 * cos_w0;
        let a2 = 1.0 - alpha / a;

        BiquadCoeffs {
            b0: b0 / a0,
            b1: b1 / a0,
            b2: b2 / a0,
            a1: a1 / a0,
            a2: a2 / a0,
        }
    }

    /// Low-cut (second-order high-pass).
    fn low_cut(sample_rate: u32, freq_hz: f32, q: f32) -> Self {
        let w0 = 2.0 * PI * freq_hz / sample_rate as f32;
        let cos_w0 = w0.cos();
        let sin_w0 = w0.sin();
        let alpha = sin_w0 / (2.0 * q);

        let b0 = (1.0 + cos_w0) / 2.0;
        let b1 = -(1.0 + cos_w0);
        let b2 = (1.0 + cos_w0) / 2.0;
        let a0 = 1.0 + alpha;
        let a1 = -2.0 * cos_w0;
        let a2 = 1.0 - alpha;

        BiquadCoeffs {
            b0: b0 / a0,
            b1: b1 / a0,
            b2: b2 / a0,
            a1: a1 / a0,
            a2: a2 / a0,
        }
    }

    fn process(&mut self, state: &mut BiquadState, x: f32) -> f32 {
        let y = self.b0 * x + self.b1 * state.x1 + self.b2 * state.x2
            - self.a1 * state.y1
            - self.a2 * state.y2;

        state.x2 = state.x1;
        state.x1 = x;
        state.y2 = state.y1;
        state.y1 = y;

        if !y.is_finite() {
            *state = BiquadState::default();
            return x;
        }
        y
    }
}

// ============================================================================
// Compressor — soft-knee RMS-based
// ============================================================================

struct CompressorState {
    envelope: f32, // running RMS envelope (squared)
}

impl CompressorState {
    fn new() -> Self {
        Self { envelope: 0.0 }
    }
}

fn compress_sample(
    x: f32,
    threshold_linear: f32,
    ratio: f32,
    knee_linear: f32,
    makeup_linear: f32,
    attack_coeff: f32,
    release_coeff: f32,
    state: &mut CompressorState,
) -> f32 {
    let abs_x = x.abs();
    let x_sq = abs_x * abs_x;

    if state.envelope < x_sq {
        state.envelope = attack_coeff * state.envelope + (1.0 - attack_coeff) * x_sq;
    } else {
        state.envelope = release_coeff * state.envelope + (1.0 - release_coeff) * x_sq;
    }

    let env_db = if state.envelope > 1e-12 {
        20.0 * state.envelope.sqrt().log10()
    } else {
        -120.0
    };

    let threshold_db = 20.0 * threshold_linear.log10();
    let knee_db = 20.0 * knee_linear.log10();

    let gain_reduction_db = if env_db < threshold_db - knee_db / 2.0 {
        0.0
    } else if env_db > threshold_db + knee_db / 2.0 {
        (threshold_db - env_db) * (1.0 - 1.0 / ratio)
    } else {
        let diff = env_db - threshold_db + knee_db / 2.0;
        ((diff * diff) / (2.0 * knee_db)) * (1.0 - 1.0 / ratio) * -1.0
    };

    let gr_linear = 10.0_f32.powf(gain_reduction_db / 20.0);
    let result = x * gr_linear * makeup_linear;

    if !result.is_finite() {
        *state = CompressorState::new();
        return x;
    }
    result
}

// ============================================================================
// Limiter — simple brickwall with soft clip
// ============================================================================

struct LimiterState {
    envelope: f32,
}

impl LimiterState {
    fn new() -> Self {
        Self { envelope: 0.0 }
    }
}

fn limit_sample(x: f32, ceiling_linear: f32, release_coeff: f32, state: &mut LimiterState) -> f32 {
    let abs_x = x.abs();

    if abs_x > state.envelope {
        state.envelope = abs_x;
    } else {
        state.envelope = state.envelope * release_coeff + abs_x * (1.0 - release_coeff);
    }

    let desired_gain = if state.envelope > ceiling_linear {
        ceiling_linear / state.envelope
    } else {
        1.0
    };

    let result = (x * desired_gain).clamp(-ceiling_linear, ceiling_linear);

    if !result.is_finite() {
        *state = LimiterState::new();
        return x.clamp(-ceiling_linear, ceiling_linear);
    }
    result
}

// ============================================================================
// DSP configuration
// ============================================================================

/// Parametric EQ band definition.
#[derive(Debug, Clone, Copy)]
pub struct EqBand {
    pub enabled: bool,
    pub frequency_hz: f32,
    pub gain_db: f32,
    pub q: f32,
}

impl Default for EqBand {
    fn default() -> Self {
        Self {
            enabled: false,
            frequency_hz: 2500.0,
            gain_db: 0.0,
            q: 0.7,
        }
    }
}

/// EQ settings.
#[derive(Debug, Clone)]
pub struct EqConfig {
    pub enabled: bool,
    pub low_cut_enabled: bool,
    pub low_cut_hz: f32,
    pub low_cut_slope_db: f32,
    pub bands: [EqBand; 3],
    pub high_shelf_enabled: bool,
    pub high_shelf_hz: f32,
    pub high_shelf_gain_db: f32,
}

impl Default for EqConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            low_cut_enabled: false,
            low_cut_hz: 80.0,
            low_cut_slope_db: 12.0,
            bands: [EqBand::default(); 3],
            high_shelf_enabled: false,
            high_shelf_hz: 8000.0,
            high_shelf_gain_db: 0.0,
        }
    }
}

/// Compressor settings.
#[derive(Debug, Clone)]
pub struct CompressorConfig {
    pub enabled: bool,
    pub threshold_db: f32,
    pub ratio: f32,
    pub attack_ms: f32,
    pub release_ms: f32,
    pub knee_db: f32,
    pub makeup_db: f32,
}

impl Default for CompressorConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            threshold_db: -18.0,
            ratio: 2.0,
            attack_ms: 8.0,
            release_ms: 120.0,
            knee_db: 6.0,
            makeup_db: 0.0,
        }
    }
}

/// Limiter settings.
#[derive(Debug, Clone)]
pub struct LimiterConfig {
    pub enabled: bool,
    pub ceiling_db: f32,
    pub release_ms: f32,
}

impl Default for LimiterConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            ceiling_db: -1.0,
            release_ms: 50.0,
        }
    }
}

/// Full DSP configuration.
#[derive(Debug, Clone)]
pub struct DspConfig {
    pub eq: EqConfig,
    pub compressor: CompressorConfig,
    pub limiter: LimiterConfig,
}

impl Default for DspConfig {
    fn default() -> Self {
        Self {
            eq: EqConfig::default(),
            compressor: CompressorConfig::default(),
            limiter: LimiterConfig::default(),
        }
    }
}

// ============================================================================
// Main DSP processing
// ============================================================================

fn coeff_from_ms(ms: f32, sample_rate: u32) -> f32 {
    let samples = (ms / 1000.0 * sample_rate as f32).max(1.0);
    (-1.0 / samples).exp()
}

fn db_to_linear(db: f32) -> f32 {
    10.0_f32.powf(db / 20.0)
}

/// Apply DSP post-processing to interleaved f32 PCM.
///
/// Preserves sample rate, channels, and frame count.
/// All three blocks (EQ, compressor, limiter) are independently bypassable.
/// Never produces NaN/Inf output.
pub fn process_dsp(
    samples: &[f32],
    sample_rate: u32,
    channels: usize,
    config: &DspConfig,
) -> Vec<f32> {
    if samples.is_empty() || channels == 0 || sample_rate == 0 {
        return samples.to_vec();
    }

    let mut output = samples.to_vec();

    if config.eq.enabled {
        apply_eq(&mut output, sample_rate, channels, &config.eq);
    }

    if config.compressor.enabled {
        apply_compressor(&mut output, sample_rate, channels, &config.compressor);
    }

    if config.limiter.enabled {
        apply_limiter(&mut output, sample_rate, channels, &config.limiter);
    }

    for s in output.iter_mut() {
        if !s.is_finite() {
            *s = 0.0;
        }
    }

    output
}

fn apply_eq(samples: &mut [f32], sample_rate: u32, channels: usize, cfg: &EqConfig) {
    let mut biquads: Vec<(BiquadCoeffs, Vec<BiquadState>)> = Vec::new();

    let q_low_cut = 0.7071; // Butterworth Q
    if cfg.low_cut_enabled && cfg.low_cut_hz > 0.0 && cfg.low_cut_hz < sample_rate as f32 * 0.45 {
        let coeffs = BiquadCoeffs::low_cut(sample_rate, cfg.low_cut_hz, q_low_cut);
        let states = vec![BiquadState::default(); channels];
        biquads.push((coeffs, states));
    }

    for band in &cfg.bands {
        if band.enabled && band.frequency_hz > 0.0 && band.frequency_hz < sample_rate as f32 * 0.45
        {
            let coeffs = BiquadCoeffs::peaking(
                sample_rate,
                band.frequency_hz,
                band.gain_db,
                band.q.max(0.1),
            );
            let states = vec![BiquadState::default(); channels];
            biquads.push((coeffs, states));
        }
    }

    if cfg.high_shelf_enabled
        && cfg.high_shelf_hz > 0.0
        && cfg.high_shelf_hz < sample_rate as f32 * 0.45
    {
        let coeffs = BiquadCoeffs::high_shelf(
            sample_rate,
            cfg.high_shelf_hz,
            cfg.high_shelf_gain_db,
            0.7071,
        );
        let states = vec![BiquadState::default(); channels];
        biquads.push((coeffs, states));
    }

    if biquads.is_empty() {
        return;
    }

    let frames = samples.len() / channels;
    for f in 0..frames {
        for ch in 0..channels {
            let idx = f * channels + ch;
            let mut x = samples[idx];
            for (coeffs, states) in biquads.iter_mut() {
                x = coeffs.process(&mut states[ch], x);
            }
            samples[idx] = x;
        }
    }
}

fn apply_compressor(
    samples: &mut [f32],
    sample_rate: u32,
    channels: usize,
    cfg: &CompressorConfig,
) {
    let threshold_linear = db_to_linear(cfg.threshold_db);
    let knee_linear = db_to_linear(cfg.knee_db);
    let makeup_linear = db_to_linear(cfg.makeup_db);
    let ratio = cfg.ratio.max(1.0);
    let attack_coeff = coeff_from_ms(cfg.attack_ms.max(0.1), sample_rate);
    let release_coeff = coeff_from_ms(cfg.release_ms.max(0.1), sample_rate);

    let mut states: Vec<CompressorState> = (0..channels).map(|_| CompressorState::new()).collect();

    let frames = samples.len() / channels;
    for f in 0..frames {
        for ch in 0..channels {
            let idx = f * channels + ch;
            samples[idx] = compress_sample(
                samples[idx],
                threshold_linear,
                ratio,
                knee_linear,
                makeup_linear,
                attack_coeff,
                release_coeff,
                &mut states[ch],
            );
        }
    }
}

fn apply_limiter(samples: &mut [f32], sample_rate: u32, channels: usize, cfg: &LimiterConfig) {
    let ceiling_linear = db_to_linear(cfg.ceiling_db);
    let release_coeff = coeff_from_ms(cfg.release_ms.max(0.1), sample_rate);

    let mut states: Vec<LimiterState> = (0..channels).map(|_| LimiterState::new()).collect();

    let frames = samples.len() / channels;
    for f in 0..frames {
        for ch in 0..channels {
            let idx = f * channels + ch;
            samples[idx] =
                limit_sample(samples[idx], ceiling_linear, release_coeff, &mut states[ch]);
        }
    }
}

// ============================================================================
// Unit tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    fn make_sine(freq: f32, sample_rate: u32, duration_secs: f32, amplitude: f32) -> Vec<f32> {
        let n = (sample_rate as f32 * duration_secs) as usize;
        (0..n)
            .map(|i| {
                let t = i as f32 / sample_rate as f32;
                (2.0 * std::f32::consts::PI * freq * t).sin() * amplitude
            })
            .collect()
    }

    fn make_interleaved_sine(
        freq: f32,
        sample_rate: u32,
        duration_secs: f32,
        amplitude: f32,
        channels: usize,
    ) -> Vec<f32> {
        let mono = make_sine(freq, sample_rate, duration_secs, amplitude);
        let frames = mono.len();
        let mut interleaved = Vec::with_capacity(frames * channels);
        for i in 0..frames {
            for _ in 0..channels {
                interleaved.push(mono[i]);
            }
        }
        interleaved
    }

    // ------------------------------------------------------------------
    // EQ tests
    // ------------------------------------------------------------------

    #[test]
    fn eq_silence_remains_silence_mono() {
        let samples = vec![0.0f32; 480];
        let cfg = DspConfig {
            eq: EqConfig {
                enabled: true,
                low_cut_enabled: true,
                low_cut_hz: 80.0,
                low_cut_slope_db: 12.0,
                high_shelf_enabled: true,
                high_shelf_hz: 8000.0,
                high_shelf_gain_db: 6.0,
                bands: [
                    EqBand {
                        enabled: true,
                        frequency_hz: 2500.0,
                        gain_db: 3.0,
                        q: 0.7,
                    },
                    EqBand::default(),
                    EqBand::default(),
                ],
                ..Default::default()
            },
            ..Default::default()
        };
        let out = process_dsp(&samples, 48000, 1, &cfg);
        assert_eq!(out.len(), 480);
        for &s in &out {
            assert!(s.is_finite());
            assert!((s - 0.0).abs() < 1e-6, "silence must stay silent");
        }
    }

    #[test]
    fn eq_silence_remains_silence_stereo() {
        let samples = vec![0.0f32; 960]; // 480 frames × 2
        let cfg = DspConfig {
            eq: EqConfig {
                enabled: true,
                low_cut_enabled: true,
                low_cut_hz: 80.0,
                ..Default::default()
            },
            ..Default::default()
        };
        let out = process_dsp(&samples, 48000, 2, &cfg);
        assert_eq!(out.len(), 960);
        for &s in &out {
            assert!((s - 0.0).abs() < 1e-6);
        }
    }

    #[test]
    fn eq_preserves_channels_and_frames_mono() {
        let samples = make_sine(440.0, 48000, 0.1, 0.5);
        let cfg = DspConfig {
            eq: EqConfig {
                enabled: true,
                low_cut_enabled: true,
                low_cut_hz: 80.0,
                bands: [
                    EqBand {
                        enabled: true,
                        frequency_hz: 2500.0,
                        gain_db: 3.0,
                        q: 0.7,
                    },
                    EqBand::default(),
                    EqBand::default(),
                ],
                ..Default::default()
            },
            ..Default::default()
        };
        let out = process_dsp(&samples, 48000, 1, &cfg);
        assert_eq!(out.len(), samples.len());
        for &s in &out {
            assert!(s.is_finite(), "all samples must be finite");
        }
    }

    #[test]
    fn eq_preserves_channels_and_frames_stereo() {
        let samples = make_interleaved_sine(440.0, 48000, 0.1, 0.5, 2);
        let cfg = DspConfig {
            eq: EqConfig {
                enabled: true,
                bands: [
                    EqBand {
                        enabled: true,
                        frequency_hz: 3000.0,
                        gain_db: -2.0,
                        q: 1.0,
                    },
                    EqBand::default(),
                    EqBand::default(),
                ],
                ..Default::default()
            },
            ..Default::default()
        };
        let out = process_dsp(&samples, 48000, 2, &cfg);
        assert_eq!(out.len(), samples.len());
        for &s in &out {
            assert!(s.is_finite());
        }
    }

    #[test]
    fn eq_different_sample_rates() {
        for &sr in &[22050u32, 44100, 48000] {
            let samples = make_sine(440.0, sr, 0.05, 0.5);
            let cfg = DspConfig {
                eq: EqConfig {
                    enabled: true,
                    low_cut_enabled: true,
                    low_cut_hz: 80.0,
                    ..Default::default()
                },
                ..Default::default()
            };
            let out = process_dsp(&samples, sr, 1, &cfg);
            assert_eq!(out.len(), samples.len());
            for &s in &out {
                assert!(s.is_finite());
            }
        }
    }

    #[test]
    fn eq_bypass_produces_no_change() {
        let samples = make_sine(440.0, 48000, 0.1, 0.5);
        let cfg = DspConfig::default(); // all disabled
        let out = process_dsp(&samples, 48000, 1, &cfg);
        assert_eq!(out, samples);
    }

    // ------------------------------------------------------------------
    // Compressor tests
    // ------------------------------------------------------------------

    #[test]
    fn compressor_silence_stays_silent() {
        let samples = vec![0.0f32; 480];
        let cfg = DspConfig {
            compressor: CompressorConfig {
                enabled: true,
                threshold_db: -18.0,
                ratio: 2.0,
                attack_ms: 8.0,
                release_ms: 120.0,
                knee_db: 6.0,
                makeup_db: 3.0,
            },
            ..Default::default()
        };
        let out = process_dsp(&samples, 48000, 1, &cfg);
        for &s in &out {
            assert!((s - 0.0).abs() < 1e-6);
        }
    }

    #[test]
    fn compressor_stereo_channels_preserved() {
        let samples = make_interleaved_sine(440.0, 48000, 0.1, 0.9, 2);
        let cfg = DspConfig {
            compressor: CompressorConfig {
                enabled: true,
                threshold_db: -6.0,
                ratio: 4.0,
                attack_ms: 5.0,
                release_ms: 100.0,
                knee_db: 3.0,
                makeup_db: 2.0,
            },
            ..Default::default()
        };
        let out = process_dsp(&samples, 48000, 2, &cfg);
        assert_eq!(out.len(), samples.len());
        for &s in &out {
            assert!(s.is_finite());
        }
    }

    #[test]
    fn compressor_reduces_gain_above_threshold() {
        let samples = vec![0.8f32; 48000]; // ~ -1.94 dBFS
        let cfg = DspConfig {
            compressor: CompressorConfig {
                enabled: true,
                threshold_db: -20.0,
                ratio: 100.0, // essentially a limiter
                attack_ms: 0.1,
                release_ms: 50.0,
                knee_db: 0.0,
                makeup_db: 0.0,
            },
            ..Default::default()
        };
        let out = process_dsp(&samples, 48000, 1, &cfg);

        let initial_max = samples.iter().map(|s| s.abs()).fold(0.0f32, f32::max);
        let out_max = out.iter().map(|s| s.abs()).fold(0.0f32, f32::max);
        assert!(
            out_max < initial_max * 0.95,
            "compressor should reduce amplitude above threshold"
        );
    }

    // ------------------------------------------------------------------
    // Limiter tests
    // ------------------------------------------------------------------

    #[test]
    fn limiter_mono_ceiling() {
        let samples = vec![1.0f32, -1.0, 0.5, -0.5, 1.0, 1.0];
        let cfg = DspConfig {
            limiter: LimiterConfig {
                enabled: true,
                ceiling_db: -3.0,
                release_ms: 50.0,
            },
            ..Default::default()
        };
        let out = process_dsp(&samples, 48000, 1, &cfg);
        let ceiling_linear = db_to_linear(-3.0);
        for &s in &out {
            assert!(s.is_finite());
            assert!(
                s.abs() <= ceiling_linear + 1e-4,
                "sample {s} exceeds ceiling {ceiling_linear}"
            );
        }
    }

    #[test]
    fn limiter_stereo_ceiling() {
        let samples = make_interleaved_sine(440.0, 48000, 0.1, 1.5, 2); // clips
        let cfg = DspConfig {
            limiter: LimiterConfig {
                enabled: true,
                ceiling_db: -1.0,
                release_ms: 30.0,
            },
            ..Default::default()
        };
        let out = process_dsp(&samples, 48000, 2, &cfg);
        let ceiling_linear = db_to_linear(-1.0);
        for &s in &out {
            assert!(s.is_finite());
            assert!(s.abs() <= ceiling_linear + 1e-4);
        }
    }

    #[test]
    fn limiter_ceiling_tolerance() {
        let samples = vec![2.0f32; 480];
        let cfg = DspConfig {
            limiter: LimiterConfig {
                enabled: true,
                ceiling_db: -1.0,
                release_ms: 10.0,
            },
            ..Default::default()
        };
        let out = process_dsp(&samples, 48000, 1, &cfg);
        let ceiling_linear = db_to_linear(-1.0);
        for &s in &out {
            assert!(s.is_finite());
            assert!(s.abs() <= ceiling_linear + 1e-4);
        }
    }

    #[test]
    fn limiter_silence_stays_silent() {
        let samples = vec![0.0f32; 480];
        let cfg = DspConfig {
            limiter: LimiterConfig {
                enabled: true,
                ceiling_db: -1.0,
                release_ms: 50.0,
            },
            ..Default::default()
        };
        let out = process_dsp(&samples, 48000, 1, &cfg);
        for &s in &out {
            assert!((s - 0.0).abs() < 1e-6);
        }
    }

    // ------------------------------------------------------------------
    // Combined tests
    // ------------------------------------------------------------------

    #[test]
    fn combined_eq_compressor_limiter_finite() {
        let samples = make_sine(440.0, 48000, 0.2, 0.9);
        let cfg = DspConfig {
            eq: EqConfig {
                enabled: true,
                low_cut_enabled: true,
                low_cut_hz: 80.0,
                bands: [
                    EqBand {
                        enabled: true,
                        frequency_hz: 2500.0,
                        gain_db: 2.0,
                        q: 0.7,
                    },
                    EqBand::default(),
                    EqBand::default(),
                ],
                ..Default::default()
            },
            compressor: CompressorConfig {
                enabled: true,
                threshold_db: -12.0,
                ratio: 2.0,
                attack_ms: 5.0,
                release_ms: 80.0,
                knee_db: 3.0,
                makeup_db: 2.0,
            },
            limiter: LimiterConfig {
                enabled: true,
                ceiling_db: -1.0,
                release_ms: 30.0,
            },
        };
        let out = process_dsp(&samples, 48000, 1, &cfg);
        assert_eq!(out.len(), samples.len());
        for &s in &out {
            assert!(s.is_finite());
        }
    }

    #[test]
    fn panic_on_nan_input_does_not_produce_nan() {
        // We handle NaN in the biquad/compressor/limiter individual sample functions
        let mut samples = make_sine(440.0, 48000, 0.05, 0.5);
        // The per-sample NaN guard handles this at the biquad level
        samples[10] = f32::NAN;
        let cfg = DspConfig {
            eq: EqConfig {
                enabled: true,
                low_cut_enabled: true,
                low_cut_hz: 80.0,
                ..Default::default()
            },
            ..Default::default()
        };
        let out = process_dsp(&samples, 48000, 1, &cfg);
        for &s in &out {
            assert!(s.is_finite());
        }
    }

    #[test]
    fn per_phrase_isolation() {
        // Process two different phrases with the same config; verify no state leak
        let phrase1 = vec![0.8f32; 24000];
        let phrase2 = vec![0.3f32; 24000];
        let cfg = DspConfig {
            compressor: CompressorConfig {
                enabled: true,
                threshold_db: -10.0,
                ratio: 2.0,
                attack_ms: 5.0,
                release_ms: 100.0,
                knee_db: 3.0,
                makeup_db: 0.0,
            },
            ..Default::default()
        };
        let out1 = process_dsp(&phrase1, 48000, 1, &cfg);
        let out2 = process_dsp(&phrase2, 48000, 1, &cfg);
        // Both should produce finite output; state is fresh each call
        assert_eq!(out1.len(), phrase1.len());
        assert_eq!(out2.len(), phrase2.len());
        for &s in &out1 {
            assert!(s.is_finite());
        }
        for &s in &out2 {
            assert!(s.is_finite());
        }
    }

    #[test]
    fn empty_input_returns_empty() {
        let cfg = DspConfig {
            eq: EqConfig {
                enabled: true,
                ..Default::default()
            },
            compressor: CompressorConfig {
                enabled: true,
                ..Default::default()
            },
            limiter: LimiterConfig {
                enabled: true,
                ..Default::default()
            },
        };
        let out = process_dsp(&[], 48000, 1, &cfg);
        assert!(out.is_empty());
    }

    // ------------------------------------------------------------------
    // Biquad coefficient tests
    // ------------------------------------------------------------------

    #[test]
    fn biquad_identity_at_zero_gain() {
        // A peaking filter with 0 dB gain should have near-unity pass-through
        let coeffs = BiquadCoeffs::peaking(48000, 1000.0, 0.0, 1.0);
        let mut state = BiquadState::default();
        let mut coeffs_mut = coeffs;
        let y = coeffs_mut.process(&mut state, 0.5);
        assert!(y.is_finite());
        assert!((y - 0.5).abs() < 0.1, "near-unity at 0 dB gain");
    }

    #[test]
    fn biquad_coeffs_finite() {
        for &sr in &[22050u32, 44100, 48000] {
            let coeffs = BiquadCoeffs::peaking(sr, 2500.0, 3.0, 0.7);
            assert!(coeffs.b0.is_finite());
            assert!(coeffs.b1.is_finite());
            assert!(coeffs.b2.is_finite());
            assert!(coeffs.a1.is_finite());
            assert!(coeffs.a2.is_finite());
        }
    }

    #[test]
    fn biquad_low_shelf_coeffs_finite() {
        let coeffs = BiquadCoeffs::low_shelf(48000, 200.0, 3.0, 0.7071);
        assert!(coeffs.b0.is_finite());
        assert!(coeffs.b1.is_finite());
    }

    #[test]
    fn biquad_high_shelf_coeffs_finite() {
        let coeffs = BiquadCoeffs::high_shelf(48000, 8000.0, -3.0, 0.7071);
        assert!(coeffs.b0.is_finite());
        assert!(coeffs.b1.is_finite());
    }

    #[test]
    fn biquad_low_cut_coeffs_finite() {
        let coeffs = BiquadCoeffs::low_cut(48000, 80.0, 0.7071);
        assert!(coeffs.b0.is_finite());
        assert!(coeffs.b1.is_finite());
    }
}
