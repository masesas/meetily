//! Keeps earlier transcript text for time ranges a re-transcription did not cover.
//!
//! Re-transcription replaces a meeting's transcript. When its VAD misses speech that the earlier
//! (live) transcript did capture, that text would be lost. On a 2026-11-16 test ~30 s of a quiet
//! remote speaker vanished this way. Segments of the earlier transcript that the new one leaves
//! mostly uncovered are therefore kept.

use crate::api::TranscriptSegment;

/// An earlier segment is kept when less than this share of its duration overlaps new segments.
const MAX_COVERED_SHARE: f64 = 0.3;

fn span(segment: &TranscriptSegment) -> Option<(f64, f64)> {
    match (segment.audio_start_time, segment.audio_end_time) {
        (Some(start), Some(end)) if end > start => Some((start, end)),
        _ => None,
    }
}

fn covered_share(range: (f64, f64), new_segments: &[TranscriptSegment]) -> f64 {
    let (start, end) = range;
    let overlap: f64 = new_segments
        .iter()
        .filter_map(span)
        .map(|(s, e)| (e.min(end) - s.max(start)).max(0.0))
        .sum();
    overlap / (end - start)
}

/// New segments plus earlier segments whose time range the new ones leave uncovered, ordered by
/// start time. Earlier segments without timestamps or text are dropped.
pub fn keep_uncovered_previous(
    new_segments: Vec<TranscriptSegment>,
    previous: Vec<TranscriptSegment>,
) -> Vec<TranscriptSegment> {
    let kept: Vec<TranscriptSegment> = previous
        .into_iter()
        .filter(|p| !p.text.trim().is_empty())
        .filter(|p| span(p).is_some_and(|range| covered_share(range, &new_segments) < MAX_COVERED_SHARE))
        .collect();

    let mut merged = new_segments;
    merged.extend(kept);
    merged.sort_by(|a, b| {
        let a = a.audio_start_time.unwrap_or(f64::MAX);
        let b = b.audio_start_time.unwrap_or(f64::MAX);
        a.total_cmp(&b)
    });
    merged
}

#[cfg(test)]
mod tests {
    use super::*;

    fn seg(text: &str, start: f64, end: f64) -> TranscriptSegment {
        TranscriptSegment {
            id: format!("{text}-{start}"),
            text: text.to_string(),
            timestamp: String::new(),
            audio_start_time: Some(start),
            audio_end_time: Some(end),
            duration: Some(end - start),
        }
    }

    fn texts(segments: &[TranscriptSegment]) -> Vec<&str> {
        segments.iter().map(|s| s.text.as_str()).collect()
    }

    #[test]
    fn keeps_previous_segment_in_a_gap() {
        let new = vec![seg("final A", 0.0, 40.0), seg("final B", 149.0, 151.0)];
        let previous = vec![seg("live A", 0.0, 39.0), seg("live gap", 120.0, 140.0)];
        let merged = keep_uncovered_previous(new, previous);
        assert_eq!(texts(&merged), vec!["final A", "live gap", "final B"]);
    }

    #[test]
    fn drops_previous_segment_mostly_covered() {
        let new = vec![seg("final", 10.0, 20.0)];
        let previous = vec![seg("live", 9.0, 19.0)];
        assert_eq!(texts(&keep_uncovered_previous(new, previous)), vec!["final"]);
    }

    #[test]
    fn keeps_previous_segment_barely_touched() {
        let new = vec![seg("final", 0.0, 11.0)];
        let previous = vec![seg("live", 10.0, 20.0)]; // 10% covered
        assert_eq!(texts(&keep_uncovered_previous(new, previous)), vec!["final", "live"]);
    }

    #[test]
    fn sums_overlap_of_several_new_segments() {
        let new = vec![seg("a", 10.0, 13.0), seg("b", 17.0, 20.0)];
        let previous = vec![seg("live", 10.0, 20.0)]; // 60% covered
        assert_eq!(texts(&keep_uncovered_previous(new, previous)), vec!["a", "b"]);
    }

    #[test]
    fn ignores_previous_without_timestamps_or_text() {
        let mut no_time = seg("no time", 0.0, 1.0);
        no_time.audio_start_time = None;
        let blank = seg("   ", 50.0, 55.0);
        let merged = keep_uncovered_previous(vec![seg("final", 0.0, 5.0)], vec![no_time, blank]);
        assert_eq!(texts(&merged), vec!["final"]);
    }

    #[test]
    fn empty_new_transcript_keeps_all_previous() {
        let previous = vec![seg("one", 0.0, 2.0), seg("two", 3.0, 4.0)];
        assert_eq!(texts(&keep_uncovered_previous(vec![], previous)), vec!["one", "two"]);
    }
}
