# ADR-006: Hybrid Communication Model

## Status

Accepted

## Date

2026-04-04

## Context

The five quanta (Core, Navi, MOTOKO, MAGGI, THE WIRED) need to communicate. They will be deployable as separate processes. Three communication patterns were considered: direct trait calls only, pure event bus, or a hybrid.

Key requirements:
- Request/response must be natural (not awkward correlation IDs)
- MOTOKO must observe everything without being threaded through every call site
- MOTOKO's PTY observation must be structurally isolated from general traffic
- All interface types must be serializable (for separate-process deployment)
- Mandatory security events must never be droppable

## Decision

Hybrid model with three communication patterns:

1. **Direct trait calls** for request/response. Async traits with owned, serializable types. In-process: function call. Separate process: gRPC over Unix socket. Caller doesn't know which.

2. **Profile-filtered event bus** for observation. Quanta emit events from within their trait method implementations. Subscribers receive what their profile allows. Mandatory events (security, lifecycle) cannot be filtered out.

3. **Dedicated channels** for high-frequency, security-critical streams. Specifically: PTY output from Core to MOTOKO flows over a structurally isolated pipe, not the event bus.

The event emission discipline: trait method implementations emit events, not callers. Callers cannot forget to emit because they don't do it.

## Consequences

### Enables

- Natural request/response (direct calls)
- Natural observation (event bus)
- MOTOKO observes without being wired into every call
- Security stream is structurally isolated
- Audit log writes via bus subscription
- Profile-based event filtering for performance tuning
- Same trait interface works in-process and across processes

### Costs

- Two communication mechanisms to understand
- Risk of forgetting event emission in new trait methods (mitigated by discipline: emission in implementation, not caller)
- Event bus infrastructure to build and maintain

## Alternatives Considered

### Direct trait calls only

No natural observation point for MOTOKO. Would require MOTOKO to be explicitly called from every method in every quantum. Rejected.

### Pure event bus

Request/response is awkward (correlation IDs, response channels). Adds latency and complexity to the most common operations. Rejected.
