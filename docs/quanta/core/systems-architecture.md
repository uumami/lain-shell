# Core — Systems Architecture

> *Components, boundaries, interfaces, and data flow within the Core quantum.*

---

## Components

```
┌──────────────────────────────────────────────────────┐
│                        CORE                          │
│                                                      │
│  ┌──────────┐  ┌──────────┐  ┌──────────┐          │
│  │   PTY    │  │ Renderer │  │  Config  │          │
│  │  Manager │  │ Pipeline │  │  Engine  │          │
│  └────┬─────┘  └────┬─────┘  └────┬─────┘          │
│       │             │              │                 │
│  ┌────┴─────┐  ┌────┴─────┐  ┌────┴─────┐          │
│  │   VTE    │  │  Theme   │  │   CLI    │          │
│  │  Parser  │  │  Engine  │  │          │          │
│  │ + Block  │  └──────────┘  └──────────┘          │
│  │  Model   │                                       │
│  └──────────┘  ┌──────────┐  ┌──────────┐          │
│                │Isolation │  │ Command  │          │
│  ┌──────────┐  │  Manager │  │  Router  │          │
│  │ Storage  │  └──────────┘  └──────────┘          │
│  └──────────┘                                       │
│                ┌──────────┐                          │
│                │  Event   │                          │
│                │   Bus    │                          │
│                └──────────┘                          │
└──────────────────────────────────────────────────────┘
```

### PTY Manager

Creates, destroys, and manages PTY instances. Uses `portable-pty`. PTYs are created on behalf of Navi (which owns the session/tab/pane structure). Each PTY may run inside an isolation environment (namespace or container) created by the Isolation Manager.

### VTE Parser + Block Model

Wraps `alacritty_terminal`. Parses ANSI/VT escape sequences, maintains the cell grid and scrollback buffer. Additionally implements the block model (ADR-005): detects command boundaries via OSC 133 shell integration sequences and maintains structured command history.

### Renderer Pipeline

Owns every pixel. GPU path via `wgpu` (ADR-004), CPU fallback via `softbuffer`. Renders in layers: background → cell grid → selection → cursor → overlays → status bar. The overlay compositor renders MOTOKO indicators, agent status, and Navi pane borders as native GPU layers.

### Config Engine

Manages `.lain/` directory and user-global config. Parses TOML via `serde`. Validates against JSON Schema. Publishes schemas via `lain schema`. Optional Lua config layer via `mlua`.

### Theme Engine

Visual customization: colors, fonts, image backgrounds, watermarks, transparency. Themes are TOML. Hot-reloadable.

### Isolation Manager

Creates and manages agent isolation at four configurable levels (ADR-009). Handles everything from naked execution (Level 0) to air-gapped containers (Level 3). Interfaces with Linux namespaces (Level 1), rootless Podman (Level 2-3), or nothing (Level 0). Queries MOTOKO for the appropriate seccomp profile and network policy, then applies them. Spawns host proxy processes alongside isolated agents for transparent Docker/GPU/tool access.

Also manages the host proxy lifecycle — one proxy per isolated agent pane, monitored by MOTOKO, allowlisted per `.lain/permissions.toml` (read from the safe mirror, not the repo copy — see ADR-010).

### Command Router

Single entry point for all commands — CLI, MCP, gRPC, Unix socket. Parses input into typed `LainCommand`, checks auth, dispatches to the appropriate quantum's trait interface, emits audit events. See `systems-architecture.md` (overall) for details.

### Event Bus

Two-tier event system. Mandatory events (security, audit, lifecycle) are synchronously written to MOTOKO's audit log — durable, ordered, fail-closed. Observable events (debug, informational, profile-filtered) are delivered via `tokio::broadcast` (in-process) or pub/sub over Unix sockets (separate processes) — best-effort, may drop for lagging receivers. See `systems-architecture.md` for the full event delivery model.

### PTY and VTE State Ownership

Core owns the full PTY pipeline: file descriptor (via `portable-pty`), VTE terminal state (`alacritty_terminal::Term` instance), and scrollback ring buffer. No other quantum holds raw PTY file descriptors or `Term` instances. Navi references PTYs by `PtyId` (a serializable identifier from `lain-types`), not by `PtyHandle` (an OS-level fd wrapper that is Core-internal only).

This means:
- Core feeds raw PTY bytes into `Term`, updating the cell grid and scrollback
- The renderer reads cells from Core (no round-trip through Navi)
- MOTOKO's dedicated channel receives raw bytes from Core
- If Navi crashes, Core keeps PTYs alive and can render "last known state"
- If Core crashes, all PTYs die (OS reclaims fds)

### Storage

State, cache, and data directories. Follows XDG on Linux (`~/.local/state/lain-shell/`, `~/.cache/lain-shell/`, `~/.local/share/lain-shell/`).

---

## Interfaces Exposed to Other Quanta

### To Navi

```rust
trait CorePtyApi {
    // Lifecycle
    async fn create_pty(&self, params: PtyParams) -> Result<PtyId>;
    async fn destroy_pty(&self, id: PtyId) -> Result<()>;
    async fn resize_pty(&self, id: PtyId, size: TerminalSize) -> Result<()>;
    async fn write_pty(&self, id: PtyId, data: &[u8]) -> Result<()>;

    // State queries (renderer and Navi use these)
    async fn get_cells(&self, id: PtyId, region: CellRegion) -> Result<CellGrid>;
    async fn get_scrollback(&self, id: PtyId, lines: usize) -> Result<Vec<Row>>;
    async fn get_cursor(&self, id: PtyId) -> Result<CursorState>;

    // Subscriptions
    fn subscribe_pty_output(&self, id: PtyId) -> PtyOutputStream;      // raw bytes (for MOTOKO)
    fn subscribe_pty_exit(&self, id: PtyId) -> PtyExitReceiver;
    fn subscribe_cell_changes(&self, id: PtyId) -> CellChangeStream;   // parsed diffs (for renderer)
}
// Note: PtyId is a serializable identifier from lain-types.
// PtyHandle (wrapping the OS fd) is Core-internal only.

trait CoreRenderApi {
    async fn allocate_surface(&self, region: RenderRegion) -> Result<SurfaceHandle>;
    async fn resize_surface(&self, handle: SurfaceHandle, region: RenderRegion) -> Result<()>;
    async fn free_surface(&self, handle: SurfaceHandle) -> Result<()>;
}

trait CoreIsolationApi {
    async fn create_isolation(&self, config: IsolationConfig) -> Result<IsolationHandle>;
    async fn destroy_isolation(&self, handle: IsolationHandle) -> Result<()>;
    async fn pause_isolation(&self, handle: IsolationHandle) -> Result<()>;
    async fn resume_isolation(&self, handle: IsolationHandle) -> Result<()>;
    async fn get_level(&self, handle: IsolationHandle) -> Result<IsolationLevel>;
    async fn switch_level(&self, handle: IsolationHandle, new_level: IsolationLevel) -> Result<IsolationHandle>;
}
```

### To MOTOKO

- **Dedicated channel**: PTY output byte stream (structurally isolated Unix socket)
- **Event bus**: command block events, config change events, isolation lifecycle events

```rust
trait CoreMotokoApi {
    fn subscribe_pty_stream(&self, id: PtyId) -> PtyByteStream;
    async fn get_seccomp_profile(&self, agent_type: AgentType) -> Result<SeccompProfile>;
    async fn get_host_proxy_log(&self, handle: IsolationHandle) -> Result<Vec<ProxiedCommand>>;
}
```

### To MAGGI

```rust
trait CoreConfigApi {
    async fn read_config(&self, key: ConfigKey) -> Result<ConfigValue>;
    async fn write_config(&self, key: ConfigKey, value: ConfigValue) -> Result<()>;
    async fn validate_config(&self, config: Config) -> Result<ValidationResult>;
    async fn get_schema(&self, config_type: ConfigType) -> Result<JsonSchema>;
}

trait CoreBlockApi {
    async fn get_recent_blocks(&self, pane: PaneId, count: usize) -> Result<Vec<CommandBlock>>;
    async fn query_blocks(&self, query: BlockQuery) -> Result<Vec<CommandBlock>>;
}
```

### To THE WIRED

```rust
trait CoreRouterApi {
    async fn execute_command(&self, cmd: LainCommand) -> Result<LainResponse>;
    async fn get_capabilities(&self) -> Result<Capabilities>;
    async fn get_schema(&self, config_type: ConfigType) -> Result<JsonSchema>;
}
```

---

## Internal Data Flow

```
User input (keyboard)
    │
    ├── Multiplexer keys → dispatched to Navi
    ├── Agent input → routed to active pane's PTY
    └── MAGGI invocation → routed to MAGGI

PTY output (from agent/shell process)
    │
    ├──▶ VTE parser (alacritty_terminal) → cell grid update
    ├──▶ Block model detector (OSC 133) → command block update
    ├──▶ Renderer → GPU pipeline → pixels on screen
    └──▶ Dedicated channel → MOTOKO (pattern matching)

Config change (user runs `lain config sync`)
    │
    ├──▶ Diff shown, user confirms
    ├──▶ Safe copy updated
    ├──▶ Config engine validates
    ├──▶ Event bus: ConfigChanged event
    └──▶ MOTOKO recompiles affected rules
```
