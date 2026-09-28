# Resource example

This example demonstrates bundle resources in the Vitul desktop runtime. It
adds `src-tauri/assets/index.js` as a resource (configured under
`tauri.conf.json > bundle > resources`) and executes it using Node.js, locating
the file through the compatible `tauri::App` path API.

## Running the example

- Build the Vitul-compatible desktop runtime from this repository:
  Linux / Mac:

```
# choose to install node cli (1)
bash .scripts/setup.sh
```

Windows:

```
./.scripts/setup.ps1
```

- Install dependencies (Run inside of this folder `examples/resources/`)

```bash
$ pnpm i
```

- Run the app in development mode (Run inside of this folder `examples/resources/`)

```bash
$ pnpm tauri dev
```

- Build an run the release app (Run inside of this folder `examples/resources/`)

```bash
$ pnpm tauri build
$ ./src-tauri/target/release/app
```
