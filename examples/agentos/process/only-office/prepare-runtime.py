#!/usr/bin/env python3
"""Create writable, non-root ONLYOFFICE configuration for an AgentOS app."""

from __future__ import annotations

import json
import os
import re
import secrets
import shutil
import sys
from pathlib import Path


def quote_nginx(value: Path | str) -> str:
    return '"' + str(value).replace("\\", "\\\\").replace('"', '\\"') + '"'


def stable_secret(state: Path, filename: str, size: int = 48) -> str:
    path = state / filename
    if path.is_file():
        value = path.read_text(encoding="utf-8").strip()
        if len(value) >= 32:
            return value
    value = secrets.token_urlsafe(size)
    path.write_text(value + "\n", encoding="utf-8")
    path.chmod(0o600)
    return value


def replace_json(path: Path, update) -> None:
    value = json.loads(path.read_text(encoding="utf-8"))
    update(value)
    temporary = path.with_suffix(path.suffix + ".tmp")
    temporary.write_text(json.dumps(value, ensure_ascii=False, indent=2) + "\n", encoding="utf-8")
    temporary.replace(path)


def rewrite_json_paths(value, replacements: tuple[tuple[str, str], ...]):
    if isinstance(value, dict):
        return {key: rewrite_json_paths(item, replacements) for key, item in value.items()}
    if isinstance(value, list):
        return [rewrite_json_paths(item, replacements) for item in value]
    if isinstance(value, str):
        for index, (source, destination) in enumerate(replacements):
            # Some destination paths contain the source path as a suffix,
            # such as /opt/agentos/app/deps/etc/onlyoffice/documentserver.
            # Collapse copies written by older releases before replacing raw
            # image paths, so repeated starts and upgrades stay idempotent.
            source_index = destination.find(source)
            if source_index >= 0:
                prefix = destination[:source_index]
                duplicated = prefix + destination
                while duplicated in value:
                    value = value.replace(duplicated, destination)

                marker = f"\x00AGENTOS_PATH_{index}\x00"
                value = value.replace(destination, marker)
                value = value.replace(source, destination)
                value = value.replace(marker, destination)
            else:
                value = value.replace(source, destination)
    return value


def main() -> int:
    if len(sys.argv) != 4:
        print("usage: prepare-runtime.py <deps-root> <data-dir> <app-dir>", file=sys.stderr)
        return 2

    runtime, data, app = (Path(value).resolve() for value in sys.argv[1:])
    state = data / "onlyoffice"
    config = state / "config"
    nginx_state = state / "nginx"
    logs = state / "logs"
    writable = (
        state,
        config,
        nginx_state / "includes",
        state / "run",
        state / "tmp",
        state / "tmp" / "fontconfig",
        state / "tmp" / "client-body",
        state / "tmp" / "proxy",
        state / "tmp" / "fastcgi",
        state / "tmp" / "uwsgi",
        state / "tmp" / "scgi",
        state / "storage",
        state / "web-apps",
        state / "lib" / "App_Data" / "cache" / "files",
        state / "lib" / "App_Data" / "docbuilder",
        logs,
    )
    for directory in writable:
        directory.mkdir(parents=True, exist_ok=True)
        directory.chmod(0o700)

    source_config = runtime / "etc/onlyoffice/documentserver"
    local_config = config / "onlyoffice"
    if not (local_config / "default.json").is_file():
        shutil.copytree(source_config, local_config, symlinks=True, dirs_exist_ok=True)

    jwt_secret = stable_secret(state, "jwt-secret")
    storage_secret = stable_secret(state, "storage-secret")
    # Docs signs /cache/files URLs with storage.fs.secretString. Nginx must
    # verify those URLs with the same secret or every Editor.bin request gets
    # a 403 even though the file exists.
    secure_secret = storage_secret

    def update_local(value):
        coauthoring = value.setdefault("services", {}).setdefault("CoAuthoring", {})
        coauthoring["secret"] = {
            key: {"string": jwt_secret}
            for key in ("browser", "inbox", "outbox", "session")
        }
        coauthoring["token"] = {
            "enable": {"browser": True, "request": {"inbox": True, "outbox": True}}
        }
        coauthoring.setdefault("request-filtering-agent", {})["allowPrivateIPAddress"] = True
        value.setdefault("storage", {}).setdefault("fs", {})["folderPath"] = str(state / "storage")
        value["storage"]["fs"]["secretString"] = storage_secret

    replace_json(local_config / "local.json", update_local)

    api_template = (
        runtime
        / "var/www/onlyoffice/documentserver/web-apps/apps/api/documents/api.js.tpl"
    )
    api_script = state / "web-apps/api.js"
    api_script.write_text(
        api_template.read_text(encoding="utf-8").replace("{{HASH_POSTFIX}}", "agentos"),
        encoding="utf-8",
    )

    path_replacements = (
        (
            "/var/www/onlyoffice/documentserver/../Data",
            str(state / "docs-data"),
        ),
        (
            "/var/lib/onlyoffice/documentserver/App_Data",
            str(state / "lib" / "App_Data"),
        ),
        (
            "/var/www/onlyoffice/documentserver",
            str(runtime / "var/www/onlyoffice/documentserver"),
        ),
        (
            "/etc/onlyoffice/documentserver",
            str(runtime / "etc/onlyoffice/documentserver"),
        ),
        ("/var/log/onlyoffice/documentserver", str(logs)),
        ("/usr/share/fonts", str(runtime / "usr/share/fonts")),
    )
    for config_file in local_config.rglob("*.json"):
        value = json.loads(config_file.read_text(encoding="utf-8"))
        value = rewrite_json_paths(value, path_replacements)
        temporary = config_file.with_suffix(config_file.suffix + ".tmp")
        temporary.write_text(json.dumps(value, ensure_ascii=False, indent=2) + "\n", encoding="utf-8")
        temporary.replace(config_file)

    # Fonts and caches must remain writable while the executable bundle is mounted read-only.
    fontconfig_source = runtime / "etc/fonts/fonts.conf"
    if fontconfig_source.is_file():
        fontconfig = fontconfig_source.read_text(encoding="utf-8")
        fontconfig = fontconfig.replace("/usr/share/fonts", str(runtime / "usr/share/fonts"))
        fontconfig = fontconfig.replace("/etc/fonts", str(runtime / "etc/fonts"))
        fontconfig = fontconfig.replace("/var/cache/fontconfig", str(state / "tmp/fontconfig"))
        (state / "fonts.conf").write_text(fontconfig, encoding="utf-8")

    def rewrite_nginx_paths(text: str) -> str:
        doc_root = runtime / "var/www/onlyoffice/documentserver"
        cache_root = state / "lib/App_Data"
        log_root = state / "logs"
        text = text.replace(
            "/var/www/onlyoffice/documentserver/",
            str(doc_root) + "/",
        )
        text = text.replace(
            "/var/lib/onlyoffice/documentserver/App_Data",
            str(cache_root),
        )
        text = text.replace(
            "/var/log/onlyoffice/documentserver",
            str(log_root),
        )
        text = text.replace("localhost:8000", "127.0.0.1:8000")
        text = re.sub(
            r'(?m)^\s*error_log\s+("[^"]+"|\S+)(?:\s+(\w+))?\s*;',
            lambda match: "error_log "
            + quote_nginx(match.group(1).strip('"'))
            + (" " + match.group(2) if match.group(2) else "")
            + ";",
            text,
        )
        text = re.sub(
            rf"(?m)^\s*alias\s+{re.escape(str(doc_root))}/\$2;",
            f"  alias {state}/web-apps/api.js;",
            text,
            count=1,
        )
        text = re.sub(
            r"(?m)^\s*alias\s+([^;]+);",
            lambda match: "  alias " + quote_nginx(match.group(1)) + ";",
            text,
        )
        if any(
            line.lstrip().startswith("location ")
            and "cache" in line.lower()
            and "files" in line.lower()
            for line in text.splitlines()
        ):
            lines = text.splitlines()
            cache_location = next(
                (
                    index
                    for index, line in enumerate(lines)
                    if line.lstrip().startswith("location ")
                    and "cache" in line.lower()
                    and "files" in line.lower()
                ),
                None,
            )
            if cache_location is None:
                raise RuntimeError("ONLYOFFICE cache-files Nginx location is missing")
            cache_alias = next(
                (
                    index
                    for index in range(cache_location + 1, min(cache_location + 5, len(lines)))
                    if lines[index].lstrip().startswith("alias ")
                ),
                None,
            )
            if cache_alias is None:
                raise RuntimeError("ONLYOFFICE cache-files Nginx alias is missing")

            # Docs stores generated Editor.bin and conversion files under
            # storage/data/<key>, while its public URLs start with
            # /cache/files/data/<key>/<file>/<download-name>. Strip the URL
            # prefix and keep the first filename as the on-disk file path.
            lines[cache_location] = r"location ~* ^\/cache\/files(\/data.*)(\/.*) {"
            lines[cache_alias] = "  alias " + quote_nginx(str(state / "storage") + "$1") + ";"
            text = "\n".join(lines) + "\n"
        return text

    nginx_includes = runtime / "etc/nginx/includes"
    for name in ("http-common.conf", "ds-common.conf", "ds-docservice.conf", "ds-mime.types.conf"):
        source = nginx_includes / name
        (nginx_state / ("includes/" + name)).write_text(
            rewrite_nginx_paths(source.read_text(encoding="utf-8")),
            encoding="utf-8",
        )

    http_common = quote_nginx(nginx_state / "includes/http-common.conf")
    ds_common = quote_nginx(nginx_state / "includes/ds-common.conf")
    ds_service = quote_nginx(nginx_state / "includes/ds-docservice.conf")
    ds_mime = quote_nginx(nginx_state / "includes/ds-mime.types.conf")
    mime_types = quote_nginx(runtime / "etc/nginx/mime.types")
    nginx_conf = f"""worker_processes 1;
pid {quote_nginx(state / 'run/nginx.pid')};
error_log {quote_nginx(logs / 'nginx-error.log')} warn;

events {{
    worker_connections 512;
}}

http {{
    include {mime_types};
    default_type application/octet-stream;
    sendfile on;
    tcp_nopush on;
    server_tokens off;
    access_log off;
    client_body_temp_path {quote_nginx(state / 'tmp/client-body')};
    proxy_temp_path {quote_nginx(state / 'tmp/proxy')};
    fastcgi_temp_path {quote_nginx(state / 'tmp/fastcgi')};
    uwsgi_temp_path {quote_nginx(state / 'tmp/uwsgi')};
    scgi_temp_path {quote_nginx(state / 'tmp/scgi')};
    include {http_common};

    server {{
        listen 127.0.0.1:8080;
        server_name localhost;
        set $cache_tag agentos;
        set $secure_link_secret {secure_secret};
        include {ds_common};
        include {ds_service};
        include {ds_mime};
    }}
}}
"""
    (nginx_state / "nginx.conf").write_text(nginx_conf, encoding="utf-8")

    return 0


if __name__ == "__main__":
    try:
        raise SystemExit(main())
    except Exception as exc:
        print(f"ONLYOFFICE runtime configuration failed: {exc}", file=sys.stderr)
        raise
