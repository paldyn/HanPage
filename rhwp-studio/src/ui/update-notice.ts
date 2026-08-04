/**
 * [#59] 데스크톱 업데이트 안내 — Claude 데스크톱 앱 방식.
 *
 * 새 버전은 사용자에게 묻지 않고 **조용히 백그라운드로 내려받은 뒤**, 준비가 끝난 시점에
 * 비침습 토스트로 한 번만 알린다. '지금 다시 시작'을 누르면 이미 받아둔 바이트로 즉시
 * 설치되므로 대기 시간이 없다(기존 구현은 승인 후 40MB 를 받느라 수 초간 정지했다).
 *
 * 브라우저(웹 빌드)에서는 데스크톱 브리지가 전부 no-op 이라 이 모듈도 아무 일도 하지 않는다.
 */

import {
  applyUpdate,
  checkUpdate,
  getUpdateStatus,
  onUpdateReady,
  type DesktopUpdateReady,
  type DesktopUpdateStatus,
} from '@/core/desktop-bridge';
import { showToast } from '@/ui/toast';
import {
  formatMb,
  updateReadyMessage,
  updateStatusMessage,
} from '@/ui/update-notice-text';

// 문구 생성은 순수 모듈이 담당한다(단위 테스트 대상). 호출부 편의를 위해 재수출한다.
export { formatMb, updateReadyMessage, updateStatusMessage };

/** 설치 후 동작이 플랫폼마다 다르다 — macOS 는 재시작, Windows 는 설치 프로그램 실행. */
function isWindows(): boolean {
  const nav = navigator as Navigator & { userAgentData?: { platform?: string } };
  const platform = (nav.userAgentData?.platform || navigator.platform || '').toLowerCase();
  return platform.includes('win') || (navigator.userAgent || '').toLowerCase().includes('windows');
}


let readyShown = false;

/** 준비 완료 토스트(중복 표시 방지). */
function showReadyToast(info: DesktopUpdateReady): void {
  if (readyShown) return;
  readyShown = true;
  const windows = isWindows();
  showToast({
    message: updateReadyMessage(info, windows),
    durationMs: 0, // 사용자가 선택할 때까지 유지
    action: {
      label: windows ? '지금 설치' : '지금 다시 시작',
      onClick: () => {
        void applyUpdate().then((r) => {
          // 성공하면 앱이 재시작/종료되므로 여기 도달하지 않는다.
          if (!r.ok) {
            readyShown = false;
            showToast({ message: `업데이트 적용에 실패했습니다.\n${r.message ?? ''}` });
          }
        });
      },
    },
    confirmLabel: '나중에', // 닫아도 받아둔 업데이트는 유지된다.
  });
}

/**
 * 업데이트 안내를 설치한다(데스크톱 전용, `initialize()` 에서 1회 호출).
 *
 * 준비 완료 이벤트를 구독하고, 웹뷰가 늦게 떠서 이벤트를 놓친 경우를 대비해 현재 상태도
 * 한 번 조회한다(펜딩 문서 드레인과 동일한 유실 방지 패턴).
 */
export function installUpdateNotice(): void {
  onUpdateReady(showReadyToast);

  void getUpdateStatus().then((status) => {
    if (status?.state === 'ready') {
      showReadyToast({ version: status.version, currentVersion: '', notes: null });
    }
  });
}

/** 메뉴 "업데이트 확인" — 상태에 따라 즉답한다. */
export async function handleManualUpdateCheck(): Promise<void> {
  const status = await getUpdateStatus();
  if (!status) return;

  if (status.state === 'ready') {
    readyShown = false; // 사용자가 닫았어도 다시 보여준다.
    showReadyToast({ version: status.version, currentVersion: '', notes: null });
    return;
  }

  const message = updateStatusMessage(status);
  if (message) {
    showToast({ message });
    if (status.state === 'upToDate' || status.state === 'error') {
      void checkUpdate(); // 마지막 결과를 보여준 뒤 최신 상태를 다시 확인한다.
    }
    return;
  }

  // idle — 아직 확인한 적이 없다.
  showToast({ message: '새 버전을 확인하고 있습니다…' });
  await checkUpdate();
}
