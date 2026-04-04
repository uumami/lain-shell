# ADR-010: .lain/ Config Mirror Pattern

## Status

Accepted

## Date

2026-04-04

## Context

The `.lain/` directory lives in the project repository. It's version-controlled and team-shared — which is good. But agents running inside the project directory have write access to it — which is bad.

If an agent can modify `.lain/permissions.toml`, it can weaken its own isolation. This is not a hypothetical attack — it's the first thing a compromised agent (or a compromised dependency running inside the agent) would try.

The agent cannot configure its own cage.

## Decision

**The `.lain/` directory in the repo is declarative source. The active configuration lives in a safe directory outside the agent's reach. Changes are applied via explicit sync.**

### How it works

```
IN THE REPO (version controlled, team-shared, agent-writable):
  ~/project/.lain/
  ├── config.toml
  ├── permissions.toml
  ├── policies.toml
  └── containers/
      └── Dockerfile

THE SAFE COPY (what lain-shell actually reads):
  ~/.local/state/lain-shell/workspaces/<project-hash>/
  ├── config.toml
  ├── permissions.toml
  ├── policies.toml
  └── containers/
      └── Dockerfile
```

- lain-shell, MOTOKO, and the Isolation Manager read ONLY from the safe copy
- The agent can modify `.lain/` in the repo (it's just project files) — but this has no effect on active isolation
- To update the active config, the user runs `lain config sync` or approves a MAGGI-initiated sync
- The sync shows a diff and requires user confirmation for security-relevant changes

### Sync flow

```
$ lain config sync

Comparing ~/project/.lain/ with active config...

  permissions.toml:
  - isolation = "sandboxed"
  + isolation = "naked"

  ⚠ Security-relevant change detected.
  Apply? [y/n]
```

MAGGI can also initiate sync conversationally:

```
MAGGI: "The .lain/permissions.toml in the repo was updated.
        Change: isolation level lowered from sandboxed to naked.
        Apply this change? [y/n]"
```

### What the agent sees

Inside isolation, the agent's bind mount includes the project directory, which contains `.lain/`. The agent CAN:
- Read `.lain/config.toml` (project config, helpful context)
- Read `.lain/containers/Dockerfile` (it may have generated it)
- Modify any file in `.lain/` (it's just files in the project)

But modifying `.lain/` in the repo does NOT change the active isolation. The active config is the safe copy, which the agent cannot reach.

### First-time setup

When a user first opens a project:

1. `lain init` creates a default `.lain/` in the project and copies it to the safe location
2. Or: the repo already has `.lain/` committed — `lain config sync` copies it to the safe location after user review
3. Or: MAGGI walks the user through setup conversationally

### Team workflow

Teams commit `.lain/` to git. When a developer pulls:

```
$ git pull   # gets updated .lain/
$ lain config sync   # reviews and applies changes
```

Code review catches malicious `.lain/` changes the same way it catches malicious Dockerfile changes.

## Consequences

### Enables

- Teams can share isolation policies via version control
- Agents cannot weaken their own isolation
- Security config changes require explicit human approval
- Git diff shows exactly what changed in security policy
- Code review catches policy tampering

### Costs

- Two copies of `.lain/` (repo + safe location)
- Users must run `lain config sync` after pulling policy changes
- Adds a step to the workflow

### Mitigations

- MAGGI can prompt for sync when it detects repo .lain/ differs from active config
- `lain config sync` can be added to git post-merge hooks
- The sync is fast (file copy + diff display)

## Alternatives Considered

### Agent has no access to .lain/

Don't bind-mount .lain/ into the agent's environment at all. Rejected because the agent benefits from reading project config (knowing the project's settings, seeing the Dockerfile it generated, etc.). The danger is write access to the *active* config, not read access to the repo copy.

### Filesystem permissions (read-only mount of .lain/)

Mount .lain/ read-only inside the agent's environment. This prevents modification but also prevents the agent from generating Dockerfiles or config suggestions. The mirror pattern is better — the agent can freely modify the repo copy (which is just a suggestion), and the human approves via sync.

### No separation (original implicit design)

lain-shell reads .lain/ directly from the repo. Rejected because this allows agents to modify their own isolation config.
