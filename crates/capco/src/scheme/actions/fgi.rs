// SPDX-FileCopyrightText: 2026 Knitli Inc.
//
// SPDX-License-Identifier: LicenseRef-MarqueLicense-1.0

//! FGI (Foreign Government Information) helpers — foreign-source
//! extraction and FGI-marker merge logic used by the lattice path's
//! FGI composition.

use marque_ism::MarkingClassification;

/// Extract the set of foreign country codes contributing to FGI
/// semantics from a `MarkingClassification`.
///
/// Used by the lattice path's
/// solely-non-US FGI suppression branch to detect source loss when
/// `ClassificationLattice`'s OrdMax winner discards a foreign source
/// observed on a lower-level portion. The semantics mirror the
/// country-extraction step of the retired
/// `PageContext::expected_fgi_marker` accessor (deleted with the
/// `PageContext::expected_*` surface) so the lattice and
/// scheme projection paths agree on which portions contribute which
/// producers to the FGI axis. The lattice-native form lives in
/// `FgiSet::from_attrs_iter` (see `crates/capco/src/lattice/fgi.rs`);
/// this helper isolates the per-variant extraction logic.
///
/// Per-variant semantic (§H.7 p123 + p128, ISM `Nato`/`Joint`
/// variant definitions):
/// - `Us(_)`: contributes nothing (US is the home authority).
/// - `Fgi(f)`: contributes every country in `f.countries`.
///   Source-concealed FGI portions (`f.countries.is_empty()`) return
///   `None` — distinct from `Some(empty)` which would mean "no FGI".
///   The `None` sentinel propagates up to the FGI-composition branch, which
///   then forces `FgiMarker::SourceConcealed` on the output
///   (§H.7 p128: "a document containing portions of both
///   source-concealed FGI and source-acknowledged FGI must have only
///   the 'FGI' marking"). Verified 2026-05-16.
/// - `Nato(_)`: contributes the literal `"NATO"` trigraph (NATO
///   ownership reciprocity per §H.7 p123 — NATO classification
///   portions surface "NATO" as the producer trigraph on the FGI
///   axis when not commingled with US).
/// - `Joint(j)`: contributes every non-USA country in `j.countries`
///   (USA is implicit on the JOINT axis per §H.3 p56).
/// - `Conflict { foreign, .. }`: recurses into the foreign payload
///   so the implicit US classification at `us` is excluded but the
///   foreign side's producers still contribute. Returns the same
///   set as the `foreign` payload would have produced as a stand-
///   alone classification.
///
/// Returns `Option<Vec<CountryCode>>` where:
/// - `None` = "this portion is source-concealed FGI" (distinct signal)
/// - `Some(vec)` = the contributing country codes (possibly empty if
///   the classification is not a foreign system)
///
/// The caller collects into a `BTreeSet` for deduplication.
///
/// Return type is `Option<Vec<CountryCode>>` rather than
/// `Option<Vec<CountryCode>>` to propagate the concealed signal.
/// Pre-fix, source-concealed FGI portions returned an empty `Vec`,
/// indistinguishable from "no FGI at all" — the FGI-composition equality check
/// then silently dropped the concealed signal and could build a
/// synthetic acknowledged marker, contradicting §H.7 p128.
pub(crate) fn extract_foreign_sources(
    c: Option<&MarkingClassification>,
) -> Option<Vec<marque_ism::CountryCode>> {
    use marque_ism::{CountryCode, ForeignClassification};
    let nato_code = || CountryCode::try_new(b"NATO").expect("NATO trigraph is valid");
    match c {
        None | Some(MarkingClassification::Us(_)) => Some(Vec::new()),
        Some(MarkingClassification::Fgi(f)) => {
            if f.countries.is_empty() {
                // Source-concealed FGI: return the sentinel None so callers
                // can detect concealment vs "no foreign source".
                None
            } else {
                Some(f.countries.to_vec())
            }
        }
        Some(MarkingClassification::Nato(_)) => Some(vec![nato_code()]),
        Some(MarkingClassification::Joint(j)) => Some(
            j.countries
                .iter()
                .filter(|c| c.as_str() != "USA")
                .copied()
                .collect(),
        ),
        Some(MarkingClassification::Conflict { foreign, .. }) => match foreign.as_ref() {
            ForeignClassification::Fgi(f) => {
                if f.countries.is_empty() {
                    None
                } else {
                    Some(f.countries.to_vec())
                }
            }
            ForeignClassification::Nato(_) => Some(vec![nato_code()]),
            ForeignClassification::Joint(j) => Some(
                j.countries
                    .iter()
                    .filter(|c| c.as_str() != "USA")
                    .copied()
                    .collect(),
            ),
        },
    }
}

/// Merge two optional `FgiMarker` values, preserving the
/// source-concealed sentinel and unioning the producer country
/// sets when both sides carry acknowledged markers.
///
/// Without this, the `join_via_lattice` FGI
/// composition discarded `expected_fgi_marker`'s
/// classification-derived producers whenever an explicit FGI marker
/// existed. This helper unions both sources so the lattice output
/// preserves every non-US producer the PageContext path would
/// surface.
pub(crate) fn merge_fgi_markers(
    a: Option<marque_ism::FgiMarker>,
    b: Option<marque_ism::FgiMarker>,
) -> Option<marque_ism::FgiMarker> {
    use marque_ism::FgiMarker;
    match (a, b) {
        (None, None) => None,
        (Some(x), None) | (None, Some(x)) => Some(x),
        // Source-concealed dominates per §H.7 pp123-124 — bare `FGI`
        // (no LIST) is the most-restrictive marker. Either operand
        // carrying it produces SourceConcealed.
        // previously cited `§H.7 p123`; verified 2026-05-16 against
        // CAPCO-2016.md — the §H.7 block begins on p123 but the
        // load-bearing supersession sentence ("If any document
        // contains portions of both source-concealed FGI ... and
        // source-acknowledged FGI ... then only the 'FGI' marking
        // without the source trigraph(s)/tetragraph(s) must appear
        // in the banner line") lands on p124 in the Precedence Rules
        // for Banner Line Guidance block. The page-span citation is
        // the precise reference.
        (Some(FgiMarker::SourceConcealed), _) | (_, Some(FgiMarker::SourceConcealed)) => {
            Some(FgiMarker::SourceConcealed)
        }
        (
            Some(FgiMarker::Acknowledged { countries: c1, .. }),
            Some(FgiMarker::Acknowledged { countries: c2, .. }),
        ) => {
            // Union the producer sets, deduplicated and sorted.
            let mut all: std::collections::BTreeSet<marque_ism::CountryCode> =
                c1.iter().copied().collect();
            all.extend(c2.iter().copied());
            FgiMarker::acknowledged(all)
        }
    }
}
