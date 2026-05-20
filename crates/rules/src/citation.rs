// SPDX-FileCopyrightText: 2026 Knitli Inc.
//
// SPDX-License-Identifier: LicenseRef-MarqueLicense-1.0

//! Typed citation surface for diagnostics.
//!
//! Lands in PR 3c.2.A per `docs/plans/2026-05-19-pr3c2-a-pm-decisions.md`
//! PM-5 / PM-7 as a definition-only scaffolding step. `Diagnostic.citation:
//! &'static str` migrates to `Diagnostic.citation: Citation` at PR
//! 3c.2.C; the pre-migration `&'static str` field continues to carry
//! string literals at PR 3c.2.A.
//!
//! # Why a typed citation surface
//!
//! - Citation-lint at `tools/citation-lint/` currently parses `&'static
//!   str` literal citations in source. A typed surface lets the lint
//!   check a structured value rather than re-deriving structure from a
//!   regex, and gives every diagnostic a machine-readable §-reference.
//! - Constitution Principle VIII: "Citation integrity is non-
//!   negotiable. Every citation embedded in a rule, diagnostic message,
//!   doc comment, plan, or docs file MUST (a) refer to a real passage
//!   in the authoritative source, (b) accurately reflect what that
//!   passage says, and (c) be re-verifiable by any reviewer with the
//!   source in hand." A typed citation makes (c) mechanical.
//! - The `Display` impl emits the citation-lint regex form
//!   (`§<Letter>[.<subsection>] [Table <table>] p<page>` — see
//!   `tools/citation-lint/src/scanner.rs`). PR 3c.2.C's
//!   `&'static str → Citation` migration is structurally compatible;
//!   the lint's `Table <N>` structural-parse extension (today the
//!   lint tolerates the substring as an occurrence anchor without
//!   parsing it into its own field) lands alongside C per Copilot
//!   inline review on PR #627.
//!
//! # Const-fn construction
//!
//! Every constructor is `const fn`. No runtime validation in the
//! constructors per D25.2 in
//! `docs/plans/2026-05-19-pr3c2-plan-and-decisions.md` — citation-lint
//! at CI time catches drift, the threat model for runtime validation
//! is purely citation drift (stale §, wrong page after a source
//! revision), and runtime code would ship to WASM unnecessarily.

use core::fmt;
use core::num::{NonZeroU8, NonZeroU16};

/// A typed citation to an authoritative source passage.
///
/// `Display` emits the canonical citation-lint regex form
/// (`§<Letter>[.<subsection>] [Table <table>] p<page>`). `document` is
/// NOT rendered today because CAPCO-2016 is the only
/// `AuthoritativeSource` variant; add a `[<doc-tag>]`-style prefix when
/// a second variant lands.
///
/// # Examples
///
/// ```
/// use core::num::{NonZeroU8, NonZeroU16};
/// use marque_rules::{AuthoritativeSource, Citation, SectionLetter, SectionRef};
///
/// // §H.4 p61 — SCI grammar.
/// const SCI_GRAMMAR: Citation = Citation::new(
///     AuthoritativeSource::Capco2016,
///     SectionRef::new(SectionLetter::H).with_subsection(NonZeroU8::new(4).unwrap()),
///     NonZeroU16::new(61).unwrap(),
/// );
/// assert_eq!(format!("{SCI_GRAMMAR}"), "§H.4 p61");
///
/// // §B.3 Table 2 p21 — caveated FD&R rule.
/// const CAVEATED: Citation = Citation::new(
///     AuthoritativeSource::Capco2016,
///     SectionRef::new(SectionLetter::B)
///         .with_subsection(NonZeroU8::new(3).unwrap())
///         .with_table(NonZeroU8::new(2).unwrap()),
///     NonZeroU16::new(21).unwrap(),
/// );
/// assert_eq!(format!("{CAVEATED}"), "§B.3 Table 2 p21");
/// ```
#[non_exhaustive]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Citation {
    /// The authoritative document.
    pub document: AuthoritativeSource,
    /// The §-reference within the document.
    pub section: SectionRef,
    /// The page number.
    pub page: PageNumber,
}

impl Citation {
    /// Const-fn constructor. No runtime validation per D25.2 in
    /// `docs/plans/2026-05-19-pr3c2-plan-and-decisions.md` —
    /// citation-lint at CI time catches drift.
    pub const fn new(document: AuthoritativeSource, section: SectionRef, page: PageNumber) -> Self {
        Self {
            document,
            section,
            page,
        }
    }
}

impl fmt::Display for Citation {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        // `document` is NOT rendered while CAPCO-2016 is the sole
        // AuthoritativeSource variant. Adding a second variant will
        // require a `[CAPCO-2016]`-style prefix here so downstream
        // consumers can disambiguate; the citation-lint regex shape at
        // `tools/citation-lint/src/scanner.rs` does not currently
        // accept a document prefix, so the prefix lands alongside the
        // citation-lint update when the second variant lands.
        write!(f, "§{}", self.section.letter.as_letter())?;
        if let Some(sub) = self.section.subsection {
            write!(f, ".{}", sub.get())?;
        }
        if let Some(table) = self.section.table {
            write!(f, " Table {}", table.get())?;
        }
        write!(f, " p{}", self.page.get())?;
        Ok(())
    }
}

/// Structured §-reference within an authoritative source.
///
/// Accommodates the two CAPCO citation shapes verified against
/// `crates/capco/docs/CAPCO-2016.md` at PR 3c.2.A authorship:
///
/// - `§<L>.<sub>` (e.g., `§H.4 p61` — SCI grammar; CAPCO-2016 §H.4 p61
///   verified at PR 3c.2.A authorship)
/// - `§<L>.<sub> Table <N>` (e.g., `§B.3 Table 2 p21` — caveated FD&R
///   rule; CAPCO-2016 §B.3 Table 2 p21 verified at PR 3c.2.A authorship
///   per project memory `project_capco_p20_caveated_definition`)
///
/// The `Option<NonZeroU8>` choice for `subsection` and `table` niche-
/// saves the `Option<u8>` tail and statically rejects sentinel-zero.
/// Bare `§H` (no subsection) is representable as `subsection = None`.
///
/// Construction is builder-style: start with [`SectionRef::new`], then
/// chain `with_subsection` / `with_table` to add the optional fields.
///
/// # Sub-subsections deliberately omitted
///
/// CAPCO-2016 has no `§X.Y.Z`-style citations in the manual — every
/// citation is either bare `§X`, subsection `§X.Y`, or subsection +
/// table `§X.Y Table N`. A `sub_subsection` field would be dead
/// capability per YAGNI (architect-reviewer F-5 on PR #627). If a
/// future authoritative source introduces 3-level subsections, this
/// shape re-extends additively via `#[non_exhaustive]`.
#[non_exhaustive]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct SectionRef {
    /// The top-level section letter (`A`–`H` per CAPCO-2016 normative
    /// range; see [`SectionLetter`]).
    pub letter: SectionLetter,
    /// Subsection number — e.g., `§H.5` → `subsection = Some(5)`.
    /// `None` for a bare `§H`-style reference.
    pub subsection: Option<NonZeroU8>,
    /// Table number — e.g., `§B.3 Table 2` → `table = Some(2)`. Always
    /// paired with a populated [`Self::subsection`] in practice (CAPCO
    /// tables live inside subsections).
    pub table: Option<NonZeroU8>,
}

impl SectionRef {
    /// Construct a bare `§<L>`-style reference. Chain
    /// `with_subsection` / `with_table` to add optional components.
    pub const fn new(letter: SectionLetter) -> Self {
        Self {
            letter,
            subsection: None,
            table: None,
        }
    }

    /// Add a subsection number (e.g., `§H` → `§H.5` via
    /// `.with_subsection(NonZeroU8::new(5).unwrap())`).
    pub const fn with_subsection(self, subsection: NonZeroU8) -> Self {
        Self {
            subsection: Some(subsection),
            ..self
        }
    }

    /// Add a table number (e.g., `§B.3` → `§B.3 Table 2` via
    /// `.with_table(NonZeroU8::new(2).unwrap())`).
    pub const fn with_table(self, table: NonZeroU8) -> Self {
        Self {
            table: Some(table),
            ..self
        }
    }
}

/// CAPCO-2016 normative section letters.
///
/// Restricted to `A`–`H` per project memory
/// `project_capco_doc_structure` ("§A–H normative; §I–K
/// (history/examples/acronyms) NOT valid citation targets"). The
/// `#[non_exhaustive]` marker reserves grow-path for future grammars
/// whose section vocabulary differs (CUI, NATO).
#[non_exhaustive]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum SectionLetter {
    A,
    B,
    C,
    D,
    E,
    F,
    G,
    H,
}

impl SectionLetter {
    /// Map to the literal section letter character.
    pub const fn as_letter(self) -> char {
        match self {
            Self::A => 'A',
            Self::B => 'B',
            Self::C => 'C',
            Self::D => 'D',
            Self::E => 'E',
            Self::F => 'F',
            Self::G => 'G',
            Self::H => 'H',
        }
    }
}

/// Page number — `NonZeroU16` because CAPCO-2016 has ≤200 pages and
/// page-zero is invalid by construction. The niche saves a byte via
/// `Option<Citation>` at compile time.
pub type PageNumber = NonZeroU16;

/// The authoritative source for a citation.
///
/// `#[non_exhaustive]` reserves grow-path for future grammars; the
/// closed-enum shape is Constitution VIII alignment ("Every grammar
/// has a designated primary source").
#[non_exhaustive]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum AuthoritativeSource {
    /// CAPCO-2016 Implementation Guide — the marque-vendored manual at
    /// `crates/capco/docs/CAPCO-2016.md` (PDF original at
    /// `crates/capco/docs/original-refs/CAPCO-2016.pdf`).
    Capco2016,
}

#[cfg(test)]
#[cfg_attr(coverage_nightly, coverage(off))]
mod tests {
    use super::*;
    use static_assertions::assert_impl_all;

    // Send + Sync forward-defense per Constitution VI. Every field is
    // `Copy` so the properties hold by construction today; any future
    // field addition that breaks them trips this compile-time guard.
    // Same posture as `Rule: Send + Sync` (PR 0 T002).
    assert_impl_all!(Citation: Send, Sync, Copy);
    assert_impl_all!(SectionRef: Send, Sync, Copy);
    assert_impl_all!(SectionLetter: Send, Sync, Copy);
    assert_impl_all!(AuthoritativeSource: Send, Sync, Copy);

    // Citation::new MUST be `const fn`. The const evaluation below
    // fails at compile time if any constructor stops being const.
    const _SCI_GRAMMAR: Citation = Citation::new(
        AuthoritativeSource::Capco2016,
        SectionRef::new(SectionLetter::H).with_subsection(NonZeroU8::new(4).unwrap()),
        NonZeroU16::new(61).unwrap(),
    );

    const _CAVEATED: Citation = Citation::new(
        AuthoritativeSource::Capco2016,
        SectionRef::new(SectionLetter::B)
            .with_subsection(NonZeroU8::new(3).unwrap())
            .with_table(NonZeroU8::new(2).unwrap()),
        NonZeroU16::new(21).unwrap(),
    );

    #[test]
    fn citation_is_copy_and_hashable() {
        // Citation flows by value through diagnostic emission, audit
        // tracing, and lookup tables. Copy + Hash + Eq lets all three
        // paths use it without boxing.
        fn assert_copy<T: Copy>() {}
        fn assert_hash<T: core::hash::Hash + Eq>() {}
        assert_copy::<Citation>();
        assert_hash::<Citation>();
        assert_copy::<SectionRef>();
        assert_hash::<SectionRef>();
        assert_copy::<SectionLetter>();
        assert_hash::<SectionLetter>();
        assert_copy::<AuthoritativeSource>();
        assert_hash::<AuthoritativeSource>();
    }

    #[test]
    fn section_letter_maps_to_real_letter() {
        // Sanity: every variant lands on its expected character.
        assert_eq!(SectionLetter::A.as_letter(), 'A');
        assert_eq!(SectionLetter::B.as_letter(), 'B');
        assert_eq!(SectionLetter::C.as_letter(), 'C');
        assert_eq!(SectionLetter::D.as_letter(), 'D');
        assert_eq!(SectionLetter::E.as_letter(), 'E');
        assert_eq!(SectionLetter::F.as_letter(), 'F');
        assert_eq!(SectionLetter::G.as_letter(), 'G');
        assert_eq!(SectionLetter::H.as_letter(), 'H');
    }
}
