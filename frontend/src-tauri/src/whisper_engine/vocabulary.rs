//! Custom vocabulary (names, company names, domain terms) passed to Whisper as the initial prompt.
//!
//! Whisper reads the initial prompt as preceding text, so listed terms are spelled the way the
//! user wrote them ("login", "purchase order") instead of being misheard. Whisper keeps only about
//! 224 prompt tokens, so the list is capped.

use std::collections::HashSet;
use std::sync::{LazyLock, Mutex};

/// Maximum number of terms kept from the user's list.
const MAX_TERMS: usize = 100;
/// Maximum prompt length in characters; roughly what fits in Whisper's prompt window.
const MAX_PROMPT_CHARS: usize = 800;
const PROMPT_PREFIX: &str = "Istilah: ";

static VOCABULARY: LazyLock<Mutex<String>> = LazyLock::new(|| Mutex::new(String::new()));

/// Builds the Whisper initial prompt from a raw, user-edited term list.
///
/// Terms are separated by commas, semicolons or new lines. Empty entries, NUL bytes and
/// case-insensitive duplicates are dropped; the first spelling wins. Returns `None` when no
/// term remains.
pub fn build_initial_prompt(raw: &str) -> Option<String> {
    let mut seen = HashSet::new();
    let mut prompt = String::from(PROMPT_PREFIX);
    let mut count = 0;

    for term in raw.split([',', ';', '\n']) {
        let term = term.replace('\0', "");
        let term = term.trim();
        if term.is_empty() || !seen.insert(term.to_lowercase()) {
            continue;
        }
        let separator = if count == 0 { "" } else { ", " };
        if count == MAX_TERMS || prompt.len() + separator.len() + term.len() + 1 > MAX_PROMPT_CHARS {
            break;
        }
        prompt.push_str(separator);
        prompt.push_str(term);
        count += 1;
    }

    if count == 0 {
        return None;
    }
    prompt.push('.');
    Some(prompt)
}

/// Minimum share of words the prompted and unprompted texts must have in common.
const MIN_SHARED_WORDS: f64 = 0.6;
/// Maximum ratio between the word counts of the two texts.
const MAX_LENGTH_RATIO: f64 = 1.5;

fn words(text: &str) -> Vec<String> {
    text.split_whitespace()
        .map(|w| w.chars().filter(|c| c.is_alphanumeric()).collect::<String>().to_lowercase())
        .filter(|w| !w.is_empty())
        .collect()
}

/// True when the text transcribed with the vocabulary prompt is a correction of the text
/// transcribed without it (mostly the same words, similar length) rather than a rewrite.
/// A prompt can make Whisper invent a different sentence (2026-11-16: 10 s of speech became
/// "Pertanyaan keempat." with any vocabulary prompt).
pub fn prompted_text_is_consistent(unprompted: &str, prompted: &str) -> bool {
    let base = words(unprompted);
    let with = words(prompted);
    if base.is_empty() || with.is_empty() {
        return base.is_empty() && with.is_empty();
    }
    let (shorter, longer) = if base.len() <= with.len() { (&base, &with) } else { (&with, &base) };
    if longer.len() as f64 / shorter.len() as f64 > MAX_LENGTH_RATIO {
        return false;
    }
    let base_set: HashSet<&String> = base.iter().collect();
    let with_set: HashSet<&String> = with.iter().collect();
    let shared = base_set.intersection(&with_set).count();
    shared as f64 / base_set.len().max(with_set.len()) as f64 >= MIN_SHARED_WORDS
}

/// Initial prompt for the vocabulary currently set by the frontend, if any.
pub fn current_initial_prompt() -> Option<String> {
    let raw = VOCABULARY.lock().ok()?.clone();
    build_initial_prompt(&raw)
}

/// Stores the user's vocabulary list (synced from the frontend on startup and on every change).
#[tauri::command]
pub async fn set_vocabulary_preference(vocabulary: String) -> Result<(), String> {
    let mut stored = VOCABULARY
        .lock()
        .map_err(|e| format!("Failed to set vocabulary: {}", e))?;
    let terms = build_initial_prompt(&vocabulary).map_or(0, |p| p.matches(", ").count() + 1);
    log::info!("Setting transcription vocabulary: {} terms", terms);
    *stored = vocabulary;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty_or_blank_input_gives_no_prompt() {
        assert_eq!(build_initial_prompt(""), None);
        assert_eq!(build_initial_prompt(" ,\n ; "), None);
    }

    #[test]
    fn splits_on_commas_semicolons_and_new_lines() {
        assert_eq!(
            build_initial_prompt("login, purchase order;PT\n super admin").as_deref(),
            Some("Istilah: login, purchase order, PT, super admin.")
        );
    }

    #[test]
    fn drops_case_insensitive_duplicates_keeping_first_spelling() {
        assert_eq!(
            build_initial_prompt("Login, login, LOGIN, PO").as_deref(),
            Some("Istilah: Login, PO.")
        );
    }

    #[test]
    fn removes_nul_bytes() {
        let prompt = build_initial_prompt("log\0in").unwrap();
        assert_eq!(prompt, "Istilah: login.");
        assert!(!prompt.contains('\0'));
    }

    #[test]
    fn caps_number_of_terms() {
        let raw: Vec<String> = (0..150).map(|i| format!("t{i}")).collect();
        let prompt = build_initial_prompt(&raw.join(",")).unwrap();
        assert_eq!(prompt.matches(", ").count() + 1, MAX_TERMS);
    }

    const SPOKEN: &str = "atau server lokal dengan Piper dan Voice Indonesia. Mau kita mulai tes kecil dulu untuk gaya bicara Indonesia-nya?";

    #[test]
    fn accepts_spelling_correction() {
        assert!(prompted_text_is_consistent(
            "Nah sekarang saya coba lecinkan",
            "Nah, sekarang saya coba login."
        ));
        assert!(prompted_text_is_consistent(SPOKEN, SPOKEN));
    }

    #[test]
    fn rejects_rewrites_seen_with_vocabulary_prompts() {
        assert!(!prompted_text_is_consistent(SPOKEN, "Pertanyaan keempat."));
        assert!(!prompted_text_is_consistent(
            SPOKEN,
            "Pertama, langkah kedua untuk mengoperasikan aplikasi, saya akan menyebut aplikasi ini seperti aplikasi-aplikasi yang ada di internet."
        ));
        assert!(!prompted_text_is_consistent(
            SPOKEN,
            "Pipers are available on other services or local servers with Piper and Voice Indonesia."
        ));
    }

    #[test]
    fn rejects_text_appearing_only_with_prompt() {
        assert!(!prompted_text_is_consistent("", "login, logout"));
        assert!(!prompted_text_is_consistent("Oke siap", ""));
        assert!(prompted_text_is_consistent("", "  "));
    }

    #[test]
    fn caps_prompt_length() {
        let raw: Vec<String> = (0..90).map(|i| format!("istilah-panjang-nomor-{i:03}")).collect();
        let prompt = build_initial_prompt(&raw.join(",")).unwrap();
        assert!(prompt.len() <= MAX_PROMPT_CHARS);
        assert!(prompt.ends_with('.'));
    }
}
