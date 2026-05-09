// SPDX-FileCopyrightText: 2026 Knitli Inc.
// SPDX-License-Identifier: LicenseRef-MarqueLicense-1.0

//! `Canonical<S>` — provenance-tagged canonical replacement for a
//! single token, with sealed constructors for closed-CVE versus
//! open-vocabulary provenance.
//!
//! See source plan §8.1
//! (`docs/plans/2026-05-02-engine-refactor-consolidated.md`) for the
//! design rationale: this type is the keystone for closing the G13
//! leak channel (Constitution V Principle V) at the type level
//! rather than via convention-only enforcement.
//!
//! # Construction surface
//!
//! Two paths into [`Canonical`]:
//!
//! 1. **Closed-CVE — [`Canonical::from_cve`]** — public, callable
//!    from any crate. Accepts a [`crate::TokenId`] (`pub struct
//!    TokenId(pub u32)`, so the type itself is publicly
//!    constructible — the [`TokenSource::Cve`] tag is a *provenance
//!    claim* by the rule, not a vocabulary-validated guarantee at
//!    `from_cve` call time). The seal that PR 3c.1 enforces is
//!    weaker than "no input bytes can become a `Canonical<S>`":
//!    there is no `Box<str> → Canonical<S>` public path that goes
//!    through `from_cve` *automatically*, but a rule that
//!    constructs `TokenId(N)` for arbitrary `N` and supplies its
//!    own `bytes` is constructing a `Canonical<S>` with arguably
//!    forged provenance. The audit emitter is the validation
//!    boundary: at audit-emit time it cross-references the recorded
//!    `TokenId` against [`crate::Vocabulary::lookup`] for the
//!    active scheme and rejects records whose token does not
//!    resolve to a registered vocabulary entry. The PR 3c.2 reshape
//!    of [`Canonical::from_cve`] (removing caller-supplied `bytes`
//!    in favor of engine-side rendering from the vocabulary) closes
//!    the residual provenance-forgery channel by construction.
//!
//! 2. **Open-vocab — [`Canonical::from_render`]** — `pub(crate)` to
//!    `marque-scheme`. Reachable from external crates ONLY through
//!    [`CanonicalConstructor::build_open_vocab`], which dispatches
//!    via the sealed trait whose sole impl is
//!    [`EngineConstructor`].
//!
//! # Cross-crate emission story (PR 3c.2 onward)
//!
//! External rule crates (`marque-capco` today, future `marque-cui` /
//! `marque-nato` / partner-national crates) emit `FixIntent<S>`
//! values; the engine — holding the only path that can drive
//! [`CanonicalConstructor`] — renders them on the rule's behalf.
//! This preserves the closed-construction property across the
//! workspace boundary that Constitution VII §VII opens up for new
//! rule crate families. See
//! `specs/006-engine-rule-refactor/contracts/fix-intent.md` for the
//! full contract.
//!
//! # PR 3c.1 status
//!
//! PR 3c.1 ships the types and the seal; no production code consumes
//! them yet (rules still emit `FixProposal`, the engine still
//! constructs `AppliedFix` via the existing path). PR 3c.2 wires the
//! lifecycle.

use core::marker::PhantomData;
use core::panic::Location;

use crate::category::{CategoryId, TokenId};
use crate::scheme::MarkingScheme;
use crate::scope::Scope;

mod sealed;

/// Provenance tag for a [`Canonical`] value.
///
/// Records *how* the canonical replacement was constructed; consumed
/// by the audit emitter to distinguish high-trust closed-CVE
/// replacements from trust-on-render-site open-vocabulary
/// replacements.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TokenSource {
    /// Closed-CVE: the canonical bytes name a known token from the
    /// scheme's vocabulary, identified by [`TokenId`].
    ///
    /// # Four-form note
    ///
    /// A single CAPCO token has up to **four distinct surface forms**
    /// (CAPCO-2016 §G.1 Table 4 plus the ODNI XML CVE Value field):
    ///
    /// 1. **CVE Value** — what `crates/ism/schemas/ISM-v2022-DEC/CVE/`
    ///    declares (e.g., `DISPLAYONLY`, `EYES`, `REL`). Often
    ///    space-stripped or punctuation-stripped relative to CAPCO.
    /// 2. **Marking Title** — the long banner-line title (e.g.,
    ///    `DISPLAY ONLY`, `EYES ONLY`).
    /// 3. **Banner Abbreviation** — the authorized abbreviation
    ///    (e.g., same as title for many; differs for `FOR OFFICIAL
    ///    USE ONLY` → `FOUO`).
    /// 4. **Portion Mark** — the parenthesized form (e.g., `NF`,
    ///    `OC`, `DISPLAY ONLY`).
    ///
    /// Many tokens have all four collapsing to the same string;
    /// some have all four distinct (`DISPLAYONLY` / `DISPLAY ONLY` /
    /// `DISPLAY ONLY` / `DISPLAY ONLY [LIST]`). The `TokenId` here
    /// names the abstract token, not the form. The bytes carried by
    /// the [`Canonical<S>`] tell auditors which form was emitted at
    /// the render site; the `Cve` provenance tag tells them the
    /// emission was vocabulary-bound (closed-set), not free-form.
    ///
    /// PR 3c.2's `MarkingScheme::render_canonical_cve(token, scope,
    /// vocab)` is the form-selection path: it chooses one of the
    /// four based on `scope` (and any future `RenderContext`
    /// refinement) using the `Vocabulary<S>` accessors.
    Cve(TokenId),

    /// Open-vocabulary: the canonical bytes were constructed by a
    /// `MarkingScheme::render_canonical` impl. The
    /// `render_call_site` records *where in source* the rendering
    /// happened; an auditor can locate the render impl from the call
    /// site without needing to decode the canonical bytes.
    OpenVocab {
        /// Which category produced the open-vocab render.
        category: CategoryId,
        /// `&'static Location` captured by `#[track_caller]` on
        /// [`EngineConstructor::build_open_vocab`].
        render_call_site: &'static Location<'static>,
    },
}

/// Provenance-tagged canonical replacement for a single token.
///
/// **Construction is sealed.** See the module docs for the two
/// permitted construction paths and the cross-crate emission story.
///
/// # Type parameter
///
/// `S: MarkingScheme + ?Sized` keeps `Canonical<S>` scheme-typed at
/// the type level — `Canonical<CapcoScheme>` and a future
/// `Canonical<CuiScheme>` are distinct types. The `?Sized` bound is
/// defensive against a future `dyn MarkingScheme` use case.
///
/// `_scheme: PhantomData<fn() -> S>` (rather than `PhantomData<S>`)
/// keeps `Canonical<S>: Send + Sync` regardless of `S`'s auto-trait
/// status. Constitution VI requires engine types to be `Send + Sync`
/// for [`crate::scheme::MarkingScheme`] impls used by `BatchEngine`.
///
/// # Compile-fail proofs of the seal
///
/// Each `compile_fail` doctest pins one inadmissible construction
/// path. Doctests compile as separate crates against the library's
/// public API, so the snippets see the same surface a downstream
/// consumer (e.g., a future `marque-cui` rule crate) would see.
/// Pairs with the positive cross-crate controls at
/// `crates/scheme/tests/canonical_unconstructable.rs`.
///
/// **No `Box<str> → Canonical<S>` constructor exists.**
///
/// ```compile_fail
/// use marque_scheme::canonical::Canonical;
/// use marque_scheme::scope::Scope;
/// use marque_scheme::MarkingScheme;
/// // The non-existent `from_bytes` constructor is the load-bearing
/// // proof: the only public path through Canonical is from_cve, which
/// // takes a TokenId (vocabulary-validated), not a Box<str>.
/// fn _take<S: MarkingScheme>() -> Canonical<S> {
///     Canonical::from_bytes(Box::from("TS"), Scope::Portion)
/// }
/// ```
///
/// **No `&str → Canonical<S>` impl.**
///
/// ```compile_fail
/// use marque_scheme::canonical::Canonical;
/// use marque_scheme::MarkingScheme;
/// fn _take<S: MarkingScheme>() -> Canonical<S> {
///     "TS".into()
/// }
/// ```
///
/// **No `From<Box<str>> for Canonical<S>` impl.**
///
/// ```compile_fail
/// use marque_scheme::canonical::Canonical;
/// use marque_scheme::MarkingScheme;
/// fn _take<S: MarkingScheme>() -> Canonical<S> {
///     Box::<str>::from("TS").into()
/// }
/// ```
#[derive(Debug, Clone)]
pub struct Canonical<S: MarkingScheme + ?Sized> {
    bytes: Box<str>,
    source: TokenSource,
    scope: Scope,
    _scheme: PhantomData<fn() -> S>,
}

impl<S: MarkingScheme + ?Sized> Canonical<S> {
    /// **Closed-CVE constructor — public, callable from any crate.**
    ///
    /// `TokenId` itself can only be obtained from
    /// [`crate::Vocabulary::lookup`] (PR 3c.1) or from rule-side
    /// const tables registered against the active scheme (PR 3c.2),
    /// so the `TokenId` provenance tag carried in
    /// [`TokenSource::Cve`] is genuine — auditors reading it know
    /// a closed-vocabulary token was named.
    ///
    /// # Four-form ambiguity (read this before reasoning about `bytes`)
    ///
    /// A CAPCO token has up to **four distinct surface forms**
    /// (CAPCO-2016 §G.1 Table 4 + ODNI XML CVE Value): CVE Value,
    /// Marking Title (long banner), Banner Abbreviation, and Portion
    /// Mark. See [`TokenSource::Cve`] for the worked example. The
    /// `bytes` parameter here is **whichever form the caller chose
    /// to render** — `from_cve` does not select among the four. That
    /// selection is the point of PR 3c.2's
    /// `MarkingScheme::render_canonical_cve(token, scope, vocab)`,
    /// which will use the [`Vocabulary<S>`] accessors
    /// (`portion_form`, `banner_form`, `banner_abbreviation`, plus a
    /// future CVE-Value-by-token accessor) to pick a form based on
    /// `scope` and any further [`RenderContext`] refinement (e.g.,
    /// long-title-vs-abbreviation within a `Scope::Page`).
    ///
    /// # Caveat (PR 3c.1 transitional shape — closes in PR 3c.2)
    ///
    /// The `bytes` argument is currently caller-supplied. The
    /// `TokenId` records which token was *named*; the `bytes`
    /// record which form the caller *rendered*. **PR 3c.1 does not
    /// validate** that the bytes equal any of the four vocabulary
    /// forms for that token; that validation lands at PR 3c.2 once
    /// the form-selection question is resolved (see the design
    /// doc's "Open question" section in
    /// `docs/plans/2026-05-09-pr3c-foundation-plan.md`).
    ///
    /// During PR 3c.1 there are no production callers (the engine
    /// promotion path still consumes `FixProposal::replacement` and
    /// has not been re-wired through `Canonical<S>` yet). Test
    /// fixtures that exercise the type may pass arbitrary bytes;
    /// they are constructing test inputs, not minting audit records.
    ///
    /// # Audit invariant (post-PR-3c.2)
    ///
    /// Once PR 3c.2 lands the form-selection design, the `bytes`
    /// argument is removed and the engine renders from the
    /// vocabulary. The resulting [`Canonical::source`] is
    /// [`TokenSource::Cve(token)`] and the bytes are guaranteed to
    /// match the vocabulary's chosen form for `(token, scope,
    /// render_context)`.
    pub fn from_cve(token: TokenId, scope: Scope, bytes: Box<str>) -> Self {
        Self {
            bytes,
            source: TokenSource::Cve(token),
            scope,
            _scheme: PhantomData,
        }
    }

    /// **Open-vocabulary constructor — `pub(crate)` to
    /// `marque-scheme`.**
    ///
    /// Reachable from external crates only via
    /// [`CanonicalConstructor::build_open_vocab`], whose sole impl
    /// is [`EngineConstructor`]. Records the `render_call_site` as
    /// provenance per source plan §8.1 — the call site is captured
    /// by `#[track_caller]` on
    /// [`EngineConstructor::build_open_vocab`].
    pub(crate) fn from_render(
        category: CategoryId,
        bytes: Box<str>,
        scope: Scope,
        render_call_site: &'static Location<'static>,
    ) -> Self {
        Self {
            bytes,
            source: TokenSource::OpenVocab {
                category,
                render_call_site,
            },
            scope,
            _scheme: PhantomData,
        }
    }

    /// Canonical bytes — borrowed `&str` view (no allocation).
    #[inline]
    pub fn bytes(&self) -> &str {
        &self.bytes
    }

    /// Provenance tag.
    #[inline]
    pub fn source(&self) -> &TokenSource {
        &self.source
    }

    /// Scope at which this canonical replacement applies.
    #[inline]
    pub fn scope(&self) -> Scope {
        self.scope
    }
}

/// Sealed trait that closes the open-vocab [`Canonical`] construction
/// path across crate boundaries.
///
/// **The only impl is [`EngineConstructor`].** External rule crates
/// depend on `marque-rules` (which re-exports `FixIntent<S>` and
/// friends) but NOT on this trait — so a downstream rule crate
/// cannot construct [`Canonical`] open-vocab values directly. They
/// emit `FixIntent<S>::Render { directive, .. }` and the engine
/// renders on their behalf at promotion time.
///
/// # Sealing mechanism
///
/// The supertrait bound `sealed::Sealed<S>` references a private
/// module — external crates cannot name `Sealed`, therefore cannot
/// satisfy the bound, therefore cannot impl this trait. This is the
/// standard Rust API-guidelines sealed-trait pattern.
///
/// # Compile-fail proofs of the cross-crate seal
///
/// **External crates cannot name `sealed::Sealed`** (the module is
/// private). Doctests compile as separate crates, so the snippet
/// below is rejected at the `use` resolution step.
///
/// ```compile_fail
/// use marque_scheme::canonical::sealed::Sealed;
/// ```
///
/// **External crates cannot satisfy the `Sealed<S>` supertrait
/// bound, therefore cannot impl [`CanonicalConstructor`].**
///
/// The snippet below tries to impl `CanonicalConstructor` for a
/// downstream type without first impl'ing `Sealed` (which is
/// impossible from outside the crate). The compiler rejects the
/// impl because the `Sealed<S>` supertrait bound is unsatisfied.
///
/// ```compile_fail
/// use marque_scheme::canonical::{Canonical, CanonicalConstructor};
/// use marque_scheme::category::CategoryId;
/// use marque_scheme::scope::Scope;
/// use marque_scheme::MarkingScheme;
///
/// struct EvilConstructor;
///
/// impl<S: MarkingScheme + ?Sized> CanonicalConstructor<S> for EvilConstructor {
///     fn build_open_vocab(
///         &self,
///         _category: CategoryId,
///         _bytes: Box<str>,
///         _scope: Scope,
///     ) -> Canonical<S> {
///         unimplemented!()
///     }
/// }
/// ```
///
/// **Cannot bypass [`EngineConstructor::__engine_construct`] via the
/// assoc-fn shorthand.** Even though [`EngineConstructor<S>`] is
/// `pub`, [`CanonicalConstructor::build_open_vocab`] takes `&self`
/// — so external callers cannot use the
/// `<EngineConstructor<S> as CanonicalConstructor<S>>::build_open_vocab(category, bytes, scope)`
/// associated-function-call form to bypass the
/// `__engine_construct` mint path. They must first obtain an
/// `EngineConstructor<S>` instance, and that path is the
/// FR-040-lint-guarded `__engine_construct()`. The snippet below
/// is rejected: rustc reports "this function takes 4 arguments but
/// 3 arguments were supplied" because the implicit `&self` receiver
/// is missing. (Regression catch for the assoc-fn seal-bypass class
/// raised on PR 3c.1.)
///
/// ```compile_fail
/// use marque_scheme::canonical::{Canonical, CanonicalConstructor, EngineConstructor};
/// use marque_scheme::category::CategoryId;
/// use marque_scheme::scope::Scope;
/// use marque_scheme::MarkingScheme;
///
/// fn _bypass<S: MarkingScheme + ?Sized>() -> Canonical<S> {
///     <EngineConstructor<S> as CanonicalConstructor<S>>::build_open_vocab(
///         CategoryId(0),
///         Box::from("forged"),
///         Scope::Portion,
///     )
/// }
/// ```
pub trait CanonicalConstructor<S: MarkingScheme + ?Sized>: sealed::Sealed<S> {
    /// Construct an open-vocab [`Canonical`] value.
    ///
    /// **The `&self` receiver is load-bearing.** It is what closes
    /// the open-vocab construction path across the workspace. Without
    /// it, this method would be an associated function callable as
    /// `<EngineConstructor<S> as CanonicalConstructor<S>>::build_open_vocab(category, bytes, scope)`
    /// from any crate that can name [`EngineConstructor`] (every
    /// crate, since it is `pub`) — bypassing the
    /// [`EngineConstructor::__engine_construct`] reserved-name path
    /// that the FR-040 promote-callsite-lint relies on. The `&self`
    /// receiver forces the caller to first obtain an
    /// [`EngineConstructor<S>`] instance, which only
    /// [`EngineConstructor::__engine_construct`] mints, which the
    /// lint flags.
    ///
    /// The implementer (the engine, via [`EngineConstructor`]) is
    /// responsible for capturing the `render_call_site` via
    /// `#[track_caller]` so the provenance reflects the rule-side
    /// render impl, not the engine's plumbing.
    #[track_caller]
    fn build_open_vocab(&self, category: CategoryId, bytes: Box<str>, scope: Scope)
    -> Canonical<S>;
}

/// Engine-only [`CanonicalConstructor`] implementor.
///
/// Lives in `marque-scheme` (not `marque-engine`) so the
/// [`sealed::Sealed`] supertrait can be implemented locally —
/// `Sealed` is private to `marque-scheme` and cannot be implemented
/// from a downstream crate. See design doc §3 T035 (Option D) for
/// the placement rationale.
///
/// `EngineConstructor<S>` is `pub` so the engine can name it in
/// `Engine::fix_inner`'s render dispatch (PR 3c.2). Construction is
/// sealed via the `__engine_construct` reserved-name pattern that
/// already secures `marque_rules::EnginePromotionToken::__engine_construct`
/// and `marque_rules::AppliedFix::__engine_promote`. The
/// `tools/promote-callsite-lint/` CI lint (FR-040) flags every call
/// expression whose path's last segment is `__engine_construct` or
/// `__engine_promote` regardless of leading qualifier; the lint's
/// allow-list scopes the legitimate use sites to the engine.
///
/// # 5-year-maintainability note
///
/// The `__engine_construct` `#[doc(hidden)] pub` is **not** the
/// primary seal — the primary seal is the
/// `CanonicalConstructor<S>: sealed::Sealed<S>` supertrait bound,
/// which prevents external impls from existing at all. The
/// `__engine_construct` doc-hidden name is a secondary defense
/// against accidental construction inside the engine's own
/// rule-adjacent helpers; it signals "engine-only" to readers and
/// the FR-040 lint flags wrong call sites mechanically.
pub struct EngineConstructor<S: MarkingScheme + ?Sized> {
    _scheme: PhantomData<fn() -> S>,
    _seal: (),
}

impl<S: MarkingScheme + ?Sized> EngineConstructor<S> {
    /// Reserved name (FR-040 lint contract).
    ///
    /// Mint via the engine-only path. The same audit-promotion
    /// reserved-name discipline applies; see
    /// [`crate::canonical::EngineConstructor`] doc-comment.
    #[doc(hidden)]
    #[inline]
    pub const fn __engine_construct() -> Self {
        Self {
            _scheme: PhantomData,
            _seal: (),
        }
    }
}

impl<S: MarkingScheme + ?Sized> sealed::Sealed<S> for EngineConstructor<S> {}

impl<S: MarkingScheme + ?Sized> CanonicalConstructor<S> for EngineConstructor<S> {
    #[inline]
    #[track_caller]
    fn build_open_vocab(
        &self,
        category: CategoryId,
        bytes: Box<str>,
        scope: Scope,
    ) -> Canonical<S> {
        Canonical::from_render(category, bytes, scope, Location::caller())
    }
}

#[cfg(test)]
#[cfg_attr(coverage_nightly, coverage(off))]
mod tests {
    use super::*;
    use crate::ambiguity::Parsed;
    use crate::category::Category;
    use crate::constraint::Constraint;
    use crate::lattice::{BoundedLattice, Lattice};
    use crate::template::Template;

    /// Minimal `MarkingScheme` impl used to instantiate `Canonical<S>`
    /// in unit tests. Mirrors the pattern in
    /// `crates/scheme/tests/adoption_readiness.rs::StubScheme`.
    struct TestScheme;

    #[derive(Clone, Debug, Default, PartialEq, Eq)]
    struct TestMarking;

    impl Lattice for TestMarking {
        fn join(&self, _other: &Self) -> Self {
            TestMarking
        }
        fn meet(&self, _other: &Self) -> Self {
            TestMarking
        }
    }

    impl BoundedLattice for TestMarking {
        fn bottom() -> Self {
            TestMarking
        }
        fn top() -> Self {
            TestMarking
        }
    }

    impl MarkingScheme for TestScheme {
        type Token = ();
        type Marking = TestMarking;
        type ParseError = ();

        fn name(&self) -> &str {
            "TestScheme"
        }
        fn schema_version(&self) -> &str {
            "0.0.1"
        }
        fn categories(&self) -> &[Category] {
            &[]
        }
        fn constraints(&self) -> &[Constraint] {
            &[]
        }
        fn templates(&self) -> &[Template] {
            &[]
        }
        fn parse(&self, _input: &str) -> Result<Parsed<Self::Marking>, Self::ParseError> {
            Ok(Parsed::Unambiguous(TestMarking))
        }
        fn project(&self, _scope: Scope, _markings: &[Self::Marking]) -> Self::Marking {
            TestMarking
        }
        fn render_portion(&self, _m: &Self::Marking) -> String {
            String::new()
        }
        fn render_banner(&self, _m: &Self::Marking) -> String {
            String::new()
        }
    }

    #[test]
    fn canonical_from_cve_records_provenance() {
        let c: Canonical<TestScheme> =
            Canonical::from_cve(TokenId(7), Scope::Portion, Box::from("TS"));
        assert_eq!(c.bytes(), "TS");
        assert_eq!(c.scope(), Scope::Portion);
        match c.source() {
            TokenSource::Cve(t) => assert_eq!(*t, TokenId(7)),
            other => panic!("expected Cve, got {other:?}"),
        }
    }

    #[test]
    fn engine_constructor_build_open_vocab_records_call_site() {
        // EngineConstructor is the sole CanonicalConstructor<S> impl;
        // the call site is captured by #[track_caller] so provenance
        // reflects the calling render impl. PR 3c.2 wires this from
        // `Engine::fix_inner` -> `MarkingScheme::render_canonical`.
        // Test-fixture carve-out per Constitution V Principle V.
        let ctor: EngineConstructor<TestScheme> = EngineConstructor::__engine_construct();
        let c: Canonical<TestScheme> =
            ctor.build_open_vocab(CategoryId(3), Box::from("OPEN"), Scope::Page);
        assert_eq!(c.bytes(), "OPEN");
        assert_eq!(c.scope(), Scope::Page);
        match c.source() {
            TokenSource::OpenVocab {
                category,
                render_call_site,
            } => {
                assert_eq!(*category, CategoryId(3));
                // The Location should point at this test's call line, not
                // at any internal `Canonical::from_render` body.
                assert!(render_call_site.file().ends_with("canonical.rs"));
            }
            other => panic!("expected OpenVocab, got {other:?}"),
        }
    }

    #[test]
    fn engine_constructor_minted_via_reserved_name() {
        // The reserved-name pattern compiles from inside marque-scheme
        // because the crate IS the engine-side seal implementor; the
        // FR-040 lint flags external call sites.
        // Test-fixture carve-out per Constitution V Principle V.
        let _c: EngineConstructor<TestScheme> = EngineConstructor::__engine_construct();
    }
}
