// SPDX-FileCopyrightText: 2026 Knitli Inc.
//
// SPDX-License-Identifier: LicenseRef-MarqueLicense-1.0

//! Typed citation surface for diagnostics.
//!
//! Landed in PR 3c.2.A per `docs/plans/2026-05-19-pr3c2-a-pm-decisions.md`
//! PM-5 / PM-7 as a definition-only scaffolding step. PR 3c.2.C
//! (this PR) migrates `Diagnostic.citation: &'static str` to
//! `Diagnostic.citation: Citation` and adds the [`capco`] /
//! [`capco_table`] const-fn ergonomic constructors plus
//! [`AuthoritativeSource::Config`] / [`AuthoritativeSource::EngineInternal`]
//! sentinel variants for non-CAPCO citations (corrections-map and
//! engine-synthetic R002).
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
/// (`§<Letter>[.<subsection>] [Table <table>] p<page>`) for CAPCO
/// citations and a bare `[<source>]` tag for non-CAPCO sentinels
/// ([`AuthoritativeSource::Config`], [`AuthoritativeSource::EngineInternal`]).
///
/// # No `From<&str> for Citation` impl
///
/// `Citation` has no string-coercion constructor by design — the
/// closed-template / typed-citation discipline of PR 3c.2.C (per
/// `docs/plans/2026-05-20-pr3c2-c-pm-decisions.md` PM-C-10) requires
/// every citation to flow through [`Citation::new`] or one of the
/// ergonomic const-fn helpers ([`capco`], [`capco_table`]) so the
/// content is statically structured.
///
/// **No `From<&str> for Citation` impl.**
///
/// ```compile_fail
/// use marque_rules::Citation;
/// let _: Citation = "CAPCO-2016 §H.4 p61".into();
/// ```
///
/// **No `From<String> for Citation` impl.**
///
/// ```compile_fail
/// use marque_rules::Citation;
/// let _: Citation = String::from("CAPCO-2016 §H.4 p61").into();
/// ```
///
/// **No `Citation::from_str` method.**
///
/// ```compile_fail
/// use marque_rules::Citation;
/// let _ = Citation::from_str("CAPCO-2016 §H.4 p61");
/// ```
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
        // CAPCO citations render with the bare `§<L>[.sub] [Table N]
        // p<page>` shape — the canonical citation-lint regex form.
        // Non-CAPCO sentinel variants render as `[<source>]` only,
        // dropping the §/page suffix entirely. That keeps
        // citation-lint a no-op for sentinel citations (no `§` to
        // scan) and avoids tripping the resolver with meaningless
        // section/page values per PR 3c.2.C PM-C-4.
        match self.document {
            AuthoritativeSource::Capco2016 => {
                write!(f, "§{}", self.section.letter.as_letter())?;
                if let Some(sub) = self.section.subsection {
                    write!(f, ".{}", sub.get())?;
                }
                if let Some(table) = self.section.table {
                    write!(f, " Table {}", table.get())?;
                }
                write!(f, " p{}", self.page.get())?;
            }
            AuthoritativeSource::Config => {
                write!(f, "[config]")?;
            }
            AuthoritativeSource::EngineInternal => {
                write!(f, "[engine-internal]")?;
            }
        }
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
/// Bare `§<L>` (no subsection) is representable as `subsection = None`.
///
/// Construction is builder-style: start with [`SectionRef::new`], then
/// chain `with_subsection` / `with_table` to add the optional fields.
///
/// # Sub-subsections deliberately omitted
///
/// CAPCO-2016 has no `§<L>.<sub>.<sub_sub>`-style citations in the
/// manual — every citation is either bare `§<L>`, subsection
/// `§<L>.<sub>`, or subsection + table `§<L>.<sub> Table <N>`. A
/// `sub_subsection` field would be dead capability per YAGNI
/// (architect-reviewer F-5 on PR #627). If a future authoritative
/// source introduces 3-level subsections, this shape re-extends
/// additively via `#[non_exhaustive]`.
#[non_exhaustive]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct SectionRef {
    /// The top-level section letter (`A`–`H` per CAPCO-2016 normative
    /// range; see [`SectionLetter`]).
    pub letter: SectionLetter,
    /// Subsection number — e.g., `§H.5` → `subsection = Some(5)`.
    /// `None` for a bare `§<L>`-style reference.
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

    /// Add a subsection number (e.g., `§<L>` → `§H.5` via
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
/// `project_capco_doc_structure` (sections `A`–`H` are normative;
/// sections `I`–`K` cover history / examples / acronyms and are NOT
/// valid citation targets). The `#[non_exhaustive]` marker reserves
/// grow-path for future grammars whose section vocabulary differs
/// (CUI, NATO).
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
///
/// # Non-CAPCO sentinel variants
///
/// [`Self::Config`] and [`Self::EngineInternal`] are sentinel sources
/// for citations that do NOT reference an authoritative published
/// manual. They cover two specific engine-internal citation slots:
///
/// - [`Self::Config`] — the user's `.marque.toml` `[corrections]`
///   table (`CORRECTIONS_MAP_CITATION`); not a CAPCO citation, not
///   citation-lint-resolvable.
/// - [`Self::EngineInternal`] — the engine-synthesized R002 re-parse
///   failure diagnostic (`R002_CITATION`); diagnostic provenance is
///   the engine itself, not a CAPCO passage.
///
/// The [`Citation::Display`] impl renders these sentinels as a bare
/// `[<source>]` tag with NO `§<L>.<sub> p<page>` suffix — the
/// section/page fields carry niche-saving sentinel values
/// (`SectionLetter::A` + page `1`) that are never displayed. This
/// design choice keeps citation-lint a no-op for sentinel citations:
/// with no `§` substring to scan, the resolver doesn't try to
/// validate a meaningless section/page against the CAPCO index.
#[non_exhaustive]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum AuthoritativeSource {
    /// CAPCO-2016 Implementation Guide — the marque-vendored manual at
    /// `crates/capco/docs/CAPCO-2016.md` (PDF original at
    /// `crates/capco/docs/original-refs/CAPCO-2016.pdf`).
    Capco2016,
    /// User configuration source — the `.marque.toml` `[corrections]`
    /// table. Used by the corrections-map sentinel citation
    /// (`marque_rules::CORRECTIONS_MAP_CITATION`). Not a CAPCO
    /// citation, so the [`Citation::Display`] impl renders this as
    /// `[config]` with no §/page suffix.
    Config,
    /// Engine-synthesized diagnostic source — used by the R002
    /// re-parse-failure diagnostic, which is produced by the engine
    /// itself (no CAPCO passage governs it). The [`Citation::Display`]
    /// impl renders this as `[engine-internal]` with no §/page suffix.
    EngineInternal,
}

// ---------------------------------------------------------------------------
// Ergonomic const-fn constructors (PR 3c.2.C PM-C-2)
// ---------------------------------------------------------------------------

/// Const-fn ergonomic constructor for CAPCO-2016 citations.
///
/// Use this in catalog rows, `static` constants, and `const fn` bodies
/// to construct [`Citation`] values without the
/// `Citation::new(AuthoritativeSource::Capco2016,
/// SectionRef::new(SectionLetter::H).with_subsection(NonZeroU8::new(4).unwrap()),
/// NonZeroU16::new(61).unwrap())` boilerplate.
///
/// `page` and `subsection` must be non-zero — `0` arguments panic at
/// const evaluation (compile error). The const-fn `match`-based panic
/// shape is safe (no `unsafe` block) and gives a compile-time
/// guarantee that no invalid sentinel-zero `Citation` can be
/// constructed.
///
/// # Examples
///
/// ```
/// use marque_rules::{capco, Citation, SectionLetter};
/// const SCI_GRAMMAR: Citation = capco(SectionLetter::H, 4, 61);
/// assert_eq!(format!("{SCI_GRAMMAR}"), "§H.4 p61");
/// ```
pub const fn capco(letter: SectionLetter, subsection: u8, page: u16) -> Citation {
    let subsection = match NonZeroU8::new(subsection) {
        Some(n) => n,
        None => panic!("capco(): subsection must be non-zero"),
    };
    let page = match NonZeroU16::new(page) {
        Some(n) => n,
        None => panic!("capco(): page must be non-zero"),
    };
    Citation::new(
        AuthoritativeSource::Capco2016,
        SectionRef::new(letter).with_subsection(subsection),
        page,
    )
}

/// Const-fn ergonomic constructor for CAPCO-2016 citations that
/// include a Table reference (e.g., `§B.3 Table 2 p21`).
///
/// All three numeric arguments must be non-zero — `0` arguments panic
/// at const evaluation.
///
/// # Examples
///
/// ```
/// use marque_rules::{capco_table, Citation, SectionLetter};
/// const CAVEATED_FDR: Citation = capco_table(SectionLetter::B, 3, 2, 21);
/// assert_eq!(format!("{CAVEATED_FDR}"), "§B.3 Table 2 p21");
/// ```
pub const fn capco_table(
    letter: SectionLetter,
    subsection: u8,
    table: u8,
    page: u16,
) -> Citation {
    let subsection = match NonZeroU8::new(subsection) {
        Some(n) => n,
        None => panic!("capco_table(): subsection must be non-zero"),
    };
    let table = match NonZeroU8::new(table) {
        Some(n) => n,
        None => panic!("capco_table(): table must be non-zero"),
    };
    let page = match NonZeroU16::new(page) {
        Some(n) => n,
        None => panic!("capco_table(): page must be non-zero"),
    };
    Citation::new(
        AuthoritativeSource::Capco2016,
        SectionRef::new(letter)
            .with_subsection(subsection)
            .with_table(table),
        page,
    )
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

    #[test]
    fn capco_helper_constructs_subsection_citation() {
        let c = capco(SectionLetter::H, 4, 61);
        assert_eq!(c.document, AuthoritativeSource::Capco2016);
        assert_eq!(c.section.letter, SectionLetter::H);
        assert_eq!(c.section.subsection.unwrap().get(), 4);
        assert!(c.section.table.is_none());
        assert_eq!(c.page.get(), 61);
        assert_eq!(format!("{c}"), "§H.4 p61");
    }

    #[test]
    fn capco_table_helper_constructs_table_citation() {
        let c = capco_table(SectionLetter::B, 3, 2, 21);
        assert_eq!(c.document, AuthoritativeSource::Capco2016);
        assert_eq!(c.section.letter, SectionLetter::B);
        assert_eq!(c.section.subsection.unwrap().get(), 3);
        assert_eq!(c.section.table.unwrap().get(), 2);
        assert_eq!(c.page.get(), 21);
        assert_eq!(format!("{c}"), "§B.3 Table 2 p21");
    }

    #[test]
    fn config_source_renders_as_bracketed_tag() {
        // [`AuthoritativeSource::Config`] sentinel — the §/page fields
        // hold niche-sentinel values that Display deliberately omits.
        let c = Citation::new(
            AuthoritativeSource::Config,
            SectionRef::new(SectionLetter::A),
            NonZeroU16::new(1).unwrap(),
        );
        assert_eq!(format!("{c}"), "[config]");
    }

    #[test]
    fn engine_internal_source_renders_as_bracketed_tag() {
        let c = Citation::new(
            AuthoritativeSource::EngineInternal,
            SectionRef::new(SectionLetter::A),
            NonZeroU16::new(1).unwrap(),
        );
        assert_eq!(format!("{c}"), "[engine-internal]");
    }

    // Compile-time pins for the const-fn helpers — both must evaluate
    // in const context. If a constructor in the call chain stops being
    // const-fn, these fail at compile time.
    const _C2_HELPER_SCI: Citation = capco(SectionLetter::H, 4, 61);
    const _C2_HELPER_TABLE: Citation = capco_table(SectionLetter::B, 3, 2, 21);
}
