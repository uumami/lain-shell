# Sealed-cage spike — checklist (THROWAWAY)

> Purpose: validate the central bet of the new-seed foundation before writing any more design.
> The `sealed` preset (ephemeral microVM, no network, workspace-only fs) must be **safe AND
> pleasant**. This is throwaway validation code — keep it light, optimize for learning, discard
> after. Do NOT build production structure here.
>
> Foundation contract: `docs/superpowers/specs/2026-06-08-new-seed-foundation-design.md`
> (validates SC-1 data confinement, SC-2 escape resistance, and the EVA cage + MOTOKO broker).

## Environment status (DONE 2026-06-08)

- Bare metal, Intel VT-x, `kvm`/`kvm_intel` loaded; `systemd-detect-virt` = none.
- `/dev/kvm` access via ACL `user:uumami:rw-` — open test passed. **No sudo / no kvm group needed.**
- `firecracker` v1.16.0 and `jailer` v1.16.0 installed at `~/.local/bin` (on PATH).
- `curl`/`wget` present; network reachable.

## Still needed before first boot (fetch in the spike session)

- [ ] Guest **kernel** (uncompressed `vmlinux`, x86_64). Use a known-good Firecracker CI kernel or build minimal.
- [ ] **Root filesystem** (ext4 image) with a minimal init + a sample coding workload (e.g. a small Rust or C repo to compile).
- [ ] Host-side launcher: drive Firecracker via its API socket (config the kernel, rootfs, vCPUs, mem).
- [ ] **vsock** device configured (host<->guest control channel for the brokered tool-call; NOT network).

## Exit criteria (measure, write the numbers down)

1. [ ] **Boot time** — wall-clock from launch to in-guest userspace ready. Target: low hundreds of ms. Honest "one-keystroke sealed session" depends on this.
2. [ ] **In-cage workload overhead** — compile/test the sample repo inside the cage vs on host. Target: near-native. This is gVisor's weak axis; confirm microVM is not.
3. [ ] **Egress is structurally blocked** — `sealed` has no network device. From inside, attempt to exfiltrate marked data (DNS, TCP, any path). Confirm it FAILS. This is SC-1 in miniature.
4. [ ] **Brokered tool-call round-trip** — one request from in-cage `cage-agent` over vsock to a host-side stub standing in for the MOTOKO broker, decision returned. Measure latency. Confirm the decision path does not live in the cage.

## Decision gate

- **Holds** (good boot time, near-native compile, exfil blocked, acceptable broker latency) -> central promise is real; proceed to design+build in risk order (foundation §10).
- **Breaks** -> we learned it now with ~300 lines, not month four. Revisit the ceiling (gVisor? different VMM? relax `sealed` definition?) before committing.

## Discipline

- Throwaway. No crates, no trait layer, no abstractions — a script/binary that boots a VM and prints numbers.
- One concern at a time; the four exit criteria can each be a separate tiny experiment.
- Capture the numbers in a short results note; that note (not the code) is the deliverable.
