//! Audio post-processing effects for TTS
//!
//! Provides pitch, speed, and volume adjustments

use symphonia::core::codecs::{CODEC_TYPE_NULL, DecoderOptions};
use symphonia::core::formats::FormatOptions;
use symphonia::core::io::MediaSourceStream;
use symphonia::core::meta::MetadataOptions;
use symphonia::core::probe::Hint;
use symphonia::core::audio::Signal;
use rubato::{FftFixedInOut, Resampler};
use pitch_shift::PitchShifter;

/// Audio effects configuration
#[derive(Debug, Clone, Copy)]
pub struct AudioEffects {
    pub pitch: i16,   // -100 to +100 (проценты)
    pub speed: i16,   // -100 to +100 (проценты)
    pub volume: i16,  // 0 to 200 (проценты, 100 = норма)
}

impl AudioEffects {
    pub fn new(pitch: i16, speed: i16, volume: i16) -> Self {
        Self {
            pitch: pitch.clamp(-100, 100),
            speed: speed.clamp(-100, 100),
            volume: volume.clamp(0, 200),
        }
    }

    /// Check if any effects are active
    #[allow(dead_code)]
    pub fn is_active(&self) -> bool {
        self.pitch != 0 || self.speed != 0 || self.volume != 100
    }

    /// Convert volume percentage to amplification factor
    /// 0% = 0.0, 100% = 1.0, 200% = 2.0
    pub fn volume_factor(&self) -> f32 {
        self.volume as f32 / 100.0
    }

    /// Convert speed percentage to playback rate
    /// Negative = slower (down to 0.25x), 0 = normal (1x), Positive = faster (up to 4x)
    /// -100% = 0.25x (slower), 0% = 1.0x (normal), +100% = 4.0x (faster)
    ///
    /// Note: This returns the INVERSE of the playback rate because resampling works inversely:
    /// - Faster playback (>1x) requires FEWER samples → lower sample rate (< original)
    /// - Slower playback (<1x) requires MORE samples → higher sample rate (> original)
    pub fn speed_factor(&self) -> f32 {
        if self.speed == 0 {
            1.0
        } else if self.speed < 0 {
            // -100 to 0 maps to 0.25 to 1.0 (slower to normal)
            // Invert: 0.25 → 4.0, 1.0 → 1.0
            let playback_rate = 1.0 - (self.speed as f32 / 100.0).abs() * 0.75;
            1.0 / playback_rate
        } else {
            // 0 to +100 maps to 1.0 to 4.0 (normal to faster)
            // Invert: 1.0 → 1.0, 4.0 → 0.25
            let playback_rate = 1.0 + (self.speed as f32 / 100.0) * 3.0;
            1.0 / playback_rate
        }
    }

    /// Convert pitch percentage to semitones
    /// -100% = -12 semitones, 0% = 0, +100% = +12 semitones
    pub fn pitch_semitones(&self) -> f32 {
        (self.pitch as f32 / 100.0) * 12.0
    }

    /// Convert pitch percentage to frequency ratio
    /// Each semitone = 2^(1/12) ≈ 1.059463
    /// -100% = 2.0x (octave up, chipmunk), 0% = 1.0x, +100% = 0.5x (octave down, bass)
    ///
    /// NOTE: This function is kept for reference but is no longer used.
    /// The new implementation uses semitones directly with phase vocoder.
    #[allow(dead_code)]
    pub fn pitch_semitones_to_ratio(&self) -> f32 {
        let semitones = -self.pitch_semitones(); // Invert sign for correct behavior
        2.0_f32.powf(semitones / 12.0)
    }
}

/// Apply audio effects to MP3 data
///
/// Returns processed MP3 data or original if no effects active
///
/// Processing pipeline:
/// 1. Decode MP3 to PCM
/// 2. Apply speed change (resampling via rubato)
/// 3. Apply pitch shift (phase vocoder - NO duration change)
/// 4. Re-encode to WAV
///
/// Note: Volume is NOT applied here - it's handled during playback via rodio
/// Note: Pitch and speed are now INDEPENDENT (no chipmunk effect)
pub fn apply_effects(mp3_data: Vec<u8>, effects: &AudioEffects) -> Result<Vec<u8>, String> {
    // Only process if pitch or speed effects are active
    if effects.pitch == 0 && effects.speed == 0 {
        return Ok(mp3_data);
    }

    // Decode MP3 to PCM
    let pcm_data = decode_mp3(&mp3_data)?;

    let mut samples = pcm_data.samples;
    let sample_rate = pcm_data.sample_rate;
    let channels = pcm_data.channels;

    // Apply speed change if needed (changes duration)
    if effects.speed != 0 {
        let speed_factor = effects.speed_factor();
        samples = apply_speed(&samples, sample_rate, speed_factor, channels)?;
        // Trim zero-padding artifacts from resampling (aggressive mode)
        samples = trim_silence(&samples, channels, sample_rate, true);
    }

    // Apply pitch shift if needed (NO duration change - phase vocoder)
    if effects.pitch != 0 {
        let semitones = effects.pitch_semitones();
        samples = apply_pitch(&samples, sample_rate, semitones, channels)?;

        // Only trim after pitch for significant negative values (pitching down by > 5 semitones)
        // Small changes (< 5 semitones) don't get trimmed because:
        // - Phase vocoder artifacts are similar for both positive and negative small shifts
        // - Trim cuts off speech tails that naturally decay faster
        // - Only large negative shifts preserve enough energy for safe trimming
        if semitones < -5.0 {
            samples = trim_silence(&samples, channels, sample_rate, false);
            tracing::debug!(
                semitones,
                "Applied trim after pitch (large negative pitch shift)"
            );
        } else {
            tracing::debug!(
                semitones,
                "Skipped trim after pitch (small pitch shift - preserving full audio)"
            );
        }
    }

    // Encode PCM back to WAV
    encode_wav(&samples, sample_rate, channels)
}

/// Decoded PCM data
struct PcmData {
    samples: Vec<f32>,
    sample_rate: u32,
    channels: usize,
}

/// Decode MP3 to PCM using symphonia
fn decode_mp3(mp3_data: &[u8]) -> Result<PcmData, String> {
    use std::io::Cursor;

    // Clone the data to own it (required for MediaSourceStream)
    let data = mp3_data.to_vec();
    let cursor = Cursor::new(data);
    let mss = MediaSourceStream::new(Box::new(cursor), Default::default());

    let mut hint = Hint::new();
    hint.with_extension("mp3");

    let meta_opts: MetadataOptions = Default::default();
    let fmt_opts: FormatOptions = Default::default();

    let probed = symphonia::default::get_probe()
        .format(&hint, mss, &fmt_opts, &meta_opts)
        .map_err(|e| format!("Failed to probe MP3: {}", e))?;

    let mut format = probed.format;

    let track = format
        .tracks()
        .iter()
        .find(|t| t.codec_params.codec != CODEC_TYPE_NULL)
        .ok_or("No valid audio track found")?;

    let decoder = symphonia::default::get_codecs()
        .make(&track.codec_params, &DecoderOptions::default())
        .map_err(|e| format!("Failed to create decoder: {}", e))?;

    let sample_rate = track
        .codec_params
        .sample_rate
        .ok_or("No sample rate")? as u32;

    let channels = track.codec_params.channels.ok_or("No channels")?.count();

    let mut samples = Vec::new();
    let mut decoder = decoder;

    loop {
        let packet = match format.next_packet() {
            Ok(packet) => packet,
            Err(symphonia::core::errors::Error::ResetRequired) => {
                // Continue after reset
                continue;
            }
            Err(_) => break,
        };

        match decoder.decode(&packet) {
            Ok(audio_buf) => {
                // Convert audio buffer to f32 samples
                match audio_buf {
                    symphonia::core::audio::AudioBufferRef::F32(buf) => {
                        // AudioBuffer has .chan() method to get channel data
                        let num_channels = buf.spec().channels.count();
                        for ch in 0..num_channels {
                            for &sample in buf.chan(ch) {
                                samples.push(sample);
                            }
                        }
                    }
                    symphonia::core::audio::AudioBufferRef::S16(buf) => {
                        // Convert from i16 to f32
                        let num_channels = buf.spec().channels.count();
                        for ch in 0..num_channels {
                            for &sample in buf.chan(ch) {
                                samples.push(sample as f32 / 32768.0);
                            }
                        }
                    }
                    symphonia::core::audio::AudioBufferRef::S24(buf) => {
                        // Convert from i24 to f32 (use .inner() method)
                        let num_channels = buf.spec().channels.count();
                        for ch in 0..num_channels {
                            for &sample in buf.chan(ch) {
                                samples.push(sample.inner() as f32 / 8388608.0);
                            }
                        }
                    }
                    symphonia::core::audio::AudioBufferRef::S32(buf) => {
                        // Convert from i32 to f32
                        let num_channels = buf.spec().channels.count();
                        for ch in 0..num_channels {
                            for &sample in buf.chan(ch) {
                                samples.push(sample as f32 / 2147483648.0);
                            }
                        }
                    }
                    symphonia::core::audio::AudioBufferRef::U8(buf) => {
                        // Convert from u8 to f32
                        let num_channels = buf.spec().channels.count();
                        for ch in 0..num_channels {
                            for &sample in buf.chan(ch) {
                                samples.push((sample as f32 - 128.0) / 128.0);
                            }
                        }
                    }
                    _ => {
                        // Unsupported format
                        return Err("Unsupported audio format".to_string());
                    }
                }
            }
            Err(symphonia::core::errors::Error::DecodeError(_)) => {
                // Skip decode errors
                continue;
            }
            Err(e) => return Err(format!("Decoder error: {}", e)),
        }
    }

    Ok(PcmData {
        samples,
        sample_rate,
        channels,
    })
}

/// Apply speed change using resampling
fn apply_speed(
    samples: &[f32],
    sample_rate: u32,
    speed_factor: f32,
    channels: usize,
) -> Result<Vec<f32>, String> {
    if (speed_factor - 1.0).abs() < 0.001 {
        return Ok(samples.to_vec());
    }

    // Calculate new sample rate based on speed factor
    let new_sample_rate = (sample_rate as f32 * speed_factor).round() as usize;

    tracing::debug!(
        input_samples = samples.len(),
        sample_rate,
        speed_factor,
        new_sample_rate,
        channels,
        "apply_speed: Starting resampling"
    );

    // Create resampler with automatic buffer size calculation
    let mut resampler = FftFixedInOut::new(
        sample_rate as usize,
        new_sample_rate,
        1024, // recommended chunk size (resampler will calculate actual buffer sizes)
        channels,
    )
    .map_err(|e| format!("Failed to create resampler: {}", e))?;

    // Get the actual input buffer size required by the resampler
    let input_buffer_size = resampler.input_frames_max();

    tracing::debug!(
        input_buffer_size,
        "apply_speed: Resampler created"
    );

    // De-interleave samples
    let mut interleaved: Vec<Vec<f32>> = vec![Vec::new(); channels];
    for (i, &sample) in samples.iter().enumerate() {
        interleaved[i % channels].push(sample);
    }

    tracing::debug!(
        samples_per_channel = interleaved[0].len(),
        "apply_speed: De-interleaved"
    );

    // Resample all channels together (rubato handles multi-channel)
    let mut resampled = vec![Vec::new(); channels];
    let mut input_idx = 0;
    let mut chunk_count = 0;

    while input_idx < interleaved[0].len() {
        let end = (input_idx + input_buffer_size).min(interleaved[0].len());

        // Prepare input for all channels
        let chunks: Vec<&[f32]> = (0..channels)
            .map(|ch| &interleaved[ch][input_idx..end])
            .collect();

        // Only process if we have enough data (rubato needs exact buffer size)
        if chunks[0].len() == input_buffer_size {
            match resampler.process(&chunks, None) {
                Ok(output) => {
                    // output is Vec<Vec<f32>> - one vector per channel
                    for (ch, channel_output) in output.iter().enumerate() {
                        resampled[ch].extend_from_slice(channel_output);
                    }
                    chunk_count += 1;
                }
                Err(_e) => {
                    #[cfg(debug_assertions)]
                    tracing::debug!("Rubato process error");
                }
            }
        }

        input_idx = end;

        // If we have less than a full buffer left, create a new resampler for the remainder
        if input_idx < interleaved[0].len() && (interleaved[0].len() - input_idx) < input_buffer_size {
            let remaining = interleaved[0].len() - input_idx;

            tracing::debug!(
                remaining,
                input_idx,
                total_input = interleaved[0].len(),
                "apply_speed: Processing final partial buffer with dedicated resampler"
            );

            // Create a new resampler sized for the remaining samples
            // Note: FftFixedInOut may adjust the buffer size for FFT requirements
            let final_chunk_size = remaining.max(64); // Minimum size for FFT
            let mut final_resampler = FftFixedInOut::new(
                sample_rate as usize,
                new_sample_rate,
                final_chunk_size,
                channels,
            )
            .map_err(|e| format!("Failed to create final resampler: {}", e))?;

            let final_input_buffer_size = final_resampler.input_frames_max();

            // Prepare the final chunks with zero-padding if needed
            let final_chunks: Vec<Vec<f32>> = (0..channels)
                .map(|ch| {
                    let mut chunk = interleaved[ch][input_idx..].to_vec();
                    // Zero-pad to the required buffer size
                    while chunk.len() < final_input_buffer_size {
                        chunk.push(0.0);
                    }
                    chunk
                })
                .collect();

            tracing::debug!(
                final_chunk_size,
                final_input_buffer_size,
                actual_input_len = interleaved[0].len() - input_idx,
                padded_len = final_chunks[0].len(),
                "apply_speed: Final resampler created with zero-padding"
            );

            // Process the final chunk (with zero-padding)
            let final_chunks_refs: Vec<&[f32]> = final_chunks.iter().map(|v| v.as_slice()).collect();
            match final_resampler.process(&final_chunks_refs, None) {
                Ok(output) => {
                    for (ch, channel_output) in output.iter().enumerate() {
                        resampled[ch].extend_from_slice(channel_output);
                    }
                    chunk_count += 1;
                }
                Err(_e) => {
                    #[cfg(debug_assertions)]
                    tracing::debug!("Rubato process error (final chunk)");
                }
            }

            break;
        }
    }

    let expected_output = (interleaved[0].len() as f32 * speed_factor).round() as usize;
    let sample_diff = resampled[0].len() as isize - expected_output as isize;

    tracing::debug!(
        chunks_processed = chunk_count,
        output_samples_per_channel = resampled[0].len(),
        expected_output,
        sample_diff,
        "apply_speed: Resampling complete"
    );

    // Trim excess samples from zero-padding artifacts
    // Zero-padding in final chunks can add extra silence at the end
    for channel in resampled.iter_mut() {
        if channel.len() > expected_output {
            channel.truncate(expected_output);
        }
    }

    // Interleave back
    let max_len = resampled.iter().map(|v| v.len()).max().unwrap_or(0);
    let mut result = Vec::with_capacity(max_len * channels);
    for sample_idx in 0..max_len {
        for channel in &resampled {
            result.push(channel.get(sample_idx).copied().unwrap_or(0.0));
        }
    }

    Ok(result)
}

/// Apply pitch shift using phase vocoder
///
/// This changes pitch WITHOUT changing duration, unlike resampling-based approaches.
/// Uses the PitchShifter which implements the phase vocoder algorithm.
fn apply_pitch(
    samples: &[f32],
    sample_rate: u32,
    semitones: f32,
    channels: usize,
) -> Result<Vec<f32>, String> {
    if semitones.abs() < 0.01 {
        return Ok(samples.to_vec());
    }

    tracing::debug!(
        input_samples = samples.len(),
        sample_rate,
        semitones,
        channels,
        "apply_pitch: Starting phase vocoder pitch shift"
    );

    // Process per channel (phase vocoder works on mono)
    let mut result = Vec::new();

    for ch in 0..channels {
        // De-interleave channel samples
        let channel_samples: Vec<f32> = samples
            .iter()
            .skip(ch)
            .step_by(channels)
            .copied()
            .collect();

        // Create pitch shifter with 50ms window duration (recommended)
        let mut shifter = PitchShifter::new(50, sample_rate as usize);

        // Create output buffer
        let mut pitched = vec![0.0; channel_samples.len()];

        // Apply pitch shift
        // over_sampling=16 for good quality, shift in semitones
        shifter.shift_pitch(16, semitones, &channel_samples, &mut pitched);

        // Interleave back into result
        for (i, &sample) in pitched.iter().enumerate() {
            // Ensure we have space for this sample
            let pos = i * channels + ch;
            if pos >= result.len() {
                result.resize(pos + 1, 0.0);
            }
            result[pos] = sample;
        }
    }

    // Fill in any missing samples for multi-channel audio
    let total_samples = (result.len() / channels) * channels;
    result.resize(total_samples, 0.0);

    tracing::debug!(
        output_samples = result.len(),
        "apply_pitch: Pitch shift complete"
    );

    Ok(result)
}

/// Apply exponential fade-out to prevent clicks and phase discontinuities
///
/// Smoothly reduces amplitude over the specified duration using an exponential curve.
/// This prevents abrupt edges that cause artifacts in FFT-based processing.
///
/// # Arguments
/// * `samples` - Audio samples to modify (interleaved if multi-channel)
/// * `fade_samples` - Number of samples over which to apply fade (per channel)
/// * `channels` - Number of audio channels (1=mono, 2=stereo)
fn apply_fade_out(samples: &mut [f32], fade_samples: usize, channels: usize) {
    let total_samples = samples.len() / channels;

    if fade_samples == 0 || fade_samples > total_samples {
        return;
    }

    // Apply exponential fade-out from the end
    for (frame_idx, chunk) in samples.chunks_exact_mut(channels).rev().enumerate() {
        if frame_idx >= fade_samples {
            break;
        }

        // Exponential curve: more gradual at start, steeper at end
        let progress = frame_idx as f32 / fade_samples as f32;
        let gain = (1.0 - progress).powi(2); // Quadratic for smoother decay

        for sample in chunk {
            *sample *= gain;
        }
    }
}

/// Trim silence from the end of audio with fade-out to prevent phase artifacts
///
/// Analyzes amplitude from the end and finds where real audio ends, then applies
/// an exponential fade-out instead of a hard cut. This prevents phase discontinuities
/// and clicks that would be amplified by FFT-based processing (resampling, pitch shifting).
///
/// # Arguments
/// * `aggressive` - If true, uses shorter tail for zero-padding cleanup (after speed).
///   If false, preserves longer tail for speech natural decay (after pitch).
fn trim_silence(samples: &[f32], channels: usize, sample_rate: u32, aggressive: bool) -> Vec<f32> {
    // Aggressive mode: short tail for zero-padding artifacts
    // Conservative mode: longer tail to preserve speech endings (fricatives, breaths)
    let (threshold, tail_ms, fade_ms) = if aggressive {
        (0.001, 5u32, 5u32)   // Short tail + fast fade for zero-padding
    } else {
        (0.005, 50u32, 30u32)  // Long tail + slow fade for speech preservation
    };

    let tail_samples = (sample_rate * tail_ms / 1000) as usize * channels;
    let fade_samples = (sample_rate * fade_ms / 1000) as usize * channels;
    let min_silence_samples = (sample_rate * 20 / 1000) as usize * channels; // 20ms minimum silence

    if samples.len() < min_silence_samples {
        return samples.to_vec();
    }

    // Analyze from the end, working backwards to find silence start
    let mut silence_count = 0;
    let mut silence_start_idx = samples.len();

    for (i, chunk) in samples.chunks_exact(channels).rev().enumerate() {
        let is_silent = chunk.iter().all(|&s| s.abs() < threshold);

        if is_silent {
            silence_count += channels;
            if silence_count >= min_silence_samples {
                // Found silence start - keep tail before this
                silence_start_idx = samples.len() - (i * channels) - silence_count + channels;
                break;
            }
        } else {
            silence_count = 0;
        }
    }

    // Determine final cut point, preserving tail
    let end_idx = if silence_start_idx + tail_samples < samples.len() {
        silence_start_idx + tail_samples
    } else {
        samples.len()
    };

    let mut trimmed = samples[..end_idx.min(samples.len())].to_vec();

    // Apply exponential fade-out to the tail to prevent phase artifacts
    apply_fade_out(&mut trimmed, fade_samples / channels, channels);

    tracing::debug!(
        original_samples = samples.len(),
        trimmed_samples = trimmed.len(),
        removed_samples = samples.len() - trimmed.len(),
        tail_ms,
        fade_ms,
        aggressive,
        "trim_silence: Removed trailing silence with fade-out"
    );

    trimmed
}

/// Encode PCM samples to WAV
///
/// WAV format: RIFF header + fmt chunk + data chunk
/// Simple container for PCM data
fn encode_wav(samples: &[f32], sample_rate: u32, channels: usize) -> Result<Vec<u8>, String> {
    use std::io::{Cursor, Write};

    // Clamp and convert to i16
    let i16_samples: Vec<i16> = samples
        .iter()
        .map(|&s| {
            let clamped = s.clamp(-1.0, 1.0);
            (clamped * 32767.0) as i16
        })
        .collect();

    let data_size = i16_samples.len() * 2; // 2 bytes per i16 sample
    let file_size = 36 + data_size; // RIFF header size + data

    let mut wav_data = Vec::with_capacity(file_size);
    let mut cursor = Cursor::new(&mut wav_data);

    // RIFF header
    cursor.write_all(b"RIFF").map_err(|e| format!("Failed to write RIFF: {}", e))?;
    cursor.write_all(&(file_size as u32).to_le_bytes())
        .map_err(|e| format!("Failed to write file size: {}", e))?;
    cursor.write_all(b"WAVE").map_err(|e| format!("Failed to write WAVE: {}", e))?;

    // fmt chunk
    cursor.write_all(b"fmt ").map_err(|e| format!("Failed to write fmt: {}", e))?;
    cursor.write_all(&16u32.to_le_bytes()) // fmt chunk size
        .map_err(|e| format!("Failed to write fmt size: {}", e))?;
    cursor.write_all(&1u16.to_le_bytes()) // PCM format
        .map_err(|e| format!("Failed to write format: {}", e))?;
    cursor.write_all(&(channels as u16).to_le_bytes())
        .map_err(|e| format!("Failed to write channels: {}", e))?;
    cursor.write_all(&sample_rate.to_le_bytes())
        .map_err(|e| format!("Failed to write sample rate: {}", e))?;
    let byte_rate = sample_rate * channels as u32 * 2;
    cursor.write_all(&byte_rate.to_le_bytes())
        .map_err(|e| format!("Failed to write byte rate: {}", e))?;
    let block_align = channels as u16 * 2;
    cursor.write_all(&block_align.to_le_bytes())
        .map_err(|e| format!("Failed to write block align: {}", e))?;
    cursor.write_all(&16u16.to_le_bytes()) // bits per sample
        .map_err(|e| format!("Failed to write bits per sample: {}", e))?;

    // data chunk
    cursor.write_all(b"data").map_err(|e| format!("Failed to write data: {}", e))?;
    cursor.write_all(&(data_size as u32).to_le_bytes())
        .map_err(|e| format!("Failed to write data size: {}", e))?;

    // Write sample data
    for sample in i16_samples {
        cursor.write_all(&sample.to_le_bytes())
            .map_err(|e| format!("Failed to write sample: {}", e))?;
    }

    Ok(wav_data)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_volume_factor() {
        let effects = AudioEffects::new(0, 0, 100);
        assert_eq!(effects.volume_factor(), 1.0);

        let effects = AudioEffects::new(0, 0, 0);
        assert_eq!(effects.volume_factor(), 0.0);

        let effects = AudioEffects::new(0, 0, 200);
        assert_eq!(effects.volume_factor(), 2.0);
    }

    #[test]
    fn test_speed_factor() {
        let effects = AudioEffects::new(0, 0, 100);
        assert_eq!(effects.speed_factor(), 1.0);

        let effects = AudioEffects::new(0, -100, 100);
        // Slower: 0.25x playback → needs 4x the samples → factor=4.0
        assert_eq!(effects.speed_factor(), 4.0);

        let effects = AudioEffects::new(0, 100, 100);
        // Faster: 4x playback → needs 0.25x the samples → factor=0.25
        assert_eq!(effects.speed_factor(), 0.25);

        // Test intermediate values
        let effects = AudioEffects::new(0, -40, 100);
        // Slower: 0.7x playback → needs 1/0.7 ≈ 1.43 samples
        assert!((effects.speed_factor() - 1.43).abs() < 0.01);

        let effects = AudioEffects::new(0, 50, 100);
        // Faster: 2.5x playback → needs 1/2.5 = 0.4 samples
        assert!((effects.speed_factor() - 0.4).abs() < 0.01);
    }

    #[test]
    fn test_pitch_semitones() {
        let effects = AudioEffects::new(0, 0, 100);
        assert_eq!(effects.pitch_semitones(), 0.0);

        let effects = AudioEffects::new(-100, 0, 100);
        assert_eq!(effects.pitch_semitones(), -12.0);

        let effects = AudioEffects::new(100, 0, 100);
        assert_eq!(effects.pitch_semitones(), 12.0);
    }

    #[test]
    fn test_is_active() {
        let effects = AudioEffects::new(0, 0, 100);
        assert!(!effects.is_active());

        let effects = AudioEffects::new(50, 0, 100);
        assert!(effects.is_active());

        let effects = AudioEffects::new(0, -50, 100);
        assert!(effects.is_active());

        let effects = AudioEffects::new(0, 0, 150);
        assert!(effects.is_active());
    }

    #[test]
    fn test_clamping() {
        let effects = AudioEffects::new(-200, 200, 300);
        assert_eq!(effects.pitch, -100);
        assert_eq!(effects.speed, 100);
        assert_eq!(effects.volume, 200);
    }
}
