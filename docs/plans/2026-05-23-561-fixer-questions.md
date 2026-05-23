<!--
SPDX-FileCopyrightText: 2026 Knitli Inc.

SPDX-License-Identifier: LicenseRef-MarqueLicense-1.0
-->

# Fixer-pass questions for PR #737 (issue #561)

## Q1: `is_fgi_invalid_ownership_token` move — code-review M3 vs.
##     refactoring-review §5 disagree about the consumer set

**Status**: deferred from FIX-5; PR proceeds without moving the symbol.

**Brief instruction (FIX-5)**: "is_fgi_invalid_ownership_token lives in
helpers.rs:229 — only FgiInvalidOwnershipTokenRule in fgi.rs consumes
it. Move to fgi.rs."

**What I verified by grep at fix-pass authorship**:

| Site | Type |
|------|------|
| `crates/capco/src/rules/text_handling.rs:30` | `use` import |
| `crates/capco/src/rules/text_handling.rs:566` | production call: `!is_fgi_invalid_ownership_token(text)` in `UnknownTokenRule::check` (E008's suppression chain) |
| `crates/capco/src/rules/fgi.rs:469` | doc-comment reference inside `FgiInvalidOwnershipTokenRule::check` body — narrates an in-rule branch; no actual call |
| `crates/capco/src/rules/helpers.rs:229` | the definition + `pub(crate)` |
| `crates/capco/src/rules/helpers.rs:14` | module-header doc-link |

**The production consumer is `UnknownTokenRule` in `text_handling.rs`,
not `FgiInvalidOwnershipTokenRule` in `fgi.rs`.** The brief's premise
("only FgiInvalidOwnershipTokenRule consumes it") is incorrect.

This matches the refactoring-review §5's view ("Cross-rule predicate")
and contradicts the code-review's M3 ("single-consumer in `fgi.rs`").

**Decision deferred**: moving to `fgi.rs` would make `text_handling.rs`
import a helper from a peer-domain rule file (`crate::rules::fgi::*`),
which is a worse cohesion outcome than keeping it in `helpers.rs`.
Three honest options:

1. Keep in `helpers.rs`. Helper is "FGI ownership-token shape predicate
   used by the E008 (UnknownTokenRule) suppression chain", which is
   structurally cross-rule.
2. Move to `fgi.rs` and have `text_handling.rs` import `super::fgi::*`.
   Mechanically possible but cohesion-wise wrong: text_handling no
   longer imports only from helpers.
3. Move to `text_handling.rs` (the actual consumer). The doc-comment
   at `fgi.rs:469` referring to the symbol would need updating, and
   the symbol's name (`is_fgi_invalid_ownership_token`) reads oddly
   for a text-handling rule file.

PR proceeds with option (1) — status quo. Code-review's M3 was filed
as deferred to a follow-up issue anyway, so leaving the cohesion
question open is consistent with that disposition.

**Follow-up filed**: see ISSUE-A in the fixer-pass reply (existing
issue tracker).
