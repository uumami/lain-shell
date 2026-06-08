# Distribution & Installability — Design (Principle + Strategic Direction)

> Status: AGREED via brainstorming, 2026-06-08. This is a **principle + direction**
> doc, not an implementation spec. Per foundation §10 (risk-first), the detailed
> installer is deferred until there is a core binary to package. See
> `2026-06-08-new-seed-foundation-design.md` (adds SC-11) and
> `2026-06-08-sealed-cage-spike-results.md` (why KVM is the real gate).

## 0. What this document is

Captures a foundation-level principle that the original design and the new-seed
foundation both missed: **lain-shell must be installable on any Linux machine, on
the user's terms** — as a native package (`apt`, `dnf`, Arch, Alpine) *and* via a
single distro-agnostic installer — with no distro hardcoded or privileged in the
core. It resolves the one hard tension the sealed-cage spike surfaced: the strong
`sealed` tier requires KVM, which not every machine has.

It records four decisions (scope, capability model, packaging direction, sealed-
asset delivery) and explicitly defers the build.

## 1. The principle — new success criterion SC-11

> **SC-11 — Installable anywhere.** lain-shell installs and runs on any modern
> Linux, on the user's terms: a native package on the major families (`apt install`
> / `dnf install` / Arch / Alpine) **and** a single distro-agnostic installer for
> everything else. No distro is hardcoded or privileged in the core; distro, arch,
> and libc are **detected, never assumed**. The base install is small and
> self-sufficient; heavyweight, hardware-gated capabilities (the sealed microVM
> stack) are fetched only when actually used. Verified by installing on a matrix of
> distros/arches and confirming the terminal runs and correctly reports which tiers
> are available.

This turns "installable anywhere" into a testable contract, like SC-1..SC-10.

## 2. Capability-degradation model (resolves the KVM tension)

The install is **layered by what the host can actually do**, detected at install
and at session-spawn — never assumed. This preserves SC-5 ("stands alone": the
terminal is fully usable with zero agents and zero network, anywhere).

```
  ANY Linux ----------------> terminal core (open preset)         [always]
       |
       +- userns/seccomp on ----------> standard sandbox AVAILABLE
       +- /dev/kvm present & usable --> sealed (microVM) AVAILABLE
       +- neither ---------------------> sealed OFF, shown honestly;
                                        strongest-available tier offered,
                                        LABELED as weaker (does NOT meet
                                        SC-2 kernel-0-day resistance)
```

The terminal core is *unconditionally* available everywhere (SC-5). The sandbox
tiers stack on top by capability: `standard` where namespaces/seccomp are enabled
(the common case), `sealed` where KVM is present. Each is detected, not assumed.

- A **`preflight` / `doctor`** probe reports per-tier availability: `/dev/kvm`
  presence and access (ACL or `kvm` group), CPU virtualization flags, nested-virt
  if running inside a VM, CPU arch, libc, and `userns`/cgroup availability (for the
  standard tier). This is SC-8 ("legible security state") extended to capability.
- **Non-negotiable honesty rule:** a fallback tier is **never labeled `sealed`**.
  When KVM is absent, the UI shows the real guarantee of whatever tier is offered.
  We surface the weaker guarantee plainly rather than give false comfort. A user
  who thinks they have microVM isolation but has namespaces is worse off than one
  who knows the truth.
- KVM availability is the **real "any Linux" gate**, not packaging. Commonly absent
  on cloud VMs without nested virt, CI runners, containers, and locked-down laptops.

## 3. Packaging strategy (direction A: one build, many channels)

Chosen because it is the only option that satisfies **both** halves of SC-11 at
once — the native-package feel *and* a general non-hardcoded installer — at low
maintenance cost.

| Layer | Decision |
|---|---|
| Build | One relocatable, mostly-static core binary (+ standard-sandbox bits). Per-arch (x86_64, aarch64). |
| Native packages | One package source -> `.deb` / `.rpm` / Arch / `.apk` generated via a single tool (`nfpm`-class). We host **signed apt + dnf repos** so `apt install lain-shell` and `dnf install lain-shell` are real. |
| Universal installer | `curl https://.../install.sh \| sh` that **detects** distro/arch/libc and installs the static binary + a user service + PATH. This is the "general installer, not hardcoded." Plain tarball + checksums also published. |
| Sealed assets | Guest kernel + rootfs + `firecracker`, arch-specific (~tens of MB). **Fetched on first sealed use** (KVM present), signature/hash-verified, cached. Base install stays smallest. |
| Air-gapped | An **offline bundle** path (`... sealed-pack import <bundle>`) so machines without network can still enable sealed from a manually-copied artifact. |
| Supply chain | Packages signed; repo metadata signed; sealed assets verified before exec. Consistent with the threat model — the distribution path is itself an attack surface. |

**Deferred channels (not v1, YAGNI):** Homebrew-on-Linux tap, Nix flake,
AppImage/Flatpak, and official distro maintainership (the slow, rule-bound,
out-of-our-control path that also resists our bundled VMM assets).

## 4. Scope discipline (risk-first)

Per foundation §10, we do **not** write the installer spec now. There is no core
binary to package yet; building distribution machinery first would be document-
first, the exact failure mode new-seed exists to avoid. This doc ratifies the
principle (SC-11), the capability model, the packaging direction, and the sealed-
asset delivery — and stops. The detailed installer/`preflight` design is picked up
when the core terminal exists, in risk order.

## 5. Open items

- **Subsystem name (low stakes):** the installer/bootstrap could be engineering
  name `installer` (or `bootstrap`) with a codename settled later (candidate:
  **ENTRY PLUG** — the Eva insertion mechanism; fits "inserting the system onto a
  host"). Not load-bearing; can settle alongside the still-open product name.
- **Repo hosting / signing-key custody:** where the apt/dnf repos live and how keys
  are managed — an implementation-time concern, flagged here so it is not forgotten.
- **Distro/arch test matrix:** the concrete list that makes SC-11 testable — define
  when there is something to install.

## 6. Decisions log

| # | Decision |
|---|---|
| D1 | Scope: terminal core + standard sandbox install on ANY Linux; `sealed` is a capability that lights up where KVM exists and degrades honestly elsewhere. |
| D2 | Add SC-11 ("installable anywhere") to the foundation success criteria. |
| D3 | Packaging direction A: one build -> native packages (deb/rpm/Arch/apk) via one source + signed apt/dnf repos + a distro-detecting universal `install.sh`. |
| D4 | Sealed assets fetched on first sealed use (verified, cached); offline-bundle import for air-gapped. |
| D5 | Honesty rule: a non-microVM fallback tier is never presented as `sealed`. |
| D6 | Implementation deferred until a core binary exists (risk-first). |
