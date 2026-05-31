// SPDX-FileCopyrightText: 2026 Knitli Inc.
//
// SPDX-License-Identifier: LicenseRef-MarqueLicense-1.0

mod categories;
mod predicates;
mod tokens;

use crate::scheme::{CapcoScheme, capco_token_category};
use marque_scheme::{
    Authority, CategoryId, Deprecation, IcMarkingVocabulary, OwnerProducer, PointOfContact, TokenId,
    TokenMetadataFull, Vocabulary,
};

pub use tokens::active_sentinel_count;

impl Vocabulary<CapcoScheme> for CapcoScheme {
    fn authority(&self, token: &TokenId) -> &'static Authority {
        tokens::authority_static(*token)
    }

    fn owner_producer(&self, token: &TokenId) -> &'static OwnerProducer {
        tokens::owner_producer_static(*token)
    }

    fn point_of_contact(&self, token: &TokenId) -> &'static PointOfContact {
        tokens::point_of_contact_static(*token)
    }

    fn deprecation(&self, token: &TokenId) -> Option<&'static Deprecation<TokenId>> {
        tokens::deprecation_static(*token)
    }

    fn forms(&self, token: &TokenId) -> &'static marque_scheme::FormSet {
        tokens::forms_static(*token)
    }

    fn metadata(&self, token: &TokenId) -> &'static TokenMetadataFull<TokenId> {
        tokens::metadata_static(*token)
    }

    #[inline]
    fn shape_admits(&self, category: CategoryId, bytes: &[u8]) -> bool {
        categories::shape_admits(category, bytes)
    }

    fn qualified_token_label(&self, token: &TokenId) -> std::borrow::Cow<'static, str> {
        use marque_scheme::MarkingScheme;

        let canonical = self.metadata(token).canonical;
        let Some(cat_id) = capco_token_category(*token) else {
            return std::borrow::Cow::Owned(format!("unknown.{canonical}"));
        };
        let cat_name = self
            .categories()
            .iter()
            .find(|c| c.id == cat_id)
            .map(|c| c.name)
            .unwrap_or("unknown");
        std::borrow::Cow::Owned(format!("{cat_name}.{canonical}"))
    }
}

impl IcMarkingVocabulary<CapcoScheme> for CapcoScheme {
    #[inline]
    fn is_fdr_dissem(&self, token: &TokenId) -> bool {
        predicates::is_fdr_dissem(*token)
    }
}

#[cfg(test)]
#[cfg_attr(coverage_nightly, coverage(off))]
mod shape_admits_tests;

#[cfg(test)]
#[cfg_attr(coverage_nightly, coverage(off))]
mod fdr_dissem_pin;
