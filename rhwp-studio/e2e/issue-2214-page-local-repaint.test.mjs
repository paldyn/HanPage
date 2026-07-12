/**
 * Issue #2214 Stage 1 diagnostic.
 *
 * This is intentionally a diagnostic runner rather than a RED/GREEN regression
 * test. It uses the real Studio document-open and keyboard input paths, records
 * the model/layout/render timeline, and keeps page-only/full-pagination controls
 * in independent document reloads.
 *
 * Usage:
 *   node e2e/issue-2214-page-local-repaint.test.mjs --mode=headless --diagnose
 *   node e2e/issue-2214-page-local-repaint.test.mjs --mode=headless --diagnose \
 *     --formats=hwp --runs=1
 */

import assert from 'node:assert/strict';
import crypto from 'node:crypto';
import { mkdirSync, readFileSync, writeFileSync } from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';
import pixelmatch from 'pixelmatch';
import { PNG } from 'pngjs';
import {
  closeBrowser,
  closePage,
  createPage,
  launchBrowser,
  loadApp,
} from './helpers.mjs';

const __dirname = path.dirname(fileURLToPath(import.meta.url));
const REPO_ROOT = path.resolve(__dirname, '../..');
const OUTPUT_ROOT = process.env.ISSUE2214_OUTPUT_ROOT
  ?? path.join(REPO_ROOT, 'output/poc/task2214/stage2');
const TARGET = Object.freeze({
  sectionIndex: 0,
  paragraphIndex: 5,
  charOffset: 130,
  parentParaIndex: 0,
  controlIndex: 2,
  cellIndex: 2,
  cellParaIndex: 5,
  cellPath: [{ controlIndex: 2, cellIndex: 2, cellParaIndex: 5 }],
});
const MAX_INSERTS = 128;

const SAMPLES = Object.freeze({
  hwp: path.join(REPO_ROOT, 'samples/issue1949_giant_cell_nested_tables_perf.hwp'),
  hwpx: path.join(REPO_ROOT, 'samples/issue1949_giant_cell_nested_tables_perf.hwpx'),
});

function cliValue(name, fallback) {
  const arg = process.argv.find((value) => value.startsWith(`--${name}=`));
  return arg ? arg.slice(name.length + 3) : fallback;
}

function sanitizeLabel(value) {
  return String(value).replace(/[^a-zA-Z0-9._-]+/g, '-');
}

function sha256(buffer) {
  return crypto.createHash('sha256').update(buffer).digest('hex');
}

function percentile(values, quantile) {
  if (!values.length) return null;
  const sorted = [...values].sort((a, b) => a - b);
  const index = Math.min(sorted.length - 1, Math.max(0, Math.ceil(sorted.length * quantile) - 1));
  return sorted[index];
}

function summarizeDurations(values) {
  return {
    count: values.length,
    p50Ms: percentile(values, 0.5),
    p95Ms: percentile(values, 0.95),
    maxMs: values.length ? Math.max(...values) : null,
  };
}

function writeJson(filePath, value) {
  mkdirSync(path.dirname(filePath), { recursive: true });
  writeFileSync(filePath, `${JSON.stringify(value, null, 2)}\n`);
}

async function waitTwoRafs(page) {
  await page.evaluate(() => new Promise((resolve) => {
    requestAnimationFrame(() => requestAnimationFrame(resolve));
  }));
}

async function delay(page, ms) {
  await page.evaluate((value) => new Promise((resolve) => setTimeout(resolve, value)), ms);
}

async function clickBlockingModalChoice(page) {
  return page.evaluate(() => {
    const allowed = new Set(['그대로 보기', '대체 글꼴로 보기']);
    const buttons = Array.from(document.querySelectorAll('button'));
    const button = buttons.find((candidate) => {
      const label = candidate.textContent?.trim() ?? '';
      const style = getComputedStyle(candidate);
      return allowed.has(label) && style.display !== 'none' && style.visibility !== 'hidden';
    });
    if (!button) return null;
    const label = button.textContent?.trim() ?? '';
    button.click();
    return label;
  });
}

async function openDocumentThroughApp(page, format, bytes) {
  const fileName = path.basename(SAMPLES[format]);
  const encoded = bytes.toString('base64');
  const requestId = `issue2214-${format}-${crypto.randomUUID()}`;

  await page.evaluate(({ base64, name, id }) => {
    const binary = atob(base64);
    const payload = new Uint8Array(binary.length);
    for (let i = 0; i < binary.length; i += 1) payload[i] = binary.charCodeAt(i);

    window.__issue2214LoadResult = null;
    const off = window.__eventBus.on('open-document-bytes:done', (result) => {
      if (result?.requestId !== id) return;
      off();
      window.__issue2214LoadResult = result;
    });
    window.__eventBus.emit('open-document-bytes', {
      bytes: payload,
      fileName: name,
      fileHandle: null,
      skipUnsavedGuard: true,
      requestId: id,
    });
  }, { base64: encoded, name: fileName, id: requestId });

  const deadline = Date.now() + 90_000;
  const modalChoices = [];
  let result = null;
  while (Date.now() < deadline) {
    const choice = await clickBlockingModalChoice(page);
    if (choice) modalChoices.push(choice);
    result = await page.evaluate(() => window.__issue2214LoadResult);
    if (result) break;
    await new Promise((resolve) => setTimeout(resolve, 100));
  }
  assert.ok(result, `${format}: open-document-bytes:done timeout`);
  assert.equal(result.ok, true, `${format}: document load failed: ${result.error ?? 'unknown'}`);

  await page.evaluate(() => document.fonts.ready);
  await waitTwoRafs(page);
  const state = await page.evaluate(() => {
    const wasm = window.__wasm;
    const input = window.__inputHandler;
    const canvasView = window.__canvasView;
    return {
      sourceFormat: wasm.getSourceFormat(),
      pageCount: wasm.pageCount,
      inputActive: input.isActive(),
      hasPageZeroCanvas: Boolean(canvasView.canvasPool.getCanvas(0)),
      fontsStatus: document.fonts.status,
      devicePixelRatio: window.devicePixelRatio,
      zoom: canvasView.viewportManager.getZoom(),
      viewport: { width: window.innerWidth, height: window.innerHeight },
    };
  });
  assert.equal(state.sourceFormat, format, `${format}: source format mismatch`);
  assert.equal(state.pageCount, 115, `${format}: expected 115 pages`);
  assert.equal(state.inputActive, true, `${format}: input handler inactive`);
  assert.equal(state.hasPageZeroCanvas, true, `${format}: page zero canvas missing`);
  assert.equal(state.fontsStatus, 'loaded', `${format}: fonts are not ready`);
  return { ...state, modalChoices };
}

async function moveToTarget(page) {
  const position = await page.evaluate((target) => {
    const input = window.__inputHandler;
    input.cursor.clearSelection();
    input.cursor.moveTo(target);
    input.cursor.resetPreferredX();
    input.updateCaret();
    input.focus();
    return {
      position: input.cursor.getPosition(),
      rect: input.cursor.getRect(),
      focused: document.activeElement === input.textarea,
    };
  }, TARGET);
  assert.equal(position.focused, true, 'hidden textarea was not focused');
  assert.equal(position.position.charOffset, TARGET.charOffset, 'target char offset mismatch');
  assert.equal(position.position.cellParaIndex, TARGET.cellParaIndex, 'target cell paragraph mismatch');
  return position;
}

async function dispatchInputSynchronously(page, text = '1') {
  return page.evaluate((value) => {
    const input = window.__inputHandler;
    input.textarea.value = value;
    const startedAt = performance.now();
    input.textarea.dispatchEvent(new InputEvent('input', {
      bubbles: true,
      inputType: 'insertText',
      data: value,
    }));
    const wasm = window.__wasm;
    const position = input.cursor.getPosition();
    const length = wasm.getCellParagraphLength(0, 0, 2, 2, 5);
    const modelText = wasm.getTextInCell(0, 0, 2, 2, 5, 0, length);
    return {
      durationMs: performance.now() - startedAt,
      cursor: position,
      modelLength: length,
      modelTail: modelText.slice(-40),
      lineInfo: wasm.getLineInfoInCell(0, 0, 2, 2, 5, position.charOffset),
      pending: input.hasDeferredPaginationPending(),
    };
  }, text);
}

async function readLineCount(page) {
  return page.evaluate(() => {
    const input = window.__inputHandler;
    const offset = input.cursor.getPosition().charOffset;
    return window.__wasm.getLineInfoInCell(0, 0, 2, 2, 5, offset).lineCount;
  });
}

async function typeKeyboardOnes(page, count) {
  const durations = [];
  for (let i = 0; i < count; i += 1) {
    const startedAt = performance.now();
    await page.keyboard.type('1');
    durations.push(performance.now() - startedAt);
  }
  return durations;
}

async function restoreTrace(page) {
  await page.evaluate(() => {
    const trace = window.__issue2214Trace;
    if (!trace) return;
    for (const restore of trace.restores ?? []) restore();
    for (const off of trace.offs ?? []) off();
    window.__issue2214Trace = null;
  });
}

async function installTrace(page) {
  await restoreTrace(page);
  await page.evaluate(() => {
    const trace = {
      startedAt: performance.now(),
      events: [],
      restores: [],
      offs: [],
      sequence: 0,
    };
    const record = (type, detail = {}) => {
      trace.events.push({
        sequence: ++trace.sequence,
        type,
        atMs: performance.now() - trace.startedAt,
        ...detail,
      });
    };
    const wrap = (object, name, type, describeArgs = () => ({}), describeResult = () => ({})) => {
      const original = object?.[name];
      if (typeof original !== 'function') return;
      object[name] = function issue2214Wrapped(...args) {
        const startedAt = performance.now();
        let result;
        try {
          result = original.apply(this, args);
          return result;
        } finally {
          record(type, {
            ...describeArgs(args),
            ...describeResult(result),
            durationMs: performance.now() - startedAt,
          });
        }
      };
      trace.restores.push(() => { object[name] = original; });
    };

    const wasm = window.__wasm;
    const input = window.__inputHandler;
    const canvasView = window.__canvasView;
    const renderer = canvasView.pageRenderer;

    wrap(wasm, 'flushDeferredPagination', 'wasm.flushDeferredPagination', (args) => ({ argsLength: args.length }));
    wrap(input, 'flushDeferredPaginationIfNeeded', 'input.flushDeferredPaginationIfNeeded', (args) => ({
      reason: args[0] ?? 'manual',
      emitChange: args[1] ?? true,
    }), (result) => ({ result }));
    wrap(wasm, 'renderPageToCanvasFiltered', 'wasm.renderPageToCanvasFiltered', (args) => ({
      pageIndex: args[0],
      scale: args[2],
      layerKind: args[3],
    }));
    wrap(wasm, 'insertTextInCellDeferredPagination', 'wasm.insertTextInCellDeferredPagination', (args) => ({
      sectionIndex: args[0],
      parentParaIndex: args[1],
      controlIndex: args[2],
      cellIndex: args[3],
      cellParaIndex: args[4],
      charOffset: args[5],
      textLength: String(args[6] ?? '').length,
    }));
    wrap(wasm, 'getCursorRectByPathNear', 'wasm.getCursorRectByPathNear', (args) => ({
      sectionIndex: args[0],
      parentParaIndex: args[1],
      charOffset: args[3],
      hintPage: args[4],
    }));
    wrap(wasm, 'getCursorRectByPath', 'wasm.getCursorRectByPath', (args) => ({
      sectionIndex: args[0],
      parentParaIndex: args[1],
      charOffset: args[3],
    }));
    wrap(input.cursor, 'updateRect', 'CursorState.updateRect');
    wrap(input.cursor, 'moveTo', 'CursorState.moveTo', (args) => ({
      charOffset: args[0]?.charOffset,
      cellParaIndex: args[0]?.cellParaIndex,
    }));
    wrap(input, 'executeOperation', 'InputHandler.executeOperation', (args) => ({
      kind: args[0]?.kind,
      commandType: args[0]?.command?.type,
    }));
    wrap(input, 'refreshAfterOperation', 'InputHandler.refreshAfterOperation', (args) => ({
      requested: args[0],
      fallback: args[1],
      commandType: args[2],
    }));
    wrap(input, 'afterPageLocalEdit', 'InputHandler.afterPageLocalEdit');
    wrap(input, 'flushDeferredPaginationForCellOverflow', 'InputHandler.flushDeferredPaginationForCellOverflow',
      () => ({}), (result) => ({ result }));
    wrap(input, 'updateCaret', 'InputHandler.updateCaret');
    wrap(renderer, 'renderPage', 'PageRenderer.renderPage', (args) => ({
      pageIndex: args[0],
      renderScale: args[2],
      zoom: args[3],
      dpr: args[4],
      context: args[5] ?? {},
    }), (result) => ({ result }));
    wrap(canvasView, 'refreshInvalidatedPageNow', 'CanvasView.refreshInvalidatedPageNow', (args) => ({
      pageIndex: args[0],
      context: args[1] ?? {},
    }));

    trace.offs.push(window.__eventBus.on('document-page-invalidated', (payload) => {
      record('event.document-page-invalidated', { payload });
    }));
    trace.offs.push(window.__eventBus.on('document-changed', (payload) => {
      record('event.document-changed', { payload });
    }));
    window.__issue2214Trace = trace;
  });
}

async function collectTrace(page) {
  return page.evaluate(() => {
    const trace = window.__issue2214Trace;
    const events = trace?.events ?? [];
    const count = (type) => events.filter((event) => event.type === type).length;
    return {
      startedAt: trace?.startedAt ?? null,
      events,
      counts: {
        wasmFlush: count('wasm.flushDeferredPagination'),
        inputFlush: count('input.flushDeferredPaginationIfNeeded'),
        invalidation: count('event.document-page-invalidated'),
        documentChanged: count('event.document-changed'),
        filteredRender: count('wasm.renderPageToCanvasFiltered'),
        pageRender: count('PageRenderer.renderPage'),
        refreshNow: count('CanvasView.refreshInvalidatedPageNow'),
        deferredInsert: count('wasm.insertTextInCellDeferredPagination'),
        cursorRectNear: count('wasm.getCursorRectByPathNear'),
        cursorRect: count('wasm.getCursorRectByPath'),
        executeOperation: count('InputHandler.executeOperation'),
      },
    };
  });
}

async function resolveCompositedClip(page) {
  const clip = await page.evaluate(() => {
    const canvasView = window.__canvasView;
    const wasm = window.__wasm;
    const canvas = canvasView.canvasPool.getCanvas(0);
    if (!canvas) throw new Error('page zero canvas unavailable');
    const rect = canvas.getBoundingClientRect();
    const tree = wasm.getPageLayerTreeObject(0);
    const ops = [];
    const visit = (node) => {
      if (!node) return;
      if (node.kind === 'group') for (const child of node.children ?? []) visit(child);
      else if (node.kind === 'clipRect') visit(node.child);
      else if (node.kind === 'leaf') ops.push(...(node.ops ?? []));
    };
    visit(tree.root);
    const targets = ops.filter((op) => {
      if (op.type !== 'textRun') return false;
      const key = op.source?.stableSourceKey ?? '';
      return key.startsWith('section:0/para:5/') && key.includes('/cell:0:2:2:5:');
    });
    const minY = targets.length
      ? Math.min(...targets.map((op) => op.bbox?.y ?? 0))
      : (window.__inputHandler.cursor.getRect()?.y ?? 200);
    const scale = tree.pageWidth > 0 ? rect.width / tree.pageWidth : 1;
    const y = Math.max(0, Math.floor(rect.top + (minY - 35) * scale));
    return {
      x: Math.max(0, Math.floor(rect.left)),
      y,
      width: Math.max(1, Math.floor(rect.width)),
      height: 240,
      pageWidth: tree.pageWidth,
      scale,
      targetOpCount: targets.length,
    };
  });
  assert.ok(clip.targetOpCount > 0, 'target paragraph textRun ops were not found');
  return clip;
}

async function captureCompositedCrop(page, filePath, clip) {
  mkdirSync(path.dirname(filePath), { recursive: true });
  await page.evaluate(() => {
    for (const element of document.querySelectorAll('#scroll-content .caret, #scroll-content .caret-composition')) {
      element.dataset.issue2214Visibility = element.style.visibility;
      element.style.visibility = 'hidden';
    }
  });
  try {
    await page.screenshot({
      path: filePath,
      clip: { x: clip.x, y: clip.y, width: clip.width, height: clip.height },
      captureBeyondViewport: true,
    });
  } finally {
    await page.evaluate(() => {
      for (const element of document.querySelectorAll('#scroll-content .caret, #scroll-content .caret-composition')) {
        element.style.visibility = element.dataset.issue2214Visibility ?? '';
        delete element.dataset.issue2214Visibility;
      }
    });
  }
  const bytes = readFileSync(filePath);
  return { filePath, sha256: sha256(bytes), size: bytes.length, clip };
}

async function collectState(page, label, timelineStart) {
  return page.evaluate(({ checkpoint, startedAt }) => {
    const wasm = window.__wasm;
    const input = window.__inputHandler;
    const canvasView = window.__canvasView;
    const position = input.cursor.getPosition();
    const rect = input.cursor.getRect();
    const length = wasm.getCellParagraphLength(0, 0, 2, 2, 5);
    const text = wasm.getTextInCell(0, 0, 2, 2, 5, 0, length);

    const lineByIndex = new Map();
    for (let offset = 0; offset <= length; offset += 1) {
      try {
        const info = wasm.getLineInfoInCell(0, 0, 2, 2, 5, offset);
        if (info && !lineByIndex.has(info.lineIndex)) lineByIndex.set(info.lineIndex, info);
      } catch { /* end offsets differ by format; retain successfully exposed lines */ }
    }
    const lines = Array.from(lineByIndex.values()).sort((a, b) => a.lineIndex - b.lineIndex);

    const pageIndex = Number.isInteger(rect?.pageIndex) ? rect.pageIndex : 0;
    const tree = wasm.getPageLayerTreeObject(pageIndex);
    const ops = [];
    const visit = (node) => {
      if (!node) return;
      if (node.kind === 'group') for (const child of node.children ?? []) visit(child);
      else if (node.kind === 'clipRect') visit(node.child);
      else if (node.kind === 'leaf') ops.push(...(node.ops ?? []));
    };
    visit(tree.root);
    const targetOps = ops.filter((op) => {
      if (op.type !== 'textRun') return false;
      const key = op.source?.stableSourceKey ?? '';
      return key.startsWith('section:0/para:5/') && key.includes('/cell:0:2:2:5:');
    });

    let layoutRuns = [];
    let layoutError = null;
    try {
      const layout = JSON.parse(wasm.doc.getPageTextLayout(pageIndex));
      layoutRuns = (layout.runs ?? []).filter((run) => (
        run.secIdx === 0
        && run.parentParaIdx === 0
        && run.controlIdx === 2
        && run.cellIdx === 2
        && run.cellParaIdx === 5
      )).map((run) => ({
        text: run.text,
        x: run.x,
        y: run.y,
        w: run.w,
        h: run.h,
        charStart: run.charStart,
        fontFamily: run.fontFamily,
        fontSize: run.fontSize,
      }));
    } catch (error) {
      layoutError = error instanceof Error ? error.message : String(error);
    }

    const caret = document.querySelector('#scroll-content .caret');
    const caretRect = caret?.getBoundingClientRect();
    const trace = window.__issue2214Trace;
    return {
      label: checkpoint,
      atMs: performance.now() - startedAt,
      model: {
        length,
        text,
        tail: text.slice(-40),
        lineCount: lines[0]?.lineCount ?? null,
        lines,
      },
      cursor: { position, rect },
      pagination: {
        pending: input.hasDeferredPaginationPending(),
        pageCount: wasm.pageCount,
      },
      renderState: {
        pendingRaf: canvasView.textEditRefreshRafId !== null,
        pendingPages: Array.from(canvasView.pendingTextEditRefreshes.keys()),
        verificationScheduled: canvasView.textEditStaticLayerVerifyTimers.has(pageIndex),
      },
      layerTree: {
        pageIndex,
        targetText: targetOps.map((op) => op.displayText ?? op.text ?? '').join(''),
        targetOps: targetOps.map((op) => ({
          text: op.displayText ?? op.text ?? '',
          bbox: op.bbox,
          baseline: op.baseline,
          stableSourceKey: op.source?.stableSourceKey ?? null,
          utf16Range: op.source?.utf16Range ?? null,
        })),
      },
      pageTextLayout: {
        targetText: layoutRuns.map((run) => run.text ?? '').join(''),
        runs: layoutRuns,
        error: layoutError,
      },
      caret: caret ? {
        display: getComputedStyle(caret).display,
        left: caret.style.left,
        top: caret.style.top,
        height: caret.style.height,
        clientRect: caretRect ? {
          x: caretRect.x,
          y: caretRect.y,
          width: caretRect.width,
          height: caretRect.height,
        } : null,
      } : null,
      traceEventCount: trace?.events.length ?? 0,
    };
  }, { checkpoint: label, startedAt: timelineStart });
}

async function captureCheckpoint(page, directory, label, timelineStart, clip) {
  const state = await collectState(page, label, timelineStart);
  const screenshotPath = path.join(directory, `${sanitizeLabel(label)}.png`);
  state.screenshot = await captureCompositedCrop(page, screenshotPath, clip);
  return state;
}

async function discoverTransition(page, format, bytes, runNumber) {
  await restoreTrace(page);
  const load = await openDocumentThroughApp(page, format, bytes);
  await moveToTarget(page);
  const initialLineCount = await readLineCount(page);
  assert.equal(initialLineCount, 4, `${format} run ${runNumber}: expected initial four lines`);

  const handlerDurationsMs = [];
  let transitionAt = null;
  for (let inserted = 1; inserted <= MAX_INSERTS; inserted += 1) {
    const startedAt = performance.now();
    await page.keyboard.type('1');
    handlerDurationsMs.push(performance.now() - startedAt);
    const lineCount = await readLineCount(page);
    if (lineCount > initialLineCount) {
      transitionAt = inserted;
      break;
    }
  }
  assert.ok(transitionAt, `${format} run ${runNumber}: no 4→5 line transition within ${MAX_INSERTS} inputs`);
  const model = await page.evaluate(() => {
    const wasm = window.__wasm;
    const input = window.__inputHandler;
    const length = wasm.getCellParagraphLength(0, 0, 2, 2, 5);
    return {
      length,
      textTail: wasm.getTextInCell(0, 0, 2, 2, 5, Math.max(0, length - 40), Math.min(40, length)),
      cursor: input.cursor.getPosition(),
      lineCount: wasm.getLineInfoInCell(0, 0, 2, 2, 5, input.cursor.getPosition().charOffset).lineCount,
      pending: input.hasDeferredPaginationPending(),
    };
  });
  return {
    runNumber,
    inputPath: 'page.keyboard.type',
    transitionAt,
    initialLineCount,
    finalLineCount: model.lineCount,
    handler: summarizeDurations(handlerDurationsMs),
    boundaryHandlerMs: handlerDurationsMs.at(-1),
    model,
    load,
  };
}

async function prepareScenario(page, format, bytes, transitionAt) {
  await restoreTrace(page);
  const load = await openDocumentThroughApp(page, format, bytes);
  await moveToTarget(page);
  const keyboardDurationsMs = await typeKeyboardOnes(page, Math.max(0, transitionAt - 1));
  await waitTwoRafs(page);
  const clip = await resolveCompositedClip(page);
  await installTrace(page);
  return { load, clip, keyboardDurationsMs };
}

async function runTimeline(page, format, bytes, transitionAt) {
  const directory = path.join(OUTPUT_ROOT, format, 'timeline');
  const prepared = await prepareScenario(page, format, bytes, transitionAt);
  const timelineStart = await page.evaluate(() => performance.now());
  const checkpoints = [];
  checkpoints.push(await captureCheckpoint(page, directory, 'before-boundary', timelineStart, prepared.clip));

  const boundaryInput = await dispatchInputSynchronously(page, '1');
  checkpoints.push(await captureCheckpoint(page, directory, 'sync-after-boundary', timelineStart, prepared.clip));
  await waitTwoRafs(page);
  checkpoints.push(await captureCheckpoint(page, directory, 'after-2raf', timelineStart, prepared.clip));
  await delay(page, 100);
  checkpoints.push(await captureCheckpoint(page, directory, 'after-100ms', timelineStart, prepared.clip));
  await delay(page, 750);
  checkpoints.push(await captureCheckpoint(page, directory, 'after-850ms', timelineStart, prepared.clip));
  await delay(page, 750);
  checkpoints.push(await captureCheckpoint(page, directory, 'after-1600ms', timelineStart, prepared.clip));

  const plusOne = await dispatchInputSynchronously(page, '1');
  await waitTwoRafs(page);
  checkpoints.push(await captureCheckpoint(page, directory, 'after-n-plus-1', timelineStart, prepared.clip));
  const plusTwo = await dispatchInputSynchronously(page, '1');
  await waitTwoRafs(page);
  checkpoints.push(await captureCheckpoint(page, directory, 'after-n-plus-2', timelineStart, prepared.clip));

  const trace = await collectTrace(page);
  assert.equal(trace.counts.wasmFlush, 0, `${format}: timeline unexpectedly flushed pagination`);
  return {
    kind: 'timeline',
    format,
    transitionAt,
    load: prepared.load,
    clip: prepared.clip,
    inputDurations: {
      beforeBoundaryKeyboard: summarizeDurations(prepared.keyboardDurationsMs),
      boundarySyncMs: boundaryInput.durationMs,
      plusOneSyncMs: plusOne.durationMs,
      plusTwoSyncMs: plusTwo.durationMs,
    },
    checkpoints,
    trace,
  };
}

async function runFullLayerControl(page, format, bytes, transitionAt) {
  const directory = path.join(OUTPUT_ROOT, format, 'full-layer-control');
  const prepared = await prepareScenario(page, format, bytes, transitionAt);
  const timelineStart = await page.evaluate(() => performance.now());
  const boundaryInput = await dispatchInputSynchronously(page, '1');
  await waitTwoRafs(page);
  const before = await captureCheckpoint(page, directory, 'before-full-layer', timelineStart, prepared.clip);
  const pageIndex = before.cursor.rect?.pageIndex ?? 0;
  await page.evaluate((value) => {
    window.__eventBus.emit('document-page-invalidated', { pageIndex: value, reason: 'unknown' });
  }, pageIndex);
  await waitTwoRafs(page);
  const after = await captureCheckpoint(page, directory, 'after-full-layer', timelineStart, prepared.clip);
  const trace = await collectTrace(page);
  assert.equal(trace.counts.wasmFlush, 0, `${format}: full-layer control flushed pagination`);
  return {
    kind: 'full-layer-control',
    format,
    transitionAt,
    load: prepared.load,
    clip: prepared.clip,
    boundaryInputMs: boundaryInput.durationMs,
    checkpoints: [before, after],
    trace,
  };
}

async function runFlushControl(page, format, bytes, transitionAt) {
  const directory = path.join(OUTPUT_ROOT, format, 'flush-control');
  const prepared = await prepareScenario(page, format, bytes, transitionAt);
  const timelineStart = await page.evaluate(() => performance.now());
  const boundaryInput = await dispatchInputSynchronously(page, '1');
  await waitTwoRafs(page);
  const before = await captureCheckpoint(page, directory, 'before-explicit-flush', timelineStart, prepared.clip);
  const traceBefore = await collectTrace(page);
  assert.equal(traceBefore.counts.wasmFlush, 0, `${format}: flush control had a premature pagination flush`);

  const explicit = await page.evaluate(() => {
    const input = window.__inputHandler;
    const position = input.cursor.getPosition();
    const startedAt = performance.now();
    const flushed = input.flushDeferredPaginationIfNeeded('issue-2214-e2e', false);
    input.cursor.moveTo(position);
    window.__eventBus.emit('document-view-changed');
    return { flushed, durationMs: performance.now() - startedAt };
  });
  assert.equal(explicit.flushed, true, `${format}: explicit pagination control did not flush`);
  await waitTwoRafs(page);
  const after = await captureCheckpoint(page, directory, 'after-explicit-flush', timelineStart, prepared.clip);
  const trace = await collectTrace(page);
  assert.equal(trace.counts.wasmFlush, 1, `${format}: explicit control expected exactly one pagination flush`);
  return {
    kind: 'flush-control',
    format,
    transitionAt,
    load: prepared.load,
    clip: prepared.clip,
    boundaryInputMs: boundaryInput.durationMs,
    explicit,
    checkpoints: [before, after],
    traceBefore,
    trace,
  };
}

function comparePng(actualPath, expectedPath, diffPath) {
  const actual = PNG.sync.read(readFileSync(actualPath));
  const expected = PNG.sync.read(readFileSync(expectedPath));
  if (actual.width !== expected.width || actual.height !== expected.height) {
    return {
      comparable: false,
      actual: { width: actual.width, height: actual.height },
      expected: { width: expected.width, height: expected.height },
    };
  }
  const diff = new PNG({ width: actual.width, height: actual.height });
  const changedPixelCount = pixelmatch(
    actual.data,
    expected.data,
    diff.data,
    actual.width,
    actual.height,
    { threshold: 0.1 },
  );
  let minX = actual.width;
  let minY = actual.height;
  let maxX = -1;
  let maxY = -1;
  let maxChannelDelta = 0;
  for (let y = 0; y < actual.height; y += 1) {
    for (let x = 0; x < actual.width; x += 1) {
      const offset = (y * actual.width + x) * 4;
      let changed = false;
      for (let channel = 0; channel < 4; channel += 1) {
        const delta = Math.abs(actual.data[offset + channel] - expected.data[offset + channel]);
        maxChannelDelta = Math.max(maxChannelDelta, delta);
        if (delta > 0) changed = true;
      }
      if (changed) {
        minX = Math.min(minX, x);
        minY = Math.min(minY, y);
        maxX = Math.max(maxX, x);
        maxY = Math.max(maxY, y);
      }
    }
  }
  mkdirSync(path.dirname(diffPath), { recursive: true });
  writeFileSync(diffPath, PNG.sync.write(diff));
  return {
    comparable: true,
    width: actual.width,
    height: actual.height,
    threshold: 0.1,
    changedPixelCount,
    diffRatio: changedPixelCount / (actual.width * actual.height),
    maxChannelDelta,
    rawDiffBbox: maxX >= 0 ? { minX, minY, maxX, maxY } : null,
    diffPath,
    diffSha256: sha256(readFileSync(diffPath)),
  };
}

function checkpointByLabel(result, label) {
  const checkpoint = result.checkpoints.find((value) => value.label === label);
  assert.ok(checkpoint, `checkpoint not found: ${label}`);
  return checkpoint;
}

function classifyFormat(result) {
  const timelineAtN = checkpointByLabel(result.timeline, 'after-2raf');
  const fullAfter = checkpointByLabel(result.fullLayer, 'after-full-layer');
  const flushAfter = checkpointByLabel(result.flush, 'after-explicit-flush');
  const comparisons = {
    timelineVsFlush: comparePng(
      timelineAtN.screenshot.filePath,
      flushAfter.screenshot.filePath,
      path.join(OUTPUT_ROOT, result.format, 'diff', 'timeline-vs-flush.png'),
    ),
    fullLayerVsFlush: comparePng(
      fullAfter.screenshot.filePath,
      flushAfter.screenshot.filePath,
      path.join(OUTPUT_ROOT, result.format, 'diff', 'full-layer-vs-flush.png'),
    ),
  };

  const modelLatest = timelineAtN.model.text.endsWith('1'.repeat(result.transitionAt));
  const linesLatest = timelineAtN.model.lineCount === 5;
  const treeMatchesFlush = timelineAtN.layerTree.targetText === flushAfter.layerTree.targetText;
  const layoutMatchesFlush = timelineAtN.pageTextLayout.targetText === flushAfter.pageTextLayout.targetText;
  const pixelStale = comparisons.timelineVsFlush.comparable
    && comparisons.timelineVsFlush.changedPixelCount > 0;
  const fullLayerRecovers = comparisons.fullLayerVsFlush.comparable
    && comparisons.fullLayerVsFlush.changedPixelCount === 0;

  let cause = 'existing-green';
  if (!modelLatest || !linesLatest) cause = 'document-core-mutation-or-reflow';
  else if (pixelStale && fullLayerRecovers) cause = 'static-reuse-or-page-local-render-context';
  else if (pixelStale && !layoutMatchesFlush && !fullLayerRecovers) cause = 'stale-layout-input';
  else if (pixelStale && !treeMatchesFlush && !fullLayerRecovers) cause = 'stale-cached-page-tree';
  else if (pixelStale && !fullLayerRecovers) cause = 'explicit-pagination-only-or-multiple-boundaries';

  return {
    verdict: pixelStale ? 'RED' : 'GREEN',
    cause,
    modelLatest,
    linesLatest,
    treeMatchesFlush,
    layoutMatchesFlush,
    fullLayerRecovers,
    comparisons,
  };
}

async function runFormat(page, format, bytes, discoveryRuns) {
  console.log(`\n[${format.toUpperCase()}] keyboard transition discovery (${discoveryRuns} runs)`);
  const discoveries = [];
  for (let run = 1; run <= discoveryRuns; run += 1) {
    const discovery = await discoverTransition(page, format, bytes, run);
    discoveries.push(discovery);
    console.log(`  run ${run}: N=${discovery.transitionAt}, boundary=${discovery.boundaryHandlerMs.toFixed(2)}ms`);
  }
  const transitionValues = new Set(discoveries.map((value) => value.transitionAt));
  assert.equal(transitionValues.size, 1, `${format}: keyboard transition N is not deterministic`);
  const transitionAt = discoveries[0].transitionAt;

  console.log(`  timeline at N=${transitionAt}`);
  const timeline = await runTimeline(page, format, bytes, transitionAt);
  console.log('  independent page-only full-layer control');
  const fullLayer = await runFullLayerControl(page, format, bytes, transitionAt);
  console.log('  independent explicit-pagination control');
  const flush = await runFlushControl(page, format, bytes, transitionAt);

  const result = {
    format,
    transitionAt,
    discoveries,
    timeline,
    fullLayer,
    flush,
  };
  result.classification = classifyFormat(result);
  writeJson(path.join(OUTPUT_ROOT, `${format}-diagnostic.json`), result);
  console.log(`  verdict=${result.classification.verdict}, cause=${result.classification.cause}`);
  return result;
}

async function main() {
  assert.ok(process.argv.includes('--diagnose'), 'Stage 1 runner requires --diagnose');
  const formats = cliValue('formats', 'hwp,hwpx')
    .split(',')
    .map((value) => value.trim().toLowerCase())
    .filter(Boolean);
  const discoveryRuns = Number(cliValue('runs', '3'));
  assert.ok(Number.isInteger(discoveryRuns) && discoveryRuns > 0, '--runs must be a positive integer');
  for (const format of formats) assert.ok(SAMPLES[format], `unsupported format: ${format}`);

  mkdirSync(OUTPUT_ROOT, { recursive: true });
  const fixtures = Object.fromEntries(formats.map((format) => {
    const bytes = readFileSync(SAMPLES[format]);
    return [format, {
      path: SAMPLES[format],
      bytes,
      size: bytes.length,
      sha256: sha256(bytes),
    }];
  }));

  const browser = await launchBrowser();
  const page = await createPage(browser, 1280, 900);
  page.on('console', (message) => {
    if (message.type() === 'error' || message.type() === 'warning') {
      console.log(`  [browser:${message.type()}] ${message.text()}`);
    }
  });
  page.on('pageerror', (error) => console.log(`  [browser:pageerror] ${error.message}`));

  const startedAt = new Date().toISOString();
  const results = [];
  try {
    await loadApp(page);
    for (const format of formats) {
      results.push(await runFormat(page, format, fixtures[format].bytes, discoveryRuns));
    }
  } finally {
    await restoreTrace(page).catch(() => {});
    await closePage(page).catch(() => {});
    await closeBrowser(browser).catch(() => {});
  }

  const summary = {
    issue: 2214,
    mode: 'diagnose',
    startedAt,
    finishedAt: new Date().toISOString(),
    environment: {
      viteUrl: process.env.VITE_URL ?? 'http://localhost:7700',
      chromePath: process.env.CHROME_PATH ?? process.env.PUPPETEER_EXECUTABLE_PATH ?? null,
      viewport: { width: 1280, height: 900, deviceScaleFactor: 1 },
      discoveryRuns,
    },
    fixtures: Object.fromEntries(Object.entries(fixtures).map(([format, value]) => [format, {
      path: value.path,
      size: value.size,
      sha256: value.sha256,
    }])),
    formats: results.map((result) => ({
      format: result.format,
      transitionAt: result.transitionAt,
      discoveryNs: result.discoveries.map((value) => value.transitionAt),
      classification: result.classification,
    })),
  };
  writeJson(path.join(OUTPUT_ROOT, 'summary.json'), summary);
  console.log(`\nIssue #2214 diagnostic written to ${OUTPUT_ROOT}`);
  console.log(JSON.stringify(summary.formats, null, 2));
}

main().catch((error) => {
  console.error(error);
  process.exitCode = 1;
});
