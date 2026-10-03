from pathlib import Path
from urllib.request import urlopen, Request
from urllib.parse import urljoin
from html.parser import HTMLParser
import json,hashlib,datetime,struct

site='https://hanpage.paldyn.com/'
root=Path('/Users/lwm/.codex/worktrees/upstream-sync-final-20261001/HanPage')
ico='c4bdbc5ab66b05b549c6c4113b030133c1691450c68bb8183232b516efde2b09'
png={128:'50bf5565ac8b2bf07ee3c31ac50ed0d9515e8aa4b239b94938c54e676dace5f5',192:'89d3e2cd23b2d5a50a7f68105cd5117ae0ddb6c9b957982845a1579bb0c36f26',256:'ba4aa7a59ac3fa4fe4dd24e844e9c670a1b1cdc0a765e287ac7f528206433af3',512:'5a3dd9f2596e8cdaf8ab398e015823b3d83f1bcac2453337aff7d36d8dc6c7fd'}
result={'observedAt':datetime.datetime.now(datetime.timezone.utc).isoformat(),'url':site,'expectedSource':'6dc1a2d55cfb0752b4688d8f303a6f802cf49b5d','scope':'Public regular URLs and byte hashes. Actual legacy service-worker upgrade is a separate owned-browser check.','responses':[],'checks':[]}
def fetch(path):
    url=urljoin(site,path)
    with urlopen(Request(url,headers={'User-Agent':'HanPage-authorized-release-verification'}),timeout=25) as response:
        data=response.read()
        record={'url':url,'finalUrl':response.url,'status':response.status,'bytes':len(data),'sha256':hashlib.sha256(data).hexdigest(),'headers':{k:v for k,v in response.headers.items() if k.lower() in ['content-type','cache-control','etag','last-modified','age','x-cache','date']}}
        result['responses'].append(record)
        assert response.status==200
        return data,record
class Links(HTMLParser):
    links=[]
    def handle_starttag(self,tag,attributes):
        if tag=='link': self.links.append(dict(attributes))
html,htmlrecord=fetch('/')
parser=Links();parser.feed(html.decode())
favicons=[x for x in parser.links if x.get('rel')=='icon']
assert len(favicons)==1
favicon=favicons[0]['href']
assert favicon=='/assets/hanpage-favicon-DGxGu2KG.ico',favicon
result['faviconHref']=favicon
data,record=fetch(favicon);assert record['sha256']==ico
assert struct.unpack('<HHH',data[:6])==(0,1,7)
_,record=fetch('/favicon.ico');assert record['sha256']==ico
manifestlink=next(x['href'] for x in parser.links if x.get('rel')=='manifest')
manifestdata,_=fetch(manifestlink);manifest=json.loads(manifestdata)
assert len(manifest['icons'])==5
result['manifestIcons']=manifest['icons']
for icon in manifest['icons']:
    size=int(icon['sizes'].split('x')[0]);assert icon['src']==f'icons/hanpage-04-{size}.png'
for size,sha in png.items():
    for prefix in ['hanpage-04','icon']:
        data,record=fetch(f'/icons/{prefix}-{size}.png')
        assert record['sha256']==sha
        assert data[:8]==b'\x89PNG\r\n\x1a\n' and struct.unpack('>II',data[16:24])==(size,size)
apple=next(x['href'] for x in parser.links if x.get('rel')=='apple-touch-icon')
assert apple=='/icons/icon-256.png'
sw,swrecord=fetch('/sw.js')
for filename in ['hanpage-favicon-DGxGu2KG.ico']+[f'hanpage-04-{size}.png' for size in png]:
    assert filename in sw.decode(),filename
result['appleTouchHref']=apple
result['serviceWorkerSha256']=swrecord['sha256']
result['checks']=['Single content-hashed new favicon link','Both favicon URLs match approved seven-frame Desktop ICO','All five manifest entries use the new versioned PNGs','All eight versioned and legacy PNGs match approved hashes and dimensions','Apple touch retains the supported new 256px alias','Published service worker precaches the new favicon and icon URLs']
result['status']='pass'
out=root/'output/pr-review/web-favicon-20261003/public-http-after.json'
out.write_text(json.dumps(result,ensure_ascii=False,indent=2)+'\n')
print(json.dumps({'status':result['status'],'checks':result['checks'],'faviconHref':favicon,'responseCount':len(result['responses']),'output':str(out)},ensure_ascii=False,indent=2))
