// SPDX-FileCopyrightText: 2026 Knitli Inc.
//
// SPDX-License-Identifier: LicenseRef-MarqueLicense-1.0

//! Session-end audit-chain integrity (issue #184, `marque-3.1`).
//!
//! At the close of an `Engine::fix` session, a verifier-checkable
//! [`SessionRoot`] is computed over the ordered sequence of serialized
//! audit-record NDJSON lines and emitted as a terminal `session_root`
//! record in the audit stream. Re-hashing the preceding records and
//! comparing against the root detects any post-emission deletion,
//! mutation, or reordering of records.
//!
//! Lives in `marque-engine` (not `marque-rules`) because `blake3` is an
//! unconditional dependency here, whereas in `marque-rules` it is gated
//! behind the optional `audit` feature; the engine is also the
//! convergence point every output surface (CLI, server, WASM) already
//! depends on. The issue permits a "sentinel variant" rather than an
//! `AuditLine` enum member, so this stays a standalone type that each
//! surface emits after the per-record audit lines.
//!
//! # Why a Merkle root (not a hash chain)
//!
//! Per-record hash chaining (each record embedding the previous
//! record's hash) forces sequential, non-parallelizable emission — a
//! cost `BatchEngine` must not pay. A session-end Merkle root works over
//! the already-produced ordered sequence, imposing no ordering
//! constraint on batch processing. The root is computed **per document**
//! (one `Engine::fix` call = one session = one root); batch callers
//! receive an independent root per document.
//!
//! # Construction (RFC 6962-style domain separation)
//!
//! The root is computed over the **exact emitted record-line bytes**
//! (the JSON object, with NO trailing newline), in emission order,
//! **excluding the terminal record itself** (a record cannot embed its
//! own hash). Domain-separation tags prevent second-preimage attacks
//! (a leaf can never be confused with an internal node):
//!
//! - leaf:  `H(0x00 ‖ line_bytes)`
//! - node:  `H(0x01 ‖ left_32 ‖ right_32)`
//! - empty: `H(0x02)` (a zero-record session still has a well-defined,
//!   verifiable root)
//! - an odd node at any level is **promoted unchanged** to the next.
//!
//! `H` is BLAKE3-256 ([`blake3`], already in the dependency tree — zero
//! new deps; the workspace pin selects the pure-Rust path on wasm32 per
//! Constitution III).
//!
//! # Reproducibility
//!
//! The root is deterministic over fixed record bytes: re-running a fix
//! under a fixed clock (so `AppliedFix` timestamps are stable) yields
//! byte-identical records and therefore an identical root. The terminal
//! record's own `ts` field is wall-clock emission time and is **not**
//! part of the hash input, so it never affects the root.
//!
//! # Verifier recipe
//!
//! 1. Collect the audit-record NDJSON lines that precede the terminal
//!    `session_root` record (each line without its trailing newline).
//! 2. Recompute [`merkle_root`] over them.
//! 3. Compare against the `root` field of the terminal record (strip the
//!    `blake3:` prefix, hex-decode to 32 bytes) via [`SessionRoot::verify`].
//!
//! # Content-ignorance (Constitution V)
//!
//! Audit-record lines are digest-only by the G13 invariant (no document
//! content), so the `session_root` record carries only a `type`
//! discriminant, the `schema` string, an integer `record_count`, the
//! BLAKE3 `root` (rendered `blake3:<hex>`), and an RFC3339 `ts`. None of
//! these is content-bearing.

/// Domain-separation prefix for a Merkle leaf.
const LEAF_TAG: u8 = 0x00;
/// Domain-separation prefix for a Merkle internal node.
const NODE_TAG: u8 = 0x01;
/// Domain-separation prefix for the empty-session marker.
const EMPTY_TAG: u8 = 0x02;

/// BLAKE3 of a single leaf: `H(0x00 ‖ line)`.
fn leaf_hash(line: &[u8]) -> [u8; 32] {
    let mut hasher = blake3::Hasher::new();
    hasher.update(&[LEAF_TAG]);
    hasher.update(line);
    *hasher.finalize().as_bytes()
}

/// BLAKE3 of an internal node: `H(0x01 ‖ left ‖ right)`.
fn node_hash(left: &[u8; 32], right: &[u8; 32]) -> [u8; 32] {
    let mut hasher = blake3::Hasher::new();
    hasher.update(&[NODE_TAG]);
    hasher.update(left);
    hasher.update(right);
    *hasher.finalize().as_bytes()
}

/// Compute the BLAKE3 Merkle root over an ordered sequence of
/// already-serialized audit-record lines.
///
/// Each element is one record's canonical NDJSON bytes **without** a
/// trailing newline. The same function is used by the producer (over
/// the bytes it emits) and by any verifier (over the bytes it reads
/// back), so the two can never drift. See the module docs for the
/// construction and the empty/odd edge cases.
pub fn merkle_root(lines: &[impl AsRef<[u8]>]) -> [u8; 32] {
    if lines.is_empty() {
        // Domain-tagged empty marker — distinct from any single-leaf
        // root, so a zero-record session is still verifiable.
        return *blake3::hash(&[EMPTY_TAG]).as_bytes();
    }

    let mut level: Vec<[u8; 32]> = lines.iter().map(|l| leaf_hash(l.as_ref())).collect();
    while level.len() > 1 {
        let mut next = Vec::with_capacity(level.len().div_ceil(2));
        let mut i = 0;
        while i < level.len() {
            if i + 1 < level.len() {
                next.push(node_hash(&level[i], &level[i + 1]));
                i += 2;
            } else {
                // Odd node at this level: promote unchanged.
                next.push(level[i]);
                i += 1;
            }
        }
        level = next;
    }
    level[0]
}

/// Render a 32-byte digest as lowercase hex (64 chars), matching the
/// BLAKE3 crate's `Display`/`to_hex` output. Hand-rolled (rather than
/// via `blake3::Hash::to_hex`) so the encoding is fixed by this code and
/// cannot drift with the blake3 crate's formatting surface.
fn to_hex(bytes: &[u8; 32]) -> String {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    let mut s = String::with_capacity(64);
    for &b in bytes {
        s.push(HEX[(b >> 4) as usize] as char);
        s.push(HEX[(b & 0x0f) as usize] as char);
    }
    s
}

/// Session-end audit-chain integrity summary (`marque-3.1`).
///
/// Computed at the close of an `Engine::fix` session over the ordered
/// audit-record lines and emitted as the terminal `session_root` record
/// in the audit stream. See the [module docs](self) for the Merkle
/// construction and the verifier recipe.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SessionRoot {
    /// Number of audit records the root covers (the count of records
    /// preceding the terminal `session_root` record in the stream).
    pub record_count: usize,
    /// The BLAKE3 Merkle root over those records.
    pub root: [u8; 32],
}

impl SessionRoot {
    /// Compute a [`SessionRoot`] over an ordered sequence of
    /// already-serialized audit-record lines (one element per record,
    /// each the canonical NDJSON bytes without a trailing newline).
    pub fn compute(lines: &[impl AsRef<[u8]>]) -> Self {
        Self {
            record_count: lines.len(),
            root: merkle_root(lines),
        }
    }

    /// The Merkle root as lowercase hex (64 chars, no prefix).
    pub fn root_hex(&self) -> String {
        to_hex(&self.root)
    }

    /// Serialize the terminal `session_root` NDJSON record.
    ///
    /// `schema` is the active audit schema (pass
    /// [`crate::AUDIT_SCHEMA_VERSION`] so the terminal record can never
    /// disagree with the per-record `schema` field — the audit canary
    /// depends on this). `ts_rfc3339` is the wall-clock emission time,
    /// pre-formatted by the caller (e.g. via `humantime::format_rfc3339`);
    /// it is informational and is NOT part of the Merkle input.
    ///
    /// The line is hand-formatted rather than serde-serialized because
    /// every field is a controlled, content-free value (a closed `type`
    /// discriminant, the schema const, an integer count, a hex digest,
    /// and an RFC3339 timestamp) — none can introduce a JSON-escaping
    /// hazard, and this keeps the terminal-record shape byte-identical
    /// across every surface that emits it.
    pub fn to_ndjson(&self, schema: &str, ts_rfc3339: &str) -> String {
        format!(
            "{{\"type\":\"session_root\",\"schema\":\"{schema}\",\"record_count\":{count},\"root\":\"blake3:{root}\",\"ts\":\"{ts}\"}}",
            schema = schema,
            count = self.record_count,
            root = self.root_hex(),
            ts = ts_rfc3339,
        )
    }

    /// Verify that `lines` re-hash to `expected_root`.
    ///
    /// A verifier feeds the audit-record lines that preceded the
    /// terminal record (each without its trailing newline) and the
    /// 32-byte root extracted from the terminal record. Returns `true`
    /// iff the recomputed root matches — deletion, mutation, or
    /// reordering of any record makes it `false`.
    pub fn verify(lines: &[impl AsRef<[u8]>], expected_root: &[u8; 32]) -> bool {
        &merkle_root(lines) == expected_root
    }
}

#[cfg(test)]
#[cfg_attr(coverage_nightly, coverage(off))]
mod tests {
    use super::*;

    fn lines(n: usize) -> Vec<String> {
        (0..n).map(|i| format!("{{\"rec\":{i}}}")).collect()
    }

    #[test]
    fn empty_session_has_well_defined_root() {
        let empty: Vec<String> = Vec::new();
        let r = SessionRoot::compute(&empty);
        assert_eq!(r.record_count, 0);
        assert_eq!(r.root, *blake3::hash(&[EMPTY_TAG]).as_bytes());
        assert_ne!(r.root, merkle_root(&lines(1)));
    }

    #[test]
    fn single_record_root_is_domain_separated_leaf() {
        let one = lines(1);
        assert_eq!(merkle_root(&one), leaf_hash(one[0].as_bytes()));
        assert_ne!(merkle_root(&one), *blake3::hash(&[EMPTY_TAG]).as_bytes());
    }

    #[test]
    fn two_record_root_is_node_over_leaves() {
        let two = lines(2);
        let expected = node_hash(&leaf_hash(two[0].as_bytes()), &leaf_hash(two[1].as_bytes()));
        assert_eq!(merkle_root(&two), expected);
    }

    #[test]
    fn three_records_promote_the_odd_leaf() {
        let three = lines(3);
        let l0 = leaf_hash(three[0].as_bytes());
        let l1 = leaf_hash(three[1].as_bytes());
        let l2 = leaf_hash(three[2].as_bytes());
        let expected = node_hash(&node_hash(&l0, &l1), &l2);
        assert_eq!(merkle_root(&three), expected);
    }

    #[test]
    fn root_is_reproducible_over_identical_lines() {
        assert_eq!(merkle_root(&lines(5)), merkle_root(&lines(5)));
    }

    #[test]
    fn mutating_a_record_invalidates_the_root() {
        let original = lines(4);
        let root = SessionRoot::compute(&original).root;
        let mut mutated = original.clone();
        mutated[2] = "{\"rec\":999}".to_string();
        assert!(!SessionRoot::verify(&mutated, &root));
    }

    #[test]
    fn deleting_a_record_invalidates_the_root() {
        let original = lines(4);
        let root = SessionRoot::compute(&original).root;
        let mut shortened = original.clone();
        shortened.remove(1);
        assert!(!SessionRoot::verify(&shortened, &root));
    }

    #[test]
    fn reordering_records_invalidates_the_root() {
        let original = lines(4);
        let root = SessionRoot::compute(&original).root;
        let mut reordered = original.clone();
        reordered.swap(0, 3);
        assert!(!SessionRoot::verify(&reordered, &root));
    }

    #[test]
    fn verify_accepts_the_unmodified_sequence() {
        let original = lines(7);
        let root = SessionRoot::compute(&original).root;
        assert!(SessionRoot::verify(&original, &root));
    }

    #[test]
    fn root_hex_is_64_lowercase_hex_chars() {
        let hex = SessionRoot::compute(&lines(2)).root_hex();
        assert_eq!(hex.len(), 64);
        assert!(hex.chars().all(|c| c.is_ascii_hexdigit() && !c.is_ascii_uppercase()));
    }

    #[test]
    fn ndjson_shape_is_content_free_and_well_formed() {
        let line = SessionRoot::compute(&lines(3)).to_ndjson("marque-3.1", "2026-05-29T00:00:00Z");
        assert!(line.starts_with("{\"type\":\"session_root\","));
        assert!(line.contains("\"schema\":\"marque-3.1\""));
        assert!(line.contains("\"record_count\":3"));
        assert!(line.contains("\"root\":\"blake3:"));
        assert!(line.contains("\"ts\":\"2026-05-29T00:00:00Z\""));
        assert!(line.ends_with('}'));
    }
}
