#!/usr/bin/env node
// HanPage Desktop 릴리스 자산 감사. 초안 또는 공개 릴리스에서 받은 자산 6개를 실행하지 않고 검사한다.
//
//   node HanPage-Desktop/scripts/release-audit.mjs <자산 폴더> <버전> [--release <ID>] [--rehearsal <N>] [--prev-tag <태그>] [--conf <파일>] [--prev-conf <파일>]
//
// - --release 를 주면 GitHub 릴리스(gh api)의 자산 목록·digest 가 감사한 파일과 같은지(공개본 = 감사본), 대상 commit 이 로컬 태그와
//   같은지 대조한다. 그래서 --conf 를 줘도 태그를 받아 둬야 한다.
// - --rehearsal 은 태그 hanpage-desktop-v<버전> 대신 리허설 태그 hanpage-desktop-v<버전>-rehearsal.<N> 을 감사한다. 태그 이름을
//   직접 받지 않는 것은 URL 검사와 릴리스 태그 대조가 태그와 자산 버전이 같다는 것까지 입증하기 때문이다.
// - updater 공개키는 기본으로 그 태그의 tauri.conf.json 에서 읽고(git), 출처와 SHA-256 을 남긴다.
// - 이전 릴리스(기본: 그보다 낮은 가장 높은 hanpage-desktop-v* 태그)와 공개키가 같아야 기존 설치본이 업데이트를 받아들인다.
// - updater 서명은 앱의 updater(tauri-plugin-updater·minisign-verify)처럼 엄격하게 디코딩한다. latest.json 의 서명 문자열을
//   그대로 쓰므로 .sig 와 바이트 단위로 같아야 한다.
// - macOS 도구(plutil·lipo·codesign·spctl·xcrun stapler·hdiutil)를 쓴다. DMG 는 읽기 전용으로 마운트한다.
// - 서명 인증서 만료일을 signingCertNotAfter 에 남기고, 90일 안이거나 읽지 못하면 warnings 와 stderr 에 적는다(실패로 치지 않는다).
// 결과 JSON 을 stdout 에 쓰고, 실패한 검사가 있으면 종료 코드 1, 중단되면 130 으로 끝난다.
import { readFileSync, readdirSync, readlinkSync, lstatSync, mkdtempSync, mkdirSync, rmSync, copyFileSync, constants as fsc } from 'node:fs';
import { createHash, createPublicKey, verify } from 'node:crypto';
import { execFileSync, spawnSync } from 'node:child_process';
import { basename, dirname, join, resolve } from 'node:path';
import { tmpdir } from 'node:os';
import { fileURLToPath } from 'node:url';
import { parseArgs } from 'node:util';

const TAG_PREFIX = 'hanpage-desktop-v';
const DOWNLOAD_PREFIX = 'https://github.com/paldyn/HanPage/releases/download/';
const CONF_PATH = 'HanPage-Desktop/src-tauri/tauri.conf.json';
const TEAM_ID = '8L78W6D8XF';
const BUNDLE_ID = 'com.paldyn.hanpage';
const repoRoot = resolve(dirname(fileURLToPath(import.meta.url)), '..', '..');

// 오타·`--release=ID`·값 없는 옵션이 조용히 무시되면 대조를 건너뛴 채 통과하므로 모르는 옵션과 남는 인자는 거부한다.
const usage = () => {
  console.error('usage: release-audit.mjs <assetDir> <version> [--release <id>] [--rehearsal <n>] [--prev-tag <tag>] [--conf <file>] [--prev-conf <file>]');
  process.exit(2);
};
let cli;
try {
  cli = parseArgs({
    allowPositionals: true, strict: true,
    options: { release: { type: 'string' }, rehearsal: { type: 'string' }, 'prev-tag': { type: 'string' }, conf: { type: 'string' }, 'prev-conf': { type: 'string' } },
  });
} catch (e) {
  console.error(e.message);
  usage();
}
const [dir, version] = cli.positionals;
const opts = { release: cli.values.release, rehearsal: cli.values.rehearsal, prevTag: cli.values['prev-tag'], conf: cli.values.conf, prevConf: cli.values['prev-conf'] };
if (cli.positionals.length !== 2 || !/^\d+\.\d+\.\d+$/.test(version) || (opts.release !== undefined && !/^\d+$/.test(opts.release))
  || (opts.rehearsal !== undefined && !/^[1-9]\d*$/.test(opts.rehearsal)) || Object.values(opts).some((v) => v === '')) usage();
// 동기 실행 중에는 신호 처리기가 돌지 않는다. 처리기는 Node 가 신호로 바로 죽어 finally(마운트 해제·임시 폴더 정리)를 건너뛰지
// 않게 할 뿐이다. 터미널 Ctrl-C 는 같은 프로세스 그룹의 자식에도 간다. 자식이 신호로 끝나면 남은 검사를 건너뛰고 130 으로
// 끝내며, 신호를 받아 스스로 끝나는 도구(hdiutil 등)는 그 검사가 실패로 남는다. 어느 쪽이든 정리는 한다.
for (const signal of ['SIGINT', 'SIGTERM']) process.on(signal, () => {});
let interrupted = null;
const tag = `${TAG_PREFIX}${version}${opts.rehearsal ? `-rehearsal.${opts.rehearsal}` : ''}`;
const out = { version, tag, release: null, signingCertNotAfter: null, warnings: [], checks: [] };
const check = (name, ok, detail = '') => { out.checks.push({ name, ok: !!ok, detail: String(detail) }); };
const attempt = (name, fn) => {
  if (interrupted) return;
  try { fn(); } catch (e) {
    if (e.signal) interrupted = e.signal;
    check(name, false, e.message.replaceAll(join(dir, '/'), ''));
  }
};
const sha256 = (b) => createHash('sha256').update(b).digest('hex');
const git = (args) => execFileSync('git', ['-C', repoRoot, ...args], { encoding: 'utf8', stdio: ['ignore', 'pipe', 'pipe'] });
const run = (cmd, args) => {
  const r = spawnSync(cmd, args, { encoding: 'utf8' });
  if (r.signal) interrupted = r.signal;
  return { ok: r.status === 0, stdout: (r.stdout ?? '').trim(), text: `${r.stdout ?? ''}${r.stderr ?? ''}`.trim() };
};
const cmpVersion = (a, b) => {
  const pa = a.split('.').map(Number), pb = b.split('.').map(Number);
  for (let i = 0; i < Math.max(pa.length, pb.length); i++) { const d = (pa[i] ?? 0) - (pb[i] ?? 0); if (d) return d; }
  return 0;
};

function loadConf(ref, file) {
  if (file) { const text = readFileSync(file, 'utf8'); return { source: file, sha256: sha256(text), json: JSON.parse(text) }; }
  const spec = `refs/tags/${ref}:${CONF_PATH}`;
  const text = git(['show', spec]);
  return { source: spec, blob: git(['rev-parse', spec]).trim(), sha256: sha256(text), json: JSON.parse(text) };
}
function previousTag() {
  const older = git(['tag', '--list', `${TAG_PREFIX}*`]).split('\n').filter(Boolean)
    .map((t) => t.slice(TAG_PREFIX.length)).filter((v) => /^\d+(\.\d+)*$/.test(v) && cmpVersion(v, version) < 0)
    .sort(cmpVersion);
  return older.length ? `${TAG_PREFIX}${older.at(-1)}` : null;
}

// --- minisign(tauri updater) ---
// Node 의 base64 디코더는 잘못된 문자를 건너뛰므로, updater 가 거부할 문자열을 받아들이지 않게 직접 확인한다.
function strictBase64(text, what) {
  if (typeof text !== 'string' || text.length % 4 || !/^[A-Za-z0-9+/]+={0,2}$/.test(text)
    || Buffer.from(text, 'base64').toString('base64') !== text) throw new Error(`${what}: not canonical base64`);
  return Buffer.from(text, 'base64');
}
// updater 는 std::str::from_utf8 과 Rust str::lines 로 읽는다. 잘못된 UTF-8 은 거부하고, LF 로 끝나지 않은 마지막 줄의 \r 는 남긴다.
function minisignLines(bytes, what) {
  let text;
  try { text = new TextDecoder('utf-8', { fatal: true, ignoreBOM: true }).decode(bytes); } catch { throw new Error(`${what}: not valid UTF-8`); }
  const parts = text.split('\n');
  const tail = parts.pop();
  const lines = parts.map((l) => l.replace(/\r$/, ''));
  if (tail !== '') lines.push(tail);
  return lines;
}
function parsePubkey(text) {
  const lines = minisignLines(strictBase64(text, 'pubkey'), 'pubkey');
  if (lines.length !== 2 || !lines[0].startsWith('untrusted comment: ')) throw new Error('unexpected minisign public key layout');
  const kp = strictBase64(lines[1], 'pubkey key');
  if (kp.length !== 42 || kp.subarray(0, 2).toString() !== 'Ed') throw new Error('unexpected minisign public key');
  const key = createPublicKey({ key: { kty: 'OKP', crv: 'Ed25519', x: kp.subarray(10).toString('base64url') }, format: 'jwk' });
  return { keyId: kp.subarray(2, 10), key };
}
function parseSig(text) {
  const lines = minisignLines(strictBase64(text, 'signature'), 'signature');
  if (lines.length !== 4 || !lines[0].startsWith('untrusted comment: ') || !lines[2].startsWith('trusted comment: ')) {
    throw new Error('unexpected signature layout');
  }
  const sp = strictBase64(lines[1], 'signature');
  const global = strictBase64(lines[3], 'global signature');
  const alg = sp.subarray(0, 2).toString();
  if (sp.length !== 74 || global.length !== 64 || !['Ed', 'ED'].includes(alg)) {
    throw new Error(`unexpected signature (alg ${alg}, ${sp.length}/${global.length} bytes)`);
  }
  return { alg, keyId: sp.subarray(2, 10), sig: sp.subarray(10), trusted: lines[2].slice('trusted comment: '.length), global };
}
// minisign·tauri signer 는 키 ID 를 little-endian 대문자 hex 로 보여 준다.
const minisignKeyId = (bytes) => Buffer.from(bytes).reverse().toString('hex').toUpperCase();
const payloadOk = (pk, s, data) => verify(null, s.alg === 'ED' ? createHash('blake2b512').update(data).digest() : data, pk.key, s.sig);
const globalOk = (pk, s, trusted) => verify(null, Buffer.concat([s.sig, Buffer.from(trusted, 'utf8')]), pk.key, s.global);

// --- PE: 버전 리소스(VS_VERSIONINFO)와 Authenticode 디렉터리 ---
function peInfo(buf) {
  const pe = buf.readUInt32LE(0x3c);
  if (buf.toString('latin1', pe, pe + 4) !== 'PE\0\0') throw new Error('not a PE file');
  const coff = pe + 4, opt = coff + 20;
  const nSections = buf.readUInt16LE(coff + 2), optSize = buf.readUInt16LE(coff + 16);
  const ddBase = opt + (buf.readUInt16LE(opt) === 0x20b ? 112 : 96);
  const dd = (i) => ({ addr: buf.readUInt32LE(ddBase + 8 * i), size: buf.readUInt32LE(ddBase + 8 * i + 4) });
  const sections = Array.from({ length: nSections }, (_, i) => {
    const s = opt + optSize + 40 * i;
    return { va: buf.readUInt32LE(s + 12), vsize: buf.readUInt32LE(s + 8), rawSize: buf.readUInt32LE(s + 16), raw: buf.readUInt32LE(s + 20) };
  });
  const offset = (rva) => {
    const s = sections.find((x) => rva >= x.va && rva < x.va + Math.max(x.vsize, x.rawSize));
    if (!s) throw new Error(`RVA 0x${rva.toString(16)} outside sections`);
    return s.raw + rva - s.va;
  };
  const rsrc = offset(dd(2).addr);
  const entries = (at) => Array.from({ length: buf.readUInt16LE(at + 12) + buf.readUInt16LE(at + 14) },
    (_, i) => ({ id: buf.readUInt32LE(at + 16 + 8 * i), target: buf.readUInt32LE(at + 20 + 8 * i) }));
  const child = (t) => rsrc + (t & 0x7fffffff);
  const type = entries(rsrc).find((e) => e.id === 16); // RT_VERSION
  if (!type) throw new Error('no version resource');
  const lang = entries(child(entries(child(type.target))[0].target))[0];
  const data = child(lang.target);
  const start = offset(buf.readUInt32LE(data));
  return { version: parseVersionInfo(buf.subarray(start, start + buf.readUInt32LE(data + 4))), securityDirSize: dd(4).size };
}
function parseVersionInfo(b) {
  // wType 은 StringFileInfo·StringTable 처럼 하위 블록을 가진 블록에도 1(텍스트)로 쓰이므로 키로 컨테이너를 가린다.
  const isContainer = (key) => ['VS_VERSION_INFO', 'StringFileInfo', 'VarFileInfo'].includes(key) || /^[0-9A-Fa-f]{8}$/.test(key);
  const block = (pos) => {
    const len = b.readUInt16LE(pos), valueLen = b.readUInt16LE(pos + 2), isText = b.readUInt16LE(pos + 4) === 1;
    let p = pos + 6, key = '';
    for (let c; p + 1 < pos + len && (c = b.readUInt16LE(p)); p += 2) key += String.fromCharCode(c);
    p = (p + 2 + 3) & ~3;
    const valueBytes = isText ? valueLen * 2 : valueLen;
    const node = { len, key, value: b.subarray(p, Math.min(p + valueBytes, pos + len)), children: [] };
    if (!isContainer(key)) {
      // 문자열 값은 wValueLength 단위가 도구마다 달라 블록 끝까지 읽고 첫 NUL 에서 자른다.
      node.text = b.subarray(p, pos + len).toString('utf16le').split('\0')[0];
      return node;
    }
    for (let q = (p + valueBytes + 3) & ~3; q + 6 <= pos + len;) {
      const c = block(q);
      if (!c.len) break;
      node.children.push(c);
      q = (q + c.len + 3) & ~3;
    }
    return node;
  };
  const root = block(0);
  if (root.key !== 'VS_VERSION_INFO' || root.value.length < 52 || root.value.readUInt32LE(0) !== 0xfeef04bd) throw new Error('bad VS_VERSIONINFO');
  const quad = (ms, ls) => `${ms >>> 16}.${ms & 0xffff}.${ls >>> 16}.${ls & 0xffff}`;
  const v = root.value;
  const strings = {};
  for (const table of root.children.filter((c) => c.key === 'StringFileInfo').flatMap((c) => c.children)) {
    for (const s of table.children) strings[s.key] = s.text;
  }
  return { fileVersion: quad(v.readUInt32LE(8), v.readUInt32LE(12)), productVersion: quad(v.readUInt32LE(16), v.readUInt32LE(20)), strings };
}

// --- macOS 앱 ---
function inspectApp(label, app) {
  // 링크면 codesign·spctl 이 링크 대상(예: 설치된 앱)을 검사하게 된다.
  const st = lstatSync(app, { throwIfNoEntry: false });
  check(`${label}: HanPage.app is a directory (not a link)`, st?.isDirectory(),
    st?.isDirectory() ? '' : st?.isSymbolicLink() ? 'symbolic link' : st ? 'not a directory' : 'missing');
  if (!st?.isDirectory()) return null;
  const plist = join(app, 'Contents/Info.plist');
  const read = (key) => execFileSync('plutil', ['-extract', key, 'raw', plist], { encoding: 'utf8' }).trim();
  check(`${label}: CFBundleShortVersionString`, read('CFBundleShortVersionString') === version, read('CFBundleShortVersionString'));
  check(`${label}: CFBundleIdentifier`, read('CFBundleIdentifier') === BUNDLE_ID, read('CFBundleIdentifier'));
  const archs = run('lipo', ['-archs', join(app, 'Contents/MacOS', read('CFBundleExecutable'))]);
  check(`${label}: arm64 executable`, archs.ok && archs.text.split(/\s+/).includes('arm64'), archs.text);
  const strict = run('codesign', ['--verify', '--deep', '--strict', app]);
  check(`${label}: codesign --verify --deep --strict`, strict.ok, strict.text.split('\n').at(-1));
  const info = run('codesign', ['-dv', '--verbose=4', app]).text;
  const authority = info.match(/^Authority=(Developer ID Application: .*)$/m)?.[1];
  check(`${label}: TeamIdentifier ${TEAM_ID} and Developer ID Application`,
    new RegExp(`^TeamIdentifier=${TEAM_ID}$`, 'm').test(info) && authority?.endsWith(`(${TEAM_ID})`), authority ?? 'no Developer ID authority');
  const gate = run('spctl', ['--assess', '--type', 'execute', '--verbose=4', app]);
  check(`${label}: Gatekeeper accepts as Notarized Developer ID`, gate.ok && gate.text.includes('source=Notarized Developer ID'),
    gate.text.split('\n').find((l) => l.startsWith('source=')) ?? gate.text);
  const staple = run('xcrun', ['stapler', 'validate', app]);
  check(`${label}: stapled notarization ticket`, staple.ok, staple.text.split('\n').at(-1));
  return info.match(/^CDHash=([0-9a-f]+)$/m)?.[1] ?? null;
}

// 만료가 가까우면 경고만 남긴다. 이미 서명한 앱은 보안 타임스탬프로 유효하고, 갱신이 필요한 것은 다음 릴리스다.
function recordSigningCert(app) {
  const prefix = join(tmp, 'signing-cert');
  const read = run('codesign', ['-d', `--extract-certificates=${prefix}`, app]).ok
    ? run('openssl', ['x509', '-inform', 'DER', '-in', `${prefix}0`, '-noout', '-enddate']) : { ok: false, stdout: '' };
  const end = read.stdout.replace(/^notAfter=/, '');
  const expires = read.ok ? Date.parse(end) : NaN;
  if (Number.isNaN(expires)) { out.warnings.push('could not read the signing certificate expiry'); return; }
  out.signingCertNotAfter = end;
  const days = Math.floor((expires - Date.now()) / 86400000);
  if (days < 90) out.warnings.push(`Developer ID certificate expires on ${end} (${days} days left)`);
}

// 1) 자산 집합
const expected = [`HanPage_${version}_aarch64.dmg`, `HanPage_${version}_x64-setup.exe`, `HanPage_${version}_x64-setup.exe.sig`,
  'HanPage_aarch64.app.tar.gz', 'HanPage_aarch64.app.tar.gz.sig', 'latest.json'].sort();
attempt('assets: readable', () => {
  const entries = readdirSync(dir, { withFileTypes: true }).filter((e) => !e.name.startsWith('.'));
  const names = entries.map((e) => e.name).sort();
  const irregular = entries.filter((e) => !e.isFile()).map((e) => e.name);
  check('assets: exactly the 6 expected regular files', JSON.stringify(names) === JSON.stringify(expected) && !irregular.length,
    names.join(', ') + (irregular.length ? ` (not regular files: ${irregular.join(', ')})` : ''));
  out.assets = Object.fromEntries(entries.filter((e) => e.isFile()).map((e) => {
    const b = readFileSync(join(dir, e.name));
    return [e.name, { bytes: b.length, sha256: sha256(b) }];
  }));
});
const asset = (name) => readFileSync(join(dir, name));

// 2) latest.json
const urlFor = { 'darwin-aarch64': 'HanPage_aarch64.app.tar.gz', 'darwin-aarch64-app': 'HanPage_aarch64.app.tar.gz',
  'windows-x86_64': `HanPage_${version}_x64-setup.exe`, 'windows-x86_64-nsis': `HanPage_${version}_x64-setup.exe` };
attempt('latest.json: readable', () => {
  const manifest = JSON.parse(asset('latest.json').toString('utf8'));
  // updater 는 매니페스트 전체를 RemoteRelease 로 읽는다. name 은 version 의 별칭이라 함께 있으면 거부되고, pub_date 는 RFC 3339 여야 한다.
  // tauri-action 은 version·notes·pub_date(toISOString)·platforms({signature, url}) 만 쓰므로 그 형태 그대로인지 본다.
  const keys = Object.keys(manifest).sort();
  check('latest.json: top-level keys are notes, platforms, pub_date, version',
    JSON.stringify(keys) === JSON.stringify(['notes', 'platforms', 'pub_date', 'version']), keys.join(', '));
  const isoDate = (s) => { try { return new Date(s).toISOString() === s; } catch { return false; } };
  check('latest.json: pub_date is a valid ISO 8601 UTC timestamp', typeof manifest.pub_date === 'string' && isoDate(manifest.pub_date),
    JSON.stringify(manifest.pub_date));
  check('latest.json: version', manifest.version === version, manifest.version);
  check('latest.json: notes empty (release notes live on the GitHub release)', manifest.notes === '', JSON.stringify(manifest.notes));
  const platforms = Object.keys(manifest.platforms ?? {}).sort();
  check('latest.json: 4 platform keys', JSON.stringify(platforms) === JSON.stringify(Object.keys(urlFor).sort()), platforms.join(', '));
  for (const [p, file] of Object.entries(urlFor)) {
    const entry = manifest.platforms?.[p];
    check(`latest.json: ${p} entry has only signature and url`,
      JSON.stringify(Object.keys(entry ?? {}).sort()) === '["signature","url"]', Object.keys(entry ?? {}).join(', '));
    check(`latest.json: ${p} url`, entry?.url === `${DOWNLOAD_PREFIX}${tag}/${file}`, entry?.url ?? 'missing');
    check(`latest.json: ${p} signature is byte-identical to ${file}.sig`, entry?.signature === asset(`${file}.sig`).toString('utf8'));
  }
});

// 3) 공개키 출처·연속성과 updater 서명
let pk = null;
attempt('updater pubkey: readable', () => {
  const conf = loadConf(tag, opts.conf);
  out.conf = { source: conf.source, blob: conf.blob, sha256: conf.sha256 };
  pk = parsePubkey(conf.json.plugins.updater.pubkey);
  out.keyId = pk.keyId.toString('hex');
  out.keyIdMinisign = minisignKeyId(pk.keyId);
  const prevTag = opts.prevConf ? null : opts.prevTag ?? previousTag();
  if (!opts.prevConf && !prevTag) throw new Error('previous release tag not found (use --prev-tag or --prev-conf)');
  const prev = loadConf(prevTag, opts.prevConf);
  out.previous = { source: prev.source, blob: prev.blob, sha256: prev.sha256 };
  check(`updater pubkey unchanged since ${prevTag ?? prev.source}`,
    prev.json.plugins.updater.pubkey === conf.json.plugins.updater.pubkey, `key ${out.keyIdMinisign}`);
});
for (const name of ['HanPage_aarch64.app.tar.gz', `HanPage_${version}_x64-setup.exe`]) {
  attempt(`minisign: ${name}`, () => {
    if (!pk) throw new Error('no updater pubkey');
    const data = asset(name);
    const s = parseSig(asset(`${name}.sig`).toString('utf8'));
    check(`minisign: ${name} key id matches updater pubkey`, s.keyId.equals(pk.keyId), minisignKeyId(s.keyId));
    check(`minisign: ${name} payload signature`, payloadOk(pk, s, data), s.alg);
    check(`minisign: ${name} global signature over trusted comment`, globalOk(pk, s, s.trusted), s.trusted);
    const rejected = [0, data.length >> 1, data.length - 1].every((i) => {
      const t = Buffer.from(data); t[i] ^= 1; return !payloadOk(pk, s, t);
    });
    check(`minisign: ${name} rejects 1-byte tamper at start/middle/end`, rejected);
    check(`minisign: ${name} rejects altered trusted comment`, !globalOk(pk, s, `${s.trusted} `));
  });
}

// 4) GitHub 릴리스의 자산이 감사한 파일과 같은지(--release)
if (opts.release) {
  attempt('release: readable', () => {
    const release = JSON.parse(execFileSync('gh', ['api', `repos/paldyn/HanPage/releases/${opts.release}`],
      { encoding: 'utf8', stdio: ['ignore', 'pipe', 'pipe'] }));
    out.release = { id: release.id, tag: release.tag_name, draft: release.draft, target: release.target_commitish };
    check('release: tag matches', release.tag_name === tag, release.tag_name);
    const tagCommit = run('git', ['-C', repoRoot, 'rev-parse', `refs/tags/${tag}^{commit}`]);
    check('release: target is the local tag commit', tagCommit.ok && release.target_commitish === tagCommit.stdout,
      `${release.target_commitish} / ${tagCommit.text}`);
    const remote = Object.fromEntries((release.assets ?? []).map((a) => [a.name, a.digest ?? '']));
    check('release: holds exactly the 6 expected assets', JSON.stringify(Object.keys(remote).sort()) === JSON.stringify(expected),
      Object.keys(remote).sort().join(', '));
    for (const name of expected) {
      const local = out.assets?.[name]?.sha256;
      check(`release: ${name} digest equals the audited file`, !!local && remote[name] === `sha256:${local}`, remote[name] || 'no digest');
    }
  });
}

// 5) 번들 메타데이터·코드 서명
const tmp = mkdtempSync(join(tmpdir(), 'hp-release-audit-'));
const mnt = join(tmp, 'mnt');
let mounted = false;
try {
  attempt('updater archive: inspect', () => {
    const archive = join(dir, 'HanPage_aarch64.app.tar.gz');
    const listing = execFileSync('tar', ['-tzf', archive], { encoding: 'utf8' }).split('\n').filter(Boolean);
    const contained = listing.length > 0 && listing.every((p) => /^HanPage\.app(\/|$)/.test(p) && !p.split('/').includes('..'));
    check('updater archive: only HanPage.app/ entries', contained, `${listing.length} entries`);
    if (!contained) throw new Error('unexpected archive layout; not extracted');
    mkdirSync(join(tmp, 'upd'));
    execFileSync('tar', ['-xzf', archive, '-C', join(tmp, 'upd')]);
    out.cdhash = { updater: inspectApp('updater .app', join(tmp, 'upd/HanPage.app')) };
    if (out.cdhash.updater) recordSigningCert(join(tmp, 'upd/HanPage.app'));
  });
  attempt('DMG: inspect', () => {
    const dmg = join(dir, `HanPage_${version}_aarch64.dmg`);
    const local = (text) => text.replaceAll(dmg, basename(dmg));
    const verified = run('hdiutil', ['verify', dmg]);
    check('DMG: hdiutil verify', verified.ok, local(verified.text.split('\n').at(-1)));
    // 컨테이너는 공증 대상이 아니지만 Developer ID 로 서명한다. 서명 없는 재포장 DMG 를 걸러낸다.
    const signed = run('codesign', ['--verify', '--strict', dmg]);
    const info = run('codesign', ['-dv', '--verbose=4', dmg]).text;
    const authority = info.match(/^Authority=(Developer ID Application: .*)$/m)?.[1];
    check(`DMG: container signed by Developer ID Application (${TEAM_ID})`,
      signed.ok && new RegExp(`^TeamIdentifier=${TEAM_ID}$`, 'm').test(info) && authority?.endsWith(`(${TEAM_ID})`),
      authority ?? local(signed.text.split('\n').at(-1)));
    mkdirSync(mnt);
    // 원본이 Finder 등에서 이미 마운트돼 있어도 충돌하지 않게 임시 폴더의 복제본(APFS 면 clone)을 마운트한다.
    const image = join(tmp, 'image.dmg');
    copyFileSync(dmg, image, fsc.COPYFILE_FICLONE);
    const attached = run('hdiutil', ['attach', '-readonly', '-nobrowse', '-noautoopen', '-mountpoint', mnt, image]);
    if (!attached.ok) throw new Error(`hdiutil attach: ${attached.text.split('\n').at(-1)}`);
    mounted = true;
    const visible = readdirSync(mnt, { withFileTypes: true }).filter((e) => !e.name.startsWith('.'));
    const link = visible.find((e) => e.name === 'Applications');
    check('DMG: volume holds only HanPage.app and the /Applications link',
      visible.length === 2 && visible.some((e) => e.name === 'HanPage.app') && link?.isSymbolicLink()
        && readlinkSync(join(mnt, 'Applications')) === '/Applications',
      visible.map((e) => `${e.name}${e.isSymbolicLink() ? '@' : e.isDirectory() ? '/' : ''}`).sort().join(', '));
    out.cdhash = { ...out.cdhash, dmg: inspectApp('DMG .app', join(mnt, 'HanPage.app')) };
  });
  if (!interrupted) {
    check('updater .app and DMG .app are the same build (CDHash)',
      out.cdhash?.updater && out.cdhash.updater === out.cdhash.dmg, `${out.cdhash?.updater} / ${out.cdhash?.dmg}`);
  }
  attempt('setup.exe: inspect', () => {
    const exe = peInfo(asset(`HanPage_${version}_x64-setup.exe`));
    const want = [version, `${version}.0`];
    check('setup.exe: VS_FIXEDFILEINFO file version', exe.version.fileVersion === `${version}.0`, exe.version.fileVersion);
    check('setup.exe: StringFileInfo FileVersion/ProductVersion',
      want.includes(exe.version.strings.FileVersion) && want.includes(exe.version.strings.ProductVersion),
      `${exe.version.strings.FileVersion} / ${exe.version.strings.ProductVersion}`);
    check('setup.exe: no Authenticode signature (unsigned by decision #122)', exe.securityDirSize === 0, `security dir size ${exe.securityDirSize}`);
  });
} finally {
  if (mounted && !run('hdiutil', ['detach', mnt]).ok) run('hdiutil', ['detach', '-force', mnt]);
  try {
    rmSync(tmp, { recursive: true, force: true });
  } catch {
    // 아카이브에 읽기 전용 디렉터리가 있으면 쓰기 권한을 준 뒤 다시 지운다.
    run('chmod', ['-R', 'u+w', tmp]);
    try { rmSync(tmp, { recursive: true, force: true }); } catch (e) { check('cleanup: temporary directory removed', false, e.message); }
  }
}

if (interrupted) check('audit: completed without interruption', false, `stopped by ${interrupted}; remaining checks skipped`);
out.pass = out.checks.every((c) => c.ok);
// 결과 JSON 은 파일로 보내므로 경고는 터미널에도 남긴다.
for (const w of out.warnings) console.error(`warning: ${w}`);
console.log(JSON.stringify(out, null, 2));
// process.exit 는 파이프로 넘기는 stdout 을 다 쓰기 전에 끝낼 수 있다.
process.exitCode = interrupted ? 130 : out.pass ? 0 : 1;
