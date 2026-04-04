# Future — Ideas Worth Building Later

> *These are ideas that emerged during design conversations. They are not part of the current scope but the architecture must not make them impossible. They represent where lain-shell could grow.*
>
> *Each idea includes enough context to pick it up months from now without losing the reasoning.*

---

## MAGGI Agent Infrastructure

> *Emerged from: context architecture discussion, MAGGI as operator agent design*

MAGGI currently has a trait interface and a model backend. To become a truly capable operator agent, it needs full agentic infrastructure:

### System prompt engineering

MAGGI's system prompt must be carefully designed:
- Role definition (operator agent, platform expert, not coding specialist)
- Dynamic context injection (current session state, active panes, permissions)
- Tool schemas (JSON Schema definitions of every `lain` CLI command)
- Platform knowledge (retrieved via RAG, not baked in — saves tokens)
- Calibrated confidence (when to act vs. when to ask)

The system prompt is not a static string — it's a **compiled context** assembled per interaction from the current session state, user preferences, and retrieved knowledge.

### Skill repository

Named, versioned capabilities MAGGI can invoke. Like Claude Code's skills but for terminal operations:
- "apply-security-profile" — applies a predefined security configuration
- "setup-workspace" — creates a workspace layout from a template
- "diagnose-network" — investigates connectivity issues
- "explain-motoko-event" — translates security events to plain language
- "author-policy" — conversational policy creation with confirmation

Skills are composable. MAGGI can chain skills for complex operations. Skills are version-controlled and team-shareable.

### RAG pipeline

- `lancedb` for semantic search over platform documentation
- `tantivy` for keyword search
- Documentation indexed on install and re-indexed on updates
- User knowledge sources as additional RAG targets (see Obsidian integration below)
- Chunk size, embedding model, and retrieval strategy need empirical tuning

### Memory system (four-dimensional)

Every memory record scoped by:
- `user_id` — who
- `session_id` — which session
- `agent_id` — which agent
- `scope` — pane / session / global

Per-pane memory: working directory, recent command blocks, pane-specific tool results.
Per-session memory: conversation history, session decisions, scratchpad.
Global memory: platform knowledge, user preferences, cross-session facts.

Cross-session access is always explicit (tool call), never automatic. Always audited.

### Conversation management

Per-session conversation threads with:
- Sliding window with summarization of older messages
- RAG retrieval of relevant prior conversation segments
- Hard truncation with system prompt always preserved
- Context window budget management (track tokens, stay within limits)
- Conversation persistence: per-session by default, configurable

### MAGGI delegation model

MAGGI coordinates specialist sub-agents for heavy tasks:
- "MAGGI, run a deep analysis of this codebase" → spawns specialist in its own pod
- Specialist gets scoped permissions, time budget, cost budget
- MAGGI monitors progress, presents results
- MAGGI is the coordinator; specialists are workers
- This extends the existing pattern of MAGGI coordinating external coding agents

---

## MOTOKO Advanced Infrastructure

> *Emerged from: security tier design, behavioral analysis discussion*

### Community rule ecosystem

- Anonymized, opt-in rule contribution
- Rules are signed packages with version history
- NERV as interim custodian, community governance as long-term goal
- Rule categories: language-specific, framework-specific, compliance-specific
- Review process: automated testing against false positive corpus + human review
- Distribution: signed packages from community registry, installable via `lain motoko rules install`

### Advanced eBPF tier (opt-in)

- Full eBPF observability via `aya` crate
- Requires elevated privileges — explicit opt-in with clear warning
- Deeper kernel visibility: file access patterns, network flow analysis, process tree tracking
- Complements Tier 1 (seccomp) and Tier 2 (statistical) — does not replace them
- Attack surface acknowledged: eBPF verifier CVEs are real

### ML-based anomaly detection

- Replace or augment Tier 2's statistical z-score with learned behavioral models
- Train on the user's own agent sessions (private, local)
- Detect subtle behavioral shifts that statistical methods miss
- Higher computational cost — run asynchronously, not on the critical path
- This is a research-grade idea — empirical validation needed before committing

### Visual event timeline

- Timeline overlay showing MOTOKO events during a session
- Reviewable after the fact — scrub through the session, see what MOTOKO saw
- Integrated with `lain postmortem` for visual forensic review
- Rendered as a Core overlay, not a separate tool

### Cross-session threat intelligence

- MOTOKO detects patterns spanning multiple agents:
  - Agent A probes filesystem paths, agent B attempts network connections to the same targets
  - One agent's output entropy spikes after another agent accessed sensitive files
- Requires the single-MOTOKO architecture (cross-session correlator)
- Event correlation algorithms need design work

---

## Knowledge Base Integration

> *Emerged from: MAGGI context architecture discussion*

### Obsidian / markdown knowledge bases

- User points MAGGI at a directory of markdown files (Obsidian vault, wiki, notes)
- MAGGI indexes them into its RAG pipeline (lancedb + tantivy)
- User's domain knowledge becomes part of MAGGI's context
- Index is local, not synced (binary, machine-specific)
- Re-indexed on file changes (via inotify/fswatch)

### Project documentation as context

- `.lain/knowledge/` directory for project-specific documentation
- Team members can add documentation that MAGGI uses for context
- Indexed alongside platform documentation
- Useful for: "MAGGI, our deployment process requires X" — MAGGI knows from docs, not just conversation

### Structured knowledge graph

- Beyond vector search: a typed knowledge graph of project facts
- Entities: files, functions, agents, policies, sessions, commands
- Relationships: "file X is tested by Y", "policy Z blocks command W"
- MAGGI queries the graph for structured reasoning
- This is ambitious — vector search + keyword search may be sufficient for a long time

---

## Session Intelligence

> *Emerged from: block model discussion, Nushell inspiration*

### Session recording and replay

- Record terminal sessions (input/output/timing)
- Replay for debugging, review, or training
- Separate from audit logs — this is about reproducing what happened visually
- Storage: compact binary format with timing information
- Replay in lain-shell or export to asciinema format

### Structured command analytics

- Block model enables: "show me all failed commands this week"
- Track command patterns: which commands are used most, which fail most
- Surface insights: "you run `docker compose down && docker compose up` 12 times today — create an alias?"
- MAGGI can use analytics for proactive suggestions

### Workspace templates

- Named, shareable configurations that define a complete workspace:
  - Session name, tabs, pane layout
  - Commands to run in each pane
  - Agent configuration per pane
  - Security profile
  - Cost budget
- `navi apply dev-backend` sets up your whole environment
- Team-shareable via `.lain/templates/`

---

## Mobile and Remote Access

> *Emerged from: THE WIRED philosophy, network-addressable design*

### Mobile companion app

- Approve destructive agent actions from phone
- View session status and MOTOKO alerts
- Receive push notifications for CRITICAL events
- THE WIRED's gRPC bridge is the backend — no additional server needed
- Auth: token-based, short-lived, device-specific
- Terminal buffer: differential updates, compressed

### Remote dashboard

- Web-based UI served by lain-shell for complex visualizations
- Agent activity graphs, cost charts, MOTOKO event feeds
- Not a terminal in a browser — a dashboard for monitoring
- Served locally by WIRED, accessible over network with auth

### Shared team sessions

- Multiple users attached to the same Navi session
- Permission-scoped views: admin sees everything, reviewer sees output only
- Real-time collaboration on terminal workflows
- Architecture supports it (attach/detach model). UX needs design.

---

## Plugin Ecosystem

> *Emerged from: WASM sandboxing discussion, Zellij plugin model*

### Plugin API design

- Define what plugins can do that MAGGI cannot:
  - Low-level rendering hooks (custom pane renderers)
  - Custom protocol handlers (new THE WIRED surfaces)
  - Persistent background processes
  - Custom MOTOKO rule types
- Define what MAGGI can do that plugins should not replicate
- WASM sandbox via `wasmtime` — plugins run in isolation

### Plugin development experience

- Plugin SDK with documentation
- Example plugins: custom status bar, git integration, cost dashboard
- Plugin testing framework
- Plugin signing and distribution

---

## Cost Intelligence

> *Emerged from: cost management philosophy*

### Cost prediction

- Before an agent session starts, estimate likely cost based on:
  - Historical patterns for similar tasks
  - Model pricing
  - Estimated token usage
- Let the user decide whether to proceed
- MAGGI presents: "This task typically costs $0.50-$2.00. Proceed?"

### Automatic backend switching

- When approaching a cost threshold, automatically switch to a cheaper model
- Configurable per policy: "switch to local model after $5/session"
- MAGGI explains the switch: "Switching to local model to stay within budget"

---

## Seamless Isolation Level Switching

> *Emerged from: isolation levels discussion, agent workflow continuity*

### Agent checkpoint/restore

When switching isolation levels mid-session (e.g., from air-gapped to sandboxed because the agent needs Docker), the agent conversation should survive:

1. Checkpoint agent state (conversation history, working directory, environment, agent-specific state)
2. Destroy old isolation environment
3. Create new isolation environment at the new level
4. Restore agent from checkpoint
5. Agent resumes — user sees a brief pause, not a restart

Requires agents to implement a checkpoint/restore interface:
```rust
trait AgentCheckpoint {
    async fn checkpoint(&self) -> Result<AgentState>;
    async fn restore(&self, state: AgentState) -> Result<()>;
}
```

For agents that don't implement this: fallback is restart fresh at the new level in the same directory. User loses conversation but keeps files.

For major agents (Claude Code, Codex): ensure checkpoint/restore is supported. Claude Code's `--resume` and conversation files make this feasible.

### MAGGI-assisted level switching

MAGGI observes agent failures caused by isolation restrictions (e.g., "docker: command not found" at Level 3) and offers to switch levels:

"Claude needs Docker access. Currently at Level 3 (air-gapped). Switch to Level 1 (sandboxed)? [y/n]"

User confirms → MAGGI triggers the switch → agent resumes with Docker access via host proxy.

---

## Host Proxy Ecosystem

> *Emerged from: isolation levels discussion, Docker-in-Docker problem*

### Curated shim profiles

Ship shim profiles per workflow:
- **web-dev**: docker, docker-compose, node, npm, yarn
- **ml-ops**: docker, nvidia-smi, python, pip, conda
- **devops**: docker, kubectl, terraform, helm, aws
- **rust-dev**: cargo, rustup, docker

Users can compose profiles and add custom shims.

### Compose file static analysis

Before the host proxy executes `docker compose up`, MOTOKO statically analyzes the compose file:
- Flag `privileged: true`
- Flag `network_mode: host`
- Flag volume mounts outside the project directory
- Flag images from untrusted registries
- Block known-malicious image hashes

This runs at Tier 1 (zero tokens, deterministic) and can block execution before it starts.

### Output fidelity

Host proxy allocates a pseudo-TTY for each proxied command, preserving:
- ANSI colors
- Progress bars (Docker build, npm install)
- Cursor movement
- Terminal width negotiation

The shim passes through the agent's terminal size. Like SSH — creates a remote PTY and tunnels it over the Unix socket.

---

*These ideas are captured here so they can be picked up in future design sessions. The current architecture must not make any of them impossible — but none of them are in scope for initial implementation.*

*When an idea is ready to be designed, it moves to a proper change proposal via OpenSpec.*
