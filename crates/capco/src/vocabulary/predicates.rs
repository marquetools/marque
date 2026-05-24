// SPDX-FileCopyrightText: 2026 Knitli Inc.
//
// SPDX-License-Identifier: LicenseRef-MarqueLicense-1.0

use crate::scheme::{FDR_DOMINATORS, capco_token_category};
use marque_scheme::{TokenId, TokenRef};

#[inline]
pub(super) fn is_fdr_dissem(token: TokenId) -> bool {
    FDR_DOMINATORS.iter().any(|entry| match entry {
        TokenRef::Token(id) => *id == token,
        TokenRef::AnyInCategory(cat) => capco_token_category(token) == Some(*cat),
    })
}
