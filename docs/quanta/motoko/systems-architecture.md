# MOTOKO — Systems Architecture

> *Components, tiers, event flow, and enforcement boundaries.*

---

## Topology

Single MOTOKO process with internal per-session isolation. See ADR-008.

```
MOTOKO (single process)
│
├── Per-session scanners (panic-isolated)
│   ├── Session "work": Aho-Corasick scanner + behavioral baseline
│   ├── Session "ops": Aho-Corasick scanner + behavioral baseline
│   └── Session "background": Aho-Corasick scanner + behavioral baseline
│
├── Cross-session correlator
│   (detects patterns spanning multiple agents/sessions)
│
├── Policy compiler
│   (reads .lain/policies.toml, compiles once, shared)
│
├── Audit log writer
│   (single, ordered, hash-chained)
│
└── Tier escalation engine
    (Tier 1 events → Tier 2 analysis → Tier 3 trigger)
```

---

## Tier Architecture

```
┌─────────────────────────────────────────────────────┐
│  Tier 1 — Deterministic (always on, zero tokens)    │
│                                                     │
│  ┌─────────────────────────────────────────┐        │
│  │ KERNEL-ENFORCED (survives MOTOKO crash): │       │
│  │ • seccomp-BPF (applied at pod spawn)    │       │
│  │ • mount namespaces                       │       │
│  │ • network policy (iptables/nft)          │       │
│  └─────────────────────────────────────────┘        │
│                                                     │
│  ┌─────────────────────────────────────────┐        │
│  │ MOTOKO-PROCESS (dies with process):      │       │
│  │ • Aho-Corasick PTY output scanning      │       │
│  │ • Pattern matching on events             │       │
│  └──────────────────┬──────────────────────┘        │
│                     │ events                         │
├─────────────────────▼───────────────────────────────┤
│  Tier 2 — Statistical (zero tokens, periodic)       │
│                                                     │
│  • Behavioral baseline comparison                   │
│  • Output entropy analysis                          │
│  • Network frequency anomalies                      │
│  • Resource consumption patterns                    │
│  • Cost tracking vs thresholds                      │
│                                                     │
│  Verdicts:                                          │
│  ├── NORMAL: session continues                      │
│  ├── ANOMALY: logged, non-blocking indicator        │
│  └── CRITICAL: session paused, user chooses         │
│                     │                                │
├─────────────────────▼───────────────────────────────┤
│  Tier 3 — Reasoning (tokens, triggered only)        │
│                                                     │
│  Invoked when:                                      │
│  • Ambiguous Tier 2 CRITICAL                        │
│  • End-of-session postmortem (async)                │
│  • User request: `lain postmortem`                  │
│  • Scheduled review                                 │
│                                                     │
│  Runs in: isolated pod, no network, no tools,       │
│  no memory across invocations                       │
│  Input: summarized event window                     │
│  Output: structured JSON verdict                    │
└─────────────────────────────────────────────────────┘
```

---

## Input Channels

### Dedicated PTY channel (from Core)

Structurally isolated Unix socket. Carries raw PTY output bytes from all agent panes. MOTOKO demuxes by pane ID and runs Aho-Corasick scanning per session.

This channel is:
- Not routable through the event bus
- Not observable by other quanta
- Not affected by bus congestion
- Mandatory (cannot be disabled by any profile)

### Host proxy audit stream (from Core)

Every host proxy operation is reported to MOTOKO:
- Which agent requested the command
- What command was proxied (docker compose up, nvidia-smi, etc.)
- Path translations applied
- Exit code and timing
- For `docker compose`: compose file contents are statically analyzed before execution

### Event bus subscription

MOTOKO subscribes to:
- Session lifecycle events (from Navi)
- Agent lifecycle events (from Navi)
- Command block events (from Core)
- Config change events (from Core)
- Isolation lifecycle events (from Core)
- Host proxy events (from Core)

These are profile-filtered but mandatory events bypass the filter.

### Audit log — single writer

MOTOKO is the **sole writer** to the audit log (append-only JSONL with hash chain). Other quanta emit mandatory events via the `AuditSink` trait (defined in `lain-types`), which routes to MOTOKO's writer. MOTOKO serializes all writes, computes hash chain links, and fsyncs. No other process appends to the audit file directly.

In dev mode, `AuditSink` is an in-process call (serialized via mpsc channel to MOTOKO's writer task). In production, it's an RPC to MOTOKO's process over Unix socket. Either way, MOTOKO owns the file handle and the chain head.

If MOTOKO is unreachable, `AuditSink::emit()` returns an error and the calling quantum's mutating operation fails (fail-closed). This is by design — no agent mutation can occur without audit.

### Policy files

MOTOKO reads `policies.toml` and `permissions.toml` from the **safe mirror** (`~/.local/state/lain-shell/workspaces/<project-hash>/`), NOT from the repo's `.lain/` directory. See ADR-010. It compiles rules at startup and recompiles when config change events arrive (triggered by `lain config sync`).

MOTOKO **never writes** policy files. Separation of powers (see `systems-architecture.md`).

---

## Interface Exposed to Other Quanta

```rust
#[async_trait]
trait MotokoApi: Send + Sync {
    // Queried by Core's Isolation Manager when creating agent isolation
    async fn get_seccomp_profile(&self, agent_type: AgentType, workspace: WorkspacePath) -> Result<SeccompProfile>;
    async fn get_network_policy(&self, agent_type: AgentType, workspace: WorkspacePath) -> Result<NetworkPolicy>;

    // Queried by users/MAGGI (read-only)
    async fn get_status(&self) -> Result<MotokoStatus>;
    async fn get_session_status(&self, session: SessionId) -> Result<SessionSecurityStatus>;
    async fn verify_audit_chain(&self) -> Result<AuditVerification>;
    async fn get_recent_events(&self, filter: EventFilter) -> Result<Vec<SecurityEvent>>;

    // Trigger Tier 3 analysis
    async fn run_postmortem(&self, session: SessionId, window: TimeWindow) -> Result<PostmortemReport>;
}
```

### MOTOKO adapts to isolation levels

MOTOKO adjusts its monitoring based on the agent's isolation level (ADR-009):

| Level | MOTOKO capabilities |
|---|---|
| Level 0 (naked) | PTY scanning only (Tier 1b). No seccomp visibility. |
| Level 1 (sandboxed) | Tier 1a (seccomp) + Tier 1b (PTY) + host proxy audit |
| Level 2 (contained) | Full Tier 1 + host proxy audit + container events |
| Level 3 (air-gapped) | Full Tier 1 + network violation alerts |

MOTOKO also monitors host proxy operations — every proxied command (docker, nvidia-smi, kubectl, etc.) is audited. A proxied `docker compose up` triggers static analysis of the compose file before execution.

### MOTOKO calls into Navi (rare, CRITICAL only)

```rust
// MOTOKO holds Arc<dyn NaviApi> and calls:
navi.pause_pane_agent(pane_id)   // pause the agent's isolation
navi.pause_session(session_id)    // pause entire session
navi.kill_session(session_id)     // kill entire session (extreme)
```

### MOTOKO emits to event bus

```rust
// Events MOTOKO publishes:
Event::SecurityBlock { session, pane, detail }     // Tier 1 block
Event::Anomaly { session, pane, analysis }          // Tier 2 ANOMALY
Event::Critical { session, pane, analysis, action } // Tier 2 CRITICAL
Event::PostmortemComplete { session, report }        // Tier 3 result
Event::HostProxyBlock { session, pane, command }    // host proxy denied command
Event::IsolationLevelChange { session, pane, old_level, new_level } // level switch audit
```

MAGGI subscribes to these events and translates them to plain language for the user.

---

## Internal Panic Isolation

Each session's analysis runs in its own tokio task. Panics are caught at the task boundary:

```
Session "work" scanner task panics
    → catch_unwind catches it
    → log the panic
    → restart the scanner for session "work"
    → other sessions: completely unaffected
    → emit Event::MotokoInternalError to bus
    → MAGGI can inform the user if relevant
```

The MOTOKO process itself does not crash. Only the panicking session's scanner restarts.
