# Default app icon

`vitul-logo.png` is the full-bleed square master. Keep the background opaque and
extend it to every edge, without an outer frame, transparent padding, or rounded
corners. Keep the logo glyph inside the canvas so platform masks do not clip it.

After editing the master, build the CLI and synchronize the project template:

```sh
cargo build -p vitul-cli --bin vitul
bash .scripts/update-default-icons.sh
```

The script copies the master to the new project's root as `app-icon.png` and
regenerates every default PNG, ICO, and ICNS asset from the same source. It only
updates the preset template, leaving custom icons in existing apps alone.
