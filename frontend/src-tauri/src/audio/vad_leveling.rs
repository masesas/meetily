//! Loudness levelling for batch VAD.
//!
//! A recording mixes the microphone with system audio, and the remote speaker is often much
//! quieter. On a 2026-11-16 test the remote speaker's 30 s explanation had RMS 0.040 against
//! 0.090 for the local speaker; Silero VAD (threshold 0.50) skipped it entirely, so the
//! finalized transcript lost it, while 4x gain made VAD find it. VAD therefore runs on a copy whose
//! quiet windows are boosted; transcription keeps using the original samples.

use crate::audio::vad::{get_speech_chunks_with_progress, SpeechSegment};
use anyhow::Result;

const SAMPLE_RATE: usize = 16_000;
/// Window over which loudness is measured and one gain is applied.
const WINDOW_SAMPLES: usize = SAMPLE_RATE;
/// Windows are boosted towards this RMS (the level of the clearly detected speaker in the test).
const TARGET_RMS: f32 = 0.08;
/// Upper bound for the boost, so noise is not raised to speech level.
const MAX_GAIN: f32 = 4.0;
/// Windows quieter than this are treated as silence and left untouched.
const SILENCE_RMS: f32 = 0.005;

fn rms(samples: &[f32]) -> f32 {
    if samples.is_empty() {
        return 0.0;
    }
    (samples.iter().map(|s| s * s).sum::<f32>() / samples.len() as f32).sqrt()
}

/// Copy of `samples` (16 kHz mono) with quiet windows amplified, for VAD input only.
/// Never attenuates; output has the same length.
pub fn level_for_vad(samples: &[f32]) -> Vec<f32> {
    let mut leveled = Vec::with_capacity(samples.len());
    for window in samples.chunks(WINDOW_SAMPLES) {
        let level = rms(window);
        let gain = if level < SILENCE_RMS {
            1.0
        } else {
            (TARGET_RMS / level).clamp(1.0, MAX_GAIN)
        };
        leveled.extend(window.iter().map(|s| (s * gain).clamp(-1.0, 1.0)));
    }
    leveled
}

/// Replaces each segment's samples with the matching slice of the original (unlevelled) audio,
/// using the segment timestamps.
pub fn with_original_samples(segments: Vec<SpeechSegment>, original: &[f32]) -> Vec<SpeechSegment> {
    let to_index = |ms: f64| ((ms / 1000.0 * SAMPLE_RATE as f64) as usize).min(original.len());
    segments
        .into_iter()
        .map(|segment| {
            let start = to_index(segment.start_timestamp_ms);
            let end = to_index(segment.end_timestamp_ms).max(start);
            SpeechSegment {
                samples: original[start..end].to_vec(),
                ..segment
            }
        })
        .collect()
}

/// Batch VAD (import, re-transcription): detects speech on a levelled copy so quiet speakers are
/// found, and returns segments carrying the original samples. Same callback contract as
/// `get_speech_chunks_with_progress`.
pub fn get_speech_chunks_leveled<F>(
    samples_mono_16k: &[f32],
    redemption_time_ms: u32,
    progress_callback: F,
) -> Result<Vec<SpeechSegment>>
where
    F: FnMut(u32, usize) -> bool,
{
    let leveled = level_for_vad(samples_mono_16k);
    let segments = get_speech_chunks_with_progress(&leveled, redemption_time_ms, progress_callback)?;
    Ok(with_original_samples(segments, samples_mono_16k))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tone(amplitude: f32, seconds: usize) -> Vec<f32> {
        (0..seconds * SAMPLE_RATE)
            .map(|i| amplitude * (i as f32 * 0.05).sin())
            .collect()
    }

    #[test]
    fn boosts_quiet_window_towards_target() {
        let quiet = tone(0.04, 1); // RMS ~0.028 -> gain ~2.8
        let leveled = level_for_vad(&quiet);
        assert!((rms(&leveled) - TARGET_RMS).abs() < 0.002, "rms {}", rms(&leveled));
    }

    #[test]
    fn caps_boost_at_max_gain() {
        let very_quiet = tone(0.015, 1); // RMS ~0.011 -> would need ~7.5x
        let leveled = level_for_vad(&very_quiet);
        let ratio = rms(&leveled) / rms(&very_quiet);
        assert!((ratio - MAX_GAIN).abs() < 0.01, "ratio {ratio}");
    }

    #[test]
    fn leaves_loud_window_unchanged() {
        let loud = tone(0.3, 1);
        assert_eq!(level_for_vad(&loud), loud);
    }

    #[test]
    fn leaves_silence_unchanged() {
        let silence = tone(0.001, 1);
        assert_eq!(level_for_vad(&silence), silence);
    }

    #[test]
    fn keeps_length_and_clamps() {
        let mut samples = tone(0.04, 2);
        samples.extend(tone(0.04, 1)[..500].iter());
        samples[10] = 0.9;
        let leveled = level_for_vad(&samples);
        assert_eq!(leveled.len(), samples.len());
        assert!(leveled.iter().all(|s| (-1.0..=1.0).contains(s)));
    }

    #[test]
    fn restores_original_samples_by_timestamp() {
        let original: Vec<f32> = (0..3 * SAMPLE_RATE).map(|i| i as f32).collect();
        let segment = SpeechSegment {
            samples: vec![0.0; 10],
            start_timestamp_ms: 1000.0,
            end_timestamp_ms: 1500.0,
            confidence: 0.9,
        };
        let restored = with_original_samples(vec![segment], &original);
        assert_eq!(restored[0].samples.len(), SAMPLE_RATE / 2);
        assert_eq!(restored[0].samples[0], SAMPLE_RATE as f32);
        assert_eq!(restored[0].start_timestamp_ms, 1000.0);
    }

    #[test]
    fn clamps_timestamps_beyond_audio_end() {
        let original = vec![0.5f32; SAMPLE_RATE];
        let segment = SpeechSegment {
            samples: vec![],
            start_timestamp_ms: 800.0,
            end_timestamp_ms: 5000.0,
            confidence: 0.9,
        };
        let restored = with_original_samples(vec![segment], &original);
        assert_eq!(restored[0].samples.len(), SAMPLE_RATE / 5);
    }
}
