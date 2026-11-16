import { invoke } from '@tauri-apps/api/core';

/**
 * Model used to re-transcribe a recording after it is saved. The live model favours speed;
 * this one favours accuracy (best result on our Indonesian meeting test, 2026-11-15).
 */
export const FINALIZE_WHISPER_MODEL = 'large-v3-q5_0';

/** Meetings whose finalization was started automatically (not from the Enhance dialog). */
const autoFinalizing = new Set<string>();

/** True once for a meeting that was being finalized automatically; clears the mark. */
export function consumeAutoFinalization(meetingId: string): boolean {
  return autoFinalizing.delete(meetingId);
}

/**
 * Starts re-transcription of a saved meeting with the accurate model. Runs in the background
 * in Rust; completion is reported through the `retranscription-complete` / `retranscription-error`
 * events. Resolves to false when it could not be started (e.g. another one is still running).
 */
export async function startTranscriptFinalization(
  meetingId: string,
  folderPath: string,
  language: string,
): Promise<boolean> {
  try {
    await invoke('start_retranscription_command', {
      meetingId,
      meetingFolderPath: folderPath,
      language: language === 'auto' || language === 'auto-translate' ? null : language,
      model: FINALIZE_WHISPER_MODEL,
      provider: 'whisper',
    });
    autoFinalizing.add(meetingId);
    return true;
  } catch (error) {
    console.warn('Transcript finalization not started:', error);
    return false;
  }
}
