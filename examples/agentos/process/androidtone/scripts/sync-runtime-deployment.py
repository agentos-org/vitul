#!/usr/bin/env python3
"""Create the fixed AndroidTone AgentOS ensure declaration from its catalog."""
import hashlib
import json
import os
import pathlib
import subprocess
import sys
import tarfile
from io import BytesIO


APP = pathlib.Path(__file__).resolve().parents[1]
CATALOG_PATH = APP / "agentos-build/androidtone-runtime/catalog.json"
CONFIG_PATH = APP / "vitul.conf.json"
RUNTIME_ROOT = APP / "agentos-build/androidtone-runtime"
MAX_PACKAGES = 512
MAX_ACTION_BYTES = 64 * 1024
APPLICATION_COMPONENTS = {"waydroid", "androidtone-waydroid-images"}


def sha256(path: pathlib.Path) -> str:
    digest = hashlib.sha256()
    with path.open("rb") as stream:
        for block in iter(lambda: stream.read(1024 * 1024), b""):
            digest.update(block)
    return digest.hexdigest()


def waydroid_dependencies(package_file: pathlib.Path) -> str:
    control_archive = subprocess.check_output(["ar", "p", str(package_file), "control.tar.zst"])
    decompressed = subprocess.check_output(["zstd", "-dc"], input=control_archive)
    with tarfile.open(fileobj=BytesIO(decompressed), mode="r:") as archive:
        member = next((item for item in archive.getmembers() if item.name.lstrip("./") == "control"), None)
        if member is None:
            raise ValueError("Waydroid Debian control record is missing.")
        control_bytes = archive.extractfile(member).read().decode("utf-8")
    fields = {}
    current = None
    for line in control_bytes.splitlines():
        if line[:1].isspace() and current:
            fields[current] += " " + line.strip()
        elif ":" in line:
            current, value = line.split(":", 1)
            fields[current] = value.strip()
    dependencies = [fields[field] for field in ("Pre-Depends", "Depends") if fields.get(field)]
    if not dependencies:
        raise ValueError("Waydroid package does not declare host dependencies.")
    return ", ".join(dependencies)


def deployment_assets(packages: list[dict]) -> tuple[list[dict], list[dict]]:
    """Use short stable archive paths so the complete reviewed action fits 64 KiB."""
    # Keep bundle inventory paths under packages/ because the runtime verifier
    # deliberately rejects Debian archives outside that directory.
    asset_root = RUNTIME_ROOT / "packages"
    asset_root.mkdir(exist_ok=True)
    assets = []
    requirements = []
    host_shared_asset_ids = []
    waydroid_asset_file = None
    names = set()

    for index, item in enumerate(sorted(packages, key=lambda package: package["package"])):
        name = item.get("package")
        version = item.get("version")
        relative = pathlib.PurePosixPath(item.get("path", ""))
        if not isinstance(name, str) or not name or name in names:
            raise ValueError(f"Invalid or duplicate Debian package name: {name!r}")
        names.add(name)
        if not isinstance(version, str) or not version:
            raise ValueError(f"Missing pinned package version for {name}.")
        if relative.is_absolute() or ".." in relative.parts or not relative.parts:
            raise ValueError(f"Unsafe runtime package path for {name}: {relative}")
        if item.get("architecture") not in ("arm64", "all"):
            raise ValueError(f"Unexpected package architecture for {name}.")

        source = RUNTIME_ROOT / pathlib.Path(*relative.parts)
        target_relative = pathlib.PurePosixPath("packages") / f"p{index}.deb"
        target = RUNTIME_ROOT / pathlib.Path(*target_relative.parts)
        for candidate in (source, target):
            if candidate.is_symlink() or not candidate.resolve().is_relative_to(RUNTIME_ROOT.resolve()):
                raise ValueError(f"Unsafe runtime package file for {name}: {candidate}")
        if source != target:
            if source.is_file():
                if source.stat().st_size != item.get("size") or sha256(source) != item.get("sha256"):
                    raise ValueError(f"Runtime package integrity check failed: {relative}")
                if target.exists():
                    if target.stat().st_size != item.get("size") or sha256(target) != item.get("sha256"):
                        raise ValueError(f"Conflicting compact runtime package file: {target_relative}")
                    source.unlink()
                else:
                    os.replace(source, target)
            elif not target.is_file():
                raise ValueError(f"Runtime package is missing: {relative}")
        if not target.is_file() or target.stat().st_size != item.get("size") or sha256(target) != item.get("sha256"):
            raise ValueError(f"Runtime package integrity check failed: {target_relative}")

        asset_id = f"p{index}"
        item["path"] = target_relative.as_posix()
        assets.append({
            "id": asset_id,
            "package": name,
            "version": version,
            "architecture": item["architecture"],
            "path": f"runtime/{target_relative.as_posix()}",
            "sha256": item["sha256"],
        })
        if name in APPLICATION_COMPONENTS:
            requirements.append({
                "id": f"app-{name}",
                "role": "application",
                "match": "exact",
                "assetId": asset_id,
            })
            if name == "waydroid":
                waydroid_asset_file = target
        else:
            host_shared_asset_ids.append(asset_id)

    missing = APPLICATION_COMPONENTS.difference(names)
    if missing:
        raise ValueError(f"Missing exact AndroidTone components: {sorted(missing)}")
    if not 1 <= len(assets) <= MAX_PACKAGES:
        raise ValueError(f"Runtime catalog must contain between 1 and {MAX_PACKAGES} packages.")

    if waydroid_asset_file is None:
        raise ValueError("Waydroid package asset is missing.")
    dependencies = waydroid_dependencies(waydroid_asset_file)
    if len(dependencies) > 1024:
        raise ValueError("Waydroid host dependency expression exceeds AgentOS's 1024-character limit.")
    requirements.append({
            "id": "waydroid-host-dependencies",
            "role": "host-shared",
            "match": "dependency",
            # Preserve the version ranges and alternatives published by the
            # exact, tested Waydroid Debian component. The runtime pool assets
            # are offline candidates; installed host packages may satisfy the
            # same Debian dependency expressions at compatible versions.
            "depends": dependencies,
            "candidateAssetIds": host_shared_asset_ids,
        })
    return assets, requirements


def main() -> None:
    catalog = json.loads(CATALOG_PATH.read_text())
    if catalog.get("architecture") != "arm64" or catalog.get("host", {}).get("distribution") != "ubuntu" or catalog.get("host", {}).get("version") != "24.04":
        raise ValueError("Only the verified Ubuntu 24.04 ARM64 runtime catalog is supported.")
    packages = catalog.get("packages")
    if not isinstance(packages, list):
        raise ValueError("Runtime catalog package list is missing.")
    assets, requirements = deployment_assets(packages)

    config = json.loads(CONFIG_PATH.read_text())
    operations = config.get("agentos", {}).get("deployment", {}).get("operations", [])
    matches = [
        (operation, index)
        for operation in operations
        if operation.get("id") == "android-runtime-setup"
        for index, action in enumerate(operation.get("actions", []))
        if action.get("type") in ("debian.packages.install-bundle", "debian.packages.ensure")
    ]
    if len(matches) != 1:
        raise ValueError("Expected exactly one Android runtime package action.")
    operation, action_index = matches[0]
    operation["version"] = 3
    operation["summary"] = "Prepare Binder and install the digest-pinned Waydroid runtime with compatible Ubuntu 24.04 host dependencies."
    action = {
        "type": "debian.packages.ensure",
        "schemaVersion": 1,
        "assets": assets,
        "requirements": requirements,
    }
    operation["actions"][action_index] = action

    normalized_actions = [
        {**item, **({"parameters": {}} if item.get("type") == "linux.module.load" and "parameters" not in item else {})}
        for item in operation["actions"]
    ]
    action_bytes = len(json.dumps(normalized_actions, ensure_ascii=False, separators=(",", ":")).encode())
    if action_bytes > MAX_ACTION_BYTES:
        raise ValueError(f"Deployment actions exceed AgentOS's {MAX_ACTION_BYTES}-byte review limit: {action_bytes}")

    temporary = CATALOG_PATH.with_suffix(".json.tmp")
    temporary.write_text(json.dumps(catalog, ensure_ascii=False, indent=2) + "\n")
    temporary.replace(CATALOG_PATH)
    temporary = CONFIG_PATH.with_suffix(".json.tmp")
    temporary.write_text(json.dumps(config, ensure_ascii=False, indent=2) + "\n")
    temporary.replace(CONFIG_PATH)
    exact_count = sum(item["package"] in APPLICATION_COMPONENTS for item in packages)
    print(f"RUNTIME_DEPLOYMENT_SYNCED {len(assets)} packages {action_bytes} bytes (applicationExact={exact_count}, hostShared={len(assets)-exact_count})")


if __name__ == "__main__":
    try:
        main()
    except (KeyError, OSError, TypeError, ValueError, json.JSONDecodeError) as error:
        print(f"Runtime deployment sync failed: {error}", file=sys.stderr)
        raise SystemExit(1)
