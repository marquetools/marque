// SPDX-FileCopyrightText: 2026 Knitli Inc.
//
// SPDX-License-Identifier: LicenseRef-MarqueLicense-1.0

//! xmloxide integration tests: Schematron validation and XPath CVE extraction.
//!
//! # Capabilities confirmed:
//! - Schematron validation works for standalone rules (no sch:include)
//! - XPath can extract CVE values from ISM XML files
//!
//! # Known limitations (xmloxide 0.4.1):
//! - Schematron namespace uses "dml" not ISO-standard "dsdl"
//! - Namespace-prefixed XPath (e.g., @ism:classification) doesn't match;
//!   use local-name() workaround
//! - sch:include not resolved (ISM_XML.sch includes ~20 library files)
//! - document() function not supported (ISM_XML.sch loads CVE files)

use std::path::Path;

fn crate_root() -> &'static Path {
    Path::new(env!("CARGO_MANIFEST_DIR"))
}

#[test]
fn schematron_validates_simple_assertions() {
    let schema = r#"<?xml version="1.0" encoding="UTF-8"?>
<schema xmlns="http://purl.oclc.org/dml/schematron">
  <pattern>
    <rule context="/root">
      <assert test="@status = 'ok'">status must be ok</assert>
    </rule>
  </pattern>
</schema>"#;

    let sch = xmloxide::validation::schematron::parse_schematron(schema).unwrap();

    // Valid document passes.
    let doc = xmloxide::Document::parse_str(r#"<root status="ok"/>"#).unwrap();
    let result = xmloxide::validation::schematron::validate_schematron(&doc, &sch);
    assert!(result.is_valid);

    // Invalid document fails with expected message.
    let doc = xmloxide::Document::parse_str(r#"<root status="bad"/>"#).unwrap();
    let result = xmloxide::validation::schematron::validate_schematron(&doc, &sch);
    assert!(!result.is_valid);
    assert_eq!(result.errors.len(), 1);
}

#[test]
fn xpath_extracts_cve_dissem_values() {
    let cve_path = crate_root().join("schemas/ISM-v2022-DEC/CVE_ISM/CVEnumISMDissem.xml");
    if !cve_path.exists() {
        return; // schemas not present in CI
    }

    let content = std::fs::read_to_string(&cve_path).unwrap();
    let doc = xmloxide::Document::parse_str(&content).unwrap();
    let root = doc.root_element().unwrap();

    let result = xmloxide::xpath::evaluate(&doc, root, "count(//*[local-name()='Value'])").unwrap();
    let count = result.to_number();
    assert!(
        count >= 20.0,
        "expected >=20 dissem control values, got {count}"
    );
}

#[test]
fn xpath_extracts_cve_sci_values() {
    let cve_path = crate_root().join("schemas/ISM-v2022-DEC/CVE_ISM/CVEnumISMSCIControls.xml");
    if !cve_path.exists() {
        return;
    }

    let content = std::fs::read_to_string(&cve_path).unwrap();
    let doc = xmloxide::Document::parse_str(&content).unwrap();
    let root = doc.root_element().unwrap();

    let result = xmloxide::xpath::evaluate(&doc, root, "count(//*[local-name()='Value'])").unwrap();
    let count = result.to_number();
    assert!(
        count >= 15.0,
        "expected >=15 SCI control values, got {count}"
    );
}
