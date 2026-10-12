#!/usr/bin/env python3
"""Package Docker-built backend + verified runtime via Vitul's standard packager.
Useful when the standard backend builder cannot reach Cargo; never fabricates artifacts.
"""
import hashlib,json,pathlib,shutil,subprocess,tempfile,os,shlex
APP=pathlib.Path(__file__).resolve().parents[1]
VITUL=APP.parents[3]/'target/debug/vitul'
def main():
    subprocess.run(['bash',str(APP/'scripts/build-frontend.sh')],cwd=APP,check=True)
    config=json.loads((APP/'vitul.conf.json').read_text())
    runtime=APP/'agentos-build/androidtone-runtime'
    if not (runtime/'catalog.json').is_file():raise RuntimeError('Complete runtime catalog required')
    binary=APP/'agentos-build/bin/androidtone-server'
    subprocess.run(['docker','run','--rm','--network','none','--platform','linux/arm64',
      '--mount',f'type=bind,src={binary},dst=/app/androidtone-server,readonly',
      '--mount',f'type=bind,src={runtime},dst=/app/runtime,readonly',
      'ubuntu:24.04','/app/androidtone-server','--verify-runtime'],check=True)
    with tempfile.TemporaryDirectory(prefix='androidtone-package-') as tmp:
        stage=pathlib.Path(tmp)
        # Actual compilation and runtime build already completed; package exactly those outputs.
        config.pop('backend',None);config['agentos']['build']['steps']=[]
        for entry in config['agentos']['build']['artifacts']:
            src=APP/entry['from'];dest=stage/entry['from'];dest.parent.mkdir(parents=True,exist_ok=True)
            if src.is_dir():shutil.copytree(src,dest)
            else:shutil.copy2(src,dest)
        (stage/'vitul.conf.json').write_text(json.dumps(config,ensure_ascii=False,indent=2)+'\n')
        shutil.copy2(APP/'app.json',stage/'app.json')
        # bsdtar may keep xattrs even with COPYFILE_DISABLE; strip both forms at archive creation.
        system_tar=shutil.which('tar')
        if not system_tar:raise RuntimeError('tar required')
        bin_dir=stage/'.packaging-bin';bin_dir.mkdir()
        wrapper=bin_dir/'tar'
        wrapper.write_text('#!/bin/sh\nexec '+shlex.quote(system_tar)+' --no-xattrs "$@"\n')
        wrapper.chmod(0o755)
        subprocess.run([str(VITUL),'build','--target','agentos/arm64'],cwd=stage,check=True,env={**os.environ,'COPYFILE_DISABLE':'1','PATH':str(bin_dir)+os.pathsep+os.environ.get('PATH','')})
        artifact=next((stage/'dist').glob('*.agt'))
        # Current AgentOS package transport supports up to 2 GiB.
        if artifact.stat().st_size>2*1024**3:raise RuntimeError('Package exceeds AgentOS 2 GiB transport limit')
        output=APP/'dist'/artifact.name;output.parent.mkdir(exist_ok=True);shutil.copy2(artifact,output)
        with output.open('rb') as f:digest=hashlib.file_digest(f,'sha256').hexdigest()
        output.with_suffix('.agt.sha256').write_text(digest+'  '+output.name+'\n')
        # Keep old packages until the new archive passes full verification.
        subprocess.run(['python3',str(APP/'runtime-build/verify-package.py'),str(output)],check=True)
        for previous in output.parent.glob('androidtone-*-agentos-*.agt'):
            if previous == output:continue
            previous.unlink()
            previous.with_suffix('.agt.sha256').unlink(missing_ok=True)
        for checksum in output.parent.glob('androidtone-*-agentos-*.agt.sha256'):
            if not checksum.with_suffix('').exists():checksum.unlink()
        print('AGT_COMPLETE',output,output.stat().st_size,digest,flush=True)
if __name__=='__main__':main()
