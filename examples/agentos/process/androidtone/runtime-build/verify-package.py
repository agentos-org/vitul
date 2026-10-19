#!/usr/bin/env python3
"""Verify final archive paths and every declared runtime file without extraction."""
import hashlib,json,pathlib,tarfile,sys
APP=pathlib.Path(__file__).resolve().parents[1]
version=json.loads((APP/'vitul.conf.json').read_text())['version']
package=pathlib.Path(sys.argv[1]) if len(sys.argv)>1 else APP/'dist'/f'androidtone-{version}-agentos-arm64.agt'
files={};catalog=None;manifest=None
with tarfile.open(package,'r|gz') as archive:
    for entry in archive:
        name=entry.name.removeprefix('./');path=pathlib.PurePosixPath(name)
        if path.is_absolute() or '..' in path.parts or entry.issym() or entry.islnk() or not (entry.isfile() or entry.isdir()):raise ValueError('Unsafe archive entry: '+name)
        if any(part.startswith('._') for part in path.parts) or any('com.apple' in key for key in entry.pax_headers):raise ValueError('macOS metadata present: '+name)
        if not entry.isfile():continue
        if name in files:raise ValueError('Duplicate archive file: '+name)
        stream=archive.extractfile(entry)
        if name in ['runtime/catalog.json','app.json']:
            if entry.size>4*1024*1024:raise ValueError('Oversized manifest')
            data=stream.read();digest=hashlib.sha256(data).hexdigest();document=json.loads(data)
            if name=='runtime/catalog.json':catalog=document
            else:manifest=document
        else:digest=hashlib.file_digest(stream,'sha256').hexdigest()
        files[name]=(entry.size,digest)
if not catalog or not manifest or manifest['architecture']!='arm64':raise ValueError('Missing runtime or wrong architecture')
if manifest.get('version')!=version:raise ValueError('Packaged app version differs from build configuration')
if manifest.get('open',{}).get('mode')!='window':raise ValueError('AndroidTone must be packaged in window mode')
if manifest.get('execution',{}).get('mode')!='native' or manifest.get('execution',{}).get('native',{}).get('lastWindowClose')!='background':raise ValueError('AndroidTone must use the desktop-user native background profile')
if manifest.get('run',{}).get('port')!=28642:raise ValueError('AndroidTone backend port must remain 28642')
if 'native.tray' not in manifest.get('permissions',[]):raise ValueError('AndroidTone tray permission missing')
operations=manifest.get('deployment',{}).get('operations',[])
setup=next((op for op in operations if op.get('id')=='android-runtime-setup'),None)
if not setup or setup.get('version')!=3 or setup.get('parameters')!={}:raise ValueError('Runtime deployment must use fixed android-runtime-setup version 3')
package_actions=[action for action in setup.get('actions',[]) if action.get('type')=='debian.packages.ensure']
if len(package_actions)!=1:raise ValueError('Runtime deployment must use one debian.packages.ensure action')
package_action=package_actions[0]
if package_action.get('schemaVersion')!=1:raise ValueError('Runtime deployment must declare debian.packages.ensure schemaVersion 1')
assets={item['package']:item for item in package_action.get('assets',[])}
assets_by_id={item['id']:item for item in package_action.get('assets',[])}
requirements=package_action.get('requirements',[])
exact={assets_by_id[item['assetId']]['package']:item for item in requirements if item.get('match')=='exact' and item.get('assetId') in assets_by_id}
if set(exact)!={'waydroid','androidtone-waydroid-images'}:raise ValueError('Waydroid and its image component must remain exact application requirements')
if any(item.get('role')!='application' for item in exact.values()):raise ValueError('AndroidTone runtime components must use application role')
host_shared=[item for item in requirements if item.get('role')=='host-shared' and item.get('match')=='dependency']
if not host_shared or any(not item.get('candidateAssetIds') or not item.get('depends') for item in host_shared):raise ValueError('Shared Debian dependencies must declare compatibility expressions and offline candidates')
if sum(len(item['candidateAssetIds']) for item in host_shared)!=len(assets)-len(exact):raise ValueError('Every shared package asset must be selected by a dependency requirement')
if any(item.get('package') in {'waydroid','androidtone-waydroid-images'} for item in assets.values() if item['package'] not in exact):raise ValueError('Runtime application package was not bound exactly')
if len(json.dumps(setup.get('actions',[]),separators=(',',':')).encode())>64*1024:raise ValueError('Deployment actions exceed AgentOS review size limit')
recovery=next((op for op in operations if op.get('id')=='android-runtime-recover'),None)
expected_recovery=[{'type':'linux.module.load','name':'binder_linux'}, {'type':'linux.binderfs.ensure','id':'androidtone-binder','devices':['binder','hwbinder','vndbinder']}, {'type':'systemd.service.control','unit':'waydroid-container.service','operation':'restart'}]
if not recovery or recovery.get('version')!=1 or recovery.get('parameters')!={}:
    raise ValueError('Missing fixed Android runtime recovery operation')
# Platform normalization may add an empty module parameter map.
actions=[{k:v for k,v in action.items() if not (k=='parameters' and v=={})} for action in recovery.get('actions',[])]
if actions!=expected_recovery:raise ValueError('Recovery must only prepare Binder and restart the fixed Waydroid service')
if any(a['type'].startswith('linux.lxc.') for op in operations for a in op.get('actions',[])):
    raise ValueError('AndroidTone must not request LXC platform actions')
binary=APP/'agentos-build/bin/androidtone-server'
with binary.open('rb') as stream:backend_digest=hashlib.file_digest(stream,'sha256').hexdigest()
if files.get('androidtone-server')!=(binary.stat().st_size,backend_digest):raise ValueError('Packaged backend differs from compiled binary')
if (catalog.get('imagesPackage')!='androidtone-waydroid-images' or len(catalog['images'])!=2
    or catalog.get('imageArchitecture')!='arm64_only' or catalog.get('aarch32El0Required') is not False
    or catalog.get('imagePackageVersion')!='0.1.8-1'):
    raise ValueError('Waydroid image installer package is missing')
image_package=next((item for item in catalog['packages'] if item.get('package')=='androidtone-waydroid-images'),None)
if not image_package or image_package.get('version')!='0.1.8-1':raise ValueError('Waydroid image package version is not current')
if assets['androidtone-waydroid-images'].get('version')!='0.1.8-1':raise ValueError('The image package must remain precisely bound to its packaged version')
for item in catalog['packages']+catalog['supplementary']:
    if files.get('runtime/'+item['path'])!=(item['size'],item['sha256']):raise ValueError('Packaged runtime mismatch: '+item['path'])
for source in (APP/'public').rglob('*'):
    if source.is_file():
        with source.open('rb') as stream:frontend_digest=hashlib.file_digest(stream,'sha256').hexdigest()
        name='public/'+source.relative_to(APP/'public').as_posix()
        if files.get(name)!=(source.stat().st_size,frontend_digest):raise ValueError('Packaged frontend mismatch: '+name)
with package.open('rb') as stream:sha256=hashlib.file_digest(stream,'sha256').hexdigest()
report={'agtBytes':package.stat().st_size,'sha256':sha256,'files':len(files),'runtimeBytes':sum(item['size'] for item in catalog['packages']+catalog['images']+catalog['supplementary']),'packageCount':len(catalog['packages']),'imageCount':len(catalog['images']),'macOSMetadata':'absent','archivePathSafety':'passed','runtimeIntegrity':'all-sha256-verified','frontendIntegrity':'all-built-assets-sha256-verified','openMode':manifest['open']['mode'],'appVersion':manifest['version'],'architecture':catalog['architecture'],'host':catalog['host'],'offlineInstallation':'not-run-by-archive-verifier','hostDeployment':'awaiting-agentos-authorization','windowPresentation':'unverified'}
destination=APP/'agentos-build/verification';destination.mkdir(exist_ok=True);(destination/'package-validation.json').write_text(json.dumps(report,indent=2)+'\n');print(json.dumps(report),flush=True)
