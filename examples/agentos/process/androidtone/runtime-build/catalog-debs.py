#!/usr/bin/env python3
import hashlib,json,pathlib,subprocess,sys,re
root=pathlib.Path(sys.argv[1]); entries=[]; sources=set()
for p in sorted((root/'packages').glob('*.deb')):
    fields=subprocess.check_output(['dpkg-deb','-f',str(p),'Package','Version','Architecture','Source'],text=True)
    d=dict(line.split(': ',1) for line in fields.splitlines() if ': ' in line)
    name=d['Package']; version=d['Version']; source=d.get('Source',name)
    match=re.fullmatch(r'([^ ]+)(?: \(([^)]+)\))?',source)
    sources.add((match[1],match[2] or version))
    entries.append(dict(path='packages/'+p.name,package=name,version=version,architecture=d['Architecture'],size=p.stat().st_size,sha256=hashlib.file_digest(p.open('rb'),'sha256').hexdigest()))
lock=pathlib.Path('/build/packages.noble-arm64.lock.json')
if lock.exists() and entries!=json.loads(lock.read_text()):raise ValueError('Offline package pool differs from pinned lock')
(root/'packages.json').write_text(json.dumps(entries,indent=2)+'\n')
# Keep corresponding distro sources together with redistributable binary packages.
import concurrent.futures,urllib.request
upstream=json.loads(pathlib.Path('/build/upstream-sources.lock.json').read_text())
known={(item['source'],item['version']):item for item in upstream}
(root/'provenance/upstream-sources.lock.json').write_text(json.dumps(upstream,indent=2)+'\n')
def download_source(pair):
    name,version=pair
    if pair in known:
        item=known[pair];path=root/'sources'/item['filename']
        def verified():
            return path.exists() and path.stat().st_size==item['size'] and hashlib.file_digest(path.open('rb'),'sha256').hexdigest()==item['sha256']
        if not verified():
            data=urllib.request.urlopen(item['url'],timeout=120).read()
            if len(data)!=item['size'] or hashlib.sha256(data).hexdigest()!=item['sha256']:raise ValueError('Upstream source checksum mismatch: '+name)
            path.write_bytes(data)
        print('UPSTREAM_SOURCE_READY',name,version,flush=True)
        return
    result=subprocess.run(['apt-get','source','--download-only',name+'='+version],cwd=root/'sources',capture_output=True,text=True)
    if result.returncode:raise RuntimeError(name+' source download failed: '+result.stdout+'\n'+result.stderr)
    print('SOURCE_READY',name,version,flush=True)
with concurrent.futures.ThreadPoolExecutor(max_workers=8) as executor:
    failures=[]
    for future in concurrent.futures.as_completed([executor.submit(download_source,pair) for pair in sorted(sources)]):
        try:future.result()
        except Exception as error:
            print('SOURCE_ERROR',str(error),flush=True);failures.append(str(error))
    if failures:raise RuntimeError('Corresponding source downloads incomplete: '+'; '.join(failures))
(root/'packages.complete').write_text('complete\n')
print('OFFLINE_PACKAGES',len(entries),'SOURCE_PACKAGES',len(sources),flush=True)
