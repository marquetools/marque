// SPDX-FileCopyrightText: 2026 Knitli Inc.
// SPDX-License-Identifier: LicenseRef-MarqueLicense-1.0

//! Bag-of-tokens fix vocabulary: `FactRef`, `ReplacementIntent`, `RecanonScope`.
//!
//! These three types name *what* a structural fix is — adding a token,
//! removing a token, or recanonicalizing a scope's rendering — without
//! ever referring to source bytes. They were originally introduced in
//! `marque-rules` (PR 3c.1, Commit 2) but moved to `marque-scheme` in
//! the PR 3c.B engine-prereq because the new
//! [`MarkingScheme::apply_intent`](crate::MarkingScheme::apply_intent)
//! trait method needs to reference them at the trait surface, and
//! `marque-rules` already depends on `marque-scheme` — importing in
//! the other direction would create a cycle.
//!
//! See `specs/006-engine-rule-refactor/architecture.md` "What fixes
//! are" for the binding structural commitment. The three-variant
//! vocabulary — `FactAdd` / `FactRemove` / `Recanonicalize` — is the
//! full surface. The pre-Commit-2 directive-enum design (closed-CVE
//! token, open-vocab `RenderDirective<S>`, byte-`Delete`) is retired
//! per architecture.md "What was lost during PR 3c.1": rules emit
//! fact-set deltas at a `Scope`, not `(span, replacement_bytes)`
//! pairs.

use core::fmt::Debug;
use core::hash::Hash;

use smallvec::SmallVec;

use crate::category::TokenId;
use crate::scheme::MarkingScheme;
use crate::scope::Scope;

/// A reference to a token in the projected fact set.
///
/// `FactRef` identifies the *fact-set position* the rule is naming,
/// not bytes in the input. This is what makes `FactRemove`
/// source-buffer-agnostic: the engine names what to remove by its
/// place in the projected lattice, never by an input span.
///
/// Closed-CVE tokens use [`FactRef::Cve`]; open-vocabulary tokens
/// (SAR program identifiers, SCI compartment / sub-compartment
/// paths, FGI tetragraphs in CAPCO) use [`FactRef::OpenVocab`] with
/// the scheme's [`MarkingScheme::OpenVocabRef`] carrier. Schemes
/// without open-vocab axes bind `OpenVocabRef = Infallible`, making
/// the `OpenVocab` variant statically unreachable.
///
/// # Constitution V Principle V (G13 closure)
///
/// `FactRef` carries no document content. `OpenVocab` payloads come
/// from the scheme's *canonicalize* output — typed structural
/// references whose payload is the canonicalized value, not raw
/// input bytes (a SAR program ID *value*, not a slice of source).
/// This is what preserves audit-content-ignorance: an `AppliedFix`
/// referring to a token via `FactRef` never stores subject-side
/// bytes.
///
/// `Debug` / `Clone` / `PartialEq` / `Eq` / `Hash` are written
/// manually rather than derived so the trait bounds resolve through
/// `S::OpenVocabRef` (which carries the full bound set per
/// `MarkingScheme`) instead of over-constraining to `S: Debug +
/// Clone + ...`.
pub enum FactRef<S: MarkingScheme> {
    /// Closed-vocabulary token; resolves to a unique entry in the
    /// scheme's CVE-registered vocabulary.
    Cve(TokenId),
    /// Open-vocabulary structural reference. The scheme's
    /// canonicalize step produces these from input; the engine
    /// re-renders from them. See [`MarkingScheme::OpenVocabRef`].
    OpenVocab(S::OpenVocabRef),
}

impl<S: MarkingScheme> Debug for FactRef<S> {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            FactRef::Cve(id) => f.debug_tuple("Cve").field(id).finish(),
            FactRef::OpenVocab(r) => f.debug_tuple("OpenVocab").field(r).finish(),
        }
    }
}

impl<S: MarkingScheme> Clone for FactRef<S> {
    fn clone(&self) -> Self {
        match self {
            FactRef::Cve(id) => FactRef::Cve(*id),
            FactRef::OpenVocab(r) => FactRef::OpenVocab(r.clone()),
        }
    }
}

impl<S: MarkingScheme> PartialEq for FactRef<S> {
    fn eq(&self, other: &Self) -> bool {
        match (self, other) {
            (FactRef::Cve(a), FactRef::Cve(b)) => a == b,
            (FactRef::OpenVocab(a), FactRef::OpenVocab(b)) => a == b,
            _ => false,
        }
    }
}

impl<S: MarkingScheme> Eq for FactRef<S> {}

impl<S: MarkingScheme> Hash for FactRef<S> {
    fn hash<H: core::hash::Hasher>(&self, state: &mut H) {
        match self {
            FactRef::Cve(id) => {
                0u8.hash(state);
                id.hash(state);
            }
            FactRef::OpenVocab(r) => {
                1u8.hash(state);
                r.hash(state);
            }
        }
    }
}

/// Three structural fix variants — see
/// `specs/006-engine-rule-refactor/architecture.md` "What fixes
/// are."
///
/// - [`ReplacementIntent::FactAdd`] — add a token to the projected
///   fact set at a given scope. Repairs `Constraint::Requires`
///   violations.
/// - [`ReplacementIntent::FactRemove`] — remove a token from the
///   projected fact set at a given scope. Repairs
///   `Constraint::Conflicts` violations.
/// - [`ReplacementIntent::Recanonicalize`] — input form diverges
///   from canonical form on a scope. The fact set is correct; only
///   the rendering isn't. Subsumes delimiter normalization, sort
///   canonicalization, abbreviation canonicalization, block
///   reordering, and banner roll-up form.
///
/// No `Box<str>` payloads. No multi-span carriers. No `Delete` /
/// `Render` byte-surgery variants. The directive-enum design was the
/// wrong abstraction layer (per architecture.md "What was lost
/// during PR 3c.1").
///
/// `Debug` and `Clone` are written manually rather than derived for
/// the same reason as on [`FactRef`] — to avoid over-constraining
/// the trait bound to `S: Debug + Clone`.
pub enum ReplacementIntent<S: MarkingScheme> {
    /// Add a token to the projected fact set at `scope`.
    FactAdd {
        /// The token to add. Closed-vocab via [`FactRef::Cve`] or
        /// open-vocab via [`FactRef::OpenVocab`].
        token: FactRef<S>,
        /// Which projection level the addition applies to
        /// (`Portion` / `Page` / `Document`).
        scope: Scope,
    },

    /// Remove one or more tokens from the projected fact set at `scope`.
    ///
    /// The common case carries exactly one fact (inline capacity `[_; 2]`
    /// keeps the single-token path heap-free). Multi-fact removal is used
    /// for atomic chain removals — e.g. E024's "RD supersedes both FRD and
    /// TFNI" is one policy decision that should land as one audit repair
    /// per Constitution V Principle V.
    ///
    /// Use [`ReplacementIntent::fact_remove`] for the single-fact case to
    /// avoid constructing a `SmallVec` at every call site. Multi-fact
    /// callers construct `FactRemove { facts: smallvec![f1, f2], scope }`
    /// directly.
    FactRemove {
        /// The token(s) to remove. Inline capacity 2 — single-fact
        /// (common case) and the FRD+TFNI pair (E024) both fit without
        /// heap allocation; longer chains spill to heap cleanly.
        facts: SmallVec<[FactRef<S>; 2]>,
        /// Which projection level the removal applies to.
        scope: Scope,
    },

    /// Recanonicalize the rendering of `scope`. Input form
    /// diverges from canonical form; the fact set is correct.
    /// The renderer re-renders the scope per `render_canonical`.
    ///
    /// # Engine dispatch contract
    ///
    /// At fix-application time, `Engine::fix_inner` consults its
    /// in-scope projection (already computed during `lint` per
    /// Constitution VI's dataflow pipeline) for the named
    /// [`RecanonScope`], then calls
    /// `render_canonical(&projection.marking, scope.into(),
    /// &mut writer)` on the active scheme. Rules NEVER carry the
    /// `ProjectedMarking` — the engine is the authority on
    /// per-scope projections; the rule only names which scope to
    /// re-render. See `MarkingScheme::render_canonical` doc
    /// comment for the full writer-passing + lattice-equal-byte-
    /// identical contract.
    Recanonicalize {
        /// The positional scope to re-render. `RecanonScope`
        /// excludes `Scope::Diff` because a diff context is not a
        /// recanonicalization target — `Diff` is a rule-context
        /// query mode (architecture.md type-sketch).
        scope: RecanonScope,
    },
}

impl<S: MarkingScheme> Debug for ReplacementIntent<S> {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            ReplacementIntent::FactAdd { token, scope } => f
                .debug_struct("FactAdd")
                .field("token", token)
                .field("scope", scope)
                .finish(),
            ReplacementIntent::FactRemove { facts, scope } => f
                .debug_struct("FactRemove")
                .field("facts", facts)
                .field("scope", scope)
                .finish(),
            ReplacementIntent::Recanonicalize { scope } => f
                .debug_struct("Recanonicalize")
                .field("scope", scope)
                .finish(),
        }
    }
}

impl<S: MarkingScheme> Clone for ReplacementIntent<S> {
    fn clone(&self) -> Self {
        match self {
            ReplacementIntent::FactAdd { token, scope } => ReplacementIntent::FactAdd {
                token: token.clone(),
                scope: *scope,
            },
            ReplacementIntent::FactRemove { facts, scope } => ReplacementIntent::FactRemove {
                facts: facts.clone(),
                scope: *scope,
            },
            ReplacementIntent::Recanonicalize { scope } => {
                ReplacementIntent::Recanonicalize { scope: *scope }
            }
        }
    }
}

impl<S: MarkingScheme> ReplacementIntent<S> {
    /// Construct a single-fact [`FactRemove`](ReplacementIntent::FactRemove) intent.
    ///
    /// Ergonomic shorthand for the common case where one policy
    /// decision removes exactly one token. The resulting `SmallVec`
    /// has length 1 and never allocates on the heap.
    ///
    /// Multi-fact callers (e.g. E024's RD/FRD/TFNI atomic cluster)
    /// should construct `FactRemove { facts: smallvec![f1, f2], scope }`
    /// directly.
    #[inline]
    pub fn fact_remove(fact: FactRef<S>, scope: Scope) -> Self {
        ReplacementIntent::FactRemove {
            facts: smallvec::smallvec![fact],
            scope,
        }
    }
}

/// The recanonicalization scope.
///
/// A narrowing of [`Scope`] that excludes `Scope::Diff`.
/// `Scope::Diff` is a rule-context query mode, not a
/// projection-output scope, so it is not a meaningful
/// recanonicalization target — see
/// `specs/006-engine-rule-refactor/architecture.md` type-sketch.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum RecanonScope {
    /// Recanonicalize a single portion's rendering.
    Portion,
    /// Recanonicalize a page's rendering (banner / CAB roll-up).
    Page,
    /// Recanonicalize a document's rendering. Usually agrees with
    /// `Page` on single-page documents.
    Document,
}

impl From<RecanonScope> for Scope {
    /// Widen a [`RecanonScope`] back to a full [`Scope`]. Inverse
    /// is the [`TryFrom<Scope>`] impl, which rejects `Scope::Diff`.
    fn from(s: RecanonScope) -> Self {
        match s {
            RecanonScope::Portion => Scope::Portion,
            RecanonScope::Page => Scope::Page,
            RecanonScope::Document => Scope::Document,
        }
    }
}

impl TryFrom<Scope> for RecanonScope {
    /// `Scope::Diff` is the rejection case — it is a rule-context
    /// query mode, not a projection-output scope, and is not a
    /// meaningful recanonicalization target (architecture.md
    /// type-sketch invariant). The other three variants round-trip
    /// with [`From<RecanonScope> for Scope`].
    type Error = ();

    fn try_from(s: Scope) -> Result<Self, Self::Error> {
        match s {
            Scope::Portion => Ok(RecanonScope::Portion),
            Scope::Page => Ok(RecanonScope::Page),
            Scope::Document => Ok(RecanonScope::Document),
            Scope::Diff => Err(()),
        }
    }
}

// Compile-time guarantee that `Debug + Clone + PartialEq + Eq + Hash`
// flow through the small positional types.
const _: fn() = || {
    fn assert_traits<T: Debug + Clone + PartialEq + Eq + Hash>() {}
    let _ = assert_traits::<RecanonScope>;
};

#[cfg(test)]
#[cfg_attr(coverage_nightly, coverage(off))]
mod tests {
    use super::*;

    #[test]
    fn recanon_scope_widens_to_scope() {
        assert_eq!(Scope::from(RecanonScope::Portion), Scope::Portion);
        assert_eq!(Scope::from(RecanonScope::Page), Scope::Page);
        assert_eq!(Scope::from(RecanonScope::Document), Scope::Document);
    }

    #[test]
    fn recanon_scope_rejects_diff() {
        assert!(RecanonScope::try_from(Scope::Diff).is_err());
        assert_eq!(
            RecanonScope::try_from(Scope::Portion),
            Ok(RecanonScope::Portion)
        );
    }
}
