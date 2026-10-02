/**
 * Desktop 업데이트 실제 DOM 회귀. Tauri IPC만 대역 처리하며 실제 설치·재시작은 하지 않는다.
 *
 * node e2e/run-with-vite.mjs -- node e2e/desktop-update-ui.test.mjs --mode=headless
 * --baseline=<Git ref>를 추가하면 이전 업데이트 모듈만 실제 제품 entry에 주입해 비교한다.
 */
import { execFileSync } from 'node:child_process';
import { mkdirSync, writeFileSync, readFileSync } from 'node:fs';
import { createHash } from 'node:crypto';
import path from 'node:path';
import { transformWithOxc } from 'vite';
import {
  runTest, loadApp, createPage, createNewDocument, typeText, assert, setTestCase,
} from './helpers.mjs';

const outputDir = path.resolve(process.env.DESKTOP_UPDATE_OUTPUT || '../output/desktop-update-ui/e2e');
const baselineRef = process.argv.find(arg => arg.startsWith('--baseline='))?.slice('--baseline='.length);
const results = [];
const screenshots = [];
const check = (condition, message) => {
  results.push({ pass: Boolean(condition), message });
  assert(condition, message);
};
const visibleCard = '#desktop-update-card:not([hidden])';
const primary = `${visibleCard} .dialog-update-primary`;
const mib = 1024 * 1024;

/** 네이티브가 소유하는 상태·이벤트·IPC만 교체한다. Studio/main/문서/저장 경로는 실제 제품이다. */
async function installNativeMock(page, { platform = 'MacIntel', state = { state: 'ready', version: '0.8.8' }, staleSnapshot = false } = {}) {
  await page.evaluateOnNewDocument((config) => {
    localStorage.setItem('rhwp-settings', JSON.stringify({ version: 1, theme: { mode: 'light' } }));
    Object.defineProperty(navigator, 'platform', { configurable: true, value: config.platform });
    Object.defineProperty(navigator, 'userAgentData', { configurable: true, value: { platform: config.platform.includes('Win') ? 'Windows' : 'macOS' } });
    const listeners = new Map();
    const calls = [];
    const pendingApply = [];
    const mock = {
      state: config.state,
      saveResult: { status: 'cancelled' },
      checkError: '',
      snapshotRace: config.staleSnapshot,
      calls,
      pendingApply,
      pendingDocuments: [],
      emit(event, payload) {
        for (const handler of listeners.get(event) || []) handler({ payload });
      },
      setState(next) { this.state = next; this.emit('hanpage://update-status', next); },
      ready(version = '0.8.8') {
        this.state = { state: 'ready', version };
        this.emit('hanpage://update-ready', { version, currentVersion: '0.8.7', notes: null });
        this.emit('hanpage://update-status', this.state);
      },
      applyFailure(message = '테스트 적용 실패') {
        const request = pendingApply.shift();
        if (!request) throw new Error('대기 중인 적용 IPC가 없습니다.');
        this.setState({ state: 'error', message, retryable: true });
        request.reject(new Error(message));
      },
      count(command) { return calls.filter(call => call.command === command).length; },
    };
    window.__updateMock = mock;
    window.__TAURI_INTERNALS__ = {};
    window.__TAURI__ = {
      core: { invoke(command, args) {
        calls.push({ command, args });
        if (command === 'cmd_take_pending_documents') return Promise.resolve(mock.pendingDocuments.splice(0));
        if (command === 'cmd_update_status') {
          if (mock.snapshotRace) {
            mock.snapshotRace = false;
            const older = mock.state;
            return new Promise(resolve => queueMicrotask(() => {
              mock.setState({ state: 'downloading', downloaded: 20 * 1024 * 1024, total: 100 * 1024 * 1024 });
              resolve(older);
            }));
          }
          return Promise.resolve(mock.state);
        }
        if (command === 'cmd_update_check') {
          if (mock.checkError) return Promise.reject(new Error(mock.checkError));
          mock.setState({ state: 'checking' });
          return Promise.resolve();
        }
        if (command === 'cmd_update_apply') return new Promise((resolve, reject) => pendingApply.push({ resolve, reject }));
        if (command === 'cmd_save_document') {
          if (mock.saveResult.error) return Promise.reject(new Error(mock.saveResult.error));
          return Promise.resolve(mock.saveResult);
        }
        throw new Error(`대역에서 허용하지 않은 네이티브 명령: ${command}`);
      } },
      event: { listen(event, handler) {
        if (!listeners.has(event)) listeners.set(event, new Set());
        listeners.get(event).add(handler);
        return Promise.resolve(() => listeners.get(event).delete(handler));
      } },
    };
  }, { platform, state, staleSnapshot });
}

async function capture(page, name) {
  mkdirSync(outputDir, { recursive: true });
  const file = path.join(outputDir, `${name}.png`);
  await page.screenshot({ path: file, fullPage: false });
  screenshots.push(file);
  console.log(`  Screenshot: ${file}`);
}

async function captureCard(page, name) {
  const clip = await page.$eval(visibleCard, el => {
    const rect = el.getBoundingClientRect();
    return { x: rect.x - 12, y: rect.y - 12, width: rect.width + 24, height: rect.height + 24 };
  });
  mkdirSync(outputDir, { recursive: true });
  const file = path.join(outputDir, `${name}.png`);
  await page.screenshot({ path: file, clip });
  screenshots.push(file);
  console.log(`  Card Screenshot: ${file}`);
}

async function snapshot(page) {
  return page.evaluate(() => {
    const card = document.getElementById('desktop-update-card');
    const bar = card?.querySelector('.dialog-update-progress');
    const button = card?.querySelector('.dialog-update-primary');
    return {
      state: card?.dataset.state,
      visible: Boolean(card && !card.hidden),
      primary: button?.textContent,
      disabled: button?.disabled,
      progressVisible: Boolean(bar && !bar.hidden),
      progressNow: bar?.getAttribute('aria-valuenow'),
      progressText: bar?.getAttribute('aria-valuetext'),
      indeterminate: bar?.classList.contains('dialog-update-indeterminate'),
      detail: card?.querySelector('.dialog-update-detail')?.textContent,
      version: card?.querySelector('.dialog-update-version')?.textContent,
      entryState: document.getElementById('desktop-update-entry')?.dataset.state,
      applyCount: window.__updateMock?.count('cmd_update_apply') || 0,
      checkCount: window.__updateMock?.count('cmd_update_check') || 0,
      dirty: window.__documentState?.isDirty(),
    };
  });
}

async function state(page, next) {
  await page.evaluate(value => window.__updateMock.setState(value), next);
  await page.waitForFunction(value => document.querySelector('#desktop-update-card')?.dataset.state === value, {}, next.state);
}

async function waitState(page, value) {
  await page.waitForFunction(next => document.querySelector('#desktop-update-card')?.dataset.state === next, {}, value);
}

async function clickUnsaved(page, label) {
  const button = await page.evaluateHandle(text => Array.from(document.querySelectorAll('.modal-overlay .dialog-btn'))
    .find(el => el.textContent?.trim() === text), label);
  const element = button.asElement();
  if (!element) throw new Error(`미저장 확인의 ${label} 버튼이 없습니다.`);
  await element.click();
  await page.waitForFunction(() => !document.querySelector('.modal-overlay:not(.dialog-update-blocker)'));
}

async function hasText(page, text) {
  return page.evaluate(value => window.__wasm.getTextRange(0, 0, 0, 200).includes(value), text);
}

async function baseline(browser) {
  if (!baselineRef) return null;
  const previousSource = execFileSync('git', ['show', `${baselineRef}:rhwp-studio/src/ui/update-notice.ts`], { encoding: 'utf8' });
  const previousText = execFileSync('git', ['show', `${baselineRef}:rhwp-studio/src/ui/update-notice-text.ts`], { encoding: 'utf8' });
  const compile = async (source, filename) => (await transformWithOxc(source, filename, { target: 'es2022' })).code
    .replaceAll('@/core/desktop-bridge', '/src/core/desktop-bridge.ts')
    .replaceAll('@/ui/toast', '/src/ui/toast.ts')
    .replaceAll('@/ui/update-notice-text', '/src/ui/update-notice-text.ts');
  // 새 main의 추가 export 조회만 호환시킨다. 이전 updater 동작에는 개입하지 않는다.
  const priorModule = `${await compile(previousSource, 'update-notice.ts')}\nexport function isApplyingUpdate() { return false; }\n`;
  const priorTextModule = await compile(previousText, 'update-notice-text.ts');
  const page = await createPage(browser);
  await installNativeMock(page, { platform: 'Win32' });
  await page.setRequestInterception(true);
  page.on('request', request => {
    const pathname = new URL(request.url()).pathname;
    const body = pathname === '/src/ui/update-notice.ts' ? priorModule : pathname === '/src/ui/update-notice-text.ts' ? priorTextModule : null;
    if (body !== null) void request.respond({ status: 200, contentType: 'application/javascript', body });
    else void request.continue();
  });
  await loadApp(page);
  await page.waitForSelector('#rhwp-toast-container button');
  await page.evaluate(() => new Promise(resolve => setTimeout(resolve, 300)));
  const before = await page.evaluate(() => ({
    labels: Array.from(document.querySelectorAll('#rhwp-toast-container button')).map(button => button.textContent),
    persistentEntry: Boolean(document.querySelector('#desktop-update-entry')),
  }));
  check(before.labels.includes('지금 설치'), '이전 코드의 실제 DOM에서 설치 버튼 문구를 재현한다');
  check(!before.labels.includes('업데이트'), '이전 코드에서 요구한 업데이트 버튼 계약은 FAIL이다');
  check(!before.persistentEntry, '이전 코드에서 Windows 상시 업데이트 재진입 계약은 FAIL이다');
  await capture(page, 'before-v087-windows-ready');
  await page.evaluate(() => Array.from(document.querySelectorAll('#rhwp-toast-container button')).find(button => button.textContent === '지금 설치').click());
  await page.waitForFunction(() => window.__updateMock.count('cmd_update_apply') === 1);
  const beforeApply = await page.evaluate(() => ({
    liveProgress: Boolean(document.querySelector('[role="progressbar"]:not([hidden])')),
    disabled: Array.from(document.querySelectorAll('#rhwp-toast-container button')).find(button => button.textContent === '지금 설치')?.disabled,
  }));
  check(!beforeApply.liveProgress && !beforeApply.disabled, '이전 코드에서 적용 중 진행·중복 방지 계약은 FAIL이다');
  await page.close();
  return { ref: baselineRef, before, beforeApply, expected: { updateButton: 'FAIL', persistentEntry: 'FAIL', applyingProgress: 'FAIL' } };
}

runTest('Desktop 업데이트 카드와 안전한 적용 흐름', async ({ page, browser }) => {
  const errors = [];
  const browserDialogs = [];
  page.on('pageerror', error => errors.push(String(error)));
  page.on('dialog', dialog => {
    browserDialogs.push({ type: dialog.type(), message: dialog.message() });
    if (dialog.type() === 'alert') void dialog.accept();
    else void dialog.dismiss();
  });
  const previous = await baseline(browser);
  await installNativeMock(page);
  await loadApp(page);
  await page.waitForSelector(visibleCard);
  await page.waitForFunction(() => window.__wasm.pageCount > 0);
  await page.evaluate(() => window.__updateMock.ready());

  setTestCase('준비 완료 · 나중에 · 재진입 · 키보드');
  let actual = await snapshot(page);
  check(actual.primary === '업데이트' && !actual.disabled, '주 동작은 활성화된 업데이트 버튼이다');
  check(actual.version === 'v0.8.7 → v0.8.8', '현재 버전과 적용할 버전을 표시한다');
  check(await page.$eval(visibleCard, el => el.getAttribute('aria-modal') === 'false'), '자동 알림은 작업을 막지 않는 카드다');
  await capture(page, 'after-macos-ready-light');
  await captureCard(page, 'ready-card');
  await page.click(`${visibleCard} .dialog-update-later`);
  check(!(await snapshot(page)).visible, '나중에를 누르면 카드가 닫힌다');
  check(await page.$eval('#desktop-update-entry', el => el.textContent === '업데이트 준비됨'), '나중에 이후에도 상태 표시줄 진입점이 남는다');
  check((await snapshot(page)).applyCount === 0, '나중에는 업데이트 적용 IPC를 호출하지 않는다');
  await page.click('#desktop-update-entry');
  await page.waitForSelector(visibleCard);
  check((await snapshot(page)).checkCount === 0, '받아둔 업데이트를 다시 열 때 다운로드 확인을 중복 실행하지 않는다');
  check(await page.evaluate(() => document.activeElement?.classList.contains('dialog-update-close')), '수동으로 연 카드로 키보드 포커스가 이동한다');
  await page.keyboard.press('Tab');
  check(await page.evaluate(() => document.activeElement?.classList.contains('dialog-update-later')), '카드의 나중에 버튼에 키보드로 접근할 수 있다');
  await page.keyboard.press('Tab');
  check(await page.evaluate(() => document.activeElement?.classList.contains('dialog-update-primary')), '업데이트 버튼에 키보드로 접근할 수 있다');
  await page.keyboard.press('Escape');
  check(!(await snapshot(page)).visible, 'Escape로 준비 완료 알림을 닫을 수 있다');
  check(await page.evaluate(() => document.activeElement?.id === 'desktop-update-entry'), '닫은 뒤 상시 진입점으로 포커스를 복원한다');
  await page.evaluate(() => window.__updateMock.emit('hanpage://menu', 'app:check-update'));
  await page.waitForSelector(visibleCard);
  check((await snapshot(page)).state === 'ready', 'macOS 네이티브 업데이트 확인 메뉴도 같은 카드를 다시 연다');

  setTestCase('실시간 다운로드 · 길이 미상 · 파일 확인');
  await state(page, { state: 'downloading', downloaded: 41 * mib, total: 100 * mib });
  actual = await snapshot(page);
  check(actual.progressVisible && actual.progressNow === '41', '실제 전송 이벤트의 진행률 41%를 진행 막대에 반영한다');
  check(actual.detail === '41% · 41.0 MB / 100.0 MB', '받은 용량과 전체 용량을 함께 표시한다');
  check(actual.disabled, '다운로드 중에는 적용 버튼을 비활성화한다');
  await capture(page, 'after-downloading-known');
  await state(page, { state: 'downloading', downloaded: 5 * mib, total: null });
  actual = await snapshot(page);
  check(actual.indeterminate && actual.progressNow === null, '전체 길이를 모르면 가짜 퍼센트 대신 불확정 막대를 표시한다');
  check(actual.detail === '5.0 MB 다운로드됨' && !actual.detail.includes('%'), '길이 미상에서는 받은 용량만 표시한다');
  await capture(page, 'after-downloading-unknown');
  await state(page, { state: 'verifying' });
  actual = await snapshot(page);
  check(actual.progressVisible && actual.indeterminate && actual.detail.includes('파일 확인'), '다운로드와 파일 확인 상태를 구분한다');
  await page.evaluate(() => window.__updateMock.ready());
  await waitState(page, 'ready');

  setTestCase('즉시 적용 표시 · 중복 클릭 · 실패 후 재시도');
  // 독립적인 저장된 내용을 준비해 적용 중 타이핑/Undo/새 문서의 보존을 검사한다.
  await page.click('.dialog-update-later');
  await createNewDocument(page);
  await page.evaluate(() => window.__inputHandler.focus());
  await typeText(page, 'SAVED_UPDATE_SEED');
  await page.evaluate(() => {
    window.__updateMock.saveResult = { status: 'saved', path: '/mock/seed.hwp', name: 'seed.hwp' };
    const item = document.querySelector('.md-item[data-cmd="file:save"]');
    item.closest('.menu-item').querySelector('.menu-title').dispatchEvent(new MouseEvent('mousedown', { bubbles: true, cancelable: true }));
    item.dispatchEvent(new MouseEvent('click', { bubbles: true, cancelable: true }));
  });
  await page.waitForFunction(() => !window.__documentState.isDirty());
  await page.evaluate(() => { window.__updateMock.saveResult = { status: 'cancelled' }; });
  await page.click('#desktop-update-entry');
  await page.click(primary);
  await waitState(page, 'applying');
  actual = await snapshot(page);
  check(actual.disabled && actual.progressVisible && actual.indeterminate, '적용 IPC를 기다리는 동안 즉시 막대와 비활성 버튼을 표시한다');
  check(actual.primary === '업데이트 중…' && actual.detail.includes('적용 중'), '적용 중인 작업을 버튼과 상태 문구로 알린다');
  // 실제 불확정 애니메이션의 마커가 트랙 안을 지나가는 프레임을 직접 캡처한다.
  await page.evaluate(() => new Promise(resolve => setTimeout(resolve, 550)));
  await captureCard(page, 'applying-card');
  await page.evaluate(() => {
    window.__updateMock.emit('hanpage://update-ready', { version: '0.8.8', currentVersion: '0.8.7', notes: null });
    window.__updateMock.emit('hanpage://update-status', { state: 'ready', version: '0.8.8' });
    window.__updateMock.emit('hanpage://update-status', { state: 'checking' });
  });
  check((await snapshot(page)).state === 'applying', '늦은 준비 완료·확인 이벤트가 적용 화면을 되돌리지 않는다');
  const savedBeforeApply = await page.evaluate(() => window.__wasm.getTextRange(0, 0, 0, 200));
  const priorFileName = await page.evaluate(() => window.__wasm.fileName);
  await page.evaluate(() => {
    const saved = window.__updateMock.calls.filter(call => call.command === 'cmd_save_document').at(-1);
    window.__updateMock.pendingDocuments.push({ name: 'must-not-open.hwp', data: saved.args.data });
    window.__updateMock.emit('hanpage://menu', 'file:new');
    window.__updateMock.emit('hanpage://documents-ready', null);
  });
  await page.waitForFunction(() => window.__updateMock.count('cmd_take_pending_documents') >= 2);
  check(await page.evaluate(name => window.__wasm.fileName === name, priorFileName), '적용 중 네이티브 메뉴·파일 연결 이벤트는 현재 문서를 교체하지 않는다');
  await page.click('#scroll-container canvas', { offset: { x: 160, y: 160 } });
  await typeText(page, 'SHOULD_NOT_BE_ADDED');
  await page.keyboard.down('Meta');
  await page.keyboard.press('z');
  await page.keyboard.up('Meta');
  await page.keyboard.down('Control');
  await page.keyboard.press('z');
  await page.keyboard.up('Control');
  await page.keyboard.down('Alt');
  await page.keyboard.press('n');
  await page.keyboard.up('Alt');
  const afterApplyInput = await page.evaluate(() => ({ text: window.__wasm.getTextRange(0, 0, 0, 200), dirty: window.__documentState.isDirty() }));
  check(savedBeforeApply.includes('SAVED_UPDATE_SEED') && afterApplyInput.text === savedBeforeApply && !afterApplyInput.dirty, '실제 적용 중에는 편집·Undo·새 문서 단축키가 저장된 문서를 바꾸지 않는다');
  await page.waitForFunction(() => window.__updateMock.count('cmd_update_apply') === 1);
  await page.evaluate(() => document.querySelector('.dialog-update-primary').click());
  check((await snapshot(page)).applyCount === 1, '적용 중의 반복 클릭은 IPC를 추가 실행하지 않는다');
  await page.focus('.dialog-update-primary');
  await page.keyboard.press('Escape');
  check((await snapshot(page)).visible, '적용 중에는 Escape로 진행 상태를 숨기지 않는다');
  check(await page.$eval('.dialog-update-later', el => el.disabled), '적용 중에는 나중에 버튼도 비활성화한다');
  await capture(page, 'after-applying');
  await page.evaluate(() => window.__updateMock.applyFailure('설치할 위치에 접근할 수 없습니다.'));
  await waitState(page, 'error');
  await page.waitForFunction(() => !document.querySelector('.dialog-update-primary').disabled);
  actual = await snapshot(page);
  check(actual.primary === '다시 업데이트' && actual.detail.includes('접근할 수 없습니다'), '적용 실패 원인과 받아둔 파일의 재시도 동작을 보여준다');
  await capture(page, 'after-apply-error');
  await page.click('#scroll-container canvas', { offset: { x: 160, y: 160 } });
  await typeText(page, 'EDIT_RESTORED');
  check((await snapshot(page)).dirty && await hasText(page, 'EDIT_RESTORED'), '적용 실패 후에는 편집 입력이 정상적으로 복구된다');
  await createNewDocument(page);
  await page.click(primary);
  await waitState(page, 'applying');
  await page.waitForFunction(() => window.__updateMock.count('cmd_update_apply') === 2);
  check((await snapshot(page)).checkCount === 0, '적용 재시도는 다운로드를 다시 확인하지 않는다');
  await page.evaluate(() => window.__updateMock.applyFailure());
  await waitState(page, 'error');

  setTestCase('미저장 문서 보호 · 취소 · 저장 실패 · 저장 성공');
  await page.click('.dialog-update-later');
  await createNewDocument(page);
  await page.evaluate(() => window.__inputHandler.focus());
  await typeText(page, 'UPDATE_KEEP_UNSAVED');
  check((await snapshot(page)).dirty && await hasText(page, 'UPDATE_KEEP_UNSAVED'), '실제 편집으로 미저장 문서를 만든다');
  await page.evaluate(() => window.__updateMock.ready());
  await page.click('#desktop-update-entry');
  await page.click(primary);
  await page.waitForSelector('.modal-overlay .dialog-wrap');
  check((await snapshot(page)).applyCount === 2, '저장 여부를 결정하기 전에는 업데이트 적용을 시작하지 않는다');
  await clickUnsaved(page, '취소');
  check((await snapshot(page)).applyCount === 2 && await hasText(page, 'UPDATE_KEEP_UNSAVED'), '미저장 확인 취소는 적용하지 않고 문서를 보존한다');
  await page.click(primary);
  await page.waitForSelector('.modal-overlay .dialog-wrap');
  await clickUnsaved(page, '저장');
  await page.waitForFunction(() => !document.querySelector('.dialog-update-primary').disabled);
  check((await snapshot(page)).applyCount === 2 && (await snapshot(page)).dirty, '네이티브 저장 취소도 적용을 막고 미저장 상태를 보존한다');
  await page.evaluate(() => { window.__updateMock.saveResult = { error: '테스트 저장 실패' }; });
  await page.click(primary);
  await page.waitForSelector('.modal-overlay .dialog-wrap');
  await clickUnsaved(page, '저장');
  await page.waitForFunction(() => !document.querySelector('.dialog-update-primary').disabled);
  check((await snapshot(page)).applyCount === 2 && (await snapshot(page)).dirty, '저장 실패도 적용을 막고 미저장 상태를 보존한다');
  // 토스트는 저장 실패 안내만이며 제품 업데이트 카드는 그대로 유지된다.
  await page.evaluate(() => document.getElementById('rhwp-toast-container')?.remove());
  await page.setViewport({ width: 390, height: 740 });
  await page.click(primary);
  await page.waitForSelector('.modal-overlay .dialog-wrap');
  const saveIsReachable = await page.evaluate(() => {
    return ['.dialog-btn-primary', '.dialog-title', '.dialog-body'].every(selector => {
      const part = document.querySelector(`.modal-overlay ${selector}`);
      const rect = part.getBoundingClientRect();
      const top = document.elementFromPoint(rect.left + rect.width / 2, rect.top + rect.height / 2);
      return top === part || part.contains(top);
    });
  });
  check(saveIsReachable, '작은 화면에서도 저장 확인의 제목·본문·버튼을 업데이트 카드가 가리지 않는다');
  await capture(page, 'after-unsaved-small');
  await clickUnsaved(page, '취소');
  await page.setViewport({ width: 1280, height: 900 });
  await page.evaluate(() => { window.__updateMock.saveResult = { status: 'saved', path: '/mock/updated-document.hwpx', name: 'updated-document.hwpx' }; });
  await page.click(primary);
  await page.waitForSelector('.modal-overlay .dialog-wrap');
  await clickUnsaved(page, '저장');
  await waitState(page, 'applying');
  await page.waitForFunction(() => window.__updateMock.count('cmd_update_apply') === 3);
  check(!(await snapshot(page)).dirty && await hasText(page, 'UPDATE_KEEP_UNSAVED'), '기존 제품 저장 경로가 성공한 뒤에만 업데이트를 적용한다');
  const nativeSave = await page.evaluate(() => window.__updateMock.calls.filter(call => call.command === 'cmd_save_document').at(-1));
  check(nativeSave.args.data.length > 0, '저장 요청에는 실제 문서 직렬화 바이트가 전달된다');
  await page.evaluate(() => window.__updateMock.applyFailure());
  await waitState(page, 'error');

  setTestCase('확인 실패 · 재확인 · 최신 버전');
  await state(page, { state: 'error', message: '네트워크 연결을 확인해주세요.', retryable: false });
  await page.evaluate(() => { window.__updateMock.checkError = '업데이트 서버에 연결할 수 없습니다.'; });
  await page.click(primary);
  await waitState(page, 'error');
  check((await snapshot(page)).detail.includes('서버에 연결'), '수동 확인 IPC 실패도 빈 화면 대신 원인을 표시한다');
  await page.evaluate(() => { window.__updateMock.checkError = ''; });
  await page.click(primary);
  await waitState(page, 'checking');
  check((await snapshot(page)).progressVisible, '재확인 동안 불확정 진행 막대를 표시한다');
  await state(page, { state: 'upToDate', version: '0.8.7' });
  check((await snapshot(page)).primary === '다시 확인', '최신 버전 결과에서도 다시 확인할 수 있다');

  setTestCase('다크 테마 · 작은 화면 · 악성 오류 텍스트');
  await page.evaluate(() => { window.__theme.setThemeMode('dark'); window.__updateMock.ready(); });
  await waitState(page, 'ready');
  const colors = await page.$eval(visibleCard, el => {
    const css = getComputedStyle(el);
    return { background: css.backgroundColor, color: css.color };
  });
  const rgb = colors.background.match(/\d+/g)?.map(Number);
  check(rgb?.slice(0, 3).every(value => value < 150) && colors.background !== colors.color, '카드는 다크 테마 표면과 읽을 수 있는 글자 색을 따른다');
  await capture(page, 'after-macos-ready-dark');
  await page.setViewport({ width: 390, height: 740 });
  const fits = await page.$eval(visibleCard, el => {
    const rect = el.getBoundingClientRect();
    return rect.left >= 0 && rect.right <= innerWidth && rect.bottom <= innerHeight && el.scrollWidth <= el.clientWidth;
  });
  check(fits, '작은 화면에서도 카드와 버튼이 뷰포트 안에 들어온다');
  await capture(page, 'after-ready-small-dark');
  await state(page, { state: 'error', message: '<img src=x onerror="window.__unexpectedUpdateHtml=true">', retryable: false });
  check(await page.$eval(visibleCard, el => !el.querySelector('.dialog-update-detail img') && el.querySelector('.dialog-update-detail').textContent.includes('<img')), '오류 문자열은 HTML로 실행되지 않고 텍스트로 표시된다');
  check(await page.evaluate(() => !window.__unexpectedUpdateHtml), '오류 본문은 스크립트를 실행하지 않는다');

  setTestCase('Windows 동일 동작 · 시작 이벤트 경합 · 웹 무효화');
  const windows = await createPage(browser);
  await installNativeMock(windows, { platform: 'Win32' });
  await loadApp(windows);
  await windows.waitForSelector(visibleCard);
  check((await snapshot(windows)).primary === '업데이트', 'Windows도 설치 대신 업데이트 버튼을 표시한다');
  check(await windows.$eval('#desktop-update-description', el => el.textContent.includes('설치 프로그램')), 'Windows에서는 적용 후 설치 프로그램이 열리는 동작을 안내한다');
  await windows.click('.dialog-update-later');
  await windows.click('#desktop-update-entry');
  await windows.waitForSelector(visibleCard);
  check((await snapshot(windows)).visible, 'Windows도 상태 표시줄에서 나중에 닫은 업데이트를 다시 연다');
  await capture(windows, 'after-windows-ready-light');
  await windows.close();
  const race = await createPage(browser);
  await installNativeMock(race, { staleSnapshot: true });
  await loadApp(race);
  await race.waitForSelector('#desktop-update-entry[data-state="downloading"]');
  await race.click('#desktop-update-entry');
  await race.waitForSelector(visibleCard);
  check((await snapshot(race)).state === 'downloading' && (await snapshot(race)).progressNow === '20', '조회 중 도착한 최신 이벤트를 오래된 ready 응답이 덮어쓰지 않는다');
  await race.close();
  const web = await createPage(browser);
  await loadApp(web);
  check(await web.evaluate(() => !document.querySelector('#desktop-update-entry') && !document.querySelector('#desktop-update-card')), '일반 웹에서는 업데이트 전용 UI와 네이티브 동작이 생기지 않는다');
  await web.close();
  check(errors.length === 0, '실제 Studio 경로에서 처리되지 않은 브라우저 예외가 없다');

  mkdirSync(outputDir, { recursive: true });
  writeFileSync(path.join(outputDir, 'results.json'), `${JSON.stringify({
    description: '실제 Studio DOM·문서·저장 경로, Tauri IPC와 전송 상태 이벤트만 통제 대역. 실제 설치·재시작은 미실행.',
    baseline: previous,
    sourceHead: execFileSync('git', ['rev-parse', 'HEAD'], { encoding: 'utf8' }).trim(),
    sourceFiles: Object.fromEntries(['src/main.ts', 'src/ui/update-notice.ts', 'src/core/desktop-bridge.ts', 'src/ui/update-notice-text.ts', 'src/styles/update-notice.css'].map(file => [file, createHash('sha256').update(readFileSync(file)).digest('hex')])),
    pass: results.filter(result => result.pass).length,
    fail: results.filter(result => !result.pass).length,
    results, screenshots, browserErrors: errors, browserDialogs,
  }, null, 2)}\n`);
}, { skipLoadApp: true });
