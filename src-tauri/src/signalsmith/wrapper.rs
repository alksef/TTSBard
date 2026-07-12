use std::ffi::c_void;

extern "C" {
    fn signalsmith_stretch_create(channels: i32, sample_rate: f32) -> *mut c_void;
    fn signalsmith_stretch_destroy(ptr: *mut c_void);
    fn signalsmith_stretch_process(
        ptr: *mut c_void,
        input: *const f32,
        input_frames: i32,
        output: *mut f32,
        output_capacity_frames: i32,
        time_factor: f32,
        pitch_semitones: f32,
        preserve_formants: i32,
    ) -> i32;
    #[allow(dead_code)]
    fn signalsmith_stretch_input_latency(ptr: *mut c_void) -> i32;
    #[allow(dead_code)]
    fn signalsmith_stretch_output_latency(ptr: *mut c_void) -> i32;
}

#[derive(Debug)]
pub struct StretchError(pub String);

impl std::fmt::Display for StretchError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "SignalsmithStretch error: {}", self.0)
    }
}

impl std::error::Error for StretchError {}

/// RAII wrapper around a Signalsmith Stretch native processor.
///
/// Created per phrase — creates a fresh processor, processes the full interleaved
/// PCM buffer offline, and destroys the processor on drop.
pub struct StretchProcessor {
    ptr: *mut c_void,
    channels: usize,
    #[allow(dead_code)]
    sample_rate: u32,
}

unsafe impl Send for StretchProcessor {}

impl StretchProcessor {
    /// Create a new stretch processor with the given configuration.
    ///
    /// # Errors
    /// Returns `StretchError` if `channels` is 0, sample_rate is out of bounds,
    /// or native allocation fails.
    pub fn new(channels: usize, sample_rate: u32) -> Result<Self, StretchError> {
        if channels == 0 {
            return Err(StretchError("channels must be >= 1".into()));
        }
        if !(8000..=384000).contains(&sample_rate) {
            return Err(StretchError(format!(
                "sample_rate {} out of valid range 8000..384000",
                sample_rate
            )));
        }
        let ptr = unsafe { signalsmith_stretch_create(channels as i32, sample_rate as f32) };
        if ptr.is_null() {
            return Err(StretchError(
                "failed to create SignalsmithStretch processor".into(),
            ));
        }
        Ok(Self {
            ptr,
            channels,
            sample_rate,
        })
    }

    /// Process interleaved float PCM with the given tempo and pitch settings.
    ///
    /// # Arguments
    /// * `input` — interleaved f32 samples (`len = frames × channels`)
    /// * `time_factor` — tempo factor: 1.0 = no change, <1.0 = slower (longer output),
    ///   >1.0 = faster (shorter output). Allowed range: 0.25..4.0.
    /// * `pitch_semitones` — pitch shift in semitones: 0.0 = no change,
    ///   +12 = octave up, -12 = octave down. Allowed range: -24..+24.
    /// * `preserve_formants` — enable formant correction to maintain natural timbre.
    ///
    /// # Returns
    /// The processed interleaved f32 output.
    ///
    /// # Errors
    /// Returns `StretchError` on invalid input or processing failure.
    pub fn process(
        &mut self,
        input: &[f32],
        time_factor: f32,
        pitch_semitones: f32,
        preserve_formants: bool,
    ) -> Result<Vec<f32>, StretchError> {
        let frames = input.len() / self.channels;
        if input.len() % self.channels != 0 {
            return Err(StretchError(
                "input length not divisible by channel count".into(),
            ));
        }
        if frames == 0 {
            return Ok(Vec::new());
        }
        if time_factor <= 0.0 {
            return Err(StretchError("time_factor must be > 0".into()));
        }
        let time_factor = time_factor.clamp(0.25, 4.0);
        let pitch_semitones = pitch_semitones.clamp(-24.0, 24.0);

        let expected_output_frames = (frames as f32 / time_factor).round() as usize;

        let output_capacity = expected_output_frames.max(1);
        let mut output = vec![0.0f32; output_capacity * self.channels];

        let result = unsafe {
            signalsmith_stretch_process(
                self.ptr,
                input.as_ptr(),
                frames as i32,
                output.as_mut_ptr(),
                output_capacity as i32,
                time_factor,
                pitch_semitones,
                preserve_formants as i32,
            )
        };

        if result < 0 {
            return Err(StretchError(format!(
                "processing failed with code {}",
                result
            )));
        }

        let actual_frames = result as usize;
        output.truncate(actual_frames * self.channels);

        Ok(output)
    }

    #[allow(dead_code)]
    pub fn input_latency(&self) -> usize {
        unsafe { signalsmith_stretch_input_latency(self.ptr) as usize }
    }

    #[allow(dead_code)]
    pub fn output_latency(&self) -> usize {
        unsafe { signalsmith_stretch_output_latency(self.ptr) as usize }
    }
}

impl Drop for StretchProcessor {
    fn drop(&mut self) {
        if !self.ptr.is_null() {
            unsafe {
                signalsmith_stretch_destroy(self.ptr);
            }
            self.ptr = std::ptr::null_mut();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn generate_sine(freq: f32, sample_rate: u32, duration_secs: f32, channels: usize) -> Vec<f32> {
        let total_frames = (sample_rate as f32 * duration_secs) as usize;
        let mut samples = Vec::with_capacity(total_frames * channels);
        for i in 0..total_frames {
            let t = i as f32 / sample_rate as f32;
            let val = (2.0 * std::f32::consts::PI * freq * t).sin() * 0.8;
            for _ in 0..channels {
                samples.push(val);
            }
        }
        samples
    }

    fn zero_crossing_rate(samples: &[f32], channels: usize) -> f32 {
        if samples.len() < channels * 2 {
            return 0.0;
        }
        let mut crossings = 0usize;
        let total = samples.len() / channels;
        for ch in 0..channels {
            for i in 1..total {
                let prev = samples[(i - 1) * channels + ch];
                let curr = samples[i * channels + ch];
                if prev.signum() != curr.signum() && curr != 0.0 {
                    crossings += 1;
                }
            }
        }
        crossings as f32 / (total * channels) as f32
    }

    #[test]
    fn test_create_destroy() {
        let p = StretchProcessor::new(1, 48000).unwrap();
        assert!(p.input_latency() > 0);
        assert!(p.output_latency() > 0);
    }

    #[test]
    fn test_create_invalid_channels() {
        assert!(StretchProcessor::new(0, 48000).is_err());
    }

    #[test]
    fn test_create_invalid_sample_rate() {
        assert!(StretchProcessor::new(1, 100).is_err());
        assert!(StretchProcessor::new(1, 500000).is_err());
    }

    #[test]
    fn test_process_noop_mono() {
        let mut p = StretchProcessor::new(1, 48000).unwrap();
        let input = generate_sine(440.0, 48000, 0.5, 1);
        let output = p.process(&input, 1.0, 0.0, false).unwrap();
        assert!(!output.is_empty());
        assert!(output.iter().all(|s| s.is_finite()));
    }

    #[test]
    fn test_process_noop_stereo() {
        let mut p = StretchProcessor::new(2, 48000).unwrap();
        let input = generate_sine(440.0, 48000, 0.5, 2);
        let output = p.process(&input, 1.0, 0.0, false).unwrap();
        assert!(!output.is_empty());
        assert!(output.iter().all(|s| s.is_finite()));
    }

    #[test]
    fn test_tempo_only_preserves_pitch_mono_48k() {
        let sample_rate = 48000u32;
        let freq = 440.0;
        let duration = 0.3;
        let input = generate_sine(freq, sample_rate, duration, 1);

        let mut p = StretchProcessor::new(1, sample_rate).unwrap();
        let time_factor = 1.25;

        let output = p.process(&input, time_factor, 0.0, false).unwrap();

        let expected_duration = duration / time_factor;
        let expected_frames = (sample_rate as f32 * expected_duration) as usize;
        let actual_frames = output.len();
        let tolerance = (sample_rate as f32 * 0.02) as usize; // 20ms tolerance

        assert!(
            actual_frames.abs_diff(expected_frames) <= tolerance,
            "expected ~{expected_frames} frames, got {actual_frames}"
        );

        let input_zcr = zero_crossing_rate(&input, 1);
        let output_zcr = zero_crossing_rate(&output, 1);
        let zcr_ratio = output_zcr / input_zcr;
        assert!(
            (zcr_ratio - 1.0).abs() < 0.1,
            "pitch should be preserved: zcr ratio = {zcr_ratio}"
        );

        assert!(output.iter().all(|s| s.is_finite()));
    }

    #[test]
    fn test_tempo_only_preserves_pitch_stereo_44k1() {
        let sample_rate = 44100u32;
        let freq = 880.0;
        let duration = 0.3;
        let input = generate_sine(freq, sample_rate, duration, 2);

        let mut p = StretchProcessor::new(2, sample_rate).unwrap();
        let output = p.process(&input, 0.75, 0.0, false).unwrap();

        let expected_frames = (duration / 0.75 * sample_rate as f32).round() as usize;
        let actual_frames = output.len() / 2;
        let tolerance = (sample_rate as f32 * 0.02) as usize;

        assert!(
            actual_frames.abs_diff(expected_frames) <= tolerance,
            "expected ~{expected_frames} frames, got {actual_frames}"
        );

        // Check both channels have similar zero-crossing rate
        let mut ch0 = Vec::with_capacity(output.len() / 2);
        let mut ch1 = Vec::with_capacity(output.len() / 2);
        for (i, &s) in output.iter().enumerate() {
            if i % 2 == 0 {
                ch0.push(s);
            } else {
                ch1.push(s);
            }
        }
        let zcr0 = zero_crossing_rate(&ch0, 1);
        let zcr1 = zero_crossing_rate(&ch1, 1);
        assert!(
            (zcr0 - zcr1).abs() < 0.01,
            "stereo channels should have similar ZCR: {zcr0} vs {zcr1}"
        );
    }

    #[test]
    fn test_pitch_shift_does_not_change_duration() {
        let sample_rate = 48000u32;
        let input = generate_sine(440.0, sample_rate, 0.3, 1);
        let input_frames = input.len();

        let mut p = StretchProcessor::new(1, sample_rate).unwrap();
        let output = p.process(&input, 1.0, 6.0, false).unwrap();

        let output_frames = output.len();
        let tolerance = 2;
        assert!(
            output_frames.abs_diff(input_frames) <= tolerance,
            "pitch-only should preserve duration: {input_frames} vs {output_frames}"
        );
    }

    #[test]
    fn test_formant_preservation_enabled() {
        let sample_rate = 48000u32;
        let input = generate_sine(440.0, sample_rate, 0.2, 1);

        let mut p = StretchProcessor::new(1, sample_rate).unwrap();
        let output = p.process(&input, 1.0, 6.0, true).unwrap();

        assert!(!output.is_empty());
        assert!(output.iter().all(|s| s.is_finite()));
    }

    #[test]
    fn test_empty_input() {
        let mut p = StretchProcessor::new(1, 48000).unwrap();
        let output = p.process(&[], 1.0, 0.0, false).unwrap();
        assert!(output.is_empty());
    }

    #[test]
    fn test_invalid_time_factor() {
        let mut p = StretchProcessor::new(1, 48000).unwrap();
        assert!(p.process(&[0.0; 480], -1.0, 0.0, false).is_err());
        assert!(p.process(&[0.0; 480], 0.0, 0.0, false).is_err());
    }

    #[test]
    fn test_repeated_calls() {
        let sample_rate = 48000u32;
        let (freq1, freq2) = (440.0, 880.0);

        let mut p = StretchProcessor::new(1, sample_rate).unwrap();

        for _ in 0..3 {
            let input = generate_sine(freq1, sample_rate, 0.1, 1);
            let output = p.process(&input, 0.75, 0.0, false).unwrap();
            assert!(!output.is_empty());
            assert!(output.iter().all(|s| s.is_finite()));

            let input = generate_sine(freq2, sample_rate, 0.1, 1);
            let output = p.process(&input, 1.5, 3.0, true).unwrap();
            assert!(!output.is_empty());
            assert!(output.iter().all(|s| s.is_finite()));
        }
    }

    #[test]
    fn test_sample_rates_16k_24k() {
        for &sr in &[16000u32, 24000u32] {
            let mut p = StretchProcessor::new(1, sr).unwrap();
            let input = generate_sine(440.0, sr, 0.2, 1);
            let output = p.process(&input, 1.0, 0.0, false).unwrap();
            assert!(!output.is_empty());
            assert!(output.iter().all(|s| s.is_finite()));
        }
    }

    #[test]
    fn test_sample_rate_22k05() {
        let sr = 22050u32;
        let mut p = StretchProcessor::new(1, sr).unwrap();
        let input = generate_sine(440.0, sr, 0.2, 1);
        let output = p.process(&input, 1.0, 0.0, false).unwrap();
        assert!(!output.is_empty());
        assert!(output.iter().all(|s| s.is_finite()));
    }
}
