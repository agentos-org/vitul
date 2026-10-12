# AndroidTone integration architecture

AndroidTone is a containerized process App with a package-owned Host Provider. It owns the Android runtime payload, Android images, APKs and application lifecycle. AgentOS owns kernel facilities, system authorization, trusted desktop-session lookup, Provider registration, private Wayland allocation and Flutter presentation.

The ordinary Provider keeps `no-new-privs` and an empty capability set. It does not call `sudo` or `pkexec`. AgentOS now provides a versioned generic system-deployment broker for declared, fixed host actions. AndroidTone does not request privileged deployment yet: the current action set cannot create and allocate Binderfs devices or initialize and manage the required LXC container. Until that generic resource contract exists, setup reports a platform dependency and leaves the host unchanged.

The package declares `native.desktop-session` and calls `org.agentos.desktop-session/session.get`. It accepts a session only when AgentOS reports it available and the returned UID/GID, runtime directory and D-Bus socket match the running identity. It does not read SSH identity, infer `/run/user/<uid>/bus`, or depend on `AGENTOS_SESSION_CONTEXT`.

For an Android display, AndroidTone asks `org.agentos.external-wayland` to create a private session. AgentOS passes that host client its allocated `XDG_RUNTIME_DIR` and `WAYLAND_DISPLAY`; the client verifies the private socket and uses the validated D-Bus address returned by `session.get`. Existing desktop compositors and Wayland sessions are not adopted.

Kernel configuration, matching module installation, module loading, Binderfs mounts and device allocation remain separate observations. Process startup does not prove window mapping, Flutter presentation, input, scaling, multiple windows or actual GPU presentation.

See the [shared integration review](../../docs/tone-platform-integration-review.md), [automatic setup status](automatic-setup.md), and [offline payload](offline-runtime-zh_CN.md).
