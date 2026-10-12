#!/usr/bin/env python3
"""Build an offline payload. No host package/service/kernel changes are made."""
import concurrent.futures,hashlib,json,pathlib,subprocess,zipfile,shutil
APP=pathlib.Path(__file__).resolve().parents[1]
OUT=APP/'agentos-build/androidtone-runtime'
CACHE=APP/'agentos-build/image-cache'
LOCK=json.loads((APP/'runtime-build/images.arm64.lock.json').read_text())
def digest(p):
    with p.open('rb') as f:return hashlib.file_digest(f,'sha256').hexdigest()
def download(item):
    from download_images import fetch_image
    fetch_image(item, workers=16)
    target=CACHE/item['filename']
    dest=OUT/'images'/str(item['kind']+'.img')
    with zipfile.ZipFile(target) as z:
        name=item['kind']+'.img'
        # Extract a single known file, never trust archive paths or symlinks.
        entry=z.getinfo(name)
        if entry.is_dir() or ((entry.external_attr>>16)&0o170000)==0o120000:raise ValueError('Unsafe image entry')
        # Always extract from the digest-verified archive; never bless a stale raw image cache.
        tmp=dest.with_suffix('.img.part')
        import shutil
        with z.open(entry) as src,tmp.open('wb') as dst:shutil.copyfileobj(src,dst,8*1024*1024)
        tmp.rename(dest)
    return dict(path='images/'+dest.name,kind=item['kind'],size=dest.stat().st_size,sha256=digest(dest),archive=item)
def main():
    for p in [OUT,CACHE,OUT/'images']:p.mkdir(parents=True,exist_ok=True)
    # Deb pool can be reused only after its final inventory was committed.
    with concurrent.futures.ThreadPoolExecutor(max_workers=2) as executor:
        futures=[executor.submit(download,item) for item in LOCK['images']]
        if not (OUT/'packages.complete').exists():
            subprocess.run(['docker','run','--rm','--platform','linux/arm64','--mount',f'type=bind,src={APP}/runtime-build,dst=/build,readonly','--mount',f'type=bind,src={OUT},dst=/out','ubuntu:24.04','sh','/build/build-debs.sh'],check=True)
        images=[f.result() for f in futures]
    subprocess.run(['docker','run','--rm','--network','none','--platform','linux/arm64',
        '--mount',f'type=bind,src={OUT},dst=/payload,readonly',
        '--mount',f'type=bind,src={APP}/runtime-build,dst=/build,readonly',
        'ubuntu:24.04','sh','/build/verify-offline.sh'],check=True)
    shutil.rmtree(OUT/'packages/partial',ignore_errors=True)
    (OUT/'packages/lock').unlink(missing_ok=True)
    (OUT/'provenance/offline-validation.json').write_text(json.dumps({
        'network':'none','architecture':'arm64','distribution':'ubuntu','version':'24.04',
        'localDebInstallation':'passed','aptDependencyCheck':'passed',
        'waydroidVersionCommand':'passed','lxcVersionCommand':'passed','pythonImports':['dbus','gi','gbinder'],
        'hostDeployment':'platform-dependency','windowPresentation':'unverified'
    },indent=2)+'\n')
    packages=json.loads((OUT/'packages.json').read_text())
    pinned=json.loads((APP/'runtime-build/packages.noble-arm64.lock.json').read_text())
    if packages!=pinned:raise ValueError('Cached package pool differs from pinned lock')
    for item in packages:
        p=OUT/item['path']
        if p.stat().st_size!=item['size'] or digest(p)!=item['sha256']:raise ValueError('Deb checksum mismatch')
    extras=[]
    for dirname in ['sources','provenance']:
        for p in sorted((OUT/dirname).rglob('*')):
            if p.is_file():extras.append(dict(path=str(p.relative_to(OUT)),size=p.stat().st_size,sha256=digest(p)))
    waydroid=next(p for p in packages if p['package']=='waydroid')
    catalog=dict(schemaVersion=1,id='com.agentos.androidtone.runtime.noble-arm64',architecture='arm64',host=dict(os='linux',distribution='ubuntu',version='24.04'),waydroidVersion=waydroid['version'],androidVersion=LOCK['androidVersion'],variant='VANILLA',deployment='platform-authorized-offline-packages',packages=packages,images=images,supplementary=extras)
    (OUT/'THIRD-PARTY-NOTICES.md').write_text('''# AndroidTone offline runtime\n\nWaydroid is GPL-3.0; Ubuntu binary packages retain their respective upstream licenses. Ubuntu corresponding source archives and version-pinned Waydroid upstream source archives are included under sources/. The upstream source commits and SHA-256 checksums are recorded under provenance/upstream-sources.lock.json. Package copyright files are installed under /usr/share/doc/<package>/copyright by dpkg. The APT archive signing keys and repository configuration are included under provenance/.\n\nAndroid 13 LineageOS VANILLA system and MAINLINE vendor images are official Waydroid builds, without Google Apps. The images contain upstream license notices. Corresponding Android/LineageOS build sources and manifests: https://github.com/waydroid/android , https://github.com/waydroid/android_device_waydroid_waydroid , https://github.com/LineageOS . Image release metadata and checksums are recorded in catalog.json. See https://docs.waydro.id/licensing/licensing for upstream licensing information.\n\nThis payload is data for a system-authorized deployment operation. It must not be executed or installed with elevated privileges by the ordinary Host Provider. Kernel modules, Binder device allocation, desktop session context and platform authorization are not supplied by this archive.\n''')
    notice=OUT/'THIRD-PARTY-NOTICES.md'
    catalog['supplementary'].append(dict(path=notice.name,size=notice.stat().st_size,sha256=digest(notice)))
    tmp=OUT/'catalog.json.part';tmp.write_text(json.dumps(catalog,indent=2)+'\n');tmp.replace(OUT/'catalog.json')
    print('RUNTIME_COMPLETE',waydroid['version'],len(packages),'packages',sum(p['size'] for p in packages+images+catalog['supplementary']),'bytes',flush=True)
if __name__=='__main__':main()
