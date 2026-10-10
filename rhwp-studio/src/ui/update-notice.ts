/**
 * Desktop 전용 업데이트 카드. 백그라운드 다운로드와 수동 재진입은 같은 상태를 표시한다.
 * 백그라운드 준비 완료는 큰 카드를 자동으로 열지 않고, 구석의 작은 알림을 버전마다 한 번 띄운다.
 */
import {
  applyUpdate, checkUpdate, getUpdateStatus, isDesktopRuntime, MENU_CHECK_UPDATE, onUpdateReady, onUpdateStatus,
  type DesktopUpdateReady, type DesktopUpdateStatus,
} from '@/core/desktop-bridge';
import { formatMb, updateProgress, updateReadyMessage, updateStatusMessage } from '@/ui/update-notice-text';
import { t } from '@/i18n/index.ts';
export { formatMb, updateReadyMessage, updateStatusMessage };

let status: DesktopUpdateStatus = { state: 'idle' };
let readyInfo: DesktopUpdateReady | null = null;
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
let nudge: HTMLElement | null = null;
let nudgeTimer: ReturnType<typeof setTimeout> | null = null;
/** 키보드로 알림에 들어오기 직전의 포커스. 알림 안에서 닫을 때만 이곳으로 되돌린다. */
let nudgeFocusOrigin: HTMLElement | null = null;
/**
 * 사용자가 마우스를 실제로 움직여 알림 위에 올려 둔 상태. `:hover` 를 쓰지 않는 이유는
 * 알림이 멈춰 있는 커서 밑에 나타나도 hover 로 판정되어 영영 닫히지 않기 때문이다.
 */
let nudgeHovered = false;
/** 작은 알림을 실제로 보여줬거나 카드에서 확인한 준비 버전. 같은 버전으로는 다시 띄우지 않는다. */
let acknowledgedVersion: string | null = null;
/** 모달이 열려 있거나 창이 뒤에 있어 미뤄 둔 준비 버전과, 모달이 모두 닫히는 것을 지켜보는 관찰자. */
let deferredNudgeVersion: string | null = null;
let modalWatcher: MutationObserver | null = null;
const NUDGE_DURATION_MS = 8000;

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
    <p class="dialog-update-return">나중에 업데이트하셔도 됩니다.
하단 업데이트 버튼에서 다시 열 수 있습니다.</p>
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

function clearNudgeTimer(): void {
  if (nudgeTimer) { clearTimeout(nudgeTimer); nudgeTimer = null; }
}

/** 업데이트 적용 차단막을 제외한 모달 대화상자 중 가장 위(나중에 연) overlay. */
function topModal(): Element | null {
  const overlays = document.querySelectorAll('.modal-overlay:not(.dialog-update-blocker)');
  return overlays[overlays.length - 1] ?? null;
}

function modalOpen(): boolean {
  return topModal() !== null;
}

/**
 * 사용자가 실제로 앱 창을 보고 있는지. 다른 앱으로 전환했거나 최소화한 동안에는 알림을 띄우지 않고
 * 자동 닫힘 시간도 세지 않는다. 버전당 한 번뿐인 알림이 아무도 못 본 채 사라지지 않게 한다.
 */
function windowAttended(): boolean {
  return document.visibilityState === 'visible' && document.hasFocus();
}

/** 마우스를 올려 두었거나 키보드 포커스가 안에 있거나 창을 보고 있지 않으면 자동으로 닫지 않는다. */
function armNudgeTimer(): void {
  clearNudgeTimer();
  if (!windowAttended()) return; // 창으로 돌아오면 onWindowAttention 이 다시 센다.
  nudgeTimer = setTimeout(() => {
    nudgeTimer = null;
    if (nudge && !nudgeHovered && !nudge.contains(document.activeElement) && windowAttended()) hideNudge();
  }, NUDGE_DURATION_MS);
}

/**
 * 알림을 닫는다. 포커스는 알림 안에 있을 때만(키보드로 들어온 경우) 들어오기 직전 위치로 되돌린다.
 * 마우스 클릭은 mousedown 기본 동작을 막아 처음부터 포커스를 옮기지 않으므로 복원이 필요 없다.
 * 모달이 열려 있으면 그 모달 안에서 들어온 경우에만 되돌리고, 모달 뒤의 편집기로는 보내지 않는다.
 */
function hideNudge(): void {
  clearNudgeTimer();
  nudgeHovered = false;
  if (!nudge || nudge.hidden) return;
  const focusInside = nudge.contains(document.activeElement);
  nudge.hidden = true;
  if (focusInside) {
    const origin = nudgeFocusOrigin?.isConnected ? nudgeFocusOrigin : null;
    const modal = topModal();
    if (!modal) (origin ?? entry)?.focus();
    else if (origin && modal.contains(origin)) origin.focus();
  }
  nudgeFocusOrigin = null;
}

function stopWatchingModals(): void {
  modalWatcher?.disconnect();
  modalWatcher = null;
}

/** 미뤄 둔 알림은 모달이 모두 닫히고 창을 보고 있을 때, 그 버전이 아직 준비 상태일 때만 띄운다. */
function resumeDeferredNudge(): void {
  const version = deferredNudgeVersion;
  if (!version || modalOpen() || !windowAttended()) return;
  deferredNudgeVersion = null;
  if (status.state === 'ready' && status.version === version) maybeNudge(version);
}

function watchModalsForDeferredNudge(): void {
  if (modalWatcher) return;
  modalWatcher = new MutationObserver(() => {
    if (modalOpen()) return;
    stopWatchingModals();
    resumeDeferredNudge();
  });
  modalWatcher.observe(document.body, { childList: true, subtree: true });
}

/** 창으로 돌아오면 미뤄 둔 알림을 띄우고, 떠 있던 알림은 그때부터 다시 센다. 창을 떠나면 멈춘다. */
function onWindowAttention(): void {
  if (!windowAttended()) { clearNudgeTimer(); return; }
  resumeDeferredNudge();
  if (nudge && !nudge.hidden && !nudgeTimer && !nudgeHovered && !nudge.contains(document.activeElement)) armNudgeTimer();
}

/** 알림 안의 키는 문서 수준 capture 처리(찾기 대화상자 등)보다 먼저 받는다. */
function handleNudgeKeys(event: KeyboardEvent): void {
  if (!nudge || nudge.hidden || !nudge.contains(event.target as Node | null)) return;
  if (event.key === 'Escape') {
    event.preventDefault();
    event.stopPropagation();
    hideNudge();
  } else if (event.key === 'Enter' || event.key === ' ' || event.key === 'Tab') {
    // 버튼 기본 동작·Tab 이동은 그대로 두고 다른 전역 단축키만 막는다.
    event.stopPropagation();
  }
}

function ensureNudge(): HTMLElement {
  if (nudge) return nudge;
  nudge = document.createElement('div');
  nudge.id = 'desktop-update-nudge';
  nudge.className = 'dialog-update-nudge';
  nudge.hidden = true;
  nudge.setAttribute('role', 'status');
  nudge.setAttribute('aria-live', 'polite');
  // 앱이 소유한 정적 markup. 버전은 textContent로만 넣는다.
  nudge.innerHTML = `
    <p class="dialog-update-nudge-title"></p>
    <p class="dialog-update-nudge-body"></p>
    <div class="dialog-update-nudge-actions"><button type="button" class="dialog-update-nudge-later">나중에</button><button type="button" class="dialog-update-nudge-primary">업데이트</button></div>`;
  // 클릭이 편집기 포커스를 빼앗지 않게 한다(WebKit 은 버튼에 포커스를 주지 않아 body 로 빠진다).
  nudge.addEventListener('mousedown', (event) => event.preventDefault());
  nudge.querySelector('.dialog-update-nudge-later')!.addEventListener('click', hideNudge);
  nudge.querySelector('.dialog-update-nudge-primary')!.addEventListener('click', () => {
    hideNudge();
    showCard(true);
    void primaryAction();
  });
  nudge.addEventListener('mousemove', () => { nudgeHovered = true; clearNudgeTimer(); });
  nudge.addEventListener('mouseleave', () => {
    nudgeHovered = false;
    if (!nudge?.contains(document.activeElement)) armNudgeTimer();
  });
  nudge.addEventListener('focusin', (event) => {
    clearNudgeTimer();
    const from = event.relatedTarget;
    if (from instanceof HTMLElement && !nudge?.contains(from)) nudgeFocusOrigin = from;
  });
  nudge.addEventListener('focusout', (event) => {
    if (!nudge?.contains(event.relatedTarget as Node | null) && !nudgeHovered) armNudgeTimer();
  });
  window.addEventListener('keydown', handleNudgeKeys, true);
  document.body.appendChild(nudge);
  return nudge;
}

/**
 * 백그라운드 준비 완료를 포커스 이동 없이 한 번 알린다. 카드를 보고 있으면 띄우지 않고 확인한 것으로
 * 보며, 모달이 열려 있거나 창이 뒤에 있으면 아무도 못 본 채 사라지지 않도록 그동안 미룬다.
 */
function maybeNudge(version: string): void {
  if (acknowledgedVersion === version || busy()) return;
  if (card && !card.hidden) { acknowledgedVersion = version; return; }
  if (modalOpen() || !windowAttended()) {
    deferredNudgeVersion = version;
    if (modalOpen()) watchModalsForDeferredNudge();
    return;
  }
  acknowledgedVersion = version;
  const el = ensureNudge();
  el.querySelector('.dialog-update-nudge-title')!.textContent = `새 버전 ${version} 준비됨`;
  el.querySelector('.dialog-update-nudge-body')!.textContent = `${isWindows() ? '업데이트하면 설치 프로그램이 열립니다.' : '업데이트하면 앱이 다시 시작됩니다.'}
나중에 하단 버튼이나 파일 메뉴에서도 할 수 있습니다.`;
  if (el.hidden) nudgeHovered = false;
  el.hidden = false;
  armNudgeTimer();
}

function showCard(manual = false): void {
  hideNudge();
  deferredNudgeVersion = null;
  stopWatchingModals();
  if (status.state === 'ready') acknowledgedVersion = status.version;
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
    const label = status.state === 'ready' ? '업데이트 준비됨'
      : status.state === 'downloading' ? `업데이트 받는 중${percent === null ? '' : ` ${percent}%`}`
      : status.state === 'checking' ? '업데이트 확인 중'
      : status.state === 'verifying' ? '업데이트 파일 확인 중'
      : status.state === 'applying' ? '업데이트 적용 중'
      : status.state === 'error' ? (status.retryable ? '다시 업데이트' : '업데이트 실패')
      : '업데이트 확인';
    entry.querySelector('.stb-update-label')!.textContent = label;
    entry.setAttribute('aria-label', `${label} · 상세 안내 열기`);
    entry.title = '업데이트 상세 안내 · 파일 메뉴에서도 다시 열 수 있습니다';
  }
  if (!card) return;
  card.dataset.state = status.state;
  let title = '새 버전을 확인하고 있습니다';
  let description = '확인이 끝나면 결과를 알려드립니다.';
  let detail = '';
  let primary = '업데이트';
  let progress = false;
  let indeterminate = true;
  switch (status.state) {
    case 'downloading':
      title = '업데이트 다운로드 중';
      description = '다운로드 중에도 계속 작업할 수 있습니다.\n준비되면 알려드립니다.';
      detail = status.total && status.total > 0
        ? `${percent}% · ${formatMb(status.downloaded)} / ${formatMb(status.total)}`
        : `${formatMb(status.downloaded)} 다운로드됨`;
      progress = true;
      indeterminate = percent === null;
      break;
    case 'verifying':
      title = '다운로드가 완료되었습니다';
      description = '업데이트 파일을 확인하고 있습니다.';
      detail = '업데이트 파일 확인 중';
      progress = true;
      break;
    case 'ready':
      title = '새 업데이트가 준비되었습니다';
      description = '새 버전 다운로드가 완료되었습니다.\n업데이트하면 앱이 다시 시작됩니다.';
      if (isWindows()) description = '새 버전 다운로드가 완료되었습니다.\n업데이트하면 설치 프로그램이 열립니다.';
      break;
    case 'applying':
      title = '업데이트를 적용하고 있습니다';
      description = isWindows() ? '설치 프로그램을 준비하고 있습니다.\n잠시만 기다려 주세요.' : '완료되면 앱이 자동으로 다시 시작됩니다.';
      detail = isWindows() ? '업데이트 적용 중' : '업데이트 적용 중 · 곧 다시 시작합니다';
      primary = '업데이트 중…';
      progress = true;
      break;
    case 'upToDate':
      title = '최신 버전을 사용하고 있습니다';
      description = `현재 버전: HanPage ${status.version}\n새 업데이트가 없습니다.`;
      primary = '다시 확인';
      break;
    case 'error':
      title = '업데이트를 완료하지 못했습니다';
      description = status.retryable ? '받아둔 파일은 유지됩니다.\n다시 업데이트할 수 있습니다.' : '연결을 확인하고 다시 시도해 주세요.';
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
  (card.querySelector('.dialog-update-return') as HTMLElement).hidden = busy() || status.state === 'upToDate';
  card.querySelector('.dialog-update-later')!.textContent = status.state === 'upToDate' ? '닫기' : '나중에';
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
    // 백그라운드 완료는 큰 카드를 열지 않는다. 상태 표시줄을 갱신하고 작은 알림을 버전마다 한 번 띄운다.
    maybeNudge(next.version);
  } else {
    deferredNudgeVersion = null;
    stopWatchingModals();
    hideNudge();
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
  entry.innerHTML = '<svg aria-hidden="true" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.8"><path d="M12 4v11m-4-4 4 4 4-4M5 16v3a1 1 0 0 0 1 1h12a1 1 0 0 0 1-1v-3"/></svg><span class="stb-update-label"></span>';
  entry.addEventListener('click', () => { void handleManualUpdateCheck(); });
  const statusBar = document.getElementById('status-bar');
  if (statusBar) statusBar.insertBefore(entry, statusBar.querySelector('.stb-right'));
  else document.body.appendChild(entry);
  // 상태 표시줄을 숨겨도 모든 Desktop 플랫폼에서 같은 상세 안내에 접근한다.
  const aboutItem = document.querySelector('.md-item[data-cmd="file:about"]');
  if (aboutItem) {
    const menuEntry = document.createElement('div');
    menuEntry.id = 'desktop-update-menu';
    menuEntry.className = 'md-item';
    menuEntry.dataset.cmd = MENU_CHECK_UPDATE;
    menuEntry.innerHTML = '<span class="md-icon"></span><span class="md-label"></span>';
    menuEntry.querySelector('.md-label')!.textContent = t('command.app.checkUpdate.label');
    aboutItem.before(menuEntry);
  }
  window.addEventListener('focus', onWindowAttention);
  window.addEventListener('blur', onWindowAttention);
  document.addEventListener('visibilitychange', onWindowAttention);
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

/** 파일 메뉴·macOS 앱 메뉴·상태 표시줄이 같은 카드를 연다. */
export async function handleManualUpdateCheck(): Promise<void> {
  if (!isDesktopRuntime()) return;
  const snapshotAt = eventCount;
  const snapshot = await getUpdateStatus();
  if (snapshot && eventCount === snapshotAt && !busy()) receiveStatus(snapshot);
  showCard(true);
  if (status.state === 'idle' || status.state === 'upToDate' || (status.state === 'error' && !status.retryable)) await startCheck();
}
