import { createRequire } from 'node:module';
import { readFileSync, writeFileSync, mkdtempSync, rmSync } from 'node:fs';
import { createHash } from 'node:crypto';
import assert from 'node:assert/strict';
const w='/Users/lwm/.codex/worktrees/upstream-sync-final-20261001/HanPage';
const o=w+'/output/pr-review/web-favicon-20261003';
const require=createRequire(w+'/rhwp-studio/package.json');
const puppeteer=require('puppeteer-core');
const sha=b=>createHash('sha256').update(b).digest('hex');
const profile=mkdtempSync('/private/tmp/hanpage-web-favicon-');
const base='http://127.0.0.1:7717';
const expected=sha(readFileSync(w+'/HanPage-Desktop/src-tauri/icons/icon.ico'));
const browser=await puppeteer.launch({headless:true,executablePath:'/Applications/Google Chrome.app/Contents/MacOS/Google Chrome',userDataDir:profile});
const result={source_sha:'a4ddf332745998970b1d28f690e610a1e92c340a',base,checks:[],page_exceptions:[],scope:'Actual built web app favicon, Apple touch, PWA URLs and offline service worker cache. App document editing and installed PWA/browser toolbar UI were not tested.'};
try {
  const page=await browser.newPage();
  page.on('pageerror',error=>result.page_exceptions.push(String(error)));
  await page.goto(base,{waitUntil:'domcontentloaded',timeout:30000});
  const urls=await page.evaluate(()=>({favicon:document.querySelector('link[rel="icon"]').href,touch:document.querySelector('link[rel="apple-touch-icon"]').href,manifest:document.querySelector('link[rel="manifest"]').href}));
  assert.match(urls.favicon,/\/assets\/hanpage-favicon-[\w-]+\.ico$/);
  const icon=await fetch(urls.favicon);assert.equal(icon.status,200);assert.equal(sha(Buffer.from(await icon.arrayBuffer())),expected);
  result.checks.push({name:'Actual built HTML requests fingerprinted new favicon',pass:true,url:urls.favicon,source_sha256:expected});
  const manifest=await(await fetch(urls.manifest)).json();
  const imageChecks=await page.evaluate(async ({urls,manifest})=>{
    const records=[];
    for(const i of manifest.icons){
      const url=new URL(i.src,urls.manifest).href;
      const image=new Image();image.src=url;await image.decode();
      records.push({url,declared:i.sizes,actual:image.naturalWidth+'x'+image.naturalHeight});
    }
    const touch=new Image();touch.src=urls.touch;await touch.decode();
    return {pwa:records,touch:{url:urls.touch,width:touch.naturalWidth,height:touch.naturalHeight}};
  },{urls,manifest});
  assert.equal(imageChecks.pwa.length,5);assert.ok(imageChecks.pwa.every(i=>i.declared===i.actual&&i.url.includes('/icons/hanpage-04-')));
  assert.equal(imageChecks.touch.width,256);assert.equal(imageChecks.touch.height,256);
  for(const row of imageChecks.pwa){const actual=Buffer.from(await(await fetch(row.url)).arrayBuffer());assert.equal(sha(actual),sha(readFileSync(w+'/rhwp-studio/public/'+new URL(row.url).pathname.slice(1))));}
  const touch=Buffer.from(await(await fetch(urls.touch)).arrayBuffer());assert.equal(sha(touch),sha(readFileSync(w+'/HanPage-Desktop/src-tauri/icons/256x256.png')));
  result.checks.push({name:'Browser decodes current PWA and Apple touch icons at declared sizes',pass:true,...imageChecks});
  await page.waitForFunction(()=>!!navigator.serviceWorker.controller,{timeout:45000});
  result.service_worker=await page.evaluate(()=>navigator.serviceWorker.controller.scriptURL);
  await page.setOfflineMode(true);
  const offline=await page.evaluate(async paths=>{
    const records=[];
    for(const path of paths){const r=await fetch(path,{cache:'no-store'});const bytes=new Uint8Array(await r.arrayBuffer());const digest=await crypto.subtle.digest('SHA-256',bytes);records.push({url:path,status:r.status,sha256:Array.from(new Uint8Array(digest),b=>b.toString(16).padStart(2,'0')).join(''),bytes:bytes.length});}
    return records;
  },[urls.favicon,urls.touch,...new Set(imageChecks.pwa.map(i=>i.url))]);
  assert.ok(offline.every(r=>r.status===200));assert.equal(offline[0].sha256,expected);
  for(const r of offline){const path=new URL(r.url).pathname;assert.equal(r.sha256,sha(readFileSync(w+'/rhwp-studio/dist'+path)));}
  result.checks.push({name:'Actual service worker supplies all six branding URLs offline',pass:true,assets:offline});
  await page.setOfflineMode(false);
  assert.equal(result.page_exceptions.length,0);
  result.status='PASS';
} catch(error){result.status='FAIL';result.error=String(error);process.exitCode=1;}
finally{await browser.close();rmSync(profile,{recursive:true,force:true});}
writeFileSync(o+'/browser-results.json',JSON.stringify(result,null,2)+'\n');
console.log(JSON.stringify(result,null,2));
