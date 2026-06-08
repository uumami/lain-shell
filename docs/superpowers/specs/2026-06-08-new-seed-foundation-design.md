# new-seed — Foundation (Re-Derivation)

> STATUS: **Foundation — agreed through dialogue, pending final user review.** This is Cycle 1
> (re-derive the foundation). It makes an explicit **keep / change / improve** call on every
> major decision inherited from the prior `lain-shell` design (now on the `main` branch). Each
> section's heading carries its decision state (AGREED / DECIDED / open items called out
> inline). The only deliberately open item is the product name (Section 8).
>
> Method: hold the *philosophy and success criteria* fixed; treat every *architecture and
> technology* decision as a proposal to confirm, change, or improve. Reuse existing building
> blocks where they are not the thing we differentiate on; build new the things that are the
> point (security and UX/UI).
>
> Scope: this document stops at *architecture*. Per-subsystem deep design and code are later
> cycles, ordered by risk (Section 10) — starting with a throwaway spike of the sealed cage.

---

## 0. What this document is

The prior design (`lain-shell`, on `main`) is a security-first "terminal operating platform"
for the agentic age: five "quanta" (Core, Navi, MOTOKO, MAGGI, The Wired), Rust, wgpu
renderer, Linux namespaces + seccomp isolation in four levels, tamper-evident audit log, a
config-mirror pattern so a compromised agent cannot weaken its own cage.

This document re-derives that foundation to make sure it satisfies the original criteria, and
improves it where it fell short. Per-quantum deep design and the first code slice are *later*
cycles. This document stops at architecture.

---

## 1. North star and testable success criteria

**North star (unchanged from the original):** the safest possible terminal for code and
agents, that is *also* genuinely pleasant — easy to use, fast, personalizable — and that works
perfectly with no agent at all. Quality over adoption.

The improvement over the original is turning vague goals into **testable acceptance criteria**:

| # | Criterion | Testable statement |
|---|---|---|
| SC-1 | Data confinement | A fully compromised coding tool running in a `sealed` workspace cannot exfiltrate workspace data off the machine. Verified by red-team: plant attacker code in a tool/dependency, confirm no bytes of marked data leave, and no covert channel of meaningful bandwidth exists. |
| SC-2 | Escape resistance | A compromised agent cannot escape its cage or reach the host filesystem outside its workspace, *even assuming a kernel 0-day* — because the `sealed` ceiling is a hardware-virtualization (microVM) boundary, not a shared-kernel one. |
| SC-3 | No self-weakening | A compromised agent cannot lower its own isolation, open a network hole, or alter the audit record. Verified by attempting each from inside the cage and confirming it fails. |
| SC-4 | Tamper-evident audit | Every security-relevant action is recorded in a human-readable, append-only, hash-chained log that detects any retroactive edit. |
| SC-5 | Stands alone | The terminal is fully usable with zero agents and zero network. Cold start to first prompt < 100 ms; render latency at parity with the fastest terminals (Ghostty class). |
| SC-6 | Lightweight | Host terminal process RSS stays small (target order: < 80 MB with GPU, < 15 MB headless). Cage weight is separate and does not count against this. |
| SC-7 | Safe path is the easy path | Spawning a sandboxed (`standard` or `sealed`) session is one command/keystroke; no config required for a strong default. Security is frictionless to invoke, or it gets bypassed. |
| SC-8 | Legible security state | At any moment the user can see, per pane, what cage it is in, what network it has, and what it is allowed to do. |
| SC-9 | Personalizable & durable | Behavior, visuals, and keybindings are configurable via human-readable, version-controlled files that survive for years. |
| SC-10 | Agents first-class, optional | Agents get typed sessions, permission scopes, lifecycle events, and cost tracking — but nothing core depends on them. |
| SC-11 | Installable anywhere | lain-shell installs and runs on any modern Linux, on the user's terms: a native package on the major families (`apt`/`dnf`/Arch/Alpine) **and** a single distro-agnostic installer for everything else. No distro hardcoded; distro/arch/libc detected, never assumed. Base install is small; the hardware-gated sealed microVM stack is fetched only when used. Tiers degrade honestly: a non-microVM fallback is never labeled `sealed`. Detail in `2026-06-08-distribution-design.md`. |

These criteria are the contract. Every later cycle is judged against them.

---

## 2. Threat model

**Primary adversary:** a fully compromised participant on the agent side — the coding agent
itself (rogue model output, prompt injection), or any tool, dependency, or MCP server it runs
(supply-chain compromise, e.g. the LiteLLM March 2026 attack). Assume hostile from second one.

**What it is trying to do:** (a) exfiltrate sensitive workspace data; (b) escape the cage and
reach the host / persist; (c) move laterally to other sessions or the user's secrets
(`.ssh`, `.aws`, `.kube`, `.gnupg`); (d) tamper with the record of what it did.

**Sophistication assumed:** up to and including a kernel 0-day (the "mythic" attacker). This is
the specific reason the ceiling must be a VM boundary, not a namespace boundary.

**Explicitly in scope:** untrusted agent code; untrusted tools/dependencies; untrusted model
output; covert exfiltration channels over allowed paths.

**Explicitly out of scope (for now, state it honestly):** a malicious host OS or compromised
kernel *before* the session starts; physical access and hardware side-channels beyond
reasonable mitigation; the human operator being the adversary (they own the machine).

---

## 3. Security architecture (DECIDED in dialogue — locked)

The user chooses the level of security; the default is strong; the human (not the agent)
controls it; restriction only ratchets up automatically.

**Two layers instead of one slider** (this is the core *improvement* over the original's
one-dimensional Level 0–3):

**Layer 1 — four orthogonal dials (the real flexibility):**
- **Mechanism** (cage strength): none -> namespaces+seccomp -> rootless container -> microVM.
- **Network**: sealed -> allowlist egress through an inspecting, default-deny proxy -> open.
- **Filesystem**: what is mounted, and ro vs rw (workspace rw; toolchains ro; secrets invisible).
- **Host tools**: which host capabilities (docker, nvidia-smi, kubectl) are proxied in, allowlisted.

These are independent, so combinations the old levels could not express are possible
("microVM with open network", "container with sealed network").

**Layer 2 — named presets (the simplicity); most users only ever pick a word:**
- **`open`** — host/namespaces, network open. Throwaway local work, GPU jobs. Honors "lightweight" (no microVM to run `ls`).
- **`standard`** (default) — rootless container, default-deny egress + allowlist, secrets hidden. Everyday coding.
- **`sealed`** (ceiling) — ephemeral microVM, no network, workspace-only filesystem. "HIPAA-grade without a BAA, survives a kernel 0-day." Auto-selected for workspaces flagged sensitive.
- **gVisor** is not a preset; it is an automatic substitution for `sealed`'s mechanism on hosts without KVM (gVisor needs no hardware virtualization, at the cost of syscall/file-I/O overhead that hurts compiles).

**Two invariants that keep "flexible" from meaning "insecure":**
1. **Highest-restriction-wins** across all sources (agent default, repo policy, directory policy, workspace flag) — keep the old `max()` resolution. Flexibility only ratchets up automatically.
2. **The agent can never lower its own cage.** The config the kernel/VMM actually enforces lives outside the agent's reach (config-mirror pattern, kept from the original); the agent may *propose* changes in repo files, but only an explicit human `sync` applies them, and the diff is shown.

Supporting pieces (kept from original, confirmed): tamper-evident append-only hash-chained
audit log with a single privileged writer; default-deny inspecting egress proxy; host-proxy
shims for allowlisted host tools.

**Why microVM ceiling (improve):** namespaces and containers share the host kernel, so a kernel
0-day = escape. Only a hardware-virtualization boundary survives that. For a coding workload,
Firecracker-class microVMs are *both* stronger and faster at compiles than gVisor (real guest
kernel via KVM, near-native syscalls), at the cost of ~125–250 ms cage startup and real
engineering (rootfs images, virtio, in-guest agent, networking). The original topped out at
"container + no network", below the bar its own threat model (LiteLLM) implied.

---

## 4. System decomposition (AGREED)

**The original:** five quanta as five separate processes (Core, Navi, MOTOKO, MAGGI, The Wired),
talking over an async trait layer that is in-process in dev and gRPC-over-Unix-socket in prod,
with every cross-boundary type forced to be `Serialize + Deserialize`.

**Critical read:** the *separation of responsibilities* is good and worth keeping. But making all
five separate processes draws the boundaries on *conceptual module lines*, not on *actual trust
boundaries*. Navi (multiplexer), Core, and The Wired (external API) all run trusted first-party
code. The only untrusted code is the agent and its tools — which live in the cage. So isolating
Core from Navi buys little security while paying full IPC + serialize-everything cost. The
original itself flagged this weight as a tension, and it works against the "lightweight" principle.

**Proposal: draw process boundaries on trust boundaries, keep the quanta as *modules*.**

- **Trusted host side = one process** ("the shell"): the terminal core (PTY, VTE, renderer,
  config, CLI), the multiplexer (sessions/tabs/panes), the external API surface
  (MCP/socket), and the agent *orchestrator*. Internally modular — the quanta survive as
  crates/modules with trait boundaries — so we *can* split any of them into a process later
  without rewrites. We just do not pay for it by default.
- **Untrusted side = cages**: each agent/tool session in its isolation cage (separate
  process / container / microVM by definition). This is the real, load-bearing process
  boundary.
- **Security guardian = a separate, more-privileged process** (the enforcement + audit core of
  MOTOKO): owns the tamper-evident audit log and the policy the kernel/VMM enforces, so that
  even a bug in the big host process cannot rewrite history or silently weaken enforcement. The
  deterministic enforcement (seccomp/namespace/VM config) is kernel-enforced regardless.

**Net:** instead of "always five processes", the default topology is **one trusted host process +
N isolated cages + one privileged guardian.** Fewer moving parts, lower latency and weight,
boundaries that match the threat model. The quanta names and responsibilities stay as the
internal architecture (and as identity).

- *Keep:* the five responsibilities; trait boundaries between them; the config-mirror; the
  two-tier event model (durable mandatory audit vs. lossy observation bus).
- *Change:* default process topology from five processes to trust-boundary-based (host /
  cage / guardian). Drop the mandatory "serialize every cross-boundary type" tax for
  in-process module calls; keep serialization only where a boundary is really crossed
  (host<->cage, host<->guardian).
- *Improve:* boundaries now defend against the actual adversary instead of partitioning
  trusted code.

### 4.1 Concrete decomposition (AGREED)

**Process map — three kinds of process:**

1. **Host process** — the `lain-shell` binary (codename **LAIN**). *Trusted.* Holds
   terminal-core, session-mux, the isolation manager, the boundary, and the optional operator.
   One process by default.
2. **Guardian process** — `guardian` (**MOTOKO**), *separate and more-privileged.* Sole writer
   of the audit log; holds policy enforcement and the tool-call broker. It is a separate
   process *specifically* so a bug in the large host process cannot rewrite history or quietly
   weaken enforcement.
3. **Cage processes** — **EVA** units, one per sandboxed session (container or microVM). Run the
   untrusted agent/tools plus a tiny in-cage bridge (`cage-agent`) that relays PTY and brokered
   tool calls.

**Crate map (Cargo workspace):**

| Crate | Codename | Role | Process |
|---|---|---|---|
| `lain-types` | — | IDs, errors, events, command model, **all trait definitions** (seam layer) | shared |
| `terminal-core` | BEBOP | PTY, VTE, renderer, config, CLI | host |
| `session-mux` | NAVI | sessions, tabs, panes, layout, attach/detach | host |
| `isolation` | GEOFRONT | cage lifecycle, dials/presets resolution | host |
| `boundary` | THE WIRED | inbound control API, outbound egress proxy, host-tool shims | host |
| `operator` | MAGI | optional operator agent | host |
| `guardian` | MOTOKO | audit log (single writer), policy enforcement, tool-call broker | **its own binary** |
| `cage-agent` | — | in-cage bridge: PTY relay + brokered tool calls | **runs inside EVA** |
| `lain-shell` | LAIN | composes the trusted host modules; entry point | host binary |

`lain-types` depends on nothing; every other crate depends on it; no quantum module depends on
another quantum module (their contracts live in `lain-types`). The host binary is the only place
that wires implementations together.

**The seam rule (the key improvement over the original):** a type must be
`Serialize + Deserialize` **only if it crosses a real process boundary** — i.e. host<->guardian
or host<->cage. In-process module calls (core<->mux<->isolation<->boundary<->operator) use owned
types but pay no serialization tax. The original forced *every* cross-boundary type to
serialize because every quantum was a process; here we pay that cost only where a boundary is
actually crossed. Trait seams stay clean so any host module can be promoted to its own process
later without an API rewrite.

**Two enforcement decisions (AGREED):**

1. **Egress enforcement is structural, at the cage boundary.** Default-deny is enforced at the
   cage's network namespace / VM — unbypassable by the agent, kernel/VM-level. THE WIRED's
   inspecting proxy is simply the *only allowed route* out, so even a buggy host process cannot
   grant a cage network it should not have; the cage has no other path. The proxy itself may run
   in-host initially (the *enforcement* is structural regardless) and can be hardened into its
   own process later.
2. **Cage tool-calls are decided and audited by the guardian, host is a dumb relay.** A cage's
   brokered actions are policy-checked and audited by **MOTOKO**, never decided inside the
   large trusted host process. The host only relays. The security decision and the audit record
   live in the privileged guardian.

---

## 5. Technology and reuse (AGREED — mostly keep)

- **Language: Rust — keep.** Right call for a systems-level, memory-safe, performance-critical
  security tool. Single ecosystem. `thiserror` + a serializable error type at boundaries; no
  `anyhow` in libraries.
- **Reuse (do not rebuild — not differentiators):** `portable-pty` (PTY), `alacritty_terminal`
  or `vte` (VTE parsing / cell grid / scrollback), `wgpu` + `glyphon`/`cosmic-text`
  (GPU text rendering), `softbuffer` (CPU fallback), `tokio` (async).
- **Reuse for the security ceiling:** Firecracker / `rust-vmm` crates for microVMs; `nix` /
  `libseccomp` for namespaces + seccomp; rootless Podman or direct OCI for containers;
  `aho-corasick` for fast PTY pattern scanning.
- **Build new (these are the point):** the isolation manager (the dials + presets engine), the
  inspecting egress proxy, the privileged guardian + audit log, the agent orchestration and its
  permission/audit broker, the personalization/config system, and the UI/overlay layer that
  makes security legible.
- **Decision to confirm later:** GPUI vs. raw wgpu. Original chose raw wgpu for pixel ownership;
  likely keep, but flag as a drill-down item since rendering is the riskiest/longest piece.

---

## 6. Agent model (AGREED reframe)

**Original:** MAGGI is a single, optional, pluggable-backend "operator agent" for the platform;
separately, external coding agents (Claude Code, etc.) run in cages.

**Reframe (improve emphasis to match the actual use case):** the platform's primary job is to
**host and govern agents** — run them sandboxed, mediate every tool/network/filesystem action
through a permission + audit broker, and make their behavior legible and reversible. A built-in
operator agent is an *optional convenience layered on top*, not the headline.

- **Keep:** agents first-class but optional (SC-10); pluggable model backend (local Ollama,
  remote API, external agent-as-engine, or absent); per-session conversation contexts; cost tracking.
- **Change:** lead with "host + govern untrusted agents," demote "we ship an operator agent"
  to secondary. The agent is untrusted by default; its capabilities are exactly what the cage +
  broker grant, nothing more.
- **Tie to security:** the agent model *is* a security boundary. Tool calls from an agent are
  brokered, permission-checked, and audited — the same machinery as SC-1..SC-4.

---

## 7. UX/UI stance (AGREED — the original barely specified this)

The original left first-launch UX and the visual layer as open questions. You named UX/UI as a
first-class goal, so it gets real principles now (detailed visual design is its own later cycle):

- **Zero-config excellence (SC-5):** great untouched, fast first run, strong defaults.
- **The safe path is the easy path (SC-7):** spawning a `standard`/`sealed` session is one
  keystroke/command. If security is tedious to invoke, people route around it; frictionless
  security is a security feature.
- **Legible security state (SC-8):** every pane shows, at a glance, its cage preset, its
  network posture, and what it is allowed to do. Security is visible, not hidden.
- **Approachable security events:** when the guardian flags something, it is explained in plain
  language with a clear next action — not a cryptic alert.
- **Extreme but human-readable customization (SC-9):** themes, fonts, layout, transparency,
  backgrounds, keybinding sets (native / tmux-like / Zellij-like), all via version-controlled
  TOML. The platform bends to the user.
- **Open item for the UX cycle:** concrete mockups for the pane status indicator, the
  session-spawn flow, and the security-event surface.

---

## 8. Identity and naming (DECIDED — scheme; product name still open)

Keep the aesthetic and "feeling" (NERV / Serial Experiments Lain / Ghost in the Shell /
Evangelion). **Every subsystem carries two names:**

- **Engineering name** — descriptive, lowercase, used in crate names, the CLI, config keys,
  log lines, and error messages. A newcomer reads it and knows what the thing does. This is
  the *load-bearing* name in code.
- **Codename** — the "feeling" name, drawn from the source universe. Used in docs, UI flavor,
  status surfaces, and internal culture. Never the sole identifier in code; always paired with
  or subordinate to the engineering name.

This separation is a deliberate **improvement** over the original, which conflated the two
(the crate literally *was* `lain-motoko`) — cute, but opaque to anyone new, and it locked the
codename into the public API.

| Subsystem (what it does) | Engineering name | Codename | Source / rationale |
|---|---|---|---|
| Platform / product | `lain-shell` *(name TBD)* | **LAIN** | SEL — pervasive presence; "present day, present time" |
| Terminal core: PTY, VTE, renderer, config, CLI | `terminal-core` (`core`) | **BEBOP** | Cowboy Bebop — the ship's body/engine; the core that makes the terminal real, that NAVI and the rest are built around. (The whole product/host is **LAIN**, not BEBOP.) |
| Multiplexer + user-facing surface: sessions/tabs/panes/layout | `session-mux` (`mux`) | **NAVI** | SEL — Lain's personal machine; the interface you touch |
| Isolation manager: cage lifecycle, the dials/presets engine | `isolation` | **GEOFRONT** | Eva — the sealed buried sphere that contains NERV HQ and the cages |
| A single sandbox/cage | `cage` | **EVA** | Eva — units kept in cages inside the Geofront, contained the instant they go berserk; cages number as EVA-01, EVA-02... |
| Security guardian: enforcement policy + audit log + tool-call broker | `guardian` | **MOTOKO** | GITS — the security operative |
| Network boundary: inbound control API + outbound egress proxy + host-tool shims | `boundary` (`net`) | **THE WIRED** | SEL — everything crossing the network/external edge goes through here |
| Operator agent (optional) | `operator` | **MAGI** | Eva — the deliberating three-way council |
| Builder identity / ethos | — | **NERV** | "Built by NERV" — the project's voice, not a subsystem |

**Naming conventions:**
- Crates use the engineering name, optionally with a project prefix once the product name is
  settled (e.g. `<product>-guardian`, not `<product>-motoko`).
- The CLI, config keys, and logs use engineering names (`isolation`, `guardian`, `boundary`).
- Codenames appear in docs headings, UI labels, and the audit/status surfaces as flavor
  (e.g. a pane badge reading "EVA-01 / sealed").
- Codename universe is intentionally mixed (SEL + GITS + Eva + Cowboy Bebop) but consistent in
  tone; new subsystems should be named from the same well.

**Still open:** the product name. `lain-shell` carries the feeling and the "ghost in the
*shell*" pun, but it is a codename, not an engineering name. Decide before the first code cycle
whether the product keeps `lain-shell` or takes an engineering-forward name with **LAIN** as
its codename.

---

## 9. Keep / change / improve summary

| Decision | Call | Note |
|---|---|---|
| Philosophy & success criteria | **Keep** (made testable) | Section 1 |
| Threat model incl. kernel-0-day adversary | **Keep + sharpen** | Section 2 |
| Isolation as configurable levels | **Keep** | user chooses, secure default |
| One-dimensional Level 0–3 | **Change -> 4 orthogonal dials + presets** | Section 3 |
| Isolation ceiling | **Improve -> microVM** (was container+no-net) | Section 3 |
| Default-deny inspecting egress | **Improve -> structural, not a side effect** | Section 3 |
| Config-mirror (agent can't self-weaken) | **Keep** | Section 3 |
| Tamper-evident audit log | **Keep** | Section 3 |
| Five quanta as responsibilities | **Keep** | Section 4 |
| Five quanta as five processes | **Change -> trust-boundary topology** | Section 4 |
| Serialize every cross-boundary type | **Change -> only across real boundaries** | Section 4 |
| Rust, reuse PTY/VTE/render libs | **Keep** | Section 5 |
| microVM / seccomp / proxy stack | **Build new (the differentiators)** | Section 5 |
| MAGGI operator agent as headline | **Change -> host+govern agents is headline** | Section 6 |
| Agents first-class but optional | **Keep** | Section 6 |
| UX/UI | **New / improve -> first-class principles** | Section 7 |
| Identity / naming | **Improve -> engineering name + codename per subsystem** | Section 8; product name TBD |
| Distribution / installability | **New -> SC-11 + capability degradation** | `2026-06-08-distribution-design.md`; KVM is the real "any Linux" gate |

---

## 10. Path forward (AGREED — risk-first, not document-first)

The original failed by being *documented* but not *validated* in its riskiest corners
(render parity, the MOTOKO tiers stayed sketches). We do not repeat that. We do **not** write
eight subsystem specs up front. We order work by **risk**, and we validate the central bet with
real code before designing the rest.

**Immediate next cycle — spike the sealed cage (throwaway code).** The load-bearing bet of the
whole project is the `sealed` preset actually being safe *and* pleasant. Validate, with a thin
spike meant to be discarded:
- A Firecracker-class microVM cage (**EVA**) that boots fast enough that "one-keystroke sealed
  session" is honest (target: low hundreds of ms).
- A real coding workload inside it running near-native (compile/test a sample repo).
- Default-deny egress enforced structurally at the cage boundary, *proven* unbypassable from
  inside (attempt exfiltration, confirm it fails).
- A single brokered tool-call round-trip through the guardian (**MOTOKO**) with measured
  latency.

Exit criteria: numbers for boot time, in-cage workload overhead, and broker round-trip latency,
plus a confirmed-blocked exfiltration attempt. If it holds, the central promise is real. If it
breaks, we learned it in week one with ~300 lines, not month four with eight specs.

**After the spike — design+build the rest just-in-time, in risk order.** Each becomes its own
spec -> plan -> build cycle, informed by what the spike taught us. Rough risk order (revisit
after the spike):
1. Isolation manager (**GEOFRONT**) — dials/presets engine, resolution, microVM path, config-mirror sync.
2. Guardian + audit (**MOTOKO**) — privileged process, hash-chained log, mandatory vs observable, the broker.
3. Egress + host-tool proxy (**THE WIRED**) — default-deny, allowlist, inspection, shims.
4. Terminal core (**BEBOP**) — PTY/VTE/render reuse, block model, render-parity risk.
5. Multiplexer + UX (**NAVI**) — first-launch, one-keystroke spawn, pane security indicator, event surface, mockups.
6. Operator agent (**MAGI**) — pluggable backend, optional.
7. Product name + repo/crate scaffolding — settle the name; lay down the workspace.

**Done already (this document):** system decomposition / trust-boundary topology / crate map
(Section 4 and 4.1).
