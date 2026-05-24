// SPDX-FileCopyrightText: 2026 Knitli Inc.
// SPDX-License-Identifier: LicenseRef-MarqueLicense-1.0

//! Positive control for [`marque_rules::Message::new`].
//!
//! The compile-fail proofs that no free-form constructor exists
//! (`from_string`, `From<&str>`, `format`, etc.) live as
//! `compile_fail` doctests on the [`marque_rules::Message`] type
//! itself in `crates/rules/src/message.rs`. They run via
//! `cargo test --doc -p marque-rules`. This integration file pins
//! the complementary positive case: the `(template, args)`
//! construction works from outside the `marque-rules` crate.

use marque_rules::{Message, MessageArgs, MessageTemplate};

#[test]
fn message_new_accepts_template_and_args_from_external_crate() {
    let m = Message::new(MessageTemplate::DecoderRecognized, MessageArgs::default());
    assert_eq!(m.template(), MessageTemplate::DecoderRecognized);
    assert_eq!(m.args(), &MessageArgs::default());
}

#[test]
fn message_template_as_str_is_stable_from_external_crate() {
    // Pins the on-the-wire string form. Mirrors the in-crate test in
    // `src/message.rs::tests` but exercises the external surface.
    assert_eq!(
        MessageTemplate::DecoderRecognized.as_str(),
        "DecoderRecognized"
    );
    assert_eq!(MessageTemplate::ReparseFailed.as_str(), "ReparseFailed");
}
