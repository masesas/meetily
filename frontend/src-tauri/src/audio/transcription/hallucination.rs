//! Filter for live Whisper output that is almost certainly not speech.
//!
//! Measured on a 32-minute Indonesian meeting (2026-11-15): Whisper emitted a standalone
//! "Terima kasih." 10 times, always at 22–24 % confidence, on pauses or noise; and 1–1.5 s chunks
//! below ~17 % confidence were fillers or invented words ("Omdat"). Both are dropped from the live
//! view. The post-recording finalization re-transcribes the whole file, so nothing is lost for good.

/// Phrases Whisper produces on silence or noise (subtitle-style endings it was trained on).
const SILENCE_PHRASES: &[&str] = &[
    "terima kasih",
    "terima kasih telah menonton",
    "terima kasih sudah menonton",
    "terima kasih atas perhatiannya",
    "thank you",
    "thanks for watching",
    "thank you for watching",
];

/// A known silence phrase is only dropped below this confidence; a real "terima kasih" is
/// usually transcribed with higher confidence.
const SILENCE_PHRASE_MAX_CONFIDENCE: f32 = 0.35;
/// Chunks shorter than this ...
const SHORT_CHUNK_MAX_SECONDS: f64 = 1.5;
/// ... and below this confidence are dropped.
const SHORT_CHUNK_MAX_CONFIDENCE: f32 = 0.17;

fn normalize(text: &str) -> String {
    text.chars()
        .filter(|c| c.is_alphanumeric() || c.is_whitespace())
        .collect::<String>()
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
        .to_lowercase()
}

/// True when a live transcript chunk should not be shown. `confidence` is `None` for engines
/// that do not report it; then nothing is dropped.
pub fn is_likely_hallucination(text: &str, confidence: Option<f32>, duration_secs: f64) -> bool {
    let Some(confidence) = confidence else {
        return false;
    };
    let normalized = normalize(text);
    if confidence < SILENCE_PHRASE_MAX_CONFIDENCE && SILENCE_PHRASES.contains(&normalized.as_str()) {
        return true;
    }
    duration_secs < SHORT_CHUNK_MAX_SECONDS && confidence < SHORT_CHUNK_MAX_CONFIDENCE
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn drops_low_confidence_silence_phrase() {
        assert!(is_likely_hallucination("Terima kasih.", Some(0.24), 3.2));
        assert!(is_likely_hallucination("  terima   kasih ", Some(0.22), 1.6));
        assert!(is_likely_hallucination("Thanks for watching!", Some(0.30), 2.0));
    }

    #[test]
    fn keeps_confident_silence_phrase() {
        assert!(!is_likely_hallucination("Terima kasih.", Some(0.80), 1.4));
    }

    #[test]
    fn keeps_phrase_inside_a_sentence() {
        assert!(!is_likely_hallucination("Oke, terima kasih Pak Hendy.", Some(0.24), 3.0));
    }

    #[test]
    fn drops_very_short_very_low_confidence_chunk() {
        assert!(is_likely_hallucination("Omdat", Some(0.15), 1.2));
    }

    #[test]
    fn keeps_short_chunk_with_reasonable_confidence() {
        assert!(!is_likely_hallucination("Oke", Some(0.40), 1.2));
    }

    #[test]
    fn keeps_longer_low_confidence_chunk() {
        assert!(!is_likely_hallucination("Di punca Sardar Pak", Some(0.15), 2.5));
    }

    #[test]
    fn keeps_everything_without_confidence() {
        assert!(!is_likely_hallucination("Terima kasih.", None, 1.0));
    }
}
