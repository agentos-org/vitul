#!/usr/bin/env python3
import json,pathlib,re,subprocess
lock=pathlib.Path('/build/packages.noble-arm64.lock.json')
packages=json.loads(lock.read_text()) if lock.exists() else []
targets=[]
for item in packages:
    if not re.fullmatch(r'[a-z0-9][a-z0-9+.-]*',item['package']) or not re.fullmatch(r'[a-zA-Z0-9.+:~_-]+',item['version']):raise ValueError('Invalid Debian package lock')
    targets.append(item['package']+'='+item['version'])
subprocess.run(['apt-get','-y','--download-only','--no-install-recommends','-o','Dir::State::status=/tmp/empty-status','-o','Dir::Cache::archives=/out/packages','install']+(targets or ['waydroid','lxc','python3-gbinder','ca-certificates']),check=True)
