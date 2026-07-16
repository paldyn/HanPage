/**
 * 최근 문서 재열기 결과 규칙 (#2285 / PR #2286 리뷰 회귀 고정).
 *
 * 바이트 스냅샷 폴백 없이 저장된 핸들의 라이브 파일로만 연다:
 * - 권한 거부           → 항목 **유지** + 안내 (다음에 다시 시도 가능)
 * - 파일 이동/삭제(read 실패) → 항목 **제거** + 안내 (재열기 영구 불가)
 * - 성공                → open-document-bytes 이벤트 (핸들 연속성 유지)
 *
 * DOM/전역 의존을 주입(deps)으로 분리해 node 테스트에서 규칙을 고정한다.
 */

import type { FileSystemFileHandleLike } from '@/command/file-system-access';
import type { RecentDoc } from './recent-store';

export interface OpenRecentDeps {
  ensurePermission: (handle: FileSystemFileHandleLike) => Promise<boolean>;
  readFile: (handle: FileSystemFileHandleLike) => Promise<{ bytes: Uint8Array; name: string }>;
  remove: (id: string) => Promise<void>;
  toast: (message: string, durationMs: number) => void;
  emitOpen: (payload: {
    bytes: Uint8Array;
    fileName: string;
    fileHandle: FileSystemFileHandleLike;
  }) => void;
}

export type OpenRecentResult = 'opened' | 'permission-denied' | 'removed';

export async function openRecentEntry(
  entry: RecentDoc,
  deps: OpenRecentDeps,
): Promise<OpenRecentResult> {
  let granted = false;
  try {
    granted = await deps.ensurePermission(entry.handle);
  } catch {
    granted = false;
  }
  if (!granted) {
    deps.toast(`"${entry.fileName}" 접근 권한이 거부되어 열 수 없습니다.`, 3000);
    return 'permission-denied';
  }

  try {
    const { bytes, name } = await deps.readFile(entry.handle);
    deps.emitOpen({ bytes, fileName: name, fileHandle: entry.handle });
    return 'opened';
  } catch (err) {
    // 파일 이동/삭제 — 재열기 영구 불가 항목은 목록에서 제거한다.
    console.warn('[file:open-recent] 파일 접근 실패(이동/삭제 추정):', err);
    await deps.remove(entry.id);
    deps.toast(`"${entry.fileName}" 파일을 찾을 수 없어 목록에서 제거했습니다.`, 3500);
    return 'removed';
  }
}
