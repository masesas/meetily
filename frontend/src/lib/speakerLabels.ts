/** Source-based speaker labels: the mic is the user, system audio is everyone on the call. */
export type SpeakerSource = 'mic' | 'system';

export const SPEAKER_LABELS: Record<SpeakerSource, string> = {
  mic: 'Saya',
  system: 'Orang lain',
};

/** Display label for a segment's speaker source, or null when the segment has none. */
export function speakerLabel(speaker: string | null | undefined): string | null {
  return speaker === 'mic' || speaker === 'system' ? SPEAKER_LABELS[speaker] : null;
}

/** "Saya: text" when the segment has a speaker, otherwise the text unchanged. */
export function withSpeakerPrefix(text: string, speaker: string | null | undefined): string {
  const label = speakerLabel(speaker);
  return label ? `${label}: ${text}` : text;
}
