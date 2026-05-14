// SPDX-FileCopyrightText: 2026 Knitli Inc.
//
// SPDX-License-Identifier: LicenseRef-MarqueLicense-1.0

//! marque-ism build script.
//!
//! Parses ODNI ISM specification files at compile time and generates Rust
//! code into `OUT_DIR/`:
//!
//! - `values.rs`      — CVE enumeration types (closed Rust enums + lookup tables)
//! - `validators.rs`  — Schematron-derived validation predicates
//! - `migrations.rs`  — deprecated marking → replacement mappings
//!
//! # Schema source
//!
//! Schemas come from the [`ism-data`](https://github.com/marquetools/ism-data)
//! workspace, consumed as `[build-dependencies]`:
//!
//! - The [`ism`] crate (`urn:us:gov:ic:ism` + every
//!   `urn:us:gov:ic:cvenum:ism:*`) — `CVE/ISM/`, `Schema/ISM/`, `Schematron/ISM/`.
//! - The [`ism_ismcat`] crate (the standalone ISMCAT package) — Tetragraph
//!   Taxonomy and the `urn:us:gov:ic:cvenum:ismcat:*` CVE enumerations.
//!
//! Both crates expose a `package_root()` returning a filesystem path that
//! resolves at the consumer's compile time to the unpacked vendored data,
//! and a `MANIFEST_DIGEST` const that pins the SHA-256 of the manifest.
//! Each crate's own `build.rs` re-hashes every file under `data/` against
//! the baked manifest before this build.rs ever runs, so the integrity
//! chain is enforced upstream.
//!
//! # Files consumed
//!
//! ```text
//! ism crate (package_root = data/ISM)
//!   CVE/ISM/             — CVEnumISM*.xml + CVEnumISM*.json (CVE values + sidecars)
//!   Schema/ISM/          — IC-ISM.xsd, CVEGenerated/CVEnumISM*.xsd
//!                          (and bundled IC-ARH.xsd / IC-NTK.xsd, not consumed here)
//!   Schematron/ISM/      — ISM_XML.sch, Lib/*.sch
//!
//! ism-ismcat crate (package_root = data/ISMCAT)
//!   Schema/ISMCAT/CVEGenerated/CVEnumISMCATRelTo.xsd  — country trigraphs
//!   Taxonomy/ISMCAT/TetragraphTaxonomyDenormalized.xml  — tetragraph membership
//! ```
//!
//! Rerun triggers: any change to this build script, Cargo.toml, or the
//! build-dep crates' versions. Cargo handles build-dep rerun automatically;
//! we don't need a `rerun-if-changed=schemas/` since there's no local
//! schema tree anymore.

use quick_xml::{Reader, XmlVersion, events::Event};

use std::{env, fs, path::Path};

/// Upstream ODNI ISM package version label (the version on ODNI's
/// publication page). Pinned here for compile-time cross-check against
/// `[package.metadata.marque] ism-schema-version` in `Cargo.toml`.
/// Bump intentionally when ODNI publishes an updated ISM package and
/// the bump is reflected in `ism-data`.
const SCHEMA_VERSION: &str = "ISM-v2022-DEC";

/// Pinned `ism-data` workspace version (YYYYMMDD.MAJOR.PATCH where
/// YYYYMMDD = ODNI snapshot date the data was vendored from). Cross-
/// checked against `[package.metadata.marque] ism-data-version`.
const ISM_DATA_VERSION: &str = "20230609.0.0";

fn main() {
    let ism_root = ism::package_root();
    let ismcat_root = ism_ismcat::package_root();
    let out_dir = env::var("OUT_DIR").expect("OUT_DIR not set");
    let out_path = Path::new(&out_dir);

    // T010: Assert schema version matches Cargo.toml metadata.
    verify_schema_version();
    // Issue #208: ISMCAT Tetragraph Taxonomy version pin.
    verify_ismcat_tetra_version();
    // Cross-check the pinned ism-data snapshot against Cargo.toml metadata.
    verify_ism_data_version();

    // Cargo automatically reruns this build script when build dependencies
    // change, so no `rerun-if-changed=schemas/` is needed (the old vendored
    // tree is gone). Keep the build.rs / Cargo.toml triggers — those are
    // not covered by the build-dep mechanism.
    println!("cargo:rerun-if-changed=build.rs");
    println!("cargo:rerun-if-changed=Cargo.toml");

    generate_values(out_path, &ism_root, &ismcat_root);
    generate_validators(out_path, &ism_root);
    generate_migrations(out_path, &ism_root);
    generate_vocabulary(out_path, &ism_root);
}

// ---------------------------------------------------------------------------
// T010: Schema version pinning assertion
// ---------------------------------------------------------------------------

fn verify_schema_version() {
    // Read the Cargo.toml metadata to verify it matches our compiled schema dir.
    let cargo_toml = fs::read_to_string("Cargo.toml").expect("failed to read Cargo.toml");
    let table: toml::Table = cargo_toml
        .parse()
        .expect("failed to parse Cargo.toml as TOML");

    let pinned = table
        .get("package")
        .and_then(|p| p.get("metadata"))
        .and_then(|m| m.get("marque"))
        .and_then(|m| m.get("ism-schema-version"))
        .and_then(|v| v.as_str())
        .unwrap_or_else(|| {
            panic!(
                "FR-011: [package.metadata.marque] ism-schema-version not found in Cargo.toml. \
                 Add: [package.metadata.marque]\nism-schema-version = \"{SCHEMA_VERSION}\""
            )
        });

    assert_eq!(
        pinned, SCHEMA_VERSION,
        "FR-011: schema version mismatch — Cargo.toml says {pinned:?} but build.rs targets \
         {SCHEMA_VERSION:?}. Update one to match the other."
    );
}

/// Verify that the `ism-data` snapshot pinned in Cargo.toml matches the
/// version this build.rs was written against. Constitution Principle IV —
/// schema versions are pinned in cargo metadata and bumped intentionally.
fn verify_ism_data_version() {
    let cargo_toml = fs::read_to_string("Cargo.toml").expect("failed to read Cargo.toml");
    let table: toml::Table = cargo_toml
        .parse()
        .expect("failed to parse Cargo.toml as TOML");

    let pinned = table
        .get("package")
        .and_then(|p| p.get("metadata"))
        .and_then(|m| m.get("marque"))
        .and_then(|m| m.get("ism-data-version"))
        .and_then(|v| v.as_str())
        .unwrap_or_else(|| {
            panic!(
                "[package.metadata.marque] ism-data-version not found in Cargo.toml. \
                 Add: ism-data-version = \"{ISM_DATA_VERSION}\""
            )
        });

    assert_eq!(
        pinned, ISM_DATA_VERSION,
        "ism-data version mismatch — Cargo.toml says {pinned:?} but build.rs \
         targets {ISM_DATA_VERSION:?}. Update one to match the other (and \
         update the [build-dependencies] ism / ism-ismcat / ism-data versions \
         in lock-step)."
    );
}

// ---------------------------------------------------------------------------
// T006: CVE XML parsing → typed Rust enums
// ---------------------------------------------------------------------------

/// Parse a CVE XML file and extract all `<Value>` text content.
/// Handles both default-namespace `<Term><Value>` and prefixed `<cve:Term><cve:Value>`.
fn parse_cve_xml(path: &Path) -> Vec<(String, String)> {
    let content = fs::read_to_string(path)
        .unwrap_or_else(|e| panic!("failed to read {}: {e}", path.display()));
    let mut reader = Reader::from_str(&content);
    let mut buf = Vec::new();

    let mut entries = Vec::new();
    let mut in_term = false;
    let mut in_value = false;
    let mut in_description = false;
    let mut current_value = String::new();
    let mut current_desc = String::new();

    loop {
        match reader.read_event_into(&mut buf) {
            Ok(Event::Start(ref e)) | Ok(Event::Empty(ref e)) => {
                let name = e.name();
                let local = local_name(name.as_ref());
                match local {
                    b"Term" => {
                        in_term = true;
                        current_value.clear();
                        current_desc.clear();
                    }
                    b"Value" if in_term => in_value = true,
                    b"Description" if in_term => in_description = true,
                    _ => {}
                }
            }
            Ok(Event::End(ref e)) => {
                let name = e.name();
                let local = local_name(name.as_ref());
                match local {
                    b"Term" => {
                        if in_term && !current_value.is_empty() {
                            entries.push((current_value.clone(), current_desc.trim().to_owned()));
                        }
                        in_term = false;
                    }
                    b"Value" => in_value = false,
                    b"Description" => in_description = false,
                    _ => {}
                }
            }
            Ok(Event::Text(ref e)) => {
                // M-10: do not silently truncate on unescape failure. A bad
                // entity in a CVE file would otherwise feed an empty value
                // into `to_rust_ident`, which would be caught by C-4 below
                // but with a confusing error message — fail loudly here.
                let decoded = e.decode().unwrap_or_else(|err| {
                    panic!("XML entity unescape error in {}: {err}", path.display())
                });
                if in_value {
                    current_value.push_str(&decoded);
                } else if in_description {
                    current_desc.push_str(&decoded);
                }
            }
            Ok(Event::Eof) => break,
            Err(e) => panic!("XML parse error in {}: {e}", path.display()),
            _ => {}
        }
        buf.clear();
    }

    entries
}

/// Strip namespace prefix from an XML name: `cve:Term` → `Term`.
fn local_name(name: &[u8]) -> &[u8] {
    match memchr::memchr(b':', name) {
        Some(pos) => &name[pos + 1..],
        None => name,
    }
}

/// Convert a CVE value string to a valid Rust identifier.
/// `HCS-O` → `HcsO`, `25X1-EO-12951` → `X25x1Eo12951`, `NF` → `Nf`.
fn to_rust_ident(s: &str) -> String {
    let mut result = String::new();
    let mut capitalize_next = true;

    // If starts with a digit, prefix with underscore character representation
    let s = if s.starts_with(|c: char| c.is_ascii_digit()) {
        format!("X{s}")
    } else {
        s.to_owned()
    };

    for ch in s.chars() {
        match ch {
            '-' | '_' | ' ' => capitalize_next = true,
            c if c.is_alphanumeric() => {
                if capitalize_next {
                    result.extend(c.to_uppercase());
                    capitalize_next = false;
                } else {
                    result.extend(c.to_lowercase());
                }
            }
            _ => {}
        }
    }

    result
}

/// Resolve every CVE entry to a `(value, ident, desc)` triple, asserting that
/// no identifier is empty and that no two identifiers collide. Detects
/// codegen-breaking CVE additions at build time rather than at consumer
/// compile time. (C-4)
fn resolve_idents(name: &str, entries: &[(String, String)]) -> Vec<(String, String, String)> {
    use std::collections::HashMap;
    // Map ident -> first CVE value that produced it, so a collision can name
    // both offenders in its panic message. (M-2)
    let mut seen: HashMap<String, String> = HashMap::with_capacity(entries.len());
    let mut resolved = Vec::with_capacity(entries.len());
    for (value, desc) in entries {
        let ident = to_rust_ident(value);
        assert!(
            !ident.is_empty(),
            "build.rs: enum {name}: CVE value {value:?} produced an empty Rust identifier"
        );
        if let Some(existing_value) = seen.get(&ident) {
            panic!(
                "build.rs: enum {name}: CVE values {existing_value:?} and {value:?} both \
                 produce the Rust identifier {ident:?}. to_rust_ident needs disambiguation."
            );
        }
        seen.insert(ident.clone(), value.clone());
        resolved.push((value.clone(), ident, desc.clone()));
    }
    resolved
}

/// Generate a Rust enum and associated methods from CVE entries.
fn emit_enum(out: &mut String, name: &str, entries: &[(String, String)], doc: &str) {
    use std::fmt::Write;

    let resolved = resolve_idents(name, entries);

    writeln!(out, "/// {doc}").unwrap();
    writeln!(
        out,
        "#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]"
    )
    .unwrap();
    writeln!(out, "#[non_exhaustive]").unwrap();
    writeln!(out, "pub enum {name} {{").unwrap();

    for (_value, ident, desc) in &resolved {
        let short_desc = desc.lines().next().unwrap_or("").trim();
        if !short_desc.is_empty() {
            writeln!(out, "    /// {short_desc}").unwrap();
        }
        writeln!(out, "    {ident},").unwrap();
    }
    writeln!(out, "}}").unwrap();
    writeln!(out).unwrap();

    // as_str() method
    writeln!(out, "impl {name} {{").unwrap();
    writeln!(
        out,
        "    /// Returns the canonical CVE string representation."
    )
    .unwrap();
    writeln!(out, "    pub fn as_str(&self) -> &'static str {{").unwrap();
    writeln!(out, "        match self {{").unwrap();
    for (value, ident, _) in &resolved {
        writeln!(out, "            {name}::{ident} => {value:?},").unwrap();
    }
    writeln!(out, "        }}").unwrap();
    writeln!(out, "    }}").unwrap();
    writeln!(out).unwrap();

    // from_str() method
    writeln!(
        out,
        "    /// Parse from the canonical CVE string. Returns `None` for unknown values."
    )
    .unwrap();
    writeln!(out, "    pub fn parse(s: &str) -> Option<Self> {{").unwrap();
    writeln!(out, "        match s {{").unwrap();
    for (value, ident, _) in &resolved {
        writeln!(out, "            {value:?} => Some({name}::{ident}),").unwrap();
    }
    writeln!(out, "            _ => None,").unwrap();
    writeln!(out, "        }}").unwrap();
    writeln!(out, "    }}").unwrap();
    writeln!(out).unwrap();

    // ALL constant
    writeln!(out, "    /// All valid values in CVE-defined order.").unwrap();
    writeln!(out, "    pub const ALL: &[{name}] = &[").unwrap();
    for (_value, ident, _) in &resolved {
        writeln!(out, "        {name}::{ident},").unwrap();
    }
    writeln!(out, "    ];").unwrap();
    writeln!(out, "}}").unwrap();
    writeln!(out).unwrap();

    // Display impl
    writeln!(out, "impl std::fmt::Display for {name} {{").unwrap();
    writeln!(
        out,
        "    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {{"
    )
    .unwrap();
    writeln!(out, "        f.write_str(self.as_str())").unwrap();
    writeln!(out, "    }}").unwrap();
    writeln!(out, "}}").unwrap();
    writeln!(out).unwrap();
}

/// Assert that a required CVE file produced at least one entry.
///
/// SAR is the only CVE with a legitimate empty-file contract (the public
/// schema intentionally ships no entries). Every other enum must be
/// non-empty — an empty `CVEnumISMDissem.xml` from a bad schema copy would
/// silently produce a valid-but-empty Rust enum and make all dissem rules
/// fire zero diagnostics. (M-1)
fn assert_required(name: &str, entries: &[(String, String)], file: &str) {
    assert!(
        !entries.is_empty(),
        "build.rs: required CVE file {file} produced zero entries for enum {name}. \
         This is almost always a bad schema copy — verify the `ism` build-dep \
         was resolved to the pinned ism-data snapshot and that {file} parses."
    );
}

fn generate_values(out: &Path, ism_root: &Path, ismcat_root: &Path) {
    use std::fmt::Write;
    let cve_dir = ism_root.join("CVE/ISM");
    let mut content =
        String::from("// Generated by build.rs from ODNI ISM CVE XML — DO NOT EDIT.\n\n");

    // --- Classification ---
    // The canonical Classification enum is hand-written in `attrs.rs` because
    // the CVE uses single-letter abbreviations (R/U/C/S/TS) while the tool
    // needs both abbreviated and full-word forms. We still parse the CVE file
    // to populate ALL_CVE_TOKENS below — but we do NOT emit a generated
    // `Classification` enum, to avoid the ambiguity of two types with the
    // same name in the crate.
    let class_entries = parse_cve_xml(&cve_dir.join("CVEnumISMClassificationAll.xml"));
    assert_required(
        "Classification (ALL_CVE_TOKENS)",
        &class_entries,
        "CVEnumISMClassificationAll.xml",
    );

    // --- SCI Controls ---
    let sci_entries = parse_cve_xml(&cve_dir.join("CVEnumISMSCIControls.xml"));
    assert_required("SciControl", &sci_entries, "CVEnumISMSCIControls.xml");
    emit_enum(
        &mut content,
        "SciControl",
        &sci_entries,
        "SCI control markings from CVEnumISMSCIControls.xml.",
    );

    // --- SCI Control Bare (subset of SCI Controls whose CVE value has no '-') ---
    // Spec: 003-sci-compartments / data-model §Types.
    // SciControlBare is the structural-anchor enum. It includes only CVE
    // values that are themselves a bare control system (no compound
    // composite). For 2022-DEC this yields the 7 variants
    // Bur, Hcs, Klm, Mvl, Rsv, Si, Tk. Generated from the live CVE so it
    // tracks future ODNI revisions automatically.
    let sci_bare_entries: Vec<(String, String)> = sci_entries
        .iter()
        .filter(|(value, _)| !value.contains('-'))
        .cloned()
        .collect();
    assert_required(
        "SciControlBare",
        &sci_bare_entries,
        "CVEnumISMSCIControls.xml (bare subset)",
    );
    emit_enum(
        &mut content,
        "SciControlBare",
        &sci_bare_entries,
        "Bare SCI control systems (CVE values with no '-') from CVEnumISMSCIControls.xml.",
    );

    // Helper: is_bare_cve_value — reports whether `s` is one of the bare
    // control-system CVE values. Used by the parser's structural path to
    // dispatch to `SciControlSystem::Published` vs `Custom`.
    writeln!(
        content,
        "/// Returns true if `s` is exactly a bare SCI control system CVE value.\n\
         /// Equivalent to `SciControlBare::parse(s).is_some()` but spelled out\n\
         /// for ergonomics at parser call sites."
    )
    .unwrap();
    writeln!(content, "pub fn is_bare_cve_value(s: &str) -> bool {{").unwrap();
    writeln!(content, "    SciControlBare::parse(s).is_some()").unwrap();
    writeln!(content, "}}").unwrap();
    writeln!(content).unwrap();

    // --- Dissemination Controls ---
    //
    // ODNI's `CVEnumISMDissem.xml` is a UNION enum that serves both the
    // ICRM (IC Register and Manual) tooling and the ISOO CUI Registry
    // tooling. Its own `Source:` field at the top of the XML names two
    // sources:
    //
    //   "1) IC Systems Register and Manual; 2) ISOO CUI Register"
    //
    // CAPCO is source 1 only. Source 2 is the ISOO CUI Registry — a
    // separate marking system that CAPCO-2016 §A explicitly disclaims:
    //
    //   "This document does not address internal IC element control
    //   markings (i.e., **caveats**) or warnings and notices …"
    //   (CAPCO-2016, line 283 of the vendored manual)
    //
    // The XML interleaves: IC Register entries first, then `WAIVED`
    // (DOD-5205-07-SAP, also out-of-CAPCO-scope), then the ISOO CUI
    // tail (AC / AWP / DL_ONLY / FED_ONLY / FEDCON / NOCON). The
    // CAPCO-2016 §A.5 page 38 table lists the IC dissem set with
    // each entry's banner / portion form; that table is the
    // authoritative cross-check (see also §H.8 page 161 for the FISA
    // template, representative of the per-marking detail pages).
    //
    // We deny-list the seven non-IC entries by exact CVE value so a
    // future ICRM revision adding a new IC dissem control flows
    // through automatically. Adding a new non-IC entry to the deny
    // list is an intentional edit that reviewers can verify against
    // the XML's stated `Source:` ordering.
    //
    // Tracking issue for the broader "second banner line / caveat
    // markings" data model: github.com/marquetools/marque#128.
    const NON_IC_DISSEM_DENY_LIST: &[&str] = &[
        // DOD-SAP-source — called out in the XML description and
        // ordering as out of CAPCO scope.
        "WAIVED",
        // ISOO CUI Registry / handling caveats — out of CAPCO scope
        // per §A line 283. These are "second banner line" markings
        // in the ICRM sense; CAPCO-2016 disclaims them explicitly.
        "AC", "AWP", "DL_ONLY", "FED_ONLY", "FEDCON", "NOCON",
    ];

    let raw_dissem_entries = parse_cve_xml(&cve_dir.join("CVEnumISMDissem.xml"));
    assert_required("DissemControl", &raw_dissem_entries, "CVEnumISMDissem.xml");
    let dissem_entries: Vec<(String, String)> = raw_dissem_entries
        .into_iter()
        .filter(|(value, _)| !NON_IC_DISSEM_DENY_LIST.contains(&value.as_str()))
        .collect();
    assert_required(
        "DissemControl (post-deny-list)",
        &dissem_entries,
        "CVEnumISMDissem.xml — every entry was filtered out by NON_IC_DISSEM_DENY_LIST",
    );
    emit_enum(
        &mut content,
        "DissemControl",
        &dissem_entries,
        "IC dissemination controls from CVEnumISMDissem.xml. \
         CUI / DOD-SAP / handling-caveat entries are deny-listed in \
         build.rs per CAPCO-2016 §A line 283 (caveats are out of \
         CAPCO scope) and the XML's own Source: field naming \
         IC Register vs ISOO CUI Registry as separate sources.",
    );

    // --- SAR Identifiers ---
    // Intentionally NOT emitted as a CVE enum.
    //
    // `CVEnumISMSAR.xml` is empty in the public ODNI ISM package (and will
    // remain so): SAR program identifiers are agency-assigned codewords, not
    // a centrally registered closed vocabulary. Code-generation is the wrong
    // tool for a category whose membership is not enumerable.
    //
    // SAR is modeled structurally via `attrs::SarMarking` / `SarProgram` /
    // `SarCompartment` and validated by syntactic rules (E026–E031) rather
    // than membership checks. See `specs/002-sar-implementation/spec.md`.

    // --- Declass Exemptions (25X codes) ---
    let declass_entries = parse_cve_xml(&cve_dir.join("CVEnumISM25X.xml"));
    assert_required("DeclassExemption", &declass_entries, "CVEnumISM25X.xml");
    emit_enum(
        &mut content,
        "DeclassExemption",
        &declass_entries,
        "Declassification exemption codes from CVEnumISM25X.xml.",
    );

    // --- ExemptFrom ---
    let exempt_entries = parse_cve_xml(&cve_dir.join("CVEnumISMExemptFrom.xml"));
    assert_required("ExemptFrom", &exempt_entries, "CVEnumISMExemptFrom.xml");
    emit_enum(
        &mut content,
        "ExemptFrom",
        &exempt_entries,
        "Exempt-from rule sets from CVEnumISMExemptFrom.xml.",
    );

    // --- T007: Trigraphs from XSD ---
    // Source: the standalone ODNI ISMCAT package (ism-ismcat crate), not the
    // bundled ISMCAT subset inside the ISM zip. The standalone package is the
    // canonical home of the `urn:us:gov:ic:cvenum:ismcat:relto` namespace.
    let trigraphs = parse_xsd_trigraphs(
        &ismcat_root
            .join("Schema/ISMCAT/CVEGenerated")
            .join("CVEnumISMCATRelTo.xsd"),
    );

    // Build the canonical recognition set from the CVE entries.
    let cve_codes: std::collections::BTreeSet<String> =
        trigraphs.into_iter().map(|(v, _)| v).collect();

    // Issue #183 PR-B: load org-specific country-code extensions
    // from `country_extensions.toml`. Empty file (or no [[code]]
    // entries) is a no-op — the generated tables are byte-
    // identical to a build without the file. Validation is
    // strict: each extension `code` must satisfy the CAPCO byte
    // set, must not duplicate a CVE entry or earlier extension,
    // and any `members` must already be recognized.
    let extensions = load_country_extensions("country_extensions.toml", &cve_codes);

    // Merge: extension recognition codes go into TRIGRAPHS
    // alongside CVE entries. The BTreeSet keeps the slice sorted
    // for `is_trigraph` binary_search.
    let mut sorted_trigraphs = cve_codes.clone();
    for ext in &extensions {
        sorted_trigraphs.insert(ext.code.clone());
    }

    // Surface extensions as a doc comment block so an auditor
    // grepping the generated values.rs for an unfamiliar code can
    // find its provenance immediately. Empty extensions list is a
    // no-op (skip the header).
    if !extensions.is_empty() {
        writeln!(content, "/// Org-specific country-code extensions from").unwrap();
        writeln!(content, "/// `crates/ism/country_extensions.toml`:").unwrap();
        for ext in &extensions {
            let kind = if ext.members.is_empty() {
                "opaque"
            } else {
                "expansion-enabled"
            };
            let mut entry_line =
                format!("/// - `{}` ({kind}): {}", ext.code, ext.description.trim(),);
            // Append temporal activity window when present (Phase 3 plumbing).
            match (&ext.active_from, &ext.active_to) {
                (Some(af), Some(at)) => {
                    entry_line.push_str(&format!(" [active {af}–{at}]"));
                }
                (Some(af), None) => {
                    entry_line.push_str(&format!(" [active from {af}]"));
                }
                (None, Some(at)) => {
                    entry_line.push_str(&format!(" [active until {at}]"));
                }
                (None, None) => {}
            }
            writeln!(content, "{entry_line}").unwrap();
        }
        writeln!(content, "///").unwrap();
    }

    // Emit the country-code recognition slice (not an enum — the
    // CVE list has hundreds of entries; `CountryCode` is the typed
    // wrapper). Despite the legacy `TRIGRAPHS` name, this slice
    // carries the full CVE recognition surface: 2-byte (`EU`),
    // 3-byte trigraphs, 4-byte tetragraphs (`FVEY`, `NATO`, …),
    // and 15-byte `AUSTRALIA_GROUP`. The name is kept for
    // backwards compatibility with consumers; a future PR may
    // rename to `COUNTRY_CODES` alongside the `is_trigraph`
    // rename.
    //
    // M-3: sort and deduplicate into a BTreeSet before emission so
    // `is_trigraph` in token_set.rs can use `binary_search` over a
    // guaranteed-sorted slice. The XSD emits entries in document order
    // (USA first, then alphabetical), so an unsorted emission would
    // silently break binary_search if the ODNI bundle ever reorders.
    writeln!(
        content,
        "/// All valid country / country-group codes — CVE entries \
         from CVEnumISMCATRelTo.xsd plus any org-specific\n\
         /// extensions from `country_extensions.toml`. Sorted \
         ascending and deduplicated. `is_trigraph` uses \
         binary_search."
    )
    .unwrap();
    writeln!(
        content,
        "/// {} entries total ({} CVE + {} extension).",
        sorted_trigraphs.len(),
        cve_codes.len(),
        extensions.len(),
    )
    .unwrap();
    writeln!(content, "pub static TRIGRAPHS: &[&str] = &[").unwrap();
    for value in &sorted_trigraphs {
        writeln!(content, "    {value:?},").unwrap();
    }
    writeln!(content, "];").unwrap();
    writeln!(content).unwrap();

    // Issue #208: parse the ISMCAT Tetragraph Taxonomy V2022-NOV. The
    // 24 `decomposable="Yes"` entries with materialized `<Country>`
    // members feed `TETRAGRAPH_MEMBERS`; the full 61-entry table feeds
    // `is_decomposable` (three-state discriminator) and
    // `TETRAGRAPH_PROVENANCE` (audit metadata). Extensions with
    // `members = […]` are appended to the membership table after the
    // taxonomy rows; extensions cannot shadow taxonomy codes because
    // `load_country_extensions` rejects any extension whose `code`
    // duplicates a CVE entry, and all 61 ISMCAT taxonomy codes appear
    // in `CVEnumISMCATRelTo.xsd`.
    let taxonomy_path = ismcat_root.join(ISMCAT_TETRA_RELPATH);
    let taxonomy = parse_tetragraph_taxonomy(&taxonomy_path);
    check_taxonomy_invariants(&taxonomy);

    // Emit the canonical tetragraph membership table consumed by
    // `marque-ism::page_context` and `marque-capco::vocab`. Sorted by
    // code for stable diffs and binary-search lookup.
    emit_tetragraph_members(&mut content, &taxonomy, &extensions);

    // Emit the three-state `is_decomposable(code)` discriminator (issue
    // #208) and the `TETRAGRAPH_PROVENANCE` audit table consumed by the
    // future `DecisionRecord` integration. Provenance struct must be
    // emitted before the static so the `&[TetragraphProvenance]` type
    // is in scope.
    emit_tax_provenance(&mut content, &taxonomy);
    emit_is_decomposable(&mut content, &taxonomy);

    // Emit `pub const ISMCAT_TETRA_VERSION` so consumers can check
    // which taxonomy snapshot the binary was built against (parallel
    // to `SCHEMA_VERSION`).
    writeln!(
        content,
        "/// ISMCAT Tetragraph Taxonomy version pinned at build time \
         (issue #208).\n\
         pub const ISMCAT_TETRA_VERSION: &str = {ISMCAT_TETRA_VERSION:?};"
    )
    .unwrap();
    writeln!(content).unwrap();

    // --- Emit a flat token list for the Aho-Corasick automaton ---
    //
    // H-8: deduplicate. The CVE files already contain NOFORN/ORCON/PROPIN/
    // IMCON in the dissem block, so the previous hand-rolled additions
    // produced duplicate entries that bloated the slice and the
    // automaton. We now collect into a `BTreeSet` (sorted, deduped) and
    // emit once. Sorting also lets `canonicalize` switch from O(n)
    // linear scan to O(log n) `binary_search` (see token_set.rs).
    let mut all_tokens: std::collections::BTreeSet<String> = std::collections::BTreeSet::new();
    for (value, _) in &class_entries {
        all_tokens.insert(value.clone());
    }
    // Full banner-word forms for classification (not present in the CVE,
    // which only ships single-letter abbreviations).
    for word in [
        "TOP SECRET",
        "SECRET",
        "CONFIDENTIAL",
        "RESTRICTED",
        "UNCLASSIFIED",
    ] {
        all_tokens.insert(word.to_owned());
    }
    for (value, _) in &sci_entries {
        all_tokens.insert(value.clone());
    }
    for (value, _) in &dissem_entries {
        all_tokens.insert(value.clone());
    }
    // SAR tokens intentionally absent: SAR identifiers are agency-assigned
    // codewords, not a closed CVE vocabulary. SAR is matched structurally by
    // the parser rather than via `ALL_CVE_TOKENS`.
    // Common expanded forms not in any CVE block. NOFORN/ORCON/PROPIN/
    // IMCON would normally live here, but the dissem CVE already covers
    // them, so the BTreeSet drops the duplicates automatically.
    all_tokens.insert("EYES ONLY".to_owned());
    // Issue #183 PR-B note: country-code extensions are NOT inserted
    // into `ALL_CVE_TOKENS`. CVE country codes themselves (`USA`,
    // `GBR`, `FVEY`, …) are not in this slice either — they live
    // exclusively in `TRIGRAPHS`, reached via `is_trigraph` from the
    // REL TO parser. Adding extensions here would give them
    // canonicalize / fuzzy-correction privileges that real CVE
    // trigraphs don't have, which would be an asymmetric and
    // surprising behavior change. Recognition flows through
    // `is_trigraph` for both CVE codes and extensions, uniformly.

    writeln!(
        content,
        "/// All known CVE tokens, sorted ascending and deduplicated.\n\
         /// Sorted order enables binary-search canonicalization in token_set.rs."
    )
    .unwrap();
    writeln!(content, "pub static ALL_CVE_TOKENS: &[&str] = &[").unwrap();
    for token in &all_tokens {
        writeln!(content, "    {token:?},").unwrap();
    }
    writeln!(content, "];").unwrap();
    writeln!(content).unwrap();

    // Schema version
    writeln!(
        content,
        "/// ISM schema version this crate was compiled against."
    )
    .unwrap();
    writeln!(
        content,
        "pub const SCHEMA_VERSION: &str = {SCHEMA_VERSION:?};"
    )
    .unwrap();

    let path = out.join("values.rs");
    fs::write(&path, &content)
        .unwrap_or_else(|e| panic!("failed to write {}: {e}", path.display()));
}

// ---------------------------------------------------------------------------
// T007: XSD trigraph parsing
// ---------------------------------------------------------------------------

fn parse_xsd_trigraphs(path: &Path) -> Vec<(String, String)> {
    let content = fs::read_to_string(path)
        .unwrap_or_else(|e| panic!("failed to read {}: {e}", path.display()));
    let mut reader = Reader::from_str(&content);
    let mut buf = Vec::new();

    let mut entries = Vec::new();

    loop {
        match reader.read_event_into(&mut buf) {
            Ok(Event::Start(ref e)) | Ok(Event::Empty(ref e)) => {
                let name = e.name();
                let local = local_name(name.as_ref());
                if local == b"enumeration" {
                    // Extract `value` attribute. We unescape XML entities for
                    // consistency with `parse_cve_xml`; trigraphs are pure
                    // ASCII today but a future XSD update could legitimately
                    // contain entities, and silent corruption here would
                    // produce wrong canonicalization at runtime.
                    for attr in e.attributes().flatten() {
                        if attr.key.as_ref() == b"value" {
                            let val = attr
                                .normalized_value(XmlVersion::Implicit1_0)
                                .unwrap_or_else(|err| {
                                    panic!(
                                        "XSD attribute unescape error in {}: {err}",
                                        path.display()
                                    )
                                });
                            entries.push((val.into_owned(), String::new()));
                            break;
                        }
                    }
                }
            }
            Ok(Event::Eof) => break,
            Err(e) => panic!("XSD parse error in {}: {e}", path.display()),
            _ => {}
        }
        buf.clear();
    }

    entries
}

// ---------------------------------------------------------------------------
// T008: Schematron → validator predicates
// ---------------------------------------------------------------------------

fn generate_validators(out: &Path, ism_root: &Path) {
    let _sch = ism_root.join("Schematron/ISM").join("ISM_XML.sch");
    let _lib = ism_root.join("Schematron/ISM").join("Lib");

    // Parse Schematron files for validation predicates.
    // Scope: fixed XPath vocabulary (attribute presence, equality, set membership).
    // For MVP, emit hand-derived predicates covering ~70% of assertions.
    // Full Schematron→Rust compilation is a post-MVP enhancement.
    let content = r#"
// Generated by build.rs from Schematron/ISM_XML.sch — DO NOT EDIT.
//
// Validation predicates derived from ODNI ISM Schematron assertions.
// Coverage: ~70% of fixed-vocabulary assertions. Complex XPath
// expressions (ancestor traversal, document-level cardinality) are
// deferred to post-MVP Schematron compilation.

// Note: we deliberately avoid importing a `Classification` type here. The
// crate has a hand-written `attrs::Classification`, and generating a second
// one from CVE XML would create a name collision inside the crate. The
// classification validator matches CVE abbreviations literally instead.
use crate::generated::values::{DissemControl, SciControl};

/// Returns true if the classification level string is a valid CVE value.
///
/// Accepts CVE abbreviations (R, U, C, S, TS). For banner-word validation,
/// see `banner_requires_full_classification`.
pub fn is_valid_classification(s: &str) -> bool {
    matches!(s, "R" | "U" | "C" | "S" | "TS")
}

/// Returns true if the SCI control string is a valid CVE value.
pub fn is_valid_sci_control(s: &str) -> bool {
    SciControl::parse(s).is_some()
}

/// Returns true if the dissemination control string is a valid CVE value.
pub fn is_valid_dissem_control(s: &str) -> bool {
    DissemControl::parse(s).is_some()
}

/// Returns true if the trigraph string is in the CVE country code list.
pub fn is_valid_trigraph(s: &str) -> bool {
    crate::generated::values::TRIGRAPHS.contains(&s)
}

/// Schematron assertion: NOFORN and REL TO are mutually exclusive.
pub fn noforn_rel_exclusive(has_noforn: bool, has_rel_to: bool) -> bool {
    !(has_noforn && has_rel_to)
}

/// Schematron assertion: REL TO requires at least one trigraph.
pub fn rel_to_requires_trigraph(has_rel: bool, trigraph_count: usize) -> bool {
    !has_rel || trigraph_count > 0
}

/// Schematron assertion: Banner must use full classification word.
pub fn banner_requires_full_classification(s: &str) -> bool {
    matches!(s, "UNCLASSIFIED" | "RESTRICTED" | "CONFIDENTIAL" | "SECRET" | "TOP SECRET")
}
"#;

    let path = out.join("validators.rs");
    fs::write(&path, content).unwrap_or_else(|e| panic!("failed to write {}: {e}", path.display()));
}

// ---------------------------------------------------------------------------
// T009: Deprecated marking migrations
// ---------------------------------------------------------------------------

fn generate_migrations(out: &Path, _ism_root: &Path) {
    // Deterministic deprecated-marking migration table.
    // Derived from CVE XML deprecation annotations and IC policy changes.
    // Confidence >= 0.95 per FR-004a.
    let content = r#"
// Generated by build.rs — DO NOT EDIT.
//
// Maps deprecated marking strings to their current replacements.
// Each entry carries a confidence score and a policy reference.

/// A deprecated marking migration entry.
pub struct MigrationEntry {
    /// The deprecated marking string.
    pub deprecated: &'static str,
    /// The replacement marking string.
    pub replacement: &'static str,
    /// Confidence score for auto-fix (0.0–1.0).
    pub confidence: f32,
    /// Policy reference (CAPCO section).
    pub reference: &'static str,
}

/// Deprecated-marking migration table.
///
/// Policy-driven replacements for markings that CAPCO has formally retired
/// or renamed. All entries have confidence >= 0.95 per FR-004a, and every
/// `reference` cites a real passage in `crates/capco/docs/CAPCO-2016.md`
/// per Constitution VIII.
///
/// # What is NOT a migration
///
/// - **Abbreviation ↔ banner form** (e.g., `NF` ↔ `NOFORN`): these are
///   the two authorized forms of the same marking, not a deprecation.
///   `E001 portion-mark-in-banner` owns the portion-form-in-banner check;
///   `E009 portion-abbreviation` owns the banner-form-in-portion check.
///   A prior `NF → NOFORN` entry was removed in T035c-4 as misleading —
///   it was filtered out by `E006 is_abbreviation_expansion` anyway.
///
/// - **FOUO → CUI**: FOUO remains a valid CAPCO dissem control per
///   CVEnumISMDissem.xml (still enumerated in the active CVE). CUI is a
///   separate marking system under NARA jurisdiction. A prior entry was
///   removed in Phase E of
///   `docs/plans/2026-04-19-recursive-lattice-and-decoder.md` (§14 "What
///   we dropped" — explicit `FOUO → CUI` bullet). Any "suggest CUI on
///   non-IC documents" behavior belongs in a future CUI adapter, gated
///   by `[agency] is_ic_member` / `[cui] migrate_fouo` config gates
///   (Phase F), not as a blanket CAPCO-level migration.
///
/// - **LIMDIS → RELIDO**: LIMDIS is a current non-IC dissem control
///   per CAPCO-2016 §H.9 (p18 of the 2008 manual, §H.9 of 2016). A
///   prior entry was incorrect.
pub static MIGRATIONS: &[MigrationEntry] = &[
    // X-shorthand date marking patterns (FR-004a, research R-3).
    //
    // CAPCO-2016 §E.6 "Retired or Invalid Declassify On Values"
    // (pp. 33-34) enumerates retired exemption forms. The substantive
    // passages about the migrations below are on p34: the "25X1 -
    // human" → "50X1 - HUM" replacement (retired by ISOO Notice
    // 2012-02) and the "25X1" through "25X9" without-date-or-event
    // exemption guidance both appear there. p33 is the section
    // heading and an unrelated bullet about OADR. The migrations
    // below catch specific corrupt/truncated forms (trailing hyphen,
    // no suffix) that can appear in real documents and rewrite them
    // to canonical forms. E007 `XShorthandDateRule` handles the
    // broader pattern-based cases.
    MigrationEntry {
        deprecated: "25X1-",
        replacement: "25X1",
        confidence: 0.97,
        reference: "CAPCO-2016 §E.6 p34",
    },
    MigrationEntry {
        deprecated: "50X1-",
        replacement: "50X1-HUM",
        confidence: 0.95,
        // §E.6 p34: "The derivative classifier must not carry forward
        // the 25X1-human declassification instruction from the source
        // document; but instead, derivative classifiers should use the
        // '50X1 - HUM' marking." The trailing-hyphen form `50X1-` is
        // canonicalized to the full replacement form.
        reference: "CAPCO-2016 §E.6 p34",
    },
];

/// Look up a deprecated marking. Returns the migration entry if found.
pub fn find_migration(deprecated: &str) -> Option<&'static MigrationEntry> {
    MIGRATIONS.iter().find(|m| m.deprecated == deprecated)
}
"#;

    let path = out.join("migrations.rs");
    fs::write(&path, content).unwrap_or_else(|e| panic!("failed to write {}: {e}", path.display()));
}

// ---------------------------------------------------------------------------
// T080 / T081: Per-token metadata from ODNI JSON sidecars
// ---------------------------------------------------------------------------
//
// The XML codepath above (T006) extracts the closed CVE token vocabulary —
// that is what the strict parser, the corrections map, and the rule
// predicates consume. The JSON codepath here is parallel and additive
// (FR-018 + R5): it reads the ODNI JSON sidecars in the same CVE_ISM/
// directory to recover *per-term metadata* the XML never carried —
// publishing authority (URN, source, schema version, point of contact)
// and the long-form description that pairs with each canonical value.
//
// The XML and JSON codepaths are both active. Neither falls back to the
// other. If a CVE file exists in JSON but not XML, the strict parser
// will not recognize the values — even though the metadata table will
// surface them; that is a deliberate split because the strict parser is
// the sole arbiter of token shape (foundational invariant from PR-3),
// and a JSON-only token would bypass that invariant.
//
// The emitted tables are the raw data backing PR-2's
// `impl Vocabulary<CapcoScheme>` (task T084). Per Constitution VII the
// `marque-scheme` `Vocabulary<S>` / `TokenMetadataFull<Token>` types
// cannot be referenced from this crate — `marque-ism` does not depend
// on `marque-scheme`. The composition into the trait surface happens in
// `marque-capco` where both dependency chains converge.

/// Per-CVE-file metadata extracted from `CVE.IRM` and the file-level
/// `CVE.specVersion` / `CVE.ism:DESVersion` fields.
struct CveFileMetadata {
    /// Stable identifier for the per-file constant emitted into
    /// `vocabulary.rs` (e.g., `CVE_DISSEM`, `CVE_SCI_CONTROLS`). Derived
    /// by stripping the `CVEnumISM` / `CVEnum` prefix and the `.json`
    /// suffix, then `to_rust_ident` + `SCREAMING_SNAKE_CASE`.
    const_ident: String,
    urn: String,
    title: String,
    source: String,
    poc_name: String,
    poc_email: String,
    owner_producer: String,
    spec_version: String,
    des_version: String,
}

/// One token's metadata: canonical value + long-form description, paired
/// with a reference to the [`CveFileMetadata`] that published it.
struct TokenMetadataEntry {
    value: String,
    description: String,
    /// Const identifier of the emitted CVE file metadata item (for example,
    /// `CVE_DISSEM`); resolved during codegen when generating references.
    cve_file_const_ident: String,
}

/// Read the entire CVE_ISM/ JSON sidecar set; return both the per-file
/// records and the flat (token, metadata) map.
///
/// JSON shape (matches every file in the v2022-DEC bundle):
///
/// ```jsonc
/// {
///   "CVE": {
///     "ism:ownerProducer": "USA",
///     "specVersion": "...",
///     "ism:DESVersion": "...",
///     "Enumeration": {
///       "Term": [ { "Description": { "text": "..." }, "Value": { "text": "..." } }, ... ]
///       // OR a single object when there is exactly one term — handled below.
///     },
///     "IRM": {
///       "URN": "urn:us:gov:ic:cvenum:ism:...",
///       "Title": { "text": "..." },
///       "Source": { "text": "..." },
///       "PointOfContact": { "Name": "...", "Email": "..." }
///     }
///   }
/// }
/// ```
fn collect_cve_metadata(cve_dir: &Path) -> (Vec<CveFileMetadata>, Vec<TokenMetadataEntry>) {
    let mut files: Vec<CveFileMetadata> = Vec::new();
    let mut tokens: Vec<TokenMetadataEntry> = Vec::new();

    let entries = fs::read_dir(cve_dir)
        .unwrap_or_else(|e| panic!("failed to read {}: {e}", cve_dir.display()));

    // Fail loudly on a per-entry I/O error rather than silently
    // dropping the bad entry — a transient `read_dir` failure should
    // not produce an incomplete vocabulary table that compiles cleanly.
    let mut paths: Vec<std::path::PathBuf> = entries
        .map(|entry| {
            entry
                .unwrap_or_else(|e| panic!("failed to read entry in {}: {e}", cve_dir.display()))
                .path()
        })
        .filter(|path| path.extension().and_then(|s| s.to_str()) == Some("json"))
        .collect();
    // Deterministic emission order so downstream binary search and
    // git-diff readability are stable across rebuilds.
    paths.sort();

    for path in &paths {
        let raw = fs::read_to_string(path)
            .unwrap_or_else(|e| panic!("failed to read {}: {e}", path.display()));
        let value: serde_json::Value = serde_json::from_str(&raw)
            .unwrap_or_else(|e| panic!("JSON parse error in {}: {e}", path.display()));

        let cve = value
            .get("CVE")
            .unwrap_or_else(|| panic!("{}: top-level `CVE` object missing", path.display()));

        let const_ident = cve_file_const_ident(path);

        let irm = cve
            .get("IRM")
            .unwrap_or_else(|| panic!("{}: CVE.IRM missing", path.display()));

        // Provenance fields that audit consumers and rule authority
        // citations depend on (`owner_producer`, POC name + email,
        // spec/DES versions). Per Constitution VIII these MUST fail
        // closed at `cargo build` if a sidecar file ships without
        // them — silently emitting an empty `&'static str` would let
        // a missing-data regression slip past `cargo build` and only
        // fire at `cargo test`, which is too late for downstream
        // CI / release pipelines that build but don't always test.
        //
        // `title` and `source` stay optional: ODNI does not guarantee
        // them on every CVE file (some retired files ship with no
        // free-form Title block) and they're informational only —
        // they don't appear in audit records or rule citations.
        let poc_obj = irm.get("PointOfContact").unwrap_or_else(|| {
            panic!(
                "{}: IRM.PointOfContact missing — every ODNI CVE file \
                 carries a PointOfContact block per Constitution VIII",
                path.display()
            )
        });
        let file_meta = CveFileMetadata {
            const_ident: const_ident.clone(),
            urn: required_string(irm, "URN", path),
            title: nested_text(irm, "Title", path).unwrap_or_default(),
            source: nested_text(irm, "Source", path).unwrap_or_default(),
            poc_name: required_nested_text(poc_obj, "Name", path),
            poc_email: required_nested_text(poc_obj, "Email", path),
            owner_producer: required_string(cve, "ism:ownerProducer", path),
            spec_version: required_string(cve, "specVersion", path),
            des_version: required_string(cve, "ism:DESVersion", path),
        };

        // Walk the term list. The JSON shape *should* be an array; we
        // accept a single-object form too because nothing in the JSON
        // spec forbids it and a future CVE file with one term might
        // deserialize that way.
        let term_value = cve.get("Enumeration").and_then(|e| e.get("Term"));
        let term_iter: Vec<&serde_json::Value> = match term_value {
            Some(serde_json::Value::Array(arr)) => arr.iter().collect(),
            Some(obj @ serde_json::Value::Object(_)) => vec![obj],
            None => Vec::new(), // SAR is legitimately empty (matches XML codepath).
            Some(other) => panic!(
                "{}: CVE.Enumeration.Term has unexpected JSON type {:?}",
                path.display(),
                other
            ),
        };

        for term in term_iter {
            let value_text = nested_text(term, "Value", path).unwrap_or_else(|| {
                panic!(
                    "{}: term missing Value.text — every CVE term must carry a canonical value",
                    path.display()
                )
            });
            // Description is intentionally optional. SAR and a handful
            // of NTK CVE entries omit `Description`; defaulting to an
            // empty string here matches the field's documented
            // contract on `TokenMetadataEntry::description` ("Empty
            // when the source CVE file did not provide a description").
            let desc_text = nested_text(term, "Description", path).unwrap_or_default();
            tokens.push(TokenMetadataEntry {
                value: value_text,
                description: desc_text,
                cve_file_const_ident: const_ident.clone(),
            });
        }

        files.push(file_meta);
    }

    (files, tokens)
}

/// Extract `obj.{field}.text` (the standard ODNI JSON shape for
/// human-readable text fields). Returns `None` when the path is absent.
///
/// Handles three observed shapes:
/// - Bare string (`"field": "value"`)
/// - Object with text field (`"field": { "text": "value", ... }`)
/// - Array of objects (`"field": [ { "text": "a", ... }, { "text": "b", ... } ]`)
///   joined with `"; "` so multi-source IRM Source fields (e.g.,
///   `CVEnumISMNotice.json`) round-trip into a readable single string.
fn nested_text(obj: &serde_json::Value, field: &str, path: &Path) -> Option<String> {
    fn extract_text(val: &serde_json::Value) -> Option<String> {
        match val {
            serde_json::Value::String(s) => Some(s.to_owned()),
            serde_json::Value::Object(map) => map
                .get("text")
                .and_then(|v| v.as_str())
                .map(|s| s.to_owned()),
            _ => None,
        }
    }

    match obj.get(field) {
        Some(serde_json::Value::Array(arr)) => {
            let parts: Vec<String> = arr.iter().filter_map(extract_text).collect();
            if parts.is_empty() {
                None
            } else {
                Some(parts.join("; "))
            }
        }
        Some(v @ (serde_json::Value::Object(_) | serde_json::Value::String(_))) => extract_text(v),
        Some(other) => panic!(
            "{}: field `{field}` has unexpected JSON type {:?}",
            path.display(),
            other
        ),
        None => None,
    }
}

/// Build-time `unwrap_or_else` panic for a required JSON string
/// field. Rejects three failure modes:
/// 1. Field absent (`obj.get(field) == None`).
/// 2. Field present but not a JSON string (number / object / null).
/// 3. Field present, type-correct, but empty or whitespace-only.
///
/// The third case is the one a Copilot review on PR #152 caught:
/// without it, an empty-but-present sidecar field (e.g.,
/// `"poc_email": ""`) would compile cleanly and emit `&'static ""` into
/// the generated `TokenMetadataFull` table — which is exactly the
/// silent-fallback failure mode `required_string` was added to
/// prevent (Constitution VIII fail-closed). Audit records that cite
/// an empty `poc_email` carry the same provenance defect as audit
/// records that cite a missing one.
fn required_string(obj: &serde_json::Value, field: &str, path: &Path) -> String {
    let raw = obj.get(field).and_then(|v| v.as_str()).unwrap_or_else(|| {
        panic!(
            "{}: required field `{field}` missing or not a string",
            path.display()
        )
    });
    if raw.trim().is_empty() {
        panic!(
            "{}: required field `{field}` is present but empty (or \
             whitespace-only); ODNI provenance fields must carry a real \
             value. Update the JSON sidecar.",
            path.display()
        );
    }
    raw.to_owned()
}

/// Like [`nested_text`] but panics on absence — the build-time
/// fail-closed companion. Use when a field is required by ODNI's CVE
/// schema and silent absence would corrupt audit-record provenance
/// (Constitution VIII).
///
/// Symmetric with [`required_string`]: rejects absent / wrong-type /
/// empty-or-whitespace. `nested_text` itself yields `Some("")` when
/// the source object's `text` member is an empty string, which would
/// otherwise slip past the absence check unchanged.
fn required_nested_text(obj: &serde_json::Value, field: &str, path: &Path) -> String {
    let raw = nested_text(obj, field, path).unwrap_or_else(|| {
        panic!(
            "{}: required nested-text field `{field}` missing — expected \
             either `{{\"{field}\": \"...\"}}` or `{{\"{field}\": {{\"text\": \"...\"}}}}`",
            path.display()
        )
    });
    if raw.trim().is_empty() {
        panic!(
            "{}: required nested-text field `{field}` is present but \
             empty (or whitespace-only). ODNI provenance fields must \
             carry a real value.",
            path.display()
        );
    }
    raw
}

/// `CVEnumISMDissem.json` → `CVE_DISSEM`, `CVEnumISMSCIControls.json` →
/// `CVE_SCI_CONTROLS`, `CVEnumISMCUIBasic.json` → `CVE_CUI_BASIC`,
/// `CVEnumISMAtomicEnergyMarkings.json` → `CVE_ATOMIC_ENERGY_MARKINGS`,
/// `CVEnumISM25X.json` → `CVE_25X`,
/// `CVEnumNTKAccessPolicy.json` → `CVE_NTK_ACCESS_POLICY`.
fn cve_file_const_ident(path: &Path) -> String {
    let stem = path
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or_else(|| panic!("non-UTF-8 file name: {}", path.display()));

    // Strip the `CVEnumISM` / `CVEnum` prefix.
    let core = stem
        .strip_prefix("CVEnumISM")
        .or_else(|| stem.strip_prefix("CVEnum"))
        .unwrap_or(stem);

    // Convert CamelCase / PascalCase → SCREAMING_SNAKE_CASE.
    // Boundary rules:
    //   - lower → upper (`AtomicE` → `ATOMIC_E`)
    //   - upper-run end → next word (`SCICont` → `SCI_CONT`: the `C`
    //     before lowercase `o` starts a new word)
    //   - digit + uppercase stays in the same word (`25X` → `25X`,
    //     not `25_X`) so generated const names match the documented
    //     examples — `CVEnumISM25X.json` → `CVE_25X`.
    let chars: Vec<char> = core.chars().collect();
    let mut body = String::with_capacity(core.len() * 2);
    for (i, &ch) in chars.iter().enumerate() {
        if i > 0 {
            let prev = chars[i - 1];
            let starts_word_by_case = ch.is_ascii_uppercase() && prev.is_ascii_lowercase();
            let ends_uppercase_run = ch.is_ascii_uppercase()
                && prev.is_ascii_uppercase()
                && i + 1 < chars.len()
                && chars[i + 1].is_ascii_lowercase();
            if starts_word_by_case || ends_uppercase_run {
                body.push('_');
            }
        }
        body.push(ch.to_ascii_uppercase());
    }

    if body.is_empty() {
        "CVE".to_owned()
    } else {
        format!("CVE_{body}")
    }
}

fn generate_vocabulary(out: &Path, ism_root: &Path) {
    use std::collections::BTreeMap;
    use std::fmt::Write;

    let cve_dir = ism_root.join("CVE/ISM");
    let (files, tokens) = collect_cve_metadata(&cve_dir);

    assert!(
        !files.is_empty(),
        "build.rs: collect_cve_metadata produced zero CveFileMetadata records — \
         the JSON sidecar set under {} is missing or unreadable.",
        cve_dir.display()
    );

    // Deduplicate tokens by value. The CVE_ISM/ JSON set has overlap
    // (e.g., FOUO appears in CVEnumISMDissem.json AND CVEnumISMNotice
    // does not, but classification levels appear in both
    // ClassificationAll and ClassificationUS). When two files publish
    // the same canonical value, prefer the more specific source — for
    // now, deterministic "first-wins by sorted file path" matches the
    // existing XML codepath's behavior, which iterates files in the
    // same order. Future PR can introduce an explicit precedence rule
    // if a real conflict arises that "first-wins" gets wrong.
    let mut by_value: BTreeMap<String, &TokenMetadataEntry> = BTreeMap::new();
    for entry in &tokens {
        by_value.entry(entry.value.clone()).or_insert(entry);
    }

    let mut content =
        String::from("// Generated by build.rs from ODNI ISM JSON sidecars — DO NOT EDIT.\n\n");

    writeln!(
        content,
        "/// Per-CVE-file publishing metadata.\n\
         ///\n\
         /// One instance per `CVEnum*.json` file under\n\
         /// `ism::package_root().join(\"CVE/ISM\")` (vendored ISM data).\n\
         /// Every token-level\n\
         /// metadata entry references one of these so the same authority,\n\
         /// point of contact, and schema-version provenance is shared\n\
         /// across all tokens published in that file.\n\
         #[derive(Debug, Clone, Copy)]\n\
         pub struct CveFileMetadata {{\n\
            /// Symbolic constant name (e.g., \"CVE_DISSEM\").\n\
            pub const_name: &'static str,\n\
            /// Source-of-record URN, e.g.,\n\
            /// `urn:us:gov:ic:cvenum:ism:dissem`.\n\
            pub urn: &'static str,\n\
            /// CVE title text.\n\
            pub title: &'static str,\n\
            /// Free-form `Source` text from the CVE IRM.\n\
            pub source: &'static str,\n\
            /// Point-of-contact name.\n\
            pub poc_name: &'static str,\n\
            /// Point-of-contact email.\n\
            pub poc_email: &'static str,\n\
            /// Owner/producer code, e.g., `\"USA\"`.\n\
            pub owner_producer: &'static str,\n\
            /// CVE `specVersion`, e.g., `\"202111.202211\"`.\n\
            pub spec_version: &'static str,\n\
            /// CVE `ism:DESVersion`, e.g., `\"202111\"`.\n\
            pub des_version: &'static str,\n\
            /// Pinned schema package version (`SCHEMA_VERSION`).\n\
            pub schema_version: &'static str,\n\
         }}\n"
    )
    .unwrap();

    writeln!(
        content,
        "/// Per-token metadata published by exactly one [`CveFileMetadata`].\n\
         #[derive(Debug, Clone, Copy)]\n\
         pub struct TokenMetadataEntry {{\n\
         \x20   /// Canonical CVE value, e.g., `\"NOFORN\"` or `\"S\"`.\n\
         \x20   pub value: &'static str,\n\
         \x20   /// Long-form description, e.g., `\"NOT RELEASABLE TO\n\
         \x20   /// FOREIGN NATIONALS\"`. Empty when the source CVE file\n\
         \x20   /// did not provide a description.\n\
         \x20   pub description: &'static str,\n\
         \x20   /// CVE file that published this token.\n\
         \x20   pub cve_file: &'static CveFileMetadata,\n\
         }}\n"
    )
    .unwrap();

    // Emit one `pub static` per CVE file.
    //
    // `f.const_ident` is reused for both the static identifier and
    // the `const_name` field — they're the SAME string by construction
    // here, which keeps the field-vs-binding round-trip
    // (`metadata.cve_file.const_name == "CVE_DISSEM"` for `&CVE_DISSEM`)
    // a structural invariant rather than a coincidence. The
    // `every_token_references_a_known_cve_file` test in
    // `crates/ism/tests/vocabulary_tables.rs` enforces it.
    for f in &files {
        writeln!(content, "/// CVE-file metadata for `{}`.", f.const_ident).unwrap();
        writeln!(
            content,
            "pub static {ident}: CveFileMetadata = CveFileMetadata {{\n\
             \x20   const_name: {name:?},\n\
             \x20   urn: {urn:?},\n\
             \x20   title: {title:?},\n\
             \x20   source: {source:?},\n\
             \x20   poc_name: {poc_name:?},\n\
             \x20   poc_email: {poc_email:?},\n\
             \x20   owner_producer: {owner_producer:?},\n\
             \x20   spec_version: {spec_version:?},\n\
             \x20   des_version: {des_version:?},\n\
             \x20   schema_version: {schema_version:?},\n\
             }};\n",
            ident = f.const_ident,
            name = f.const_ident,
            urn = f.urn,
            title = f.title,
            source = f.source,
            poc_name = f.poc_name,
            poc_email = f.poc_email,
            owner_producer = f.owner_producer,
            spec_version = f.spec_version,
            des_version = f.des_version,
            schema_version = SCHEMA_VERSION,
        )
        .unwrap();
    }

    // Slice of every CVE file metadata record, in deterministic order.
    writeln!(
        content,
        "/// Every CVE-file metadata record published by this build.\n\
         pub static CVE_FILES: &[&CveFileMetadata] = &["
    )
    .unwrap();
    for f in &files {
        writeln!(content, "    &{},", f.const_ident).unwrap();
    }
    writeln!(content, "];\n").unwrap();

    // Sorted-by-value token table, paired with its CVE file metadata.
    // BTreeMap iteration is sorted, so the emitted slice is sorted too —
    // `lookup_token_metadata` can use `binary_search_by_key`.
    writeln!(
        content,
        "/// Sorted (by canonical value) per-token metadata entries.\n\
         /// Use [`lookup_token_metadata`] for O(log n) name → metadata\n\
         /// lookup.\n\
         pub static TOKEN_METADATA: &[TokenMetadataEntry] = &["
    )
    .unwrap();
    for (value, entry) in &by_value {
        writeln!(
            content,
            "    TokenMetadataEntry {{ value: {value:?}, description: {desc:?}, cve_file: &{cve_file} }},",
            value = value,
            desc = entry.description,
            cve_file = entry.cve_file_const_ident,
        )
        .unwrap();
    }
    writeln!(content, "];\n").unwrap();

    // Lookup helper.
    writeln!(
        content,
        "/// Look up per-token metadata by canonical CVE value.\n\
         /// Returns `None` for values not published by any CVE file in\n\
         /// the active schema package.\n\
         pub fn lookup_token_metadata(value: &str) -> Option<&'static TokenMetadataEntry> {{\n\
         \x20   TOKEN_METADATA\n\
         \x20       .binary_search_by_key(&value, |e| e.value)\n\
         \x20       .ok()\n\
         \x20       .map(|i| &TOKEN_METADATA[i])\n\
         }}\n"
    )
    .unwrap();

    let path = out.join("vocabulary.rs");
    fs::write(&path, &content)
        .unwrap_or_else(|e| panic!("failed to write {}: {e}", path.display()));
}

// ---------------------------------------------------------------------------
// Issue #208: ISMCAT Tetragraph Taxonomy V2022-NOV — build-time parsing
// ---------------------------------------------------------------------------
//
// Replaces the previous hand-curated `BUILTIN_TETRAGRAPH_MEMBERS` slice with
// data sourced from the ODNI taxonomy in the `ism-ismcat` crate at
// `ism_ismcat::package_root().join("Taxonomy/ISMCAT/TetragraphTaxonomyDenormalized.xml")`.
// The same data drives three generated artifacts:
//
//   - `TETRAGRAPH_MEMBERS` / `lookup_tetragraph_members` — country lists
//     for the 24 `decomposable="Yes"` codes (used by REL TO intersection).
//   - `is_decomposable(code)` — three-state ODNI-authoritative flag
//     consumed by issue #206's S005 rule (silent-loss diagnostic).
//   - `TETRAGRAPH_PROVENANCE` — full audit row preserving the
//     `decomposable` value, membership shape, deprecation date, and
//     `dateLastVerified`. Reserved for the `DecisionRecord` work in the
//     2026-04-20 roadmap; not yet exposed publicly.
//
// We parse only the **denormalized** form. The canonical hierarchical
// `TetragraphTaxonomy.xml` is vendored alongside for divergence detection
// (Constitution Principle VIII — authoritative source fidelity); a future
// taxonomy revision that introduces unexpected `<Organization>` refs in
// the denormalized file is caught at build time by guard #4 below.

const ISMCAT_TETRA_VERSION: &str = "2022-NOV";
/// Relative path under `ism_ismcat::package_root()` to the denormalized
/// ISMCAT Tetragraph Taxonomy XML.
const ISMCAT_TETRA_RELPATH: &str = "Taxonomy/ISMCAT/TetragraphTaxonomyDenormalized.xml";

/// Mirrors the XSD `DecomposableType` enumeration. Three-state, not four:
/// `NA` is documented as "applied to deprecated tetragraphs" — every NA
/// entry in V2022-NOV also carries a `deprecated="YYYY-MM-DD"` attribute.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum TaxDecomposable {
    Yes,
    No,
    Na,
}

impl TaxDecomposable {
    fn as_xml(self) -> &'static str {
        match self {
            Self::Yes => "Yes",
            Self::No => "No",
            Self::Na => "NA",
        }
    }
}

/// Mirrors the `<Membership>` `xs:choice` from `Tetragraph.xsd` exactly.
/// The XSD guarantees exactly one variant is populated per entry; the
/// parser asserts that invariant and panics on violation.
#[derive(Debug)]
enum TaxMembership {
    /// One-or-more `<Country>` and/or `<Organization>` children.
    /// `recursive == true` if any `<Organization>` reference appears
    /// (the BHTF case in V2022-NOV — NA-deprecated, runtime-inert).
    /// Organization identifiers are not persisted yet; add them when
    /// organization-aware diagnostics are implemented.
    Members {
        countries: Vec<String>,
        recursive: bool,
    },
    /// `<Description>` free text — typically an OCA-deferral pointer.
    /// Retained verbatim so issue #206's S005 emitter can quote it
    /// without re-parsing the XML at runtime; surfaced through
    /// [`TETRAGRAPH_PROVENANCE`].
    Description(String),
    /// `<MembershipSupressed/>` sentinel (ODNI's spelling — single `p`).
    Suppressed,
}

impl TaxMembership {
    fn shape_label(&self) -> &'static str {
        match self {
            Self::Members {
                recursive: true, ..
            } => "Members(recursive)",
            Self::Members { .. } => "Members",
            Self::Description(_) => "Description",
            Self::Suppressed => "Suppressed",
        }
    }
}

/// One parsed `<Tetragraph>` entry.
#[derive(Debug)]
struct TaxEntry {
    code: String,
    decomposable: TaxDecomposable,
    /// Always `Some` for `decomposable == Na` in V2022-NOV; `None`
    /// elsewhere. Stored verbatim from the XSD `xs:date` attribute (no
    /// jiff dep in build.rs); the four-guard pass below validates the
    /// 1:1 NA-deprecated invariant.
    deprecated: Option<String>,
    /// `<Membership dateLastVerified="YYYY-MM-DD">` — required by XSD.
    last_verified: String,
    membership: TaxMembership,
}

/// Read [`package.metadata.marque`] `ismcat-tetra-version` from
/// `Cargo.toml` and panic on mismatch with [`ISMCAT_TETRA_VERSION`].
/// Constitution Principle IV — schema versions are pinned in cargo
/// metadata and bumped intentionally.
fn verify_ismcat_tetra_version() {
    let cargo_toml = fs::read_to_string("Cargo.toml").expect("failed to read Cargo.toml");
    let table: toml::Table = cargo_toml
        .parse()
        .expect("failed to parse Cargo.toml as TOML");

    let pinned = table
        .get("package")
        .and_then(|p| p.get("metadata"))
        .and_then(|m| m.get("marque"))
        .and_then(|m| m.get("ismcat-tetra-version"))
        .and_then(|v| v.as_str())
        .unwrap_or_else(|| {
            panic!(
                "issue #208: [package.metadata.marque] ismcat-tetra-version not found in \
                 Cargo.toml. Add: ismcat-tetra-version = \"{ISMCAT_TETRA_VERSION}\""
            )
        });

    assert_eq!(
        pinned, ISMCAT_TETRA_VERSION,
        "issue #208: ISMCAT taxonomy version mismatch — Cargo.toml says \
         {pinned:?} but build.rs targets {ISMCAT_TETRA_VERSION:?}. Update \
         one to match the other (and re-vendor the taxonomy XML if bumping)."
    );
}

/// Parse the ISMCAT denormalized taxonomy XML into a `Vec<TaxEntry>` in
/// document order. Panics on malformed XML or XSD violations the
/// downstream emitters depend on (every `<Tetragraph>` carries a
/// `decomposable` attribute, a `<TetraToken>`, and a `<Membership>`
/// with exactly one populated `xs:choice` branch).
fn parse_tetragraph_taxonomy(path: &Path) -> Vec<TaxEntry> {
    let content = fs::read_to_string(path)
        .unwrap_or_else(|e| panic!("failed to read {}: {e}", path.display()));
    let mut reader = Reader::from_str(&content);
    let mut buf = Vec::new();

    let mut entries: Vec<TaxEntry> = Vec::new();

    let mut in_tetragraph = false;
    let mut current_decomposable: Option<TaxDecomposable> = None;
    let mut current_deprecated: Option<String> = None;

    let mut in_tetra_token = false;
    let mut current_token = String::new();

    let mut in_membership = false;
    let mut current_last_verified: Option<String> = None;
    let mut current_countries: Vec<String> = Vec::new();
    let mut current_organizations: Vec<String> = Vec::new();
    let mut current_membership_description: Option<String> = None;
    let mut current_suppressed = false;

    let mut in_country = false;
    let mut current_country = String::new();

    let mut in_organization = false;
    let mut current_organization = String::new();

    let mut in_membership_description = false;
    let mut current_description = String::new();

    loop {
        match reader.read_event_into(&mut buf) {
            Ok(Event::Start(ref e)) => {
                let local = local_name(e.name().as_ref()).to_owned();
                match local.as_slice() {
                    b"Tetragraph" => {
                        in_tetragraph = true;
                        current_decomposable = None;
                        current_deprecated = None;
                        current_token.clear();
                        current_last_verified = None;
                        current_countries.clear();
                        current_organizations.clear();
                        current_membership_description = None;
                        current_suppressed = false;
                        for attr_res in e.attributes() {
                            let attr = attr_res.unwrap_or_else(|err| {
                                panic!(
                                    "{}: attribute parse error in <Tetragraph>: {err}",
                                    path.display()
                                )
                            });
                            let key_local = local_name(attr.key.as_ref()).to_owned();
                            match key_local.as_slice() {
                                b"decomposable" => {
                                    let value = attr
                                        .normalized_value(XmlVersion::Implicit1_0)
                                        .unwrap_or_else(|err| {
                                            panic!(
                                                "{}: failed to unescape `decomposable`: {err}",
                                                path.display()
                                            )
                                        });
                                    current_decomposable = Some(match value.as_ref() {
                                        "Yes" => TaxDecomposable::Yes,
                                        "No" => TaxDecomposable::No,
                                        "NA" => TaxDecomposable::Na,
                                        other => panic!(
                                            "{}: unknown decomposable value {other:?} \
                                             (expected Yes|No|NA per Tetragraph.xsd)",
                                            path.display()
                                        ),
                                    });
                                }
                                b"deprecated" => {
                                    let value = attr
                                        .normalized_value(XmlVersion::Implicit1_0)
                                        .unwrap_or_else(|err| {
                                            panic!(
                                                "{}: failed to unescape `deprecated`: {err}",
                                                path.display()
                                            )
                                        });
                                    current_deprecated = Some(value.into_owned());
                                }
                                _ => {}
                            }
                        }
                    }
                    b"TetraToken" if in_tetragraph => in_tetra_token = true,
                    b"Membership" if in_tetragraph => {
                        in_membership = true;
                        for attr_res in e.attributes() {
                            let attr = attr_res.unwrap_or_else(|err| {
                                panic!(
                                    "{}: attribute parse error in <Membership>: {err}",
                                    path.display()
                                )
                            });
                            if local_name(attr.key.as_ref()) == b"dateLastVerified" {
                                let value = attr
                                    .normalized_value(XmlVersion::Implicit1_0)
                                    .unwrap_or_else(|err| {
                                        panic!(
                                            "{}: failed to unescape `dateLastVerified`: {err}",
                                            path.display()
                                        )
                                    });
                                current_last_verified = Some(value.into_owned());
                            }
                        }
                    }
                    b"Country" if in_membership => in_country = true,
                    b"Organization" if in_membership => in_organization = true,
                    b"Description" if in_membership => in_membership_description = true,
                    // spellchecker:off
                    // <MembershipSupressed> (note ODNI's misspelling — single
                    // `p`). The taxonomy ships it as a self-closing
                    // `<MembershipSupressed/>`, but XML allows the equivalent
                    // `<MembershipSupressed></MembershipSupressed>` Start+End
                    // form, and a future ODNI tool that round-trips through a
                    // generic XML library could emit either. Set the flag on
                    // both Start and Empty (the matching End arm below is a
                    // no-op for this element since we don't track an
                    // `in_suppressed` state — there's nothing inside).
                    b"MembershipSupressed" if in_membership => {
                        current_suppressed = true;
                    }
                    // spellchecker:on
                    _ => {}
                }
            }
            Ok(Event::Empty(ref e))
                if in_membership && local_name(e.name().as_ref()) == b"MembershipSupressed" =>
            {
                current_suppressed = true;
            }
            Ok(Event::End(ref e)) => {
                let local = local_name(e.name().as_ref()).to_owned();
                match local.as_slice() {
                    b"Tetragraph" => {
                        if in_tetragraph {
                            let token = current_token.trim().to_owned();
                            if token.is_empty() {
                                panic!("{}: <Tetragraph> with empty <TetraToken>", path.display());
                            }
                            let decomposable = current_decomposable.unwrap_or_else(|| {
                                panic!(
                                    "{}: <Tetragraph> {token:?} missing required \
                                     `decomposable` attribute",
                                    path.display()
                                )
                            });
                            let last_verified =
                                current_last_verified.clone().unwrap_or_else(|| {
                                    panic!(
                                        "{}: <Tetragraph> {token:?} missing required \
                                         <Membership dateLastVerified>",
                                        path.display()
                                    )
                                });

                            // Resolve <Membership> xs:choice. Schema validity
                            // requires exactly one branch populated; we panic
                            // on violation rather than silently picking a
                            // default.
                            let has_members =
                                !current_countries.is_empty() || !current_organizations.is_empty();
                            let has_description = current_membership_description.is_some();
                            let has_suppressed = current_suppressed;
                            let populated = (has_members as u8)
                                + (has_description as u8)
                                + (has_suppressed as u8);
                            if populated != 1 {
                                panic!(
                                    "{}: <Tetragraph> {token:?} <Membership> violates \
                                     xs:choice — {populated} branches populated \
                                     (members={has_members}, \
                                     description={has_description}, \
                                     suppressed={has_suppressed})",
                                    path.display()
                                );
                            }
                            let membership = if has_suppressed {
                                TaxMembership::Suppressed
                            } else if let Some(desc) = current_membership_description.take() {
                                TaxMembership::Description(desc.trim().to_owned())
                            } else {
                                let recursive = !current_organizations.is_empty();
                                TaxMembership::Members {
                                    countries: std::mem::take(&mut current_countries),
                                    recursive,
                                }
                            };

                            entries.push(TaxEntry {
                                code: token,
                                decomposable,
                                deprecated: current_deprecated.take(),
                                last_verified,
                                membership,
                            });
                        }
                        in_tetragraph = false;
                    }
                    b"TetraToken" => in_tetra_token = false,
                    b"Membership" => in_membership = false,
                    b"Country" => {
                        if in_country {
                            let val = current_country.trim().to_owned();
                            if !val.is_empty() {
                                current_countries.push(val);
                            }
                        }
                        current_country.clear();
                        in_country = false;
                    }
                    b"Organization" => {
                        if in_organization {
                            let val = current_organization.trim().to_owned();
                            if !val.is_empty() {
                                current_organizations.push(val);
                            }
                        }
                        current_organization.clear();
                        in_organization = false;
                    }
                    b"Description" => {
                        if in_membership_description {
                            current_membership_description = Some(current_description.clone());
                            current_description.clear();
                        }
                        in_membership_description = false;
                    }
                    _ => {}
                }
            }
            Ok(Event::Text(ref e)) => {
                let decoded = e.decode().unwrap_or_else(|err| {
                    panic!("XML entity unescape error in {}: {err}", path.display())
                });
                if in_tetra_token {
                    current_token.push_str(&decoded);
                } else if in_country {
                    current_country.push_str(&decoded);
                } else if in_organization {
                    current_organization.push_str(&decoded);
                } else if in_membership_description {
                    current_description.push_str(&decoded);
                }
            }
            Ok(Event::Eof) => break,
            Err(e) => panic!("XML parse error in {}: {e}", path.display()),
            _ => {}
        }
        buf.clear();
    }

    println!("cargo:rerun-if-changed={}", path.display());

    if entries.is_empty() {
        panic!(
            "{}: parsed zero <Tetragraph> entries — taxonomy file is empty \
             or the schema changed shape (issue #208)",
            path.display()
        );
    }

    entries
}

/// Emit `cargo:warning=` for any taxonomy entry that violates the
/// V2022-NOV-empirical invariants the runtime API depends on. These are
/// soft guards — the build still succeeds — so a future ODNI revision
/// can land without breaking CI, but the operator gets a signal that
/// `is_decomposable` semantics may need revisiting.
fn check_taxonomy_invariants(entries: &[TaxEntry]) {
    for entry in entries {
        // Guard 1: decomposable=Yes with non-Members membership shape.
        if entry.decomposable == TaxDecomposable::Yes {
            match &entry.membership {
                TaxMembership::Members {
                    recursive: false, ..
                } => {}
                _ => {
                    println!(
                        "cargo:warning=ISMCAT taxonomy: <Tetragraph> {:?} has \
                         decomposable=\"Yes\" but membership shape is {} \
                         (V2022-NOV had zero such entries; verify is_decomposable \
                         mapping)",
                        entry.code,
                        entry.membership.shape_label()
                    );
                }
            }
        }

        // Guard 2: decomposable=NA without `deprecated` attribute.
        if entry.decomposable == TaxDecomposable::Na && entry.deprecated.is_none() {
            println!(
                "cargo:warning=ISMCAT taxonomy: <Tetragraph> {:?} has \
                 decomposable=\"NA\" but no `deprecated` attribute \
                 (V2022-NOV had 18/18 NA entries with deprecated; future revision \
                 may decouple — NA→None mapping may need revisiting)",
                entry.code
            );
        }

        // Guard 3: decomposable=No with non-Suppressed membership.
        if entry.decomposable == TaxDecomposable::No
            && !matches!(entry.membership, TaxMembership::Suppressed)
        {
            println!(
                "cargo:warning=ISMCAT taxonomy: <Tetragraph> {:?} has \
                 decomposable=\"No\" but membership shape is {} (expected \
                 Suppressed) — atom-by-authority semantics may not apply",
                entry.code,
                entry.membership.shape_label()
            );
        }

        // Guard 4: <Organization> ref in the denormalized file outside
        // NA-deprecated entries. V2022-NOV only has BHTF (NA) recursive.
        if let TaxMembership::Members {
            recursive: true, ..
        } = &entry.membership
        {
            if entry.decomposable != TaxDecomposable::Na {
                println!(
                    "cargo:warning=ISMCAT taxonomy: <Tetragraph> {:?} carries an \
                     <Organization> ref in the denormalized file with \
                     decomposable={:?} (V2022-NOV only had BHTF/NA recursive). \
                     Re-vendor against a freshly-denormalized release or \
                     implement recursive-fixed-point expansion.",
                    entry.code,
                    entry.decomposable.as_xml()
                );
            }
        }
    }
}

/// Emit the `TetragraphProvenance` row type, a `pub(crate)` static
/// table of rows, and the public `lookup_tetragraph_provenance(code)`
/// accessor. Preserves the full three-state `decomposable` flag, the
/// `<Membership>` shape variant, both dates, and the verbatim
/// `<Description>` text — collapsed by the binary `is_decomposable`
/// runtime API.
///
/// SemVer surface: the accessor function is the **stable** entry point
/// for cross-crate consumers (PR-2's S005 in marque-capco). The
/// `TetragraphProvenance` struct is `pub` because the accessor returns
/// `&'static TetragraphProvenance`, but is marked `#[doc(hidden)]` to
/// signal that field additions are not major-version events. The
/// underlying `TETRAGRAPH_PROVENANCE` static is `pub(crate)` — only
/// the accessor surfaces it externally.
fn emit_tax_provenance(content: &mut String, taxonomy: &[TaxEntry]) {
    use std::fmt::Write;

    writeln!(
        content,
        "/// Provenance metadata row returned by [`lookup_tetragraph_provenance`].\n\
         ///\n\
         /// `#[doc(hidden)]`: the struct is `pub` only because the accessor returns\n\
         /// a reference to it across crate boundaries (PR-2 S005 consumer). Field\n\
         /// additions and renames are conventionally **not** major-version events;\n\
         /// stable consumers should call accessor methods or pattern-match defensively.\n\
         /// Issue #208."
    )
    .unwrap();
    writeln!(content, "#[doc(hidden)]").unwrap();
    writeln!(content, "#[derive(Debug, Clone, Copy)]").unwrap();
    writeln!(content, "pub struct TetragraphProvenance {{").unwrap();
    writeln!(content, "    pub code: &'static str,").unwrap();
    writeln!(
        content,
        "    /// One of `\"Yes\"`, `\"No\"`, `\"NA\"` — verbatim XSD attribute."
    )
    .unwrap();
    writeln!(content, "    pub decomposable: &'static str,").unwrap();
    writeln!(
        content,
        "    /// One of `\"Members\"`, `\"Members(recursive)\"`, `\"Description\"`, `\"Suppressed\"`."
    )
    .unwrap();
    writeln!(content, "    pub membership_shape: &'static str,").unwrap();
    writeln!(
        content,
        "    /// `<Tetragraph deprecated=\"YYYY-MM-DD\">` when present."
    )
    .unwrap();
    writeln!(content, "    pub deprecated: Option<&'static str>,").unwrap();
    writeln!(
        content,
        "    /// `<Membership dateLastVerified=\"YYYY-MM-DD\">` (XSD-required)."
    )
    .unwrap();
    writeln!(content, "    pub last_verified: &'static str,").unwrap();
    writeln!(
        content,
        "    /// Verbatim `<Description>` body for `Description`-shape entries\n    \
         /// (typically OCA-deferral pointers); `None` for other shapes. The text is\n    \
         /// already classification U/USA-marked and is surfaced verbatim by\n    \
         /// issue #206's S005 diagnostic — Constitution V audit-content-ignorance\n    \
         /// applies (this is ODNI taxonomy data, not user-document content)."
    )
    .unwrap();
    writeln!(content, "    pub description: Option<&'static str>,").unwrap();
    writeln!(content, "}}").unwrap();
    writeln!(content).unwrap();

    let mut sorted: Vec<&TaxEntry> = taxonomy.iter().collect();
    sorted.sort_by(|a, b| a.code.cmp(&b.code));

    writeln!(
        content,
        "/// Per-tetragraph ISMCAT V{ISMCAT_TETRA_VERSION} provenance metadata.\n\
         ///\n\
         /// `pub(crate)` — external consumers MUST go through\n\
         /// [`lookup_tetragraph_provenance`] (the SemVer-stable accessor).\n\
         /// Sorted by `code` for binary-search lookup. {} entries total.",
        sorted.len()
    )
    .unwrap();
    writeln!(
        content,
        "pub(crate) static TETRAGRAPH_PROVENANCE: &[TetragraphProvenance] = &["
    )
    .unwrap();
    for entry in sorted {
        let deprecated = match &entry.deprecated {
            Some(d) => format!("Some({d:?})"),
            None => "None".to_owned(),
        };
        let description = match &entry.membership {
            TaxMembership::Description(text) => format!("Some({text:?})"),
            _ => "None".to_owned(),
        };
        writeln!(
            content,
            "    TetragraphProvenance {{ code: {:?}, decomposable: {:?}, \
             membership_shape: {:?}, deprecated: {}, last_verified: {:?}, \
             description: {} }},",
            entry.code,
            entry.decomposable.as_xml(),
            entry.membership.shape_label(),
            deprecated,
            entry.last_verified,
            description,
        )
        .unwrap();
    }
    writeln!(content, "];").unwrap();
    writeln!(content).unwrap();

    // Public stable accessor.
    writeln!(
        content,
        "/// Look up a tetragraph's provenance metadata.\n\
         ///\n\
         /// Returns `None` for codes absent from the ISMCAT V{ISMCAT_TETRA_VERSION}\n\
         /// taxonomy entirely (org-fork extensions, unknown codes, trigraphs).\n\
         /// Cross-crate consumers — notably issue #206's S005 silent-loss\n\
         /// diagnostic in `marque-capco` — call this instead of touching the\n\
         /// underlying static directly. Issue #208.\n\
         pub fn lookup_tetragraph_provenance(code: &str) -> Option<&'static TetragraphProvenance> {{\n\
         \x20   TETRAGRAPH_PROVENANCE\n\
         \x20       .binary_search_by_key(&code, |row| row.code)\n\
         \x20       .ok()\n\
         \x20       .map(|i| &TETRAGRAPH_PROVENANCE[i])\n\
         }}\n"
    )
    .unwrap();
}

/// Emit the public three-state `is_decomposable(code)` discriminator.
///
/// Mapping:
///
/// - `Some(true)` — taxonomy `decomposable="Yes"` AND non-recursive
///   `<Members>` with non-empty `<Country>` list (24 codes in V2022-NOV).
/// - `Some(false)` — taxonomy `decomposable="No"` (19 codes — atom by
///   authority).
/// - `None` — `decomposable="NA"` (18 deprecated codes), or any `Yes`
///   entry that fails the materialized-members precondition (zero such
///   entries in V2022-NOV; guard #1 emits a `cargo:warning=` if one
///   ever appears), or a code absent from the taxonomy entirely
///   (org-fork extensions and unknown codes).
///
/// Org-fork extensions (`country_extensions.toml`) deliberately route to
/// `None` even when they declare `members = […]` — extensions don't
/// carry ODNI authority. `lookup_tetragraph_members` still resolves
/// extension members; `is_decomposable` reflects taxonomy status only.
/// Issue #206's S005 rule depends on this distinction.
fn emit_is_decomposable(content: &mut String, taxonomy: &[TaxEntry]) {
    use std::fmt::Write;

    let mut yes: Vec<&str> = Vec::new();
    let mut no: Vec<&str> = Vec::new();
    for entry in taxonomy {
        match entry.decomposable {
            TaxDecomposable::Yes => {
                if let TaxMembership::Members {
                    countries,
                    recursive: false,
                    ..
                } = &entry.membership
                {
                    if !countries.is_empty() {
                        yes.push(&entry.code);
                    }
                }
            }
            TaxDecomposable::No => no.push(&entry.code),
            TaxDecomposable::Na => {}
        }
    }
    yes.sort_unstable();
    no.sort_unstable();

    writeln!(
        content,
        "/// Three-state ISMCAT V{ISMCAT_TETRA_VERSION} decomposability flag (issue #208).\n\
         ///\n\
         /// Returns:\n\
         ///\n\
         /// - `Some(true)` — taxonomy `decomposable=\"Yes\"` with materialized\n\
         ///   non-recursive members ({yes_n} codes).\n\
         /// - `Some(false)` — taxonomy `decomposable=\"No\"` — atom by authority\n\
         ///   ({no_n} codes).\n\
         /// - `None` — taxonomy `decomposable=\"NA\"` (deprecated; membership\n\
         ///   suppressed, OCA-deferred, or recursive); OR code absent from\n\
         ///   taxonomy entirely.\n\
         ///\n\
         /// Org-fork extensions (`country_extensions.toml`) deliberately route to `None`\n\
         /// even when they declare members — extensions don't carry ODNI authority. Use\n\
         /// `lookup_tetragraph_members` for materialized member resolution that includes\n\
         /// extensions; use `is_decomposable` for the ODNI-authoritative discriminator\n\
         /// consumed by issue #206's S005 rule.\n\
         ///\n\
         /// Source: `Taxonomy/ISMCAT/TetragraphTaxonomyDenormalized.xml`.",
        yes_n = yes.len(),
        no_n = no.len(),
    )
    .unwrap();

    writeln!(
        content,
        "pub fn is_decomposable(code: &str) -> Option<bool> {{"
    )
    .unwrap();
    writeln!(content, "    match code {{").unwrap();
    for c in &yes {
        writeln!(content, "        {c:?} => Some(true),").unwrap();
    }
    for c in &no {
        writeln!(content, "        {c:?} => Some(false),").unwrap();
    }
    writeln!(content, "        _ => None,").unwrap();
    writeln!(content, "    }}").unwrap();
    writeln!(content, "}}").unwrap();
    writeln!(content).unwrap();
}

// ---------------------------------------------------------------------------
// Issue #183 PR-B: org-specific country-code extensions
// ---------------------------------------------------------------------------

/// Parsed extension entry from `country_extensions.toml`.
struct CountryExtension {
    code: String,
    description: String,
    members: Vec<String>,
    /// Build-time validated active_from date string (YYYY, YYYY-MM, or YYYY-MM-DD).
    active_from: Option<String>,
    /// Build-time validated active_to date string (YYYY, YYYY-MM, or YYYY-MM-DD).
    active_to: Option<String>,
}

/// TOML wire types for the extensions file.
mod ext_toml {
    use serde::Deserialize;

    #[derive(Deserialize, Default)]
    pub(super) struct ExtensionsFile {
        #[serde(default)]
        pub code: Vec<CodeEntry>,
    }

    #[derive(Deserialize)]
    pub(super) struct CodeEntry {
        pub code: String,
        // Optional at the wire level so a missing field surfaces as
        // our own targeted panic naming the offending `code`, rather
        // than a generic serde "missing field `description`" error
        // with no context. The validation in `load_country_extensions`
        // rejects None (and whitespace-only) with a clear message.
        #[serde(default)]
        pub description: Option<String>,
        #[serde(default)]
        pub members: Option<Vec<String>>,
        /// First date on which this code was active (ISO 8601: `YYYY`,
        /// `YYYY-MM`, or `YYYY-MM-DD`). Optional; `None` means
        /// "active from the beginning of the period covered by this
        /// release." Build-time validated with [`validate_temporal_field`].
        #[serde(default)]
        pub active_from: Option<String>,
        /// Last date on which this code was active, inclusive.
        /// Optional; `None` means "still active." Same formats as
        /// `active_from`. Must be `>= active_from` when both are present
        /// (build-time validated).
        #[serde(default)]
        pub active_to: Option<String>,
    }
}

/// Returns true if `b` is in the CAPCO country-code byte set:
/// ASCII uppercase, ASCII digit, or underscore. Mirrors
/// `marque_ism::CountryCode::is_valid_byte` — keep these two in
/// sync if the byte set ever widens.
fn is_valid_extension_byte(b: u8) -> bool {
    b.is_ascii_uppercase() || b.is_ascii_digit() || b == b'_'
}

/// Returns the number of days in the given month, correctly accounting for
/// leap years. Used by [`validate_temporal_field`] without a jiff dep in
/// the build script.
fn build_days_in_month(year: i32, month: u8) -> u8 {
    match month {
        1 | 3 | 5 | 7 | 8 | 10 | 12 => 31,
        4 | 6 | 9 | 11 => 30,
        2 => {
            let leap = (year % 4 == 0 && year % 100 != 0) || (year % 400 == 0);
            if leap { 29 } else { 28 }
        }
        _ => 0, // invalid month — caught before this is reached
    }
}

/// Validate a temporal date string from `country_extensions.toml`.
///
/// Accepted forms:
/// - `YYYY` — `xsd:gYear` (e.g. `"2003"`)
/// - `YYYY-MM` — `xsd:gYearMonth` (e.g. `"2003-04"`)
/// - `YYYY-MM-DD` — `xsd:date` (e.g. `"2003-04-15"`)
///
/// Returns `Err(msg)` with a human-readable error string if the value
/// does not match one of these forms or the calendar components are
/// out of range. This is called from `load_country_extensions` which
/// `panic!`s with the error and the offending `code` so the build
/// error is unambiguous.
fn validate_temporal_field(s: &str, field: &str, code: &str, path: &str) {
    let bytes = s.as_bytes();
    let ok = match bytes.len() {
        // YYYY
        4 => bytes.iter().all(|b| b.is_ascii_digit()),
        // YYYY-MM
        7 => {
            bytes[4] == b'-'
                && bytes[0..4].iter().all(|b| b.is_ascii_digit())
                && bytes[5..7].iter().all(|b| b.is_ascii_digit())
                && {
                    let month = (bytes[5] - b'0') * 10 + (bytes[6] - b'0');
                    (1..=12).contains(&month)
                }
        }
        // YYYY-MM-DD — validate full calendar correctness (incl. leap years)
        10 => {
            bytes[4] == b'-'
                && bytes[7] == b'-'
                && bytes[0..4].iter().all(|b| b.is_ascii_digit())
                && bytes[5..7].iter().all(|b| b.is_ascii_digit())
                && bytes[8..10].iter().all(|b| b.is_ascii_digit())
                && {
                    let y = (bytes[0] - b'0') as i32 * 1000
                        + (bytes[1] - b'0') as i32 * 100
                        + (bytes[2] - b'0') as i32 * 10
                        + (bytes[3] - b'0') as i32;
                    let m = (bytes[5] - b'0') * 10 + (bytes[6] - b'0');
                    let d = (bytes[8] - b'0') * 10 + (bytes[9] - b'0');
                    (1..=12).contains(&m) && d >= 1 && d <= build_days_in_month(y, m)
                }
        }
        _ => false,
    };
    if !ok {
        panic!(
            "{path}: country extension `code = {code:?}` has invalid \
             `{field}` value {s:?}. Expected ISO 8601 date: \
             `YYYY`, `YYYY-MM`, or `YYYY-MM-DD` (calendar-correct)."
        );
    }
}

/// Returns the *start-of-span* `(year, month, day)` for a validated ISO 8601
/// date string (`YYYY`, `YYYY-MM`, or `YYYY-MM-DD`).
///
/// - `YYYY`       → (year, 1,  1)
/// - `YYYY-MM`    → (year, mm, 1)
/// - `YYYY-MM-DD` → (year, mm, dd)
fn temporal_start_ymd(s: &str) -> (i32, u8, u8) {
    let b = s.as_bytes();
    let y = (b[0] - b'0') as i32 * 1000
        + (b[1] - b'0') as i32 * 100
        + (b[2] - b'0') as i32 * 10
        + (b[3] - b'0') as i32;
    match s.len() {
        4 => (y, 1, 1),
        7 => {
            let m = (b[5] - b'0') * 10 + (b[6] - b'0');
            (y, m, 1)
        }
        _ => {
            let m = (b[5] - b'0') * 10 + (b[6] - b'0');
            let d = (b[8] - b'0') * 10 + (b[9] - b'0');
            (y, m, d)
        }
    }
}

/// Returns the *end-of-span* `(year, month, day)` for a validated ISO 8601
/// date string (`YYYY`, `YYYY-MM`, or `YYYY-MM-DD`).
///
/// - `YYYY`       → (year, 12, 31)
/// - `YYYY-MM`    → (year, mm, last day of month)
/// - `YYYY-MM-DD` → (year, mm, dd)
fn temporal_end_ymd(s: &str) -> (i32, u8, u8) {
    let b = s.as_bytes();
    let y = (b[0] - b'0') as i32 * 1000
        + (b[1] - b'0') as i32 * 100
        + (b[2] - b'0') as i32 * 10
        + (b[3] - b'0') as i32;
    match s.len() {
        4 => (y, 12, 31),
        7 => {
            let m = (b[5] - b'0') * 10 + (b[6] - b'0');
            (y, m, build_days_in_month(y, m))
        }
        _ => {
            let m = (b[5] - b'0') * 10 + (b[6] - b'0');
            let d = (b[8] - b'0') * 10 + (b[9] - b'0');
            (y, m, d)
        }
    }
}

/// Read & validate `country_extensions.toml`. Returns extensions
/// in declaration order (matters for forward-reference checks in
/// `members`). Empty / missing file is a no-op (returns `[]`).
///
/// Validation:
/// - `code` length 2..=16, all bytes in CAPCO byte set
/// - `code` not duplicating a CVE entry or earlier extension
/// - each `members` entry must already be recognized (CVE or
///   earlier extension); forward references rejected
/// - `active_from` and `active_to` (when present) must be valid ISO 8601
///   date strings (`YYYY`, `YYYY-MM`, or `YYYY-MM-DD`); `active_to` must
///   not sort before `active_from` when both are present
///
/// Failures `panic!` from `build.rs`, which surfaces as a clear
/// build error pointing at the offending entry.
fn load_country_extensions(
    path: &str,
    cve_codes: &std::collections::BTreeSet<String>,
) -> Vec<CountryExtension> {
    use ext_toml::*;

    // Rerun the build whenever the extensions file changes.
    println!("cargo:rerun-if-changed={path}");

    let raw = match fs::read_to_string(path) {
        Ok(s) => s,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Vec::new(),
        Err(e) => panic!("failed to read {path}: {e}"),
    };

    let parsed: ExtensionsFile =
        toml::from_str(&raw).unwrap_or_else(|e| panic!("failed to parse {path}: {e}"));

    let mut out: Vec<CountryExtension> = Vec::new();
    let mut seen: std::collections::BTreeSet<String> = cve_codes.clone();

    for entry in parsed.code {
        // Destructure upfront so the field bindings are explicit
        // and a future reader doesn't have to track per-field
        // partial moves out of `entry`.
        let CodeEntry {
            code,
            description: raw_description,
            members: raw_members,
            active_from: raw_active_from,
            active_to: raw_active_to,
        } = entry;

        // Required-description check (issue #183 PR-B): the
        // `description` field is required for auditor traceability —
        // a code lookup in the generated output must surface a
        // human-readable string explaining what the code means.
        // Both "field omitted" and "field whitespace-only" route
        // through this single panic so the build error always
        // names the offending `code` and `path`.
        //
        // Reject `\n` / `\r` in the description because the value
        // is emitted directly into a generated `///` doc comment
        // block — a multi-line value would break the next line out
        // of the comment and produce a values.rs that fails to
        // compile. Single-line is a stricter constraint than the
        // CAPCO byte set requires for `code`, but the auditor-
        // traceability use case is well-served by single-line
        // descriptions.
        let description = match raw_description {
            Some(d) if !d.trim().is_empty() => {
                if d.contains('\n') || d.contains('\r') {
                    panic!(
                        "{path}: country extension `code = {code:?}` has a \
                         multi-line `description`. The value is emitted into \
                         a generated `///` doc comment in values.rs; \
                         line-break characters (`\\n`, `\\r`) must be \
                         removed (or replaced with spaces) before the build \
                         can proceed.",
                    );
                }
                d
            }
            _ => panic!(
                "{path}: country extension `code = {code:?}` is missing \
                 required `description` field (or it is whitespace-only). \
                 Every extension MUST carry a non-empty description so an \
                 auditor tracing the code can find its provenance.",
            ),
        };

        // Length check.
        if !(2..=16).contains(&code.len()) {
            panic!(
                "{path}: country extension `code = {code:?}` has invalid \
                 length {} — CAPCO codes are 2..=16 bytes",
                code.len(),
            );
        }
        // Byte-set check.
        for &b in code.as_bytes() {
            if !is_valid_extension_byte(b) {
                panic!(
                    "{path}: country extension `code = {code:?}` contains \
                     byte 0x{b:02X} ({:?}) outside the CAPCO byte set \
                     (ASCII uppercase letter, ASCII digit, underscore). \
                     Hyphens are NOT accepted — no CVE entry uses them.",
                    b as char,
                );
            }
        }
        // Duplicate check (CVE + earlier extensions).
        if !seen.insert(code.clone()) {
            panic!(
                "{path}: country extension `code = {code:?}` duplicates a \
                 CVE entry or an earlier extension. Each code must be \
                 unique across the union.",
            );
        }

        // Validate members (if present): each must already be in
        // the recognition set, must be a 3-byte trigraph (atomic
        // country code — tetragraph-of-tetragraphs is rejected so
        // single-level expansion stays well-defined), and must not
        // be the extension's own code (self-reference is rejected
        // — `seen.insert(code)` above intentionally goes before
        // member validation so self-membership is caught here as
        // a distinct error rather than via a forward-reference
        // confusion). `members = []` is treated as `members = None`
        // (recognition-only, opaque).
        let members: Vec<String> = match raw_members {
            None => Vec::new(),
            Some(m) if m.is_empty() => Vec::new(),
            Some(m) => {
                for member in &m {
                    if member == &code {
                        panic!(
                            "{path}: country extension `code = {code:?}` \
                             lists itself in `members`. An extension cannot \
                             expand to itself.",
                        );
                    }
                    if member.len() != 3 {
                        panic!(
                            "{path}: country extension `code = {code:?}` \
                             references member {member:?} of length {}. \
                             Tetragraph membership entries MUST be 3-byte \
                             country trigraphs (USA, GBR, AUS, …) so \
                             single-level expansion stays well-defined; \
                             tetragraph-of-tetragraphs would require \
                             recursive expansion which the consumer \
                             (`expand_tetragraph`) does not perform.",
                            member.len(),
                        );
                    }
                    if !seen.contains(member) {
                        panic!(
                            "{path}: country extension `code = {code:?}` \
                             references member {member:?} which is not in \
                             the recognition set. Members must be CVE \
                             trigraphs or 3-byte extensions defined \
                             earlier in this file (forward references \
                             are rejected).",
                        );
                    }
                }
                m
            }
        };

        // Validate active_from and active_to temporal fields.
        if let Some(ref af) = raw_active_from {
            validate_temporal_field(af, "active_from", &code, path);
        }
        if let Some(ref at) = raw_active_to {
            validate_temporal_field(at, "active_to", &code, path);
        }
        // active_to must not precede active_from.
        //
        // Comparison uses span-aware logic: `active_from` uses its
        // *start-of-span* (first day) and `active_to` uses its
        // *end-of-span* (last day), so `active_to = "2003"` (meaning
        // all of 2003, end = 2003-12-31) is not rejected when
        // `active_from = "2003-04-01"` (start = 2003-04-01).
        if let (Some(af), Some(at)) = (&raw_active_from, &raw_active_to) {
            let from_start = temporal_start_ymd(af);
            let to_end = temporal_end_ymd(at);
            if to_end < from_start {
                panic!(
                    "{path}: country extension `code = {code:?}` has \
                     `active_to` ({at:?}) before `active_from` ({af:?}). \
                     The activity window must be non-negative.",
                );
            }
        }

        out.push(CountryExtension {
            code,
            description,
            members,
            active_from: raw_active_from,
            active_to: raw_active_to,
        });
    }

    out
}

/// Emit `pub static TETRAGRAPH_MEMBERS: &[(&str, &[&str])]`. Rows are
/// sourced from the ISMCAT taxonomy (24 `decomposable="Yes"` entries with
/// materialized `<Country>` lists in V2022-NOV) and appended with
/// `country_extensions.toml` entries that declare non-empty `members`.
/// The final slice is sorted by code for stable diffs and binary-search
/// lookup at consumer sites.
///
/// Extensions cannot shadow taxonomy codes — `load_country_extensions`
/// rejects any extension whose `code` is already in the CVE recognition
/// set, and all 61 ISMCAT taxonomy codes appear in
/// `CVEnumISMCATRelTo.xsd`. The shadowing question (plan §8 Q2) is
/// therefore resolved by the existing duplicate guard; no `override`
/// opt-in is needed.
fn emit_tetragraph_members(
    content: &mut String,
    taxonomy: &[TaxEntry],
    extensions: &[CountryExtension],
) {
    use std::fmt::Write;

    // Collect (code, members) tuples from taxonomy entries that pass
    // the materialized-non-recursive-Members predicate, then append
    // extensions with non-empty `members`. Extensions without members
    // participate in recognition (via TRIGRAPHS) but not in expansion.
    //
    // Member-list order is sorted ASCII-alphabetical at emit time. The
    // ODNI XML lists members in publication order (e.g. FVEY as
    // `AUS, CAN, NZL, GBR, USA`), but that order carries no semantic
    // weight — when the set hits banner roll-up the consumer re-sorts
    // per CAPCO §H.8 (USA first, then trigraph-alpha, tetragraph-alpha)
    // anyway, and the alphabetical form is unambiguously friendlier
    // for `pub const FVEY` reviewers and for diff stability across
    // future taxonomy revisions that may reorder typographically.
    let mut rows: Vec<(&str, Vec<&str>)> = Vec::new();
    let mut taxonomy_count = 0usize;
    for entry in taxonomy {
        if let TaxMembership::Members {
            countries,
            recursive: false,
            ..
        } = &entry.membership
        {
            if !countries.is_empty() {
                let mut members: Vec<&str> = countries.iter().map(String::as_str).collect();
                members.sort_unstable();
                rows.push((entry.code.as_str(), members));
                taxonomy_count += 1;
            }
        }
    }
    let mut extension_count = 0usize;
    for ext in extensions {
        if !ext.members.is_empty() {
            let mut members: Vec<&str> = ext.members.iter().map(String::as_str).collect();
            members.sort_unstable();
            rows.push((ext.code.as_str(), members));
            extension_count += 1;
        }
    }
    rows.sort_by_key(|(code, _)| *code);

    writeln!(
        content,
        "/// Canonical tetragraph / country-group code membership table.\n\
         ///\n\
         /// Sourced from the ISMCAT V{ISMCAT_TETRA_VERSION} Tetragraph Taxonomy\n\
         /// (`Taxonomy/ISMCAT/TetragraphTaxonomyDenormalized.xml` — issue #208) plus\n\
         /// any `members`-bearing entries from `country_extensions.toml` (issue #183 PR-B).\n\
         ///\n\
         /// Codes absent from this table — taxonomy `decomposable=\"No\"` entries (EU,\n\
         /// GCCH, KFOR, …, atom by authority), `decomposable=\"NA\"` deprecated entries\n\
         /// (RSMA, ISAF, MCFI, …, membership suppressed or OCA-deferred), and codes\n\
         /// outside the taxonomy entirely — are opaque to expansion. They survive REL TO\n\
         /// intersection only when present in every portion's list. Use `is_decomposable`\n\
         /// for the ODNI-authoritative discriminator that distinguishes these cases.\n\
         ///\n\
         /// Sorted by code for stable diffs and `binary_search` lookup. {} entries total\n\
         /// ({} taxonomy + {} extension).",
        rows.len(),
        taxonomy_count,
        extension_count,
    )
    .unwrap();
    writeln!(
        content,
        "pub static TETRAGRAPH_MEMBERS: &[(&str, &[&str])] = &["
    )
    .unwrap();
    for (code, members) in &rows {
        write!(content, "    ({code:?}, &[").unwrap();
        for (i, m) in members.iter().enumerate() {
            if i > 0 {
                write!(content, ", ").unwrap();
            }
            write!(content, "{m:?}").unwrap();
        }
        writeln!(content, "]),").unwrap();
    }
    writeln!(content, "];").unwrap();
    writeln!(content).unwrap();

    // Lookup helper.
    writeln!(
        content,
        "/// Look up a tetragraph's constituent trigraphs.\n\
         ///\n\
         /// Returns `None` for codes not in the membership table — \
         either trigraphs (for which\n\
         /// expansion is undefined), opaque tetragraphs (NATO, \
         RSMA, …), or unrecognized\n\
         /// codes. Callers should treat `None` as \"opaque atom \
         in intersection\".\n\
         pub fn lookup_tetragraph_members(code: &str) -> Option<&'static [&'static str]> {{\n\
         \x20   TETRAGRAPH_MEMBERS\n\
         \x20       .binary_search_by_key(&code, |(c, _)| *c)\n\
         \x20       .ok()\n\
         \x20       .map(|i| TETRAGRAPH_MEMBERS[i].1)\n\
         }}\n"
    )
    .unwrap();
}
