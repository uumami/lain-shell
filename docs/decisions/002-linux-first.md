# ADR-002: Linux First, Portable Architecture

## Status

Accepted

## Date

2026-04-03

## Context

lain-shell's security model depends heavily on OS primitives: seccomp-BPF, mount namespaces, network namespaces, rootless Podman, auditd. These are Linux-specific. macOS has rough equivalents (`sandbox-exec`, Endpoint Security Framework) but they differ significantly in capability and API.

Building for both platforms simultaneously would double the implementation surface for every security-critical feature while both are still being designed.

## Decision

Build for Linux first. Structure the code so that all platform-specific functionality is behind traits/interfaces, making the macOS port a matter of implementing the same interfaces with different OS primitives — not restructuring the architecture.

### What "Linux first" means

- Security enforcement uses Linux primitives (seccomp-BPF, namespaces, auditd)
- Container isolation uses rootless Podman
- Rendering targets Linux display servers (Wayland primary, X11 fallback)
- All tests run on Linux
- CI/CD targets Linux

### What "portable architecture" means

- Platform-specific code lives behind defined trait boundaries
- No Linux-specific assumptions leak into cross-cutting logic
- The trait boundaries are designed with macOS equivalents in mind
- WSL2 should work with minimal additional effort (it is Linux)
- macOS port is a matter of implementing existing interfaces, not redesigning

### macOS considerations held for later

- `sandbox-exec` is deprecated by Apple (still functional, but no guarantee of longevity)
- Endpoint Security Framework requires entitlements and has a different observation model than auditd
- Rootless Podman on macOS runs a Linux VM, which changes the isolation model
- Metal rendering vs Vulkan/OpenGL differs from the Linux GPU path
- AppKit/Cocoa window management vs X11/Wayland

## Consequences

### Enables

- Focused implementation without platform fragmentation
- Clean trait boundaries emerge from designing with one platform deeply
- Linux is the natural environment for the security model (kernel-level enforcement)
- WSL2 support comes nearly free

### Costs

- macOS users cannot use lain-shell initially
- macOS-specific design decisions are deferred (and may surface constraints later)

### Risks

- Trait boundaries designed only from the Linux perspective may not cleanly accommodate macOS primitives
- Mitigation: review trait designs against macOS equivalents periodically, even without implementing them

## Alternatives Considered

### Build both simultaneously

Rejected. Doubles the implementation surface. Both platform implementations would be half-finished instead of one being complete and well-tested.

### macOS first

Rejected. macOS has weaker kernel-level enforcement primitives. The security model is more naturally expressed on Linux. Starting with the stronger platform means the architecture can be designed for the full security story and then gracefully degraded for macOS.
