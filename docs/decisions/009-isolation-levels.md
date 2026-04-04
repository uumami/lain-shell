# ADR-009: Isolation Levels — Spectrum, Not Binary

## Status

Accepted

## Date

2026-04-04

## Context

The original architecture described agent panes as running in "pods" (containers via rootless Podman). This creates problems:

1. **CUDA/GPU workloads** — GPU passthrough into containers requires NVIDIA Container Toolkit and specific runtime config. Some GPU operations don't work well containerized.
2. **Docker-in-Docker** — if an agent in a container runs `docker compose up`, you get nested containers. Path translations break, lifecycle management tangles, it's fragile and slow.
3. **Memory overhead** — each container has overhead. 4-5 agent panes on a 16GB laptop is painful.
4. **Not always desired** — some users trust their agents and don't want container overhead.

Meanwhile, container isolation isn't the only option. Linux namespaces, seccomp-BPF, and mount restrictions provide meaningful security without full containers.

## Decision

**Isolation is a configurable spectrum with four levels, not a binary "pod or no pod."**

### Level 0 — Naked

Agent runs directly on the host. Same user, same filesystem, same network. MOTOKO provides PTY scanning only (Tier 1b). Like running Claude Code today.

For: trusted agents, GPU workloads where passthrough is impractical, users who explicitly opt out.

### Level 1 — Sandboxed (default)

Agent runs in a Linux namespace with seccomp-BPF restrictions. No container runtime needed.

- **seccomp-BPF**: blocks dangerous syscalls (ptrace, mount, setns, bpf, kexec, etc.)
- **PID namespace**: agent can only see its own process tree
- **Mount namespace**: controlled filesystem view — project directory read-write, specific dotfiles read-only, host tools (/usr, /bin) read-only, /home empty except mounts
- **Network**: host network stack with outbound filtering via iptables
- **Host proxy**: optional, for Docker/GPU/tool access from inside the sandbox

MOTOKO: Tier 1a (seccomp) + Tier 1b (PTY scanning).

For: daily coding, the default for most users.

### Level 2 — Contained

Agent runs in a rootless Podman container with its own filesystem.

- Everything Level 1 provides, plus full filesystem isolation
- Agent sees only the container image + bind mounts
- Cannot fingerprint the host (doesn't see host /usr, /etc)
- Package installs don't leak to host
- Host proxy spawned alongside for Docker/GPU/tool access
- Shim binaries inside container transparently forward proxied commands
- Container image: curated base images or agent-generated Dockerfile

MOTOKO: full Tier 1.

For: untrusted agents, sensitive repos, team compliance requirements, reproducible environments.

### Level 3 — Air-gapped

Level 2 + all network access dropped. Agent can only read/write mounted files. No outbound connections.

For: maximum paranoia, sensitive repositories, compliance environments.

### Host Proxy

For Levels 1-3, a host proxy process runs alongside the sandbox/container. It:

- Listens on a Unix socket bind-mounted into the agent's environment
- Accepts allowlisted operations only (docker, nvidia-smi, cargo, kubectl, etc.)
- Executes commands on the host with the user's real Docker/GPU/toolchain access
- Translates paths between sandbox and host filesystem
- Streams stdout/stderr/exit code back to the agent
- Is monitored by MOTOKO (every proxied command is audited)

Inside the sandbox/container, proxied commands are replaced with shim binaries. The agent runs `docker compose up` normally — the shim forwards it to the host proxy. The agent doesn't know the difference.

The allowlist is defined in `.lain/permissions.toml` and enforced by MOTOKO.

### Per-pane, highest wins

Isolation level is configurable per-pane, with defaults at multiple levels:

- **Per-agent defaults**: claude-code defaults to Level 1, untrusted-review-bot defaults to Level 3
- **Per-repository policy**: `.lain/permissions.toml` can set minimum level
- **Per-directory policy**: user's global config can set minimum for specific paths

Resolution: `max(agent_default, repo_policy, directory_policy)`. The highest restriction wins.

Users can lower the level per-pane with explicit confirmation and audit logging.

### Pod Manager → Isolation Manager

The Core component previously called "Pod Manager" is renamed to **Isolation Manager**. It handles all four levels, not just containers.

```rust
trait CoreIsolationApi {
    async fn create_isolation(&self, config: IsolationConfig) -> Result<IsolationHandle>;
    async fn destroy_isolation(&self, handle: IsolationHandle) -> Result<()>;
    async fn pause_isolation(&self, handle: IsolationHandle) -> Result<()>;
    async fn resume_isolation(&self, handle: IsolationHandle) -> Result<()>;
    async fn get_level(&self, handle: IsolationHandle) -> Result<IsolationLevel>;
}
```

Navi holds `IsolationHandle` per pane, not `PodHandle`. Navi doesn't care about the isolation level — it just holds the handle.

## Consequences

### Enables

- Users who don't want containers can still get meaningful security (Level 1)
- CUDA/GPU workloads run without container friction (Level 0 or Level 1)
- Docker operations work transparently from any level via host proxy
- Security scales with trust level — untrusted agents get tighter isolation
- The same agent can work at different levels in different contexts
- Teams can enforce minimum isolation levels per repository

### Costs

- Four levels to implement and test instead of one
- Host proxy is a new component to build and maintain
- Shim binaries need curation and maintenance
- Level 1 with mount namespace requires careful filesystem view construction

### Risks

- Level 1 mount namespace misconfiguration could expose unintended files
- Host proxy is a privilege escalation path — its allowlist must be correct
- Users may over-lower isolation levels and blame lain-shell when things go wrong

### Mitigations

- Curated, tested isolation profiles ship with lain-shell
- MOTOKO monitors host proxy operations
- Level changes are audited and require user confirmation
- Documentation clearly explains what each level provides and doesn't

## Alternatives Considered

### Containers only (original design)

Rejected. Creates friction for CUDA, Docker, and users who don't want overhead. Not all users can or want to run containers.

### No isolation

Rejected. Contradicts the philosophy ("security is structural, not promised").

### Only seccomp (no namespaces)

Considered. seccomp alone provides syscall filtering but doesn't restrict filesystem visibility. Adding PID and mount namespaces at Level 1 is low cost and significantly increases security.
