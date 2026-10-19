# Wine 9 cryptbase compatibility

WeChat 4.1.15.53 x64's Crashpad handler loads `cryptbase.dll` before resolving
`advapi32!SystemFunction036` for random initialization. Ubuntu's Wine 9 runtime
has the advapi32 implementation but no cryptbase module. A failed load triggers
an intentional breakpoint at `crashpad_handler.exe+0x118d10`, leaving WeChat
waiting for its handler.

This small x64 PE DLL forwards `SystemFunction036` to Wine's advapi32 implementation.
It does not implement or replace random number generation. The launcher copies it
into system32 only when no cryptbase DLL or symlink is present, including in
previously initialized prefixes. It applies only to the FEX x64 Wine 9 path.
Do not carry this forwarder into newer Wine versions that forward advapi32's
SystemFunction036 back to cryptbase; use their built-in cryptbase instead.

The Dockerfile builds it from source using MinGW. On 2026-10-16, adding the
forwarder to the existing WeChat prefix on an ARM64 AgentOS test device removed
the Crashpad breakpoint; the Wayland session reported a mapped window, and the
user confirmed the login screen. Login, messaging, audio, and GPU operation were
not tested. The test device has the prefix repair; a new runtime package still
needs to be built for distribution.
