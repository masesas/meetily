import { useEffect, useState } from 'react';
import { Switch } from './ui/switch';
import { useConfig } from '@/contexts/ConfigContext';
import { FINALIZE_WHISPER_MODEL } from '@/lib/transcriptFinalization';

const VOCABULARY_PLACEHOLDER = 'login, purchase order, PO, nama PT, nama rekan kerja, istilah internal';

/** Vocabulary and post-recording finalization settings for Indonesian transcription accuracy. */
export function TranscriptionAccuracySettings() {
  const {
    transcriptionVocabulary,
    setTranscriptionVocabulary,
    isAutoFinalize,
    toggleIsAutoFinalize,
  } = useConfig();
  const [draft, setDraft] = useState(transcriptionVocabulary);

  useEffect(() => setDraft(transcriptionVocabulary), [transcriptionVocabulary]);

  const saveDraft = () => {
    if (draft !== transcriptionVocabulary) {
      setTranscriptionVocabulary(draft);
    }
  };

  return (
    <div className="flex flex-col gap-4 mt-4">
      <div className="bg-white rounded-lg border border-gray-200 p-6 shadow-sm">
        <h3 className="text-lg font-semibold text-gray-900 mb-2">Kamus istilah</h3>
        <p className="text-sm text-gray-600 mb-3">
          Nama orang, nama PT, dan istilah yang sering salah dengar. Pisahkan dengan koma atau baris
          baru (sekitar 100 istilah pertama). Dipakai saat finalisasi dan import, tidak saat live:
          setiap segmen ditranskrip dengan dan tanpa kamus, dan hasil dengan kamus hanya dipakai bila
          isinya tetap sama (kamus boleh membetulkan ejaan, tidak boleh mengubah kalimat).
        </p>
        <textarea
          value={draft}
          onChange={e => setDraft(e.target.value)}
          onBlur={saveDraft}
          placeholder={VOCABULARY_PLACEHOLDER}
          rows={5}
          className="w-full rounded-md border border-gray-300 p-3 text-sm focus:outline-none focus:ring-2 focus:ring-blue-500"
        />
        <div className="mt-2 flex justify-end">
          <button
            onClick={saveDraft}
            disabled={draft === transcriptionVocabulary}
            className="px-4 py-2 text-sm font-medium text-white bg-blue-600 rounded-md hover:bg-blue-700 disabled:opacity-50"
          >
            Simpan kamus
          </button>
        </div>
      </div>

      <div className="bg-white rounded-lg border border-gray-200 p-6 shadow-sm">
        <div className="flex items-center justify-between gap-4">
          <div>
            <h3 className="text-lg font-semibold text-gray-900 mb-2">Finalisasi transkrip otomatis</h3>
            <p className="text-sm text-gray-600">
              Setelah rekaman disimpan, transkrip ditranskrip ulang dengan model akurat
              ({FINALIZE_WHISPER_MODEL}) dan menggantikan hasil live. Model ini harus sudah diunduh.
            </p>
          </div>
          <Switch checked={isAutoFinalize} onCheckedChange={toggleIsAutoFinalize} />
        </div>
      </div>
    </div>
  );
}
