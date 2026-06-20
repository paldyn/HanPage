import type { AutosaveDraft } from './autosave-store.ts';

export function recoveryFileName(fileName: string): string {
  const trimmed = fileName.trim() || '문서.hwp';
  const dot = trimmed.lastIndexOf('.');
  if (dot > 0) {
    return `${trimmed.slice(0, dot)} 복구본${trimmed.slice(dot)}`;
  }
  return `${trimmed} 복구본.hwp`;
}

export function formatDraftSavedAt(timestamp: number): string {
  if (!Number.isFinite(timestamp) || timestamp <= 0) return '저장 시각 알 수 없음';
  return new Date(timestamp).toLocaleString('ko-KR');
}

export function formatDraftSize(byteLength: number): string {
  if (!Number.isFinite(byteLength) || byteLength < 0) return '크기 알 수 없음';
  if (byteLength < 1024) return `${byteLength} B`;
  const kb = byteLength / 1024;
  if (kb < 1024) return `${kb.toFixed(1)} KB`;
  return `${(kb / 1024).toFixed(1)} MB`;
}

export function describeDraft(draft: AutosaveDraft): string {
  return `${formatDraftSavedAt(draft.savedAt)} · ${formatDraftSize(draft.byteLength)} · ${draft.sourceFormat.toUpperCase()}`;
}
