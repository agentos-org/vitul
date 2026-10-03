# AgentOS URL app

Use a URL app when the website is already hosted over HTTPS. Vitul packages the URL and app metadata; it does not download the site or bundle an application backend.

From the directory for the new app, create a starter `Vitulfile`:

```sh
mkdir my-url-app
cd my-url-app
vitul agentos init --type remote-web --url https://example.com
```

Review the generated app name and identifier, then validate and build the package:

```sh
vitul agentos validate
vitul build --target agentos
```

The generated profile has this shape:

```yaml
schemaVersion: 1
target: agentos/arm64

app:
  type: remote-web
  id: com.example.website
  name: Example Website
  version: 1.0.0

web:
  url: https://example.com

agentos:
  titleBar: system
```

The declared target only validates the selected Vitul build target. URL packages are
architecture independent and produce an `agentos-any.agt` package. Remote pages
do not receive AgentOS native capabilities, process storage, or OpenTool
bindings. Login, media playback, navigation, and embedding behavior depend on
the website's own policies.

See the [Vitulfile guide](../../crates/vitul-cli/src/vitulfile/README.md) for the full configuration reference.
