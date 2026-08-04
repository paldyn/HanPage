import test from 'node:test';
import assert from 'node:assert/strict';

import {
  formatMb,
  updateReadyMessage,
  updateStatusMessage,
} from '../src/ui/update-notice-text.ts';

test('formatMb — 소수 1자리 MB 표기', () => {
  assert.equal(formatMb(0), '0.0 MB');
  assert.equal(formatMb(1024 * 1024), '1.0 MB');
  assert.equal(formatMb(41.24 * 1024 * 1024), '41.2 MB');
});

test('updateStatusMessage — 다운로드 진행률(총 길이 있음)은 퍼센트로 안내', () => {
  const msg = updateStatusMessage({
    state: 'downloading',
    downloaded: 41 * 1024 * 1024,
    total: 100 * 1024 * 1024,
  });
  assert.ok(msg?.includes('41%'), msg ?? '(null)');
  assert.ok(msg?.includes('준비되면 알려드릴게요'));
});

test('updateStatusMessage — 총 길이 미상(불확정)이면 받은 용량만 안내', () => {
  const msg = updateStatusMessage({
    state: 'downloading',
    downloaded: 5 * 1024 * 1024,
    total: null,
  });
  assert.ok(msg?.includes('5.0 MB'), msg ?? '(null)');
  assert.ok(!msg?.includes('%'), '불확정 상태에서 퍼센트를 표시하면 안 된다');
});

test('updateStatusMessage — 최신/오류는 안내, ready/idle 은 전용 처리(null)', () => {
  assert.ok(updateStatusMessage({ state: 'upToDate', version: '0.8.2' })?.includes('0.8.2'));
  assert.ok(updateStatusMessage({ state: 'error', message: '네트워크' })?.includes('네트워크'));
  assert.equal(updateStatusMessage({ state: 'ready', version: '0.8.3' }), null);
  assert.equal(updateStatusMessage({ state: 'idle' }), null);
});

test('updateReadyMessage — 플랫폼별 안내 문구', () => {
  const info = { version: '0.8.3', currentVersion: '0.8.2', notes: null };
  const mac = updateReadyMessage(info, false);
  assert.ok(mac.includes('0.8.3') && mac.includes('현재 0.8.2'));
  assert.ok(mac.includes('다시 시작하면'));

  const win = updateReadyMessage(info, true);
  assert.ok(win.includes('설치 프로그램이 실행됩니다'));
});

test('updateReadyMessage — currentVersion 이 없으면 괄호를 생략한다', () => {
  const msg = updateReadyMessage({ version: '0.8.3', currentVersion: '', notes: null }, false);
  assert.ok(msg.includes('0.8.3'));
  assert.ok(!msg.includes('(현재'), msg);
});
