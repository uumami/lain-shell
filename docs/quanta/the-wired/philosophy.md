# THE WIRED — Philosophy

> *The connectivity layer. How lain-shell faces outward.*

---

## Purpose

THE WIRED is the set of bridges and interfaces through which external agents, automation, remote clients, and integrations connect to lain-shell. Named after the network in Serial Experiments Lain — the space where everything connects.

Every surface is a translation layer over the same `lain` CLI internals. One implementation, many clients. Every command through THE WIRED has structured output.

---

## Principles

### Machine-readable by design

Agents never need to scrape terminal text. Every operation is structured. `lain schema` dumps the full JSON Schema of every config file. `lain capabilities` tells any connecting agent exactly what the current session permits. `lain doctor` diagnoses issues in plain language.

### Multiple protocols, one implementation

THE WIRED exposes lain-shell through:
- **MCP server** — for agents that speak MCP natively (Claude Code, OpenCode, others)
- **gRPC bridge** — for programmatic integrations, CI/CD, companion apps
- **Unix socket** — for local scripts, plugins, and tooling

All three translate to the same internal command model. Adding a new protocol surface does not require reimplementing functionality.

### Network-addressable from day one

**Principle.** THE WIRED is designed to work over a network, not just locally. Even if the mobile companion app ships much later, the architecture must not make it painful to add.

> **Idea:** The mobile use case is approving a destructive agent action from a phone while away from the desk. Auth is token-based. Session buffer serialization is efficient.

### Permission-aware

Every connection through THE WIRED carries a permission context. An external agent connecting via MCP gets the permissions defined in its manifest — no more, no less. THE WIRED enforces this, MOTOKO audits it.

---

## What THE WIRED Owns

- MCP server implementation
- gRPC service definitions and server
- Unix socket listener
- Protocol translation to internal command model
- Connection authentication and authorization
- Structured output formatting
- Schema publication (`lain schema`)
- Capability advertisement (`lain capabilities`)

---

## What THE WIRED Does Not Own

- The commands themselves (Core's CLI)
- Permission definitions (config model in Core)
- Permission enforcement at the OS level (MOTOKO)
- Agent intelligence (MAGGI)

---

## Open Questions

1. **Auth model.** Token-based. But how are tokens issued, revoked, and scoped? Short-lived per-session? Long-lived per-agent?
2. **gRPC service design.** What services and RPCs does the gRPC bridge expose?
3. **MCP capabilities.** What MCP tools does lain-shell expose? How do they map to `lain` CLI commands?
4. **Remote session buffer.** For mobile/remote access: how is the terminal buffer serialized and transmitted efficiently?
5. **Discovery.** How does an external agent find a running lain-shell instance? mDNS? Well-known socket path?
