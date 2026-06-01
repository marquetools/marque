// SPDX-FileCopyrightText: 2026 Knitli Inc.
// SPDX-License-Identifier: LicenseRef-MarqueLicense-1.0

//! Bag-of-tokens fix vocabulary: `FactRef`, `ReplacementIntent`, `RecanonScope`.
//!
//! These three types name *what* a structural fix is — adding a token,
//! removing a token, or recanonicalizing a scope's rendering — without
//! ever referring to source bytes. They live in `marque-scheme` because
//! the [`MarkingScheme::apply_intent`](crate::MarkingScheme::apply_intent)
//! trait method needs to reference them at the trait surface, and
//! `marque-rules` already depends on `marque-scheme` — importing in
//! the other direction would create a cycle.
//!
//! The three-variant vocabulary — `FactAdd` / `FactRemove` /
//! `Recanonicalize` — is the full surface: rules emit fact-set deltas at
//! a `Scope`, not `(span, replacement_bytes)` pairs.

use core::fmt::Debug;
use core::hash::Hash;

use smallvec::SmallVec;

use crate::category::TokenId;
use crate::scheme::MarkingScheme;
use crate::scope::Scope;
use crate::span::Span;

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
/// # Constitution V Principle V (audit content-ignorance)
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
pub enum FactRef<S: MarkingScheme + ?Sized> {
    /// Closed-vocabulary token; resolves to a unique entry in the
    /// scheme's CVE-registered vocabulary.
    Cve(TokenId),
    /// Open-vocabulary structural reference. The scheme's
    /// canonicalize step produces these from input; the engine
    /// re-renders from them. See [`MarkingScheme::OpenVocabRef`].
    OpenVocab(S::OpenVocabRef),
}

impl<S: MarkingScheme + ?Sized> Debug for FactRef<S> {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            FactRef::Cve(id) => f.debug_tuple("Cve").field(id).finish(),
            FactRef::OpenVocab(r) => f.debug_tuple("OpenVocab").field(r).finish(),
        }
    }
}

impl<S: MarkingScheme + ?Sized> Clone for FactRef<S> {
    fn clone(&self) -> Self {
        match self {
            FactRef::Cve(id) => FactRef::Cve(*id),
            FactRef::OpenVocab(r) => FactRef::OpenVocab(r.clone()),
        }
    }
}

impl<S: MarkingScheme + ?Sized> PartialEq for FactRef<S> {
    fn eq(&self, other: &Self) -> bool {
        match (self, other) {
            (FactRef::Cve(a), FactRef::Cve(b)) => a == b,
            (FactRef::OpenVocab(a), FactRef::OpenVocab(b)) => a == b,
            _ => false,
        }
    }
}

impl<S: MarkingScheme + ?Sized> Eq for FactRef<S> {}

impl<S: MarkingScheme + ?Sized> Hash for FactRef<S> {
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

/// Reversibility pre-state for a [`ReplacementIntent::Recanonicalize`].
///
/// Records the pre-fix structural facts needed to reverse a
/// recanonicalization: the tokens that were present, the byte span the
/// rendering occupied, and a BLAKE3 digest of the pre-fix bytes. This is
/// the #824 rough-in — Phase 0 only *reserves* this state; the reversal
/// pass and the additive `marque-3.x` audit-schema bump that records it
/// are deferred (#824). Token-level fixes are self-reversible from the
/// audit log; free-form text corrections are reversible only against the
/// caller's retained original (Constitution II).
///
/// # Constitution V Principle V (audit content-ignorance)
///
/// Every field is an audit-permitted term: [`FactRef`] token canonicals,
/// a [`Span`]'s byte offsets, and a BLAKE3 `[u8; 32]` digest. There are
/// no document bytes or free-form strings — the G13 content-ignorance
/// surface is unchanged.
///
/// `Debug` / `Clone` / `PartialEq` / `Eq` are written manually (over
/// `S::OpenVocabRef` bounds) for the same reason as on [`FactRef`] — a
/// blanket derive over `S` over-constrains to `S: Debug + Clone + ...`.
pub struct RecanonPriorState<S: MarkingScheme + ?Sized> {
    /// The tokens present in the scope before recanonicalization.
    pub prior_tokens: Box<[FactRef<S>]>,
    /// The byte span the pre-fix rendering occupied.
    pub prior_span: Span,
    /// BLAKE3 digest of the pre-fix bytes (content-ignorant digest, not
    /// the bytes themselves).
    pub digest: [u8; 32],
}

impl<S: MarkingScheme + ?Sized> Debug for RecanonPriorState<S> {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("RecanonPriorState")
            .field("prior_tokens", &self.prior_tokens)
            .field("prior_span", &self.prior_span)
            .field("digest", &self.digest)
            .finish()
    }
}

impl<S: MarkingScheme + ?Sized> Clone for RecanonPriorState<S> {
    fn clone(&self) -> Self {
        Self {
            prior_tokens: self.prior_tokens.clone(),
            prior_span: self.prior_span,
            digest: self.digest,
        }
    }
}

impl<S: MarkingScheme + ?Sized> PartialEq for RecanonPriorState<S> {
    fn eq(&self, other: &Self) -> bool {
        self.prior_tokens == other.prior_tokens
            && self.prior_span == other.prior_span
            && self.digest == other.digest
    }
}

impl<S: MarkingScheme + ?Sized> Eq for RecanonPriorState<S> {}

/// Reversibility pre-state for a [`ReplacementIntent::Relocate`].
///
/// Records the pre-fix structural facts needed to reverse a relocation
/// (D8, relocate-not-evict): the token that moved, the byte span it
/// occupied at its origin, and a BLAKE3 digest of the pre-fix bytes.
/// Like [`RecanonPriorState`], this is the #824 rough-in — Phase 0 only
/// *reserves* this state.
///
/// # Constitution V Principle V (audit content-ignorance)
///
/// Every field is an audit-permitted term: a [`FactRef`] token canonical,
/// a [`Span`]'s byte offsets, and a BLAKE3 `[u8; 32]` digest — no
/// document bytes, no free-form strings.
///
/// `Debug` / `Clone` / `PartialEq` / `Eq` are written manually (over
/// `S::OpenVocabRef` bounds) for the same reason as on [`FactRef`].
pub struct RelocatePriorState<S: MarkingScheme + ?Sized> {
    /// The token that was relocated.
    pub token: FactRef<S>,
    /// The byte span the token occupied at its origin scope.
    pub origin_span: Span,
    /// BLAKE3 digest of the pre-fix bytes (content-ignorant digest).
    pub digest: [u8; 32],
}

impl<S: MarkingScheme + ?Sized> Debug for RelocatePriorState<S> {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("RelocatePriorState")
            .field("token", &self.token)
            .field("origin_span", &self.origin_span)
            .field("digest", &self.digest)
            .finish()
    }
}

impl<S: MarkingScheme + ?Sized> Clone for RelocatePriorState<S> {
    fn clone(&self) -> Self {
        Self {
            token: self.token.clone(),
            origin_span: self.origin_span,
            digest: self.digest,
        }
    }
}

impl<S: MarkingScheme + ?Sized> PartialEq for RelocatePriorState<S> {
    fn eq(&self, other: &Self) -> bool {
        self.token == other.token
            && self.origin_span == other.origin_span
            && self.digest == other.digest
    }
}

impl<S: MarkingScheme + ?Sized> Eq for RelocatePriorState<S> {}

/// Four structural fix variants.
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
/// - [`ReplacementIntent::Relocate`] — move a token from one scope to
///   another (D8, relocate-not-evict): the token is correct but lives
///   at the wrong scope (e.g. a token that belongs on the banner found
///   only on a portion). The fact set is preserved; the token changes
///   home.
///
/// No `Box<str>` payloads. No multi-span carriers. No `Delete` /
/// `Render` byte-surgery variants — those were the wrong abstraction
/// layer.
///
/// `Debug` and `Clone` are written manually rather than derived for
/// the same reason as on [`FactRef`] — to avoid over-constraining
/// the trait bound to `S: Debug + Clone`.
#[non_exhaustive]
pub enum ReplacementIntent<S: MarkingScheme + ?Sized> {
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
        /// Reversibility pre-state (#824 rough-in). `None` until the
        /// reversal pass (#824) populates it; Phase 0 only reserves
        /// the field. Content-ignorant when present (see
        /// [`RecanonPriorState`]).
        prior: Option<RecanonPriorState<S>>,
    },

    /// Relocate a token from one scope to another (D8,
    /// relocate-not-evict). The token is correct but lives at the
    /// wrong scope; the fact set is preserved across the move.
    Relocate {
        /// The scope the token currently lives at.
        from: Scope,
        /// The scope the token should move to.
        to: Scope,
        /// The token being relocated.
        token: FactRef<S>,
        /// Reversibility pre-state (#824 rough-in). Content-ignorant
        /// (see [`RelocatePriorState`]).
        prior: RelocatePriorState<S>,
    },
}

impl<S: MarkingScheme + ?Sized> Debug for ReplacementIntent<S> {
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
            ReplacementIntent::Recanonicalize { scope, prior } => f
                .debug_struct("Recanonicalize")
                .field("scope", scope)
                .field("prior", prior)
                .finish(),
            ReplacementIntent::Relocate {
                from,
                to,
                token,
                prior,
            } => f
                .debug_struct("Relocate")
                .field("from", from)
                .field("to", to)
                .field("token", token)
                .field("prior", prior)
                .finish(),
        }
    }
}

impl<S: MarkingScheme + ?Sized> Clone for ReplacementIntent<S> {
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
            ReplacementIntent::Recanonicalize { scope, prior } => {
                ReplacementIntent::Recanonicalize {
                    scope: *scope,
                    prior: prior.clone(),
                }
            }
            ReplacementIntent::Relocate {
                from,
                to,
                token,
                prior,
            } => ReplacementIntent::Relocate {
                from: *from,
                to: *to,
                token: token.clone(),
                prior: prior.clone(),
            },
        }
    }
}

// Constructor impl — requires `S: Sized` (no `?Sized`) so the
// `SmallVec` return type has a known size. Calling `fact_remove`
// from a `?Sized` context is a compile-time error by design;
// downstream callers always have a concrete scheme.
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
/// recanonicalization target.
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
    /// `Scope::Diff` and `Scope::Bundle` are the rejection cases.
    /// `Diff` is a rule-context query mode, not a projection-output
    /// scope; `Bundle` is a document-set rollup, not a
    /// recanonicalization / render target (same treatment as `Diff`).
    /// Neither is a meaningful recanonicalization target
    /// (architecture.md type-sketch invariant). `RecanonScope` does
    /// not gain a `Bundle` variant — it MAY later if #823 needs it,
    /// but it is not needed now. The other three variants round-trip
    /// with [`From<RecanonScope> for Scope`].
    type Error = ();

    fn try_from(s: Scope) -> Result<Self, Self::Error> {
        match s {
            Scope::Portion => Ok(RecanonScope::Portion),
            Scope::Page => Ok(RecanonScope::Page),
            Scope::Document => Ok(RecanonScope::Document),
            // A bundle is not a recanonicalization / render target —
            // same treatment as `Scope::Diff`.
            Scope::Bundle => Err(()),
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

    #[test]
    fn recanon_scope_rejects_bundle() {
        // A bundle is not a recanonicalization / render target — same
        // treatment as `Scope::Diff` (007 Phase 0b, T001).
        assert!(RecanonScope::try_from(Scope::Bundle).is_err());
    }

    // ---- SC-006 reversibility pre-state round-trip (T009) ----

    // A minimal closed-vocab scheme: `OpenVocabRef = Infallible` makes
    // the `FactRef::OpenVocab` arm statically unreachable, so the
    // pre-state carries only `FactRef::Cve` tokens.
    struct PriorStateScheme;

    #[derive(Debug, Clone, PartialEq, Eq)]
    struct PriorStateMarking(u32);

    impl crate::lattice::JoinSemilattice for PriorStateMarking {
        fn join(&self, other: &Self) -> Self {
            Self(self.0.max(other.0))
        }
    }

    impl MarkingScheme for PriorStateScheme {
        type Token = TokenId;
        type Marking = PriorStateMarking;
        type ParseError = ();
        type OpenVocabRef = core::convert::Infallible;
        type Parsed<'src> = ();
        type Canonical = ();
        type Projected = ();

        fn name(&self) -> &str {
            "prior-state-fake"
        }
        fn schema_version(&self) -> &str {
            "v0"
        }
        fn categories(&self) -> &[crate::category::Category] {
            &[]
        }
        fn constraints(&self) -> &[crate::constraint::Constraint] {
            &[]
        }
        fn templates(&self) -> &[crate::template::Template] {
            &[]
        }
        fn parse(
            &self,
            _: &str,
        ) -> Result<crate::ambiguity::Parsed<Self::Marking>, Self::ParseError> {
            Err(())
        }
        fn validate(&self, _: &Self::Marking) -> Vec<crate::constraint::ConstraintViolation> {
            vec![]
        }
        fn project(&self, _: Scope, _: &[Self::Marking]) -> Self::Marking {
            PriorStateMarking(0)
        }
        fn render_canonical(
            &self,
            _: &Self::Marking,
            _: &crate::RenderContext,
            _: &mut dyn core::fmt::Write,
        ) -> core::fmt::Result {
            Ok(())
        }
    }

    #[test]
    fn recanonicalize_prior_state_round_trips() {
        // Construct a `Recanonicalize` carrying synthetic audit-permitted
        // pre-state (FactRef::Cve tokens, a Span, a zero digest) and assert
        // every field reads back unchanged after a clone.
        //
        // `Recanonicalize { prior: None }` is OUT of round-trip scope until
        // #824 populates it — this test exercises only the populated form.
        //
        // G13 content-ignorance: the pre-state holds only token canonicals,
        // Span byte offsets, and a BLAKE3 digest — no content fields were
        // added, so the content-ignorance surface is unchanged.
        let prior = RecanonPriorState::<PriorStateScheme> {
            prior_tokens: Box::new([FactRef::Cve(TokenId(3)), FactRef::Cve(TokenId(7))]),
            prior_span: Span::new(4, 12),
            digest: [0u8; 32],
        };
        let intent: ReplacementIntent<PriorStateScheme> = ReplacementIntent::Recanonicalize {
            scope: RecanonScope::Page,
            prior: Some(prior.clone()),
        };
        // `ReplacementIntent` itself carries no `PartialEq`; the pre-state
        // structs do, so we read fields back through the clone.
        let cloned = intent.clone();
        match cloned {
            ReplacementIntent::Recanonicalize {
                scope,
                prior: Some(p),
            } => {
                assert_eq!(scope, RecanonScope::Page);
                assert_eq!(
                    &*p.prior_tokens,
                    &[FactRef::Cve(TokenId(3)), FactRef::Cve(TokenId(7))]
                );
                assert_eq!(p.prior_span, Span::new(4, 12));
                assert_eq!(p.digest, [0u8; 32]);
                assert_eq!(p, prior);
            }
            _ => panic!("expected Recanonicalize with Some(prior)"),
        }
    }

    #[test]
    fn relocate_prior_state_round_trips() {
        // SC-006 (T009): a `Relocate` with synthetic audit-permitted
        // pre-state round-trips through clone + field read-back.
        let prior = RelocatePriorState::<PriorStateScheme> {
            token: FactRef::Cve(TokenId(9)),
            origin_span: Span::new(0, 5),
            digest: [0u8; 32],
        };
        let intent: ReplacementIntent<PriorStateScheme> = ReplacementIntent::Relocate {
            from: Scope::Portion,
            to: Scope::Page,
            token: FactRef::Cve(TokenId(9)),
            prior: prior.clone(),
        };
        let cloned = intent.clone();
        match cloned {
            ReplacementIntent::Relocate {
                from,
                to,
                token,
                prior: p,
            } => {
                assert_eq!(from, Scope::Portion);
                assert_eq!(to, Scope::Page);
                assert_eq!(token, FactRef::Cve(TokenId(9)));
                assert_eq!(p.origin_span, Span::new(0, 5));
                assert_eq!(p.digest, [0u8; 32]);
                assert_eq!(p, prior);
            }
            _ => panic!("expected Relocate"),
        }
    }

    // ---- Debug / Clone / PartialEq impl coverage (manual impls) ----

    fn recanon_prior() -> RecanonPriorState<PriorStateScheme> {
        RecanonPriorState {
            prior_tokens: Box::new([FactRef::Cve(TokenId(3)), FactRef::Cve(TokenId(4))]),
            prior_span: Span::new(0, 4),
            digest: [1u8; 32],
        }
    }

    fn relocate_prior() -> RelocatePriorState<PriorStateScheme> {
        RelocatePriorState {
            token: FactRef::Cve(TokenId(9)),
            origin_span: Span::new(1, 6),
            digest: [2u8; 32],
        }
    }

    #[test]
    fn recanon_prior_state_debug_clone_eq() {
        let prior = recanon_prior();

        // Exercises the manual `Debug` impl.
        let dbg = format!("{prior:?}");
        assert!(dbg.contains("RecanonPriorState"));
        assert!(dbg.contains("prior_tokens"));
        assert!(dbg.contains("prior_span"));
        assert!(dbg.contains("digest"));

        // Exercises the manual `Clone` + `PartialEq` (equal branch).
        let cloned = prior.clone();
        assert_eq!(prior, cloned);

        // Exercises the not-equal branch of `PartialEq` (one field differs).
        let different = RecanonPriorState::<PriorStateScheme> {
            prior_tokens: prior.prior_tokens.clone(),
            prior_span: Span::new(0, 9),
            digest: prior.digest,
        };
        assert_ne!(prior, different);
    }

    #[test]
    fn relocate_prior_state_debug_clone_eq() {
        let prior = relocate_prior();

        // Exercises the manual `Debug` impl.
        let dbg = format!("{prior:?}");
        assert!(dbg.contains("RelocatePriorState"));
        assert!(dbg.contains("token"));
        assert!(dbg.contains("origin_span"));
        assert!(dbg.contains("digest"));

        // Exercises the manual `Clone` + `PartialEq` (equal branch).
        let cloned = prior.clone();
        assert_eq!(prior, cloned);

        // Exercises the not-equal branch of `PartialEq` (one field differs).
        let different = RelocatePriorState::<PriorStateScheme> {
            token: prior.token.clone(),
            origin_span: Span::new(2, 7),
            digest: prior.digest,
        };
        assert_ne!(prior, different);
    }

    #[test]
    fn replacement_intent_debug_covers_all_arms() {
        // FactAdd arm.
        let fact_add: ReplacementIntent<PriorStateScheme> = ReplacementIntent::FactAdd {
            token: FactRef::Cve(TokenId(1)),
            scope: Scope::Portion,
        };
        assert!(format!("{fact_add:?}").contains("FactAdd"));

        // FactRemove arm (via the `fact_remove` constructor).
        let fact_remove = ReplacementIntent::<PriorStateScheme>::fact_remove(
            FactRef::Cve(TokenId(2)),
            Scope::Page,
        );
        let fr_dbg = format!("{fact_remove:?}");
        assert!(fr_dbg.contains("FactRemove"));
        assert!(matches!(
            fact_remove,
            ReplacementIntent::FactRemove { ref facts, .. } if facts.len() == 1
        ));

        // Recanonicalize arm with `prior: Some(..)` — covers the
        // `Recanonicalize` Debug arm and clone path.
        let recanon: ReplacementIntent<PriorStateScheme> = ReplacementIntent::Recanonicalize {
            scope: RecanonScope::Document,
            prior: Some(recanon_prior()),
        };
        let rc_dbg = format!("{recanon:?}");
        assert!(rc_dbg.contains("Recanonicalize"));
        assert!(rc_dbg.contains("RecanonPriorState"));
        let _ = recanon.clone();

        // Relocate arm — covers the `Relocate` Debug arm and clone path.
        let relocate: ReplacementIntent<PriorStateScheme> = ReplacementIntent::Relocate {
            from: Scope::Portion,
            to: Scope::Document,
            token: FactRef::Cve(TokenId(5)),
            prior: relocate_prior(),
        };
        let rl_dbg = format!("{relocate:?}");
        assert!(rl_dbg.contains("Relocate"));
        assert!(rl_dbg.contains("RelocatePriorState"));
        let _ = relocate.clone();
    }
}
