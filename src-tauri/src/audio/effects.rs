//! Audio post-processing effects for TTS
//!
//! Provides tempo, pitch, and volume adjustments using Signalsmith Stretch.

use symphonia::core::audio::Signal;
use symphonia::core::codecs::{DecoderOptions, CODEC_TYPE_NULL};
use symphonia::core::formats::FormatOptions;
use symphonia::core::io::MediaSourceStream;
use symphonia::core::meta::MetadataOptions;
use symphonia::core::probe::Hint;

use ndarray::prelude::*;
use std::cell::RefCell;
use std::collections::HashMap;
// The `deep_filter` crate exposes its library under the name `df`.
use df::tract::{DfParams, DfTract, RuntimeParams};

// Lazily-initialized DeepFilterNet model templates, keyed by channel count.
//
// The tract runtime (ONNX graph compilation) is the expensive part and is built
// exactly once per channel configuration. Actual processing clones a pristine
// template so streaming state never leaks between phrases.
//
// Uses `thread_local!` + `RefCell` instead of `static OnceLock<Mutex<...>>`
// because tract-core 0.21.4's `SimpleState<..., Box<dyn OpState>, ...>` is not
// `Send`/`Sync` (the `OpState` trait lacks a `Send` bound in this version).
thread_local! {
    static DF_TEMPLATES: RefCell<HashMap<usize, DfTract>> = RefCell::new(HashMap::new());
}

/// Get a fresh DeepFilterNet instance for the given channel count.
///
/// The heavy model initialization happens only once per channel count; subsequent
/// calls clone the cached template (cheap compared to rebuilding the tract graph).
fn get_df_model(channels: usize) -> Result<DfTract, String> {
    DF_TEMPLATES.with(|templates| {
        let mut guard = templates.borrow_mut();

        if !guard.contains_key(&channels) {
            tracing::info!(channels, "Initializing DeepFilterNet model (one-time)");
            let rp = RuntimeParams::default_with_ch(channels);
            let df_params = DfParams::default();
            let df = DfTract::new(df_params, &rp)
                .map_err(|e| format!("Failed to initialize DeepFilterNet: {:#}", e))?;
            guard.insert(channels, df);
        }

        Ok(guard
            .get(&channels)
            .expect("DeepFilterNet template must exist after insert")
            .clone())
    })
}

/// Audio effects configuration
#[derive(Debug, Clone, Copy)]
pub struct AudioEffects {
    pub pitch: i16,            // -100 to +100 (percent → -12..+12 semitones)
    pub speed: i16,            // -100 to +100 (percent → 0.75..1.50 tempo factor)
    pub volume: i16,           // 0 to 200 (percent, 100 = normal)
    pub enhance_enabled: bool, // DeepFilterNet noise suppression
    pub enhance_atten_db: f32, // attenuation limit in dB (5..30)
    pub fail_on_enhance_error: bool,
    pub formant_preserved: bool, // Signalsmith formant correction (default: true)
}

impl AudioEffects {
    pub fn new(pitch: i16, speed: i16, volume: i16) -> Self {
        Self {
            pitch: pitch.clamp(-100, 100),
            speed: speed.clamp(-100, 100),
            volume: volume.clamp(0, 200),
            enhance_enabled: false,
            enhance_atten_db: 12.0,
            fail_on_enhance_error: false,
            formant_preserved: true,
        }
    }

    /// Configure DeepFilterNet noise suppression (builder-style).
    ///
    /// `atten_db` is clamped to the supported 5..30 dB range.
    pub fn with_enhance(mut self, enabled: bool, atten_db: f32) -> Self {
        self.enhance_enabled = enabled;
        self.enhance_atten_db = atten_db.clamp(5.0, 30.0);
        self
    }

    /// Configure failure behavior on DeepFilterNet error (builder-style).
    pub fn with_fail_on_enhance_error(mut self, fail: bool) -> Self {
        self.fail_on_enhance_error = fail;
        self
    }

    /// Configure formant preservation (builder-style).
    pub fn with_formant_preserved(mut self, preserved: bool) -> Self {
        self.formant_preserved = preserved;
        self
    }

    /// Check if any effects are active
    #[allow(dead_code)]
    pub fn is_active(&self) -> bool {
        self.pitch != 0 || self.speed != 0 || self.volume != 100 || self.enhance_enabled
    }

    /// Convert volume percentage to amplification factor
    /// 0% = 0.0, 100% = 1.0, 200% = 2.0
    pub fn volume_factor(&self) -> f32 {
        self.volume as f32 / 100.0
    }

    /// Convert speed percentage to tempo factor for Signalsmith Stretch.
    ///
    /// Safe range: -100 → 0.75× (slower, longer output), 0 → 1.0× (normal),
    /// +100 → 1.50× (faster, shorter output).
    ///
    /// Note: the field retains the name `speed` for backward compatibility with
    /// existing storage/API, but the semantic is now **tempo** (time stretch with
    /// pitch preservation), NOT the old resampling-based speed change.
    pub fn speed_factor(&self) -> f32 {
        self.tempo_factor()
    }

    /// Convert speed slider position (-100..100) to tempo factor (0.75..1.50).
    ///
    /// - Negative values → slower (output is longer): 1.0 → 0.75
    /// - Zero → normal: 1.0
    /// - Positive values → faster (output is shorter): 1.0 → 1.50
    pub fn tempo_factor(&self) -> f32 {
        if self.speed <= 0 {
            // -100..0 → 0.75..1.0
            1.0 - (self.speed as f32 / 100.0).abs() * 0.25
        } else {
            // 0..+100 → 1.0..1.50
            1.0 + (self.speed as f32 / 100.0) * 0.50
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

/// Apply audio effects to audio data
///
/// Returns processed WAV data or original if no effects active
///
/// Processing pipeline:
/// 1. Decode audio to PCM (Symphonia probing handles WAV, MP3, and other formats)
/// 2. Apply DeepFilterNet noise suppression (if enabled)
/// 3. Apply Signalsmith Stretch (tempo + pitch + formant correction)
/// 4. Re-encode to WAV
///
/// Note: Volume is NOT applied here - it's handled during playback via rodio
/// Note: Pitch and tempo are now INDEPENDENT with formant correction via Signalsmith
pub fn apply_effects(audio_data: Vec<u8>, effects: &AudioEffects) -> Result<Vec<u8>, String> {
    // Only process if any effect is active
    if effects.pitch == 0 && effects.speed == 0 && !effects.enhance_enabled {
        return Ok(audio_data);
    }

    // Decode audio to PCM
    let pcm_data = decode_audio(&audio_data)?;

    let mut samples = pcm_data.samples;
    let mut sample_rate = pcm_data.sample_rate;
    let channels = pcm_data.channels;

    // Apply DeepFilterNet noise suppression first, on the clean decoded signal
    // (before tempo/pitch alter the time/frequency content).
    if effects.enhance_enabled {
        match apply_enhance(&samples, sample_rate, channels, effects.enhance_atten_db) {
            Ok(enhanced) => {
                samples = enhanced;
                sample_rate = 48000; // DeepFilterNet model output is always 48 kHz
                tracing::debug!(
                    atten_db = effects.enhance_atten_db,
                    channels,
                    "Applied DeepFilterNet enhancement"
                );
            }
            Err(e) => {
                if effects.fail_on_enhance_error {
                    return Err(format!("DeepFilterNet enhancement failed: {}", e));
                }
                tracing::error!(error = %e, "DeepFilterNet enhancement failed, skipping");
            }
        }
    }

    // Apply Signalsmith Stretch for tempo + pitch + formant correction
    if effects.speed != 0 || effects.pitch != 0 {
        samples = apply_stretch(
            &samples,
            channels,
            sample_rate,
            effects.tempo_factor(),
            effects.pitch_semitones(),
            effects.formant_preserved,
        )?;
    }

    // Encode PCM back to WAV
    encode_wav(&samples, sample_rate, channels)
}

/// Apply Signalsmith Stretch to interleaved float PCM.
///
/// Handles tempo (time stretch), pitch shift, and optional formant correction
/// in a single integrated processing pass.
fn apply_stretch(
    samples: &[f32],
    channels: usize,
    sample_rate: u32,
    tempo_factor: f32,
    pitch_semitones: f32,
    preserve_formants: bool,
) -> Result<Vec<f32>, String> {
    if samples.is_empty() || channels == 0 {
        return Ok(samples.to_vec());
    }

    let mut processor = crate::signalsmith::StretchProcessor::new(channels, sample_rate)
        .map_err(|e| format!("Failed to create SignalsmithStretch: {}", e))?;

    processor
        .process(samples, tempo_factor, pitch_semitones, preserve_formants)
        .map_err(|e| format!("SignalsmithStretch processing failed: {}", e))
}

/// Decoded PCM data
struct PcmData {
    samples: Vec<f32>,
    sample_rate: u32,
    channels: usize,
}

/// Decode audio to PCM using Symphonia (auto-detects WAV, MP3, and other formats)
fn decode_audio(audio_data: &[u8]) -> Result<PcmData, String> {
    use std::io::Cursor;

    // Clone the data to own it (required for MediaSourceStream)
    let data = audio_data.to_vec();
    let cursor = Cursor::new(data);
    let mss = MediaSourceStream::new(Box::new(cursor), Default::default());

    let hint = Hint::new();

    let meta_opts: MetadataOptions = Default::default();
    let fmt_opts: FormatOptions = Default::default();

    let probed = symphonia::default::get_probe()
        .format(&hint, mss, &fmt_opts, &meta_opts)
        .map_err(|e| format!("Failed to probe audio: {}", e))?;

    let mut format = probed.format;

    let track = format
        .tracks()
        .iter()
        .find(|t| t.codec_params.codec != CODEC_TYPE_NULL)
        .ok_or("No valid audio track found")?;

    let decoder = symphonia::default::get_codecs()
        .make(&track.codec_params, &DecoderOptions::default())
        .map_err(|e| format!("Failed to create decoder: {}", e))?;

    let sample_rate = track.codec_params.sample_rate.ok_or("No sample rate")? as u32;

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

/// Apply DeepFilterNet noise suppression to interleaved PCM samples.
///
/// The DeepFilterNet model operates at a fixed sample rate (48 kHz for DFN3) and
/// consumes frames of exactly `hop_size` samples per channel. This function:
/// 1. De-interleaves the PCM into a `[channels, frames]` array.
/// 2. Resamples to the model's sample rate (if the input differs).
/// 3. Streams `hop_size` frames through `DfTract::process`.
/// 4. Resamples back to the original sample rate.
/// 5. Re-interleaves the result.
///
/// `atten_db` is the attenuation limit (5..30). Lower values mean gentler cleanup.
fn apply_enhance(
    samples: &[f32],
    sample_rate: u32,
    channels: usize,
    atten_db: f32,
) -> Result<Vec<f32>, String> {
    if samples.is_empty() || channels == 0 {
        return Ok(samples.to_vec());
    }

    // Obtain a fresh model instance (cheap clone of a one-time initialized template)
    // so streaming state never leaks between phrases.
    let mut model = get_df_model(channels)?;
    model.set_atten_lim(atten_db.clamp(5.0, 30.0));

    let model_sr = model.sr;
    let hop_size = model.hop_size;

    // De-interleave into [channels, frames_per_ch].
    let frames_per_ch = samples.len() / channels;
    if frames_per_ch == 0 {
        return Ok(samples.to_vec());
    }
    let mut deinterleaved: Array2<f32> = Array2::zeros((channels, frames_per_ch));
    for (i, &s) in samples.iter().enumerate() {
        let ch = i % channels;
        let f = i / channels;
        if f < frames_per_ch {
            deinterleaved[[ch, f]] = s;
        }
    }

    // Resample to the model's sample rate if needed.
    let needs_resample = sample_rate as usize != model_sr;
    let input = if needs_resample {
        df::transforms::resample(deinterleaved.view(), sample_rate as usize, model_sr, None)
            .map_err(|e| format!("Resample to {} Hz failed: {:?}", model_sr, e))?
    } else {
        deinterleaved
    };

    // Stream hop_size frames through the model.
    let total = input.len_of(Axis(1));
    let mut enhanced: Array2<f32> = Array2::zeros((channels, total));
    for (ns_f, mut enh_f) in input
        .view()
        .axis_chunks_iter(Axis(1), hop_size)
        .zip(enhanced.view_mut().axis_chunks_iter_mut(Axis(1), hop_size))
    {
        if ns_f.len_of(Axis(1)) < hop_size {
            // Partial trailing frame: pass through unprocessed to preserve length.
            enh_f.assign(&ns_f);
            break;
        }
        model
            .process(ns_f, enh_f.view_mut())
            .map_err(|e| format!("DeepFilterNet process failed: {}", e))?;
    }

    // Re-interleave.
    let out_frames = enhanced.len_of(Axis(1));
    let mut result = Vec::with_capacity(out_frames * channels);
    for f in 0..out_frames {
        for ch in 0..channels {
            result.push(enhanced[[ch, f]]);
        }
    }

    Ok(result)
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
    cursor
        .write_all(b"RIFF")
        .map_err(|e| format!("Failed to write RIFF: {}", e))?;
    cursor
        .write_all(&(file_size as u32).to_le_bytes())
        .map_err(|e| format!("Failed to write file size: {}", e))?;
    cursor
        .write_all(b"WAVE")
        .map_err(|e| format!("Failed to write WAVE: {}", e))?;

    // fmt chunk
    cursor
        .write_all(b"fmt ")
        .map_err(|e| format!("Failed to write fmt: {}", e))?;
    cursor
        .write_all(&16u32.to_le_bytes()) // fmt chunk size
        .map_err(|e| format!("Failed to write fmt size: {}", e))?;
    cursor
        .write_all(&1u16.to_le_bytes()) // PCM format
        .map_err(|e| format!("Failed to write format: {}", e))?;
    cursor
        .write_all(&(channels as u16).to_le_bytes())
        .map_err(|e| format!("Failed to write channels: {}", e))?;
    cursor
        .write_all(&sample_rate.to_le_bytes())
        .map_err(|e| format!("Failed to write sample rate: {}", e))?;
    let byte_rate = sample_rate * channels as u32 * 2;
    cursor
        .write_all(&byte_rate.to_le_bytes())
        .map_err(|e| format!("Failed to write byte rate: {}", e))?;
    let block_align = channels as u16 * 2;
    cursor
        .write_all(&block_align.to_le_bytes())
        .map_err(|e| format!("Failed to write block align: {}", e))?;
    cursor
        .write_all(&16u16.to_le_bytes()) // bits per sample
        .map_err(|e| format!("Failed to write bits per sample: {}", e))?;

    // data chunk
    cursor
        .write_all(b"data")
        .map_err(|e| format!("Failed to write data: {}", e))?;
    cursor
        .write_all(&(data_size as u32).to_le_bytes())
        .map_err(|e| format!("Failed to write data size: {}", e))?;

    // Write sample data
    for sample in i16_samples {
        cursor
            .write_all(&sample.to_le_bytes())
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
    fn test_tempo_factor() {
        let effects = AudioEffects::new(0, 0, 100);
        assert_eq!(effects.tempo_factor(), 1.0);

        let effects = AudioEffects::new(0, -100, 100);
        // -100 = slowest: 0.75x tempo
        assert!((effects.tempo_factor() - 0.75).abs() < 0.01);

        let effects = AudioEffects::new(0, 100, 100);
        // +100 = fastest: 1.50x tempo
        assert!((effects.tempo_factor() - 1.50).abs() < 0.01);

        let effects = AudioEffects::new(0, -40, 100);
        // -40 = 1.0 - 0.40*0.25 = 0.90
        assert!((effects.tempo_factor() - 0.90).abs() < 0.01);

        let effects = AudioEffects::new(0, 50, 100);
        // +50 = 1.0 + 0.50*0.50 = 1.25
        assert!((effects.tempo_factor() - 1.25).abs() < 0.01);
    }

    #[test]
    fn test_formant_preserved_default() {
        let effects = AudioEffects::new(0, 0, 100);
        assert!(effects.formant_preserved);
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

    /// Diagnostic test: reproduces DfTract initialization standalone.
    /// Must succeed after the fix is applied.
    #[test]
    fn test_df_tract_initialize_mono() {
        let rp = df::tract::RuntimeParams::default_with_ch(1);
        let df_params = df::tract::DfParams::default();
        let df =
            df::tract::DfTract::new(df_params, &rp).expect("DfTract::new(mono) should succeed");
        assert!(df.sr > 0);
        assert!(df.hop_size > 0);
    }

    /// Integration test: generates a small WAV fixture, processes through
    /// DeepFilterNet enhancement, and verifies finite output.
    #[test]
    fn test_deep_filter_audio_fixture() {
        let sample_rate = 48000u32;
        let channels = 1usize;
        let duration_samples = 480 * 20; // 20 hops × 480 samples = 0.2s
        let freq = 440.0f32;

        // Generate a sine wave mixed with low-level deterministic noise
        let samples: Vec<f32> = (0u32..duration_samples as u32)
            .map(|i| {
                let t = i as f32 / sample_rate as f32;
                let sine = (2.0 * std::f32::consts::PI * freq * t).sin() * 0.5;
                // Simple LCG for deterministic pseudo-random noise
                let noise = (i.wrapping_mul(1664525).wrapping_add(1013904223) as f32
                    / u32::MAX as f32
                    - 0.5)
                    * 0.02;
                (sine + noise).clamp(-1.0, 1.0)
            })
            .collect();

        let wav_data = encode_wav(&samples, sample_rate, channels).expect("encode WAV fixture");

        let fx = AudioEffects::new(0, 0, 100).with_enhance(true, 12.0);

        let result = apply_effects(wav_data, &fx).expect("apply_effects should succeed");

        // Decode result and validate
        let pcm = decode_audio(&result).expect("decode result WAV");

        assert!(!pcm.samples.is_empty(), "output must be non-empty");
        assert_eq!(pcm.channels, channels);
        // DeepFilterNet outputs 48 kHz
        assert_eq!(pcm.sample_rate, 48000);

        assert!(
            pcm.samples.iter().all(|&s| s.is_finite()),
            "all output samples must be finite"
        );
        assert!(
            pcm.samples.iter().any(|&s| s != 0.0),
            "enhanced output must contain non-zero audio"
        );

        eprintln!(
            "enhanced: {} samples, {} channels, {} Hz",
            pcm.samples.len(),
            pcm.channels,
            pcm.sample_rate
        );
    }
}
