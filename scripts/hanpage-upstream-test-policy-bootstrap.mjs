#!/usr/bin/env node

import { execFileSync } from 'node:child_process';
import { fileURLToPath } from 'node:url';
import path from 'node:path';

export const BOOTSTRAP = Object.freeze({
  repository: 'paldyn/HanPage',
  baseBranch: 'devel',
  baseSha: 'cecaf1bbfec9a10778484d30ebb747280e7a02af',
  headBranch: 'codex/upstream-sync-20261001',
  upstreamSha: '02530b9ed567a44663edb26c65fb565c4a79f00d',
  upstreamUrl: 'https://github.com/edwardkim/rhwp.git',
});

// These are the candidate inputs consumed by the unchanged upstream policy checks.
export const EQUIVALENT_PATHS = Object.freeze([
  'src',
  'crates',
  'tests',
  'Cargo.lock',
  'rust-toolchain.toml',
  '.cargo',
  '.config/nextest.toml',
  'scripts/rust-test-suite-manifest.mjs',
  'scripts/rust-unit-test-tiers.mjs',
  'scripts/rust-test-policy-base.mjs',
]);

export function selectBootstrapBaseline(metadata) {
  if (
    metadata.eventName !== 'pull_request' ||
    metadata.repository !== BOOTSTRAP.repository ||
    metadata.baseRepository !== BOOTSTRAP.repository ||
    metadata.headRepository !== BOOTSTRAP.repository ||
    metadata.baseBranch !== BOOTSTRAP.baseBranch ||
    metadata.baseSha !== BOOTSTRAP.baseSha ||
    metadata.headBranch !== BOOTSTRAP.headBranch
  ) {
    return null;
  }
  if (!/^[0-9a-f]{40}$/.test(metadata.headSha ?? '')) {
    throw new Error('HanPage bootstrap requires an exact PR head SHA.');
  }
  return BOOTSTRAP.upstreamSha;
}

export function verifyBootstrapIdentity(metadata, checkoutSha, parents) {
  if (
    checkoutSha !== metadata.headSha &&
    (parents.length !== 2 ||
      parents[0] !== BOOTSTRAP.baseSha ||
      parents[1] !== metadata.headSha)
  ) {
    throw new Error('HanPage bootstrap checkout is not the exact PR head or base/head merge.');
  }
}

export function verifyBootstrapEquivalentTrees(changedPaths, upstreamCargo, candidateCargo) {
  if (changedPaths.length > 0) {
    throw new Error(
      'HanPage bootstrap differs from the pinned upstream policy inputs: ' +
        changedPaths.join(', '),
    );
  }
  const upstreamRepository = 'repository = "https://github.com/edwardkim/rhwp"';
  const candidateRepository = 'repository = "https://github.com/paldyn/HanPage"';
  const repositoryLines = (text) => text.match(/^repository = .*$/gm) ?? [];
  if (
    repositoryLines(upstreamCargo).filter((line) => line === upstreamRepository).length !== 1 ||
    repositoryLines(candidateCargo).filter((line) => line === candidateRepository).length !== 1 ||
    upstreamCargo.replace(upstreamRepository, candidateRepository) !== candidateCargo
  ) {
    throw new Error('HanPage bootstrap permits only the package repository URL change in Cargo.toml.');
  }
}

export function verifyBootstrap(root, metadata, runGit = null) {
  const baseline = selectBootstrapBaseline(metadata);
  if (baseline === null) return null;
  const git = runGit ?? ((args) => execFileSync('git', args, {
    cwd: root,
    encoding: 'utf8',
    maxBuffer: 32 * 1024 * 1024,
    stdio: ['ignore', 'pipe', 'pipe'],
  }));
  const checkoutSha = git(['rev-parse', 'HEAD']).trim();
  const parents = git(['show', '-s', '--format=%P', 'HEAD']).trim().split(/\s+/).filter(Boolean);
  verifyBootstrapIdentity(metadata, checkoutSha, parents);
  try {
    git(['cat-file', '-e', `${baseline}^{commit}`]);
  } catch {
    git([
      'fetch', '--no-tags', '--no-write-fetch-head', '--depth=1',
      BOOTSTRAP.upstreamUrl, baseline,
    ]);
  }
  if (git(['rev-parse', `${baseline}^{commit}`]).trim() !== baseline) {
    throw new Error('HanPage bootstrap upstream commit identity changed.');
  }
  const changedPaths = git([
    'diff', '--name-only', '-z', baseline, 'HEAD', '--', ...EQUIVALENT_PATHS,
  ]).split('\0').filter(Boolean);
  verifyBootstrapEquivalentTrees(
    changedPaths,
    git(['show', `${baseline}:Cargo.toml`]),
    git(['show', 'HEAD:Cargo.toml']),
  );
  return baseline;
}

if (process.argv[1] && path.resolve(process.argv[1]) === fileURLToPath(import.meta.url)) {
  const metadata = {
    eventName: process.env.RHWP_TEST_POLICY_EVENT_NAME,
    repository: process.env.RHWP_REPOSITORY,
    baseRepository: process.env.RHWP_TEST_POLICY_BASE_REPOSITORY,
    baseBranch: process.env.RHWP_TEST_POLICY_BASE_BRANCH,
    baseSha: process.env.RHWP_TEST_POLICY_BASE_SHA,
    headRepository: process.env.RHWP_TEST_POLICY_HEAD_REPOSITORY,
    headBranch: process.env.RHWP_TEST_POLICY_HEAD_BRANCH,
    headSha: process.env.RHWP_TEST_POLICY_HEAD_SHA,
  };
  const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '..');
  const baseline = verifyBootstrap(root, metadata);
  process.stdout.write(baseline === null ? '' : `${baseline}\n`);
}
