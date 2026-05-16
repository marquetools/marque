// SPDX-FileCopyrightText: 2026 Knitli Inc.
//
// SPDX-License-Identifier: LicenseRef-MarqueLicense-1.0

//! Smoke test: verify the Landlock sandbox applies without error on Linux.
//!
//! Asserted by CI on Ubuntu (kernel ≥ 5.13 → Landlock V1+; ≥ 6.7 → V4).
//! On non-Linux targets the test is compiled but always passes as
//! `SandboxStatus::NotEnforced` (the compile-time no-op stub).

use marque_server::sandbox::{SandboxStatus, apply};

/// Verify that `apply()` does not panic or return `NotEnforced` on a Linux
/// runner.
///
/// On Linux with Landlock V1+ (kernel ≥ 5.13) the sandbox must be at least
/// `FilesystemOnly`.  On kernels ≥ 6.7 (Landlock V4, the Ubuntu-24.04 CI
/// baseline) the expected status is `FullyEnforced`.
///
/// The test uses a temporary directory as `config_dir`.  The value of
/// `config_dir` does not affect which filesystem paths are accessible (the
/// sandbox grants read-only access to all of `/`); it is used only for the
/// startup `INFO` log entry.
#[test]
fn sandbox_applies_without_error() {
    let tmp = std::env::temp_dir();
    let status = apply(&tmp);

    #[cfg(target_os = "linux")]
    {
        assert_ne!(
            status,
            SandboxStatus::NotEnforced,
            "Landlock sandbox returned NotEnforced on Linux. \
             This likely means the CI kernel does not support Landlock \
             (requires kernel ≥ 5.13 with CONFIG_SECURITY_LANDLOCK=y). \
             Expected FilesystemOnly or FullyEnforced."
        );
    }

    #[cfg(not(target_os = "linux"))]
    {
        // Non-Linux targets always return NotEnforced (compile-time stub).
        assert_eq!(status, SandboxStatus::NotEnforced);
    }
}

/// Verify that calling `apply()` twice does not panic.
///
/// Landlock restrictions accumulate (most-restrictive intersection), so a
/// second call adds restrictions on top of the first.  This test confirms
/// the function is safe to call in contexts where it might be invoked more
/// than once (e.g., test harness, re-entrant startup logic).
#[test]
fn sandbox_apply_twice_does_not_panic() {
    let tmp = std::env::temp_dir();
    let _s1 = apply(&tmp);
    // Second call accumulates additional restrictions — both should succeed.
    let _s2 = apply(&tmp);
}
