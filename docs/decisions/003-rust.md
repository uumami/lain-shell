# ADR-003: Rust as Sole Language

## Status

Accepted

## Date

2026-04-03

## Context

lain-shell needs a language (or languages) for the terminal core, GPU renderer, multiplexer, security enforcement, agent runtime, and external API surfaces. Requirements:

- Memory safety without GC (real-time render loop, security-sensitive code)
- Zero-cost abstractions (performance ceiling comparable to C/C++)
- Strong async story (event-driven architecture, streaming PTY output)
- Mature ecosystem for terminal-relevant crates
- Platform abstraction via traits (Linux now, macOS later)

Three candidates were seriously evaluated: Rust, Go, and Zig. The question of polyglot (Rust + Go, Rust + Zig) was also evaluated.

## Decision

**Rust as the sole language.** No polyglot.

### Why Rust

- **Memory safety without GC** — critical for a real-time render loop and security-sensitive code
- **Zero-cost abstractions** — performance ceiling comparable to C/C++ without the safety tradeoffs
- **Ecosystem** — every modern high-performance terminal (Alacritty, WezTerm, Ghostty's `libvaxis` patterns) is in this space. The crates lain-shell needs exist and are mature: `portable-pty`, `alacritty_terminal`, `wgpu`, `tokio`, `tonic`, `cosmic-text`
- **Trait system** — natural fit for the quantum boundary pattern (`Arc<dyn NaviApi>`) and platform abstraction (Linux vs macOS behind traits)
- **Cargo ecosystem** — dependency management, build system, testing, benchmarking integrated
- **Type system expressiveness** — enums, pattern matching, and the ownership model encode invariants that other languages check at runtime

### Why not Go

Go was seriously considered, especially for the network-facing quanta (THE WIRED, MAGGI's HTTP client work):

**Go's strengths:**
- Faster compilation (10-50x faster than Rust)
- Simpler concurrency model (goroutines vs async/await)
- Easier onboarding for contributors
- Excellent HTTP/gRPC ecosystem (`grpc-go`, `net/http`)
- Lower cognitive overhead for CRUD-like service code

**Why rejected:**
- **GC pauses in the render loop.** Go's GC is low-latency (~1ms) but non-zero. A GPU render pipeline targeting 144fps has a 6.9ms frame budget. GC pauses are unpredictable and would cause visible frame drops.
- **No zero-cost abstractions.** Interface dispatch in Go always goes through a vtable. Rust's monomorphization means generic code compiles to specialized machine code.
- **Weaker type system.** Go lacks sum types (enums with data), pattern matching, and ownership semantics. The quantum boundary pattern (`Arc<dyn MotokoApi>`) has no natural Go equivalent — you'd use interfaces, but lose the compile-time guarantees about Send + Sync.
- **No ecosystem overlap.** `alacritty_terminal`, `wgpu`, `portable-pty` — the critical terminal crates don't exist in Go. We'd be building from scratch or using CGo to call into C/Rust, which negates Go's advantages.
- **CGo is painful.** If Go quanta need to call into Rust crates (and they would), CGo introduces build complexity, runtime overhead, and debugging difficulty.

### Why not Zig

**Zig's strengths:**
- Comptime is genuinely powerful (compile-time code generation)
- Excellent C interop (better than any other language)
- No hidden control flow, no hidden allocations
- Growing community with systems focus

**Why rejected:**
- **Ecosystem maturity.** Zig is pre-1.0. The standard library is still changing. Production-critical crates (`wgpu`, `tokio`, `tonic`, `alacritty_terminal`) don't exist in Zig.
- **No async runtime.** Zig has async but no production-ready runtime comparable to tokio.
- **Community size.** Finding contributors or getting help is harder. Documentation is sparse.
- **Risk for a foundational bet.** lain-shell is a multi-year project. Betting on a pre-1.0 language for the entire platform is a risk the project doesn't need to take.
- **Could revisit for specific components.** If a particular hot path (e.g., Aho-Corasick scanning) benefits from Zig's comptime, it could be called from Rust via C FFI. But this is optimization, not architecture.

### Why not polyglot (Rust + Go or Rust + Zig)

The IPC boundary between quanta already exists architecturally (gRPC over Unix sockets in production). This means different quanta *could* be different languages. The question is whether they *should* be.

**Why rejected:**
- **The IPC boundary exists for process isolation, not language mixing.** It's there so MOTOKO can crash without taking down Core. Adding language diversity to a boundary designed for reliability adds complexity without benefit.
- **Two build systems.** Cargo + Go modules (or Cargo + Zig build). CI doubles. Developer setup doubles. Debugging across the boundary requires proficiency in both.
- **Shared types diverge.** `SessionId`, `PaneId`, `LainCommand` — the types that cross quantum boundaries would need manual synchronization between two type systems, or protobuf becomes the source of truth even in development mode.
- **Contributor barrier.** "Know Rust" is one requirement. "Know Rust AND Go" is a much smaller pool.
- **The quanta that would benefit most from Go (THE WIRED, MAGGI) are the least performance-critical.** THE WIRED is I/O-bound. MAGGI is waiting on LLM responses. The Go advantages (fast compilation, simple concurrency) matter least here.

## Consequences

### Enables

- Single build system, single dependency tree, single type system
- Direct use of the richest terminal-development crate ecosystem
- Memory safety guarantees across the entire codebase
- Performance competitive with C/C++ terminals
- Natural platform abstraction via traits
- Shared types across all quantum boundaries without translation

### Costs

- Contributors must know Rust (smaller pool than Go)
- Compile times require attention (workspace splitting, incremental compilation tuning)
- Async Rust has a learning curve (lifetimes + async = complexity)
- Prototyping is slower than Go

### Mitigations

- Cargo workspace with small crates to keep incremental builds fast
- `cargo check` for rapid feedback during development
- `mold` or `lld` linker for faster linking
- Good error types and documentation to ease onboarding

## Alternatives Considered

See detailed analysis above for Go, Zig, polyglot, and C++ (rejected for memory safety concerns that directly contradict MOTOKO's security philosophy).
