import { createRequire } from 'node:module';
import { mkdtempSync, mkdirSync, readFileSync, writeFileSync, existsSync } from 'node:fs';
import { dirname, join } from 'node:path';

const repo = '/Users/lwm/.codex/worktrees/upstream-sync-final-20261001/HanPage';
const primary = '/Users/lwm/vault/dev/company/paldyn/project/in-house/HanPage';
const out = join(repo, 'output/pr-review/web-favicon-20261003');
const phase = process.argv[2] || 'before';
if (!['before', 'after'].includes(phase)) throw new Error('Expected before or after phase');
const ownershipId = 'hanpage-public-upgrade-20261003';
let profile = process.argv[3];
if (phase === 'before' && !profile) {
  const ownedRoot = mkdtempSync('/private/tmp/hanpage-web-public-upgrade-');
  profile = join(ownedRoot, 'profile');
  mkdirSync(profile);
  writeFileSync(join(ownedRoot, 'ownership.json'), JSON.stringify({ ownershipId, profile, createdAt: new Date().toISOString() }) + '\n');
}
if (!profile?.startsWith('/private/tmp/hanpage-web-public-upgrade-')) throw new Error('Only owned temporary profile may be used');
const marker = join(dirname(profile), 'ownership.json');
if (!existsSync(marker) || JSON.parse(readFileSync(marker, 'utf8')).ownershipId !== ownershipId) throw new Error('Missing owned profile marker');

const require = createRequire(join(primary, 'rhwp-studio/package.json'));
const puppeteer = require('puppeteer-core');
const expectedFaviconHash = 'c4bdbc5ab66b05b549c6c4113b030133c1691450c68bb8183232b516efde2b09';
const expectedPwaHashes = {
  '128x128': '50bf5565ac8b2bf07ee3c31ac50ed0d9515e8aa4b239b94938c54e676dace5f5',
  '192x192': '89d3e2cd23b2d5a50a7f68105cd5117ae0ddb6c9b957982845a1579bb0c36f26',
  '256x256': 'ba4aa7a59ac3fa4fe4dd24e844e9c670a1b1cdc0a765e287ac7f528206433af3',
  '512x512': '5a3dd9f2596e8cdaf8ab398e015823b3d83f1bcac2453337aff7d36d8dc6c7fd',
};
const result = {
  phase, at: new Date().toISOString(), url: 'https://hanpage.paldyn.com/',
  ownedProfile: profile, helper: '/private/tmp/hanpage-web-public-upgrade-20261003.mjs',
  scope: 'Public HTML favicon, manifest icons and actual service worker cache in an exclusively owned Chrome profile. User browser, installed PWA, Desktop app and document editing untouched.',
  pageExceptions: [], requestFailures: [], browserConsoleErrors: [], checks: [],
  brandingResponses: [],
};
mkdirSync(out, { recursive: true });
const browser = await puppeteer.launch({
  headless: true, executablePath: '/Applications/Google Chrome.app/Contents/MacOS/Google Chrome',
  userDataDir: profile, args: ['--no-first-run', '--no-default-browser-check'],
});
try {
  const page = await browser.newPage();
  page.on('pageerror', error => result.pageExceptions.push(String(error)));
  page.on('requestfailed', req => result.requestFailures.push({ url: req.url(), error: req.failure()?.errorText }));
  page.on('console', msg => { if (msg.type() === 'error') result.browserConsoleErrors.push(msg.text()); });
  page.on('response', response => {
    if (/\/favicon\.ico|\/assets\/hanpage-favicon-|\/icons\/|\/manifest\.webmanifest|\/sw\.js/.test(response.url())) {
      result.brandingResponses.push({ url: response.url(), status: response.status(), fromServiceWorker: response.fromServiceWorker(), fromCache: response.fromCache() });
    }
  });
  const response = await page.goto(result.url, { waitUntil: 'domcontentloaded', timeout: 30000 });
  result.navigation = { status: response.status(), url: response.url(), fromServiceWorker: response.fromServiceWorker(), headers: response.headers() };
  result.document = await page.evaluate(() => ({
    url: location.href, title: document.title,
    favicon: { attribute: document.querySelector('link[rel="icon"]')?.getAttribute('href'), url: document.querySelector('link[rel="icon"]')?.href },
    appleTouch: document.querySelector('link[rel="apple-touch-icon"]')?.href,
    manifest: document.querySelector('link[rel="manifest"]')?.href,
    serviceWorkerSupported: 'serviceWorker' in navigator,
  }));

  if (phase === 'after') {
    result.initialDocumentBeforeAutomaticUpdate = result.document;
    result.initialController = await page.evaluate(() => navigator.serviceWorker.controller ? { scriptURL: navigator.serviceWorker.controller.scriptURL, state: navigator.serviceWorker.controller.state } : null);
    try {
      await page.waitForFunction(() => /\/assets\/hanpage-favicon-[\w-]+\.ico$/.test(document.querySelector('link[rel="icon"]')?.href || ''), { timeout: 45000 });
      result.automaticUpdateObservation = 'New favicon HTML became visible on this visit without cache clearing or explicit registration.update';
    } catch (error) {
      result.automaticUpdateObservation = 'New favicon HTML was not visible within 45 seconds; one ordinary page reload performed';
      result.automaticUpdateWaitError = String(error);
      const reload = await page.reload({ waitUntil: 'domcontentloaded', timeout: 30000 });
      result.ordinaryReload = { status: reload.status(), fromServiceWorker: reload.fromServiceWorker() };
    }
    result.document = await page.evaluate(() => ({
      url: location.href, title: document.title,
      favicon: { attribute: document.querySelector('link[rel="icon"]')?.getAttribute('href'), url: document.querySelector('link[rel="icon"]')?.href },
      appleTouch: document.querySelector('link[rel="apple-touch-icon"]')?.href,
      manifest: document.querySelector('link[rel="manifest"]')?.href,
      serviceWorkerSupported: 'serviceWorker' in navigator,
    }));
  }

  const fetchRecord = async url => page.evaluate(async url => {
    const c = new AbortController(); const timer = setTimeout(() => c.abort(), 12000);
    try {
      const r = await fetch(url, { signal: c.signal }); const bytes = new Uint8Array(await r.arrayBuffer());
      const digest = await crypto.subtle.digest('SHA-256', bytes);
      return { url, finalUrl: r.url, status: r.status, type: r.type, bytes: bytes.length,
        contentType: r.headers.get('content-type'), sha256: Array.from(new Uint8Array(digest), b => b.toString(16).padStart(2, '0')).join('') };
    } finally { clearTimeout(timer); }
  }, url);
  result.favicon = await fetchRecord(result.document.favicon.url);
  result.appleTouch = await fetchRecord(result.document.appleTouch);
  result.manifest = await page.evaluate(async url => {
    const c = new AbortController(); const timer = setTimeout(() => c.abort(), 12000);
    try { const r = await fetch(url, { signal: c.signal }); return { url, status: r.status, value: await r.json() }; }
    finally { clearTimeout(timer); }
  }, result.document.manifest);
  result.manifestIcons = [];
  for (const icon of result.manifest.value.icons || []) {
    const url = new URL(icon.src, result.document.manifest).href;
    const asset = await fetchRecord(url);
    const decoded = await page.evaluate(async url => {
      const img = new Image(); img.src = url;
      await Promise.race([img.decode(), new Promise((_, reject) => setTimeout(() => reject(new Error('Image decode timeout')), 12000))]);
      return { width: img.naturalWidth, height: img.naturalHeight };
    }, url);
    result.manifestIcons.push({ ...icon, ...asset, decoded });
  }

  try {
    await page.waitForFunction(() => !!navigator.serviceWorker.controller, { timeout: 45000 });
    result.controllerWait = 'controlled';
  } catch (error) { result.controllerWait = 'uncontrolled'; result.controllerWaitError = String(error); }
  result.serviceWorker = await page.evaluate(async () => ({
    controller: navigator.serviceWorker.controller ? { scriptURL: navigator.serviceWorker.controller.scriptURL, state: navigator.serviceWorker.controller.state } : null,
    registrations: (await navigator.serviceWorker.getRegistrations()).map(r => ({
      scope: r.scope, active: r.active ? { scriptURL: r.active.scriptURL, state: r.active.state } : null,
      waiting: r.waiting ? { scriptURL: r.waiting.scriptURL, state: r.waiting.state } : null,
      installing: r.installing ? { scriptURL: r.installing.scriptURL, state: r.installing.state } : null,
    })),
  }));
  result.cache = await page.evaluate(async urls => {
    const names = await caches.keys(); const matches = [];
    for (const name of names) {
      const cache = await caches.open(name);
      for (const url of urls) {
        const r = await cache.match(url, { ignoreSearch: true });
        if (!r) continue;
        const bytes = new Uint8Array(await r.arrayBuffer()); const digest = await crypto.subtle.digest('SHA-256', bytes);
        matches.push({ name, url, status: r.status, bytes: bytes.length, sha256: Array.from(new Uint8Array(digest), b => b.toString(16).padStart(2, '0')).join('') });
      }
    }
    return { names, brandingMatches: matches };
  }, [result.document.favicon.url, result.document.appleTouch, result.document.manifest, ...new Set(result.manifestIcons.map(i => i.url))]);
  result.checks.push({ name: 'Public HTML and current favicon fetched', pass: result.navigation.status === 200 && result.favicon.status === 200 });
  result.checks.push({ name: 'Manifest images decoded at declared sizes', pass: result.manifestIcons.length === 5 && result.manifestIcons.every(i => i.status === 200 && `${i.decoded.width}x${i.decoded.height}` === i.sizes) });
  result.checks.push({ name: 'Existing service worker controls this owned profile', pass: !!result.serviceWorker.controller });
  result.checks.push({ name: 'Existing branding assets are in service worker cache', pass: result.cache.brandingMatches.some(i => i.url === result.document.favicon.url && i.sha256 === result.favicon.sha256) });
  if (phase === 'before') {
    result.checks.push({ name: 'Live site still serves the recorded old favicon', pass: result.document.favicon.attribute === '/favicon.ico' && result.favicon.sha256 === '552ba2454b39bcbddd2d17243622ddcb3f16d05f110555e158ba6e500ee24c9f' });
  } else {
    result.expectedNewFaviconSha256 = expectedFaviconHash;
    result.checks.push({ name: 'This same profile now requests and receives new hashed favicon', pass: /\/assets\/hanpage-favicon-[\w-]+\.ico$/.test(result.document.favicon.url) && result.favicon.sha256 === expectedFaviconHash });
    result.checks.push({ name: 'This same profile receives every new named PWA icon and new Apple touch icon', pass: result.manifestIcons.every(i => /\/icons\/hanpage-04-/.test(i.url) && i.sha256 === expectedPwaHashes[i.sizes]) && result.appleTouch.sha256 === expectedPwaHashes['256x256'] });
    result.fallbackFavicon = await fetchRecord(new URL('/favicon.ico', result.url).href);
    result.checks.push({ name: 'Fallback favicon also receives the new approved ICO', pass: result.fallbackFavicon.status === 200 && result.fallbackFavicon.sha256 === expectedFaviconHash });
    const offlineUrls = [result.document.favicon.url, result.document.appleTouch, result.fallbackFavicon.url, ...new Set(result.manifestIcons.map(i => i.url))];
    try {
      await page.setOfflineMode(true);
      result.offlineAssets = [];
      for (const url of offlineUrls) result.offlineAssets.push(await fetchRecord(url));
      result.checks.push({ name: 'The updated real service worker supplies all seven branding URLs offline', pass: result.offlineAssets.length === 7 && result.offlineAssets.every(i => i.status === 200 && i.sha256 === ([result.document.favicon.url, result.fallbackFavicon.url].includes(i.url) ? expectedFaviconHash : i.url === result.document.appleTouch ? expectedPwaHashes['256x256'] : expectedPwaHashes[result.manifestIcons.find(m => m.url === i.url)?.sizes])) });
    } finally { await page.setOfflineMode(false); }
  }
  result.checks.push({ name: 'No unhandled public page exception during branding validation', pass: result.pageExceptions.length === 0 });
  result.status = result.checks.every(c => c.pass) ? 'PASS' : 'FAIL';
  if (result.status !== 'PASS') process.exitCode = 1;
} catch (error) { result.status = 'FAIL'; result.error = String(error); process.exitCode = 1; }
finally {
  await browser.close();
  result.browserClosed = true;
  result.profileRetainedForUpgrade = true;
  writeFileSync(join(out, `public-browser-${phase}.json`), JSON.stringify(result, null, 2) + '\n');
  console.log(JSON.stringify({ status: result.status, checks: result.checks, ownedProfile: profile, output: join(out, `public-browser-${phase}.json`), pageExceptions: result.pageExceptions, error: result.error }, null, 2));
}
