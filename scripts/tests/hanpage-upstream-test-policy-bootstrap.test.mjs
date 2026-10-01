import assert from 'node:assert/strict';
import { execFileSync } from 'node:child_process';
import { existsSync, mkdirSync, mkdtempSync, readFileSync, rmSync, writeFileSync } from 'node:fs';
import os from 'node:os';
import path from 'node:path';
import test from 'node:test';
import { fileURLToPath } from 'node:url';

import {
  BOOTSTRAP,
  EQUIVALENT_PATHS,
  selectBootstrapBaseline,
  verifyBootstrap,
  verifyBootstrapEquivalentTrees,
  verifyBootstrapIdentity,
} from '../hanpage-upstream-test-policy-bootstrap.mjs';

const metadata = {
  eventName: 'pull_request',
  repository: 'paldyn/HanPage',
  baseRepository: 'paldyn/HanPage',
  baseBranch: 'devel',
  baseSha: 'cecaf1bbfec9a10778484d30ebb747280e7a02af',
  headRepository: 'paldyn/HanPage',
  headBranch: 'codex/upstream-sync-20261001',
  headSha: 'a'.repeat(40),
};
const upstreamCargo = '[package]\nname = "rhwp"\nrepository = "https://github.com/edwardkim/rhwp"\n';
const candidateCargo = upstreamCargo.replace('edwardkim/rhwp', 'paldyn/HanPage');

test('이번 HanPage 통합의 exact base와 같은 저장소 branch만 bootstrap을 선택한다', () => {
  assert.equal(selectBootstrapBaseline(metadata), '02530b9ed567a44663edb26c65fb565c4a79f00d');
  for (const [field, value] of [
    ['eventName', 'workflow_dispatch'],
    ['repository', 'edwardkim/rhwp'],
    ['baseRepository', 'other/HanPage'],
    ['baseBranch', 'main'],
    ['baseSha', 'b'.repeat(40)],
    ['headRepository', 'fork/HanPage'],
    ['headBranch', 'devel'],
  ]) {
    assert.equal(selectBootstrapBaseline({ ...metadata, [field]: value }), null, field);
  }
  assert.throws(() => selectBootstrapBaseline({ ...metadata, headSha: '' }), /exact PR head SHA/);
});

test('비대상 PR은 Git 조회나 fetch도 하지 않고 기존 base 검사를 유지한다', () => {
  assert.equal(verifyBootstrap('.', { ...metadata, baseSha: 'b'.repeat(40) }, () => {
    assert.fail('ordinary PR must not execute bootstrap Git commands');
  }), null);
});

test('checkout은 exact head 또는 요청한 base/head의 synthetic merge이어야 한다', () => {
  verifyBootstrapIdentity(metadata, metadata.headSha, []);
  verifyBootstrapIdentity(metadata, 'c'.repeat(40), [BOOTSTRAP.baseSha, metadata.headSha]);
  for (const parents of [[], [metadata.headSha], ['b'.repeat(40), metadata.headSha], [BOOTSTRAP.baseSha, 'b'.repeat(40)]]) {
    assert.throws(() => verifyBootstrapIdentity(metadata, 'c'.repeat(40), parents), /exact PR head/);
  }
});

test('동일 upstream source만 허용하며 Rust·test·policy 변경을 grandfather하지 않는다', () => {
  assert.deepEqual(EQUIVALENT_PATHS, [
    'src', 'crates', 'tests', 'Cargo.lock', 'rust-toolchain.toml', '.cargo',
    '.config/nextest.toml', 'scripts/rust-test-suite-manifest.mjs',
    'scripts/rust-unit-test-tiers.mjs', 'scripts/rust-test-policy-base.mjs',
  ]);
  verifyBootstrapEquivalentTrees([], upstreamCargo, candidateCargo);
  for (const changedPath of [
    'src/lib.rs', 'crates/leaf/src/lib.rs', 'tests/cases/new.rs',
    'tests/suites/unit-test-tier-policy.json', 'Cargo.lock',
    'scripts/rust-test-suite-manifest.mjs',
  ]) {
    assert.throws(() => verifyBootstrapEquivalentTrees([changedPath], upstreamCargo, candidateCargo), /pinned upstream policy inputs/);
  }
  assert.throws(() => verifyBootstrapEquivalentTrees([], upstreamCargo, `${candidateCargo}autotests = false\n`), /only the package repository URL/);
  assert.throws(() => verifyBootstrapEquivalentTrees([], upstreamCargo, upstreamCargo), /only the package repository URL/);
});

test('Git 검증은 exact upstream SHA와 HEAD의 tree를 대조한다', () => {
  const calls = [];
  let fetched = false;
  const baseline = verifyBootstrap('.', metadata, (args) => {
    calls.push(args);
    if (args[0] === 'rev-parse') return `${args[1] === 'HEAD' ? metadata.headSha : BOOTSTRAP.upstreamSha}\n`;
    if (args[0] === 'cat-file' && !fetched) throw new Error('missing upstream object');
    if (args[0] === 'fetch') fetched = true;
    if (args[0] === 'show' && args[1] === '-s') return `${BOOTSTRAP.baseSha}\n`;
    if (args[0] === 'show') return args[1] === 'HEAD:Cargo.toml' ? candidateCargo : upstreamCargo;
    return '';
  });
  assert.equal(baseline, BOOTSTRAP.upstreamSha);
  assert.deepEqual(calls.find((args) => args[0] === 'fetch'), [
    'fetch', '--no-tags', '--no-write-fetch-head', '--depth=1',
    'https://github.com/edwardkim/rhwp.git', '02530b9ed567a44663edb26c65fb565c4a79f00d',
  ]);
  assert.deepEqual(calls.find((args) => args[0] === 'diff'), [
    'diff', '--name-only', '-z', BOOTSTRAP.upstreamSha, 'HEAD', '--', ...EQUIVALENT_PATHS,
  ]);
});

test('CI는 입증된 pinned worktree에서 원래 manifest·unit 정책을 모두 실행한다', () => {
  const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '../..');
  const workflow = readFileSync(path.join(root, '.github/workflows/ci.yml'), 'utf8');
  assert.match(workflow, /node scripts\/hanpage-upstream-test-policy-bootstrap\.mjs/);
  assert.match(workflow, /policy_base_ref="\$RHWP_TEST_POLICY_BASE_SHA"\n\s+policy_check_root="\$PWD"/);
  assert.match(workflow, /node "\$bootstrap_policy_root\/scripts\/rust-test-suite-manifest\.mjs" --prepare/);
  assert.match(workflow, /git worktree add --detach --no-checkout "\$bootstrap_policy_root" "\$bootstrap_base_ref"/);
  assert.match(workflow, /git -C "\$bootstrap_policy_root" sparse-checkout set --cone src crates tests scripts \.config/);
  assert.match(workflow, /git -C "\$bootstrap_policy_root" checkout --detach "\$bootstrap_base_ref"/);
  assert.match(workflow, /node "\$policy_check_root\/scripts\/rust-test-suite-manifest\.mjs" --check/);
  assert.match(workflow, /node "\$policy_check_root\/scripts\/rust-unit-test-tiers\.mjs" --check/);
  assert.match(workflow, /policy_base_ref="\$bootstrap_base_ref"/);
  assert.doesNotMatch(workflow, /policy_base_ref="HEAD"/);
});

test('sparse worktree는 정책 입력과 root 파일을 채우고 큰 자산 폴더를 checkout하지 않는다', (t) => {
  const temporaryRoot = mkdtempSync(path.join(os.tmpdir(), 'hanpage-policy-sparse-'));
  const repository = path.join(temporaryRoot, 'repository');
  const worktree = path.join(temporaryRoot, 'policy');
  mkdirSync(repository);
  const git = (args, cwd = repository) => execFileSync('git', args, {
    cwd,
    encoding: 'utf8',
    stdio: ['ignore', 'pipe', 'pipe'],
    env: { ...process.env, GIT_LFS_SKIP_SMUDGE: '1' },
  });
  let registered = false;
  t.after(() => {
    try {
      if (registered) git(['worktree', 'remove', '--force', worktree]);
    } finally {
      rmSync(temporaryRoot, { recursive: true, force: true });
    }
  });
  git(['init']);
  const required = [
    'src/lib.rs', 'crates/leaf/src/lib.rs', 'tests/cases/example.rs',
    'tests/suites/suite-policy.json', 'tests/suites/unit-test-tier-policy.json',
    'scripts/rust-test-suite-manifest.mjs', 'scripts/rust-unit-test-tiers.mjs',
    '.config/nextest.toml', 'Cargo.toml', 'Cargo.lock', 'rust-toolchain.toml',
  ];
  const omitted = ['samples/large.hwp', 'pdf/large.pdf', 'mydocs/README.md', 'rhwp-studio/package.json'];
  for (const filename of [...required, ...omitted]) {
    const destination = path.join(repository, filename);
    mkdirSync(path.dirname(destination), { recursive: true });
    writeFileSync(destination, `fixture: ${filename}\n`);
  }
  git(['add', '.']);
  git(['-c', 'user.name=Fixture', '-c', 'user.email=fixture@example.invalid', 'commit', '-m', 'sparse policy fixture']);
  const baseline = git(['rev-parse', 'HEAD']).trim();
  git(['worktree', 'add', '--detach', '--no-checkout', worktree, baseline]);
  registered = true;
  assert.equal(existsSync(path.join(worktree, 'src/lib.rs')), false);
  git(['sparse-checkout', 'set', '--cone', 'src', 'crates', 'tests', 'scripts', '.config'], worktree);
  git(['checkout', '--detach', baseline], worktree);
  assert.equal(git(['rev-parse', 'HEAD'], worktree).trim(), baseline);
  for (const filename of required) {
    assert.equal(readFileSync(path.join(worktree, filename), 'utf8'), `fixture: ${filename}\n`, filename);
  }
  for (const filename of omitted) {
    assert.equal(existsSync(path.join(worktree, filename)), false, filename);
  }
});
