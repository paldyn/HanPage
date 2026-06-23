import test from 'node:test';
import assert from 'node:assert/strict';
import { readFileSync } from 'node:fs';
import { dirname, join } from 'node:path';
import { fileURLToPath } from 'node:url';

const rootDir = dirname(dirname(fileURLToPath(import.meta.url)));

function source(path: string): string {
  return readFileSync(join(rootDir, path), 'utf8');
}

function cellSelectionMouseDownBlock(): string {
  const mouse = source('src/engine/input-handler-mouse.ts');
  const start = mouse.indexOf('if (this.cursor.isInCellSelectionMode()) {');
  assert.notEqual(start, -1, 'cell selection mouse block not found');
  const end = mouse.indexOf('\n  // 우클릭 → 텍스트 선택 블록 유지', start);
  assert.notEqual(end, -1, 'cell selection mouse block end not found');
  return mouse.slice(start, end);
}

function resizeHoverBlock(): string {
  const mouse = source('src/engine/input-handler-mouse.ts');
  const start = mouse.indexOf('export function handleResizeHover');
  assert.notEqual(start, -1, 'handleResizeHover not found');
  const end = mouse.indexOf('\nexport function onMouseUp', start);
  assert.notEqual(end, -1, 'handleResizeHover end not found');
  return mouse.slice(start, end);
}

// #1491 후속: Shift+경계선 드래그는 셀 선택 확장보다 resize 판정이 우선해야 한다.
test('셀 선택 모드 Shift+경계선 클릭은 확장 선택보다 리사이즈를 먼저 시도한다', () => {
  const block = cellSelectionMouseDownBlock();
  const resizeIdx = block.indexOf('this.startResizeDrag(edge, pageX, pageY, pageBboxes, e.shiftKey)');
  const shiftSelectIdx = block.indexOf('if (e.shiftKey || e.ctrlKey || e.metaKey)');

  assert.notEqual(resizeIdx, -1, '경계선 resize 시작 경로 필요');
  assert.notEqual(shiftSelectIdx, -1, 'Shift/Ctrl 셀 선택 경로 필요');
  assert.ok(
    resizeIdx < shiftSelectIdx,
    '경계선 위 Shift+마우스는 셀 선택 확장이 아니라 단일 셀 resize로 들어가야 함',
  );
});

test('표 경계 hover는 hitTest 실패 시 직전 bbox 캐시로 경계선을 다시 판정한다', () => {
  const block = resizeHoverBlock();
  const fallbackIdx = block.indexOf('직전 표 bbox 캐시로 한 번 더 경계선을 확인');
  const clearCacheIdx = block.indexOf('this.cachedTableRef = null');

  assert.notEqual(fallbackIdx, -1, 'hitTest 실패 시 캐시 기반 hover fallback 필요');
  assert.notEqual(clearCacheIdx, -1, '표 밖에서는 캐시 정리 경로 유지 필요');
  assert.ok(fallbackIdx < clearCacheIdx, '캐시를 지우기 전에 경계선 fallback을 먼저 수행해야 함');
  assert.match(block, /this\.cachedCellBboxes\.filter/, 'fallback은 직전 bbox 캐시를 사용해야 함');
  assert.match(block, /hitTestBorder\(pageX,\s*pageY,\s*pageBboxes\)/, 'fallback도 경계선 hitTest를 사용해야 함');
});
