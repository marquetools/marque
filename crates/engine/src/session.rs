// SPDX-FileCopyrightText: 2026 Knitli Inc.
//
// SPDX-License-Identifier: LicenseRef-MarqueLicense-1.0

//! Session-level audit metadata (`marque-3.2`, issue #399).
//!
//! `marque-3.2` is additive over `marque-3.1`: it introduces a single
//! `session_metadata` record emitted as the **first** line of a
//! non-empty audit stream (a sibling to the terminal
//! [`crate::SessionRoot`] record). It records the facts that are
//! constant for the whole fix session rather than per-record:
//!
//! - **Versioning** — the Marque core version, the active audit schema,
//!   the marking scheme's lattice version, and the decoder version, so
//!   any applied fix can be traced to the exact engine/lattice/decoder
//!   revision that produced it.
//! - **Integrity seal** — a BLAKE3 fingerprint over the four version
//!   strings ([`SessionMetadata::seal`]). Because the metadata record
//!   is the first line and is covered by the [`crate::SessionRoot`]
//!   Merkle root, tampering with any version, the interface, the
//!   identity, or the signature breaks the session root.
//! - **Interface identification** — which surface applied the fix
//!   ([`InterfaceCode`]: server / CLI / WASM / other).
//! - **Classifier identity** — `classifier_id` and
//!   `classification_authority`, resolved per-call (a `FixOptions`
//!   override beats the engine `Config`).
//! - **Signature (carry-only)** — an optional caller-supplied detached
//!   signature string. Marque does **not** sign in-tree yet; it stamps
//!   whatever the caller provides. Full in-engine X.509 signing is
//!   tracked as a follow-up.
//!
//! # Content-ignorance (Constitution V)
//!
//! Every field is a permitted-identifier type: version strings, a
//! closed interface enum, a `blake3:`-prefixed seal, the
//! engine-controlled classifier identity, and a caller-supplied
//! signature token. No document content reaches this record. The
//! `crates/engine/tests/audit_g13_canary.rs` corpus sweep renders and
//! scans it alongside the per-record lines.

use std::sync::Arc;

use smol_str::SmolStr;

/// Domain-separation tag for the integrity seal. Versioned so a future
/// change to the seal construction is distinguishable.
const SEAL_DOMAIN: &[u8] = b"marque-seal-v1\0";

/// The interface through which a fix was applied.
///
/// Codes per issue #399: `S` (server), `C` (CLI), `W` (WASM), `O`
/// (other / embedder). Closed enum — a new surface adds a variant in
/// lockstep with [`Self::as_str`]. Defaults to [`InterfaceCode::Other`]
/// so a caller that does not declare its interface still produces a
/// well-formed record.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum InterfaceCode {
    /// REST microservice (`marque-server`).
    Server,
    /// Command-line interface (`marque`).
    Cli,
    /// WebAssembly embedding (`marque-wasm`).
    Wasm,
    /// Any other embedder / unknown surface.
    #[default]
    Other,
}

impl InterfaceCode {
    /// Single-character wire code.
    #[inline]
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Server => "S",
            Self::Cli => "C",
            Self::Wasm => "W",
            Self::Other => "O",
        }
    }
}

/// Session-level audit metadata record (`marque-3.2`).
///
/// Built by the engine at the close of every `Engine::fix` call and
/// attached to [`crate::FixResult::session_metadata`]. Each output
/// surface emits [`Self::to_ndjson`] as the first line of a non-empty
/// audit stream and folds it into the [`crate::SessionRoot`] Merkle
/// computation.
///
/// `#[non_exhaustive]` reserves a grow path (e.g., a future signing
/// algorithm identifier) without a breaking change.
#[derive(Debug, Clone)]
#[non_exhaustive]
pub struct SessionMetadata {
    /// Marque core version (the engine crate's `CARGO_PKG_VERSION`).
    pub marque_version: &'static str,
    /// Active audit-record schema ([`crate::AUDIT_SCHEMA_VERSION`]).
    pub audit_schema: &'static str,
    /// The marking scheme's lattice version
    /// (`MarkingScheme::lattice_version`).
    pub lattice_version: SmolStr,
    /// The probabilistic decoder's version
    /// ([`crate::DECODER_VERSION`]).
    pub decoder_version: &'static str,
    /// Which interface applied the fix.
    pub interface: InterfaceCode,
    /// Resolved classifier identity (`FixOptions` override beats
    /// `Config`); `None` if neither is configured.
    pub classifier_id: Option<Arc<str>>,
    /// Resolved classification authority; `None` if not configured.
    pub classification_authority: Option<Arc<str>>,
    /// Optional caller-supplied detached signature (carry-only).
    pub signature: Option<Arc<str>>,
}

impl SessionMetadata {
    /// Integrity seal: `blake3:<hex>` over the four version strings.
    ///
    /// Per issue #399 the seal is a fingerprint of the combined
    /// versions — Marque, lattice, decoder, and audit schema. The
    /// inputs are domain-tagged and NUL-delimited so the concatenation
    /// is unambiguous (no version string can masquerade as another by
    /// shifting a boundary). The interface and identity are NOT part of
    /// the seal itself — they are bound into the audit chain instead
    /// via the [`crate::SessionRoot`] Merkle root that covers this
    /// whole record.
    #[must_use]
    pub fn seal(&self) -> String {
        let mut hasher = blake3::Hasher::new();
        hasher.update(SEAL_DOMAIN);
        hasher.update(self.marque_version.as_bytes());
        hasher.update(&[0]);
        hasher.update(self.lattice_version.as_bytes());
        hasher.update(&[0]);
        hasher.update(self.decoder_version.as_bytes());
        hasher.update(&[0]);
        hasher.update(self.audit_schema.as_bytes());
        format!("blake3:{}", hasher.finalize().to_hex())
    }

    /// Serialize the `session_metadata` NDJSON record (no trailing
    /// newline).
    ///
    /// Field order is fixed (`type`, `schema`, `marque_version`,
    /// `lattice_version`, `decoder_version`, `interface`, `seal`, then
    /// the optional `classifier_id` / `classification_authority` /
    /// `signature`) so the record is byte-identical across every
    /// surface that emits it (CLI, server, WASM) — the cross-surface
    /// parity tests depend on it. Optional identity / signature fields
    /// are omitted when `None`.
    ///
    /// All values are serialized through `serde_json` rather than raw
    /// interpolation, so a caller-supplied identity / signature
    /// containing a quote or backslash still yields well-formed JSON.
    #[must_use]
    pub fn to_ndjson(&self) -> String {
        let mut map = serde_json::Map::new();
        map.insert("type".into(), "session_metadata".into());
        map.insert("schema".into(), self.audit_schema.into());
        map.insert("marque_version".into(), self.marque_version.into());
        map.insert(
            "lattice_version".into(),
            self.lattice_version.as_str().into(),
        );
        map.insert("decoder_version".into(), self.decoder_version.into());
        map.insert("interface".into(), self.interface.as_str().into());
        map.insert("seal".into(), self.seal().into());
        if let Some(id) = self.classifier_id.as_deref() {
            map.insert("classifier_id".into(), id.into());
        }
        if let Some(authority) = self.classification_authority.as_deref() {
            map.insert("classification_authority".into(), authority.into());
        }
        if let Some(sig) = self.signature.as_deref() {
            map.insert("signature".into(), sig.into());
        }
        serde_json::Value::Object(map).to_string()
    }
}

#[cfg(test)]
#[cfg_attr(coverage_nightly, coverage(off))]
mod tests {
    use super::*;

    fn sample() -> SessionMetadata {
        SessionMetadata {
            marque_version: "0.2.1",
            audit_schema: "marque-3.2",
            lattice_version: SmolStr::new("capco-lattice-1"),
            decoder_version: "decoder-1",
            interface: InterfaceCode::Cli,
            classifier_id: None,
            classification_authority: None,
            signature: None,
        }
    }

    #[test]
    fn interface_codes_are_single_chars() {
        assert_eq!(InterfaceCode::Server.as_str(), "S");
        assert_eq!(InterfaceCode::Cli.as_str(), "C");
        assert_eq!(InterfaceCode::Wasm.as_str(), "W");
        assert_eq!(InterfaceCode::Other.as_str(), "O");
        assert_eq!(InterfaceCode::default(), InterfaceCode::Other);
    }

    #[test]
    fn seal_is_blake3_prefixed_and_deterministic() {
        let m = sample();
        let s = m.seal();
        assert!(s.starts_with("blake3:"));
        assert_eq!(s.len(), "blake3:".len() + 64);
        assert_eq!(s, sample().seal());
    }

    #[test]
    fn seal_changes_when_any_version_changes() {
        let base = sample().seal();
        let mut m = sample();
        m.decoder_version = "decoder-2";
        assert_ne!(base, m.seal());
        let mut m = sample();
        m.lattice_version = SmolStr::new("capco-lattice-2");
        assert_ne!(base, m.seal());
        let mut m = sample();
        m.audit_schema = "marque-9.9";
        assert_ne!(base, m.seal());
        let mut m = sample();
        m.marque_version = "9.9.9";
        assert_ne!(base, m.seal());
    }

    #[test]
    fn seal_ignores_interface_and_identity() {
        // The seal covers only the four version strings; the interface
        // and identity are bound via the SessionRoot Merkle chain, not
        // the seal itself.
        let base = sample().seal();
        let mut m = sample();
        m.interface = InterfaceCode::Server;
        m.classifier_id = Some(Arc::from("12345"));
        m.signature = Some(Arc::from("SIG"));
        assert_eq!(base, m.seal());
    }

    #[test]
    fn ndjson_shape_is_content_free_and_well_formed() {
        let line = sample().to_ndjson();
        let v: serde_json::Value = serde_json::from_str(&line).unwrap();
        assert_eq!(v["type"], "session_metadata");
        assert_eq!(v["schema"], "marque-3.2");
        assert_eq!(v["marque_version"], "0.2.1");
        assert_eq!(v["lattice_version"], "capco-lattice-1");
        assert_eq!(v["decoder_version"], "decoder-1");
        assert_eq!(v["interface"], "C");
        assert!(v["seal"].as_str().unwrap().starts_with("blake3:"));
        // Optional fields omitted when None.
        assert!(v.get("classifier_id").is_none());
        assert!(v.get("classification_authority").is_none());
        assert!(v.get("signature").is_none());
    }

    #[test]
    fn ndjson_includes_identity_and_signature_when_present() {
        let mut m = sample();
        m.classifier_id = Some(Arc::from("12345"));
        m.classification_authority = Some(Arc::from("EO 13526"));
        m.signature = Some(Arc::from("SIG"));
        let v: serde_json::Value = serde_json::from_str(&m.to_ndjson()).unwrap();
        assert_eq!(v["classifier_id"], "12345");
        assert_eq!(v["classification_authority"], "EO 13526");
        assert_eq!(v["signature"], "SIG");
    }

    #[test]
    fn ndjson_escapes_hostile_identity_input() {
        let mut m = sample();
        m.classifier_id = Some(Arc::from("ev\"il\\id"));
        m.signature = Some(Arc::from("sig\"with\\escapes"));
        let v: serde_json::Value =
            serde_json::from_str(&m.to_ndjson()).expect("to_ndjson must emit parseable JSON");
        assert_eq!(v["classifier_id"], "ev\"il\\id");
        assert_eq!(v["signature"], "sig\"with\\escapes");
    }
}
