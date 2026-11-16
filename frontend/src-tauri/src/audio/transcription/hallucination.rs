//! Filter for Whisper output that is almost certainly not speech.
//!
//! Whisper produces subtitle-style phrases on pauses or noise. In a 32-minute Indonesian meeting
//! (2026-11-15) a standalone "Terima kasih." appeared 10 times, each in a pause; and a
//! finalization replaced 10 s of real speech with "Terima kasih telah menonton!". A segment whose
//! whole text is such a phrase is dropped. Only the text is used: the "confidence" Meetily
//! reports is derived from text length, not from the model.

/// Phrases Whisper emits on silence or noise.
const SILENCE_PHRASES: &[&str] = &[
    "terima kasih",
    "thank you",
];
/// Video-outro phrases from Whisper's subtitle training data; practically never said in a meeting.
const OUTRO_PHRASES: &[&str] = &[
    "terima kasih telah menonton",
    "terima kasih sudah menonton",
    "terima kasih telah menyaksikan",
    "thanks for watching",
    "thank you for watching",
];

fn normalize(text: &str) -> String {
    text.chars()
        .filter(|c| c.is_alphanumeric() || c.is_whitespace())
        .collect::<String>()
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
        .to_lowercase()
}

/// True when the whole text is a video-outro phrase.
pub fn is_outro_phrase(text: &str) -> bool {
    OUTRO_PHRASES.contains(&normalize(text).as_str())
}

/// True when a transcript segment should be dropped as a likely hallucination.
pub fn is_likely_hallucination(text: &str) -> bool {
    let normalized = normalize(text);
    SILENCE_PHRASES.contains(&normalized.as_str()) || OUTRO_PHRASES.contains(&normalized.as_str())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn drops_standalone_silence_phrase() {
        assert!(is_likely_hallucination("Terima kasih."));
        assert!(is_likely_hallucination("  terima   kasih "));
        assert!(is_likely_hallucination("Thank you!"));
    }

    #[test]
    fn drops_outro_phrase() {
        // Finalization output on 10 s of real speech, 2026-11-16
        assert!(is_likely_hallucination("Terima kasih telah menonton!"));
        assert!(is_likely_hallucination("Thanks for watching!"));
    }

    #[test]
    fn keeps_phrase_inside_a_sentence() {
        assert!(!is_likely_hallucination("Oke, terima kasih Pak Hendy."));
        assert!(!is_likely_hallucination("Terima kasih atas waktunya, sampai ketemu besok."));
    }

    #[test]
    fn keeps_short_acknowledgements() {
        assert!(!is_likely_hallucination("Oke"));
        assert!(!is_likely_hallucination("Ya."));
    }

    #[test]
    fn recognizes_outro_phrase_by_text_only() {
        assert!(is_outro_phrase(" Terima kasih telah menonton! "));
        assert!(!is_outro_phrase("Terima kasih."));
        assert!(!is_outro_phrase("Oke, terima kasih telah menonton demonya"));
    }
}
