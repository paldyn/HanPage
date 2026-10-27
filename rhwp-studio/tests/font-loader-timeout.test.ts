import assert from 'node:assert/strict';
import test, { type TestContext } from 'node:test';

import { FONT_RULE_WEBFONT_ENTRIES } from '../src/core/font-rule-runtime.ts';

async function flushPromises(): Promise<void> {
  for (let i = 0; i < 20; i++) await Promise.resolve();
}

function installFontEnvironment(t: TestContext) {
  const documentDescriptor = Object.getOwnPropertyDescriptor(globalThis, 'document');
  const fontFaceDescriptor = Object.getOwnPropertyDescriptor(globalThis, 'FontFace');
  const styles: Array<{ id: string; textContent: string }> = [];
  const requests: Array<{ family: string; resolve: () => void }> = [];
  const registered: string[] = [];

  class FakeFontFace {
    family: string;

    constructor(family: string, _source: string) { this.family = family; }

    load(): Promise<FakeFontFace> {
      return new Promise(resolve => {
        requests.push({ family: this.family, resolve: () => resolve(this) });
      });
    }
  }

  Object.defineProperty(globalThis, 'document', {
    configurable: true,
    value: {
      head: { appendChild: (style: { id: string; textContent: string }) => styles.push(style) },
      createElement: () => ({ id: '', textContent: '' }),
      getElementById: (id: string) => styles.find(style => style.id === id) ?? null,
      fonts: {
        check: () => false,
        add: (face: FakeFontFace) => registered.push(face.family),
      },
    },
  });
  Object.defineProperty(globalThis, 'FontFace', { configurable: true, value: FakeFontFace });
  t.mock.method(console, 'log', () => undefined);
  t.mock.method(console, 'debug', () => undefined);
  t.mock.method(console, 'warn', () => undefined);
  t.mock.timers.enable({ apis: ['setTimeout'] });
  t.after(() => {
    if (documentDescriptor) Object.defineProperty(globalThis, 'document', documentDescriptor);
    else Reflect.deleteProperty(globalThis, 'document');
    if (fontFaceDescriptor) Object.defineProperty(globalThis, 'FontFace', fontFaceDescriptor);
    else Reflect.deleteProperty(globalThis, 'FontFace');
  });
  return { requests, registered };
}

test('CDN 폰트가 응답하지 않아도 개별 5초 제한 뒤 문서 열기를 계속한다', async t => {
  const environment = installFontEnvironment(t);
  const { loadWebFonts } = await import('../src/core/font-loader.ts?timeout=individual');
  const progress: number[] = [];
  let finished = false;
  const loading = loadWebFonts([], loaded => progress.push(loaded)).then(() => { finished = true; });
  assert.ok(environment.requests.length > 0, '필수 웹폰트의 실제 load 요청을 실행한다');

  t.mock.timers.tick(4999);
  await flushPromises();
  assert.equal(finished, false);
  t.mock.timers.tick(1);
  await flushPromises();
  assert.equal(finished, true, '영구 pending CDN 요청이 문서 열기를 막지 않는다');
  await loading;
  assert.equal(progress.at(-1), environment.requests.length);
  assert.equal(environment.registered.length, 0, '받지 못한 폰트를 로드 완료로 등록하지 않는다');
});

test('전체 대기는 8초 안에 끝나고 남은 모든 배치와 별칭 등록은 백그라운드로 계속한다', async t => {
  const environment = installFontEnvironment(t);
  const { loadWebFonts, getWebFontSupplySnapshot } = await import('../src/core/font-loader.ts?timeout=budget');
  const required: string[] = [];
  const files = new Set<string>();
  for (const entry of FONT_RULE_WEBFONT_ENTRIES) {
    if (entry.name.startsWith('함초롬') || files.has(entry.file)) continue;
    required.push(entry.name);
    files.add(entry.file);
    if (required.length === 9) break;
  }
  assert.equal(required.length, 9);
  // font-loader 의 CRITICAL_FONTS(함초롬 2종 + upstream #8191 의 옛한글)는 요청과 무관하게 함께 등록된다.
  const expectedNames = new Set([...required, '함초롬바탕', '함초롬돋움', 'RHWP Old Hangul']);
  const progress: number[] = [];
  let finished = false;
  const loading = loadWebFonts(required, loaded => progress.push(loaded)).then(() => { finished = true; });
  const firstBatch = environment.requests.slice();
  assert.equal(firstBatch.length, 4);

  t.mock.timers.tick(4000);
  firstBatch.forEach(request => request.resolve());
  await flushPromises();
  t.mock.timers.tick(0);
  await flushPromises();
  assert.ok(environment.requests.length > firstBatch.length, '둘째 배치가 실제로 시작된다');

  t.mock.timers.tick(3999);
  await flushPromises();
  assert.equal(finished, false);
  t.mock.timers.tick(1);
  await flushPromises();
  assert.equal(finished, true, '둘째 배치가 아직 로딩 중이어도 전체 8초 예산은 지킨다');
  await loading;
  const foregroundProgressCount = progress.length;

  // 받아진 폰트는 실제 document.fonts에 등록하고 셋째 배치도 빠짐없이 실행한다.
  for (let i = 0; i < 5; i++) {
    environment.requests.forEach(request => request.resolve());
    await flushPromises();
    t.mock.timers.tick(0);
    await flushPromises();
  }
  assert.deepEqual(new Set(environment.registered), expectedNames);
  assert.equal(progress.length, foregroundProgressCount, '종료된 문서 로딩 화면의 진행 콜백은 다시 호출하지 않는다');
  for (const name of expectedNames) assert.equal(getWebFontSupplySnapshot(name).status, 'loaded');

  const requestCount = environment.requests.length;
  await loadWebFonts(required);
  assert.equal(environment.requests.length, requestCount, '배경에서 등록한 별칭은 다음 요청에서 다시 로드하지 않는다');
});

test('8초 예산 뒤 늦게 끝난 CDN 글꼴은 그사이 로컬로 공급된 글꼴을 덮지 않는다', async t => {
  const environment = installFontEnvironment(t);
  const { loadWebFonts, registerLoadedLocalFontFace } = await import('../src/core/font-loader.ts?timeout=late-local');
  const required: string[] = [];
  const files = new Set<string>();
  for (const entry of FONT_RULE_WEBFONT_ENTRIES) {
    if (entry.name.startsWith('함초롬') || files.has(entry.file)) continue;
    required.push(entry.name);
    files.add(entry.file);
    if (required.length === 9) break;
  }
  let finished = false;
  const loading = loadWebFonts(required).then(() => { finished = true; });
  const firstBatch = environment.requests.slice();
  t.mock.timers.tick(4000);
  firstBatch.forEach(request => request.resolve());
  await flushPromises();
  t.mock.timers.tick(0);
  await flushPromises();
  const secondBatch = environment.requests.slice(firstBatch.length);
  assert.ok(secondBatch.length > 0, '둘째 배치가 4초에 시작된다');
  t.mock.timers.tick(4000);
  await flushPromises();
  assert.equal(finished, true, '8초 예산이 끝나 문서 열기는 계속된다');
  await loading;

  // 다음 문서의 선언 환경이 같은 글꼴을 로컬 프로그램으로 공급한 뒤, 둘째 배치(5초 제한 전)가 끝난다.
  const family = secondBatch[0].family;
  registerLoadedLocalFontFace({ family, status: 'loaded' } as unknown as FontFace);
  secondBatch.forEach(request => request.resolve());
  await flushPromises();
  assert.equal(environment.registered.filter(name => name === family).length, 1,
    '로컬 face 만 남고 CDN face 를 추가로 등록하지 않는다');
});
