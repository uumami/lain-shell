# THE WIRED — Systems Design

> *Implementation details for the connectivity layer.*

---

## Decided Technology

| Component | Choice | Notes |
|---|---|---|
| gRPC | `tonic` over tokio | Typed, schema-driven. Service definitions in protobuf. |
| MCP server | Rust MCP SDK | Standard agent protocol. Do not reinvent. |
| Unix socket | tokio Unix socket | For local plugins and tooling. Low overhead. |
| Auth | Token-based | Short-lived, scoped. Details TBD. |
| Serialization | protobuf (gRPC), JSON (MCP, Unix socket) | Protobuf for structured RPC, JSON for simpler surfaces. |

---

## Key Design Questions (Remaining)

1. **Internal command model.** All three protocol surfaces translate to the same internal command model — the same model the `lain` CLI uses. Define this model first, then the protocol translations are mechanical.

   ```
   LainCommand {
     action: Action,           // e.g., SessionCreate, PaneList, ConfigGet
     target: Option<Target>,   // e.g., specific session, pane, agent
     params: Params,           // action-specific typed parameters
     auth: AuthContext,        // who is making the request, what permissions
   }
   
   LainResponse {
     status: Status,
     data: Option<Value>,      // structured response
     events: Vec<Event>,       // side-effect events (for MOTOKO audit)
   }
   ```

2. **gRPC service design.** Likely services:
   - `SessionService` — create, list, attach, detach, destroy sessions
   - `PaneService` — create, list, resize, navigate panes
   - `ConfigService` — get, set, validate, schema
   - `AgentService` — register, query status, coordinate
   - `AuditService` — query audit log, verify chain
   - `SystemService` — doctor, capabilities, version

3. **MCP tool mapping.** Each `lain` CLI command maps to an MCP tool. When an agent connects via MCP, it discovers available tools through the standard MCP protocol. Tool definitions are generated from the same JSON Schema used by MAGGI's tool system.

4. **Unix socket protocol.** JSON-RPC 2.0 over Unix domain socket. Simple, well-specified, tooling exists. No need for a custom protocol.

5. **Auth implementation.**
   - Tokens issued by Navi at connection time
   - Scoped to specific permissions (read-only, session-manage, agent-control, admin)
   - Short-lived by default (expire with session)
   - Long-lived tokens available for automation (explicit user creation, logged)
   - Token verification on every request through THE WIRED
   - MOTOKO audits all authenticated requests

6. **Session buffer serialization.** For remote/mobile access:
   - Terminal cell grid serialized as a compact binary format
   - Differential updates (only changed cells transmitted)
   - Compressed (zstd)
   - This is a future feature — design the protocol to support it, don't implement initially

---

## References — Prior Art

### MCP Protocol

The Model Context Protocol is the standard for agent-to-tool communication. Claude Code, OpenCode, and others speak MCP natively.

**Use directly:** Rust MCP SDK as a dependency. Implement lain-shell as an MCP server. Do not deviate from the protocol spec.

**Study:** How other MCP servers define their tool schemas. The patterns for describing capabilities, required parameters, and structured responses.

### gRPC Ecosystem

`tonic` is mature, well-documented, and integrates with tokio.

**Study:** How other Rust gRPC services structure their protobuf definitions. Use `buf` for protobuf linting and code generation if the schema grows complex.

### tmux Control Mode (as protocol reference)

Even though Navi replaces tmux, tmux's control mode protocol shows what operations a multiplexer needs to expose programmatically: session listing, pane creation, layout manipulation, key sending, output capture.

**Borrow:** The operation taxonomy — what THE WIRED needs to expose for session/pane management.
**Avoid:** Text-based protocol design. THE WIRED is typed and structured.
