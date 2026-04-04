# MAGGI — Systems Design

> *Implementation details for the operator agent.*

---

## Decided Technology

| Component | Choice | Notes |
|---|---|---|
| Agent framework | Direct tool-calling over model API | `lain` CLI commands as JSON Schema function definitions. No framework dependency. |
| Knowledge/RAG | `lancedb` (vector) + `tantivy` (keyword) | Embedded, no separate process. Dual retrieval: semantic + keyword. |
| Local model backend | Ollama API | Dominant, zero friction, good Rust client support. |
| Remote model backend | OpenAI-compatible API | Covers Claude, GPT, Gemini, any compatible endpoint. |
| Model interface | Custom Rust trait | `MaggiBackend` trait. Implementations for Ollama, remote API, external agent. |

---

## Key Design Questions (Remaining)

1. **Tool system.** Every `lain` CLI subcommand is a potential MAGGI tool. Tools are defined as JSON Schema function definitions and passed to the model. The model calls them. MAGGI dispatches the call internally (since MAGGI and Core are in the same process, this is a function call, not IPC).

2. **Context window management.** As conversations grow, MAGGI must manage context. Options:
   - Sliding window with summarization of older messages
   - RAG retrieval of relevant prior conversation segments
   - Hard truncation with system prompt always preserved
   - Likely: summarization + RAG hybrid

3. **System prompt construction.** MAGGI's system prompt includes:
   - Role definition (operator agent, platform expert)
   - Current session context (active panes, agents, workspace)
   - Permission scope (what MAGGI can and cannot do)
   - Retrieved documentation (via RAG, on demand)
   - Static platform knowledge should be minimal — retrieve dynamically to save tokens

4. **Knowledge index.** MAGGI indexes lain-shell's documentation into `lancedb` for semantic retrieval. The index lives in `~/.local/state/lain-shell/maggi/knowledge/`. Rebuilt on lain-shell updates. Not synced across machines (binary, machine-specific).

5. **Local model integration.** Ollama client discovers available models. User configures which model powers MAGGI in config. If the configured model isn't available, MAGGI explains clearly what's needed.

6. **Conversation persistence.** Per-workspace by default. Stored in `.lain/agents/maggi/`. User can configure per-session (ephemeral) or per-user (global). Conversations are not synced unless the user explicitly chooses to (encrypted).

---

## References — Prior Art

### Claude Code, Codex CLI, OpenCode, Goose, Amp

The coding agents lain-shell hosts. They are tenants, not peers to MAGGI. Studying them reveals what agents need from the environment.

**Study directly:**
- **Tool schema design** — what capabilities agents declare, what operations they request, what structured responses they expect. MAGGI's tool system should be at least as capable as what these agents expect from their environments.
- **Approval/confirmation UX** — how agents surface dangerous actions for human review. Directly informs MOTOKO's Tier 2 CRITICAL pause-and-present model and how MAGGI explains MOTOKO events.
- **Git worktree usage** — Claude Code frequently uses multiple worktrees simultaneously. This motivated lain-shell's flat peer-session model (multiple sibling agent sessions, no nesting).
- **Session scoping** — how agents scope context to a repository. Informs how MAGGI hands off context to a specialist agent.

**Borrow:** Tool schema patterns, approval UX, session scoping strategies.
**Avoid:** The assumption that the terminal is a dumb text pipe. lain-shell inverts this.

### Nushell (for MAGGI's structured understanding)

Nushell's structured output concept informs how MAGGI can understand shell output beyond raw text.

**Inspiration for:**
- If the shell emits structured data (Nushell does this natively; bash/zsh via shell integration sequences), MAGGI can reason about command output structurally.
- The block model (ADR-005) in Core gives MAGGI structured command history. MAGGI can query "what was the output of the last failed command" without text scraping.
