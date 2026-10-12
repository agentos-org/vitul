#!/usr/bin/env python3
"""Check the real Rust service and all Vite entry assets under the Shell URL."""
import sys, urllib.request, urllib.error
from html.parser import HTMLParser
from urllib.parse import urlsplit
base = sys.argv[1].rstrip('/')
class Assets(HTMLParser):
    paths = set()
    def handle_starttag(self, tag, attrs):
        attrs = dict(attrs)
        path = attrs.get('src') if tag == 'script' else attrs.get('href') if tag == 'link' else None
        if path:
            assert path.startswith('/apps/androidtone/'), path
            self.paths.add(path)
def check(path, kind, content=b''):
    with urllib.request.urlopen(base + path, timeout=20) as response:
        assert response.status == 200, path
        assert kind in response.headers['Content-Type'], (path, response.headers)
        data = response.read()
        assert content in data, path
    print('PASS', path, flush=True)
    return data
check('/health', 'application/json', b'androidtone')
parser = Assets()
for path in ['/apps/androidtone/', '/apps/androidtone', '/apps/androidtone/index.html']:
    parser.feed(check(path, 'text/html', b'AndroidTone').decode())
parser.feed(check('/apps/androidtone/credits.html', 'text/html', b'AndroidTone').decode())
assert any(path.startswith('/apps/androidtone/assets/') and path.endswith('.js') for path in parser.paths), 'Expected Vite production JS; old frontend may have been restored'
assert any(path.startswith('/apps/androidtone/assets/') and path.endswith('.css') for path in parser.paths), 'Expected Vite production CSS'
for path in sorted(parser.paths):
    ext = urlsplit(path).path.rsplit('.', 1)[-1]
    check(path, {'js': 'javascript', 'css': 'text/css', 'png': 'image/png'}[ext])
check('/apps/androidtone/icon.png', 'image/png', b'\x89PNG')
check('/apps/androidtone/api/status', 'application/json', b'androidtone')
for path in ['/apps/androidtone/does-not-exist.png', '/apps/androidtone/assets/missing.js']:
    try:
        urllib.request.urlopen(base + path, timeout=10)
        raise AssertionError('Missing assets must return 404')
    except urllib.error.HTTPError as error:
        assert error.code == 404
print('FRONTEND_HTTP_VERIFIED', flush=True)
