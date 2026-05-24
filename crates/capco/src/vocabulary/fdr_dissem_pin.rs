// SPDX-FileCopyrightText: 2026 Knitli Inc.
//
// SPDX-License-Identifier: LicenseRef-MarqueLicense-1.0

use crate::CapcoScheme;
use crate::scheme::FDR_DOMINATORS;
use marque_scheme::{TokenId, TokenRef, Vocabulary};

fn vocab() -> CapcoScheme {
    CapcoScheme::new()
}

#[test]
fn fdr_dominators_entries_all_reachable() {
    let v = vocab();
    for entry in FDR_DOMINATORS {
        match entry {
            TokenRef::Token(id) => assert!(
                v.is_fdr_dissem(id),
                "FDR_DOMINATORS entry {entry:?} not recognized by \
                 is_fdr_dissem — the override has drifted from the \
                 authoritative slice. Verify the iter().any(...) \
                 walk in vocabulary/predicates.rs reaches every entry.",
            ),
            TokenRef::AnyInCategory(cat) => {
                let routed = sentinel_routed_to_category(*cat);
                assert!(
                    routed,
                    "FDR_DOMINATORS contains AnyInCategory({cat:?}) \
                     but no known sentinel TokenId resolves to that \
                     category via capco_token_category. The \
                     is_fdr_dissem override iterates over \
                     FDR_DOMINATORS and routes single TokenIds \
                     through `capco_token_category` to hit \
                     AnyInCategory arms; without a category-routing \
                     arm the new category is unreachable. Add a \
                     capco_token_category arm covering the new \
                     category before this row lands.",
                );
            }
        }
    }
}

fn sentinel_routed_to_category(cat: marque_scheme::CategoryId) -> bool {
    use crate::scheme::*;
    let probes: &[TokenId] = &[
        TOK_NOFORN,
        TOK_JOINT,
        TOK_USA,
        TOK_RESTRICTED,
        TOK_RD,
        TOK_FRD,
        TOK_TFNI,
        TOK_CNWDI,
        TOK_UCNI,
        TOK_DCNI,
        TOK_HCS,
        TOK_NODIS,
        TOK_EXDIS,
        TOK_RELIDO,
        TOK_DISPLAY_ONLY,
        TOK_ORCON,
        TOK_ORCON_USGOV,
        TOK_REL_TO,
        TOK_SBU_NF,
        TOK_LES_NF,
        TOK_IMCON,
        TOK_DSEN,
        TOK_RSEN,
        TOK_FOUO,
        TOK_PROPIN,
        TOK_FISA,
        TOK_RAWFISA,
        TOK_LIMDIS,
        TOK_LES,
        TOK_SBU,
        TOK_SSI,
        TOK_NNPI,
        TOK_EYES,
        TOK_ATOMAL,
        TOK_BALK,
        TOK_BOHEMIA,
        TOK_SI_G,
        TOK_HCS_O,
        TOK_HCS_P,
        TOK_TK_BLFH,
        TOK_TK_IDIT,
        TOK_TK_KAND,
    ];
    probes
        .iter()
        .any(|id| capco_token_category(*id) == Some(cat))
}

#[test]
fn relido_admits_despite_is_fdr_dominator_excluding_it() {
    use crate::scheme::TOK_RELIDO;
    let v = vocab();
    assert!(
        v.is_fdr_dissem(&TOK_RELIDO),
        "RELIDO is unambiguously an FD&R member per §B.3.a p19. \
         The override must not delegate through \
         `is_fdr_dominator`, which deliberately excludes RELIDO \
         for the RELIDO-conflict family predicate.",
    );
}
