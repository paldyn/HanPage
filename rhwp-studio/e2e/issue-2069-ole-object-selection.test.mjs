/**
 * E2E: 한셀 OLE 미리보기는 표처럼 보이더라도 셀 내부 편집으로 진입하지 않는다.
 */
import {
  runTest,
  loadHwpFile,
  screenshot,
  assert,
} from './helpers.mjs';

runTest('Issue #2069 OLE 개체 선택', async ({ page }) => {
  await loadHwpFile(page, '한셀OLE.hwp');

  await page.evaluate(() => {
    window.__wasm.setShowParagraphMarks(true);
    window.__canvasView.loadDocument();
  });

  const clickPoint = await page.evaluate(() => {
    const layout = window.__wasm.getPageControlLayout(0);
    const ole = layout.controls.find(c => c.type === 'ole');
    if (!ole) throw new Error('OLE layout not found');

    const scrollContent = document.querySelector('#scroll-content');
    const rect = scrollContent.getBoundingClientRect();
    const zoom = window.__inputHandler.viewportManager.getZoom();
    const pageIdx = 0;
    const pageOffset = window.__inputHandler.virtualScroll.getPageOffset(pageIdx);
    const pageLeft = window.__inputHandler.virtualScroll.getPageLeftResolved(
      pageIdx,
      scrollContent.clientWidth,
    );

    return {
      x: rect.left + pageLeft + (ole.x + 10) * zoom,
      y: rect.top + pageOffset + (ole.y + 10) * zoom,
      ole,
    };
  });

  await page.mouse.click(clickPoint.x, clickPoint.y);
  await page.evaluate(() => new Promise(resolve => setTimeout(resolve, 300)));

  const state = await page.evaluate(() => {
    const cursor = window.__inputHandler.cursor;
    const ole = window.__wasm.getPageControlLayout(0).controls.find(c => c.type === 'ole');
    const oleCaretRect = window.__wasm.getCursorRect(0, 0, 0);
    return {
      ole,
      selected: cursor.getSelectedPictureRef?.() ?? null,
      rect: cursor.getRect?.() ?? null,
      oleCaretRect,
      isCell: cursor.isInCell?.() ?? false,
      isPictureSelection: cursor.isInPictureObjectSelection?.() ?? false,
    };
  });

  assert(state.isPictureSelection === true, 'OLE 내부 클릭은 개체 선택 상태로 진입해야 함');
  assert(state.selected?.type === 'ole', `선택 개체 타입 확인: ${JSON.stringify(state.selected)}`);
  assert(state.isCell === false, 'OLE 미리보기 내부를 표 셀처럼 편집하면 안 됨');
  assert(
    Math.abs(state.oleCaretRect.x - (state.ole.x + state.ole.w)) <= 1.0,
    `OLE 선택 caret은 OLE 오른쪽에 있어야 함: rect=${JSON.stringify(state.oleCaretRect)}, ole=${JSON.stringify(state.ole)}`,
  );

  await screenshot(page, 'ole-click-object-selection');
});
