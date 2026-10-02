/** Desktop 전용 업데이트 카드. 백그라운드 다운로드와 수동 재진입은 같은 상태를 표시한다. */
import {
  applyUpdate, checkUpdate, getUpdateStatus, isDesktopRuntime, onUpdateReady, onUpdateStatus,
  type DesktopUpdateReady, type DesktopUpdateStatus,
} from '@/core/desktop-bridge';
import { formatMb, updateProgress, updateReadyMessage, updateStatusMessage } from '@/ui/update-notice-text';
export { formatMb, updateReadyMessage, updateStatusMessage };

let status: DesktopUpdateStatus = { state: 'idle' };
let readyInfo: DesktopUpdateReady | null = null;
let shownVersion = '';
let installed = false;
let applying = false;
let beforeApply: () => Promise<boolean> = async () => true;
let card: HTMLElement | null = null;
let entry: HTMLButtonElement | null = null;
let eventCount = 0;
let priorFocus: HTMLElement | null = null;
let applyBlocker: HTMLElement | null = null;
let blockedRoot: HTMLElement | null = null;
let rootWasInert = false;

export function isApplyingUpdate(): boolean { return status.state === 'applying'; }

function blockApplyInput(event: Event): void {
  event.preventDefault();
  event.stopImmediatePropagation();
}

/** 저장 확인이 끝난 뒤 적용 중에만 편집을 잠근다. 실패하면 기존 상태를 복원한다. */
function syncApplyBlocker(): void {
  if (!card) return;
  if (isApplyingUpdate() && !applyBlocker) {
    applyBlocker = document.createElement('div');
    applyBlocker.className = 'modal-overlay dialog-update-blocker';
    applyBlocker.setAttribute('aria-hidden', 'true');
    document.body.insertBefore(applyBlocker, card);
    blockedRoot = document.getElementById('studio-root');
    if (blockedRoot) { rootWasInert = blockedRoot.inert; blockedRoot.inert = true; }
    card.setAttribute('aria-modal', 'true');
    card.tabIndex = -1;
    card.focus();
    // 기존 document capture(전체화면 등)보다 먼저 입력을 막는다.
    window.addEventListener('keydown', blockApplyInput, true);
    window.addEventListener('beforeinput', blockApplyInput, true);
  } else if (!isApplyingUpdate() && applyBlocker) {
    applyBlocker.remove();
    applyBlocker = null;
    if (blockedRoot) blockedRoot.inert = rootWasInert;
    blockedRoot = null;
    card.setAttribute('aria-modal', 'false');
    window.removeEventListener('keydown', blockApplyInput, true);
    window.removeEventListener('beforeinput', blockApplyInput, true);
  }
}

function isWindows(): boolean {
  return /win/i.test(navigator.platform || '') || /windows/i.test(navigator.userAgent || '');
}

function busy(): boolean {
  return applying || status.state === 'applying';
}

function hideCard(): void {
  if (busy() || !card) return;
  const restoreFocus = card.contains(document.activeElement);
  card.hidden = true;
  if (restoreFocus) (priorFocus?.isConnected ? priorFocus : entry)?.focus();
}

function ensureCard(): HTMLElement {
  if (card) return card;
  card = document.createElement('section');
  card.id = 'desktop-update-card';
  card.className = 'dialog-update-card';
  card.hidden = true;
  card.setAttribute('role', 'dialog');
  card.setAttribute('aria-modal', 'false');
  card.setAttribute('aria-labelledby', 'desktop-update-title');
  card.setAttribute('aria-describedby', 'desktop-update-description');
  // 모두 앱이 소유한 정적 markup. 버전/오류/notes는 textContent로만 삽입한다.
  card.innerHTML = `
    <div class="dialog-update-header">
      <span class="dialog-update-icon" aria-hidden="true"><svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.7"><path d="M12 16V4m-4 4 4-4 4 4M5 14v5a1 1 0 0 0 1 1h12a1 1 0 0 0 1-1v-5"/></svg></span>
      <span class="dialog-update-brand">HanPage<span>앱 업데이트</span></span>
      <button type="button" class="dialog-update-close" aria-label="업데이트 알림 닫기">×</button>
    </div>
    <h2 id="desktop-update-title"></h2>
    <p id="desktop-update-description"></p>
    <span class="dialog-update-version"></span>
    <div class="dialog-update-progress" role="progressbar" aria-label="업데이트 진행 상태" aria-valuemin="0" aria-valuemax="100"><span></span></div>
    <p class="dialog-update-detail" role="status" aria-live="polite"></p>
    <p class="dialog-update-return">나중에 하셔도 괜찮아요. 상태 표시줄의 ‘업데이트 확인’에서 다시 열 수 있어요.</p>
    <div class="dialog-update-actions"><button type="button" class="dialog-update-later">나중에</button><button type="button" class="dialog-update-primary">업데이트</button></div>`;
  card.querySelector('.dialog-update-close')!.addEventListener('click', hideCard);
  card.querySelector('.dialog-update-later')!.addEventListener('click', hideCard);
  card.querySelector('.dialog-update-primary')!.addEventListener('click', () => { void primaryAction(); });
  card.addEventListener('keydown', (event) => {
    if (event.key === 'Escape') { event.stopPropagation(); hideCard(); }
  });
  document.body.appendChild(card);
  return card;
}

function showCard(manual = false): void {
  const panel = ensureCard();
  if (panel.hidden) priorFocus = document.activeElement instanceof HTMLElement ? document.activeElement : null;
  panel.hidden = false;
  render();
  if (manual) (panel.querySelector('.dialog-update-close') as HTMLButtonElement).focus();
}

function render(): void {
  const percent = updateProgress(status);
  if (entry) {
    entry.dataset.state = status.state;
    entry.textContent = status.state === 'ready' || (status.state === 'error' && status.retryable)
      ? '업데이트 준비됨' : status.state === 'downloading'
        ? `업데이트 받는 중${percent === null ? '' : ` ${percent}%`}`
        : status.state === 'applying' ? '업데이트 적용 중' : '업데이트 확인';
    entry.title = '업데이트 확인 · 알림을 닫았어도 여기에서 다시 열 수 있습니다';
  }
  if (!card) return;
  card.dataset.state = status.state;
  let title = '새 버전을 확인하고 있어요';
  let description = '확인이 끝나면 여기에서 결과를 알려드릴게요.';
  let detail = '';
  let primary = '업데이트';
  let progress = false;
  let indeterminate = true;
  switch (status.state) {
    case 'downloading':
      title = '새 버전을 받고 있어요';
      description = '계속 작업하셔도 괜찮아요. 준비되면 알려드릴게요.';
      detail = status.total && status.total > 0
        ? `${percent}% · ${formatMb(status.downloaded)} / ${formatMb(status.total)}`
        : `${formatMb(status.downloaded)} 다운로드됨`;
      progress = true;
      indeterminate = percent === null;
      break;
    case 'verifying':
      title = '다운로드가 끝났어요';
      description = '업데이트 파일이 안전한지 확인하고 있어요.';
      detail = '업데이트 파일 확인 중';
      progress = true;
      break;
    case 'ready':
      title = '더 좋아진 HanPage가 준비됐어요';
      description = '새 버전 다운로드가 완료됐어요. 업데이트하면 앱이 다시 시작됩니다.';
      if (isWindows()) description = '새 버전 다운로드가 완료됐어요. 업데이트를 누르면 설치 프로그램이 열립니다.';
      break;
    case 'applying':
      title = '업데이트를 적용하고 있어요';
      description = isWindows() ? '설치 프로그램을 준비하고 있어요. 잠시만 기다려 주세요.' : '완료되면 앱이 자동으로 다시 시작됩니다.';
      detail = isWindows() ? '업데이트 적용 중' : '업데이트 적용 중 · 곧 다시 시작합니다';
      primary = '업데이트 중…';
      progress = true;
      break;
    case 'upToDate':
      title = '최신 버전을 사용하고 있어요';
      description = `HanPage ${status.version} · 지금은 새 업데이트가 없어요.`;
      primary = '다시 확인';
      break;
    case 'error':
      title = '업데이트를 완료하지 못했어요';
      description = status.retryable ? '받아둔 파일은 유지돼요. 다시 업데이트할 수 있습니다.' : '연결을 확인하고 다시 시도해 주세요.';
      detail = status.message;
      primary = status.retryable ? '다시 업데이트' : '다시 확인';
      break;
    case 'idle':
    case 'checking':
      detail = '업데이트 확인 중';
      progress = true;
      break;
  }
  card.querySelector('h2')!.textContent = title;
  card.querySelector('#desktop-update-description')!.textContent = description;
  const version = card.querySelector('.dialog-update-version') as HTMLElement;
  const versionText = status.state === 'ready' || status.state === 'applying' ? status.version : readyInfo?.version;
  version.hidden = !versionText || (status.state !== 'ready' && status.state !== 'applying');
  version.textContent = readyInfo?.currentVersion ? `v${readyInfo.currentVersion} → v${versionText}` : `v${versionText ?? ''}`;
  const bar = card.querySelector('.dialog-update-progress') as HTMLElement;
  bar.hidden = !progress;
  bar.classList.toggle('dialog-update-indeterminate', indeterminate);
  bar.setAttribute('aria-valuetext', detail);
  if (progress && !indeterminate && percent !== null) bar.setAttribute('aria-valuenow', String(percent));
  else bar.removeAttribute('aria-valuenow');
  (bar.firstElementChild as HTMLElement).style.width = indeterminate ? '' : `${percent}%`;
  card.querySelector('.dialog-update-detail')!.textContent = detail;
  (card.querySelector('.dialog-update-return') as HTMLElement).hidden = busy();
  const action = card.querySelector('.dialog-update-primary') as HTMLButtonElement;
  action.textContent = primary;
  action.disabled = busy() || ['idle', 'checking', 'downloading', 'verifying'].includes(status.state);
  for (const button of card.querySelectorAll<HTMLButtonElement>('.dialog-update-close, .dialog-update-later')) button.disabled = busy();
  card.setAttribute('aria-busy', String(busy()));
  syncApplyBlocker();
}

function receiveStatus(next: DesktopUpdateStatus): void {
  eventCount++;
  // 파일 적용이 시작된 뒤 늦게 도착한 준비/다운로드 이벤트로 화면을 되돌리지 않는다.
  if (status.state === 'applying' && next.state !== 'applying' && next.state !== 'error') return;
  status = next;
  if (next.state === 'ready') {
    if (readyInfo?.version !== next.version) readyInfo = { version: next.version, currentVersion: '', notes: null };
    // 한 버전은 자동으로 한 번만 안내한다. '나중에' 후에는 상시 버튼/메뉴로 연다.
    if (shownVersion !== next.version) {
      shownVersion = next.version;
      showCard();
    }
  }
  render();
}

async function startCheck(): Promise<void> {
  receiveStatus({ state: 'checking' });
  const result = await checkUpdate();
  if (!result.ok) receiveStatus({ state: 'error', message: result.message ?? '업데이트를 확인하지 못했습니다.' });
}

async function primaryAction(): Promise<void> {
  if (busy()) return;
  if (status.state !== 'ready' && !(status.state === 'error' && status.retryable)) {
    if (status.state === 'upToDate' || status.state === 'error') await startCheck();
    return;
  }
  applying = true; // 저장 확인 중의 연속 클릭도 한 번으로 묶는다.
  render();
  try {
    card?.classList.add('dialog-update-awaiting-save');
    let approved: boolean;
    try { approved = await beforeApply(); }
    finally { card?.classList.remove('dialog-update-awaiting-save'); }
    if (!approved) return;
    const version = status.state === 'ready' ? status.version : readyInfo?.version ?? '';
    receiveStatus({ state: 'applying', version });
    // IPC 시작 전에 상태가 실제로 paint되도록 다음 프레임까지 기다린다.
    await new Promise<void>(resolve => requestAnimationFrame(() => requestAnimationFrame(() => resolve())));
    const result = await applyUpdate();
    if (!result.ok) receiveStatus({ state: 'error', message: result.message ?? '업데이트 적용에 실패했습니다.', retryable: true });
  } catch (error) {
    receiveStatus({ state: 'error', message: String(error), retryable: true });
  } finally {
    applying = false;
    render();
    if (!isApplyingUpdate() && card && !card.hidden) (card.querySelector('.dialog-update-primary') as HTMLButtonElement).focus();
  }
}

/** Desktop만 상시 진입점을 만들고, 이벤트 등록 완료 후 현재 상태를 복구한다. */
export function installUpdateNotice(options?: { beforeApply?: () => Promise<boolean> }): void {
  if (!isDesktopRuntime() || installed) return;
  installed = true;
  beforeApply = options?.beforeApply ?? beforeApply;
  entry = document.createElement('button');
  entry.id = 'desktop-update-entry';
  entry.className = 'stb-update-button';
  entry.type = 'button';
  entry.addEventListener('click', () => { void handleManualUpdateCheck(); });
  const statusBar = document.getElementById('status-bar');
  if (statusBar) statusBar.insertBefore(entry, statusBar.querySelector('.stb-right'));
  else document.body.appendChild(entry);
  render();
  void (async () => {
    try {
      await onUpdateReady(info => { readyInfo = info; receiveStatus({ state: 'ready', version: info.version }); });
      await onUpdateStatus(receiveStatus);
      const snapshotAt = eventCount;
      const snapshot = await getUpdateStatus();
      if (snapshot && eventCount === snapshotAt) receiveStatus(snapshot);
    } catch (error) {
      console.warn('[update-notice] 진행 알림 등록 실패:', error);
    }
  })();
}

/** macOS 메뉴와 모든 Desktop 플랫폼의 상태 표시줄이 같은 카드를 연다. */
export async function handleManualUpdateCheck(): Promise<void> {
  if (!isDesktopRuntime()) return;
  const snapshotAt = eventCount;
  const snapshot = await getUpdateStatus();
  if (snapshot && eventCount === snapshotAt && !busy()) receiveStatus(snapshot);
  showCard(true);
  if (status.state === 'idle' || status.state === 'upToDate' || (status.state === 'error' && !status.retryable)) await startCheck();
}
