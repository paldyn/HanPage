import test from 'node:test';
import assert from 'node:assert/strict';

let moduleId = 0;
async function freshBridge() {
  return import(`../src/core/desktop-bridge.ts?test=${++moduleId}`);
}

test('브라우저에서는 native 열기·저장·큐를 등록하지 않는다', async () => {
  const previous = Object.getOwnPropertyDescriptor(globalThis, 'window');
  Object.defineProperty(globalThis, 'window', { configurable: true, value: {} });
  try {
    const bridge = await freshBridge();
    await bridge.initDesktopBridge({ openDocument() { assert.fail('native open'); }, dispatchCommand() {} });
    assert.equal(bridge.getDesktopOpenHandler(), null);
    assert.equal(bridge.getDesktopSaveHandler(), null);
  } finally {
    if (previous) Object.defineProperty(globalThis, 'window', previous);
    else Reflect.deleteProperty(globalThis, 'window');
  }
});

type Invoked = { command: string; args?: Record<string, unknown> };

/** [#156] 대역 창: 문서를 실제로 연 뒤 main.ts 처럼 문서 교체를 브리지에 알린다. */
function windowDeps(bridge: any, events: string[], hold?: Promise<void>, holdName?: string) {
  return {
    async openDocument(bytes: Uint8Array, name: string) {
      events.push(`start:${name}:${bytes[0]}`);
      if (hold && name === holdName) await hold;
      bridge.noteDesktopDocumentSwapped();
      events.push(`finish:${name}`);
    },
    dispatchCommand() {},
    isDirty: () => false,
  };
}

function recordNewWindow(calls: Invoked[], events: string[], command: string, args?: Record<string, unknown>): void {
  calls.push({ command, args });
  if (command === 'cmd_open_in_new_window') {
    events.push(`new-window:${args?.name}:${(args?.data as number[])[0]}:${args?.path ?? ''}`);
  }
}

test('시작 문서는 이 창에서 열고 함께 온 다음 문서는 새 창으로 넘긴 뒤 startup을 끝낸다', async () => {
  const previous = Object.getOwnPropertyDescriptor(globalThis, 'window');
  const events: string[] = [];
  const calls: Invoked[] = [];
  const handlers = new Map<string, (event: { payload: unknown }) => void>();
  const queue: { name: string; path: string; data: number[] }[] = [];
  let releaseOpen!: () => void;
  const firstOpen = new Promise<void>((resolve) => { releaseOpen = resolve; });
  Object.defineProperty(globalThis, 'window', { configurable: true, value: {
    __TAURI_INTERNALS__: { metadata: { currentWindow: { label: 'main' } } },
    __TAURI__: {
      core: { async invoke(command: string, args?: Record<string, unknown>) {
        recordNewWindow(calls, events, command, args);
        if (command === 'cmd_take_pending_documents') {
          events.push(`drain:${args?.initial}`);
          return queue.splice(0);
        }
        return null;
      } },
      event: { async listen(name: string, handler: (event: { payload: unknown }) => void, options?: { target?: string }) {
        if (name === 'hanpage://documents-ready') {
          assert.deepEqual(options, { target: 'main' }, '문서 도착은 이 창 것만 받는다');
          // 파일이 listener 등록 전에 도착하면 이벤트는 놓쳐도 큐에는 남는다.
          queue.push({ name: 'first.hwp', path: '/docs/first.hwp', data: [1] }, { name: 'second.hwpx', path: '/docs/second.hwpx', data: [2] });
          await Promise.resolve();
        }
        handlers.set(name, handler);
        events.push(`listen:${name}`);
        return () => handlers.delete(name);
      } },
    },
  } });
  try {
    const bridge = await freshBridge();
    let ready = false;
    const startup = bridge.initDesktopBridge(windowDeps(bridge, events, firstOpen, 'first.hwp')).then(() => { ready = true; });
    await new Promise<void>((resolve) => setImmediate(resolve));
    assert.equal(ready, false, '기본 빈문서를 여는 시작 흐름은 native 문서 오픈을 기다려야 한다');
    assert.ok(events.indexOf('listen:hanpage://documents-ready') < events.indexOf('drain:true'));
    assert.ok(events.includes('start:first.hwp:1'));
    assert.ok(!events.some((e) => e.startsWith('new-window:')), '첫 문서를 여는 동안 다음 문서를 판단하지 않는다');
    releaseOpen();
    await startup;
    assert.equal(ready, true);
    assert.deepEqual(events.filter((e) => e.startsWith('start:') || e.startsWith('new-window:')),
      ['start:first.hwp:1', 'new-window:second.hwpx:2:/docs/second.hwpx']);

    // 문서를 연 창에 나중에 온 문서도 새 창으로 넘긴다.
    bridge.markDesktopWindowReady();
    assert.deepEqual(calls.filter((c) => c.command === 'cmd_report_window_state').at(-1)?.args,
      { pristine: false, dirty: false, nativePath: '/docs/first.hwp' });
    queue.push({ name: 'warm.hwp', path: '/docs/warm.hwp', data: [3] });
    handlers.get('hanpage://documents-ready')!({ payload: null });
    await new Promise<void>((resolve) => setImmediate(resolve));
    assert.equal(events.at(-1), 'new-window:warm.hwp:3:/docs/warm.hwp');
    assert.deepEqual(calls.filter((c) => c.command === 'cmd_take_pending_documents').map((c) => c.args?.initial),
      [true, false], '첫 호출만 페이지 시작을 알린다');
  } finally {
    releaseOpen();
    if (previous) Object.defineProperty(globalThis, 'window', previous);
    else Reflect.deleteProperty(globalThis, 'window');
  }
});

test('등록 중 이벤트가 먼저 큐를 소비해도 startup과 다음 문서 판단은 진행 중인 열기를 기다린다', async () => {
  const previous = Object.getOwnPropertyDescriptor(globalThis, 'window');
  const events: string[] = [];
  const calls: Invoked[] = [];
  const handlers = new Map<string, (event: { payload: unknown }) => void>();
  const queue: { name: string; path: string; data: number[] }[] = [];
  let releaseOpen!: () => void;
  const firstOpen = new Promise<void>(resolve => { releaseOpen = resolve; });
  Object.defineProperty(globalThis, 'window', { configurable: true, value: {
    __TAURI_INTERNALS__: {},
    __TAURI__: {
      core: { async invoke(command: string, args?: Record<string, unknown>) {
        recordNewWindow(calls, events, command, args);
        return command === 'cmd_take_pending_documents' ? queue.splice(0) : null;
      } },
      event: { async listen(name: string, handler: (event: { payload: unknown }) => void) {
        handlers.set(name, handler);
        if (name === 'hanpage://documents-ready') {
          queue.push({ name: 'registration.hwp', path: '', data: [1] });
          // 등록 완료 Promise보다 이벤트가 먼저 배달되어 초기 drain과 경합한다.
          handler({ payload: null });
          await Promise.resolve();
        }
        return () => handlers.delete(name);
      } },
    },
  } });
  try {
    const bridge = await freshBridge();
    let ready = false;
    const startup = bridge.initDesktopBridge(windowDeps(bridge, events, firstOpen, 'registration.hwp')).then(() => { ready = true; });
    await new Promise<void>(resolve => setImmediate(resolve));
    assert.ok(events.includes('start:registration.hwp:1'), '등록 이벤트가 큐 문서를 실제로 가져간다');
    assert.equal(ready, false, '초기 drain이 빈 큐를 받아도 앞선 이벤트의 열기가 끝나기 전에는 준비 완료가 아니다');

    queue.push({ name: 'during-open.hwpx', path: '/docs/during.hwpx', data: [2] });
    handlers.get('hanpage://documents-ready')!({ payload: null });
    await new Promise<void>(resolve => setImmediate(resolve));
    assert.ok(!events.some((e) => e.includes('during-open')), '문서 교체와 다음 문서 판단을 동시에 실행하지 않는다');
    releaseOpen();
    await startup;
    await new Promise<void>(resolve => setImmediate(resolve));
    assert.deepEqual(events, [
      'start:registration.hwp:1', 'finish:registration.hwp',
      'new-window:during-open.hwpx:2:/docs/during.hwpx',
    ]);
  } finally {
    releaseOpen();
    if (previous) Object.defineProperty(globalThis, 'window', previous);
    else Reflect.deleteProperty(globalThis, 'window');
  }
});

test('손대지 않은 준비된 창만 도착한 문서를 열고, 시작 흐름 중이거나 문서를 연 뒤에는 새 창으로 넘긴다', async () => {
  const previous = Object.getOwnPropertyDescriptor(globalThis, 'window');
  const events: string[] = [];
  const calls: Invoked[] = [];
  const handlers = new Map<string, (event: { payload: unknown }) => void>();
  const queue: { name: string; path: string; data: number[] }[] = [];
  Object.defineProperty(globalThis, 'window', { configurable: true, value: {
    __TAURI_INTERNALS__: { metadata: { currentWindow: { label: 'doc-2' } } },
    __TAURI__: {
      core: { async invoke(command: string, args?: Record<string, unknown>) {
        recordNewWindow(calls, events, command, args);
        return command === 'cmd_take_pending_documents' ? queue.splice(0) : null;
      } },
      event: { async listen(name: string, handler: (event: { payload: unknown }) => void) {
        handlers.set(name, handler);
        return () => handlers.delete(name);
      } },
    },
  } });
  try {
    const bridge = await freshBridge();
    assert.equal(bridge.isSecondaryDesktopWindow(), true);
    await bridge.initDesktopBridge(windowDeps(bridge, events));
    // 시작 흐름(복구 제안·빈 문서) 중 도착한 문서는 이 창에서 열지 않는다.
    queue.push({ name: 'early.hwp', path: '/docs/early.hwp', data: [4] });
    handlers.get('hanpage://documents-ready')!({ payload: null });
    await new Promise<void>((resolve) => setImmediate(resolve));
    assert.deepEqual(events, ['new-window:early.hwp:4:/docs/early.hwp']);
    assert.ok(!calls.some((c) => c.command === 'cmd_report_window_state'), '시작 흐름이 끝나기 전에는 상태를 보고하지 않는다');

    bridge.markDesktopWindowReady();
    assert.deepEqual(calls.at(-1), { command: 'cmd_report_window_state', args: { pristine: true, dirty: false, nativePath: null } });
    queue.push({ name: 'a.hwp', path: '/docs/a.hwp', data: [5] }, { name: 'b.hwp', path: '/docs/b.hwp', data: [6] });
    handlers.get('hanpage://documents-ready')!({ payload: null });
    await new Promise<void>((resolve) => setImmediate(resolve));
    assert.deepEqual(events.slice(1), ['start:a.hwp:5', 'finish:a.hwp', 'new-window:b.hwp:6:/docs/b.hwp']);
    assert.deepEqual(calls.filter((c) => c.command === 'cmd_report_window_state').at(-1)?.args,
      { pristine: false, dirty: false, nativePath: '/docs/a.hwp' });
    assert.equal(bridge.shouldOpenInNewDesktopWindow(), true, '끌어다 놓은 문서도 새 창으로 보낸다');
    bridge.noteDesktopBlankDocument();
    assert.equal(bridge.shouldOpenInNewDesktopWindow(), false, '새 빈 문서가 되면 다시 받는다');
  } finally {
    if (previous) Object.defineProperty(globalThis, 'window', previous);
    else Reflect.deleteProperty(globalThis, 'window');
  }
});

test('저장하지 않은 창의 닫기 요청은 받았다고 알린 뒤 사용자 답을 네이티브에 돌려준다', async () => {
  const previous = Object.getOwnPropertyDescriptor(globalThis, 'window');
  const calls: Invoked[] = [];
  const handlers = new Map<string, (event: { payload: unknown }) => void>();
  const targets = new Map<string, unknown>();
  let answer = false;
  let asked = 0;
  Object.defineProperty(globalThis, 'window', { configurable: true, value: {
    __TAURI_INTERNALS__: { metadata: { currentWindow: { label: 'doc-1' } } },
    __TAURI__: {
      core: { async invoke(command: string, args?: Record<string, unknown>) {
        calls.push({ command, args });
        return command === 'cmd_take_pending_documents' ? [] : null;
      } },
      event: { async listen(name: string, handler: (event: { payload: unknown }) => void, options?: { target?: string }) {
        handlers.set(name, handler);
        targets.set(name, options);
        return () => handlers.delete(name);
      } },
    },
  } });
  try {
    const bridge = await freshBridge();
    await bridge.initDesktopBridge({
      async openDocument() {},
      dispatchCommand() {},
      async confirmClose() { asked += 1; return answer; },
    });
    assert.deepEqual(targets.get('hanpage://close-requested'), { target: 'doc-1' });
    assert.deepEqual(targets.get('hanpage://menu'), { target: 'doc-1' }, '메뉴 명령도 이 창 것만 받는다');
    const close = handlers.get('hanpage://close-requested')!;
    close({ payload: null });
    close({ payload: null }); // 묻는 중 거듭 누른 닫기는 무시한다.
    await new Promise<void>((resolve) => setImmediate(resolve));
    answer = true;
    close({ payload: null });
    await new Promise<void>((resolve) => setImmediate(resolve));
    assert.equal(asked, 2);
    assert.deepEqual(calls.filter((c) => c.command.startsWith('cmd_close')), [
      { command: 'cmd_close_ack', args: undefined },
      // 묻는 중 다시 온 요청에는 살아 있다고만 다시 알린다.
      { command: 'cmd_close_ack', args: undefined },
      { command: 'cmd_close_window', args: { confirmed: false } },
      { command: 'cmd_close_ack', args: undefined },
      { command: 'cmd_close_window', args: { confirmed: true } },
    ]);
  } finally {
    if (previous) Object.defineProperty(globalThis, 'window', previous);
    else Reflect.deleteProperty(globalThis, 'window');
  }
});

test('업데이트 준비 알림은 네이티브가 처음 띄우는 창으로 허락할 때만 띄우고, 확인할 수 없으면 띄운다', async () => {
  const previous = Object.getOwnPropertyDescriptor(globalThis, 'window');
  const answers: unknown[] = [true, false, undefined];
  Object.defineProperty(globalThis, 'window', { configurable: true, value: {
    __TAURI__: { core: { async invoke(command: string) {
      assert.equal(command, 'cmd_claim_update_nudge');
      if (answers.length === 0) throw new Error('이전 런타임');
      return answers.shift();
    } } },
  } });
  try {
    const bridge = await freshBridge();
    assert.equal(await bridge.claimUpdateNudge('0.9.0'), true);
    assert.equal(await bridge.claimUpdateNudge('0.9.0'), false);
    assert.equal(await bridge.claimUpdateNudge('0.9.0'), true, '응답이 없으면 띄운다');
    assert.equal(await bridge.claimUpdateNudge('0.9.0'), true, '명령이 없으면 띄운다');
  } finally {
    if (previous) Object.defineProperty(globalThis, 'window', previous);
    else Reflect.deleteProperty(globalThis, 'window');
  }
});

test('네이티브가 읽지 못한 큐 항목은 열지 않고 알리며 뒤 문서는 순서대로 연다', async () => {
  const previous = Object.getOwnPropertyDescriptor(globalThis, 'window');
  const events: string[] = [];
  const queue: { name: string; path: string; data: number[]; error?: string }[] = [
    { name: 'locked.hwp', path: '/docs/locked.hwp', data: [], error: 'Permission denied (os error 13)' },
    { name: 'ok.hwpx', path: '/docs/ok.hwpx', data: [7] },
  ];
  Object.defineProperty(globalThis, 'window', { configurable: true, value: {
    __TAURI_INTERNALS__: {},
    __TAURI__: {
      core: { async invoke(command: string) {
        assert.equal(command, 'cmd_take_pending_documents');
        return queue.splice(0);
      } },
      event: { async listen() { return () => {}; } },
    },
  } });
  const originalError = console.error;
  console.error = () => {};
  try {
    const bridge = await freshBridge();
    await bridge.initDesktopBridge({
      async openDocument(bytes: Uint8Array, name: string) {
        events.push(`open:${name}:${bytes[0]}`);
      },
      dispatchCommand() {},
      notifyOpenFailure(name: string, message: string) {
        events.push(`fail:${name}:${message}`);
      },
    });
    assert.deepEqual(events, [
      'fail:locked.hwp:Permission denied (os error 13)',
      'open:ok.hwpx:7',
    ]);
  } finally {
    console.error = originalError;
    if (previous) Object.defineProperty(globalThis, 'window', previous);
    else Reflect.deleteProperty(globalThis, 'window');
  }
});

test('업데이트 진행 구독을 완료한 뒤 상태를 읽고 실제 payload를 전달한다', async () => {
  const previous = Object.getOwnPropertyDescriptor(globalThis, 'window');
  const handlers = new Map<string, (event: { payload: unknown }) => void>();
  const events: string[] = [];
  Object.defineProperty(globalThis, 'window', { configurable: true, value: {
    __TAURI_INTERNALS__: {},
    __TAURI__: {
      core: { async invoke(command: string) {
        events.push(command);
        return { state: 'downloading', downloaded: 41, total: 100 };
      } },
      event: { async listen(name: string, handler: (event: { payload: unknown }) => void) {
        await new Promise<void>(resolve => setImmediate(resolve));
        handlers.set(name, handler);
        events.push(name);
        return () => handlers.delete(name);
      } },
    },
  } });
  try {
    const bridge = await freshBridge();
    const received: unknown[] = [];
    await bridge.onUpdateStatus(status => received.push(status));
    assert.deepEqual(await bridge.getUpdateStatus(), { state: 'downloading', downloaded: 41, total: 100 });
    handlers.get('hanpage://update-status')!({ payload: { state: 'applying', version: '0.8.8' } });
    assert.deepEqual(received, [{ state: 'applying', version: '0.8.8' }]);
    assert.deepEqual(events, ['hanpage://update-status', 'cmd_update_status']);
  } finally {
    if (previous) Object.defineProperty(globalThis, 'window', previous);
    else Reflect.deleteProperty(globalThis, 'window');
  }
});

test('수동 업데이트 IPC 실패를 화면이 처리할 수 있도록 반환한다', async () => {
  const previous = Object.getOwnPropertyDescriptor(globalThis, 'window');
  Object.defineProperty(globalThis, 'window', { configurable: true, value: { __TAURI__: {
    core: { async invoke() { throw new Error('연결 실패'); } },
  } } });
  try {
    const bridge = await freshBridge();
    assert.deepEqual(await bridge.checkUpdate(), { ok: false, message: '연결 실패' });
  } finally {
    if (previous) Object.defineProperty(globalThis, 'window', previous);
    else Reflect.deleteProperty(globalThis, 'window');
  }
});

test('네이티브 열기는 PDF 가져오기 지원 여부를 대화상자 필터로 넘긴다', async () => {
  const previous = Object.getOwnPropertyDescriptor(globalThis, 'window');
  const calls: { command: string; args: unknown }[] = [];
  Object.defineProperty(globalThis, 'window', { configurable: true, value: {
    __TAURI_INTERNALS__: {},
    __TAURI__: {
      core: { async invoke(command: string, args?: unknown) {
        calls.push({ command, args });
        if (command === 'cmd_take_pending_documents') return [];
        return null;
      } },
      event: { async listen() { return () => {}; } },
    },
  } });
  try {
    const bridge = await freshBridge();
    await bridge.initDesktopBridge({ async openDocument() {}, dispatchCommand() {} });
    bridge.markDesktopWindowReady();
    const open = bridge.getDesktopOpenHandler();
    assert.ok(open);
    // [#156] 고른 문서는 네이티브가 이 창의 대기 문서(또는 새 창)로 보낸다. 바이트를 직접 넘기지 않는다.
    await open(() => assert.fail('문서는 대기 문서로 도착해야 한다'), { pdfImportEnabled: true });
    await open(() => assert.fail('문서는 대기 문서로 도착해야 한다'));
    assert.deepEqual(calls.filter(c => c.command === 'cmd_open_document').map(c => c.args),
      [{ allowPdf: true }, { allowPdf: false }]);
    assert.ok(!calls.some((c) => c.command === 'cmd_open_in_new_window'));
  } finally {
    if (previous) Object.defineProperty(globalThis, 'window', previous);
    else Reflect.deleteProperty(globalThis, 'window');
  }
});

test('한 번 편집하거나 저장한 창, 문서를 여는 중인 창은 손대지 않은 창이 아니다', async () => {
  const previous = Object.getOwnPropertyDescriptor(globalThis, 'window');
  const calls: Invoked[] = [];
  let dirty = false;
  Object.defineProperty(globalThis, 'window', { configurable: true, value: {
    __TAURI_INTERNALS__: { metadata: { currentWindow: { label: 'main' } } },
    __TAURI__: {
      core: { async invoke(command: string, args?: Record<string, unknown>) {
        calls.push({ command, args });
        if (command === 'cmd_take_pending_documents') return [];
        if (command === 'cmd_save_document') return { status: 'saved', path: '/docs/새 보고서.hwp', name: '새 보고서.hwp' };
        return null;
      } },
      event: { async listen() { return () => {}; } },
    },
  } });
  const lastReport = () => calls.filter((c) => c.command === 'cmd_report_window_state').at(-1)?.args;
  try {
    const bridge = await freshBridge();
    await bridge.initDesktopBridge({ async openDocument() {}, dispatchCommand() {}, isDirty: () => dirty });
    bridge.markDesktopWindowReady();
    assert.equal(lastReport()?.pristine, true);
    // 여는 동안은 받지 않고, 열기에 실패하면 다시 받는다.
    bridge.noteDesktopLoadStarted();
    assert.equal(bridge.shouldOpenInNewDesktopWindow(), true);
    assert.equal(lastReport()?.pristine, false);
    bridge.noteDesktopLoadSettled();
    assert.equal(bridge.shouldOpenInNewDesktopWindow(), false);
    assert.equal(lastReport()?.pristine, true, '열기에 실패하면 예약을 풀도록 다시 보고한다');
    // 편집 없이 바로 저장해도 이제 파일이 있는 창이다.
    const save = bridge.getDesktopSaveHandler();
    assert.ok(save);
    assert.deepEqual(await save({ bytes: new Uint8Array([1]), suggestedName: '새 보고서.hwp', saveAs: true }),
      { status: 'saved', fileName: '새 보고서.hwp' });
    assert.deepEqual(lastReport(), { pristine: false, dirty: false, nativePath: '/docs/새 보고서.hwp' });
    assert.equal(bridge.shouldOpenInNewDesktopWindow(), true);
    // 새 빈 문서를 만들면 다시 받는다.
    bridge.noteDesktopBlankDocument();
    assert.deepEqual(lastReport(), { pristine: true, dirty: false, nativePath: null });
    // 입력했다가 되돌려(또는 저장해) 깨끗해져도 손댄 창이다.
    dirty = true;
    bridge.noteDesktopDirtyChanged();
    dirty = false;
    bridge.noteDesktopDirtyChanged();
    assert.deepEqual(lastReport(), { pristine: false, dirty: false, nativePath: null });
    assert.equal(bridge.shouldOpenInNewDesktopWindow(), true);
  } finally {
    if (previous) Object.defineProperty(globalThis, 'window', previous);
    else Reflect.deleteProperty(globalThis, 'window');
  }
});

test('대기 문서를 열지 못하면(암호 취소·읽기 실패) 이 창의 예약을 풀도록 다시 보고한다', async () => {
  const previous = Object.getOwnPropertyDescriptor(globalThis, 'window');
  const calls: Invoked[] = [];
  const handlers = new Map<string, (event: { payload: unknown }) => void>();
  const queue: { name: string; path: string; data: number[] }[] = [];
  Object.defineProperty(globalThis, 'window', { configurable: true, value: {
    __TAURI_INTERNALS__: { metadata: { currentWindow: { label: 'main' } } },
    __TAURI__: {
      core: { async invoke(command: string, args?: Record<string, unknown>) {
        calls.push({ command, args });
        return command === 'cmd_take_pending_documents' ? queue.splice(0) : null;
      } },
      event: { async listen(name: string, handler: (event: { payload: unknown }) => void) {
        handlers.set(name, handler);
        return () => handlers.delete(name);
      } },
    },
  } });
  try {
    const bridge = await freshBridge();
    await bridge.initDesktopBridge({
      async openDocument() { /* 암호 입력을 취소해 문서를 바꾸지 않았다 */ },
      dispatchCommand() {},
      isDirty: () => false,
    });
    bridge.markDesktopWindowReady();
    queue.push({ name: 'secret.hwp', path: '/docs/secret.hwp', data: [9] });
    handlers.get('hanpage://documents-ready')!({ payload: null });
    await new Promise<void>((resolve) => setImmediate(resolve));
    const take = calls.findLastIndex((c) => c.command === 'cmd_take_pending_documents');
    const reportsAfterOpen = calls.slice(take + 1).filter((c) => c.command === 'cmd_report_window_state');
    assert.ok(reportsAfterOpen.length > 0, '열기를 시도한 뒤 다시 보고해야 네이티브가 예약을 푼다');
    assert.deepEqual(reportsAfterOpen.at(-1)?.args, { pristine: true, dirty: false, nativePath: null });
  } finally {
    if (previous) Object.defineProperty(globalThis, 'window', previous);
    else Reflect.deleteProperty(globalThis, 'window');
  }
});

test('업데이트 사전 확인: 다른 창 때문에 적용할 수 없으면 안내 문구, 아니면 null', async () => {
  const previous = Object.getOwnPropertyDescriptor(globalThis, 'window');
  const answers: unknown[] = ['다른 창(보고서.hwp)에 저장하지 않은 문서가 있습니다.', null, undefined];
  Object.defineProperty(globalThis, 'window', { configurable: true, value: {
    __TAURI__: { core: { async invoke(command: string) {
      assert.equal(command, 'cmd_update_preflight');
      if (answers.length === 0) throw new Error('이전 런타임');
      return answers.shift();
    } } },
  } });
  try {
    const bridge = await freshBridge();
    assert.equal(await bridge.updatePreflight(), '다른 창(보고서.hwp)에 저장하지 않은 문서가 있습니다.');
    assert.equal(await bridge.updatePreflight(), null);
    assert.equal(await bridge.updatePreflight(), null);
    assert.equal(await bridge.updatePreflight(), null, '확인할 수 없으면 막지 않는다(네이티브 적용 단계가 다시 확인한다)');
  } finally {
    if (previous) Object.defineProperty(globalThis, 'window', previous);
    else Reflect.deleteProperty(globalThis, 'window');
  }
});
