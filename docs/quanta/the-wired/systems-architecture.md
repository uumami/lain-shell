# THE WIRED — Systems Architecture

> *Protocol surfaces, connection model, command routing, and integration with other quanta.*

---

## Topology

Single WIRED process exposing three protocol surfaces. All translate to the same internal command model.

```
THE WIRED (single process)
│
├── Protocol Listeners
│   ├── MCP Server (for agents: Claude Code, OpenCode, etc.)
│   ├── gRPC Server (for programmatic integrations, CI/CD, companion apps)
│   └── Unix Socket Listener (for local scripts, plugins, tooling)
│
├── Protocol Translation Layer
│   ├── MCP → LainCommand translator
│   ├── gRPC → LainCommand translator
│   └── JSON-RPC → LainCommand translator
│
├── Auth Engine
│   ├── Token verification
│   ├── Permission scoping
│   └── Connection lifecycle management
│
└── Command Forwarder
    └── Forwards LainCommand → Core's Command Router
```

---

## Data Flow

```
External client (agent, script, remote app)
    │
    ├── MCP protocol ──────▶ MCP Server
    ├── gRPC protocol ─────▶ gRPC Server
    └── JSON-RPC over UDS ─▶ Unix Socket Listener
                                │
                     ┌──────────▼──────────┐
                     │  Protocol-specific   │
                     │  deserialization     │
                     │  → LainCommand       │
                     └──────────┬──────────┘
                                │
                     ┌──────────▼──────────┐
                     │    Auth Engine       │
                     │  1. Verify token     │
                     │  2. Check permissions│
                     │  3. Attach AuthCtx   │
                     └──────────┬──────────┘
                                │
                     ┌──────────▼──────────┐
                     │  Command Forwarder   │
                     │  → Core Router       │
                     │  (same as CLI path)  │
                     └──────────┬──────────┘
                                │
                     ┌──────────▼──────────┐
                     │  Core Command Router │
                     │  dispatches to the   │
                     │  target quantum      │
                     └──────────┬──────────┘
                                │
                     ┌──────────▼──────────┐
                     │  Response flows back │
                     │  → LainResponse      │
                     │  → protocol-specific │
                     │    serialization     │
                     │  → client receives   │
                     └─────────────────────┘
```

The key invariant: **THE WIRED never executes commands itself.** It translates, authenticates, forwards, and serializes responses. All execution goes through Core's Command Router, which handles dispatch and audit.

---

## Components

### MCP Server

Exposes lain-shell as a standard MCP server. Agents that speak MCP natively discover available tools through the protocol's tool listing mechanism.

- Tool definitions generated from the same JSON Schema used by MAGGI's tool system
- Each `lain` CLI command maps to an MCP tool
- Supports streaming responses for long-running operations (agent output, session monitoring)
- Connection lifecycle: agent connects → authenticates → discovers tools → invokes tools → disconnects

```
MCP tool example:
{
  "name": "session_create",
  "description": "Create a new lain-shell session",
  "parameters": {
    "name": { "type": "string" },
    "workspace": { "type": "string", "optional": true }
  }
}
→ translated to: LainCommand { action: SessionCreate, params: { name, workspace }, auth: ... }
```

### gRPC Server

Typed, schema-driven service using protobuf definitions. For programmatic integrations that need strong typing and code generation.

Services:
- `SessionService` — create, list, attach, detach, destroy sessions
- `PaneService` — create, list, resize, split, navigate, get output
- `ConfigService` — get, set, validate, schema
- `AgentService` — register, query status, coordinate
- `AuditService` — query audit log, verify chain
- `SystemService` — doctor, capabilities, version, health

Each RPC method translates to a `LainCommand`. The gRPC service definition is the external contract; the internal model is the same.

### Unix Socket Listener

JSON-RPC 2.0 over Unix domain socket at `/run/user/$UID/lain/wired.sock`.

- For local scripts, plugins, and simple tooling
- No protobuf dependency required — plain JSON
- Same command model, same auth, same audit
- Lowest overhead for local integrations

### Auth Engine

Every connection through THE WIRED carries a permission context.

```
Connection established
    │
    ├── Token presented (bearer token in header/handshake)
    ├── Token verified against token store
    ├── Permission scope extracted:
    │   ├── read-only         (query sessions, config, status)
    │   ├── session-manage    (create/destroy sessions, panes)
    │   ├── agent-control     (spawn/pause/kill agents)
    │   └── admin             (config writes, policy changes)
    │
    └── AuthContext attached to every LainCommand from this connection
```

Token lifecycle:
- **Short-lived tokens**: issued at connection time, expire with session (default)
- **Long-lived tokens**: explicit user creation, for automation/CI (logged, auditable)
- **Revocation**: immediate, checked on every request
- **Issuance**: via `lain token create --scope read-only --ttl 1h`

THE WIRED verifies tokens. MOTOKO audits all authenticated requests via the event bus.

### Command Forwarder

Thin layer that takes an authenticated `LainCommand` and forwards it to Core's Command Router. The forwarder:
- Adds connection metadata (protocol, remote address if applicable, connection ID)
- Emits `ConnectionRequest` events to the event bus (MOTOKO observes)
- Receives `LainResponse` from the router
- Translates back to protocol-specific response format

---

## Internal Command Model

All three protocol surfaces translate to the same model — the same model the `lain` CLI uses:

```rust
struct LainCommand {
    action: Action,           // SessionCreate, PaneList, ConfigGet, etc.
    target: Option<Target>,   // specific session, pane, agent
    params: Params,           // action-specific typed parameters
    auth: AuthContext,        // who, what permissions, connection info
}

struct LainResponse {
    status: Status,           // Ok, Error, Streaming
    data: Option<Value>,      // structured response
    events: Vec<Event>,       // side-effect events (for audit)
}
```

This is the same entry point whether the command comes from:
- `lain session create work` (CLI → Core Router directly)
- An MCP agent calling `session_create` (MCP → THE WIRED → Core Router)
- A CI script via gRPC (gRPC → THE WIRED → Core Router)
- A local plugin via Unix socket (JSON-RPC → THE WIRED → Core Router)

---

## Interface Exposed to Other Quanta

```rust
#[async_trait]
trait WiredApi: Send + Sync {
    // Lifecycle
    async fn start(&self) -> Result<()>;
    async fn stop(&self) -> Result<()>;
    async fn health(&self) -> Result<WiredHealth>;

    // Connection management
    async fn list_connections(&self) -> Result<Vec<ConnectionInfo>>;
    async fn revoke_connection(&self, conn_id: ConnectionId) -> Result<()>;

    // Token management
    async fn create_token(&self, scope: TokenScope, ttl: Duration) -> Result<Token>;
    async fn revoke_token(&self, token_id: TokenId) -> Result<()>;
    async fn list_tokens(&self) -> Result<Vec<TokenInfo>>;

    // Discovery
    async fn get_capabilities(&self) -> Result<Capabilities>;
    async fn get_schema(&self, resource: SchemaResource) -> Result<JsonSchema>;
}
```

### THE WIRED calls into Core

```rust
// THE WIRED holds:
router: Arc<dyn CoreRouterApi>,  // all commands go through Core's router
```

THE WIRED does NOT hold direct references to Navi, MOTOKO, or MAGGI. All command dispatch goes through Core's Command Router. This enforces single-entry-point auth and audit.

### THE WIRED emits to event bus

```rust
// Events THE WIRED publishes:
Event::ConnectionEstablished { protocol, auth_context, conn_id }
Event::ConnectionClosed { conn_id, reason }
Event::ExternalCommand { conn_id, command_summary }  // no sensitive params
Event::TokenCreated { token_id, scope, ttl }
Event::TokenRevoked { token_id }
```

MOTOKO subscribes to these events for security monitoring (e.g., unusual connection patterns, repeated auth failures, privilege escalation attempts).

### THE WIRED subscribes to event bus

```rust
// Events THE WIRED listens for:
Event::SessionCreated { .. }    // update capability advertisements
Event::SessionDestroyed { .. }  // clean up streaming connections to that session
Event::ConfigChanged { .. }     // reload auth config if relevant
```

---

## Protocol-Specific Concerns

### MCP — Streaming

MCP supports streaming for tool results. THE WIRED uses this for:
- Agent output streaming (client watches an agent pane's output)
- Long-running command progress
- Session state change notifications

Internally, these map to tokio streams from Core/Navi.

### gRPC — Bidirectional Streaming

gRPC server-side and bidirectional streaming for:
- `AttachSession` — bidirectional stream (client sends input, receives rendered output)
- `WatchEvents` — server-side stream of filtered events
- `MonitorPaneOutput` — server-side stream of pane output

### Unix Socket — Simplicity

JSON-RPC 2.0 is request/response only. For streaming use cases, the Unix socket protocol supports:
- Polling (`get_pane_output` with `since` parameter)
- Or upgrading to a raw stream mode for specific operations

---

## Integration Points

- **Core**: THE WIRED forwards all commands to Core's Command Router. This is the only direct call path.
- **MOTOKO**: MOTOKO observes THE WIRED's events. MOTOKO does NOT call into THE WIRED. If MOTOKO detects a suspicious connection pattern, it acts through Navi (pause session) or the event bus (alert).
- **MAGGI**: MAGGI can query THE WIRED's connection state via tools (`wired.list_connections`). External agents can interact with MAGGI through THE WIRED's MCP/gRPC surface.
- **Navi**: External requests for session management route through THE WIRED → Core Router → Navi. THE WIRED never calls Navi directly.

---

## Discovery

How external agents find a running lain-shell instance:

- **Local**: well-known socket path `/run/user/$UID/lain/wired.sock`
- **MCP**: standard MCP discovery mechanisms (config file pointing to socket or process)
- **Network** (future): mDNS advertisement or explicit configuration. Not implemented initially, but the architecture supports it — THE WIRED listeners can bind to network sockets with TLS.

---

## Security Model

THE WIRED is an attack surface. Design accordingly:

1. **Auth on every request.** No "already authenticated" session state that persists beyond token verification.
2. **Principle of least privilege.** Default token scope is `read-only`. Escalation requires explicit user action.
3. **Rate limiting.** Per-connection request rate limits to prevent abuse.
4. **No raw PTY exposure.** External clients cannot get raw PTY byte streams. They get structured command output or rendered cell grids.
5. **MOTOKO observes everything.** Every connection, every command, every token operation emits events that MOTOKO processes.
6. **Token rotation.** Short-lived tokens by default. Long-lived tokens require explicit creation and are flagged in audit.
