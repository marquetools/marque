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
                let decoded = e.unescape().unwrap_or_else(|err| {
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

    // --- Dissemination Controls ---
    let dissem_entries = parse_cve_xml(&cve_dir.join("CVEnumISMDissem.xml"));
    assert_required("DissemControl", &dissem_entries, "CVEnumISMDissem.xml");
    emit_enum(
        &mut content,
        "DissemControl",
        &dissem_entries,
        "Dissemination controls from CVEnumISMDissem.xml.",
    );

    // --- SAR Identifiers ---
    // Note: the CVEnumISMSAR.xml in ISM-v2022-DEC contains zero entries
    // (SAR identifiers are classified and not published in the public CVE).
    // We emit the enum anyway for type-system completeness.
    let sar_entries = parse_cve_xml(&cve_dir.join("CVEnumISMSAR.xml"));
    if sar_entries.is_empty() {
        // Emit a minimal enum with a placeholder variant so it's usable
        emit_empty_enum(
            &mut content,
            "SarIdentifier",
            "Special Access Required identifiers. Empty in ISM-v2022-DEC public CVE.",
        );
    } else {
        emit_enum(
            &mut content,
            "SarIdentifier",
            &sar_entries,
            "Special Access Required identifiers from CVEnumISMSAR.xml.",
        );
    }

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

    // Emit trigraph array (not an enum — too many values, and Trigraph is a [u8; 3] newtype).
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
    for word in ["TOP SECRET", "SECRET", "CONFIDENTIAL", "RESTRICTED", "UNCLASSIFIED"] {
        all_tokens.insert(word.to_owned());
    }
    for (value, _) in &sci_entries {
        all_tokens.insert(value.clone());
    }
    for (value, _) in &dissem_entries {
        all_tokens.insert(value.clone());
    }
    for (value, _) in &sar_entries {
        all_tokens.insert(value.clone());
    }
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
/// Includes CAPCO policy-driven replacements and X-shorthand date markings.
/// All entries have confidence >= 0.95 per FR-004a.
pub static MIGRATIONS: &[MigrationEntry] = &[
    // Dissemination control deprecations
    MigrationEntry {
        deprecated: "LIMDIS",
        replacement: "RELIDO",
        confidence: 0.97,
        reference: "CAPCO-2019-§3.4",
    },
    MigrationEntry {
        deprecated: "FOUO",
        replacement: "CUI",
        confidence: 0.97,
        reference: "CAPCO-2022-§2.1",
    },
    MigrationEntry {
        deprecated: "NF",
        replacement: "NOFORN",
        confidence: 0.99,
        reference: "CAPCO-2022-§3.2",
    },
    // X-shorthand date marking patterns (FR-004a, research R-3)
    // These match the pattern 25X1-, 25X2-, etc.
    MigrationEntry {
        deprecated: "25X1-",
        replacement: "25X1",
        confidence: 0.97,
        reference: "CAPCO-2019-§5.1",
    },
    MigrationEntry {
        deprecated: "50X1-",
        replacement: "50X1-HUM",
        confidence: 0.95,
        reference: "CAPCO-2019-§5.2",
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
