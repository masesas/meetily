//! Per-window loudness of the microphone and of system audio, recorded where the two are mixed.
//!
//! The saved recording is a mono mix, so after the fact it cannot tell who spoke. This log keeps,
//! for every mixing window (50 ms), the RMS of each source. Finalization uses it to label each
//! transcript segment "mic" (the user, "Saya") or "system" (everyone on the call, "Orang lain").
//! Raw RMS values are stored so the classification thresholds can be recalibrated later.

use anyhow::{anyhow, Result};
use serde::{Deserialize, Serialize};
use std::path::Path;
use std::sync::{LazyLock, Mutex};

pub const FILE_NAME: &str = "source_activity.json";
pub const SPEAKER_MIC: &str = "mic";
pub const SPEAKER_SYSTEM: &str = "system";

/// A window counts as speech when the louder source is above this RMS.
const SPEECH_RMS: f32 = 0.005;
/// A speech window belongs to system audio when its RMS is at least this share of the mic RMS.
/// Below it, system sound picked up by the mic (speaker echo) is not mistaken for the remote side.
/// Initial value, to be calibrated from recorded logs.
const SYSTEM_MIN_SHARE_OF_MIC: f32 = 0.5;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct SourceActivity {
    pub version: u32,
    pub sample_rate: u32,
    /// (samples in the window, mic RMS, system RMS), in recording order.
    pub frames: Vec<(u32, f32, f32)>,
}

static CURRENT: LazyLock<Mutex<Option<SourceActivity>>> = LazyLock::new(|| Mutex::new(None));

fn rms(samples: &[f32]) -> f32 {
    if samples.is_empty() {
        return 0.0;
    }
    (samples.iter().map(|s| s * s).sum::<f32>() / samples.len() as f32).sqrt()
}

fn round(v: f32) -> f32 {
    (v * 100_000.0).round() / 100_000.0
}

/// Starts a new log for a recording (replaces any previous one).
pub fn start(sample_rate: u32) {
    if let Ok(mut current) = CURRENT.lock() {
        *current = Some(SourceActivity { version: 1, sample_rate, frames: Vec::new() });
    }
}

/// Records one mixing window. No-op when no log is active.
pub fn record(mic_window: &[f32], sys_window: &[f32]) {
    let samples = mic_window.len().max(sys_window.len()) as u32;
    if let Ok(mut current) = CURRENT.lock() {
        if let Some(log) = current.as_mut() {
            log.frames.push((samples, round(rms(mic_window)), round(rms(sys_window))));
        }
    }
}

/// Ends the active log and writes it to the meeting folder.
pub fn finish_and_save(folder: &Path) -> Result<()> {
    let log = CURRENT
        .lock()
        .map_err(|e| anyhow!("source activity lock poisoned: {}", e))?
        .take()
        .ok_or_else(|| anyhow!("no source activity recorded"))?;
    std::fs::write(folder.join(FILE_NAME), serde_json::to_vec(&log)?)?;
    Ok(())
}

/// Loads the log of a meeting folder, if the recording produced one.
pub fn load(folder: &Path) -> Option<SourceActivity> {
    let bytes = std::fs::read(folder.join(FILE_NAME)).ok()?;
    serde_json::from_slice(&bytes).ok()
}

fn window_source(mic: f32, sys: f32) -> Option<&'static str> {
    if mic.max(sys) < SPEECH_RMS {
        None
    } else if sys >= mic * SYSTEM_MIN_SHARE_OF_MIC {
        Some(SPEAKER_SYSTEM)
    } else {
        Some(SPEAKER_MIC)
    }
}

impl SourceActivity {
    /// Source that dominates the speech windows between `start` and `end` (seconds of the
    /// recording), or `None` when that range holds no speech in the log.
    pub fn speaker_for_range(&self, start: f64, end: f64) -> Option<&'static str> {
        let rate = self.sample_rate as f64;
        let (mut position, mut mic_windows, mut system_windows) = (0.0f64, 0u32, 0u32);
        for &(samples, mic, sys) in &self.frames {
            let window_start = position;
            position += samples as f64 / rate;
            if position <= start || window_start >= end {
                continue;
            }
            match window_source(mic, sys) {
                Some(SPEAKER_MIC) => mic_windows += 1,
                Some(_) => system_windows += 1,
                None => {}
            }
        }
        match (mic_windows, system_windows) {
            (0, 0) => None,
            (m, s) if m > s => Some(SPEAKER_MIC),
            _ => Some(SPEAKER_SYSTEM),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const W: u32 = 2400; // 50 ms at 48 kHz

    fn log(frames: Vec<(f32, f32)>) -> SourceActivity {
        SourceActivity {
            version: 1,
            sample_rate: 48_000,
            frames: frames.into_iter().map(|(m, s)| (W, m, s)).collect(),
        }
    }

    #[test]
    fn classifies_windows() {
        assert_eq!(window_source(0.001, 0.002), None);
        assert_eq!(window_source(0.08, 0.0), Some(SPEAKER_MIC));
        assert_eq!(window_source(0.0, 0.06), Some(SPEAKER_SYSTEM));
        // remote speech with speaker echo in the mic
        assert_eq!(window_source(0.04, 0.05), Some(SPEAKER_SYSTEM));
        // local speech with faint system sound
        assert_eq!(window_source(0.10, 0.01), Some(SPEAKER_MIC));
    }

    #[test]
    fn labels_range_by_dominant_source() {
        // 0.0-0.5 s mic, 0.5-1.0 s system
        let mut frames = vec![(0.08, 0.0); 10];
        frames.extend(vec![(0.01, 0.07); 10]);
        let activity = log(frames);
        assert_eq!(activity.speaker_for_range(0.0, 0.5), Some(SPEAKER_MIC));
        assert_eq!(activity.speaker_for_range(0.5, 1.0), Some(SPEAKER_SYSTEM));
        assert_eq!(activity.speaker_for_range(0.0, 0.8), Some(SPEAKER_MIC));
    }

    #[test]
    fn no_label_for_silence_or_out_of_range() {
        let activity = log(vec![(0.001, 0.001); 10]);
        assert_eq!(activity.speaker_for_range(0.0, 0.5), None);
        assert_eq!(log(vec![(0.08, 0.0); 10]).speaker_for_range(5.0, 6.0), None);
    }

    #[test]
    fn records_and_saves_through_global_log() {
        let dir = std::env::temp_dir().join(format!("source-activity-test-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        start(48_000);
        record(&vec![0.1; W as usize], &vec![0.0; W as usize]);
        record(&vec![0.0; W as usize], &vec![0.2; W as usize]);
        finish_and_save(&dir).unwrap();
        let loaded = load(&dir).unwrap();
        assert_eq!(loaded.frames.len(), 2);
        assert_eq!(loaded.frames[0], (W, 0.1, 0.0));
        assert_eq!(loaded.speaker_for_range(0.05, 0.1), Some(SPEAKER_SYSTEM));
        assert!(finish_and_save(&dir).is_err(), "log is consumed by the first save");
        std::fs::remove_dir_all(&dir).ok();
    }
}
