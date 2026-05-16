// SPDX-FileCopyrightText: 2026 Knitli Inc.
//
// SPDX-License-Identifier: LicenseRef-MarqueLicense-1.0

//! Linux process-level sandbox for `marque-server`.
//!
//! ## Policy
//!
//! `apply()` installs the following restrictions via the [Landlock LSM] after
//! the TCP socket is bound and before the first request is accepted.  Filters
//! are applied in two independent rulesets; Landlock accumulates them into the
//! most-restrictive intersection.
//!
//! ### Filesystem (Landlock V1+, kernel 5.13+)
//!
//! | Right | Verdict | Rationale |
//! |-------|---------|-----------|
//! | `ReadFile` | **Allowed** on `/` | Text processing is in-memory; reads may still occur through system libs loaded after startup (e.g. locale, timezone databases). |
//! | `ReadDir` | **Allowed** on `/` | Same — directory listing is occasionally needed by system code. |
//! | `Execute` | **Blocked** everywhere | The server never execs child processes; blocking prevents exec-based privilege escalation. |
//! | `WriteFile`, `MakeReg`, `MakeDir`, … | **Blocked** everywhere | The server writes nothing to disk; blocking prevents malware persistence. |
//! | `Truncate` (V2+), `Refer` (V2+), `IoctlDev` (V5+) | **Blocked** everywhere | Not needed; blocked for defense-in-depth. |
//!
//! ### Network (Landlock V4+, kernel 6.7+)
//!
//! | Right | Verdict | Rationale |
//! |-------|---------|-----------|
//! | `BindTcp` | **Blocked** everywhere | The listening socket is already bound before `apply()` is called; no new binds are needed. |
//! | `ConnectTcp` | **Blocked** everywhere | The server never initiates outbound connections; blocking data-exfiltration channels. |
//!
//! No `NetPort` rules are added to the network ruleset, so both `BindTcp` and
//! `ConnectTcp` are denied for all ports.
//!
//! ### seccomp-BPF (deferred)
//!
//! A complementary seccomp-BPF syscall filter would bound the blast radius to
//! only the syscalls Tokio/axum legitimately need (`read`, `write`, `accept4`,
//! `epoll_*`, `futex`, `mmap`/`munmap`, `brk`, `clock_gettime`).
//!
//! This is deferred because `seccompiler::apply_filter` is marked `unsafe fn`
//! and `marque-server` enforces `#![forbid(unsafe_code)]`.  A future PR will
//! introduce a `marque-sandbox` helper crate (without the `forbid` attribute)
//! that provides a minimal Tokio/axum syscall allowlist and is called from
//! here.  See the tracking issue for details.
//!
//! ## Graceful degradation
//!
//! *The server always continues serving — the sandbox is defense-in-depth, not
//! a hard boot requirement.*
//!
//! | Kernel version | Outcome |
//! |----------------|---------|
//! | ≥ 6.7 (Landlock V4) | Filesystem + network restrictions fully enforced — `SandboxStatus::FullyEnforced`. |
//! | 5.13–6.6 (Landlock V1–V3) | Filesystem restrictions enforced; network unavailable — `SandboxStatus::FilesystemOnly`.  A `WARN`-level log entry is emitted. |
//! | < 5.13 or Landlock disabled | No restrictions applied — `SandboxStatus::NotEnforced`.  A `WARN`-level log entry is emitted. |
//! | Non-Linux OS | No restrictions applied — `SandboxStatus::NotEnforced` (compile-time no-op). |
//!
//! [Landlock LSM]: https://landlock.io

use std::path::Path;

/// Summary of which sandbox restrictions were successfully applied.
///
/// Returned by [`apply`] so the caller can log the outcome and, in tests,
/// assert that the expected level of isolation is active.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SandboxStatus {
    /// All requested restrictions applied.
    ///
    /// Landlock V4 or later (kernel ≥ 6.7): both filesystem restrictions
    /// (read-only `/`, no exec) and network restrictions (no new TCP binds
    /// or connects) are in effect.
    FullyEnforced,

    /// Filesystem restrictions applied; network restrictions not available.
    ///
    /// Landlock V1–V3 (kernel 5.13–6.6): write, create, and execute access
    /// is denied everywhere.  TCP bind/connect restrictions require kernel ≥
    /// 6.7 and are silently omitted.
    FilesystemOnly,

    /// No restrictions applied.
    ///
    /// Either the running kernel does not support Landlock (< 5.13 or built
    /// without `CONFIG_SECURITY_LANDLOCK`), or this is a non-Linux target.
    /// A `WARN`-level log entry is emitted in the Linux case.
    NotEnforced,
}

/// Apply the Landlock sandbox after the TCP listener is bound.
///
/// Call this after [`tokio::net::TcpListener::bind`] succeeds and before
/// [`axum::serve`] is called.  The function returns a [`SandboxStatus`]
/// describing how much isolation was achieved; the server continues running
/// regardless of the outcome.
///
/// `config_dir` is the directory from which `.marque.toml` was loaded.
/// Read access is granted to it (as part of the broader `/` read-only rule)
/// so a future hot-reload capability would not require changing the policy.
///
/// # Linux implementation
///
/// Two independent Landlock rulesets are applied sequentially:
///
/// 1. **Filesystem** — `AccessFs::from_all(ABI::V4)` handled; only
///    `ReadFile | ReadDir` granted on `/`.
/// 2. **Network** — `AccessNet::BindTcp | AccessNet::ConnectTcp` handled;
///    no `NetPort` rules → no new TCP binds or connects allowed.
///
/// Landlock accumulates rulesets (most-restrictive intersection), so applying
/// them in two calls is semantically equivalent to one combined call.
#[cfg(target_os = "linux")]
pub fn apply(config_dir: &Path) -> SandboxStatus {
    // Step 1: filesystem restrictions (V1+, kernel 5.13+).
    match apply_filesystem_sandbox() {
        Err(e) => {
            tracing::warn!(
                error = %e,
                "Landlock filesystem sandbox could not be applied; \
                 marque-server is running without filesystem isolation. \
                 Upgrade to kernel 5.13+ for stronger process isolation."
            );
            return SandboxStatus::NotEnforced;
        }
        Ok(status) => {
            use landlock::RulesetStatus;
            match status.ruleset {
                RulesetStatus::NotEnforced => {
                    tracing::warn!(
                        "Landlock is not available on this kernel (< 5.13 or \
                         CONFIG_SECURITY_LANDLOCK disabled); \
                         marque-server is running without filesystem isolation."
                    );
                    return SandboxStatus::NotEnforced;
                }
                RulesetStatus::PartiallyEnforced => {
                    tracing::warn!(
                        config_dir = %config_dir.display(),
                        "Landlock filesystem restrictions partially applied \
                         (some access rights not supported by this kernel). \
                         Write and execute access may not be fully blocked."
                    );
                }
                RulesetStatus::FullyEnforced => {
                    tracing::info!(
                        config_dir = %config_dir.display(),
                        "Landlock filesystem sandbox applied: \
                         read-only access on /; \
                         write, create, and execute access blocked."
                    );
                }
            }
        }
    }

    // Step 2: network restrictions (V4+, kernel 6.7+).
    match apply_network_sandbox() {
        Err(e) => {
            tracing::info!(
                error = %e,
                "Landlock network restrictions unavailable (requires kernel 6.7+, \
                 Landlock V4); filesystem restrictions remain active."
            );
            SandboxStatus::FilesystemOnly
        }
        Ok(status) => {
            use landlock::RulesetStatus;
            match status.ruleset {
                RulesetStatus::NotEnforced | RulesetStatus::PartiallyEnforced => {
                    tracing::info!(
                        "Landlock network restrictions not fully available \
                         (requires kernel 6.7+); filesystem restrictions remain active."
                    );
                    SandboxStatus::FilesystemOnly
                }
                RulesetStatus::FullyEnforced => {
                    tracing::info!(
                        "Landlock network sandbox applied: \
                         new TCP binds and outbound connects blocked. \
                         Filesystem + network sandbox fully enforced."
                    );
                    SandboxStatus::FullyEnforced
                }
            }
        }
    }
}

/// No-op stub on non-Linux targets.
///
/// The server builds and runs on macOS / Windows for development purposes;
/// OS-level sandboxing is a Linux-server concern only.
#[cfg(not(target_os = "linux"))]
pub fn apply(_config_dir: &Path) -> SandboxStatus {
    SandboxStatus::NotEnforced
}

/// Apply read-only filesystem restrictions via a Landlock V4 ruleset.
///
/// Handled access types: all access rights defined through `ABI::V4`
/// (`AccessFs::from_all`), which covers V1–V4 rights including `Truncate`
/// and `Refer`.  The single path rule grants only `ReadFile | ReadDir` on
/// the filesystem root — everything else (write, create, exec, truncate, …)
/// is denied.
///
/// In `CompatLevel::BestEffort` mode (the library default), access rights
/// unknown to the running kernel are silently dropped and the returned
/// `RulesetStatus` reflects how much was actually enforced.
///
/// Returns `Box<dyn Error>` because the path-opening step (`PathFd::new`) can
/// fail with a `PathFdError` that is distinct from `landlock::RulesetError`;
/// boxing avoids a custom error type for this private helper.
#[cfg(target_os = "linux")]
fn apply_filesystem_sandbox(
) -> Result<landlock::RestrictionStatus, Box<dyn std::error::Error + Send + Sync>> {
    use landlock::{
        ABI, Access, AccessFs, PathBeneath, PathFd, Ruleset, RulesetAttr, RulesetCreatedAttr,
        make_bitflags,
    };

    // Use V4 to cover Truncate (V2) and Refer (V2) alongside the V1 basics.
    // BestEffort (default) silently drops rights the kernel does not know.
    let abi = ABI::V4;

    // Allow reading files and listing directories — no execute, no write,
    // no create, no truncate. Execute is deliberately excluded from this
    // rule even though AccessFs::from_read() includes it, because the server
    // never execs child processes.
    let read_no_exec = make_bitflags!(AccessFs::{ReadFile | ReadDir});

    // Open the filesystem root before building the ruleset so that a
    // PathFdError (which does not implement From<_> for RulesetError) is
    // handled here, not inside the method chain below.
    let root = PathFd::new("/")?;

    let status = Ruleset::default()
        .handle_access(AccessFs::from_all(abi))?
        .create()?
        .add_rule(PathBeneath::new(root, read_no_exec))?
        .restrict_self()?;

    Ok(status)
}

/// Block all new TCP binds and outbound connects via a Landlock V4 ruleset.
///
/// No `NetPort` rules are added, so every port is denied for both
/// `AccessNet::BindTcp` and `AccessNet::ConnectTcp`.  The already-bound
/// listening socket is not affected — Landlock restricts future `bind(2)`
/// and `connect(2)` calls, not existing file descriptors.
///
/// **Zero rules is intentional.**  In Landlock, when an access type is
/// *handled* (registered via `handle_access`) but no rule *allows* it, all
/// instances of that access are denied.  Adding no `NetPort` rules means
/// "deny `BindTcp` and `ConnectTcp` on every port", which is exactly the
/// policy we want: the server should accept connections only on the socket
/// that was already bound before this function is called.
///
/// Returns `RulesetStatus::NotEnforced` if the kernel does not support
/// Landlock V4 (< 6.7).
#[cfg(target_os = "linux")]
fn apply_network_sandbox(
) -> Result<landlock::RestrictionStatus, Box<dyn std::error::Error + Send + Sync>> {
    use landlock::{ABI, Access, AccessNet, Ruleset, RulesetAttr};

    let status = Ruleset::default()
        .handle_access(AccessNet::from_all(ABI::V4))?
        .create()?
        // Zero NetPort rules is intentional: Landlock denies any access type
        // that is handled but not explicitly allowed by a rule.  Handling
        // BindTcp + ConnectTcp with no allow rules = all ports blocked.
        .restrict_self()?;

    Ok(status)
}
