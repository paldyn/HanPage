import test from 'node:test';
import assert from 'node:assert/strict';

import {
  normalizeHmlSaveState,
  readHmlSaveContext,
  resolveHmlSaveCapability,
} from '../src/core/hml-save-capability.ts';

test('HML 저장은 명시적 savable 메타데이터와 exporter가 모두 있을 때만 활성화된다', () => {
  assert.deepEqual(resolveHmlSaveCapability({ hmlSavable: true, saveBlockers: [] }, true), {
    hmlEnabled: true,
    diagnostic: null,
  });
  assert.equal(resolveHmlSaveCapability({ hmlSavable: true, saveBlockers: [] }, false).hmlEnabled, false);
  assert.equal(resolveHmlSaveCapability({ hmlSavable: false, saveBlockers: [] }, true).hmlEnabled, false);
  assert.equal(resolveHmlSaveCapability({}, true).hmlEnabled, false);
  assert.equal(resolveHmlSaveCapability(null, true).hmlEnabled, false);
});

test('누락되거나 잘못된 HML savability 필드는 저장 불가로 정규화된다', () => {
  assert.deepEqual(normalizeHmlSaveState({}), {
    hmlSavable: false,
    saveBlockers: [],
  });
  assert.deepEqual(normalizeHmlSaveState({ hmlSavable: true, saveBlockers: 'bad' }), {
    hmlSavable: false,
    saveBlockers: [],
  });
  assert.deepEqual(normalizeHmlSaveState({
    hmlSavable: true,
    saveBlockers: [{ code: 7, xmlPath: '/HWPML', message: 'bad' }],
  }), {
    hmlSavable: false,
    saveBlockers: [],
  });
  assert.deepEqual(normalizeHmlSaveState({
    hmlSavable: true,
    saveBlockers: [{ code: 'LOSS', xmlPath: '/HWPML', message: 'blocked' }],
  }), {
    hmlSavable: false,
    saveBlockers: [{ code: 'LOSS', xmlPath: '/HWPML', message: 'blocked' }],
  });
  assert.equal(normalizeHmlSaveState(null), null);
});

test('HML 저장 비활성 사유는 capability와 metadata 문제를 구분한다', () => {
  assert.match(resolveHmlSaveCapability({ hmlSavable: true, saveBlockers: [] }, false).diagnostic ?? '', /WASM/);
  assert.match(resolveHmlSaveCapability(null, true).diagnostic ?? '', /저장 정보를 확인/);
  assert.match(resolveHmlSaveCapability({ hmlSavable: false, saveBlockers: [] }, true).diagnostic ?? '', /보존할 수 없는/);
});

test('metadata 또는 exporter 조회가 던져도 HML 저장은 진단 정보와 함께 fail-closed된다', () => {
  const metadataFailure = readHmlSaveContext(
    () => { throw new Error('metadata unavailable'); },
    () => true,
  );
  assert.deepEqual(metadataFailure, { metadata: null, exporterAvailable: true });
  assert.equal(resolveHmlSaveCapability(
    metadataFailure.metadata,
    metadataFailure.exporterAvailable,
  ).hmlEnabled, false);

  const exporterFailure = readHmlSaveContext(
    () => ({
      hmlSavable: false,
      saveBlockers: [{ code: 'Loss', xmlPath: '/HWPML/BODY', message: 'blocked' }],
    }),
    () => { throw new Error('exporter unavailable'); },
  );
  assert.equal(exporterFailure.exporterAvailable, false);
  assert.deepEqual(exporterFailure.metadata?.saveBlockers, [
    { code: 'Loss', xmlPath: '/HWPML/BODY', message: 'blocked' },
  ]);
});
