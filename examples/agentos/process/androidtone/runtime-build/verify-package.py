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
for item in catalog['packages']+catalog['images']+catalog['supplementary']:
    if files.get('runtime/'+item['path'])!=(item['size'],item['sha256']):raise ValueError('Packaged runtime mismatch: '+item['path'])
for source in (APP/'public').rglob('*'):
    if source.is_file():
        with source.open('rb') as stream:frontend_digest=hashlib.file_digest(stream,'sha256').hexdigest()
        name='public/'+source.relative_to(APP/'public').as_posix()
        if files.get(name)!=(source.stat().st_size,frontend_digest):raise ValueError('Packaged frontend mismatch: '+name)
with package.open('rb') as stream:sha256=hashlib.file_digest(stream,'sha256').hexdigest()
report={'agtBytes':package.stat().st_size,'sha256':sha256,'files':len(files),'runtimeBytes':sum(item['size'] for item in catalog['packages']+catalog['images']+catalog['supplementary']),'packageCount':len(catalog['packages']),'imageCount':len(catalog['images']),'macOSMetadata':'absent','archivePathSafety':'passed','runtimeIntegrity':'all-sha256-verified','frontendIntegrity':'all-built-assets-sha256-verified','openMode':manifest['open']['mode'],'appVersion':manifest['version'],'architecture':catalog['architecture'],'host':catalog['host'],'offlineInstallation':'passed-by-runtime-builder','hostDeployment':'platform-dependency','windowPresentation':'unverified'}
destination=APP/'agentos-build/verification';destination.mkdir(exist_ok=True);(destination/'package-validation.json').write_text(json.dumps(report,indent=2)+'\n');print(json.dumps(report),flush=True)
