# AndroidTone integration architecture

AndroidTone 0.1.9 is a desktop-user Native process App with a package-owned Host Provider. Its service runs outside the Docker App sandbox and owns the Android runtime payload, Android images, APKs and application lifecycle. AgentOS owns kernel facilities, system authorization, trusted desktop-session lookup, Provider registration, private Wayland allocation and Flutter presentation.

The ordinary Provider keeps `no-new-privs` and an empty capability set. It does not call `sudo` or `pkexec`. AndroidTone 0.1.9 registers the generic system-deployment methods and declares one digest-pinned offline package operation. AgentOS verifies the installed app identity, exact `.deb` hashes, versions and architecture, then asks the administrator to authorize the fixed host Binder module, Binderfs and package actions. No Binder device is assigned to an App container. APK/runtime business logic remains in AndroidTone; AgentOS provides kernel, authorization, provider and window primitives.

The package declares `native.desktop-session` and calls `org.agentos.desktop-session/session.get`. It accepts a session only when AgentOS reports it available and the returned UID/GID, runtime directory and D-Bus socket match the running identity. It does not read SSH identity, infer `/run/user/<uid>/bus`, or depend on `AGENTOS_SESSION_CONTEXT`.

For an Android display, AndroidTone asks `org.agentos.external-wayland` to create a private session. AgentOS passes that host client its allocated `XDG_RUNTIME_DIR` and `WAYLAND_DISPLAY`; the client verifies the private socket and uses the validated D-Bus address returned by `session.get`. Existing desktop compositors and Wayland sessions are not adopted.

Kernel configuration, matching module installation, module loading and Binderfs mounts remain separate observations. Package installation and Waydroid image initialization are also reported separately from Waydroid service and display-session state. Starting a Waydroid process does not prove window mapping, Flutter presentation, input, scaling, multiple windows or actual GPU presentation. If the host Waydroid command or system container service is stopped, AndroidTone reports the missing generic platform lifecycle contract instead of elevating its Provider.

See the [shared integration review](../../docs/tone-platform-integration-review.md), [automatic setup status](automatic-setup.md), and [offline payload](offline-runtime-zh_CN.md).
