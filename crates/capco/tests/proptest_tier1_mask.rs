// SPDX-FileCopyrightText: 2026 Knitli Inc.
//
// SPDX-License-Identifier: LicenseRef-MarqueLicense-1.0

//! Property-based parity gate for the PR-E (#371) tier-1 mask-form
//! predicates against an independent structural oracle.
//!
//! # Scope
//!
//! The four tier-1 named-dispatch `Constraint::Custom` rows that PR-E
//! compiled from structural slice walks to [`FactBitmask`] mask logic:
//!
//! - `E021/rd-frd-requires-noforn` — §H.6 p104 + p111
//! - `E024/rd-precedence` — §H.6 p104
//! - `E038/nodis-or-exdis-requires-noforn` — §H.9 p172 + p174
//! - `E070/frd-tfni-precedence` — §H.6 p120
//!
//! # Oracle
//!
//! Each `oracle_*` function below re-derives the rule's predicate
//! directly from the CAPCO-2016 source text — it does NOT call
//! `derive_bits` or any sibling of the production code path. The
//! proptest asserts that the mask form and the oracle agree on a
//! Boolean "fires / does not fire" decision across randomly generated
//! `CanonicalAttrs` shapes.
//!
//! The mask predicates' diagnostic synthesis (message, citation, span,
//! severity) is exercised by the co-located unit tests in
//! `crates/capco/src/scheme/predicates/tier1_mask.rs::tests` and the
//! corpus parity gate. The proptest here is the algebraic-predicate
//! check; combining the two layers gives the same confidence as the
//! PR-C `closure_table_equivalence` gate gave the closure rewire.

use marque_capco::CapcoScheme;
use marque_ism::{
    AeaMarking, Classification, DissemControl, MarkingClassification, NonIcDissem,
    canonical::CanonicalAttrs,
};
use marque_scheme::{ConstraintViolation, MarkingScheme};
use proptest::prelude::*;

// ---------------------------------------------------------------------
// Strategies
// ---------------------------------------------------------------------

fn arb_aea_markings() -> impl Strategy<Value = Box<[AeaMarking]>> {
    // Tiny strategy: each AEA atom is independently present/absent.
    // Bounded slice length to 0..=3 avoids combinatorial blowup while
    // still covering every two-of-three coupling required by E024 /
    // E070.
    (any::<bool>(), any::<bool>(), any::<bool>()).prop_map(|(rd, frd, tfni)| {
        let mut v = Vec::new();
        if rd {
            v.push(AeaMarking::Rd(Default::default()));
        }
        if frd {
            v.push(AeaMarking::Frd(Default::default()));
        }
        if tfni {
            v.push(AeaMarking::Tfni);
        }
        v.into_boxed_slice()
    })
}

fn arb_dissem_us() -> impl Strategy<Value = Box<[DissemControl]>> {
    // Generates a subset of the dissem tokens that the tier-1
    // predicates read (NOFORN + RELIDO directly; ORCON and others as
    // dissem-axis noise that must not perturb the firing decision).
    (any::<bool>(), any::<bool>(), any::<bool>()).prop_map(|(nf, relido, oc)| {
        let mut v = Vec::new();
        if nf {
            v.push(DissemControl::Nf);
        }
        if relido {
            v.push(DissemControl::Relido);
        }
        if oc {
            v.push(DissemControl::Oc);
        }
        v.into_boxed_slice()
    })
}

fn arb_non_ic_dissem() -> impl Strategy<Value = Box<[NonIcDissem]>> {
    (any::<bool>(), any::<bool>(), any::<bool>()).prop_map(|(nodis, exdis, limdis)| {
        let mut v = Vec::new();
        if nodis {
            v.push(NonIcDissem::Nodis);
        }
        if exdis {
            v.push(NonIcDissem::Exdis);
        }
        if limdis {
            v.push(NonIcDissem::Limdis);
        }
        v.into_boxed_slice()
    })
}

fn arb_rel_to_present() -> impl Strategy<Value = Box<[marque_ism::CountryCode]>> {
    // Empty vs non-empty is what the E021 §123/§144 carve-out keys on.
    // A single USA entry is sufficient — the predicate reads presence,
    // not the country list.
    any::<bool>().prop_map(|present| {
        if present {
            vec![marque_ism::CountryCode::try_new(b"USA").expect("USA trigraph")].into_boxed_slice()
        } else {
            Box::new([])
        }
    })
}

fn arb_attrs() -> impl Strategy<Value = CanonicalAttrs> {
    (
        arb_aea_markings(),
        arb_dissem_us(),
        arb_non_ic_dissem(),
        arb_rel_to_present(),
    )
        .prop_map(|(aea, dissem, ndis, rel_to)| {
            let mut a = CanonicalAttrs::default();
            // Classification fixed at US::Secret — the tier-1
            // predicates do NOT read classification (they read AEA /
            // dissem / non-IC dissem / rel_to). A constant value keeps
            // the strategy narrow.
            a.classification = Some(MarkingClassification::Us(Classification::Secret));
            a.aea_markings = aea;
            a.dissem_us = dissem;
            a.non_ic_dissem = ndis;
            a.rel_to = rel_to;
            a
        })
}

// ---------------------------------------------------------------------
// Independent oracles — re-derived from CAPCO-2016 source text
// ---------------------------------------------------------------------

/// Oracle for `E021/rd-frd-requires-noforn` — fires iff
/// (RD or FRD present) AND (no NOFORN) AND (no REL TO list) AND (no
/// RELIDO).
///
/// §H.6 p104 (RD): "Is always used with NOFORN unless a sharing
/// agreement has been established per the Atomic Energy Act."
/// §H.6 p111 (FRD): same rule. §H.6 p120 explicitly excludes TFNI;
/// §H.6 pp116/118 exclude UCNI variants.
fn oracle_e021(attrs: &CanonicalAttrs) -> bool {
    let has_rd_or_frd = attrs
        .aea_markings
        .iter()
        .any(|a| matches!(a, AeaMarking::Rd(_) | AeaMarking::Frd(_)));
    if !has_rd_or_frd {
        return false;
    }
    let has_noforn = attrs.dissem_iter().any(|d| matches!(d, DissemControl::Nf));
    if has_noforn {
        return false;
    }
    if !attrs.rel_to.is_empty()
        || attrs
            .dissem_iter()
            .any(|d| matches!(d, DissemControl::Relido))
    {
        return false;
    }
    true
}

/// Oracle for `E024/rd-precedence` — fires iff (RD present) AND
/// (FRD or TFNI present). §H.6 p104.
fn oracle_e024(attrs: &CanonicalAttrs) -> bool {
    let has_rd = attrs
        .aea_markings
        .iter()
        .any(|a| matches!(a, AeaMarking::Rd(_)));
    if !has_rd {
        return false;
    }
    attrs
        .aea_markings
        .iter()
        .any(|a| matches!(a, AeaMarking::Frd(_) | AeaMarking::Tfni))
}

/// Oracle for `E038/nodis-or-exdis-requires-noforn` — fires iff
/// (NODIS or EXDIS present) AND (no NOFORN). §H.9 p172 + p174.
fn oracle_e038(attrs: &CanonicalAttrs) -> bool {
    let has_trigger = attrs
        .non_ic_dissem
        .iter()
        .any(|d| matches!(d, NonIcDissem::Nodis | NonIcDissem::Exdis));
    if !has_trigger {
        return false;
    }
    !attrs.dissem_iter().any(|d| matches!(d, DissemControl::Nf))
}

/// Oracle for `E070/frd-tfni-precedence` — fires iff FRD AND TFNI both
/// present. §H.6 p120.
fn oracle_e070(attrs: &CanonicalAttrs) -> bool {
    let has_frd = attrs
        .aea_markings
        .iter()
        .any(|a| matches!(a, AeaMarking::Frd(_)));
    let has_tfni = attrs
        .aea_markings
        .iter()
        .any(|a| matches!(a, AeaMarking::Tfni));
    has_frd && has_tfni
}

// ---------------------------------------------------------------------
// Bridge: invoke the production dispatch through the trait surface
// ---------------------------------------------------------------------

/// Shared scheme instance for every proptest case.
///
/// `CapcoScheme::new()` builds the full categories/constraints tables
/// (the catalog has 38 registered rules + 27 class-floor rows + 5 SCI
/// per-system rows + 7 core-catalog rows). Per-case construction
/// across 4 cases × 1024 iterations = 4096 catalog builds and added
/// noticeable test-suite overhead — caching the instance amortizes
/// the build to once per test-binary load.
fn shared_scheme() -> &'static CapcoScheme {
    use std::sync::OnceLock;
    static SCHEME: OnceLock<CapcoScheme> = OnceLock::new();
    SCHEME.get_or_init(CapcoScheme::new)
}

fn fires(name: &'static str, attrs: &CanonicalAttrs) -> bool {
    let marking = marque_capco::CapcoMarking::new(attrs.clone());
    let bits = shared_scheme().precompute_bits(&marking);
    let out: Vec<ConstraintViolation> = shared_scheme().evaluate_custom(name, &marking, bits);
    !out.is_empty()
}

// ---------------------------------------------------------------------
// Proptests
// ---------------------------------------------------------------------

proptest! {
    #![proptest_config(ProptestConfig {
        cases: 1024,
        .. ProptestConfig::default()
    })]

    #[test]
    fn e021_mask_matches_oracle(attrs in arb_attrs()) {
        prop_assert_eq!(fires("E021/rd-frd-requires-noforn", &attrs), oracle_e021(&attrs));
    }

    #[test]
    fn e024_mask_matches_oracle(attrs in arb_attrs()) {
        prop_assert_eq!(fires("E024/rd-precedence", &attrs), oracle_e024(&attrs));
    }

    #[test]
    fn e038_mask_matches_oracle(attrs in arb_attrs()) {
        prop_assert_eq!(fires("E038/nodis-or-exdis-requires-noforn", &attrs), oracle_e038(&attrs));
    }

    #[test]
    fn e070_mask_matches_oracle(attrs in arb_attrs()) {
        prop_assert_eq!(fires("E070/frd-tfni-precedence", &attrs), oracle_e070(&attrs));
    }
}
