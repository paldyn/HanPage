#!/usr/bin/env node
// HanPage Desktop draft-release audit (read-only on assets; never executes them).
// usage: node audit-draft.mjs <assetDir> <tauri.conf.json> <version>
import { readFileSync, readdirSync, mkdtempSync, rmSync } from 'node:fs';
import { createHash, createPublicKey, verify } from 'node:crypto';
import { execFileSync } from 'node:child_process';
import { join } from 'node:path';
import { tmpdir } from 'node:os';

const [dir, confPath, version] = process.argv.slice(2);
const tag = `hanpage-desktop-v${version}`;
const out = { version, tag, checks: [] };
const check = (name, ok, detail = '') => { out.checks.push({ name, ok: !!ok, detail }); };
const sha256 = (b) => createHash('sha256').update(b).digest('hex');

// 1) asset set
const expected = [
  `HanPage_${version}_aarch64.dmg`, `HanPage_${version}_x64-setup.exe`, `HanPage_${version}_x64-setup.exe.sig`,
  'HanPage_aarch64.app.tar.gz', 'HanPage_aarch64.app.tar.gz.sig', 'latest.json',
];
const files = readdirSync(dir).filter((f) => !f.startsWith('.')).sort();
check('assets: exactly the 6 expected files', JSON.stringify(files) === JSON.stringify([...expected].sort()), files.join(', '));
out.assets = Object.fromEntries(files.map((f) => { const b = readFileSync(join(dir, f)); return [f, { bytes: b.length, sha256: sha256(b) }]; }));

// 2) latest.json
const manifest = JSON.parse(readFileSync(join(dir, 'latest.json'), 'utf8'));
check('latest.json: version', manifest.version === version, manifest.version);
const platforms = Object.keys(manifest.platforms ?? {}).sort();
const wantPlatforms = ['darwin-aarch64', 'darwin-aarch64-app', 'windows-x86_64', 'windows-x86_64-nsis'];
check('latest.json: 4 platform keys', JSON.stringify(platforms) === JSON.stringify(wantPlatforms), platforms.join(', '));
const urlFor = { 'darwin-aarch64': 'HanPage_aarch64.app.tar.gz', 'darwin-aarch64-app': 'HanPage_aarch64.app.tar.gz',
  'windows-x86_64': `HanPage_${version}_x64-setup.exe`, 'windows-x86_64-nsis': `HanPage_${version}_x64-setup.exe` };
for (const p of wantPlatforms) {
  const entry = manifest.platforms?.[p];
  const want = `https://github.com/paldyn/HanPage/releases/download/${tag}/${urlFor[p]}`;
  check(`latest.json: ${p} url`, entry?.url === want, entry?.url ?? 'missing');
  const sigFile = readFileSync(join(dir, `${urlFor[p]}.sig`), 'utf8').trim();
  check(`latest.json: ${p} signature equals .sig asset`, entry?.signature?.trim() === sigFile);
}

// 3) minisign verification against the real payloads + tamper rejection
const conf = JSON.parse(readFileSync(confPath, 'utf8'));
const keyText = Buffer.from(conf.plugins.updater.pubkey, 'base64').toString('utf8').split(/\r?\n/).filter(Boolean);
const kp = Buffer.from(keyText[1], 'base64');
const pub = createPublicKey({ key: { kty: 'OKP', crv: 'Ed25519', x: kp.subarray(10).toString('base64url') }, format: 'jwk' });
function minisignVerify(data, sigOuter) {
  const lines = Buffer.from(sigOuter.trim(), 'base64').toString('utf8').split(/\r?\n/).filter(Boolean);
  if (lines.length !== 4) throw new Error('unexpected signature layout');
  const sp = Buffer.from(lines[1], 'base64');
  const globalSig = Buffer.from(lines[3], 'base64');
  if (!sp.subarray(2, 10).equals(kp.subarray(2, 10))) throw new Error('key id mismatch');
  const alg = sp.subarray(0, 2).toString();
  const message = alg === 'ED' ? createHash('blake2b512').update(data).digest() : data;
  const payloadOk = verify(null, message, pub, sp.subarray(10));
  const comment = Buffer.from(lines[2].replace(/^trusted comment: /, ''), 'utf8');
  const globalOk = verify(null, Buffer.concat([sp.subarray(10), comment]), pub, globalSig);
  return { alg, keyId: kp.subarray(2, 10).toString('hex'), payloadOk, globalOk, trusted: comment.toString() };
}
for (const name of ['HanPage_aarch64.app.tar.gz', `HanPage_${version}_x64-setup.exe`]) {
  const data = readFileSync(join(dir, name));
  const sig = readFileSync(join(dir, `${name}.sig`), 'utf8');
  const r = minisignVerify(data, sig);
  check(`minisign: ${name} payload+global signature`, r.payloadOk && r.globalOk, `${r.alg} key ${r.keyId} | ${r.trusted}`);
  const tampered = Buffer.from(data); tampered[tampered.length - 1] ^= 1;
  check(`minisign: ${name} rejects 1-byte tamper`, !minisignVerify(tampered, sig).payloadOk);
}

// 4) versions inside bundles
const tmp = mkdtempSync(join(tmpdir(), 'hp-audit-'));
try {
  execFileSync('tar', ['-xzf', join(dir, 'HanPage_aarch64.app.tar.gz'), '-C', tmp]);
  const plist = join(tmp, 'HanPage.app/Contents/Info.plist');
  const v = execFileSync('plutil', ['-extract', 'CFBundleShortVersionString', 'raw', plist]).toString().trim();
  check('updater .app Info.plist version', v === version, v);
  const exe = readFileSync(join(dir, `HanPage_${version}_x64-setup.exe`));
  check('setup.exe contains UTF-16 version string', exe.includes(Buffer.from(version, 'utf16le')));
  // PE security directory (Authenticode) — expected empty for unsigned distribution (#122)
  const peOff = exe.readUInt32LE(0x3c);
  const magic = exe.readUInt16LE(peOff + 24);
  const ddOff = peOff + 24 + (magic === 0x20b ? 112 : 96) + 4 * 8;
  const secSize = exe.readUInt32LE(ddOff + 4);
  check('setup.exe Authenticode absent (unsigned by decision #122)', secSize === 0, `security dir size ${secSize}`);
} finally { rmSync(tmp, { recursive: true, force: true }); }

out.pass = out.checks.every((c) => c.ok);
console.log(JSON.stringify(out, null, 2));
process.exit(out.pass ? 0 : 1);
