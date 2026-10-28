import test from 'node:test';
import assert from 'node:assert/strict';
import { readFileSync } from 'node:fs';

/** [#156] 문서마다 창 하나 — upstream 파일(main.ts·update-notice)에 둔 얇은 연결 지점을 고정한다. */
const main = readFileSync(new URL('../src/main.ts', import.meta.url), 'utf8');
const updateNotice = readFileSync(new URL('../src/ui/update-notice.ts', import.meta.url), 'utf8');

function between(source: string, start: string, end: string): string {
  const from = source.indexOf(start);
  assert.ok(from >= 0, `${start} 를 찾지 못했다`);
  const to = source.indexOf(end, from + start.length);
  assert.ok(to > from, `${end} 를 찾지 못했다`);
  return source.slice(from, to);
}

test('main.ts 는 첫 문서를 열기 전에 문서 교체·변경 여부를 창 목록에 알리도록 등록한다', () => {
  const hooks = main.indexOf("eventBus.on('document-swapped', () => noteDesktopDocumentSwapped())");
  const dirty = main.indexOf("eventBus.on('document-dirty-changed', () => noteDesktopDirtyChanged())");
  const bridge = main.indexOf('const desktopReady = initDesktopBridge({');
  assert.ok(hooks > 0 && dirty > 0 && hooks < bridge && dirty < bridge);
  const deps = between(main, 'const desktopReady = initDesktopBridge({', 'installUpdateNotice(');
  assert.match(deps, /isDirty: \(\) => documentState\.isDirty\(\)/);
  assert.match(deps, /confirmClose: confirmBeforeWindowClose/);
});

test('시작 흐름: 나중에 연 문서 창은 복구를 제안하지 않고, 흐름이 끝나야 문서를 받는다', () => {
  const chain = between(main, 'await desktopReady;', '})();');
  const skip = between(main, 'function shouldSkipInitialAutosaveRecovery(): boolean {', '\n}\n');
  assert.match(skip, /return params\.has\('url'\) \|\| isSecondaryDesktopWindow\(\);/);
  const blank = chain.indexOf('await openBlankDocumentIfIdle();');
  const ready = chain.indexOf('markDesktopWindowReady();');
  assert.ok(blank > 0 && ready > blank, '빈 문서를 만든 뒤에 준비됨을 알린다');
});

test('새 빈 문서는 다시 손대지 않은 창이 되고, 끌어다 놓은 문서는 받을 수 없으면 새 창으로 보낸다', () => {
  const create = between(main, 'async function createNewDocument(): Promise<void> {', '\n}\n');
  assert.ok(create.indexOf('noteDesktopBlankDocument();') > create.indexOf('await initializeDocument('));
  const drop = between(main, "container.addEventListener('drop', async (e) => {", '\n  });\n}');
  const route = drop.indexOf('if (shouldOpenInNewDesktopWindow()) {');
  assert.ok(route > 0 && route < drop.lastIndexOf('await loadFile(file);'));
  assert.match(drop, /await openInNewDesktopWindow\(new Uint8Array\(await file\.arrayBuffer\(\)\), file\.name\);/);
});

test('창 닫기 확인: 저장 확인을 통과해야 닫고, 저장 안 함이면 임시 저장본을 지운다', () => {
  const close = between(main, 'async function confirmBeforeWindowClose(): Promise<boolean> {', '\n}\n');
  assert.match(close, /if \(isApplyingUpdate\(\)\) return false;/);
  const ask = close.indexOf('if (!await canReplaceCurrentDocument()) return false;');
  const discard = close.indexOf("await autosaveManager.discardCurrentDraft('window-close-discard')");
  assert.ok(ask > 0 && discard > ask);
});

test('업데이트 안내: 준비 알림은 처음 띄우는 창만, 적용은 모든 창에 카드와 입력 보호를 보인다', () => {
  const nudge = between(updateNotice, 'function maybeNudge(version: string): void {', 'function showNudge(');
  assert.match(nudge, /void claimUpdateNudge\(version\)\.then\(\(first\) => \{/);
  assert.match(nudge, /showNudge\(version\);/);
  const receive = between(updateNotice, 'function receiveStatus(next: DesktopUpdateStatus): void {', '\n}\n');
  assert.match(receive, /if \(next\.state === 'applying' && \(!card \|\| card\.hidden\)\) showCard\(\);/);
});
