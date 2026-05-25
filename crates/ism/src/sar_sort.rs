// SPDX-FileCopyrightText: 2026 Knitli Inc.
//
// SPDX-License-Identifier: LicenseRef-MarqueLicense-1.0

//! SAR identifier sort key.
//!
//! Per CAPCO §H.5 (p99–100): "ascending sort order with numbered values
//! first, followed by alphabetic values" at each hierarchical level.
//! The implementation splits the identifier at its leading digit run.
//!
//! Re-exported at `marque_ism::sar_sort_key`.

/// Sort key for SAR identifiers per CAPCO §H.5 (p99–100): "ascending sort
/// order with numbered values first, followed by alphabetic values" at each
/// hierarchical level.
///
/// Splits the identifier at its leading digit run. If present, the digits are
/// parsed as `u64` and the tuple `(false, n, rest)` is returned (with `false`
/// sorting before `true`). Pure-alpha identifiers return `(true, 0, s)`.
///
/// This helper is the canonical SAR sort-key implementation; both
/// `marque-ism` (banner roll-up via lattice) and `marque-capco` (rules
/// E028/E029) use it via the `marque_ism::sar_sort_key` re-export.
pub fn sar_sort_key(s: &str) -> (bool, u64, &str) {
    let prefix_len = s.bytes().take_while(|b| b.is_ascii_digit()).count();
    if prefix_len == 0 {
        (true, 0, s)
    } else {
        let n: u64 = s[..prefix_len].parse().unwrap_or(u64::MAX);
        (false, n, &s[prefix_len..])
    }
}

#[cfg(test)]
#[cfg_attr(coverage_nightly, coverage(off))]
mod tests {
    use super::*;

    #[test]
    fn pure_alpha_returns_true_zero_string() {
        // Pure-alpha identifiers sort after pure-numeric.
        assert_eq!(sar_sort_key("ABCD"), (true, 0, "ABCD"));
    }

    #[test]
    fn pure_numeric_returns_false_value_empty_suffix() {
        // Pure-numeric identifiers sort first (false < true).
        assert_eq!(sar_sort_key("42"), (false, 42, ""));
    }

    #[test]
    fn numeric_prefix_then_alpha_returns_false_value_alpha_suffix() {
        // Mixed identifiers split at the leading digit run.
        assert_eq!(sar_sort_key("12X"), (false, 12, "X"));
    }

    #[test]
    fn numeric_overflow_saturates_to_u64_max() {
        // Defensive: overlong numeric prefixes saturate rather than panic.
        let long = "99999999999999999999"; // > u64::MAX
        let (sort_first, n, rest) = sar_sort_key(long);
        assert!(!sort_first);
        assert_eq!(n, u64::MAX);
        assert_eq!(rest, "");
    }
}
