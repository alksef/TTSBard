//! Boundary processing for PCM audio phrases.
//!
//! Provides pure functions to:
//! - Clean up DC offset and start/end discontinuities in a single `AudioPcm` phrase.
//! - Perform an equal-power crossfade between two compatible PCM buffers.

use super::effects::AudioPcm;

// ---------------------------------------------------------------------------
// Configuration
// ---------------------------------------------------------------------------

#[derive(Debug, Clone)]
pub struct BoundaryConfig {
    pub dc_offset_threshold: f32,
    pub fade_length_ms: f32,
    pub jump_threshold: f32,
}

impl Default for BoundaryConfig {
    fn default() -> Self {
        Self {
            dc_offset_threshold: 0.005,
            fade_length_ms: 5.0,
            jump_threshold: 2.0,
        }
    }
}

// ---------------------------------------------------------------------------
// Public API
// ---------------------------------------------------------------------------

/// Process a single PCM phrase to clean up boundary artifacts.
///
/// Uses a sensible default configuration. For tuned parameters, use
/// [`process_boundaries_with_config`].
pub fn process_boundaries(pcm: &AudioPcm) -> AudioPcm {
    process_boundaries_with_config(pcm, &BoundaryConfig::default())
}

/// Process a single PCM phrase to clean up boundary artifacts.
///
/// What it does (conditionally):
/// 1. Subtracts DC offset only if the mean exceeds `dc_offset_threshold`.
/// 2. Applies a short linear fade-in only if the phrase starts with a
///    noticeable jump.
/// 3. Applies a short linear fade-out only if the phrase ends with a
///    noticeable jump.
///
/// Never changes sample rate, channels, or frame count. Empty, single-frame,
/// and very-short phrases are returned unchanged.
pub fn process_boundaries_with_config(pcm: &AudioPcm, config: &BoundaryConfig) -> AudioPcm {
    let channels = pcm.channels;
    let sample_rate = pcm.sample_rate;
    let frame_count = pcm.frame_count();
    let mut samples = pcm.samples.clone();

    if frame_count < 2 {
        return pcm.clone();
    }

    let fade_frames = ((config.fade_length_ms / 1000.0) * sample_rate as f32)
        .round()
        .max(1.0) as usize;

    if frame_count <= fade_frames * 2 {
        return AudioPcm::new(samples, sample_rate, channels).unwrap_or_else(|_| pcm.clone());
    }

    let mean = samples.iter().sum::<f32>() / samples.len() as f32;
    let max_abs = samples.iter().map(|s| s.abs()).fold(0.0f32, f32::max);
    if mean.abs() > config.dc_offset_threshold && mean.abs() > max_abs * 0.02 {
        for s in &mut samples {
            *s -= mean;
        }
    }

    if has_start_jump(&samples, channels, sample_rate, config.jump_threshold) {
        apply_fade_in(&mut samples, channels, fade_frames);
    }

    if has_end_jump(&samples, channels, sample_rate, config.jump_threshold) {
        apply_fade_out(&mut samples, channels, fade_frames);
    }

    AudioPcm::new(samples, sample_rate, channels).unwrap_or_else(|_| pcm.clone())
}

/// Equal-power crossfade between two compatible PCM buffers.
///
/// The end of `a` and the beginning of `b` are mixed over `fade_secs`
/// seconds using a sin/cos ramp (equal-power law).
///
/// `fade_secs` is clamped to `min(a.duration, b.duration)` so the call
/// never panics on short inputs.
///
/// Returns an error if `a` and `b` differ in sample rate or channel count.
pub fn crossfade(a: &AudioPcm, b: &AudioPcm, fade_secs: f32) -> Result<AudioPcm, String> {
    if a.sample_rate != b.sample_rate {
        return Err(format!(
            "crossfade sample-rate mismatch: {} vs {}",
            a.sample_rate, b.sample_rate
        ));
    }
    if a.channels != b.channels {
        return Err(format!(
            "crossfade channel mismatch: {} vs {}",
            a.channels, b.channels
        ));
    }

    let sample_rate = a.sample_rate;
    let channels = a.channels;
    let a_frames = a.frame_count();
    let b_frames = b.frame_count();

    let fade_samples_per_ch = ((fade_secs * sample_rate as f32) as usize)
        .min(a_frames)
        .min(b_frames);

    if fade_samples_per_ch == 0 {
        let mut combined = a.samples.clone();
        combined.extend_from_slice(&b.samples);
        return AudioPcm::new(combined, sample_rate, channels);
    }

    let fade_total = fade_samples_per_ch * channels;
    let a_keep_samples = a.samples.len().saturating_sub(fade_total);
    let b_skip_samples = fade_total;

    let out_len = a_keep_samples + fade_total + b.samples.len().saturating_sub(b_skip_samples);
    let mut out = Vec::with_capacity(out_len);

    out.extend_from_slice(&a.samples[..a_keep_samples]);

    for i in 0..fade_samples_per_ch {
        let t = i as f32 / fade_samples_per_ch as f32;
        let fade_out_gain = (std::f32::consts::FRAC_PI_2 * t).cos();
        let fade_in_gain = (std::f32::consts::FRAC_PI_2 * t).sin();

        for ch in 0..channels {
            let a_val = a.samples[a_keep_samples + i * channels + ch];
            let b_val = b.samples[i * channels + ch];
            out.push(a_val * fade_out_gain + b_val * fade_in_gain);
        }
    }

    out.extend_from_slice(&b.samples[b_skip_samples..]);

    AudioPcm::new(out, sample_rate, channels)
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

fn has_start_jump(samples: &[f32], channels: usize, sample_rate: u32, threshold: f32) -> bool {
    let check_frames = ((sample_rate as f32 * 0.005) as usize).max(1); // 5 ms
    let check_samples = check_frames * channels;
    if samples.len() < check_samples * 2 {
        return false;
    }

    let first_peak = samples[..check_samples]
        .iter()
        .map(|s| s.abs())
        .fold(0.0f32, f32::max);

    let rest_sum: f32 = samples[check_samples..check_samples * 2]
        .iter()
        .map(|s| s.abs())
        .sum();
    let rest_avg = rest_sum / (check_samples as f32);

    if rest_avg < 1e-10 {
        return first_peak > 0.05;
    }
    first_peak / rest_avg > threshold
}

fn has_end_jump(samples: &[f32], channels: usize, sample_rate: u32, threshold: f32) -> bool {
    let check_frames = ((sample_rate as f32 * 0.005) as usize).max(1); // 5 ms
    let check_samples = check_frames * channels;
    let total = samples.len();
    if total < check_samples * 2 {
        return false;
    }

    let last_peak = samples[total - check_samples..]
        .iter()
        .map(|s| s.abs())
        .fold(0.0f32, f32::max);

    let rest_sum: f32 = samples[total - check_samples * 2..total - check_samples]
        .iter()
        .map(|s| s.abs())
        .sum();
    let rest_avg = rest_sum / (check_samples as f32);

    if rest_avg < 1e-10 {
        return last_peak > 0.05;
    }
    last_peak / rest_avg > threshold
}

fn apply_fade_in(samples: &mut [f32], channels: usize, fade_frames: usize) {
    let fade_samples = fade_frames * channels;
    let fade_count = fade_samples.min(samples.len());
    if fade_count < 2 {
        return;
    }
    for (i, sample) in samples.iter_mut().enumerate().take(fade_count) {
        let gain = i as f32 / (fade_count - 1) as f32;
        *sample *= gain;
    }
}

fn apply_fade_out(samples: &mut [f32], channels: usize, fade_frames: usize) {
    let fade_samples = fade_frames * channels;
    let fade_count = fade_samples.min(samples.len());
    if fade_count < 2 {
        return;
    }
    let total = samples.len();
    for i in 0..fade_count {
        let gain = 1.0 - (i as f32 / (fade_count - 1) as f32);
        samples[total - fade_count + i] *= gain;
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    fn make_pcm(samples: Vec<f32>, sample_rate: u32, channels: usize) -> AudioPcm {
        AudioPcm::new(samples, sample_rate, channels).expect("test fixture valid")
    }

    // -- process_boundaries --------------------------------------------------

    #[test]
    fn normal_phrase_unchanged() {
        // A sine wave with natural zero-crossing: no DC, no jumps.
        let sr = 48000;
        let channels = 1;
        let frames = 2000;
        let samples: Vec<f32> = (0..frames)
            .map(|i| (2.0 * std::f32::consts::PI * 440.0 * i as f32 / sr as f32).sin() * 0.5)
            .collect();
        let pcm = make_pcm(samples.clone(), sr, channels);
        let result = process_boundaries(&pcm);
        assert_eq!(result.sample_rate, sr);
        assert_eq!(result.channels, channels);
        assert_eq!(result.frame_count(), frames);
        // Normal speech-like audio should not be distorted.
        for (a, b) in samples.iter().zip(result.samples.iter()) {
            assert!((a - b).abs() < 0.001, "normal phrase changed at offset");
        }
    }

    #[test]
    fn nonzero_first_sample_gets_soft_start() {
        // Phrase starts at full amplitude (discontinuity).
        let sr = 48000;
        let channels = 1;
        let frames = 1000;
        let mut samples: Vec<f32> = vec![0.0; frames];
        samples[0] = 0.95; // sharp jump
                           // Fill the rest with reasonable audio.
        for (i, sample) in samples.iter_mut().enumerate().skip(1) {
            *sample = (2.0 * std::f32::consts::PI * 440.0 * i as f32 / sr as f32).sin() * 0.5;
        }
        let pcm = make_pcm(samples.clone(), sr, channels);
        let result = process_boundaries(&pcm);

        assert_eq!(result.frame_count(), frames);
        assert!(
            result.samples[0].abs() < 0.95,
            "first sample should be faded"
        );
        // The first sample must be smaller (faded) than the raw input.
        assert!(
            result.samples[0].abs() < samples[0].abs() * 0.5,
            "fade-in should reduce initial sample significantly"
        );
        // Samples beyond the fade-in region (5 ms = 240 frames) should be untouched.
        let fade_margin = (0.005 * sr as f32) as usize * 2; // twice the fade length for safety
        for (i, sample) in samples.iter().enumerate().skip(fade_margin) {
            assert!(
                (result.samples[i] - sample).abs() < 0.001,
                "non-faded region changed at sample {i}"
            );
        }
    }

    #[test]
    fn sharp_end_peak_gets_fade_out() {
        let sr = 48000;
        let channels = 1;
        let frames = 1000;
        let mut samples: Vec<f32> = (0..frames)
            .map(|i| (2.0 * std::f32::consts::PI * 440.0 * i as f32 / sr as f32).sin() * 0.5)
            .collect();
        samples[frames - 1] = 0.95; // sharp jump at end
        let pcm = make_pcm(samples.clone(), sr, channels);
        let result = process_boundaries(&pcm);

        assert_eq!(result.frame_count(), frames);
        assert!(
            result.samples[frames - 1].abs() < 0.95,
            "last sample should be faded"
        );
        assert!(
            result.samples[frames - 1].abs() < samples[frames - 1].abs() * 0.5,
            "fade-out should reduce final sample"
        );
        // Early samples outside the fade-out region should be untouched.
        let fade_margin = (0.005 * sr as f32) as usize * 2;
        for (i, sample) in samples.iter().enumerate().take(frames - fade_margin) {
            assert!(
                (result.samples[i] - sample).abs() < 0.001,
                "non-faded region changed at sample {i}"
            );
        }
    }

    #[test]
    fn very_short_phrase_no_panic() {
        let pcm = make_pcm(vec![0.5, -0.3], 48000, 1);
        let result = process_boundaries(&pcm);
        assert_eq!(result.frame_count(), 2);
        assert_eq!(result.channels, 1);
        assert!(result.samples.iter().all(|s| s.is_finite()));
    }

    #[test]
    fn empty_phrase_safe() {
        let pcm = make_pcm(vec![], 48000, 1);
        let result = process_boundaries(&pcm);
        assert!(result.samples.is_empty());
        assert_eq!(result.channels, 1);
        assert_eq!(result.sample_rate, 48000);
    }

    #[test]
    fn single_frame_phrase_safe() {
        let pcm = make_pcm(vec![0.5], 44100, 1);
        let result = process_boundaries(&pcm);
        assert_eq!(result.frame_count(), 1);
        assert_eq!(result.samples, vec![0.5]);
    }

    #[test]
    fn phrase_shorter_than_fade_no_panic() {
        let sr = 48000;
        // 5 ms fade = 240 samples. 4 frames ≪ 240.
        let pcm = make_pcm(vec![0.9, -0.9, 0.5, -0.5], sr, 1);
        let result = process_boundaries(&pcm);
        assert_eq!(result.frame_count(), 4);
        assert!(result.samples.iter().all(|s| s.is_finite()));
    }

    // -- crossfade -----------------------------------------------------------

    #[test]
    fn crossfade_preserves_validity_channels_and_length() {
        let sr = 48000;
        let a = make_pcm((0..4800).map(|i| (i as f32 * 0.001).sin()).collect(), sr, 1);
        let b = make_pcm(
            (0..4800).map(|i| (i as f32 * 0.001 + 1.0).sin()).collect(),
            sr,
            1,
        );

        let fade_secs = 0.01;
        let result = crossfade(&a, &b, fade_secs).expect("crossfade should succeed");

        assert_eq!(result.sample_rate, sr);
        assert_eq!(result.channels, 1);

        let a_frames = a.frame_count();
        let b_frames = b.frame_count();
        let fade_frames = (fade_secs * sr as f32) as usize;
        let expected_frames = a_frames + b_frames - fade_frames;
        assert_eq!(
            result.frame_count(),
            expected_frames,
            "expected {expected_frames} frames, got {}",
            result.frame_count()
        );
        assert!(
            result.samples.iter().all(|s| s.is_finite()),
            "crossfade output must be finite"
        );
    }

    #[test]
    fn crossfade_incompatible_rate_rejected() {
        let a = make_pcm(vec![0.0, 0.5, -0.5], 44100, 1);
        let b = make_pcm(vec![0.0, 0.5, -0.5], 48000, 1);
        assert!(crossfade(&a, &b, 0.01).is_err());
    }

    #[test]
    fn crossfade_incompatible_channels_rejected() {
        let a = make_pcm(vec![0.0, 0.5, -0.5], 44100, 1);
        let b = make_pcm(vec![0.0, 0.5, -0.5, 0.5], 44100, 2);
        assert!(crossfade(&a, &b, 0.01).is_err());
    }

    #[test]
    fn crossfade_no_nan_inf() {
        let sr = 48000;
        let frames = 4800;
        let samples: Vec<f32> = (0..frames).map(|i| (i as f32 * 0.01).sin() * 0.5).collect();
        let a = make_pcm(samples.clone(), sr, 1);
        let b = make_pcm(samples, sr, 1);

        let result = crossfade(&a, &b, 0.05).expect("crossfade succeed");
        for (i, &s) in result.samples.iter().enumerate() {
            assert!(s.is_finite(), "non-finite sample at index {i}: {s}");
        }
    }

    #[test]
    fn crossfade_fade_clamped_to_available_length() {
        let sr = 48000;
        let a = make_pcm(vec![0.0, 0.5, -0.5], sr, 1); // 3 frames
        let b = make_pcm(vec![0.2, -0.2, 0.1], sr, 1); // 3 frames

        // fade_secs would normally exceed available duration.
        let result = crossfade(&a, &b, 0.5).expect("crossfade with long fade");

        // fade clamped to min(3, 3) = 3 frames → 3 + 3 - 3 = 3 output frames.
        assert_eq!(result.frame_count(), 3);
        assert!(result.samples.iter().all(|s| s.is_finite()));
    }

    #[test]
    fn crossfade_empty_inputs() {
        let a = make_pcm(vec![], 48000, 1);
        let b = make_pcm(vec![0.3, -0.3], 48000, 1);
        let result = crossfade(&a, &b, 0.01).expect("crossfade empty a");
        assert_eq!(result.frame_count(), 2);

        let a2 = make_pcm(vec![0.3, -0.3], 48000, 1);
        let b2 = make_pcm(vec![], 48000, 1);
        let result2 = crossfade(&a2, &b2, 0.01).expect("crossfade empty b");
        assert_eq!(result2.frame_count(), 2);
    }

    // -- DC offset -----------------------------------------------------------

    #[test]
    fn dc_offset_removed_when_significant() {
        let sr = 48000;
        let mut samples: Vec<f32> = (0..1000)
            .map(|i| (2.0 * std::f32::consts::PI * 440.0 * i as f32 / sr as f32).sin() * 0.3)
            .collect();
        for s in &mut samples {
            *s += 0.05; // significant DC offset > 0.005
        }
        let pcm = make_pcm(samples, sr, 1);
        let result = process_boundaries(&pcm);

        let mean_after: f32 = result.samples.iter().sum::<f32>() / result.samples.len() as f32;
        assert!(
            mean_after.abs() < 0.005,
            "DC offset should be removed: got mean {mean_after}"
        );
    }

    #[test]
    fn normal_speech_dc_not_removed() {
        let sr = 48000;
        let samples: Vec<f32> = (0..1000)
            .map(|i| (2.0 * std::f32::consts::PI * 440.0 * i as f32 / sr as f32).sin() * 0.5)
            .collect();
        let pcm = make_pcm(samples.clone(), sr, 1);
        let result = process_boundaries(&pcm);
        for (a, b) in samples.iter().zip(result.samples.iter()) {
            assert!((a - b).abs() < 0.001);
        }
    }
}
