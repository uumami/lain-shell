# MOTOKO — Philosophy

> *The security and observation layer. Quiet, cheap, correct.*

---

## Purpose

MOTOKO watches actions, enforces boundaries, records events, and escalates only when necessary. Named after Major Motoko Kusanagi from Ghost in the Shell — the one who sees everything, questions everything, and operates at the boundary between machine, identity, control, and trust.

MOTOKO is not a feature for marketing. It is a real enforcement layer with structural teeth.

---

## Principles

### Security is structural, not promised

If something is dangerous, it must be prevented by architecture — kernel restrictions, process isolation, mount namespaces, network boundaries. Not by policy files that agents can read and work around. Not by promises. By the kernel.

The LiteLLM supply chain attack (March 2026) is the reference point. A trusted dependency was compromised. The payload harvested credentials and moved laterally. Severity 9.4/10. Real structural enforcement would have caught the credential file access attempt before any data left the system. That is MOTOKO's standard.

### Three tiers, escalating cost

MOTOKO operates in three tiers of escalating sophistication and cost:

**Tier 1 — Deterministic enforcement.** Always running. Zero tokens. Near-zero latency. seccomp-BPF, network policies, mount restrictions, compiled pattern matching. Handles the vast majority of security decisions without any model. Cannot be disabled without explicit, logged opt-out.

**Tier 2 — Statistical and heuristic analysis.** Zero tokens. Periodic or triggered. Behavioral baseline comparison, entropy analysis, network frequency anomalies, resource consumption patterns. No model invocation.

**Tier 3 — Reasoning analysis.** Tokens. Triggered only. Invoked only when genuinely needed: ambiguous Tier 2 CRITICALs, end-of-session postmortem, explicit user request, scheduled review.

### The postmortem pattern

The key insight for token economy: **analyze in the quiet moments, not during the work.**

During active sessions: Tier 1 and 2 only. Near-zero tokens. Session uninterrupted. Events recorded.

After sessions (or during idle compute): Tier 3 reviews flagged events. Brief: what happened, likely explanations, recommended rule adjustments. If nothing was flagged, the report is empty.

Work is not interrupted. Security is not theater.

### Quiet, not dramatic

Tier 1 blocks happen silently. The syscall fails. The packet drops. The agent adapts or stops. No popup, no warning — unless the user checks the status indicator.

Tier 2 anomalies are non-blocking indicators. Session continues. Anomaly queued for postmortem.

Tier 2 CRITICALs pause — not kill — the session. The user sees what triggered it and chooses.

### MOTOKO cannot die silently

**Principle.** If MOTOKO dies unexpectedly, all active agent sessions are paused or terminated. The system enters an explicit failure state. Silent loss of the watching layer is not acceptable.

### Rules are Turing-incomplete

**Principle.** MOTOKO's rules use pattern matching and conditions only. No general computation. A reasoning agent cannot exploit a rule engine that cannot reason back.

Rules are:
- Encrypted at rest, decrypted into memory only at runtime
- Human-readable in plaintext form
- Signed so tampering is detectable

> **Idea:** Community rule contributions: anonymized submission optional, off by default.

---

## What MOTOKO Owns

- Security profile generation per isolation level (ADR-009)
- seccomp-BPF profile management and application
- Network policy enforcement
- Mount namespace / filesystem restriction
- Host proxy allowlist enforcement and audit
- PTY output pattern matching (compiled, Aho-Corasick)
- Behavioral baseline tracking
- Output entropy analysis
- Event logging and the tamper-evident audit trail
- Tier escalation logic
- Postmortem generation
- Security event correlation across sessions

---

## What MOTOKO Does Not Own

- Explaining events to humans (MAGGI interprets MOTOKO events)
- Permission manifest authoring (MAGGI helps users write these)
- Agent lifecycle (Navi)
- Model invocation for anything other than Tier 3 analysis

---

## Open Questions

1. **Behavioral baseline establishment.** How is the baseline built for a new agent? First session? First week? Per-agent type? Per-project?
2. **Tier 3 model selection.** Smallest model that reliably distinguishes genuine exfiltration from false positive. Empirical.
3. **Tier 3 isolation.** The reasoner runs in its own pod — no network, no tools, no memory across sessions. How is the summarized event window prepared? What information is included vs. excluded?
4. **Rule update mechanism.** How are MOTOKO rules updated? Signed packages? Community contributions?
5. **macOS parity.** macOS lacks seccomp-BPF. `sandbox-exec` is deprecated. What is the Tier 1 story on macOS?
6. **eBPF as advanced opt-in.** Full eBPF observability introduces attack surface (elevated privileges, verifier CVEs). Worth it for advanced users? Where is the line?

---

## Ideas

1. **MOTOKO learning.** Anonymized, opt-in sharing of security patterns. Privacy-preserving aggregate learning.
2. **Cross-session correlation.** Detect patterns that span multiple agent sessions — e.g., one agent probing what another is doing.
3. **Visual event timeline.** A timeline overlay showing MOTOKO events during a session, reviewable after the fact.

---

## Why Not Full eBPF by Default

Full eBPF observability introduces real attack surface — elevated privileges, real CVEs in the verifier, complexity that can be exploited.

The safer base:
- seccomp-BPF for blocking (immutable, inherited, kernel-enforced)
- auditd on Linux / Endpoint Security Framework on macOS for observation
- Podman network policies and mount namespaces for isolation

Full eBPF is an opt-in advanced feature. Not the default.
