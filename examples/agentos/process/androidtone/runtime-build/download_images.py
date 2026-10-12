#!/usr/bin/env python3
"""Resumable bounded parallel range downloads; official SHA-256 remains authoritative."""
import concurrent.futures,hashlib,json,pathlib,time,urllib.request
APP=pathlib.Path(__file__).resolve().parents[1];CACHE=APP/'agentos-build/image-cache'
CHUNK=4*1024*1024

def fetch_image(item,workers=32):
    CACHE.mkdir(parents=True,exist_ok=True); target=CACHE/item['filename']
    def valid(p):
        with p.open('rb') as f:return p.stat().st_size==item['size'] and hashlib.file_digest(f,'sha256').hexdigest()==item['sha256']
    if target.exists() and valid(target):return
    # This is an official SourceForge mirror; content is pinned by the OTA digest.
    url=item['url'].replace('https://sourceforge.net/projects/waydroid/files/','https://zenlayer.dl.sourceforge.net/project/waydroid/').removesuffix('/download')
    parts=CACHE/(item['filename']+'.chunks');parts.mkdir(exist_ok=True)
    def part(index):
        start=index*CHUNK;end=min(item['size'],start+CHUNK)-1;size=end-start+1;p=parts/str(index)
        if p.exists() and p.stat().st_size==size:return p
        for attempt in range(5):
            try:
                req=urllib.request.Request(url,headers={'Range':f'bytes={start}-{end}','User-Agent':'AndroidTone-Offline-Builder/1'})
                with urllib.request.urlopen(req,timeout=180) as r:
                    if r.status!=206 or not r.headers.get('Content-Range','').startswith(f'bytes {start}-{end}/'):raise ValueError('Range response mismatch')
                    data=r.read(size+1)
                    if len(data)!=size:raise ValueError('Range length mismatch')
                tmp=p.with_suffix('.part');tmp.write_bytes(data);tmp.rename(p);return p
            except Exception:
                if attempt==4:raise
                time.sleep(min(2**attempt,15))
    count=(item['size']+CHUNK-1)//CHUNK;done=0
    with concurrent.futures.ThreadPoolExecutor(max_workers=workers) as e:
        for f in concurrent.futures.as_completed([e.submit(part,i) for i in range(count)]):
            f.result();done+=1
            if done%8==0 or done==count:print(item['kind'],done,'/',count,'chunks',flush=True)
    tmp=target.with_suffix('.zip.assembled')
    with tmp.open('wb') as out:
        for i in range(count):out.write((parts/str(i)).read_bytes())
    if not valid(tmp):raise ValueError('Official image SHA-256 mismatch')
    tmp.rename(target);print(item['kind'],'SHA256_VERIFIED',item['sha256'],flush=True)
if __name__=='__main__':
    lock=json.loads((APP/'runtime-build/images.arm64.lock.json').read_text())
    # Limit aggregate connections, do not run both images at full concurrency.
    for item in lock['images']:fetch_image(item)
