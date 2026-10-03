from pathlib import Path
import json,hashlib,subprocess,urllib.request,urllib.parse
w=Path('/Users/lwm/.codex/worktrees/upstream-sync-final-20261001/HanPage');o=w/'output/pr-review/desktop089'
headers={'User-Agent':'HanPage-release-audit','Accept':'application/vnd.github+json'}
def fetch(url):
    req=urllib.request.Request(url,headers=headers)
    with urllib.request.urlopen(req,timeout=60) as response:
        resolved=urllib.parse.urlsplit(response.url)
        return response.read(),{'status':response.status,'requested_url':url,'resolved_url_without_query':urllib.parse.urlunsplit((resolved.scheme,resolved.netloc,resolved.path,'','')),'temporary_download_query_omitted':bool(resolved.query)}
raw, http=fetch('https://api.github.com/repos/paldyn/HanPage/releases/latest');j=json.loads(raw)
assert j['tag_name']=='hanpage-desktop-v0.8.9' and not j['draft'] and not j['prerelease']
assert j['target_commitish']=='7aeee23482b20a637fcf4acf6ff9a00046b5b313'
draft=json.loads((o/'draft-release.json').read_text())
assert j['id']==draft['id']
expected_assets={a['name']:(a['id'],a['size'],a['digest']) for a in draft['assets']}
assert len(expected_assets)==6 and {a['name']:(a['id'],a['size'],a['digest']) for a in j['assets']}==expected_assets
assert all(a['state']=='uploaded' and a['browser_download_url']=='https://github.com/paldyn/HanPage/releases/download/hanpage-desktop-v0.8.9/'+a['name'] for a in j['assets'])
notes=Path('/private/tmp/hanpage089_release_notes.md').read_text().strip()
assert j['body'].strip()==notes and '\ufeff' not in j['body'] and '??' not in j['body']
expected=(o/'release/latest.json').read_bytes()
actual,endpoint=fetch('https://github.com/paldyn/HanPage/releases/latest/download/latest.json')
assert actual==expected
manifest=json.loads(actual);assert manifest['version']=='0.8.9'
assert set(manifest['platforms'])=={'darwin-aarch64','darwin-aarch64-app','windows-x86_64','windows-x86_64-nsis'}
release={'release_id':j['id'],'url':j['html_url'],'tag':j['tag_name'],'published_at':j['published_at'],'source_sha':j['target_commitish'],'is_draft':j['draft'],'is_prerelease':j['prerelease'],'latest_release_http':http,'manifest_version':manifest['version'],'manifest_sha256':hashlib.sha256(actual).hexdigest(),'manifest_byte_identical_to_verified_draft':True,'platforms':sorted(manifest['platforms']),'endpoint_http':endpoint,'anonymous_requests':True,'six_assets_identical_to_verified_draft':True,'six_asset_public_urls_match_tag':True,'release_notes_exact_utf8_verified':True,'six_assets':[(a['name'],a['size'],a.get('digest')) for a in j['assets']]}
(o/'public-release.json').write_text(json.dumps(j,ensure_ascii=False,indent=2)+'\n')
(o/'public-verification.json').write_text(json.dumps(release,ensure_ascii=False,indent=2)+'\n')
print(json.dumps(release,ensure_ascii=False,indent=2))
