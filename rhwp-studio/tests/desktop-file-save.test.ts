import test from 'node:test';
import assert from 'node:assert/strict';
import { saveDocumentToDesktop } from '../src/command/desktop-file-save.ts';
import { persistWithContentLoss, type ContentLossReport } from '../src/core/export-content-loss.ts';

const report: ContentLossReport = {
  schemaVersion: 1,
  outputFormat: 'hwpx',
  count: 1,
  losses: [{ code: 'binaryContentEmptied', subject: 'binaryData', path: 'BinData/image7.png', reason: 'resourceReadFailedOrLimitExceeded' }],
};

test('native 저장은 같은 artifact 바이트를 쓴 뒤에만 상태와 내용 손실 알림을 갱신한다', async () => {
  const events: string[] = [];
  const bytes = new Uint8Array([80, 75, 3, 4, 0, 255]);
  const result = await persistWithContentLoss(
    report,
    () => saveDocumentToDesktop({
      blob: new Blob([bytes]), suggestedName: '문서.hwpx', forceSaveAs: true,
      save: async (request) => {
        assert.deepEqual(request.bytes, bytes);
        assert.equal(request.suggestedName, '문서.hwpx');
        assert.equal(request.saveAs, true);
        events.push('native-write');
        return { status: 'saved', fileName: '다른이름.hwpx' };
      },
    }),
    (saved) => saved !== 'cancelled' && saved.method !== 'fallback',
    () => { events.push('clean-and-refresh'); },
    () => { events.push('loss-notice'); },
  );
  assert.deepEqual(result, { method: 'save-picker', handle: null, fileName: '다른이름.hwpx' });
  assert.deepEqual(events, ['native-write', 'clean-and-refresh', 'loss-notice']);
});

test('native 저장 취소는 dirty·보호 상태와 내용 손실 알림을 바꾸지 않는다', async () => {
  const events: string[] = [];
  const result = await persistWithContentLoss(
    report,
    () => saveDocumentToDesktop({ blob: new Blob(['protected']), suggestedName: '문서.hwp', forceSaveAs: false,
      save: async () => ({ status: 'cancelled' }), }),
    (saved) => saved !== 'cancelled' && saved.method !== 'fallback',
    () => { events.push('clean'); },
    () => { events.push('notify'); },
  );
  assert.equal(result, 'cancelled');
  assert.deepEqual(events, []);
});

test('native write 실패는 성공 처리하거나 웹 다운로드로 우회하지 않는다', async () => {
  const events: string[] = [];
  await assert.rejects(persistWithContentLoss(
    report,
    () => saveDocumentToDesktop({ blob: new Blob(['protected']), suggestedName: '문서.hwp', forceSaveAs: false,
      save: async () => ({ status: 'failed', message: 'disk full' }), }),
    () => true,
    () => { events.push('clean'); },
    () => { events.push('notify'); },
  ), /disk full/);
  assert.deepEqual(events, []);
});
