# Open Questions

> *These require dedicated design work, benchmarking, or real usage data.*
> *They are not deferred out of laziness.*
> *As answers emerge, they move to the relevant systems doc or become ADRs.*
> *Resolved questions are marked with ✓ and the document where the answer lives.*

---

## Platform-Wide

1. **First launch UX.** What does the configuration moment feel like? How does MAGGI participate if present? How does the CLI flow work without a model?

2. ~~**Cross-quantum interface definitions.**~~ ✓ Resolved in `systems-architecture.md`. Hybrid model: direct trait calls for request/response, event bus for observation, dedicated channels for security streams.

3. ~~**Event bus design.**~~ ✓ Resolved in `systems-architecture.md`. Profile-filtered pub/sub. Mandatory events cannot be disabled. Dedicated MOTOKO channel is structurally isolated from the bus.

4. **Plugin sandboxing.** WASM via `wasmtime` is decided. But: what is the plugin API surface? What can plugins do that MAGGI cannot? How do plugins interact with the quanta? See `future.md` for captured ideas.

---

## Core

5. **Rendering pipeline.** How do GPU (wgpu) and CPU (softbuffer) paths coexist behind a trait? How does the overlay compositor work? Layer model is sketched in `quanta/core/systems-design.md` but implementation details remain.

6. **Config hot-reload.** Which config changes take effect immediately vs. requiring a session restart?

7. **Theme engine.** How do themes compose? How do image backgrounds work? What format?

8. ~~**Pod manager details.**~~ ✓ Resolved. Pod Manager renamed to Isolation Manager. Four isolation levels defined (ADR-009). Host proxy pattern for Docker/GPU access. See `systems-architecture.md` and `quanta/core/systems-architecture.md`.

---

## Navi

9. **Session persistence format.** TOML serialization decided. But exact schema and what survives restart vs. what is reconstructed needs detail. See `quanta/navi/systems-design.md`.

10. **Layout engine.** Binary tree decided. But: how do layout presets work? How does zoom interact with the tree? How are saved layouts applied?

11. **Keybinding system.** Multiple keymaps decided (native, tmux-compatible, Zellij-style). But: how is the dispatch system implemented? How are custom keymaps validated?

12. ~~**Container-backed panes.**~~ ✓ Resolved. Generalized to isolation levels (ADR-009). Navi requests isolation from Core's Isolation Manager. Level 0 (naked), Level 1 (sandboxed, default), Level 2 (contained), Level 3 (air-gapped). Host proxy for transparent Docker/GPU access. See `systems-architecture.md`.

13. ~~**Multi-client attach model.**~~ ✓ Resolved. Session state (tabs, panes, PTYs) is shared. View state (active tab, focused pane, scroll, terminal size) is per-client on `AttachHandle`. Independent navigation. Smallest-client-wins resize. See `quanta/navi/systems-architecture.md`.

---

## MOTOKO

13. **Behavioral baseline establishment.** How is the baseline built for a new agent? Suggested: first N sessions (default: 5). But: what data is collected? What algorithm? How is the baseline stored?

14. **Tier 3 model selection.** Smallest model that reliably distinguishes genuine exfiltration from false positive. Empirical.

15. **Tier 3 isolation.** How is the summarized event window prepared? What information is included vs. excluded?

16. **macOS security parity.** macOS lacks seccomp-BPF. `sandbox-exec` is deprecated. What is the Tier 1 story on macOS?

17. **Rule DSL design.** Falco-shaped, Turing-incomplete. Sketch exists in `quanta/motoko/systems-design.md`. Full language specification needed.

---

## MAGGI

18. **Trust boundary with coding agents.** ✓ Partially resolved. Four-dimensional memory scoping (user, session, agent, scope) in `systems-architecture.md`. Model-provider level isolation still needs verification at implementation time.

19. **System prompt design.** How much platform knowledge is baked into the prompt vs. retrieved via RAG? See `future.md` for MAGGI infrastructure ideas.

20. ~~**Conversation persistence.**~~ ✓ Resolved. Per-session by default. Configurable. Four-dimensional memory scoping. See `systems-architecture.md`.

---

## THE WIRED

21. **Auth model.** Token-based. Short-lived by default. Scoped to permissions. But: how are tokens issued and verified? Integration with MOTOKO audit?

22. **gRPC service design.** Sketched in `quanta/the-wired/systems-design.md`. Full protobuf definitions needed.

23. **Remote session buffer.** For mobile/remote: what format? Differential updates? See `future.md`.

---

## Community and Governance

24. **Community MOTOKO rule governance.** Who reviews and signs community rules? NERV as interim custodian, community governance as goal. See `future.md`.

25. **Plugin registry governance.** Community-operated, but how? Who signs? Trust levels?

26. **Shared team sessions.** Architecture supports it. UX and permission model need design. See `future.md`.

---

## Isolation and Security

27. **Host proxy shim curation.** Which commands ship with shims by default (docker, podman, nvidia-smi, kubectl)? How do users add custom shims? Is it a config list or do they create shim binaries?

28. **Host proxy path translation.** When translating paths from sandbox to host (e.g., for Docker volume mounts in compose files), how deep does the rewriting go? Does it rewrite inside YAML files, or only command-line arguments?

29. **Nested container monitoring.** When an agent launches `docker compose` via host proxy, MOTOKO can statically analyze the compose file and monitor host-level network/resource usage. But it cannot scan PTY output inside composed containers. Is this acceptable, or does MOTOKO need deeper visibility here?

30. **Agent checkpoint/restore for level switching.** Seamless isolation level switching requires checkpointing agent state (conversation, working directory, environment) and restoring in a new isolation environment. Which agents support this? What's the fallback for agents that don't? See `future.md`.

31. **Level 1 mount namespace construction.** Exactly what directories are visible at Level 1? How is the mount view constructed? Which dotfiles are mounted by default vs. configured?

32. **Config sync automation.** Can `lain config sync` be automated via git hooks (post-merge, post-checkout)? Should MAGGI auto-detect when repo `.lain/` differs from active config?
