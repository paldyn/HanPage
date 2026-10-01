import test from 'node:test';
import assert from 'node:assert/strict';

import { openDocumentViaPicker } from '../src/command/file-open-picker.ts';
import { getLocale, setLocale } from '../src/i18n/core.ts';

function createFileInput() {
  return {
    dataset: {} as Record<string, string | undefined>,
    clickCount: 0,
    click() {
      this.clickCount += 1;
    },
  };
}

test('교차 출처 SecurityError가 나면 열기 피커는 숨김 파일 입력으로 폴백한다', async () => {
  const input = createFileInput();
  let warnings = 0;
  let alerts = 0;

  await openDocumentViaPicker({
    canReplace: async () => true,
    windowLike: {
      showOpenFilePicker: async () => {
        throw new DOMException('blocked by cross-origin policy', 'SecurityError');
      },
    },
    findFileInput: () => input as unknown as HTMLInputElement,
    emitOpenDocument: () => assert.fail('파일 handle 없이 문서를 열면 안 된다'),
    warn: () => { warnings += 1; },
    alert: () => { alerts += 1; },
  });

  assert.equal(input.clickCount, 1);
  assert.equal(input.dataset.skipUnsavedGuard, 'true');
  assert.equal(warnings, 1);
  assert.equal(alerts, 0);
});

test('사용자가 native 열기 피커를 취소하면 폴백을 다시 열지 않는다', async () => {
  const input = createFileInput();

  await openDocumentViaPicker({
    canReplace: async () => true,
    windowLike: {
      showOpenFilePicker: async () => {
        throw new DOMException('cancelled', 'AbortError');
      },
    },
    findFileInput: () => input as unknown as HTMLInputElement,
    emitOpenDocument: () => assert.fail('취소한 picker가 문서를 열면 안 된다'),
    warn: () => assert.fail('사용자 취소는 경고가 아니어야 한다'),
    alert: () => assert.fail('사용자 취소는 오류 안내가 아니어야 한다'),
  });

  assert.equal(input.clickCount, 0);
  assert.equal(input.dataset.skipUnsavedGuard, undefined);
});

test('데스크톱 파일 읽기 거부는 현재 문서를 바꾸지 않고 번역된 오류를 안내한다', async (context) => {
  const originalLocale = getLocale();
  context.after(() => setLocale(originalLocale));
  const logs: unknown[][] = [];
  context.mock.method(console, 'error', (...args: unknown[]) => { logs.push(args); });

  for (const { locale, error, message } of [
    { locale: 'ko' as const, error: new Error('permission denied'), message: '파일 열기에 실패했습니다:\npermission denied' },
    { locale: 'en' as const, error: 'permission denied', message: 'Opening the file failed:\npermission denied' },
  ]) {
    setLocale(locale);
    const alerts: string[] = [];
    let nativeOpens = 0;
    await assert.doesNotReject(openDocumentViaPicker({
      canReplace: async () => true,
      desktopOpen: async () => { nativeOpens += 1; throw error; },
      windowLike: { showOpenFilePicker: async () => assert.fail('데스크톱 오류 후 웹 picker를 열면 안 된다') },
      findFileInput: () => assert.fail('데스크톱 오류 후 파일 input을 열면 안 된다'),
      emitOpenDocument: () => assert.fail('읽기 실패가 현재 문서를 교체하면 안 된다'),
      warn: () => assert.fail('데스크톱 읽기 실패는 웹 picker 폴백이 아니어야 한다'),
      alert: (message) => { alerts.push(message); },
    }));
    assert.equal(nativeOpens, 1);
    assert.deepEqual(alerts, [message]);
  }
  assert.deepEqual(logs, [
    ['[file:open] 열기 실패:', 'permission denied'],
    ['[file:open] 열기 실패:', 'permission denied'],
  ]);
});

test('데스크톱 열기 취소는 오류 안내나 웹 폴백 없이 끝난다', async () => {
  await openDocumentViaPicker({
    canReplace: async () => true,
    desktopOpen: async () => {},
    windowLike: {},
    findFileInput: () => assert.fail('취소 뒤에 파일 input을 열면 안 된다'),
    emitOpenDocument: () => assert.fail('취소한 picker가 문서를 교체하면 안 된다'),
    warn: () => assert.fail('취소는 경고가 아니어야 한다'),
    alert: () => assert.fail('취소는 오류 안내가 아니어야 한다'),
  });
});

test('미저장 문서 교체가 거부되면 데스크톱 picker를 열지 않는다', async () => {
  await openDocumentViaPicker({
    canReplace: async () => false,
    desktopOpen: async () => assert.fail('미저장 문서 보호를 우회하면 안 된다'),
    windowLike: {},
    findFileInput: () => assert.fail('미저장 문서 보호를 우회하면 안 된다'),
    emitOpenDocument: () => assert.fail('미저장 문서 보호를 우회하면 안 된다'),
    warn: () => assert.fail('교체 거부는 경고가 아니어야 한다'),
    alert: () => assert.fail('교체 거부는 오류 안내가 아니어야 한다'),
  });
});

test('데스크톱 열기 성공은 교체 확인 뒤에 읽은 바이트를 한 번 전달한다', async () => {
  const events: string[] = [];
  const bytes = new Uint8Array([0, 255, 72, 87, 80]);
  await openDocumentViaPicker({
    canReplace: async () => { events.push('confirm'); return true; },
    desktopOpen: async (emit) => { events.push('open'); emit(bytes, '문서.hwp'); },
    windowLike: {},
    findFileInput: () => assert.fail('데스크톱 열기 뒤에 웹 폴백을 열면 안 된다'),
    emitOpenDocument: (payload) => {
      events.push('emit');
      assert.deepEqual(payload, { bytes, fileName: '문서.hwp', fileHandle: null, skipUnsavedGuard: true });
    },
    warn: () => assert.fail('성공은 경고가 아니어야 한다'),
    alert: () => assert.fail('성공은 오류 안내가 아니어야 한다'),
  });
  assert.deepEqual(events, ['confirm', 'open', 'emit']);
});
