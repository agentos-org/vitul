#!/usr/bin/env python3
"""Exercise xdg initial-state requests on a disposable compositor as the current user.

No Android, root, AgentOS grant sockets, or desktop session is used. The only
socket this client connects to belongs to the compositor this script launches.
"""
import argparse
import json
import os
import pathlib
import resource
import socket
import struct
import subprocess
import tempfile
import threading
import time
import uuid


def u32(value):
    return struct.pack('<I', value)


def string(value):
    data = value.encode() + b'\0'
    return u32(len(data)) + data + b'\0' * (-len(data) % 4)


def message(obj, opcode, args=b''):
    return u32(obj) + u32(((8 + len(args)) << 16) | opcode) + args


class Client:
    def __init__(self, path):
        self.socket = socket.socket(socket.AF_UNIX)
        self.socket.settimeout(4)
        self.socket.connect(str(path))
        self.buffer = b''
        self.globals = {}

    def send(self, obj, opcode, args=b''):
        self.socket.sendall(message(obj, opcode, args))

    def sync(self, callback):
        self.send(1, 0, u32(callback))
        deadline = time.monotonic() + 4
        while True:
            if time.monotonic() >= deadline:
                raise TimeoutError('Wayland roundtrip exceeded four seconds')
            while len(self.buffer) < 8:
                self.socket.settimeout(max(0.001, deadline - time.monotonic()))
                block = self.socket.recv(65536)
                if not block:
                    raise EOFError('Compositor closed the Wayland connection')
                self.buffer += block
            obj, word = struct.unpack('<II', self.buffer[:8])
            size, opcode = word >> 16, word & 0xffff
            if size < 8 or size % 4:
                raise ValueError('Invalid Wayland message length')
            while len(self.buffer) < size:
                self.socket.settimeout(max(0.001, deadline - time.monotonic()))
                block = self.socket.recv(65536)
                if not block:
                    raise EOFError('Compositor closed mid-message')
                self.buffer += block
            args, self.buffer = self.buffer[8:size], self.buffer[size:]
            if obj == 2 and opcode == 0:  # wl_registry.global
                name, length = struct.unpack('<II', args[:8])
                interface = args[8:8 + length - 1].decode()
                version = struct.unpack_from('<I', args, 8 + ((length + 3) & ~3))[0]
                self.globals[interface] = (name, version)
            elif obj == 1 and opcode == 0:
                raise RuntimeError('wl_display.error: ' + repr(args))
            elif obj == 5 and opcode == 0:  # xdg_wm_base.ping
                self.send(5, 3, args)
            elif obj == 7 and opcode == 0:  # xdg_surface.configure
                self.send(7, 4, args)  # ack_configure
            elif obj == callback and opcode == 0:
                return

    def create_toplevel(self):
        self.send(1, 1, u32(2))  # get_registry
        self.sync(3)
        for interface, obj, maximum in [('wl_compositor', 4, 4), ('xdg_wm_base', 5, 1)]:
            name, version = self.globals[interface]
            self.send(2, 0, u32(name) + string(interface) + u32(min(version, maximum)) + u32(obj))
        self.send(4, 0, u32(6))  # create_surface
        self.send(5, 2, u32(7) + u32(6))  # get_xdg_surface
        self.send(7, 1, u32(8))  # get_toplevel


def run_case(binary, library_path, case):
    with tempfile.TemporaryDirectory(prefix='at-xdg-') as temporary:
        root = pathlib.Path(temporary)
        os.chmod(root, 0o700)
        frames = socket.socket(socket.AF_UNIX)
        frames.bind(str(root / 'f'))
        frames.listen(1)
        frames.settimeout(10)
        env = {'PATH': '/usr/bin:/bin', 'XDG_RUNTIME_DIR': str(root)}
        if library_path:
            env['LD_LIBRARY_PATH'] = library_path
        process = subprocess.Popen(
            [binary, '--frame-socket', str(root / 'f'), '--session-id', uuid.uuid4().hex],
            stdin=subprocess.PIPE, stdout=subprocess.PIPE, stderr=subprocess.PIPE,
            env=env, preexec_fn=lambda: resource.setrlimit(resource.RLIMIT_CORE, (0, 0)),
        )
        stdout, stderr = [], []
        def collect(stream, target):
            for line in iter(stream.readline, b''):
                target.append(line.decode(errors='replace').strip())
        def drain_frames():
            try:
                stream, _ = frames.accept()
                with stream:
                    while stream.recv(65536):
                        pass
            except (OSError, TimeoutError):
                pass
        readers = [threading.Thread(target=collect, args=(process.stdout, stdout), daemon=True),
                   threading.Thread(target=collect, args=(process.stderr, stderr), daemon=True),
                   threading.Thread(target=drain_frames, daemon=True)]
        for reader in readers:
            reader.start()
        error = None
        client = None
        try:
            deadline = time.monotonic() + 10
            while not any('"display.ready"' in line for line in stdout):
                if process.poll() is not None or time.monotonic() >= deadline:
                    raise RuntimeError('Disposable compositor did not become ready')
                time.sleep(0.02)
            process.stdin.write(b'{"type":"initialWindowSize","width":900,"height":680}\n')
            process.stdin.flush()
            client = Client(root / 'w')
            client.create_toplevel()
            if case.startswith('after-commit-'):
                client.send(6, 6)  # initial wl_surface.commit
                client.sync(9)
            state = case.removeprefix('after-commit-').removeprefix('before-commit-')
            if state == 'maximize':
                client.send(8, 9)
            elif state == 'unmaximize':
                client.send(8, 10)
            elif state == 'fullscreen':
                client.send(8, 11, u32(0))
            else:
                raise ValueError(case)
            # Wayland client object IDs must be allocated consecutively. The
            # after-commit case already consumed callback 9 above.
            client.sync(10 if case.startswith('after-commit-') else 9)
        except Exception as failure:
            error = str(failure)
            try:
                process.wait(timeout=1)
            except subprocess.TimeoutExpired:
                pass
        finally:
            exit_code = process.poll()
            if client:
                client.socket.close()
            if process.poll() is None:
                process.terminate()
            try:
                process.wait(timeout=2)
            except subprocess.TimeoutExpired:
                process.kill()
                process.wait(timeout=2)
            frames.close()
            for reader in readers[:2]:
                reader.join(timeout=1)
        return {'case': case, 'exitBeforeCleanup': exit_code, 'error': error,
                'stderr': stderr, 'compositorCrashed': exit_code is not None and exit_code != 0}


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--compositor', default='/opt/agentos-shell/labwc/bin/agentos-wayland-host')
    parser.add_argument('--library-path', default='')
    parser.add_argument('--output')
    args = parser.parse_args()
    if os.geteuid() == 0:
        parser.error('Run this isolated diagnostic as a normal user')
    results = [run_case(args.compositor, args.library_path, case) for case in (
        'after-commit-maximize', 'before-commit-maximize',
        'before-commit-unmaximize', 'before-commit-fullscreen')]
    result = {'compositor': args.compositor, 'results': results}
    if args.output:
        pathlib.Path(args.output).write_text(json.dumps(result, ensure_ascii=False, indent=2) + '\n')
    # Retain complete renderer logs in --output without flooding the terminal.
    summary = [{key: value for key, value in case.items() if key != 'stderr'} |
               {'assertions': [line for line in case['stderr'] if 'Assertion' in line]}
               for case in results]
    print(json.dumps({'compositor': args.compositor, 'results': summary},
                     ensure_ascii=False, indent=2))
    return int(any(case['error'] or case['compositorCrashed'] for case in results))


if __name__ == '__main__':
    raise SystemExit(main())
