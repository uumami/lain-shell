## ADDED Requirements

### Requirement: Core owns PTY file descriptors and VTE state
Core SHALL own the full PTY pipeline: file descriptor, `alacritty_terminal::Term` instance, and scrollback ring buffer. No other quantum SHALL hold raw PTY file descriptors or `Term` instances.

#### Scenario: Core creates and owns PTY
- **WHEN** Navi requests a new PTY via `CorePtyApi::create_pty`
- **THEN** Core creates the PTY, allocates a `Term`, and returns a `PtyId` (not a file descriptor)

#### Scenario: Core maintains cell grid
- **WHEN** a PTY produces output
- **THEN** Core feeds bytes into `alacritty_terminal::Term`, updating the cell grid and scrollback

### Requirement: Navi stores PtyId not PtyHandle
Navi SHALL reference PTYs by `PtyId` (a serializable identifier), not by `PtyHandle` (which wraps an OS file descriptor). `PtyHandle` SHALL be Core-internal only.

#### Scenario: PtyId is serializable
- **WHEN** Navi persists session state to TOML
- **THEN** `PtyId` values are written as integers or UUIDs, not OS handles

#### Scenario: Navi queries cells via Core API
- **WHEN** a renderer needs the current cell grid for a pane
- **THEN** Navi (or the renderer directly) calls `CorePtyApi::get_cells(pty_id, region)` to get the data from Core

### Requirement: CorePtyApi includes cell-grid query methods
`CorePtyApi` SHALL expose `get_cells`, `get_scrollback`, and `get_cursor` methods so that consumers can read parsed terminal state without accessing the `Term` directly.

#### Scenario: Renderer reads cells
- **WHEN** the renderer needs to draw a pane
- **THEN** it calls `get_cells(pty_id, visible_region)` and receives a `CellGrid` with character, style, and width data

#### Scenario: Scrollback query returns lines
- **WHEN** a user scrolls up in a pane
- **THEN** `get_scrollback(pty_id, 100)` returns up to 100 lines of scrollback history

### Requirement: Crash semantics are explicit
The system SHALL document and enforce these crash semantics: Core death kills all PTYs (fd closed, VTE state lost). Navi death preserves PTYs in Core (topology lost, recoverable from persisted TOML). MOTOKO death preserves PTYs and sessions (Tier 1 kernel enforcement survives).

#### Scenario: Navi crashes and recovers
- **WHEN** Navi process dies and the supervisor restarts it
- **THEN** Navi reads persisted session topology from TOML, queries Core for live PTYs, and reconciles: panes with matching live PTYs are restored, panes whose PTYs died show "[exited]"

#### Scenario: Core crashes
- **WHEN** Core dies
- **THEN** all PTYs are destroyed (OS reclaims fds), Navi's pane references become stale, supervisor restarts Core, Navi re-requests PTYs for restorable panes (new processes, scrollback lost)

#### Scenario: Cell change subscription for renderer
- **WHEN** a PTY produces output and the cell grid changes
- **THEN** Core emits a diff on `subscribe_cell_changes(pty_id)` that the renderer consumes to update only changed regions
