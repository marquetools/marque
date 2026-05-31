// SPDX-FileCopyrightText: 2026 Knitli Inc.
//
// SPDX-License-Identifier: LicenseRef-MarqueLicense-1.0

//! Two orthogonal provenance axes for document-scoped artifact nodes.
//!
//! A document artifact (CAB, declassify instruction, notice, ...) carries
//! *two* independent provenance questions that must not be collapsed into a
//! single tag:
//!
//! - [`RecognitionProvenance`] — the **adapter axis**: "how sure am I that
//!   this span *is* this node?" It answers a recognition question and
//!   licenses fix-assertiveness (a node read from a structured field can be
//!   rewritten more confidently than one inferred from prose).
//! - [`ValueDerivation`] — the **DAG-node axis**: "how was this node's
//!   *value* computed?" It answers a derivation question (authored vs.
//!   rolled-up vs. methodology-driven vs. canned-string).
//!
//! The two axes are independent: a node can be
//! [`ValueDerivation::DerivedMaxOverSources`] *and*
//! [`RecognitionProvenance::DocumentContent`] at the same time — the value
//! was computed by a max over source markings, yet recognized from prose
//! rather than a structured field. Keeping them separate is what lets the
//! engine reason about value lineage and recognition confidence without one
//! distorting the other.
//!
//! Both enums are domain-neutral (no CAPCO vocabulary) and carry no
//! document content — they are pure enum tags, satisfying Constitution V
//! Principle V (audit content-ignorance).

/// Recognition provenance — the **adapter axis**.
///
/// Answers "how was this node's presence *recognized* in the source?" and
/// licenses how assertively a fix may rewrite it. This is the
/// value-derivation-orthogonal half of issue #176's `InputSource` concept;
/// the full `InputSource` promotion (threading the axis through the
/// recognizer and into the audit record) is a later phase. Here it is only
/// the value-type.
///
/// `#[non_exhaustive]` reserves grow-path: future recognizers (e.g. a
/// metadata-sidecar reader) may add a variant additively.
#[non_exhaustive]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum RecognitionProvenance {
    /// Recognized by reading a document's structural layout — a banner
    /// line, a CAB at a fixed position, a footer. High recognition
    /// confidence: the position itself testifies to the node's identity.
    StructureRead,
    /// Recognized from a structured field (extracted-document metadata, an
    /// Office document property, a form field). Highest recognition
    /// confidence: the field is explicitly typed by the source format.
    StructuredField,
    /// Recognized from free-flowing document content (prose). Lowest
    /// recognition confidence: the node was inferred from text rather than
    /// read from a designated structure, so fixes touching it stay
    /// conservative.
    DocumentContent,
}

/// Value derivation — the **DAG-node axis**.
///
/// Answers "how was this node's *value* computed?" and is orthogonal to
/// [`RecognitionProvenance`]: the recognition axis says how sure we are the
/// span is this node; this axis says where the node's value came from.
///
/// `#[non_exhaustive]` reserves grow-path for future derivation modes.
#[non_exhaustive]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ValueDerivation {
    /// Authored by an original-classification authority — e.g. the original
    /// CAB on a source document. The value is asserted, not derived.
    Authored,
    /// Derived as the maximum (most-restrictive join) over multiple source
    /// markings. Reserved for the bundle → document source-derivation line
    /// (issue #823); not wired at this phase.
    DerivedMaxOverSources,
    /// Driven by a derivation methodology — e.g. a HUMINT source mapping to
    /// a `50X1-HUM` declassification instruction.
    MethodologyDriven,
    /// A mandated canned policy string — e.g. the §E.4 / §E.5 standard
    /// notices whose text is fixed by policy rather than computed from the
    /// document's markings.
    CannedPolicyString,
    /// Rolled up from the document's portion markings — the banner /
    /// page-level aggregate value computed by lattice join over portions.
    RolledUp,
}

#[cfg(test)]
#[cfg_attr(coverage_nightly, coverage(off))]
mod tests {
    use super::*;

    #[test]
    fn recognition_provenance_variants_are_distinct() {
        // The three recognition tiers must be distinguishable — they
        // license different fix-assertiveness levels.
        assert_ne!(
            RecognitionProvenance::StructureRead,
            RecognitionProvenance::StructuredField
        );
        assert_ne!(
            RecognitionProvenance::StructuredField,
            RecognitionProvenance::DocumentContent
        );
        assert_ne!(
            RecognitionProvenance::StructureRead,
            RecognitionProvenance::DocumentContent
        );
    }

    #[test]
    fn value_derivation_variants_are_distinct() {
        let all = [
            ValueDerivation::Authored,
            ValueDerivation::DerivedMaxOverSources,
            ValueDerivation::MethodologyDriven,
            ValueDerivation::CannedPolicyString,
            ValueDerivation::RolledUp,
        ];
        // Every pair is distinct (no accidental aliasing of variants).
        for (i, a) in all.iter().enumerate() {
            for (j, b) in all.iter().enumerate() {
                if i == j {
                    assert_eq!(a, b);
                } else {
                    assert_ne!(a, b);
                }
            }
        }
    }

    #[test]
    fn axes_are_independent() {
        // The defining property: a node can carry any combination of the
        // two axes. Construct a "DerivedMaxOverSources × DocumentContent"
        // pairing to document that the two axes do not constrain each other.
        let derivation = ValueDerivation::DerivedMaxOverSources;
        let recognition = RecognitionProvenance::DocumentContent;
        assert_eq!(derivation, ValueDerivation::DerivedMaxOverSources);
        assert_eq!(recognition, RecognitionProvenance::DocumentContent);

        // And the inverse pairing is equally representable.
        let derivation2 = ValueDerivation::RolledUp;
        let recognition2 = RecognitionProvenance::StructuredField;
        assert_eq!(derivation2, ValueDerivation::RolledUp);
        assert_eq!(recognition2, RecognitionProvenance::StructuredField);
    }

    #[test]
    fn provenance_axes_are_copy() {
        // Both axes flow by value through artifact construction; Copy keeps
        // call sites allocation-free.
        fn assert_copy<T: Copy>() {}
        assert_copy::<RecognitionProvenance>();
        assert_copy::<ValueDerivation>();
    }
}
