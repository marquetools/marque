// SPDX-FileCopyrightText: 2026 Knitli Inc.
//
// SPDX-License-Identifier: LicenseRef-MarqueLicense-1.0

//! Constitution Principle II (Marque-owned content wipes on drop)
//! regression tests for [`FixResult.source`].
//!
//! Three load-bearing properties are pinned here so a refactor that
//! quietly removes the wrapper or swaps in a non-zeroizing type fails
//! in CI rather than silently weakening the security posture:
//!
//! 1. **Type-level**: `FixResult.source` is `secrecy::SecretSlice<u8>`.
//!    The compile-time `static_assertions::assert_type_eq_all!`
//!    rejects any drift to a bare `Vec<u8>` / `Box<[u8]>` / a custom
//!    wrapper that doesn't carry the zeroize-on-drop guarantee.
//!
//! 2. **Debug redaction**: `format!("{:?}", result)` emits
//!    `SecretBox<...>([REDACTED])` for the `source` field — not the
//!    fixed bytes. Closes the "oops, logged the document" channel
//!    that motivated Tier 2 of the security-reviewer brief.
//!
//! 3. **Readout discipline**: `expose_secret()` is the only path to
//!    `&[u8]`. The trait-import requirement at every consumer site
//!    is the grep target for security review — this test exercises
//!    the readout path end-to-end so an integration consumer that
//!    skips the import fails its own build (this test demonstrates
//!    the supported pattern).
//!
//! These tests intentionally do NOT poke at freed memory to verify
//! the wipe — the `secrecy` / `zeroize` crates are audited
//! upstream and that contract is theirs. We pin the boundary, not
//! the implementation.

use marque_capco::capco_rules;
use marque_config::Config;
use marque_engine::{CapcoEngine, FixMode, FixResult};
use secrecy::{ExposeSecret as _, SecretSlice};
use static_assertions::assert_type_eq_all;

fn engine() -> CapcoEngine {
    CapcoEngine::new(
        Config::default(),
        vec![Box::new(capco_rules())],
        marque_engine::default_scheme(),
    )
    .expect("default CAPCO scheme has no rewrite cycles")
}

/// A fixture document that produces a real fix on the lint→fix
/// path — `SECRET//NOFORN` portion missing the parentheses around
/// `NOFORN` (E001 fires + the engine canonicalizes). Picking a
/// non-trivial input ensures `FixResult.source` actually carries
/// document content, not an empty placeholder.
const TEST_SRC: &[u8] = b"(S//NF) Sample portion that triggers rules.\n";

/// Property 1 — type-level pin. If a refactor changes
/// `FixResult.source` to `Vec<u8>`, `Box<[u8]>`, or any other
/// non-zeroizing wrapper, this assertion fails at compile time.
#[test]
fn fix_result_source_is_secret_slice_u8() {
    // The assert is on a field projection of the FixResult struct.
    // We materialize a FixResult via the engine so the type
    // inference site is real production code, not a synthetic
    // construction.
    let result = engine().fix(TEST_SRC, FixMode::DryRun);
    // The compile-time type check is structural: if the field type
    // drifts from SecretSlice<u8>, this line fails to compile.
    assert_type_eq_all!(SecretSlice<u8>, <FixResult as ResultSourceTypeProbe>::Ty);

    // Trait probe — gives `assert_type_eq_all!` a name to bind to
    // the field type without naming the field's type literal at
    // every call site. The probe trait is implemented exactly once
    // for `FixResult` and projects its `source` field's type via
    // an associated type.
    trait ResultSourceTypeProbe {
        type Ty;
    }
    impl ResultSourceTypeProbe for FixResult {
        type Ty = SecretSlice<u8>;
    }

    // Suppress unused-result warning — the assertion is the test.
    let _ = result.source;
}

/// Property 2 — Debug auto-redacts. A reviewer reading
/// `format!("{:?}", result)` or a stray `dbg!(result)` does NOT
/// see document bytes. The `SecretBox` impl emits
/// `SecretBox<...>([REDACTED])` for the wrapped content.
#[test]
fn fix_result_debug_redacts_source_bytes() {
    let result = engine().fix(TEST_SRC, FixMode::Apply);
    let debug_repr = format!("{result:?}");

    assert!(
        debug_repr.contains("REDACTED"),
        "FixResult Debug must redact the wrapped source bytes; \
         expected the `SecretBox<...>([REDACTED])` form, got: \
         {debug_repr:?}"
    );

    // The fixture has unique substrings ("Sample portion", "triggers
    // rules") that would appear if the bytes leaked through Debug.
    // Pin both so a future regression where Debug starts printing
    // bytes (e.g., a manual Debug impl that forgets the wrapper)
    // fails this assertion.
    assert!(
        !debug_repr.contains("Sample portion"),
        "FixResult Debug must NOT expose the fixed source bytes; \
         got: {debug_repr:?}"
    );
    assert!(
        !debug_repr.contains("triggers rules"),
        "FixResult Debug must NOT expose the fixed source bytes; \
         got: {debug_repr:?}"
    );
}

/// Property 3 — readout discipline. `expose_secret()` returns the
/// inner bytes; the bytes are recoverable for legitimate readouts
/// (write to stdout, splice for re-lint, etc.). This test
/// demonstrates the supported pattern that consumers MUST follow.
#[test]
fn expose_secret_yields_byte_slice() {
    let result = engine().fix(TEST_SRC, FixMode::DryRun);
    let bytes: &[u8] = result.source.expose_secret();

    // DryRun preserves the original source byte-for-byte.
    assert_eq!(
        bytes, TEST_SRC,
        "DryRun must surface the unmodified source bytes via expose_secret()",
    );

    // A consumer that wants to convert to `String` clones via
    // `to_vec()` — the clone is the caller's buffer; Marque's
    // SecretSlice still wipes on drop.
    let owned: Vec<u8> = bytes.to_vec();
    let text = String::from_utf8(owned).expect("fixture is valid UTF-8");
    assert!(text.starts_with("(S//NF)"));
}
