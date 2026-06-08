## ADDED Requirements

### Requirement: Path security via mount namespace not seccomp
Level 1 isolation SHALL use mount namespace construction to control file visibility. Seccomp SHALL NOT be used for path-level file policy. The security escalation diagram in `systems-architecture.md` SHALL be corrected to reflect this.

#### Scenario: Agent cannot see ~/.ssh
- **WHEN** an agent at Level 1 runs `ls ~/.ssh` or `cat ~/.ssh/id_rsa`
- **THEN** the operation fails with ENOENT ("No such file or directory") because `~/.ssh` is not mounted

#### Scenario: Seccomp does not filter by path
- **WHEN** inspecting the Level 1 seccomp profile
- **THEN** no rules reference file paths — all rules filter by syscall number and argument class

### Requirement: Level 1 mount table is defined
Level 1 SHALL mount exactly these paths into the agent's namespace:

Read-only from host: `/usr`, `/bin`, `/lib`, `/lib64`, `/etc/resolv.conf`, `/etc/hosts`, `/etc/ssl`.
Read-write: project directory at `/workspace`, private `/tmp` (tmpfs).
Curated dotfiles read-only: `~/.cargo`, `~/.rustup`, `~/.npm`, `~/.config/git`.
Standard devices: `/dev/null`, `/dev/zero`, `/dev/urandom`.
PID-namespaced `/proc`.
Host proxy socket at `/run/lain/proxy.sock`.

Everything else SHALL NOT be mounted (invisible to the agent).

#### Scenario: Agent can access project files
- **WHEN** an agent at Level 1 reads or writes files in `/workspace`
- **THEN** the operations succeed, and changes are visible on the host in the project directory

#### Scenario: Agent cannot access other user projects
- **WHEN** an agent at Level 1 attempts to read `~/other-project/`
- **THEN** the path does not exist in the mount namespace

#### Scenario: Agent can use cargo
- **WHEN** an agent at Level 1 runs `cargo build`
- **THEN** it succeeds because `/usr`, `~/.cargo`, and `~/.rustup` are mounted read-only

#### Scenario: Sensitive directories are invisible
- **WHEN** an agent at Level 1 attempts to access `~/.ssh`, `~/.gnupg`, `~/.aws`, or `~/.kube`
- **THEN** all return ENOENT — the directories do not exist in the namespace

### Requirement: Level 1 seccomp blocks syscall classes
Level 1 seccomp profile SHALL block dangerous syscall classes: `ptrace`, `mount`/`umount2`, `setns`/`unshare`, `bpf`, `kexec_load`, `init_module`, `pivot_root`/`chroot`, `personality`, `reboot`, `swapon`/`swapoff`. All standard coding syscalls (file I/O, process creation, networking, memory management, terminal ops) SHALL be allowed.

#### Scenario: Agent cannot ptrace
- **WHEN** an agent at Level 1 calls `ptrace(PTRACE_ATTACH, pid)`
- **THEN** the syscall is blocked by seccomp and returns EPERM

#### Scenario: Agent cannot escape namespace
- **WHEN** an agent at Level 1 calls `setns()` or `unshare()`
- **THEN** the syscall is blocked by seccomp

#### Scenario: Agent can fork and exec
- **WHEN** an agent at Level 1 calls `fork()` and `execve()`
- **THEN** the syscalls succeed (PID namespace limits visibility, not capability)

### Requirement: Mount table is configurable per workspace
Additional read-only mounts (e.g., `~/.pyenv`, `~/.goenv`) SHALL be configurable in `.lain/permissions.toml`. The configuration is read from the safe mirror (ADR-010), not the repo copy.

#### Scenario: Python project mounts pyenv
- **WHEN** `.lain/permissions.toml` includes `extra_mounts = [{ source = "~/.pyenv", target = "/home/agent/.pyenv", mode = "ro" }]`
- **THEN** the Level 1 mount namespace includes `~/.pyenv` as read-only

#### Scenario: Extra mount from repo copy is ignored
- **WHEN** the repo `.lain/permissions.toml` adds an extra mount but the safe mirror has not been synced
- **THEN** the extra mount is not applied — only the safe mirror is read

### Requirement: Security escalation diagram is corrected
The security escalation diagram in `systems-architecture.md` SHALL replace "seccomp blocks the read syscall" with "mount namespace: path not mounted → ENOENT" as the primary Level 1 protection.

#### Scenario: Diagram shows mount namespace as primary protection
- **WHEN** reading the security escalation diagram in `systems-architecture.md`
- **THEN** the first tier shows "mount namespace: ~/.ssh not mounted → ENOENT", not "seccomp blocks the read syscall"
