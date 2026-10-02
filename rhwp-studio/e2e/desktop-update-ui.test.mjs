/**
 * Desktop 업데이트 실제 DOM 회귀. Tauri IPC만 대역 처리하며 실제 설치·재시작은 하지 않는다.
 *
 * node e2e/run-with-vite.mjs -- node e2e/desktop-update-ui.test.mjs --mode=headless
 * --baseline=<Git ref>는 이전 토스트를, --quiet-baseline=<Git ref>는 이전 자동 카드를 비교한다.
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
const quietBaselineRef = process.argv.find(arg => arg.startsWith('--quiet-baseline='))?.slice('--quiet-baseline='.length);
const results = [];
const screenshots = [];
const layoutChecks = [];
const cardLayoutChecks = [];
const cardThemeChecks = [];
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

/** 조용한 자동 확인 화면의 하단 진입점도 실제 픽셀로 보존한다. */
async function captureStatusBar(page, name) {
  const clip = await page.$eval('#status-bar', el => {
    const rect = el.getBoundingClientRect();
    const x = Math.max(0, rect.x - 4);
    const y = Math.max(0, rect.y - 8);
    return { x, y, width: Math.min(innerWidth - x, rect.width + 8), height: Math.min(innerHeight - y, rect.height + 12) };
  });
  mkdirSync(outputDir, { recursive: true });
  const file = path.join(outputDir, `${name}.png`);
  await page.screenshot({ path: file, clip });
  screenshots.push(file);
  console.log(`  Status Bar Screenshot: ${file}`);
  const entryClip = await page.$eval('#desktop-update-entry', el => {
    const rect = el.getBoundingClientRect();
    const x = Math.max(0, rect.x - 16);
    const y = Math.max(0, rect.y - 14);
    return { x, y, width: Math.min(innerWidth - x, rect.width + 80), height: Math.min(innerHeight - y, rect.height + 28) };
  });
  const entryFile = path.join(outputDir, `${name}-entry.png`);
  await page.screenshot({ path: entryFile, clip: entryClip });
  screenshots.push(entryFile);
}

/** 카드의 실제 배치 결과를 읽는다. 작은 높이에서는 세로 스크롤을 허용한다. */
async function cardLayout(page) {
  return page.$eval(visibleCard, el => {
    const rect = el.getBoundingClientRect();
    return {
      viewport: { width: innerWidth, height: innerHeight },
      left: rect.left, right: rect.right, top: rect.top, bottom: rect.bottom,
      width: rect.width, height: rect.height,
      centerX: rect.left + rect.width / 2, centerY: rect.top + rect.height / 2,
      scrollWidth: el.scrollWidth, clientWidth: el.clientWidth,
      scrollHeight: el.scrollHeight, clientHeight: el.clientHeight,
    };
  });
}

function centeredAndFits(layout) {
  return layout.width > 0 && layout.height > 0
    && Math.abs(layout.centerX - layout.viewport.width / 2) <= 1
    && Math.abs(layout.centerY - layout.viewport.height / 2) <= 1
    && layout.left >= -0.5 && layout.top >= -0.5
    && layout.right <= layout.viewport.width + 0.5 && layout.bottom <= layout.viewport.height + 0.5
    && layout.scrollWidth <= layout.clientWidth;
}

/** 실제 제품 테마 모듈을 통해 스킨을 읽고 바꾼다. DOM 속성을 직접 덮지 않는다. */
async function themeSettings(page) {
  return page.evaluate(async () => {
    const theme = await import('/src/core/theme.ts');
    return { mode: theme.getThemeMode(), skin: theme.getThemeSkin() };
  });
}

async function setThemeSettings(page, settings) {
  await page.evaluate(async value => {
    const theme = await import('/src/core/theme.ts');
    theme.setThemeSkin(value.skin);
    theme.setThemeMode(value.mode);
  }, settings);
}

/** CSS color(srgb ...)와 rgb(...)를 같은 실제 8비트 픽셀로 정규화한다. */
async function cardColors(page) {
  return page.$eval(visibleCard, async el => {
    const theme = await import('/src/core/theme.ts');
    const css = getComputedStyle(el);
    const probe = document.createElement('span');
    probe.style.cssText = 'position:fixed;visibility:hidden;pointer-events:none;width:1px;height:1px;background:var(--ui-surface)';
    el.append(probe);
    const raw = {
      background: css.backgroundColor,
      surface: getComputedStyle(probe).backgroundColor,
      text: css.color,
      description: getComputedStyle(el.querySelector('#desktop-update-description')).color,
      return: getComputedStyle(el.querySelector('.dialog-update-return')).color,
    };
    probe.remove();
    const canvas = document.createElement('canvas');
    canvas.width = canvas.height = 1;
    const context = canvas.getContext('2d');
    if (!context) throw new Error('카드의 실제 CSS 색을 정규화할 Canvas2D가 없습니다.');
    const rgba = Object.fromEntries(Object.entries(raw).map(([key, value]) => {
      context.clearRect(0, 0, 1, 1);
      context.fillStyle = value;
      context.fillRect(0, 0, 1, 1);
      return [key, Array.from(context.getImageData(0, 0, 1, 1).data)];
    }));
    const luminance = channels => {
      const [r, g, b] = channels.slice(0, 3).map(channel => {
        const value = channel / 255;
        return value <= 0.04045 ? value / 12.92 : ((value + 0.055) / 1.055) ** 2.4;
      });
      return 0.2126 * r + 0.7152 * g + 0.0722 * b;
    };
    const background = luminance(rgba.background);
    const contrast = Object.fromEntries(['text', 'description', 'return'].map(key => {
      const foreground = luminance(rgba[key]);
      return [key, (Math.max(background, foreground) + 0.05) / (Math.min(background, foreground) + 0.05)];
    }));
    return {
      mode: theme.getThemeMode(), skin: theme.getThemeSkin(), raw, rgba, contrast,
      surfaceDifference: Math.max(...rgba.background.slice(0, 3).map((channel, index) => Math.abs(channel - rgba.surface[index]))),
    };
  });
}

async function clickFileMenu(page, command) {
  await page.bringToFront();
  console.log(`  Menu: ${command} (${page.url()})`);
  await page.click('.menu-item[data-menu="file"] .menu-title');
  await page.click(`.menu-item[data-menu="file"] .md-item[data-cmd="${command}"]`);
}

async function quietDocumentSnapshot(page) {
  return page.evaluate(() => ({
    text: window.__wasm.getTextRange(0, 0, 0, 200),
    dirty: window.__documentState.isDirty(),
    fileName: window.__wasm.fileName,
    focusPreserved: document.activeElement === window.__quietUpdateFocus,
    visibleCard: Boolean(document.querySelector('#desktop-update-card:not([hidden])')),
    modal: Boolean(document.querySelector('.dialog-update-blocker')),
  }));
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
      later: card?.querySelector('.dialog-update-later')?.textContent?.trim(),
      returnHidden: card?.querySelector('.dialog-update-return')?.hidden,
      returnVisible: Boolean(card?.querySelector('.dialog-update-return')?.getClientRects().length),
      disabled: button?.disabled,
      progressVisible: Boolean(bar && !bar.hidden),
      progressNow: bar?.getAttribute('aria-valuenow'),
      progressText: bar?.getAttribute('aria-valuetext'),
      indeterminate: bar?.classList.contains('dialog-update-indeterminate'),
      detail: card?.querySelector('.dialog-update-detail')?.textContent,
      version: card?.querySelector('.dialog-update-version')?.textContent,
      entryState: document.getElementById('desktop-update-entry')?.dataset.state,
      entryLabel: (document.querySelector('#desktop-update-entry .stb-update-label') ?? document.getElementById('desktop-update-entry'))?.textContent?.trim(),
      applyCount: window.__updateMock?.count('cmd_update_apply') || 0,
      checkCount: window.__updateMock?.count('cmd_update_check') || 0,
      dirty: window.__documentState?.isDirty(),
    };
  });
}

async function state(page, next) {
  await page.evaluate(value => window.__updateMock.setState(value), next);
  await page.waitForFunction(value => document.querySelector('#desktop-update-entry')?.dataset.state === value, {}, next.state);
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

/** 이전 카드 코드를 같은 제품 진입점에서 실행하여 자동 팝업 결함만 재현한다. */
async function quietBaseline(browser) {
  if (!quietBaselineRef) return null;
  const source = execFileSync('git', ['show', `${quietBaselineRef}:rhwp-studio/src/ui/update-notice.ts`], { encoding: 'utf8' });
  const module = (await transformWithOxc(source, 'update-notice.ts', { target: 'es2022' })).code
    .replaceAll('@/core/desktop-bridge', '/src/core/desktop-bridge.ts')
    .replaceAll('@/ui/update-notice-text', '/src/ui/update-notice-text.ts');
  const page = await createPage(browser);
  const errors = [];
  page.on('pageerror', error => errors.push(String(error)));
  await installNativeMock(page);
  await page.setRequestInterception(true);
  page.on('request', request => {
    if (new URL(request.url()).pathname === '/src/ui/update-notice.ts') {
      void request.respond({ status: 200, contentType: 'application/javascript', body: module });
    } else void request.continue();
  });
  await loadApp(page);
  await page.waitForSelector(visibleCard);
  const initial = await snapshot(page);
  check(errors.length === 0 && initial.visible && initial.entryState === 'ready', '이전 코드의 정상 실행에서 초기 ready 조회의 자동 팝업 계약은 FAIL이다');
  await capture(page, 'before-v088-ready-auto-card');
  await page.click('.dialog-update-later');
  await page.evaluate(() => window.__updateMock.ready('0.8.9'));
  await page.waitForSelector(visibleCard);
  const nextVersion = await snapshot(page);
  check(nextVersion.visible && nextVersion.version?.includes('0.8.9'), '이전 코드의 다른 버전 ready 이벤트에서도 자동 팝업 계약은 FAIL이다');
  await page.close();
  return { ref: quietBaselineRef, initial, nextVersion, browserErrors: errors, expected: { initialReadyAutoPopup: 'FAIL', nextVersionReadyAutoPopup: 'FAIL' } };
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
  const previousQuiet = await quietBaseline(browser);
  await installNativeMock(page);
  await loadApp(page);
  await page.bringToFront();
  await page.waitForSelector('#desktop-update-entry[data-state="ready"]');
  await page.waitForFunction(() => window.__wasm.pageCount > 0);

  setTestCase('자동 확인은 하단만 표시 · 초기 조회 · 늦은 이벤트 · 편집 보존');
  check(!(await snapshot(page)).visible && (await snapshot(page)).entryLabel === '업데이트 준비됨', '초기 ready 조회는 큰 알림 없이 하단의 업데이트 준비 상태만 표시한다');
  await createNewDocument(page);
  await page.evaluate(() => window.__inputHandler.focus());
  await typeText(page, 'QUIET_BACKGROUND_SEED');
  await page.evaluate(() => { window.__quietUpdateFocus = document.activeElement; });
  const quietBefore = await quietDocumentSnapshot(page);
  check(quietBefore.dirty && quietBefore.text.includes('QUIET_BACKGROUND_SEED'), '자동 업데이트 경계의 실제 미저장 편집 내용을 준비한다');
  for (const [next, label] of [
    [{ state: 'idle' }, '업데이트 확인'],
    [{ state: 'checking' }, '업데이트 확인 중'],
    [{ state: 'downloading', downloaded: 41 * mib, total: 100 * mib }, '업데이트 받는 중 41%'],
    [{ state: 'downloading', downloaded: 5 * mib, total: null }, '업데이트 받는 중'],
    [{ state: 'verifying' }, '업데이트 파일 확인 중'],
    [{ state: 'error', message: '백그라운드 연결 실패', retryable: false }, '업데이트 실패'],
    [{ state: 'error', message: '적용 재시도 가능', retryable: true }, '다시 업데이트'],
    [{ state: 'upToDate', version: '0.8.8' }, '업데이트 확인'],
    [{ state: 'ready', version: '0.8.8' }, '업데이트 준비됨'],
  ]) {
    await state(page, next);
    const quietAfter = await quietDocumentSnapshot(page);
    check((await snapshot(page)).entryLabel === label, `백그라운드 ${next.state} 상태를 하단에 정확히 표시한다 (${label})`);
    check(!quietAfter.visibleCard && !quietAfter.modal && quietAfter.focusPreserved
      && quietAfter.text === quietBefore.text && quietAfter.dirty === quietBefore.dirty && quietAfter.fileName === quietBefore.fileName,
    `백그라운드 ${next.state} 변화는 팝업·포커스·편집 내용을 바꾸지 않는다`);
  }
  for (const version of ['0.8.8', '0.8.9', '0.8.9']) {
    await page.evaluate(value => window.__updateMock.ready(value), version);
    const quietAfter = await quietDocumentSnapshot(page);
    check(!quietAfter.visibleCard && !quietAfter.modal && quietAfter.focusPreserved
      && quietAfter.text === quietBefore.text && quietAfter.dirty === quietBefore.dirty,
    `늦은 ready·다른 버전·중복 이벤트도 조용히 준비 상태만 갱신한다 (${version})`);
  }
  setTestCase('Desktop 최소·일반 크기에서 하단 업데이트와 확대 제어의 경계');
  for (const [width, height] of [[800, 600], [1280, 900]]) {
    await page.setViewport({ width, height });
    // 가장 긴 단계 문구를 실제 하단 버튼에 넣어 이웃 제어와의 경계를 검사한다.
    await state(page, { state: 'verifying' });
    const layout = await page.evaluate(() => {
      const box = el => {
        const rect = el.getBoundingClientRect();
        return { left: rect.left, right: rect.right, top: rect.top, bottom: rect.bottom, width: rect.width, height: rect.height };
      };
      const isVisible = el => el && getComputedStyle(el).display !== 'none' && el.getBoundingClientRect().width > 0;
      const footer = document.getElementById('status-bar');
      const entry = document.getElementById('desktop-update-entry');
      const label = entry.querySelector('.stb-update-label') ?? entry;
      const zoom = footer.querySelector('.stb-right');
      const left = ['sb-page', 'sb-section', 'sb-mode', 'sb-cell-selection'].map(id => document.getElementById(id)).filter(isVisible);
      const zoomControls = Array.from(zoom.children).filter(isVisible);
      return {
        viewport: { width: innerWidth, height: innerHeight },
        footer: box(footer), entry: box(entry), label: box(label), zoom: box(zoom),
        left: left.map(box), zoomControls: zoomControls.map(box),
        entryScrollWidth: entry.scrollWidth, entryClientWidth: entry.clientWidth,
      };
    });
    layoutChecks.push(layout);
    const inside = (child, parent) => child.left >= parent.left - 0.5 && child.right <= parent.right + 0.5
      && child.top >= parent.top - 0.5 && child.bottom <= parent.bottom + 0.5;
    check(layout.entry.width > 0 && inside(layout.entry, layout.footer) && inside(layout.label, layout.entry)
      && layout.footer.left >= 0 && layout.footer.right <= width && layout.footer.bottom <= height
      && layout.entryScrollWidth <= layout.entryClientWidth && layout.entry.right <= layout.zoom.left + 0.5
      && layout.left.every(item => item.right <= layout.entry.left + 0.5)
      // 기존 확대 버튼의 24px 클릭 영역은 22px 상태줄보다 높다. 새 버튼이 영향을
      // 주는 가로 경계는 확대 제어까지, 새 진입점·문구는 세로 경계까지 검사한다.
      && layout.zoomControls.every(item => item.left >= layout.footer.left - 0.5 && item.right <= layout.footer.right + 0.5),
    `${width}×${height}에서 하단 업데이트 문구가 잘리거나 상태·확대 제어와 겹치지 않는다`);
  }
  await page.evaluate(() => window.__updateMock.ready('0.8.9'));
  await capture(page, 'after-quiet-ready-light');
  await captureStatusBar(page, 'after-quiet-statusbar-light');
  await page.evaluate(() => { window.__theme.setThemeMode('dark'); window.__updateMock.ready(); });
  check(!(await snapshot(page)).visible, '다크 테마에서도 백그라운드 준비 완료가 큰 알림을 열지 않는다');
  await capture(page, 'after-quiet-ready-dark');
  await captureStatusBar(page, 'after-quiet-statusbar-dark');
  await page.evaluate(() => window.__theme.setThemeMode('light'));
  await page.click('#desktop-update-entry');
  await page.waitForSelector(visibleCard);

  setTestCase('수동 준비 완료 · 나중에 · 재진입 · 키보드');
  let actual = await snapshot(page);
  check(actual.primary === '업데이트' && !actual.disabled, '주 동작은 활성화된 업데이트 버튼이다');
  check(actual.version === 'v0.8.7 → v0.8.8', '현재 버전과 적용할 버전을 표시한다');
  check(actual.later === '나중에' && actual.returnVisible, '준비된 업데이트에는 나중에 버튼과 추후 업데이트 안내를 유지한다');
  check(await page.$eval(visibleCard, el => el.getAttribute('aria-modal') === 'false'), '수동으로 연 상세도 적용을 누르기 전에는 작업을 막지 않는다');
  await capture(page, 'after-macos-ready-light');
  await captureCard(page, 'ready-card');

  setTestCase('업데이트 카드 중앙 · 최소 창 · 좁은 창 · 짧은 높이');
  const initialViewport = page.viewport();
  for (const [width, height] of [[1280, 900], [800, 600], [390, 740], [800, 320]]) {
    await page.setViewport({ width, height });
    const layout = await cardLayout(page);
    cardLayoutChecks.push({ state: 'ready', ...layout });
    check(centeredAndFits(layout), `${width}×${height}에서 업데이트 카드가 화면 중앙에 있고 뷰포트를 벗어나지 않는다`);
  }
  const shortActions = await page.$eval(visibleCard, el => {
    const actions = Array.from(el.querySelectorAll('.dialog-update-actions button')).map(button => {
      button.scrollIntoView({ block: 'nearest', inline: 'nearest' });
      const rect = button.getBoundingClientRect();
      const top = document.elementFromPoint(rect.left + rect.width / 2, rect.top + rect.height / 2);
      return { label: button.textContent.trim(), visible: rect.top >= 0 && rect.bottom <= innerHeight && (top === button || button.contains(top)) };
    });
    return { viewport: { width: innerWidth, height: innerHeight }, actions };
  });
  cardLayoutChecks.push({ state: 'ready', shortActions });
  check(shortActions.actions.length === 2 && shortActions.actions.every(action => action.visible), '짧은 높이에서도 스크롤하여 나중에·업데이트 버튼에 실제로 접근할 수 있다');
  await capture(page, 'after-ready-short-height');
  await page.setViewport(initialViewport);
  await page.$eval(visibleCard, el => { el.scrollTop = 0; });

  setTestCase('모든 스킨의 밝기 · 은은한 표면 차이 · 본문 가독성');
  const originalTheme = await themeSettings(page);
  try {
    for (const skin of ['default', 'flat', 'oldschool']) {
      for (const mode of ['light', 'dark']) {
        await setThemeSettings(page, { skin, mode });
        const colors = await cardColors(page);
        const layout = await cardLayout(page);
        cardThemeChecks.push({ state: 'ready', ...colors, layout });
        // 8비트 채널 변화는 32 이하로 제한하고, 본문·설명·작은 안내는 4.5:1 대비를 지킨다.
        check(colors.rgba.background[3] === 255 && colors.surfaceDifference > 0 && colors.surfaceDifference <= 32
          && Object.values(colors.contrast).every(value => value >= 4.5) && centeredAndFits(layout),
        `${skin}/${mode}에서 중앙 카드가 기본 표면과 은은히 구별되고 본문·설명·안내를 읽을 수 있다`);
      }
    }
  } finally {
    await setThemeSettings(page, originalTheme);
  }
  const restoredTheme = await themeSettings(page);
  check(restoredTheme.mode === originalTheme.mode && restoredTheme.skin === originalTheme.skin, '스킨 검증 뒤 사용자의 원래 밝기와 스킨을 복원한다');
  setTestCase('수동 준비 완료 · 나중에 · 재진입 · 키보드');
  await page.click(`${visibleCard} .dialog-update-later`);
  check(!(await snapshot(page)).visible, '나중에를 누르면 카드가 닫힌다');
  check(await page.$eval('#desktop-update-entry', el => el.textContent.trim() === '업데이트 준비됨'), '나중에 이후에도 상태 표시줄 진입점이 남는다');
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
  await page.click('.dialog-update-later');
  await page.evaluate(() => { window.__inputHandler.focus(); window.__quietUpdateFocus = document.activeElement; });
  await page.evaluate(() => window.__updateMock.ready('0.8.9'));
  const dismissedReady = await quietDocumentSnapshot(page);
  check(!dismissedReady.visibleCard && dismissedReady.focusPreserved && dismissedReady.text === quietBefore.text && dismissedReady.dirty,
    '나중에 닫은 뒤 다른 버전이 준비돼도 재팝업 없이 미저장 작업을 유지한다');
  await page.evaluate(() => window.__updateMock.ready());
  await clickFileMenu(page, 'app:check-update');
  await page.waitForSelector(visibleCard);
  check((await snapshot(page)).state === 'ready' && (await snapshot(page)).checkCount === 0,
    'Desktop 파일 메뉴는 준비된 상세를 재다운로드 없이 명시적으로 연다');

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
  actual = await snapshot(page);
  check(actual.later === '닫기' && !actual.disabled, '최신 버전 화면의 보조 동작은 나중에 대신 닫기다');
  check(actual.returnHidden && !actual.returnVisible, '최신 버전 화면에는 업데이트를 미루라는 하단 안내를 표시하지 않는다');
  const latestTheme = await page.evaluate(() => window.__theme.getThemeMode());
  for (const mode of ['light', 'dark']) {
    await page.evaluate(value => window.__theme.setThemeMode(value), mode);
    cardThemeChecks.push({ state: 'upToDate', ...(await cardColors(page)), layout: await cardLayout(page) });
    await capture(page, `after-latest-${mode}`);
    await captureCard(page, `latest-${mode}-card`);
  }
  check((await snapshot(page)).later === '닫기' && !(await snapshot(page)).returnVisible
    && (await snapshot(page)).primary === '다시 확인', '최신 버전의 닫기·안내 숨김·다시 확인은 다크 테마에서도 유지된다');
  await page.evaluate(value => window.__theme.setThemeMode(value), latestTheme);
  const beforeRecheck = (await snapshot(page)).checkCount;
  await page.click(primary);
  await waitState(page, 'checking');
  check((await snapshot(page)).checkCount === beforeRecheck + 1 && (await snapshot(page)).progressVisible, '최신 버전의 다시 확인 버튼도 실제 확인 IPC와 진행 표시를 실행한다');
  await state(page, { state: 'upToDate', version: '0.8.7' });
  await page.click(`${visibleCard} .dialog-update-later`);
  await page.waitForFunction(() => document.getElementById('desktop-update-card').hidden);
  check(!(await snapshot(page)).visible, '최신 버전의 닫기 버튼을 누르면 실제 카드가 닫힌다');

  setTestCase('다크 테마 · 작은 화면 · 악성 오류 텍스트');
  await page.evaluate(() => { window.__theme.setThemeMode('dark'); window.__updateMock.ready(); });
  await page.click('#desktop-update-entry');
  await page.waitForSelector(visibleCard);
  await waitState(page, 'ready');
  const colors = await cardColors(page);
  check(colors.rgba.background.slice(0, 3).every(value => value < 150)
    && colors.rgba.background.slice(0, 3).some((value, index) => value !== colors.rgba.text[index])
    && colors.contrast.text >= 4.5, '카드는 다크 테마 표면과 읽을 수 있는 글자 색을 따른다');
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
  await windows.waitForSelector('#desktop-update-entry[data-state="ready"]');
  check(!(await snapshot(windows)).visible, 'Windows 초기 준비 완료도 큰 알림을 자동으로 표시하지 않는다');
  await windows.click('#desktop-update-entry');
  await windows.waitForSelector(visibleCard);
  check((await snapshot(windows)).primary === '업데이트', 'Windows도 설치 대신 업데이트 버튼을 표시한다');
  check(await windows.$eval('#desktop-update-description', el => el.textContent.includes('설치 프로그램')), 'Windows에서는 적용 후 설치 프로그램이 열리는 동작을 안내한다');
  await windows.click('.dialog-update-later');
  await windows.click('#desktop-update-entry');
  await windows.waitForSelector(visibleCard);
  check((await snapshot(windows)).visible, 'Windows도 상태 표시줄에서 나중에 닫은 업데이트를 다시 연다');
  await capture(windows, 'after-windows-ready-light');
  await windows.click('.dialog-update-later');
  await windows.evaluate(() => document.getElementById('studio-root').classList.add('rhwp-chrome-no-status'));
  check(await windows.$eval('#status-bar', el => getComputedStyle(el).display === 'none'), '상태 표시줄 숨김 설정은 자동 업데이트가 임의로 변경하지 않는다');
  await clickFileMenu(windows, 'app:check-update');
  await windows.waitForSelector(visibleCard);
  check((await snapshot(windows)).visible && (await snapshot(windows)).state === 'ready' && (await snapshot(windows)).checkCount === 0,
    'Windows에서도 숨긴 상태 표시줄 대신 파일 메뉴로 준비된 업데이트를 다시 연다');
  await windows.close();
  const race = await createPage(browser);
  await installNativeMock(race, { staleSnapshot: true });
  await loadApp(race);
  await race.waitForSelector('#desktop-update-entry[data-state="downloading"]');
  check(!(await snapshot(race)).visible, '초기 조회 경합 중 최신 다운로드 이벤트도 자동 팝업을 만들지 않는다');
  await race.click('#desktop-update-entry');
  await race.waitForSelector(visibleCard);
  check((await snapshot(race)).state === 'downloading' && (await snapshot(race)).progressNow === '20', '조회 중 도착한 최신 이벤트를 오래된 ready 응답이 덮어쓰지 않는다');
  await race.close();
  const web = await createPage(browser);
  await loadApp(web, '/?lang=ko');
  check(await web.evaluate(() => !document.querySelector('#desktop-update-entry') && !document.querySelector('#desktop-update-card')
    && !document.querySelector('#desktop-update-menu') && !document.querySelector('.md-item[data-cmd="app:check-update"]')),
    '일반 웹에서는 업데이트 카드·상태 버튼·파일 메뉴가 모두 생기지 않는다');

  setTestCase('실제 한국어·영어 제품 정보 · PALDYN 재배포 표기');
  for (const locale of ['ko', 'en']) {
    console.log(`  About: ${locale}`);
    const aboutPage = locale === 'ko' ? web : await createPage(browser);
    if (locale === 'en') await loadApp(aboutPage, '/?lang=en');
    await clickFileMenu(aboutPage, 'file:about');
    await aboutPage.waitForSelector('.modal-overlay .about-body');
    const about = await aboutPage.evaluate(() => ({
      locale: document.documentElement.lang,
      product: document.querySelector('.about-product-name-ko')?.textContent,
      notices: Array.from(document.querySelectorAll('.about-notice')).map(el => el.textContent),
      copyright: document.querySelector('.about-copyright')?.textContent,
    }));
    check(about.locale === locale && about.product?.includes('HanPage') && about.copyright?.includes('PALDYN')
      && !about.copyright?.includes('paldyn'), `${locale} 실제 제품 정보는 HanPage 재배포자를 PALDYN으로 표시한다`);
    check(about.notices.some(notice => notice.includes('rhwp') && notice.includes('MIT') && notice.includes('Edward Kim'))
      && about.copyright?.includes('Edward Kim'), `${locale} 실제 제품 정보는 기반 rhwp의 MIT·Edward Kim 저작권을 보존한다`);
    await capture(aboutPage, `after-about-${locale}-paldyn`);
    await aboutPage.close();
  }
  check(errors.length === 0, '실제 Studio 경로에서 처리되지 않은 브라우저 예외가 없다');

  mkdirSync(outputDir, { recursive: true });
  writeFileSync(path.join(outputDir, 'results.json'), `${JSON.stringify({
    description: '실제 Studio DOM·문서·저장 경로, Tauri IPC와 전송 상태 이벤트만 통제 대역. 실제 설치·재시작은 미실행.',
    baseline: previous,
    quietBaseline: previousQuiet,
    quietDocumentBefore: quietBefore,
    sourceHead: execFileSync('git', ['rev-parse', 'HEAD'], { encoding: 'utf8' }).trim(),
    sourceFiles: Object.fromEntries(['src/main.ts', 'src/ui/update-notice.ts', 'src/core/desktop-bridge.ts', 'src/ui/update-notice-text.ts', 'src/styles/update-notice.css', 'src/ui/about-dialog.ts', 'src/i18n/locales/ko.ts', 'src/i18n/locales/en.ts', 'e2e/desktop-update-ui.test.mjs'].map(file => [file, createHash('sha256').update(readFileSync(file)).digest('hex')])),
    pass: results.filter(result => result.pass).length,
    fail: results.filter(result => !result.pass).length,
    results, screenshots, layoutChecks, cardLayoutChecks, cardThemeChecks, browserErrors: errors, browserDialogs,
  }, null, 2)}\n`);
}, { skipLoadApp: true });
