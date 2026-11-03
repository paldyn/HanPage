/**
 * [#156] HanPage Desktop 문서마다 창 하나 — 실제 Studio 화면 + 모의 Tauri 브리지.
 *
 * 창을 만들고 문서를 고르는 판단은 Rust(`windows.rs` 단위 테스트, 실기 실행)가 맡는다. 여기서는 웹뷰
 * 쪽 계약을 실제 화면으로 확인한다: 창 전용 이벤트 구독, 창 상태 보고, 창 제목, 끌어다 놓기의 새 창
 * 전환, 저장하지 않은 창의 닫기 확인.
 *
 * 실행: CHROME_PATH=... node e2e/run-with-vite.mjs -- node e2e/desktop-multi-window.test.mjs --mode=headless
 */
import { runTest, loadApp, typeText, setTestCase, assert as check } from './helpers.mjs';

async function installNativeMock(page, label) {
  await page.evaluateOnNewDocument((windowLabel) => {
    localStorage.setItem('rhwp-settings', JSON.stringify({ version: 1, theme: { mode: 'light' } }));
    const listeners = [];
    const calls = [];
    window.__mwMock = {
      calls,
      listens: [],
      emit(event, payload) {
        for (const l of listeners) if (l.event === event) l.handler({ payload });
      },
      count(command) { return calls.filter((c) => c.command === command).length; },
      last(command) { return calls.filter((c) => c.command === command).at(-1); },
    };
    window.__TAURI_INTERNALS__ = { metadata: { currentWindow: { label: windowLabel } } };
    window.__TAURI__ = {
      core: { invoke(command, args, options) {
        calls.push({ command, args, options });
        if (command === 'cmd_take_pending_documents') return Promise.resolve([]);
        if (command === 'cmd_update_status') return Promise.resolve({ state: 'upToDate', version: '0.8.11' });
        if (command === 'cmd_claim_update_nudge') return Promise.resolve(true);
        return Promise.resolve(null);
      } },
      event: { listen(event, handler, options) {
        window.__mwMock.listens.push({ event, options: options ?? null });
        const entry = { event, handler };
        listeners.push(entry);
        return Promise.resolve(() => listeners.splice(listeners.indexOf(entry), 1));
      } },
    };
  }, label);
}

async function dropSample(page, sample, name) {
  return page.evaluate(async (path, fileName) => {
    const bytes = new Uint8Array(await (await fetch(path)).arrayBuffer());
    const transfer = new DataTransfer();
    transfer.items.add(new File([bytes], fileName));
    const target = document.getElementById('scroll-container');
    target.dispatchEvent(new DragEvent('drop', { dataTransfer: transfer, bubbles: true, cancelable: true }));
    return bytes.length;
  }, `/samples/${encodeURIComponent(sample)}`, name);
}

async function clickDialogButton(page, text) {
  await page.waitForFunction((label) => Array.from(document.querySelectorAll('.modal-overlay .dialog-btn'))
    .some((el) => el.textContent?.trim() === label), {}, text);
  await page.evaluate((label) => Array.from(document.querySelectorAll('.modal-overlay .dialog-btn'))
    .find((el) => el.textContent?.trim() === label).click(), text);
}

runTest('Desktop 문서마다 창 하나 — 웹뷰 계약', async ({ page }) => {
  const errors = [];
  page.on('pageerror', (error) => errors.push(String(error)));
  await installNativeMock(page, 'main');
  await loadApp(page);
  await page.bringToFront();
  await page.waitForFunction(() => window.__mwMock.count('cmd_report_window_state') > 0, { timeout: 30000 });

  setTestCase('창 전용 이벤트 구독 · 시작 흐름 뒤 손대지 않은 창 보고 · 창 제목');
  const listens = await page.evaluate(() => window.__mwMock.listens);
  for (const event of ['hanpage://documents-ready', 'hanpage://menu', 'hanpage://close-requested']) {
    const found = listens.find((l) => l.event === event);
    check(found && found.options?.target === 'main', `${event} 는 이 창(main) 이벤트만 받는다`);
  }
  const updateListen = listens.find((l) => l.event === 'hanpage://update-status');
  check(updateListen && updateListen.options === null, '업데이트 상태는 앱 전체 이벤트로 받는다');
  const first = await page.evaluate(() => window.__mwMock.last('cmd_report_window_state').args);
  check(first.pristine === true && first.dirty === false && first.nativePath === null,
    `빈 문서로 시작한 창은 손대지 않은 창으로 보고한다 (${JSON.stringify(first)})`);
  const takes = await page.evaluate(() => window.__mwMock.calls.filter((c) => c.command === 'cmd_take_pending_documents').map((c) => c.args));
  check(takes[0]?.initial === true, '첫 대기 문서 요청은 페이지 시작을 알린다');

  setTestCase('끌어다 놓기: 손대지 않은 창은 그 창에서, 문서를 연 창은 새 창으로');
  await dropSample(page, '2010-01-06.hwp', '첫 문서.hwp');
  await page.waitForFunction(() => window.__wasm?.fileName === '첫 문서.hwp' && window.__wasm.pageCount > 0, { timeout: 30000 });
  check(await page.evaluate(() => window.__mwMock.count('cmd_open_in_new_window') === 0), '손대지 않은 창에 놓은 문서는 이 창에서 연다');
  await page.waitForFunction(() => window.__mwMock.last('cmd_report_window_state').args.pristine === false);
  check(await page.evaluate(() => window.__mwMock.calls.some((c) => c.command === 'cmd_set_window_title' && c.args.title.includes('첫 문서.hwp'))),
    '네이티브 창 제목을 문서 이름에 맞춘다');
  const size = await dropSample(page, '143E433F503322BD33.hwp', '둘째 문서.hwp');
  await page.waitForFunction(() => window.__mwMock.count('cmd_open_in_new_window') === 1, { timeout: 10000 });
  const handoff = await page.evaluate(() => {
    const call = window.__mwMock.last('cmd_open_in_new_window');
    const header = (name) => decodeURIComponent(call.options?.headers?.[name] ?? '');
    return {
      raw: call.args instanceof Uint8Array,
      name: header('x-hanpage-name'),
      path: header('x-hanpage-path'),
      length: call.args.length,
      fileName: window.__wasm.fileName,
    };
  });
  check(handoff.raw && handoff.name === '둘째 문서.hwp' && handoff.path === '' && handoff.length === size && handoff.fileName === '첫 문서.hwp',
    `문서를 연 창에 놓은 문서는 바이트를 요청 본문 그대로 새 창으로 넘기고 지금 문서는 그대로 둔다 (${JSON.stringify(handoff)})`);

  setTestCase('저장하지 않은 창 닫기: 받았다고 알리고, 취소하면 남고, 저장 안 함이면 닫는다');
  await page.evaluate(() => window.__inputHandler.focus());
  await typeText(page, 'MULTI_WINDOW_EDIT');
  await page.waitForFunction(() => window.__mwMock.last('cmd_report_window_state').args.dirty === true);
  check(true, '편집하면 변경 있음을 보고한다');
  await page.evaluate(() => window.__mwMock.emit('hanpage://close-requested', null));
  await clickDialogButton(page, '취소');
  await page.waitForFunction(() => window.__mwMock.count('cmd_close_window') === 1);
  const cancelled = await page.evaluate(() => ({
    ack: window.__mwMock.count('cmd_close_ack'),
    answer: window.__mwMock.last('cmd_close_window').args,
    text: window.__wasm.getTextRange(0, 0, 0, 200),
  }));
  check(cancelled.ack === 1 && cancelled.answer.confirmed === false && cancelled.text.includes('MULTI_WINDOW_EDIT'),
    `취소하면 창을 닫지 않고 편집 내용을 그대로 둔다 (${JSON.stringify({ ack: cancelled.ack, answer: cancelled.answer })})`);
  await page.evaluate(() => window.__mwMock.emit('hanpage://close-requested', null));
  await clickDialogButton(page, '저장 안 함');
  await page.waitForFunction(() => window.__mwMock.count('cmd_close_window') === 2);
  const discarded = await page.evaluate(() => window.__mwMock.last('cmd_close_window').args);
  check(discarded.confirmed === true, '저장 안 함을 고르면 창을 닫는다');

  setTestCase('메뉴 명령은 이 창에서만 실행된다');
  const zoomBefore = await page.evaluate(() => document.getElementById('sb-zoom-val')?.textContent ?? '');
  await page.evaluate(() => window.__mwMock.emit('hanpage://menu', 'view:zoom-in'));
  await page.waitForFunction((before) => (document.getElementById('sb-zoom-val')?.textContent ?? '') !== before, { timeout: 5000 }, zoomBefore)
    .catch(() => {});
  const zoomAfter = await page.evaluate(() => document.getElementById('sb-zoom-val')?.textContent ?? '');
  check(zoomAfter !== zoomBefore, `이 창에 온 메뉴 명령(확대)을 실행한다 (${zoomBefore} → ${zoomAfter})`);

  check(errors.length === 0, `처리되지 않은 브라우저 예외가 없다 (${errors.join(' | ')})`);
}, { skipLoadApp: true });
