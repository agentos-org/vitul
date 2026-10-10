#!/usr/bin/env python3
"""Extract a Docker-exported rootfs into an app bundle without host ownership."""

from __future__ import annotations

import os
import posixpath
import sys
import tarfile
from pathlib import Path, PurePosixPath


EXCLUDED = (
    "dev/",
    "proc/",
    "sys/",
    "run/",
    "tmp/",
    "root/",
    "var/log/",
    "var/tmp/",
    "var/cache/",
    "var/lib/apt/lists/",
    "usr/share/doc/",
    "usr/share/man/",
    "usr/share/info/",
    "var/www/onlyoffice/documentserver-example/",
)
EXCLUDED_FILES = {
    "etc/hosts",
    "etc/hostname",
    "etc/resolv.conf",
    "etc/mtab",
}


def normalized_name(name: str) -> str:
    while name.startswith("./"):
        name = name[2:]
    name = name.rstrip("/")
    path = PurePosixPath(name)
    if not name or path.is_absolute() or any(part in ("", ".", "..") for part in path.parts):
        raise ValueError(f"unsafe archive path: {name!r}")
    return path.as_posix()


def excluded(name: str) -> bool:
    if name in EXCLUDED_FILES:
        return True
    return any(name == prefix[:-1] or name.startswith(prefix) for prefix in EXCLUDED)


def is_case_sensitive_filesystem(root: Path) -> bool:
    probe = root / ".AgentOSCaseSensitivityProbe"
    alternate = root / ".agentosCaseSensitivityProbe"
    probe.write_text("probe", encoding="ascii")
    try:
        return not alternate.exists()
    finally:
        probe.unlink(missing_ok=True)


def casefold_sibling(path: Path) -> Path | None:
    folded_name = path.name.casefold()
    for sibling in path.parent.iterdir():
        if sibling.name != path.name and sibling.name.casefold() == folded_name:
            return sibling
    return None


def prune_dangling_symlinks(root: Path) -> int:
    """Drop image symlinks whose targets were intentionally omitted from the bundle."""
    removed = 0
    while True:
        paths = list(root.rglob("*"))
        entries = {path.relative_to(root).as_posix() for path in paths}
        dangling = []
        for path in paths:
            if not path.is_symlink():
                continue
            name = path.relative_to(root).as_posix()
            link = os.readlink(path)
            if link.startswith("/"):
                target = posixpath.normpath(link.lstrip("/"))
            else:
                target = posixpath.normpath(posixpath.join(posixpath.dirname(name), link))
            if target == ".." or target.startswith("../") or target not in entries:
                dangling.append(path)
        if not dangling:
            break
        for path in dangling:
            path.unlink()
            removed += 1
    if removed:
        print(f"removed {removed} dangling image symlink(s)", file=sys.stderr)
    return removed


def main() -> int:
    if len(sys.argv) != 3:
        print("usage: extract-rootfs.py <docker-export.tar|-> <destination>", file=sys.stderr)
        return 2

    source, destination = sys.argv[1:]
    root = Path(destination).resolve()
    root.mkdir(parents=True, exist_ok=True)
    case_sensitive_fs = is_case_sensitive_filesystem(root)
    deferred_hardlinks: list[tuple[Path, str]] = []
    directory_modes: dict[Path, int] = {}
    stream = sys.stdin.buffer if source == "-" else None

    with tarfile.open(source if stream is None else None, mode="r|", fileobj=stream) as archive:
        for member in archive:
            name = normalized_name(member.name)
            if excluded(name):
                continue

            target = root.joinpath(*PurePosixPath(name).parts)
            target.parent.mkdir(parents=True, exist_ok=True)

            if member.isdir():
                target.mkdir(parents=True, exist_ok=True)
                directory_modes[target] = member.mode & 0o777
                target.chmod((member.mode & 0o777) | 0o700)
            elif member.issym():
                link = member.linkname
                if not case_sensitive_fs:
                    collision = casefold_sibling(target)
                    if collision is not None:
                        if link.startswith("/"):
                            link_target = posixpath.normpath(link.lstrip("/"))
                        else:
                            link_target = posixpath.normpath(
                                posixpath.join(posixpath.dirname(name), link)
                            )
                        collision_name = collision.relative_to(root).as_posix()
                        if (
                            link_target.casefold() == collision_name.casefold()
                            and collision.is_file()
                            and not collision.is_symlink()
                        ):
                            # Keep the real file when a case-only alias (for
                            # example arial.ttf -> Arial.ttf) cannot coexist on
                            # the host filesystem.
                            continue
                        if collision.is_symlink():
                            collision.unlink()
                        else:
                            raise RuntimeError(
                                "host filesystem cannot represent case-distinct "
                                f"rootfs entries: {name!r} conflicts with "
                                f"{collision_name!r}"
                            )
                if target.exists() or target.is_symlink():
                    target.unlink()
                if link.startswith("/"):
                    # Docker rootfs links are absolute in the guest. Make them
                    # relative so they remain inside this app's bundled tree.
                    link = posixpath.relpath(link.lstrip("/"), posixpath.dirname(name) or ".")
                os.symlink(link, target)
            elif member.islnk():
                deferred_hardlinks.append((target, normalized_name(member.linkname)))
            elif member.isfile():
                if not case_sensitive_fs:
                    collision = casefold_sibling(target)
                    if collision is not None:
                        if collision.is_symlink():
                            collision.unlink()
                        else:
                            collision_name = collision.relative_to(root).as_posix()
                            raise RuntimeError(
                                "host filesystem cannot represent case-distinct "
                                f"rootfs entries: {name!r} conflicts with "
                                f"{collision_name!r}"
                            )
                data = archive.extractfile(member)
                if data is None:
                    continue
                if target.exists() or target.is_symlink():
                    target.unlink()
                flags = os.O_WRONLY | os.O_CREAT | os.O_TRUNC
                fd = os.open(target, flags, member.mode & 0o777)
                with os.fdopen(fd, "wb") as output:
                    while chunk := data.read(1024 * 1024):
                        output.write(chunk)
                # Do not carry setuid/setgid bits from the source image into an
                # app mounted in AgentOS' container.
                target.chmod(member.mode & 0o777)

    for target, link_name in deferred_hardlinks:
        source_path = root.joinpath(*PurePosixPath(link_name).parts)
        if source_path.is_file():
            try:
                os.link(source_path, target)
            except OSError:
                # A copy is safer than failing the whole bundle on filesystems
                # that do not support hardlinks.
                import shutil

                shutil.copyfile(source_path, target)
                target.chmod(source_path.stat().st_mode & 0o777)

    # The image exporter omits transient /run contents, but the stock Ubuntu
    # /var/run and /var/lock links should still resolve inside the bundled tree.
    (root / "run/lock").mkdir(parents=True, exist_ok=True)
    prune_dangling_symlinks(root)

    for directory, mode in sorted(directory_modes.items(), key=lambda item: len(item[0].parts), reverse=True):
        try:
            # Keep the package tree owner-writable so the local build can remove
            # its staging directory without needing elevated host permissions.
            # AgentOS mounts the installed app bundle read-only at runtime.
            directory.chmod(mode | 0o700)
        except OSError:
            pass

    for required in (
        "var/www/onlyoffice/documentserver/server/DocService/docservice",
        "var/www/onlyoffice/documentserver/server/FileConverter/converter",
        "usr/sbin/nginx",
        "usr/bin/supervisord",
        "etc/onlyoffice/documentserver/default.json",
        "etc/nginx/nginx.conf",
    ):
        if not root.joinpath(required).is_file():
            raise RuntimeError(f"Docker image is missing required runtime file: {required}")

    return 0


if __name__ == "__main__":
    try:
        raise SystemExit(main())
    except Exception as exc:
        print(f"rootfs extraction failed: {exc}", file=sys.stderr)
        raise
