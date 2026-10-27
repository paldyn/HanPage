import test from 'node:test';
import assert from 'node:assert/strict';
import { readFileSync } from 'node:fs';

import {
  desktopConvertedPdfSaveFormat,
  forgetDesktopConvertedPdfSave,
  rememberDesktopConvertedPdfSave,
} from '../src/command/desktop-converted-save.ts';

test('데스크톱 PDF 변환 저장 형식은 같은 문서 인스턴스에서만 기억한다', () => {
  forgetDesktopConvertedPdfSave();
  assert.equal(desktopConvertedPdfSaveFormat(3), null, '첫 저장은 형식을 물어야 한다');
  rememberDesktopConvertedPdfSave(3, '보고서.hwp', 'hwpx');
  assert.equal(desktopConvertedPdfSaveFormat(3), 'hwp', '실제 저장 파일 확장자를 고른 형식보다 우선한다');
  assert.equal(desktopConvertedPdfSaveFormat(4), null, '다른 문서를 열면 다시 묻는다');
  rememberDesktopConvertedPdfSave(4, '이름', 'hwpx');
  assert.equal(desktopConvertedPdfSaveFormat(4), 'hwpx', '확장자가 없으면 고른 형식을 쓴다');
  rememberDesktopConvertedPdfSave(undefined, '보고서.hwp', 'hwp');
  assert.equal(desktopConvertedPdfSaveFormat(undefined), null, '세대를 모르면 기억하지 않는다');
  forgetDesktopConvertedPdfSave();
});

test('file.ts 는 데스크톱 PDF 저장에서만 변환 형식을 기억하고 다음 저장에 재사용한다', () => {
  const source = readFileSync(new URL('../src/command/commands/file.ts', import.meta.url), 'utf8');
  assert.match(source,
    /sourceFormat === 'pdf' && result\.handle === null && getDesktopSaveHandler\(\)\) \{\s*rememberDesktopConvertedPdfSave\(services\.wasm\.documentGeneration, result\.fileName, saveFormat\)/);
  assert.match(source,
    /sourceFormat === 'pdf' && getDesktopSaveHandler\(\)\s*\?\s*desktopConvertedPdfSaveFormat\(services\.wasm\.documentGeneration\)/);
  const resolveAt = source.indexOf('let target = resolveSaveTarget(');
  const reuseAt = source.indexOf('desktopConvertedPdfSaveFormat(services.wasm.documentGeneration)');
  const choiceAt = source.indexOf('requiresSaveFormatChoice(target, hmlEnabled)');
  assert.ok(resolveAt > 0 && resolveAt < reuseAt && reuseAt < choiceAt, '형식 선택 대화상자보다 먼저 기억한 형식을 적용한다');
});
