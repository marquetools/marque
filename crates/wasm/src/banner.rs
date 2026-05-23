// SPDX-FileCopyrightText: 2026 Knitli Inc.
//
// SPDX-License-Identifier: LicenseRef-MarqueLicense-1.0

#[cfg(not(target_arch = "wasm32"))]
use std::time::{SystemTime, UNIX_EPOCH};

/// Seconds in a Julian year (365.25 × 24 × 3600), used to approximate the
/// current calendar year from a UNIX timestamp.
pub(crate) const SECONDS_PER_JULIAN_YEAR: u64 = 31_557_600;

/// Returns the current calendar year, usable in both native and WASM contexts.
///
/// In WASM, uses `Date.now()` via wasm_bindgen. In native, uses `SystemTime`.
pub(crate) fn current_year() -> u32 {
    #[cfg(target_arch = "wasm32")]
    {
        let millis = crate::date_now_ms() as u64;
        let secs = millis / 1000;
        1970 + (secs / SECONDS_PER_JULIAN_YEAR) as u32
    }
    #[cfg(not(target_arch = "wasm32"))]
    {
        let secs = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();
        1970 + (secs / SECONDS_PER_JULIAN_YEAR) as u32
    }
}

// ---------------------------------------------------------------------------
// compute_banner — scanner + parser + scheme.project (no rules engine)
// ---------------------------------------------------------------------------

/// Compute the expected CAPCO banner string from portion markings in `text`.
///
/// Scans the text for portion markings only, parses each, accumulates the
/// per-portion `CanonicalAttrs`, and returns the canonical banner via
/// `scheme.render_banner(scheme.project(Scope::Page, ...))`. Does NOT run
/// the rules engine — this is purely: scanner → parser → scheme.project →
/// render_banner.
///
/// Returns `"UNCLASSIFIED"` if no portions are found or none parse.
///
/// Banner derivation runs through the scheme's
/// `render_canonical(Scope::Page, ...)` per the `MarkingScheme`
/// trait's "single source of truth for canonical form" contract
/// (the `MarkingScheme::render_canonical` doc in `crates/scheme/src/scheme.rs`).
pub fn compute_banner_native(text: &str) -> Result<String, String> {
    use marque_capco::CapcoMarking;
    use marque_capco::scheme::CapcoScheme;
    use marque_core::{Parser, Scanner};
    use marque_ism::{CapcoTokenSet, MarkingType};
    use marque_scheme::MarkingScheme as _;

    let scheme = CapcoScheme::new();
    let token_set = CapcoTokenSet;
    let parser = Parser::new(&token_set);
    let candidates = Scanner::scan(text.as_bytes());
    let mut markings: Vec<CapcoMarking> = Vec::new();

    for candidate in &candidates {
        if candidate.kind != MarkingType::Portion {
            continue;
        }
        if let Ok(parsed) = parser.parse(candidate, text.as_bytes()) {
            // PR 3c.2.B B3 (PM-B-1, PM-B-3): canonicalization seam
            // migrated to the `MarkingScheme::canonicalize` trait
            // method. `scheme` was constructed above;
            // reuse — no new allocation. This function is documented
            // as "Does NOT run the rules engine" (callers reach for it
            // when they want banner roll-up without rule dispatch);
            // the migration preserves that behavior byte-for-byte.
            let attrs = scheme.canonicalize(parsed.attrs);
            markings.push(CapcoMarking::new(attrs));
        }
    }

    if markings.is_empty() {
        return Ok("UNCLASSIFIED".to_owned());
    }

    let projected = scheme.project(marque_scheme::Scope::Page, &markings);
    Ok(scheme.render_banner(&projected))
}

// ---------------------------------------------------------------------------
// generate_cab — Classification Authority Block text
// ---------------------------------------------------------------------------

/// Generate a Classification Authority Block (CAB) text block.
///
/// Scans `text` for portion markings to determine the document's expected
/// classification and declassification marking, then produces a formatted CAB:
///
/// ```text
/// Classified By: <classified_by>
/// Derived From: <derived_from>
/// Declassify On: <declass>
/// ```
///
/// # Declassification logic
///
/// 1. If an explicit `declassify_on` date or `declass_exemption` is found in a
///    parsed marking in `text`, that value is used verbatim.
/// 2. Otherwise, the default is **25 years from the current year** per
///    EO 13526, section 1.5(a) (the default duration of original
///    classification when no other instruction is present, restated in
///    CAPCO-2016 §E.1 p31).
/// 3. If the document computes as UNCLASSIFIED (with or without dissem
///    controls), returns an **empty string** — no CAB is required for
///    UNCLASSIFIED documents.
///
/// `classified_by` defaults to `"Derivative Classifier"` if not provided.
/// `derived_from` defaults to `"Multiple Sources"` if not provided.
pub fn generate_cab_native(
    text: &str,
    classified_by: Option<String>,
    derived_from: Option<String>,
) -> Result<String, String> {
    use marque_capco::CapcoMarking;
    use marque_capco::scheme::CapcoScheme;
    use marque_core::{Parser, Scanner};
    use marque_ism::{CapcoTokenSet, Classification, MarkingType};
    use marque_scheme::MarkingScheme as _;

    let classified_by = classified_by.unwrap_or_else(|| "Derivative Classifier".to_owned());
    let derived_from = derived_from.unwrap_or_else(|| "Multiple Sources".to_owned());

    // Scan text and accumulate per-portion `CanonicalAttrs` along with
    // (a) the first declassify_on date observed (CAB-specific — first
    // wins by historical contract, NOT the lattice MaxDate semantic),
    // (b) the first declass_exemption observed (CAB-specific — first
    // wins, NOT the page-rollup last-observed semantic), and
    // (c) the last-observed exemption as a fallback for the
    // `page_context.expected_declass_exemption` migration path
    // (which used last-observed). The double accumulator preserves
    // the pre-PR-4b-E semantics exactly: `found_declass_exemption`
    // (first-wins) is consulted first; the last-observed fallback
    // fires only when no portion carried an explicit value and the
    // page is otherwise classified.
    let scheme = CapcoScheme::new();
    let token_set = CapcoTokenSet;
    let parser = Parser::new(&token_set);
    let candidates = Scanner::scan(text.as_bytes());
    let mut markings: Vec<CapcoMarking> = Vec::new();
    let mut found_declass_date: Option<String> = None;
    let mut found_declass_exemption: Option<String> = None;
    // PR 4b-E (OQ-1 option a): inline per-portion accumulator for the
    // last-observed declass_exemption (formerly via
    // `page_context.expected_declass_exemption()`). CAB-only fields
    // (`declass_exemption`, `classified_by`, `derived_from`,
    // `token_spans`) stay excluded from `ProjectedMarking` by design
    // (see `crates/ism/src/projected.rs` "page aggregate, not a CAB"
    // contract). The architect plan defers a dedicated `CabProjection`
    // type to a future PR; for now we inline the 3-line accumulator
    // at the only consumer. If a second CAB consumer arrives,
    // promote to a `CabProjection` type in `marque-ism`.
    let mut last_observed_exemption: Option<marque_ism::DeclassExemption> = None;

    for candidate in &candidates {
        if let Ok(parsed) = parser.parse(candidate, text.as_bytes()) {
            // PR 3c.2.B B3 (PM-B-1, PM-B-3): canonicalization seam
            // migrated to the `MarkingScheme::canonicalize` trait
            // method. `scheme` was constructed above;
            // reuse — no new allocation. CAB-line generation runs
            // outside the rules engine by design; the migration
            // preserves that behavior byte-for-byte.
            let attrs = scheme.canonicalize(parsed.attrs);
            if found_declass_date.is_none() {
                if let Some(date) = &attrs.declassify_on {
                    // `to_maxdate_str()` always returns 8-digit YYYYMMDD:
                    // Year(y) → "{y}1231", YearMonth(y,m) → last day of month,
                    // Date / DateHourMin / DateTime → YYYYMMDD of the date component.
                    // This is the format expected on a CAB "Declassify On:" line.
                    found_declass_date = Some(date.to_maxdate_str().into());
                }
            }
            if found_declass_exemption.is_none() {
                if let Some(ex) = attrs.declass_exemption {
                    found_declass_exemption = Some(ex.as_str().to_owned());
                }
            }
            // Track the last-observed exemption across portions for
            // the fallback below — mirrors
            // `DeclassExemptionAccumulator::from_attrs_iter`'s
            // last-wins semantic.
            //
            // M-4 (PR 4b-E review fix-up): the dual-accumulator
            // asymmetry here is intentional. `last_observed_exemption`
            // is portion-kind-gated because that's the accumulator the
            // CAB fallback ladder uses when no explicit `declass_*`
            // CAB-line field appears in the input (and the §E.3 pp
            // 32-33 "longest period of protection" semantics is what
            // the Phase-3 successor will produce). `found_*` above are
            // first-wins across ALL candidate kinds (banner / CAB /
            // portion) — they're capturing the explicit CAB-line
            // values when present, which can appear in a banner-or-CAB
            // candidate that is NOT itself a portion. The two
            // accumulators feed different rungs of the fallback ladder
            // (see the `let declass = ...` below) per OQ-1 option (a) /
            // architect plan §3 Decision 1.
            if candidate.kind == MarkingType::Portion {
                if let Some(ex) = attrs.declass_exemption {
                    last_observed_exemption = Some(ex);
                }
                markings.push(CapcoMarking::new(attrs));
            }
        }
    }

    // If the document is unclassified, there is no CAB at all.
    // CAPCO: a CAB is only required for classified NSI documents; an
    // UNCLASSIFIED banner (with or without dissem controls) carries no
    // "Classified By", "Derived From", or "Declassify On" fields.
    //
    // PR 4b-E: `page_context.is_classified()` migrated to a
    // projected-marking read. The scheme's `project(Scope::Page, ...)`
    // composes the per-axis lattice projection of which classification
    // is one component; the predicate is the same — "effective
    // classification level above Unclassified."
    // No portions → no classification, treat as Unclassified.
    // (Same fall-through behavior as the pre-PR-4b-E
    // `page_context.is_classified()` over an empty accumulator.)
    if markings.is_empty() {
        return Ok(String::new());
    }
    let projected = scheme.project(marque_scheme::Scope::Page, &markings);
    let is_classified = projected
        .0
        .classification
        .as_ref()
        .is_some_and(|c| c.effective_level() > Classification::Unclassified);
    if !is_classified {
        return Ok(String::new());
    }

    // Determine the declassification marking.
    //
    // PR 4b-E (OQ-1 option a resolved): the third-priority fallback
    // formerly went through `page_context.expected_declass_exemption()`;
    // it now reads `last_observed_exemption` accumulated inline above.
    // Same last-observed semantic; same Phase-3 TODO carries over on
    // `DeclassExemptionAccumulator` for a duration-aware comparator
    // (§E.3 pp 32-33 "longest period of protection").
    let declass = if let Some(date) = found_declass_date {
        date
    } else if let Some(ex) = found_declass_exemption {
        ex
    } else if let Some(ex) = last_observed_exemption {
        ex.as_str().to_owned()
    } else {
        // EO 13526 §1.5(a) default: 25 years from the date of origin.
        // Since we cannot determine the document date from raw text, we
        // use the current year as a conservative base (the user should
        // supply a known origination date via a future API parameter when
        // precision matters).
        // Format as YYYYMMDD (December 31, conventional end-of-year date).
        let base_year = current_year();
        format!("{}1231", base_year + 25)
    };

    Ok(format!(
        "Classified By: {classified_by}\nDerived From: {derived_from}\nDeclassify On: {declass}"
    ))
}
