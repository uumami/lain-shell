# MOTOKO — Systems Design

> *Implementation details for the security and observation layer.*

---

## Decided Technology

| Component | Choice | Notes |
|---|---|---|
| Tier 1 blocking | seccomp-BPF via `libseccomp` Rust bindings | Applied at agent process spawn. Immutable, kernel-enforced. |
| Tier 1 namespace isolation | `nix` crate for mount/network/PID namespaces | Direct syscall wrappers. |
| Tier 1 isolation | Four levels (ADR-009): naked, sandboxed (namespace+seccomp), contained (Podman), air-gapped | Level 1 (sandboxed) is default. Level 2+ uses Podman. |
| Tier 1 pattern matching | `aho-corasick` crate | Compiled multi-pattern. All patterns matched simultaneously. Microsecond latency. |
| Tier 1 network policy | iptables via nft (Level 1) / Podman network policy (Level 2+) | Forbidden packets dropped before leaving the namespace/container. |
| Host proxy monitoring | Custom audit stream | Every proxied command logged, allowlist enforced, compose files statically analyzed. |
| Tier 2 analysis | Custom | Behavioral baselines specific to lain-shell sessions. |
| Tier 3 reasoning | Model API call (pluggable) | No custom inference. Isolated pod, summarized input. |
| Audit log | Custom append-only JSONL + hash chain | If query complexity grows: add `rusqlite` index. |
| Rule engine | Custom Turing-incomplete DSL, Falco-shaped | Compiled to efficient matchers. Signed. |
| System observation | auditd (Linux) | No eBPF by default. eBPF via `aya` as future opt-in. |

---

## Key Design Questions (Remaining)

1. **seccomp-BPF profiles.** What syscalls are blocked by default for agent processes?
   - Likely deny: `execve` of binaries outside allowlist, `ptrace`, `mount`, `reboot`, raw socket creation
   - Likely allow: basic I/O, memory allocation, file operations within permitted paths
   - The profile is set at spawn and cannot be loosened. It can only be tightened.

2. **Pattern matching patterns.** What ships by default in the Aho-Corasick pattern set?
   - SSH private key content patterns
   - AWS/GCP/Azure credential patterns
   - Common API key formats
   - Known exfiltration encodings (base64-encoded secret patterns)
   - Custom patterns defined in `.lain/policies.toml`

3. **Behavioral baseline.**
   - What data: syscall frequency distribution, network connection frequency, filesystem access patterns, CPU/memory usage curves
   - How established: first N sessions for a given agent type (N is configurable, suggested default: 5)
   - What algorithm: statistical deviation from baseline. Simple z-score or percentile-based initially. ML-based anomaly detection as future enhancement.
   - Storage: baseline profiles stored in `~/.local/state/lain-shell/motoko/baselines/`

4. **Entropy analysis.** Detects encoded/encrypted data in agent output.
   - Measure: Shannon entropy over sliding window of agent PTY output
   - Normal code/text: entropy ~4.5-5.5 bits/byte
   - Compressed/encrypted data: entropy ~7.5-8.0 bits/byte
   - Trigger: sustained high entropy (> threshold for > N bytes) flags ANOMALY

5. **Audit log format.**
   ```json
   {
     "ts": "2026-04-03T14:22:01.003Z",
     "seq": 1042,
     "prev_hash": "sha256:abc123...",
     "event": "tier1_block",
     "session_id": "s-a1b2c3",
     "agent": "claude-code",
     "detail": {
       "syscall": "connect",
       "target": "198.51.100.1:443",
       "policy": "net_allowlist",
       "action": "deny"
     }
   }
   ```
   Each entry includes hash of previous entry. Chain is verifiable with `lain audit verify`.

6. **Tier 3 pod.** Isolated process. No network. No filesystem access except the summarized event window passed as stdin. No tools. No memory across invocations. Output: structured JSON verdict with plain English reasoning.

7. **Rule DSL.** Turing-incomplete. Falco-inspired. Pattern matching + conditions only.
   ```
   rule credential_file_read:
     event: fs.read
     condition: path matches "/home/*/.ssh/*" or path matches "/home/*/.aws/*"
     agent: any
     action: tier2_critical
     message: "Agent attempted to read credential file: {path}"
   ```
   Rules are compiled to efficient matchers at load time. Encrypted at rest. Signed.

---

## References — Prior Art

### Falco (CNCF)

Cloud-native runtime security. Uses kernel events to detect anomalous behavior in containers.

**Study directly:**
- **Rule schema** — Falco's condition-based, event-driven, human-readable rule format. MOTOKO's rule DSL should feel similar: conditions on events, Turing-incomplete, readable by a security-conscious human.
- **Community rule ecosystem** — how Falco manages contributions, versioning, and signing. Directly relevant to lain-shell's community MOTOKO rules governance question.
- **Event taxonomy** — Falco's classification of kernel events into meaningful security events. MOTOKO needs a similar taxonomy.

**Borrow:** Rule language shape, community governance model, event classification patterns.
**Avoid:** Falco itself as a dependency (too heavy, designed for Kubernetes, not terminal sessions).

### Tetragon (Cilium)

Kubernetes security using eBPF for kernel-level visibility.

**Study directly:**
- **eBPF observability model** — how Tetragon uses eBPF for deep process monitoring without polling. Reference for MOTOKO's future opt-in eBPF tier.
- **Lateral movement detection** — how Tetragon detects an agent pivoting from one context to another. Relevant to MOTOKO's cross-session correlation.
- **Policy-as-code** — Tetragon's approach to defining runtime behavior policies declaratively.

**Borrow:** eBPF patterns for the advanced tier. Detection strategies for exfiltration and lateral movement.
**Avoid:** eBPF as the default (attack surface too high for default tier). Kubernetes-specific assumptions.

### Sysdig

System inspection and forensics. Captures syscalls, network events, process behavior.

**Study directly:**
- **Postmortem workflow** — how a session's activity is reconstructed from structured event logs. Directly informs `lain postmortem` design.
- **Event summarization** — how raw kernel events become intelligible findings for human review. Relevant to how Tier 3 summarizes its input.
- **Forensic capture format** — Sysdig's capture format for replaying system activity. Inspiration for MOTOKO's forensic snapshot on CRITICAL events.

**Borrow:** Postmortem reconstruction patterns, event summarization strategies.
