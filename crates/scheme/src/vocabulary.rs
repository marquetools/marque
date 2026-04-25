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
//! - Active tokens populate every non-`Option` field.
//! - Deprecated tokens additionally populate `deprecation`.
//! - The FOUO → CUI migration is absent from the migration table
//!   (FR-020): FOUO remains an active valid dissemination control.

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
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Deprecation<Token> {
    /// Schema version at which the token was deprecated, e.g.,
    /// `"ISM-v2022-DEC"`.
    pub since: &'static str,
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

    /// Canonical portion form for `token`, e.g., `"(S)"`.
    fn portion_form(&self, token: &S::Token) -> &'static str;

    /// Canonical banner form for `token`, e.g., `"SECRET"`.
    fn banner_form(&self, token: &S::Token) -> &'static str;

    /// Banner abbreviation when defined, else `None`.
    fn banner_abbreviation(&self, token: &S::Token) -> Option<&'static str>;

    /// Full metadata record for `token`.
    ///
    /// Preferred accessor — individual per-field methods exist so
    /// call sites can avoid pulling the entire struct when they only
    /// need one field, but the full record is the authoritative
    /// view.
    fn metadata(&self, token: &S::Token) -> &'static TokenMetadataFull<S::Token>;
}
