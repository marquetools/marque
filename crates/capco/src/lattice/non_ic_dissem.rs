// SPDX-FileCopyrightText: 2026 Knitli Inc.
//
// SPDX-License-Identifier: LicenseRef-MarqueLicense-1.0

//! [`NonIcDissemSet`] — lattice over the non-IC dissem axis with
//! classification-gated SBU-NF / LES-NF split + NODIS / EXDIS
//! NF-injection.

use marque_ism::{CanonicalAttrs, Classification, NonIcDissem};
use std::collections::BTreeSet;

// ---------------------------------------------------------------------------
// NonIcDissemSet — lattice over the non-IC dissem axis with
// classification-gated SBU-NF / LES-NF split + NODIS / EXDIS NF-injection.
// ---------------------------------------------------------------------------

/// Lattice form of the non-IC dissem axis.
///
/// Carries the union of per-portion `non_ic_dissem` tokens after
/// the classification-independent compound-supersedes-bare overlay
/// (§H.9 p178 / p185 — see #552 below), the classification-gated
/// SBU-NF / LES-NF transformations (§H.9 p178 / p185 — see #541),
/// and the NODIS / EXDIS NF-injection flag (§H.9 p172 / p174).
///
/// # Same-axis compound supersession (#552)
///
/// Before any classification gate fires, two co-presence rules apply
/// regardless of classification level:
///
/// - `{Sbu, SbuNf} ⊆ set` → drop `Sbu`. **§H.9 p178** (SBU NOFORN
///   Precedence Rules for Banner Line Guidance): *"When a document
///   contains both SBU-NF and SBU portions, SBU NOFORN supersedes
///   SBU in the banner line."*
/// - `{Les, LesNf} ⊆ set` → drop `Les`. **§H.9 p185** + canonical
///   banner-form examples: portion `(U//LES-NF)` rolls up to banner
///   `UNCLASSIFIED//LES NOFORN`; the LES-NF compound carries the LES
///   family marker in unclassified banner form, so bare LES is
///   redundant when LES-NF is also present.
///
/// The supersession runs BEFORE the classified gate so the
/// post-supersession set is what the gate sees. Net result for the
/// four U/S × SBU/LES quadrants:
///
/// | Input | Unclassified | Classified |
/// |---|---|---|
/// | `{Sbu, SbuNf}` | `{SbuNf}` | `{}` + `needs_nf` |
/// | `{Les, LesNf}` | `{LesNf}` | `{Les}` + `needs_nf` |
///
/// `needs_nf` is set when:
/// - SBU-NF appears on a classified page (§H.9 p178 — SBU vanishes
///   from the set; only `needs_nf` is asserted — see asymmetry note
///   below), OR
/// - LES-NF appears on a classified page (§H.9 p185 — `Les` is
///   inserted into the set AND `needs_nf` is asserted), OR
/// - Any portion carries NODIS or EXDIS (classification-independent
///   per §H.9 p172 / p174 — the manual does not gate the NF injection
///   on classification level for these tokens).
///
/// # SBU-NF / LES-NF classified-context asymmetry (the §H.9 p178 vs
/// §H.9 p185 difference)
///
/// On classified pages the two compound-NF non-IC dissem tokens
/// behave OPPOSITELY:
///
/// - **SBU-NF**: the bare SBU vanishes entirely from the output set;
///   only NOFORN is injected via `needs_nf`. **§H.9 p178** (SBU NOFORN
///   Commingling Rule(s) Within a Portion): *"If the portion is
///   classified, the classification level of the portion adequately
///   protects the SBU information, so SBU is not reflected in the
///   portion mark; however a NOFORN marking must be added to the
///   portion mark, e.g., (C//NF)."* The classification level subsumes
///   SBU's role as administrative-protection marker.
///
/// - **LES-NF**: the bare LES is RETAINED in the output set; NOFORN
///   is injected via `needs_nf` in parallel. **§H.9 p185** (LES NOFORN
///   Precedence Rules for Banner Line Guidance): *"The LES marking
///   always appears in the banner line if LES information (either LES
///   or LES NOFORN) is contained in the document, regardless of the
///   document's classification level. When a classified document
///   contains portions of U//LES-NF, the 'LES' marking is used in the
///   banner line and the NOFORN marking is applied as a Dissemination
///   Control Marking. For example: SECRET//NOFORN//LES."* LES carries
///   independent regulatory discipline (law-enforcement legal-process
///   restrictions per §H.9 p182 LES Warning Statement, originator-
///   control discipline per §H.9 p186 Notes — and the
///   `SECRET//NOFORN//LES` worked example at §H.9 p184 Notional Example
///   Page 4) that classification does NOT subsume — hence the
///   asymmetry with SBU.
///
/// **`Default`** is the bottom: empty set, `needs_nf = false`.
///
/// **Projection helper, NOT a `JoinSemilattice`.** Earlier review
/// passes flagged the missing trait impl (rust-reviewer H-3); the
/// lattice-consultant verdict was that the missing impl is the
/// architecturally correct shape, not a gap. The classified-context
/// SBU-NF / LES-NF transformations are gated on the page-level
/// `is_classified` predicate, which depends on the OUTER
/// classification axis being known. A pure per-axis `join` cannot
/// read the classification axis; implementing the trait would
/// silently produce wrong output on any cross-axis composition path.
/// Production consumers use [`Self::from_attrs_iter`] directly. See
/// [`super::declass_exemption::DeclassExemptionAccumulator`] (which
/// retired its `JoinSemilattice` impl in PR 4b-E review for the dual
/// reason: a commutativity violation) for the same precedent. The
/// structural template is **"don't claim a trait when the laws can't
/// hold."**
///
/// §-authority (verified 2026-05-18 against
/// `crates/capco/docs/CAPCO-2016.md`):
/// - §H.9 p172 (EXDIS — REL TO not authorized in banner; NOFORN
///   conveys).
/// - §H.9 p174 (NODIS — same).
/// - §H.9 p178 (SBU-NF — SBU vanishes on classified; NOFORN added).
/// - §H.9 p185 (LES-NF — LES retained on classified; NOFORN added).
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct NonIcDissemSet {
    set: BTreeSet<NonIcDissem>,
    needs_nf: bool,
}

impl NonIcDissemSet {
    /// An empty non-IC dissem set — the lattice bottom.
    pub fn empty() -> Self {
        Self::default()
    }

    /// Construct from a slice of `CanonicalAttrs`. Applies the
    /// classification-gated SBU-NF / LES-NF split (the page is
    /// considered classified if any portion's classification is above
    /// `Unclassified`) and the unconditional NODIS / EXDIS
    /// NF-injection flag.
    ///
    /// Mirrors `PageContext::expected_non_ic_dissem`'s shape exactly,
    /// returning `(set, needs_nf)` via `into_inner_with_needs_nf`.
    pub fn from_attrs_iter(portions: &[CanonicalAttrs]) -> Self {
        // Classification gate: any portion above Unclassified makes
        // the page "classified" for the SBU-NF / LES-NF split.
        let classified = portions.iter().any(|a| {
            a.classification
                .as_ref()
                .is_some_and(|c| c.effective_level() > Classification::Unclassified)
        });

        let mut set: BTreeSet<NonIcDissem> = BTreeSet::new();
        for attrs in portions {
            for d in attrs.non_ic_dissem.iter() {
                set.insert(*d);
            }
        }

        // #552 — Classification-independent same-axis supersession:
        // compound NOFORN-bearing token dominates its bare sibling.
        // Applied BEFORE the classification-gated #541 transformations
        // so the post-supersession set is what feeds the classified
        // strip/split below.
        //
        // §H.9 p178 (SBU NOFORN Precedence Rules for Banner Line
        // Guidance): "When a document contains both SBU-NF and SBU
        // portions, SBU NOFORN supersedes SBU in the banner line."
        // Drop bare SBU; keep SBU-NF. At classified the existing #541
        // strip then removes SBU-NF entirely, leaving `{}` + needs_nf
        // (banner `SECRET//NOFORN`). At unclassified the SBU-NF
        // survives (banner `UNCLASSIFIED//SBU NOFORN`).
        if set.contains(&NonIcDissem::SbuNf) {
            set.remove(&NonIcDissem::Sbu);
        }
        // §H.9 p185 (LES NOFORN — banner-form heading + Notional
        // Example Page 1): the banner for `(U//LES-NF)` portions is
        // `UNCLASSIFIED//LES NOFORN`, i.e. the LES-NF compound carries
        // the LES family marker in unclassified banner form. With both
        // `Les` and `LesNf` portions present, LES-NF dominates bare
        // LES on the unclassified banner. The existing #541 classified
        // split then transforms `{LesNf}` → `{Les}` + needs_nf at
        // classified, yielding `SECRET//NOFORN//LES` per §H.9 p185
        // (LES NOFORN Precedence Rules for Banner Line Guidance).
        if set.contains(&NonIcDissem::LesNf) {
            set.remove(&NonIcDissem::Les);
        }

        let mut needs_nf = false;
        if classified {
            // §H.9 p178 (SBU NOFORN Commingling Rule(s) Within a
            // Portion): "If the portion is classified, the
            // classification level of the portion adequately protects
            // the SBU information, so SBU is not reflected in the
            // portion mark; however a NOFORN marking must be added to
            // the portion mark, e.g., (C//NF)." SBU vanishes entirely;
            // NOFORN injection happens via `needs_nf`. Asymmetric with
            // the LES-NF branch immediately below (LES survives) —
            // see the type-level doc-comment for the regulatory
            // rationale. #541. Re-verified 2026-05-18 against
            // `crates/capco/docs/CAPCO-2016.md`.
            if set.remove(&NonIcDissem::SbuNf) {
                needs_nf = true;
            }
            // §H.9 p185 (LES NOFORN Precedence Rules for Banner Line
            // Guidance): "The LES marking always
            // appears in the banner line if LES information (either
            // LES or LES NOFORN) is contained in the document,
            // regardless of the document's classification level. When
            // a classified document contains portions of U//LES-NF,
            // the 'LES' marking is used in the banner line and the
            // NOFORN marking is applied as a Dissemination Control
            // Marking. For example: SECRET//NOFORN//LES." LES is
            // RETAINED in the output set (asymmetric with SBU above);
            // NOFORN injection happens via `needs_nf` in parallel.
            // Re-verified 2026-05-18 against
            // `crates/capco/docs/CAPCO-2016.md`.
            if set.remove(&NonIcDissem::LesNf) {
                set.insert(NonIcDissem::Les);
                needs_nf = true;
            }
        }

        // §H.9 p172 (EXDIS) / p174 (NODIS): NF must be injected into
        // the dissem block regardless of classification level. NODIS
        // / EXDIS themselves stay in the non-IC set.
        if set.contains(&NonIcDissem::Nodis) || set.contains(&NonIcDissem::Exdis) {
            needs_nf = true;
        }

        Self { set, needs_nf }
    }

    /// Whether NOFORN must be injected into the dissem block at
    /// banner roll-up.
    pub fn needs_nf(&self) -> bool {
        self.needs_nf
    }

    /// Borrow the underlying set.
    pub fn as_set(&self) -> &BTreeSet<NonIcDissem> {
        &self.set
    }

    /// Render to a `Box<[NonIcDissem]>` in BTreeSet natural order.
    pub fn into_boxed_slice(self) -> Box<[NonIcDissem]> {
        self.set.into_iter().collect::<Vec<_>>().into_boxed_slice()
    }

    /// Consume into `(set, needs_nf)` to match
    /// `PageContext::expected_non_ic_dissem`'s tuple shape.
    pub fn into_inner_with_needs_nf(self) -> (Vec<NonIcDissem>, bool) {
        (self.set.into_iter().collect(), self.needs_nf)
    }

    /// Render to a `Vec<NonIcDissem>` for compatibility.
    pub fn to_vec(&self) -> Vec<NonIcDissem> {
        self.set.iter().copied().collect()
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
#[cfg_attr(coverage_nightly, coverage(off))]
mod tests {
    use super::*;
    use crate::lattice::test_support::*;

    #[test]
    fn non_ic_dissem_set_default_is_empty_bottom() {
        let s = NonIcDissemSet::default();
        assert!(s.as_set().is_empty());
        assert!(!s.needs_nf());
    }

    #[test]
    fn non_ic_dissem_set_empty_equals_default() {
        assert_eq!(NonIcDissemSet::empty(), NonIcDissemSet::default());
    }

    #[test]
    fn non_ic_dissem_set_sbu_nf_drops_sbu_on_classified() {
        // §H.9 p178 (Commingling Rule(s) Within a Portion): "If the
        // portion is classified, the classification level of the
        // portion adequately protects the SBU information, so SBU is
        // not reflected in the portion mark; however a NOFORN marking
        // must be added to the portion mark, e.g., (C//NF)." SBU
        // vanishes entirely; only NOFORN survives via `needs_nf`.
        // #541.
        let mut p = portion_us(Classification::Secret);
        p.non_ic_dissem = Box::new([NonIcDissem::SbuNf]);
        let s = NonIcDissemSet::from_attrs_iter(&[p]);
        assert!(
            !s.as_set().contains(&NonIcDissem::Sbu),
            "§H.9 p178: SBU is not reflected on classified portion; \
             set must NOT contain Sbu after SBU-NF strip. set = {:?}",
            s.as_set(),
        );
        assert!(
            !s.as_set().contains(&NonIcDissem::SbuNf),
            "SBU-NF must be removed from the set (it's transformed \
             into NOFORN-via-needs_nf). set = {:?}",
            s.as_set(),
        );
        assert!(
            s.needs_nf(),
            "§H.9 p178: NOFORN must be added to the portion mark for \
             classified-context SBU-NF. needs_nf = {}",
            s.needs_nf(),
        );
    }

    #[test]
    fn non_ic_dissem_set_sbu_nf_kept_on_unclassified() {
        // §H.9 p178 (canonical unclassified form): SBU-NF on
        // unclassified pages survives verbatim — banner
        // `UNCLASSIFIED//SBU NOFORN`, portion `(U//SBU-NF)`. No
        // transformation. Symmetric with the LES-NF unclassified
        // case immediately below.
        let mut p = portion_us(Classification::Unclassified);
        p.non_ic_dissem = Box::new([NonIcDissem::SbuNf]);
        let s = NonIcDissemSet::from_attrs_iter(&[p]);
        assert!(
            s.as_set().contains(&NonIcDissem::SbuNf),
            "§H.9 p178 (canonical unclassified form): SBU-NF must \
             survive verbatim on unclassified pages. set = {:?}",
            s.as_set(),
        );
        assert!(
            !s.needs_nf(),
            "§H.9 p178: unclassified SBU-NF does not trigger NOFORN \
             injection (NF is encoded in the compound token itself). \
             needs_nf = {}",
            s.needs_nf(),
        );
    }

    #[test]
    fn non_ic_dissem_set_les_nf_splits_on_classified() {
        // §H.9 p185 (LES NOFORN Precedence Rules for Banner Line
        // Guidance): "The LES marking always appears in the banner
        // line if LES information (either LES or LES NOFORN) is
        // contained in the document, regardless of the document's
        // classification level. When a classified document contains
        // portions of U//LES-NF, the 'LES' marking is used in the
        // banner line and the NOFORN marking is applied as a
        // Dissemination Control Marking. For example:
        // SECRET//NOFORN//LES."
        //
        // This is the negative-regression gate for #541's asymmetry:
        // LES MUST survive classification (unlike SBU). LES carries
        // independent regulatory authority (law-enforcement
        // legal-process restrictions, originator-control discipline)
        // that classification does NOT subsume; SBU is purely
        // admin-protection that classification DOES subsume. A
        // future "make it symmetric" change-of-mind must trip this
        // test before it can land.
        let mut p = portion_us(Classification::Secret);
        p.non_ic_dissem = Box::new([NonIcDissem::LesNf]);
        let s = NonIcDissemSet::from_attrs_iter(&[p]);
        assert!(
            s.as_set().contains(&NonIcDissem::Les),
            "§H.9 p185: LES survives on classified pages; set must \
             contain Les after LES-NF split. set = {:?}",
            s.as_set(),
        );
        assert!(
            !s.as_set().contains(&NonIcDissem::LesNf),
            "LES-NF must be removed (transformed into Les + NOFORN). \
             set = {:?}",
            s.as_set(),
        );
        assert!(
            s.needs_nf(),
            "§H.9 p185: NOFORN must be added at banner roll-up. \
             needs_nf = {}",
            s.needs_nf(),
        );
    }

    #[test]
    fn non_ic_dissem_set_les_nf_kept_on_unclassified() {
        // §H.9 p185 (canonical unclassified form): portion form
        // `(U//LES-NF)` retained as-is on unclassified pages.
        // Symmetric with the SBU-NF unclassified case.
        let mut p = portion_us(Classification::Unclassified);
        p.non_ic_dissem = Box::new([NonIcDissem::LesNf]);
        let s = NonIcDissemSet::from_attrs_iter(&[p]);
        assert!(
            s.as_set().contains(&NonIcDissem::LesNf),
            "§H.9 p185 (canonical unclassified form): LES-NF must \
             survive verbatim on unclassified pages. set = {:?}",
            s.as_set(),
        );
        assert!(
            !s.needs_nf(),
            "§H.9 p185: unclassified LES-NF does not trigger NOFORN \
             injection (NF is encoded in the compound token itself). \
             needs_nf = {}",
            s.needs_nf(),
        );
    }

    // -----------------------------------------------------------
    // #552 — same-axis compound-supersedes-bare overlay tests.
    // -----------------------------------------------------------

    #[test]
    fn non_ic_dissem_set_sbu_nf_supersedes_sbu_on_unclassified() {
        // §H.9 p178 (SBU NOFORN Precedence Rules for Banner Line
        // Guidance): "When a document contains both SBU-NF and SBU
        // portions, SBU NOFORN supersedes SBU in the banner line."
        // Net unclassified output: `{SbuNf}` only; banner
        // `UNCLASSIFIED//SBU NOFORN`. #552.
        let mut p_sbu = portion_us(Classification::Unclassified);
        p_sbu.non_ic_dissem = Box::new([NonIcDissem::Sbu]);
        let mut p_sbu_nf = portion_us(Classification::Unclassified);
        p_sbu_nf.non_ic_dissem = Box::new([NonIcDissem::SbuNf]);
        let s = NonIcDissemSet::from_attrs_iter(&[p_sbu, p_sbu_nf]);
        assert!(
            !s.as_set().contains(&NonIcDissem::Sbu),
            "§H.9 p178: SBU-NF supersedes SBU; bare Sbu must be \
             dropped on co-presence. set = {:?}",
            s.as_set(),
        );
        assert!(
            s.as_set().contains(&NonIcDissem::SbuNf),
            "§H.9 p178: compound SBU-NF survives the supersession. \
             set = {:?}",
            s.as_set(),
        );
        assert!(
            !s.needs_nf(),
            "§H.9 p178: unclassified SBU-NF does not trigger NOFORN \
             injection (NF is encoded in the compound token itself). \
             needs_nf = {}",
            s.needs_nf(),
        );
    }

    #[test]
    fn non_ic_dissem_set_les_nf_supersedes_les_on_unclassified() {
        // §H.9 p185 (LES NOFORN — banner-form heading + Notional
        // Example Page 1): banner for `(U//LES-NF)` portions is
        // `UNCLASSIFIED//LES NOFORN`; LES-NF compound carries the
        // LES family marker, so bare LES is redundant on
        // co-presence. Net unclassified output: `{LesNf}` only;
        // banner `UNCLASSIFIED//LES NOFORN`. #552.
        let mut p_les = portion_us(Classification::Unclassified);
        p_les.non_ic_dissem = Box::new([NonIcDissem::Les]);
        let mut p_les_nf = portion_us(Classification::Unclassified);
        p_les_nf.non_ic_dissem = Box::new([NonIcDissem::LesNf]);
        let s = NonIcDissemSet::from_attrs_iter(&[p_les, p_les_nf]);
        assert!(
            !s.as_set().contains(&NonIcDissem::Les),
            "§H.9 p185: LES-NF supersedes LES on co-presence; bare \
             Les must be dropped. set = {:?}",
            s.as_set(),
        );
        assert!(
            s.as_set().contains(&NonIcDissem::LesNf),
            "§H.9 p185: compound LES-NF survives the supersession. \
             set = {:?}",
            s.as_set(),
        );
        assert!(
            !s.needs_nf(),
            "§H.9 p185: unclassified LES-NF does not trigger NOFORN \
             injection (NF is encoded in the compound token itself). \
             needs_nf = {}",
            s.needs_nf(),
        );
    }

    #[test]
    fn non_ic_dissem_set_classified_sbu_and_sbu_nf_strip_to_needs_nf() {
        // #552 + #541 interaction: both bare SBU and compound SBU-NF
        // present on a classified page. Step 1 (#552 supersession)
        // drops bare SBU. Step 2 (#541 classified gate) strips
        // SBU-NF and asserts `needs_nf`. Net: empty set + `needs_nf`
        // → banner `SECRET//NOFORN`. §H.9 p178.
        let mut p_sbu = portion_us(Classification::Secret);
        p_sbu.non_ic_dissem = Box::new([NonIcDissem::Sbu]);
        let mut p_sbu_nf = portion_us(Classification::Secret);
        p_sbu_nf.non_ic_dissem = Box::new([NonIcDissem::SbuNf]);
        let s = NonIcDissemSet::from_attrs_iter(&[p_sbu, p_sbu_nf]);
        assert!(
            s.as_set().is_empty(),
            "§H.9 p178: classified strip after #552 supersession \
             must leave the non-IC set empty. set = {:?}",
            s.as_set(),
        );
        assert!(
            s.needs_nf(),
            "§H.9 p178: NOFORN must be injected on classified \
             SBU-NF strip. needs_nf = {}",
            s.needs_nf(),
        );
    }

    #[test]
    fn non_ic_dissem_set_classified_les_and_les_nf_split_to_les() {
        // #552 + #541 interaction: both bare LES and compound LES-NF
        // present on a classified page. Step 1 (#552 supersession)
        // drops bare LES. Step 2 (#541 classified gate) splits
        // LES-NF → re-inserts bare Les and asserts `needs_nf`. Net:
        // `{Les}` + `needs_nf` → banner `SECRET//NOFORN//LES` per
        // §H.9 p185.
        let mut p_les = portion_us(Classification::Secret);
        p_les.non_ic_dissem = Box::new([NonIcDissem::Les]);
        let mut p_les_nf = portion_us(Classification::Secret);
        p_les_nf.non_ic_dissem = Box::new([NonIcDissem::LesNf]);
        let s = NonIcDissemSet::from_attrs_iter(&[p_les, p_les_nf]);
        assert!(
            s.as_set().contains(&NonIcDissem::Les),
            "§H.9 p185: classified split after #552 supersession \
             must leave bare Les in the set. set = {:?}",
            s.as_set(),
        );
        assert!(
            !s.as_set().contains(&NonIcDissem::LesNf),
            "§H.9 p185: LES-NF must be transformed into Les + \
             NOFORN on classified pages. set = {:?}",
            s.as_set(),
        );
        assert!(
            s.needs_nf(),
            "§H.9 p185: NOFORN must be injected on classified \
             LES-NF split. needs_nf = {}",
            s.needs_nf(),
        );
    }

    #[test]
    fn non_ic_dissem_set_nodis_injects_nf_regardless_of_classification() {
        // §H.9 p174: NODIS → NF in banner, classification-independent.
        let mut p = portion_us(Classification::Unclassified);
        p.non_ic_dissem = Box::new([NonIcDissem::Nodis]);
        let s = NonIcDissemSet::from_attrs_iter(&[p]);
        assert!(s.as_set().contains(&NonIcDissem::Nodis));
        assert!(s.needs_nf());
    }

    #[test]
    fn non_ic_dissem_set_exdis_injects_nf() {
        // §H.9 p172: EXDIS → NF in banner.
        let mut p = portion_us(Classification::Secret);
        p.non_ic_dissem = Box::new([NonIcDissem::Exdis]);
        let s = NonIcDissemSet::from_attrs_iter(&[p]);
        assert!(s.as_set().contains(&NonIcDissem::Exdis));
        assert!(s.needs_nf());
    }

    #[test]
    fn non_ic_dissem_set_from_empty_input_is_bottom() {
        let s = NonIcDissemSet::from_attrs_iter(&[]);
        assert_eq!(s, NonIcDissemSet::empty());
    }
}
