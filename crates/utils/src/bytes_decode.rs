// Adapted from code originally in [CocoIndex](https://CocoIndex)
// Original code from CocoIndex is copyrighted by CocoIndex
// and licensed under the Apache-2.0 License.
// SPDX-License-Identifier: Apache-2.0
// SPDX-FileCopyrightText: 2026 CocoIndex
//
// All modifications from the upstream for Marque are copyrighted by Knitli Inc.
// SPDX-FileCopyrightText: 2026 Knitli Inc. (Marque)
// SPDX-License-Identifier: LicenseRef-MarqueLicense-1.0

//! Decodes a byte buffer to text by sniffing the byte-order mark, then
//! falling back to UTF-8.

use encoding_rs::Encoding;

/// Decodes `bytes` to a string, returning the text and whether decoding hit any
/// malformed sequences.
///
/// A leading byte-order mark wins: a UTF-8, UTF-16LE, or UTF-16BE BOM picks the
/// encoding outright (UTF-32 is not handled). With no BOM, the bytes are read as
/// UTF-8. Malformed input still decodes — invalid sequences become U+FFFD and
/// the second return value is `true`. The result borrows `bytes` when no
/// re-encoding is needed and allocates otherwise.
pub fn bytes_to_string<'a>(bytes: &'a [u8]) -> (std::borrow::Cow<'a, str>, bool) {
    // 1) BOM sniff first (definitive for UTF-8/16; UTF-32 is not supported here).
    if let Some((enc, bom_len)) = Encoding::for_bom(bytes) {
        let (cow, had_errors) = enc.decode_without_bom_handling(&bytes[bom_len..]);
        return (cow, had_errors);
    }
    // 2) Otherwise, try UTF-8 (accepts input with or without a UTF-8 BOM).
    let (cow, had_errors) = encoding_rs::UTF_8.decode_with_bom_removal(bytes);
    (cow, had_errors)
}

#[cfg(test)]
#[cfg_attr(coverage_nightly, coverage(off))]
mod tests {
    use super::*;

    #[test]
    fn test_empty_input() {
        let (cow, had_errors) = bytes_to_string(b"");
        assert_eq!(cow, "");
        assert!(!had_errors);
    }

    #[test]
    fn test_utf8_without_bom() {
        let text = "Hello, world! 😊";
        let (cow, had_errors) = bytes_to_string(text.as_bytes());
        assert_eq!(cow, text);
        assert!(!had_errors);
    }

    #[test]
    fn test_utf8_with_bom() {
        let text = "Hello, world! 😊";
        let mut bytes = vec![0xEF, 0xBB, 0xBF];
        bytes.extend_from_slice(text.as_bytes());

        let (cow, had_errors) = bytes_to_string(&bytes);
        assert_eq!(cow, text);
        assert!(!had_errors);
    }

    #[test]
    fn test_utf16le_with_bom() {
        // "Hi" in UTF-16LE: 'H' (0x0048), 'i' (0x0069)
        let bytes = vec![0xFF, 0xFE, 0x48, 0x00, 0x69, 0x00];
        let (cow, had_errors) = bytes_to_string(&bytes);
        assert_eq!(cow, "Hi");
        assert!(!had_errors);
    }

    #[test]
    fn test_utf16be_with_bom() {
        // "Hi" in UTF-16BE: 'H' (0x0048), 'i' (0x0069)
        let bytes = vec![0xFE, 0xFF, 0x00, 0x48, 0x00, 0x69];
        let (cow, had_errors) = bytes_to_string(&bytes);
        assert_eq!(cow, "Hi");
        assert!(!had_errors);
    }

    #[test]
    fn test_invalid_utf8() {
        // 0x80 is an invalid leading byte in UTF-8
        let bytes = vec![0x80, 0x81];
        let (cow, had_errors) = bytes_to_string(&bytes);
        assert!(had_errors);
        // encoding_rs replaces invalid byte sequences with U+FFFD REPLACEMENT CHARACTER
        assert!(cow.contains('\u{FFFD}'));
    }
}
