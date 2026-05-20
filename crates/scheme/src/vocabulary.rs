// SPDX-FileCopyrightText: 2026 Knitli Inc.
//
// SPDX-License-Identifier: LicenseRef-MarqueLicense-1.0

//! Per-token metadata surface.
//!
//! A [`Vocabulary`] exposes structured metadata about every token a
//! scheme knows — authority, owner/producer, point of contact,
//! deprecation status, canonical portion/banner forms, and the full
//! [`TokenMetadataFull`] record.
//!
//! Phase E wires every active ISM/CAPCO token to a `&'static`
//! `TokenMetadataFull` entry generated at build time from the ODNI
//! JSON sidecar (`marque-ism/build.rs`, task T080). Phase 2 ships the
//! trait surface only; the impl lands in Phase 5 (tasks T080–T084).
//!
//! ## Invariants (enforced by impl + tests in Phase 5)
//!
//! - Every return is `&'static` data — no runtime allocation (SC-008).
//! - Active tokens populate every non-`Option` field, with one
//!   documented carve-out: a scheme adapter MAY elide informational
//!   `&'static str` fields (free-form names, descriptions, contact
//!   details — not URNs, codes, or schema versions) to an empty
//!   string on size-constrained build targets (today: `wasm32`),
//!   provided the elision is documented on the field doc-comment.
//!   See `crates/ism/build.rs::generate_vocabulary` (issue #453) for
//!   the CAPCO adapter's `wasm32` elision of `Authority::source_name`,
//!   `PointOfContact::name`/`email`, and `TokenMetadataEntry::description`.
//! - Deprecated tokens additionally populate `deprecation`.
//! - The FOUO → CUI migration is absent from the migration table
//!   (FR-020): FOUO remains an active valid dissemination control.

use crate::category::CategoryId;
use crate::scheme::MarkingScheme;

/// Authority under which a token is published.
///
/// For ISM tokens this is ODNI plus the schema package version
/// (`ISM-v2022-DEC`). Future NATO / FGI / JOINT vocabularies will
/// carry their own authority records.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Authority {
    /// Human-readable source name, e.g., `"ODNI"` or
    /// `"NATO Military Committee"`.
    pub source_name: &'static str,
    /// URN for the authority. Granularity is scheme-defined — a
    /// scheme MAY populate this with a coarse system-root URN
    /// (e.g., `"urn:us:gov:ic:ism"`) shared across every token, or a
    /// finer per-publishing-file URN (e.g.,
    /// `"urn:us:gov:ic:cvenum:ism:dissem"`). CAPCO's adapter uses
    /// the per-CVE-file form so each token's `metadata.urn` and its
    /// `metadata.authority.urn` agree by construction; future
    /// schemes (CUI, NATO, JOINT) are free to choose differently.
    /// Audit consumers should not assume a relationship between
    /// `Authority::urn` and a token's own URN beyond what the
    /// scheme's documentation states.
    pub urn: &'static str,
    /// Schema version identifier, e.g., `"ISM-v2022-DEC"`.
    pub schema_version: &'static str,
    /// Point of contact for this authority.
    pub point_of_contact: PointOfContact,
}

/// Kind of owner/producer.
///
/// The ODNI `CVEnumISMCATOwnerProducer` values split along these
/// categories. Phase 5's build.rs derives this from the XML's union
/// pattern (NATO prefix, trigraph enumeration, FGI marker).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OwnerProducerKind {
    /// A national-level owner/producer (GENC trigraph).
    National,
    /// NATO-specific owner/producer.
    Nato,
    /// Foreign Government Information — non-specific placeholder.
    Fgi,
    /// An organization, coalition, or tetragraph owner/producer from
    /// the IC Markings System Register.
    Organization,
}

/// Entity that owns or produces a token.
///
/// For CAPCO classification levels this is the U.S. government
/// (`"USA"`). For FGI or JOINT markings this identifies the
/// originating nation / coalition / organization.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct OwnerProducer {
    /// The canonical code for the owner/producer (e.g., `"USA"`,
    /// `"NATO"`, `"FGI"`, a tetragraph).
    pub code: &'static str,
    /// Human-readable name, e.g., `"United States of America"`.
    pub name: &'static str,
    /// Classification of the owner/producer.
    pub kind: OwnerProducerKind,
}

/// Point of contact for dispute / update traffic.
///
/// Held `&'static` — populated at build time from ODNI JSON.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PointOfContact {
    pub name: &'static str,
    pub email: &'static str,
    pub organization: &'static str,
}

/// Deprecation metadata for a retired or superseded token.
///
/// `replacement = None` denotes the "no known replacement" case
/// (FR-017). The decoder uses that signal to avoid silently rewriting
/// a token into a replacement that does not exist.
///
/// The `Token` parameter carries no bounds on the struct definition —
/// the bounds live on the derives so downstream code doesn't inherit
/// unrelated constraints. `Clone`, `Debug`, `PartialEq`, and `Eq` are
/// auto-implemented when `Token` satisfies them; consumers that only
/// read via `&'static Deprecation<_>` get the type unconditionally.
///
/// PR 3d (FR-054) adds `valid_from` / `valid_until` to carry the
/// validity-window metadata schema needs for "evaluate as valid at
/// time of authoring". Both are `Option<&'static str>` because the
/// upstream sources (ODNI XSD annotations, JSON sidecars) carry
/// version metadata at the file level, not per-token, so today every
/// generated entry leaves them as `None`. The data plumbing is wired
/// so a future ODNI revision that exposes per-term version info can
/// populate them without a trait change.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Deprecation<Token> {
    /// Schema version at which the token was deprecated, e.g.,
    /// `"ISM-v2022-DEC"`.
    pub since: &'static str,
    /// Schema version at which the token was first published.
    ///
    /// Defaults to `None` when build.rs cannot derive it from ODNI
    /// XSD annotations or the migration table. The build-time
    /// invariant `valid_from <= since` MUST hold when both are
    /// populated (a token cannot be deprecated before it was
    /// published); see `crates/ism/tests/migrations_invariant.rs`.
    pub valid_from: Option<&'static str>,
    /// Schema version after which the token is no longer valid in
    /// newly-authored documents.
    ///
    /// Defaults to `None` when the token has no successor in the
    /// migration table (rare — FOUO-style "no replacement" cases per
    /// FR-017).
    pub valid_until: Option<&'static str>,
    /// Replacement token id, when one is defined.
    pub replacement: Option<Token>,
}

/// Full per-token metadata record.
///
/// Baked as `&'static` into the generated tables by
/// `marque-ism/build.rs` (task T080). Accessible through
/// [`Vocabulary::metadata`].
///
/// The `Token` parameter carries no bounds on the struct — see the
/// note on [`Deprecation`] for rationale.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TokenMetadataFull<Token> {
    /// Canonical (authoritative) token name, e.g., `"SECRET"`.
    pub canonical: &'static str,
    /// ODNI URN for the term.
    pub urn: &'static str,
    /// Schema version that first published this form.
    pub schema_version: &'static str,
    /// Publishing authority record.
    pub authority: Authority,
    /// Owner/producer record.
    pub owner_producer: OwnerProducer,
    /// Contact for disputes and updates.
    pub point_of_contact: PointOfContact,
    /// Deprecation metadata, when applicable.
    pub deprecation: Option<Deprecation<Token>>,
    /// Canonical portion-mark form, e.g., `"(S)"`.
    pub portion_form: &'static str,
    /// Canonical banner form, e.g., `"SECRET"`.
    pub banner_form: &'static str,
    /// Banner abbreviation when one exists (e.g., `"S"`).
    pub banner_abbreviation: Option<&'static str>,
}

/// Aggregated form-set for a single token (PR 3d, FR-053).
///
/// Replaces the closed-world "exactly three forms per token"
/// assumption baked into the original per-form trait methods. Every
/// scheme exposes a `&'static FormSet` per token via
/// [`Vocabulary::forms`]; the per-form accessors
/// ([`Vocabulary::portion_form`], [`Vocabulary::banner_form`],
/// [`Vocabulary::banner_abbreviation`]) become default-method
/// projections over the `FormSet`.
///
/// Field naming follows CAPCO-2016 §G.1 Table 4 column terms
/// (pp 36–38):
///
/// - `portion` — column 3, "Authorized Portion Mark". Always present.
/// - `banner_title` — column 1, "Authorized Banner Line Marking
///   Title". Always present.
/// - `banner_abbreviation` — column 2, "Authorized Banner Line
///   Abbreviation". `Some` only when the abbreviation is distinct
///   from `banner_title`; classifications carry `None` because the
///   Register lists no abbreviation for any classification row.
///
/// `recognized_aliases` carries forms the scheme accepts on input
/// but does not emit by default — e.g., ODNI ISM `<Description>`
/// titles that diverge from the CAPCO `banner_title`, or historical
/// aliases from a prior CAPCO revision. Engine policy (not data
/// shape) decides whether any alias may be promoted to emission.
///
/// Construction is build-time-only: a scheme's `forms()` impl returns
/// references into a `&'static` table populated by the build script
/// or hand-rolled const data. There is no public `FormSet::new`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FormSet {
    /// CAPCO §G.1 Table 4 column 3 — "Authorized Portion Mark".
    pub portion: &'static str,
    /// CAPCO §G.1 Table 4 column 1 — "Authorized Banner Line Marking
    /// Title".
    pub banner_title: &'static str,
    /// CAPCO §G.1 Table 4 column 2 — "Authorized Banner Line
    /// Abbreviation". `Some` only when distinct from `banner_title`.
    pub banner_abbreviation: Option<&'static str>,
    /// Forms recognized on input but not emitted by default. Each
    /// entry pairs a [`FormKind`] tag with the alias string.
    pub recognized_aliases: &'static [(FormKind, &'static str)],
}

/// Kind tag for an entry in [`FormSet::recognized_aliases`].
///
/// Marked `#[non_exhaustive]` because future schemes (NATO, CUI,
/// JOINT) will add their own recognize-only alias kinds without a
/// breaking change. Integration tests crossing the crate boundary
/// MUST include a wildcard arm.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum FormKind {
    /// ODNI ISM CVE `<Description>` body, when the published title
    /// disagrees with the scheme's `banner_title`. Recognize-only by
    /// default.
    IsmDescriptionTitle,
    /// Pre-dated alias from a prior CAPCO revision. Recognize-only.
    HistoricalAlias,
}

/// Vocabulary metadata accessors for a [`MarkingScheme`].
///
/// Every method returns `&'static` data so rule bodies can reference
/// metadata without allocating. A scheme implements this alongside
/// [`MarkingScheme`] — see `impl Vocabulary<CapcoScheme> for CapcoScheme`
/// (task T084 in Phase 5).
///
/// The trait takes the token by reference (`&S::Token`) — no `Copy`
/// bound is required. The Phase 5 CAPCO implementation uses the
/// simple [`crate::category::TokenId`] (`u32`-wrapper, trivially
/// `Copy`); schemes that prefer a richer non-`Copy` symbol type can
/// still implement this trait without change. The metadata structs
/// ([`TokenMetadataFull`] and [`Deprecation`]) likewise carry no
/// `Copy` bound on their `Token` parameter.
///
/// Implementations MUST be `Send + Sync` so the engine can hold them
/// in an `Arc<dyn Vocabulary<S>>` and dispatch across threads. Every
/// accessor returns `&'static` data, so the bound is essentially free
/// for the in-tree CAPCO impl; it forecloses a future scheme adopter
/// from accidentally building a `!Send` Vocabulary (e.g., one backed
/// by an interior-mutable cache) that the engine could not compile
/// against — pinning the bound on the trait makes the constraint
/// visible at the definition site instead of surfacing as a
/// downstream `Send`/`Sync` compile error in `BatchEngine`. Mirrors
/// the bound on [`crate::recognizer::Recognizer`] and
/// [`crate::codec::Codec`].
pub trait Vocabulary<S: MarkingScheme + ?Sized>: Send + Sync {
    /// Authority record for `token`.
    fn authority(&self, token: &S::Token) -> &'static Authority;

    /// Owner/producer record for `token`.
    fn owner_producer(&self, token: &S::Token) -> &'static OwnerProducer;

    /// Point-of-contact record for `token`.
    fn point_of_contact(&self, token: &S::Token) -> &'static PointOfContact;

    /// Deprecation metadata for `token`, or `None` if active.
    fn deprecation(&self, token: &S::Token) -> Option<&'static Deprecation<S::Token>>;

    /// Aggregated form-set for `token` (PR 3d, FR-053).
    ///
    /// Returns `&'static FormSet` carrying portion / banner-title /
    /// banner-abbreviation / recognized-aliases. The per-form
    /// accessors ([`Self::portion_form`], [`Self::banner_form`],
    /// [`Self::banner_abbreviation`]) project over this struct via
    /// the default impls below.
    ///
    /// **Open-vocabulary tokens** (country trigraphs, custom SCI
    /// control systems, SAR program IDs) are NOT addressable here —
    /// `forms()` is defined only over the closed-CVE token enum that
    /// the scheme's `Token` type enumerates. Schemes that need to
    /// surface open-vocab forms should expose a scheme-specific
    /// accessor that takes raw bytes rather than `&S::Token`.
    fn forms(&self, token: &S::Token) -> &'static FormSet;

    /// Canonical portion form for `token`, e.g., `"(S)"`.
    ///
    /// Default impl projects [`FormSet::portion`]. Schemes only
    /// override if they need a non-`FormSet`-based codepath.
    fn portion_form(&self, token: &S::Token) -> &'static str {
        self.forms(token).portion
    }

    /// Canonical banner form for `token`, e.g., `"SECRET"`.
    ///
    /// Default impl preserves the pre-3d "abbreviation when distinct,
    /// else title" semantics byte-for-byte:
    /// `forms(token).banner_abbreviation.unwrap_or(forms(token).banner_title)`.
    /// Schemes only override if they need a non-`FormSet`-based codepath.
    fn banner_form(&self, token: &S::Token) -> &'static str {
        let f = self.forms(token);
        f.banner_abbreviation.unwrap_or(f.banner_title)
    }

    /// Banner abbreviation when defined, else `None`.
    ///
    /// Default impl projects [`FormSet::banner_abbreviation`].
    /// Schemes only override if they need a non-`FormSet`-based
    /// codepath.
    fn banner_abbreviation(&self, token: &S::Token) -> Option<&'static str> {
        self.forms(token).banner_abbreviation
    }

    /// Returns `true` when `token` participates in this scheme's
    /// foreign-disclosure-and-release (FD&R) dominator set.
    ///
    /// In CAPCO, the FD&R set per §B.3.a p19 (definitional enumeration;
    /// §B.3 Table 2 pp 21-22 is the scenario-summary table, not the
    /// canonical enumeration) + §H.8 is
    /// `{NOFORN, RELIDO, DISPLAY ONLY, REL TO [any LIST], EYES}`
    /// (EYES deprecated 2017-10-01 per §H.8 p157 but still recognized
    /// for legacy-input compatibility). Membership in this set
    /// determines:
    ///
    /// 1. Eligibility as a [`crate::builtins::SupersessionSet`]
    ///    element on the dissem axis — the FD&R chain joins via
    ///    supersession, not plain union.
    /// 2. Suppressor membership for closure rules that propagate
    ///    NOFORN by default (Trio 1 of `marque-applied.md` §4.7.1):
    ///    an FD&R token already present in a portion suppresses the
    ///    implicit-NOFORN inference.
    /// 3. The canonical FD&R-set-membership accessor. In `marque-capco`
    ///    the implementation iterates a private `FDR_DOMINATORS` slice
    ///    (in `crates/capco/src/scheme.rs`); that same slice is the
    ///    data backing `Constraint::ConflictsWithFamily` predicates
    ///    such as `is_fdr_dominator`. **The two predicates answer
    ///    different questions** and intentionally diverge on RELIDO:
    ///    `is_fdr_dissem` answers "is `token` an FD&R *member*" and
    ///    admits RELIDO; `is_fdr_dominator` answers "is `token` an
    ///    FD&R *dominator over RELIDO*" for the RELIDO-conflict
    ///    family role and excludes RELIDO (RELIDO-vs-RELIDO is a
    ///    tautology). Do not delegate `is_fdr_dissem` through
    ///    `is_fdr_dominator` — see the maintenance contract on
    ///    `FDR_DOMINATORS` in `crates/capco/src/scheme.rs`.
    ///
    /// # Default impl returns `false`
    ///
    /// **A scheme that declares an FD&R dissemination family MUST
    /// override this method.** The default `false` is correct only
    /// for schemes with no FD&R concept (e.g., a hypothetical
    /// CUI-only scheme where every disclosure decision is
    /// unconditional). A scheme that *does* have FD&R but forgets to
    /// override will silently return `false` for every dominator
    /// token, which produces wrong banner roll-ups and incorrect
    /// closure firings. This is the same opt-in discipline used by
    /// [`crate::scheme::MarkingScheme::iter_present_tokens`] — see
    /// its doc comment. An override MUST cover every token that
    /// participates in the FD&R chain AND every category whose
    /// tokens collectively participate (e.g., CAPCO's REL TO
    /// country-list category).
    ///
    /// # Used by
    ///
    /// `marque-capco`'s `Lattice<DissemSet>` impl (PR 4b — to be
    /// wired) and the closure operator's Trio-1 implicit-NOFORN
    /// propagation (PR 3.7 catalog rows).
    fn is_fdr_dissem(&self, token: &S::Token) -> bool {
        let _ = token;
        false
    }

    /// Full metadata record for `token`.
    ///
    /// Preferred accessor — individual per-field methods exist so
    /// call sites can avoid pulling the entire struct when they only
    /// need one field, but the full record is the authoritative
    /// view.
    fn metadata(&self, token: &S::Token) -> &'static TokenMetadataFull<S::Token>;

    /// Qualified namespaced token label for audit-record `token_id` emission.
    ///
    /// Produces the namespaced `"Category.Token"` form (e.g.
    /// `"classification.secret"`) that consumers see in the `marque-1.0`
    /// audit NDJSON record's `replacement.canonical.token_id` field.
    /// Required by `contracts/audit-record.md` so audit consumers can
    /// resolve the canonical token's category without performing a
    /// separate vocabulary lookup per record (self-describing
    /// property).
    ///
    /// # Default impl
    ///
    /// The default returns `Cow::Borrowed("unknown.unknown")` —
    /// deliberately unhelpful. A scheme that exposes ANY tokens MUST
    /// override this method to produce the real `Category.Token`
    /// projection. The default is shaped to allow the trait to compile
    /// without panicking when called against schemes that have not yet
    /// migrated; PR 3c.2.D ships the CAPCO override but downstream
    /// audit consumers reading from a scheme that has not overridden
    /// this method will see `"unknown.unknown"` as a visible signal to
    /// open a migration ticket.
    ///
    /// # Return type
    ///
    /// `Cow<'static, str>` rather than `&'static str` because the
    /// projection may need to compose at runtime (no per-token baked
    /// table exists today; a future scheme build-time enhancement may
    /// pre-bake the namespaced string and return `Cow::Borrowed`).
    /// Audit emit happens off the lint/scan hot path so the per-call
    /// allocation of a short owned string (typically ≤32 bytes) is
    /// acceptable. Track follow-up at the audit-record-contract
    /// migration table.
    ///
    /// # PM-D-10 (PR 3c.2.D)
    ///
    /// Added per `docs/plans/2026-05-20-pr3c2-d-pm-decisions.md`.
    /// Constitution IV / VII preserved — the accessor is a wire-format
    /// projection helper, NOT a new lattice surface; it does not
    /// extend [`MarkingScheme`] or [`crate::Lattice`].
    fn qualified_token_label(&self, token: &S::Token) -> std::borrow::Cow<'static, str> {
        let _ = token;
        std::borrow::Cow::Borrowed("unknown.unknown")
    }

    /// Test whether `bytes` is admissible as a token in `category`.
    ///
    /// Admission is the union of two predicates depending on the
    /// category's vocabulary shape:
    ///
    /// - **Closed-CVE categories** (e.g., U.S. classification, IC
    ///   dissemination controls, AEA, non-US classification, joint
    ///   classification, non-IC dissemination): admission is exactly
    ///   *vocabulary membership*. Implementations should resolve
    ///   `bytes` against the canonical token table for the category
    ///   and return `true` iff the lookup succeeds. Schemes MAY
    ///   accept either the portion form or the banner form when
    ///   both are defined for a token (e.g., `b"SECRET"` and `b"S"`
    ///   should both admit for a US-classification category whose
    ///   tokens have distinct portion / banner forms).
    ///
    /// - **Open-vocabulary categories** (e.g., FGI/REL-TO country
    ///   trigraphs, SAR program identifiers, SCI compartments and
    ///   sub-compartments where the vocabulary allows agency-specific
    ///   extensions): admission is by *generative shape rule* —
    ///   character class (ASCII alpha vs. alphanumeric), length
    ///   bounds, and any required prefix/suffix structure. The shape
    ///   rule MUST trace to an authoritative passage in the scheme's
    ///   primary source (Constitution Principle VIII).
    ///
    /// Implementations are total over `(CategoryId, &[u8])`: an
    /// unknown `category` MUST return `false` rather than panicking,
    /// so callers (notably parser admission sites — see FR-015 of
    /// `specs/006-engine-rule-refactor/spec.md`) can route every
    /// open-vocabulary slot through this method without category
    /// existence checks. The empty byte slice MUST return `false`
    /// for every category — no token has zero length.
    ///
    /// # Performance contract
    ///
    /// `Arc<dyn Vocabulary<S>>` precludes cross-crate
    /// devirtualization (FR-030), so implementations sit on the
    /// parser hot path. Implementations MUST avoid heap allocation,
    /// regex compilation, and UTF-8 decoding overhead beyond what
    /// is required to identify the byte class. ASCII byte
    /// comparisons are the expected operation set; anything richer
    /// requires measurement against `SC-001`.
    fn shape_admits(&self, category: CategoryId, bytes: &[u8]) -> bool;
}
