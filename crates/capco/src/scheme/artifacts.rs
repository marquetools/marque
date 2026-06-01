// SPDX-FileCopyrightText: 2026 Knitli Inc.
//
// SPDX-License-Identifier: LicenseRef-MarqueLicense-1.0

//! `impl SchemeArtifacts for CapcoScheme` — the document-scoped artifact
//! opt-in for the CAPCO scheme.
//!
//! Feature 007 Phase 0a lands the additive `SchemeArtifacts` extension
//! trait surface in `marque-scheme`. CapcoScheme opts in here so the
//! `DocumentArtifact<CapcoScheme>` type is constructible, but binds the
//! `ArtifactPayload` to `()` as a placeholder: the real `Cab` payload
//! (and the declassify / notice payloads) land in Phase D when the CAB
//! fields move off the `CanonicalAttrs` pivot type.
//!
//! The defaulted `MarkingScheme::document_artifacts()` /
//! `derivation_edges()` methods stay at their empty-slice defaults for
//! now — no behavior change to the existing CAPCO pipeline.

use marque_scheme::SchemeArtifacts;

use super::adapter::CapcoScheme;

impl SchemeArtifacts for CapcoScheme {
    /// Placeholder payload. Phase D replaces this with the parsed `Cab`
    /// artifact type once the CAB fields relocate off `CanonicalAttrs`.
    /// Bound to `()` here so `DocumentArtifact<CapcoScheme>` is a valid
    /// type and the opt-in is wired, without yet committing to the
    /// payload shape.
    ///
    /// # Constitution V Principle V (audit content-ignorance)
    ///
    /// `()` carries no document content. The future `Cab` payload MUST
    /// likewise stay content-ignorant for audit-adjacent uses (parsed
    /// structural form, never raw document bytes).
    type ArtifactPayload = ();
}

#[cfg(test)]
#[cfg_attr(coverage_nightly, coverage(off))]
mod tests {
    use super::*;
    use marque_scheme::{
        ArtifactKind, ArtifactState, DocumentArtifact, MarkingScheme, Scope, ValueDerivation,
    };

    #[test]
    fn capco_scheme_implements_scheme_artifacts() {
        // The bound is what matters: this function only compiles if
        // CapcoScheme: SchemeArtifacts holds.
        fn assert_scheme_artifacts<S: SchemeArtifacts>() {}
        assert_scheme_artifacts::<CapcoScheme>();
    }

    #[test]
    fn document_artifact_over_capco_constructs() {
        // A DocumentArtifact parameterized by CapcoScheme is a valid type;
        // the placeholder `()` payload constructs the Present state.
        let node: DocumentArtifact<CapcoScheme> = DocumentArtifact {
            kind: ArtifactKind::AuthorityBlock,
            scope: Scope::Document,
            state: ArtifactState::Present(()),
            derivation: ValueDerivation::Authored,
            inbound: Box::new([]),
            span: None,
        };
        assert_eq!(node.kind, ArtifactKind::AuthorityBlock);
        assert!(matches!(node.state, ArtifactState::Present(())));
    }

    #[test]
    fn document_artifacts_and_edges_default_empty() {
        // No behavior change: the defaulted inventory methods stay empty
        // for CapcoScheme at this phase.
        let scheme = CapcoScheme::new();
        assert!(scheme.document_artifacts().is_empty());
        assert!(scheme.derivation_edges().is_empty());
    }
}
