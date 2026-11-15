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

    #[test]
    fn caps_prompt_length() {
        let raw: Vec<String> = (0..90).map(|i| format!("istilah-panjang-nomor-{i:03}")).collect();
        let prompt = build_initial_prompt(&raw.join(",")).unwrap();
        assert!(prompt.len() <= MAX_PROMPT_CHARS);
        assert!(prompt.ends_with('.'));
    }
}
