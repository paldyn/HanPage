/**
 * [#59] 업데이트 안내 문구 생성 — 순수 함수 모듈.
 *
 * DOM·브리지에 의존하지 않아 단위 테스트가 가능하다(값 import 가 없으므로 `@/` 별칭
 * 해석이 필요 없다 — 타입 import 는 런타임에 소거된다).
 */

import type { DesktopUpdateReady, DesktopUpdateStatus } from '@/core/desktop-bridge';

/** MB 표기(소수 1자리). 진행 안내용. */
export function formatMb(bytes: number): string {
  return `${(bytes / (1024 * 1024)).toFixed(1)} MB`;
}

/**
 * 상태별 안내 문구(수동 확인용). `null` 이면 이 경로에서 안내할 것이 없다는 뜻이다
 * (`ready` 는 전용 토스트, `idle` 은 확인을 새로 시작).
 */
export function updateStatusMessage(status: DesktopUpdateStatus): string | null {
  switch (status.state) {
    case 'downloading':
      return status.total
        ? `새 버전을 내려받고 있습니다… (${Math.floor((status.downloaded / status.total) * 100)}%)\n준비되면 알려드릴게요.`
        : `새 버전을 내려받고 있습니다… ${formatMb(status.downloaded)}\n준비되면 알려드릴게요.`;
    case 'checking':
      return '새 버전을 확인하고 있습니다…';
    case 'upToDate':
      return `현재 최신 버전입니다. (${status.version})`;
    case 'error':
      return `업데이트 확인에 실패했습니다.\n${status.message}`;
    case 'ready':
    case 'idle':
      return null;
  }
}

/** 준비 완료 토스트 본문. `currentVersion` 이 비면 괄호를 생략한다. */
export function updateReadyMessage(info: DesktopUpdateReady, windows: boolean): string {
  const current = info.currentVersion ? ` (현재 ${info.currentVersion})` : '';
  const head = `새 버전 ${info.version} 이(가) 준비되었습니다.${current}`;
  const tail = windows
    ? '적용하면 설치 프로그램이 실행됩니다.'
    : '다시 시작하면 바로 적용됩니다.';
  return `${head}\n${tail}`;
}
