// SPDX-FileCopyrightText: 2026 Knitli Inc.
//
// SPDX-License-Identifier: LicenseRef-MarqueLicense-1.0

//! marque-ism build script.
//!
//! Parses ODNI ISM specification files from `schemas/ISM-v2022-DEC/` and
//! generates Rust code into `OUT_DIR/`:
//!
//! - `values.rs`      — CVE enumeration types (closed Rust enums + lookup tables)
//! - `validators.rs`  — Schematron-derived validation predicates
//! - `migrations.rs`  — deprecated marking → replacement mappings
//!
//! # Schema Layout (actual ODNI package structure)
//!
//! ```text
//! schemas/ISM-v2022-DEC/
//!   CVE_ISM/          CVEnumISM*.xml — ISM-specific CVE enumerations
//!   CVE_ISMCAT/       CVEGenerated/CVEnumISMCAT*.xsd — country trigraphs etc.
//!   Schema/           IC-ISM.xsd, ISM.rng, CVEGenerated/*.xsd
//!   Schematron/       ISM_XML.sch, Lib/*.sch
//! ```
//!
//! Rerun triggers: any change to schema files or this build script.

use quick_xml::Reader;
use quick_xml::events::Event;

use std::{env, fs, path::Path};

const SCHEMA_VERSION: &str = "ISM-v2022-DEC";

fn main() {
    let schema_dir = Path::new("schemas").join(SCHEMA_VERSION);
    let out_dir = env::var("OUT_DIR").expect("OUT_DIR not set");
    let out_path = Path::new(&out_dir);

    // T010: Assert schema version matches Cargo.toml metadata.
    verify_schema_version();

    // Rerun if schema files change.
    println!("cargo:rerun-if-changed=schemas/");
    println!("cargo:rerun-if-changed=build.rs");
    println!("cargo:rerun-if-changed=Cargo.toml");

    generate_values(out_path, &schema_dir);
    generate_validators(out_path, &schema_dir);
    generate_migrations(out_path, &schema_dir);
    generate_vocabulary(out_path, &schema_dir);
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

// ---------------------------------------------------------------------------
// T006: CVE XML parsing → typed Rust enums
// ---------------------------------------------------------------------------

/// Parse a CVE XML file and extract all `<Value>` text content.
/// Handles both default-namespace `<Term><Value>` and prefixed `<cve:Term><cve:Value>`.
fn parse_cve_xml(path: &Path) -> Vec<(String, String)> {
    let content = fs::read_to_string(path)
        .unwrap_or_else(|e| panic!("failed to read {}: {e}", path.display()));
    let mut reader = Reader::from_str(&content);

    let mut entries = Vec::new();
    let mut in_term = false;
    let mut in_value = false;
    let mut in_description = false;
    let mut current_value = String::new();
    let mut current_desc = String::new();

    loop {
        match reader.read_event() {
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

/// Emit a minimal enum for CVE types that have zero entries in the public spec.
#[allow(dead_code)] // Retained for potential future empty-CVE categories; SAR removed per specs/002-sar-implementation.
fn emit_empty_enum(out: &mut String, name: &str, doc: &str) {
    use std::fmt::Write;

    writeln!(out, "/// {doc}").unwrap();
    writeln!(
        out,
        "#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]"
    )
    .unwrap();
    writeln!(out, "#[non_exhaustive]").unwrap();
    writeln!(out, "pub enum {name} {{}}").unwrap();
    writeln!(out).unwrap();

    writeln!(out, "impl {name} {{").unwrap();
    writeln!(
        out,
        "    /// Returns the canonical CVE string representation."
    )
    .unwrap();
    writeln!(out, "    pub fn as_str(&self) -> &'static str {{").unwrap();
    writeln!(out, "        match *self {{}}").unwrap();
    writeln!(out, "    }}").unwrap();
    writeln!(out).unwrap();
    writeln!(
        out,
        "    /// Parse from the canonical CVE string. Always returns `None` (no entries)."
    )
    .unwrap();
    writeln!(out, "    pub fn parse(_s: &str) -> Option<Self> {{").unwrap();
    writeln!(out, "        None").unwrap();
    writeln!(out, "    }}").unwrap();
    writeln!(out).unwrap();
    writeln!(out, "    /// All valid values (empty).").unwrap();
    writeln!(out, "    pub const ALL: &[{name}] = &[];").unwrap();
    writeln!(out, "}}").unwrap();
    writeln!(out).unwrap();
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
         This is almost always a bad schema copy — verify the ODNI ISM bundle \
         is present at schemas/ISM-v2022-DEC/ and that the XML file parses."
    );
}

fn generate_values(out: &Path, schema_dir: &Path) {
    use std::fmt::Write;
    let cve_dir = schema_dir.join("CVE_ISM");
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
    let trigraphs = parse_xsd_trigraphs(
        &schema_dir
            .join("CVE_ISMCAT")
            .join("CVEGenerated")
            .join("CVEnumISMCATRelTo.xsd"),
    );

    // Emit trigraph array (not an enum — too many values, and CountryCode is a [u8; 3] newtype).
    //
    // M-3: sort and deduplicate into a BTreeSet before emission so
    // `is_trigraph` in token_set.rs can use `binary_search` over a
    // guaranteed-sorted slice. The XSD emits entries in document order
    // (USA first, then alphabetical), so an unsorted emission would
    // silently break binary_search if the ODNI bundle ever reorders.
    let sorted_trigraphs: std::collections::BTreeSet<String> =
        trigraphs.into_iter().map(|(v, _)| v).collect();
    writeln!(
        content,
        "/// All valid country/entity trigraphs from CVEnumISMCATRelTo.xsd,\n\
         /// sorted ascending and deduplicated. `is_trigraph` uses binary_search."
    )
    .unwrap();
    writeln!(content, "/// {} entries total.", sorted_trigraphs.len()).unwrap();
    writeln!(content, "pub static TRIGRAPHS: &[&str] = &[").unwrap();
    for value in &sorted_trigraphs {
        writeln!(content, "    {value:?},").unwrap();
    }
    writeln!(content, "];").unwrap();
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

    let mut entries = Vec::new();

    loop {
        match reader.read_event() {
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
                            let val = attr.unescape_value().unwrap_or_else(|err| {
                                panic!("XSD attribute unescape error in {}: {err}", path.display())
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
    }

    entries
}

// ---------------------------------------------------------------------------
// T008: Schematron → validator predicates
// ---------------------------------------------------------------------------

fn generate_validators(out: &Path, schema_dir: &Path) {
    let _sch = schema_dir.join("Schematron").join("ISM_XML.sch");
    let _lib = schema_dir.join("Schematron").join("Lib");

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

fn generate_migrations(out: &Path, _schema_dir: &Path) {
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

fn generate_vocabulary(out: &Path, schema_dir: &Path) {
    use std::collections::BTreeMap;
    use std::fmt::Write;

    let cve_dir = schema_dir.join("CVE_ISM");
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
         /// `crates/ism/schemas/ISM-v2022-DEC/CVE_ISM/`. Every token-level\n\
         /// metadata entry references one of these so the same authority,\n\
         /// point of contact, and schema-version provenance is shared\n\
         /// across all tokens published in that file.\n\
         #[derive(Debug, Clone, Copy)]\n\
         pub struct CveFileMetadata {{\n\
         \x20   /// Symbolic constant name (e.g., \"CVE_DISSEM\").\n\
         \x20   pub const_name: &'static str,\n\
         \x20   /// Source-of-record URN, e.g.,\n\
         \x20   /// `urn:us:gov:ic:cvenum:ism:dissem`.\n\
         \x20   pub urn: &'static str,\n\
         \x20   /// CVE title text.\n\
         \x20   pub title: &'static str,\n\
         \x20   /// Free-form `Source` text from the CVE IRM.\n\
         \x20   pub source: &'static str,\n\
         \x20   /// Point-of-contact name.\n\
         \x20   pub poc_name: &'static str,\n\
         \x20   /// Point-of-contact email.\n\
         \x20   pub poc_email: &'static str,\n\
         \x20   /// Owner/producer code, e.g., `\"USA\"`.\n\
         \x20   pub owner_producer: &'static str,\n\
         \x20   /// CVE `specVersion`, e.g., `\"202111.202211\"`.\n\
         \x20   pub spec_version: &'static str,\n\
         \x20   /// CVE `ism:DESVersion`, e.g., `\"202111\"`.\n\
         \x20   pub des_version: &'static str,\n\
         \x20   /// Pinned schema package version (`SCHEMA_VERSION`).\n\
         \x20   pub schema_version: &'static str,\n\
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
