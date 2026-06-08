# Sealed-cage spike — RESULTS (THROWAWAY)

> Deliverable of the checklist `2026-06-08-sealed-cage-spike-checklist.md`.
> Validates the central bet of the new-seed foundation: the `sealed` preset
> (ephemeral Firecracker microVM, no network, workspace-only fs) can be **safe
> AND pleasant**. Throwaway validation — code lives in `.spike/sealed-cage/`
> (gitignored), the numbers below are the point.

Date: 2026-06-08. Host: Pop!_OS 22.04, Intel VT-x, 8 cores, 31 GiB RAM,
Firecracker v1.16.0, KVM via `/dev/kvm` ACL (no sudo/kvm-group).

## Setup (what actually ran)

- Guest kernel: `vmlinux-5.10.233` (Firecracker CI bucket, v1.12/x86_64).
- Guest rootfs: Ubuntu 24.04 CI squashfs -> repacked to a 1.5 GB ext4 (rootless,
  `mke2fs -d`), plus a hermetic **Zig 0.13** toolchain (`zig cc` = static
  Clang+musl, runs identically on host and guest -> fair compile comparison,
  no host-glibc dependency), the SQLite 3.46 amalgamation as the workload, a
  static `vsock-client` (cage-agent stand-in), and a minimal bash `init`.
- Host launcher drives Firecracker over its API socket; guest serial markers
  are timestamped host-side. `sealed` == **no network interface is ever
  configured** (the `/network-interfaces` API call is simply never made).
- vCPU=2, mem=2048 MiB for all runs.

## Exit criteria — the four numbers

### 1. Boot time (launch -> in-guest userspace ready): ~370-400 ms  [PASS]

| config | boot-to-userspace |
|---|---|
| stock args, full serial logging, bash init | ~1400 ms |
| `quiet`, bash init | ~900 ms |
| `quiet` + i8042 disabled, **static init (floor)** | **~370 ms** |
| `quiet` + i8042 disabled, **bash init (real userspace)** | **~400 ms** |

The win was diagnostic, not luck: a verbose boot timeline showed a single
**480 ms gap probing the i8042 PS/2 keyboard controller** — a device a microVM
has no use for. Disabling it (`i8042.noaux i8042.nomux i8042.nopnp
i8042.dumbkbd`) plus `quiet` dropped boot from 1.4 s to ~0.4 s. Notably a real
bash userspace adds only ~30 ms over a do-nothing static init, so the residual
cost is **kernel boot**, not our init. Hits the "low hundreds of ms" target with
a stock CI kernel; a purpose-built minimal kernel config (drop loop/iSCSI/ACPI
probing) is the known path toward Firecracker's ~125 ms floor if we want it.

### 2. In-cage compile overhead (vs host, identical toolchain): ~1.0x  [PASS]

Workload: `zig cc -O2 -c sqlite3.c` (257k-line single TU, ~7.5 MB object,
~460 MB RSS — a real CPU-bound optimizing compile). compiler-rt warm excluded;
cache-busted each run so it is genuine work, not a cache hit.

- In-cage: **88.70 s** (rc=0, identical 7.5 MB object).
- Host baseline (2 cores, same machine, back-to-back): 81.58 / 82.84 / 87.39 s
  (median **82.84 s**).
- Ratio (in-cage / host): **~1.07x** (~6-7% over native, and partly cold-IO:
  the guest read `sqlite3.c` cold from ext4 while the host hit page cache).

Confirms the microVM is **not** gVisor's weak axis: CPU-bound build work runs at
near-native speed because there is no syscall interception layer — the guest
runs on bare KVM.

### 3. Egress structurally blocked (SC-1 in miniature): blocked  [PASS]

From inside the sealed cage:

- Interfaces: only `lo` (`/sys/class/net` = `lo`). No NIC exists.
- Exfil of a marked canary attempted three ways, all FAILED:
  - TCP -> 1.1.1.1:53  =>  `OSError 101 Network is unreachable`
  - UDP DNS -> 8.8.8.8:53  =>  `OSError 101 Network is unreachable`
  - DNS resolve  =>  `gaierror -3 Temporary failure in name resolution`

This is **structural, not policy**: there is no network device to misconfigure,
no firewall rule to get wrong. The canary cannot leave because there is no path.

### 4. Brokered tool-call round-trip over vsock: ~360 µs median  [PASS]

In-cage `vsock-client` (cage-agent stand-in) -> host broker (MOTOKO stand-in)
over virtio-vsock, 1000 iterations:

- 1000/1000 succeeded.
- min **222 µs**, p50 **360 µs**, p99 **1986 µs**, avg **473 µs**.
- The allow/deny decision (`/workspace/` write -> allow, else deny) was computed
  **on the host broker** and returned to the cage. The decision path does not
  live in the cage — exactly the MOTOKO-on-the-host model.

Sub-millisecond median for a brokered call is comfortably inside "pleasant" for
per-tool-call mediation.

## Decision gate: HOLDS

Good boot time (~400 ms, path to ~125 ms known), near-native compile (~1.0x),
exfil structurally blocked, sub-ms broker latency. The central promise of the
`sealed` preset is real. Proceed to design+build in risk order (foundation §10).

## Caveats / honest edges (inputs to the next steps)

- **KVM is the hard dependency, and it is the real "any Linux machine" gate.**
  Everything here assumes `/dev/kvm` + hardware virt. Absent on many targets
  (cloud VMs without nested virt, CI, containers, locked-down laptops). The
  installer must detect KVM and decide the fallback (refuse `sealed`, or drop to
  a weaker gVisor/namespace tier). See memory `installer-any-linux`. Brainstorm
  packaging next, now that the bet is confirmed.
- Build-time vs runtime: `unsquashfs`/`mke2fs`/the CI bucket/the dev distro are
  **build-time only** and never ship. The runtime payload is small and fixed:
  `firecracker` + bundled `vmlinux` + prebuilt `rootfs.ext4` + cage agent.
- Boot floor measured with a minimal init; a real session also needs the cage
  agent + workspace mount + warm toolchain — still userspace-cheap (~30 ms over
  floor here), but measure the real thing when it exists.
- Compile workload is one big C TU; representative of CPU-bound builds. A
  Rust/cargo build (many crates, heavy linking, more IO) is a different mix —
  worth a second data point before over-generalizing "near-native".
- Numbers are single-machine, lightly loaded; host baseline itself varied
  72-86 s across runs, so treat the compile ratio as "~1.0x, within noise",
  not a precise figure.
