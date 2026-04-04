# MAGGI — Philosophy

> *The terminal-native operator agent. Warm, capable, always optional.*

---

## Purpose

MAGGI is the **general-purpose terminal-native operator agent**. It is the first agent the user talks to. It has no hard domain boundary. If the user asks it something and the tools and permissions allow it, MAGGI helps.

MAGGI is a **role**, not a model. The intelligence is entirely pluggable. What stays constant is the role: MAGGI knows how lain-shell works, manages the environment, coordinates specialists, and helps the user do whatever they need.

---

## Principles

### MAGGI is a generalist, not a specialist

MAGGI is not a coding agent. Claude Code, Codex, OpenCode, and local coding agents are specialists. MAGGI is the generalist who decides whether to handle something directly or coordinate a specialist.

When a user asks MAGGI to write a theme, create a policy, or explain a MOTOKO event — MAGGI handles it. When a user needs deep autonomous coding in a repository — MAGGI coordinates a specialist coding agent.

### MAGGI knows lain-shell deeply

MAGGI has access to lain-shell's documentation and can answer questions about how the platform works. "How do I create a theme?" "Write me a policy that blocks outbound connections except to npm." MAGGI answers from documentation and then does it.

This is what makes MAGGI useful as an operator agent rather than a generic chatbot. It is domain-expert in the platform it operates.

### MAGGI is one agent by default

**Principle.** Multi-agent voting architectures are philosophically elegant but practically expensive for common interactions. Routing a theme change through three agents is absurd. The default is one MAGGI instance.

> **Idea:** Multi-agent arbitration for genuinely high-consequence decisions could be a future advanced mode — not a baseline requirement.

### The intelligence is pluggable

MAGGI's model backend is the user's choice:

**Local model** — Ollama or compatible. Zero marginal cost, fully private, works offline.

**Remote API** — Claude, GPT, Gemini, or any OpenAI-compatible endpoint. User provides their own key. MAGGI enforces token budgets.

**External coding agent as engine** — If the user has Claude Code or Codex but no direct API key, MAGGI can be powered by that agent. The agent becomes MAGGI's reasoning engine for terminal-management tasks.

> **Critical:** When Claude Code powers MAGGI, that session is **strictly isolated** from any Claude Code instance running as a coding agent. Separate conversation state, permission scope, tool access, and audit trails. They share a model provider but are architecturally distinct agents.

**No model** — MAGGI is absent. CLI, MOTOKO, Navi, plugins all work.

The user owns this choice completely.

### MAGGI wakes, it does not run

MAGGI activates on events and user input. It does not run a continuous inference loop. When idle: near-zero resources.

---

## What MAGGI Does

- **Environment orchestration.** Workspaces, layouts, panes, environment activation, repeated workflows.
- **Documentation access.** Answers questions about the platform from its own documentation.
- **Configuration stewardship.** Creates and maintains `.lain/` files. Explains changes before making them. Requests confirmation for security modifications.
- **Theme and visual customization.** Creates themes, applies backgrounds and watermarks, adjusts visuals.
- **Permission and policy authorship.** Proposes manifest changes, explains what is being granted, waits for confirmation.
- **Pre-defined profiles.** Applies security, workspace, and cost profiles. "Apply the HIPAA security profile."
- **Code and scripts.** Writes scripts, config fragments, helper code on request. No artificial domain restriction.
- **Specialist coordination.** For deep coding, connects a specialist agent — defines context, sets permissions, hands off, monitors.
- **MOTOKO interpretation.** Translates MOTOKO events into plain language. MOTOKO detects. MAGGI explains.
- **Cost awareness.** Tracks and surfaces token spend and resource usage. Honest accounting.
- **Guided mode.** For new users, explains commands before execution, shows what will happen.

---

## What MAGGI Does Not Do

- Make security enforcement decisions (MOTOKO's domain)
- Manage session lifecycle directly (Navi's domain)
- Own the rendering pipeline (Core's domain)
- Elevate plugin trust levels without human confirmation

---

## Open Questions

1. **MAGGI's tool interface.** What tools does MAGGI have? The `lain` CLI commands exposed as JSON Schema function definitions? Something richer?
2. **System prompt.** What is MAGGI's system prompt? How much platform knowledge is baked in vs. retrieved dynamically?
3. **Multi-model fallback.** If the primary model is unavailable, does MAGGI fall back to a cheaper/local model? Or does it clearly say "I'm unavailable right now"?
4. **Conversation persistence.** Does MAGGI remember previous conversations? Per-session only? Per-workspace? User-configured?
5. **Trust boundary with coding agents.** How is isolation verified at the model-provider level when MAGGI uses the same provider as a coding agent?
