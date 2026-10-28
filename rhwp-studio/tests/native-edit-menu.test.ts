import test from 'node:test';
import assert from 'node:assert/strict';
import { readFileSync } from 'node:fs';

import { routeNativeEditToTextInput } from '../src/core/native-edit-menu.ts';

function fakeDocument(active: Record<string, unknown> | null) {
  const commands: string[] = [];
  return {
    commands,
    doc: { activeElement: active, execCommand: (name: string) => { commands.push(name); return true; } } as unknown as Document,
  };
}

function textInput(insideHost: boolean, tagName = 'TEXTAREA') {
  const element = {
    tagName,
    isContentEditable: false,
    selected: 0,
    parentElement: null,
    getAttribute: () => null,
    closest: (selector: string) => (insideHost && selector.includes('#side-panel-host') ? {} : null),
    select() { this.selected += 1; },
  };
  return element;
}

test('AI 패널·대화상자 입력창에 커서가 있으면 네이티브 편집 메뉴를 문서로 보내지 않는다', () => {
  const panelInput = textInput(true);
  for (const [id, expected] of [['edit:undo', 'undo'], ['edit:redo', 'redo'], ['edit:cut', 'cut'], ['edit:copy', 'copy']] as const) {
    const { doc, commands } = fakeDocument(panelInput);
    assert.equal(routeNativeEditToTextInput(id, doc), true, id);
    assert.deepEqual(commands, [expected]);
  }
  const paste = fakeDocument(panelInput);
  assert.equal(routeNativeEditToTextInput('edit:paste', paste.doc), true, '붙이기도 문서에 넣지 않는다');
  assert.deepEqual(paste.commands, []);
  const selectAll = fakeDocument(panelInput);
  assert.equal(routeNativeEditToTextInput('edit:select-all', selectAll.doc), true);
  assert.equal(panelInput.selected, 1, '모두 선택은 그 입력창 안에서만 한다');
});

test('편집기 입력·비편집 명령·포커스 없음은 그대로 문서 명령으로 보낸다', () => {
  assert.equal(routeNativeEditToTextInput('edit:paste', fakeDocument(textInput(false)).doc), false, '편집기 숨김 입력 영역');
  assert.equal(routeNativeEditToTextInput('edit:paste', fakeDocument(textInput(true, 'BUTTON')).doc), false, '패널의 버튼');
  assert.equal(routeNativeEditToTextInput('edit:paste', fakeDocument(null).doc), false);
  assert.equal(routeNativeEditToTextInput('file:save', fakeDocument(textInput(true)).doc), false, '편집 명령이 아니면 관여하지 않는다');
});

test('main.ts 는 네이티브 메뉴 보호와 업데이트 적용 전 AI 정지를 연결한다', () => {
  const source = readFileSync(new URL('../src/main.ts', import.meta.url), 'utf8');
  assert.match(source, /if \(routeNativeEditToTextInput\(id\)\) return;\s*dispatcher\.dispatch\(id\);/);
  assert.match(source, /installUpdateNotice\(\{ beforeApply: confirmBeforeUpdateApply \}\)/);
  const fn = source.slice(source.indexOf('async function confirmBeforeUpdateApply'));
  const first = fn.indexOf('canReplaceCurrentDocument()');
  const unload = fn.indexOf("plugins.unload('ai-assistant')");
  const second = fn.indexOf('canReplaceCurrentDocument()', unload);
  assert.ok(first >= 0 && first < unload && unload < second, '저장 확인 → AI 정지 → 그사이 편집 재확인 순서');
});
