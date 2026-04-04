# MAGGI — Systems Architecture

> *Components, model interface, tool system, and context management.*

---

## Topology

Single MAGGI process with per-session conversation contexts. See ADR-008.

```
MAGGI (single process)
│
├── Model Backend (one connection)
│   ├── Ollama (local)
│   ├── Remote API (Claude, GPT, etc.)
│   └── External agent (Claude Code as engine)
│
├── Shared Knowledge Store
│   ├── lancedb (semantic search)
│   ├── tantivy (keyword search)
│   └── Platform documentation index
│
├── Session Contexts (isolated)
│   ├── Session "work"
│   │   ├── Conversation history
│   │   ├── Session state snapshot
│   │   ├── Per-pane contexts
│   │   └── Session-scoped scratchpad
│   ├── Session "ops"
│   │   └── (same structure)
│   └── Session "background"
│       └── (same structure)
│
└── Tool Dispatcher
    ├── Core tools (config, blocks, CLI)
    ├── Navi tools (session, tab, pane)
    └── MOTOKO tools (status, audit - read-only)
```

---

## Context Architecture — Pull, Not Push

MAGGI's context for any interaction is **assembled on demand**, not carried globally:

```
User types in Session "work", Tab 1, Pane A:
  "MAGGI, what did the last command output?"

Context assembled:
┌─────────────────────────────────────────┐
│ System prompt (role, permissions)        │  always present
│ Current session state (tabs, panes)     │  always present
│ Pane A context (cwd, recent blocks)     │  auto-included
│ Session "work" conversation history     │  auto-included
│                                         │
│ NOT included (pull if needed):          │
│ - Other panes' output                   │
│ - Other sessions' conversations          │
│ - Global knowledge store                │
└─────────────────────────────────────────┘

If MAGGI needs more:
  → tool call: core.get_recent_blocks(pane_c, 10)
  → tool call: maggi.search_knowledge("docker compose")
  → tool call: navi.get_session("ops")
```

### Memory scoping (four-dimensional)

| Dimension | Purpose | Examples |
|---|---|---|
| `user_id` | Who owns this memory | "uumami" |
| `session_id` | Which session | "work", "ops" |
| `agent_id` | Which agent created it | "maggi", "claude-code" |
| `scope` | Visibility level | pane, session, global |

- **Pane scope**: only visible in that pane's MAGGI context
- **Session scope**: visible across all panes in that session
- **Global scope**: queryable from any session (explicit tool call)

### Cross-session access

Always explicit. Always audited. Never automatic.

```
MAGGI in session "work":
  → tool call: maggi.query_session("ops")
  → receives: summary of "ops" session state
  → does NOT receive: full conversation history
```

---

## Components

### Model Backend

Abstract trait for all model providers:

```rust
#[async_trait]
trait MaggiBackend: Send + Sync {
    async fn complete(&self, context: CompletionContext, tools: Vec<ToolDef>) -> Result<CompletionResponse>;
    fn capabilities(&self) -> BackendCapabilities;
    fn cost_estimate(&self, context: &CompletionContext) -> TokenEstimate;
}
```

Implementations:
- `OllamaBackend` — local model via Ollama API
- `RemoteApiBackend` — OpenAI-compatible API (Claude, GPT, Gemini)
- `ExternalAgentBackend` — Claude Code or Codex as MAGGI's engine (strictly isolated from coding sessions)
- `NoneBackend` — MAGGI unavailable, returns clear error

### Tool Dispatcher

Every MAGGI tool is a JSON Schema function definition. The model calls tools by name. The dispatcher routes to the appropriate quantum's trait API:

```
Tool call from model: { "name": "create_pane", "params": { "session": "work", ... } }
    │
    ├── Dispatcher looks up "create_pane" → maps to NaviApi
    ├── Calls navi.create_pane(params)
    ├── Returns result to model
    └── Event emitted by Navi (not by dispatcher — emission is in the implementation)
```

Tools are grouped by quantum:
- **Core tools**: `read_config`, `write_config`, `query_blocks`, `get_schema`
- **Navi tools**: `create_session`, `create_tab`, `split_pane`, `list_sessions`, `get_pane_output`
- **MOTOKO tools**: `get_status`, `get_session_security`, `verify_audit` (all read-only)
- **MAGGI internal**: `search_knowledge`, `query_session`, `save_memory`

### Shared Knowledge Store

- `lancedb`: semantic vector search over platform documentation
- `tantivy`: keyword search for exact matches
- Indexed on install, re-indexed on updates
- Local to machine (not synced — binary, machine-specific)
- Stored in `~/.local/state/lain-shell/maggi/knowledge/`

### Session Context Manager

Manages per-session conversation threads:
- Maintains message history per session
- Handles context window management (summarization when history grows)
- Provides session state snapshots for system prompt assembly
- Persists conversations to `.lain/agents/maggi/` per workspace

---

## Interface Exposed to Other Quanta

```rust
#[async_trait]
trait MaggiApi: Send + Sync {
    // User interaction (via THE WIRED or direct)
    async fn send_message(&self, session: SessionId, message: UserMessage) -> Result<MaggiResponse>;

    // MOTOKO event interpretation
    async fn explain_security_event(&self, event: SecurityEvent) -> Result<Explanation>;

    // Session context queries (for THE WIRED external agents)
    async fn get_session_context(&self, session: SessionId) -> Result<SessionContext>;
}
```

### MAGGI calls into other quanta

```rust
// MAGGI holds:
core: Arc<dyn CoreConfigApi + CoreBlockApi>,
navi: Arc<dyn NaviApi>,
motoko: Arc<dyn MotokoApi>,  // read-only queries only
```

### MAGGI subscribes to event bus

```rust
// Events MAGGI listens for:
Event::Critical { .. }        // translate security events to plain language
Event::Anomaly { .. }         // queue for potential user notification
Event::SessionCreated { .. }  // update session context
Event::SessionDestroyed { .. } // cleanup session context
```
