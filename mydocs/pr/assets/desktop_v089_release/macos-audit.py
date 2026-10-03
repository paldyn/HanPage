from pathlib import Path
import argparse,subprocess,plistlib,json,hashlib,tarfile
parser=argparse.ArgumentParser();parser.add_argument('--repo',type=Path,required=True);parser.add_argument('--source-sha',required=True)
args=parser.parse_args();w=args.repo.resolve();o=w/'output/pr-review/desktop089';r=o/'release';logs=o/'logs';logs.mkdir(exist_ok=True)
sha=lambda p:hashlib.sha256(p.read_bytes()).hexdigest()
icon=subprocess.check_output(['git','show',args.source_sha+':HanPage-Desktop/src-tauri/icons/icon.icns'],cwd=w)
expected=hashlib.sha256(icon).hexdigest()
def cmd(name,argv):
    c=subprocess.run(argv,capture_output=True,text=True,timeout=120)
    (logs/(name+'.log')).write_text(c.stdout+c.stderr)
    assert c.returncode==0,(name,c.returncode,c.stdout+c.stderr)
    return c.stdout+c.stderr
def appcheck(app,label):
    p=plistlib.loads((app/'Contents/Info.plist').read_bytes())
    assert p['CFBundleShortVersionString']==p['CFBundleVersion']=='0.8.9'
    assert p['CFBundleIdentifier']=='com.paldyn.hanpage'
    actual=sha(app/'Contents/Resources/icon.icns');assert actual==expected
    arch=cmd(label+'-arch',['file',str(app/'Contents/MacOS/HanPage')]);assert 'arm64' in arch
    cmd(label+'-codesign-verify',['codesign','--verify','--deep','--strict','--verbose=2',str(app)])
    identity=cmd(label+'-codesign-identity',['codesign','-dv','--verbose=4',str(app)])
    assert 'TeamIdentifier=8L78W6D8XF' in identity and 'Developer ID Application:' in identity
    gk=cmd(label+'-gatekeeper',['spctl','--assess','--type','execute','--verbose=4',str(app)])
    assert 'Notarized Developer ID' in gk
    cmd(label+'-stapler',['xcrun','stapler','validate',str(app)])
    return {'version':'0.8.9','bundle_identifier':p['CFBundleIdentifier'],'architecture':'arm64','icon_sha256':actual,'icon_matches_source':True,'source_sha':args.source_sha,'codesign_strict':True,'developer_id_team':'8L78W6D8XF','gatekeeper_notarized':True,'stapler_valid':True,'app_launched_or_installed':False}
unpack=o/'mac-archive';unpack.mkdir(exist_ok=True)
archive=r/'HanPage_aarch64.app.tar.gz'
with tarfile.open(archive) as t:t.extractall(unpack,filter='data')
apps=list(unpack.rglob('HanPage.app'));assert len(apps)==1
b=appcheck(apps[0],'archive');b['archive_sha256']=sha(archive);b['archive_bytes']=archive.stat().st_size
(o/'macos-bundle.json').write_text(json.dumps(b,indent=2)+'\n')
dmg=r/'HanPage_0.8.9_aarch64.dmg';cmd('dmg-integrity',['hdiutil','verify',str(dmg)])
mount=o/'dmg-mount';mount.mkdir(exist_ok=True)
mounted=False
try:
    cmd('dmg-attach',['hdiutil','attach','-readonly','-nobrowse','-mountpoint',str(mount),str(dmg)]);mounted=True
    d=appcheck(mount/'HanPage.app','dmg-app');d.update(dmg_sha256=sha(dmg),dmg_integrity=True,mounted_readonly=True)
finally:
    if mounted:cmd('dmg-detach',['hdiutil','detach',str(mount)])
info=cmd('hdiutil-info',['hdiutil','info']);assert str(dmg) not in info
d['mounted_volume_detached']=True
(o/'dmg.json').write_text(json.dumps(d,indent=2)+'\n')
print(json.dumps({'mac_archive':b,'dmg':d},indent=2))
