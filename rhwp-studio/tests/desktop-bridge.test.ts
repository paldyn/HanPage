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

test('listener 등록 중 온 파일도 큐에서 꺼내고 순서대로 연 뒤 startup을 끝낸다', async () => {
  const previous = Object.getOwnPropertyDescriptor(globalThis, 'window');
  const events: string[] = [];
  const handlers = new Map<string, (event: { payload: unknown }) => void>();
  const queue: { name: string; data: number[] }[] = [];
  let releaseOpen!: () => void;
  const firstOpen = new Promise<void>((resolve) => { releaseOpen = resolve; });
  Object.defineProperty(globalThis, 'window', { configurable: true, value: {
    __TAURI_INTERNALS__: {},
    __TAURI__: {
      core: { async invoke(command: string) {
        assert.equal(command, 'cmd_take_pending_documents');
        events.push('drain');
        return queue.splice(0);
      } },
      event: { async listen(name: string, handler: (event: { payload: unknown }) => void) {
        if (name === 'hanpage://documents-ready') {
          // 파일이 listener 등록 전에 도착하면 이벤트는 놓쳐도 큐에는 남는다.
          queue.push({ name: 'first.hwp', data: [1] }, { name: 'second.hwpx', data: [2] });
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
    const startup = bridge.initDesktopBridge({
      async openDocument(bytes: Uint8Array, name: string) {
        events.push(`open:${name}`);
        assert.equal(bytes[0], name === 'first.hwp' ? 1 : name === 'second.hwpx' ? 2 : 3);
        if (name === 'first.hwp') await firstOpen;
      },
      dispatchCommand() {},
    }).then(() => { ready = true; });
    await new Promise<void>((resolve) => setImmediate(resolve));
    assert.equal(ready, false, '기본 빈문서를 여는 시작 흐름은 native 문서 오픈을 기다려야 한다');
    assert.ok(events.indexOf('listen:hanpage://documents-ready') < events.indexOf('drain'));
    assert.ok(events.includes('open:first.hwp'));
    assert.ok(!events.includes('open:second.hwpx'));
    releaseOpen();
    await startup;
    assert.equal(ready, true);
    assert.deepEqual(events.filter((event) => event.startsWith('open:')), ['open:first.hwp', 'open:second.hwpx']);

    queue.push({ name: 'warm.hwp', data: [3] });
    handlers.get('hanpage://documents-ready')!({ payload: null });
    await new Promise<void>((resolve) => setImmediate(resolve));
    assert.ok(events.includes('open:warm.hwp'));
  } finally {
    releaseOpen();
    if (previous) Object.defineProperty(globalThis, 'window', previous);
    else Reflect.deleteProperty(globalThis, 'window');
  }
});

test('등록 중 이벤트가 먼저 큐를 소비해도 startup과 다음 native 열기는 진행 중인 열기를 기다린다', async () => {
  const previous = Object.getOwnPropertyDescriptor(globalThis, 'window');
  const events: string[] = [];
  const handlers = new Map<string, (event: { payload: unknown }) => void>();
  const queue: { name: string; data: number[] }[] = [];
  let releaseOpen!: () => void;
  const firstOpen = new Promise<void>(resolve => { releaseOpen = resolve; });
  Object.defineProperty(globalThis, 'window', { configurable: true, value: {
    __TAURI_INTERNALS__: {},
    __TAURI__: {
      core: { async invoke(command: string) {
        assert.equal(command, 'cmd_take_pending_documents');
        return queue.splice(0);
      } },
      event: { async listen(name: string, handler: (event: { payload: unknown }) => void) {
        handlers.set(name, handler);
        if (name === 'hanpage://documents-ready') {
          queue.push({ name: 'registration.hwp', data: [1] });
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
    const startup = bridge.initDesktopBridge({
      async openDocument(_bytes: Uint8Array, name: string) {
        events.push(`start:${name}`);
        if (name === 'registration.hwp') await firstOpen;
        events.push(`finish:${name}`);
      },
      dispatchCommand() {},
    }).then(() => { ready = true; });
    await new Promise<void>(resolve => setImmediate(resolve));
    assert.ok(events.includes('start:registration.hwp'), '등록 이벤트가 큐 문서를 실제로 가져간다');
    assert.equal(ready, false, '초기 drain이 빈 큐를 받아도 앞선 이벤트의 열기가 끝나기 전에는 준비 완료가 아니다');

    queue.push({ name: 'during-open.hwpx', data: [2] });
    handlers.get('hanpage://documents-ready')!({ payload: null });
    await new Promise<void>(resolve => setImmediate(resolve));
    assert.ok(!events.includes('start:during-open.hwpx'), '미저장 guard와 문서 교체를 동시에 실행하지 않는다');
    releaseOpen();
    await startup;
    await new Promise<void>(resolve => setImmediate(resolve));
    assert.deepEqual(events, [
      'start:registration.hwp', 'finish:registration.hwp',
      'start:during-open.hwpx', 'finish:during-open.hwpx',
    ]);

    queue.push({ name: 'warm.hwp', data: [3] });
    handlers.get('hanpage://documents-ready')!({ payload: null });
    await new Promise<void>(resolve => setImmediate(resolve));
    assert.deepEqual(events.slice(-2), ['start:warm.hwp', 'finish:warm.hwp']);
  } finally {
    releaseOpen();
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
