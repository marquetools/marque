use super::{WasmConfig, build_cache_key, parse_deadline_ms, parse_wasm_config};
use std::time::Duration;

fn default_wasm_config() -> WasmConfig {
    WasmConfig {
        classifier_id: None,
        classification_authority: None,
        confidence_threshold: None,
        corrections: None,
        deadline_ms: None,
        signature: None,
    }
}

#[test]
fn parse_deadline_ms_none_yields_none() {
    assert_eq!(parse_deadline_ms(None).unwrap(), None);
}

#[test]
fn parse_deadline_ms_zero_yields_zero_duration() {
    assert_eq!(parse_deadline_ms(Some(0.0)).unwrap(), Some(Duration::ZERO));
}

#[test]
fn parse_deadline_ms_positive_rounds_down() {
    assert_eq!(
        parse_deadline_ms(Some(1.7)).unwrap(),
        Some(Duration::from_millis(1))
    );
}

#[test]
fn parse_deadline_ms_negative_returns_error() {
    let err = parse_deadline_ms(Some(-1.0)).unwrap_err();
    assert!(
        err.contains("non-negative"),
        "error must mention non-negative constraint, got: {err}"
    );
}

#[test]
fn parse_deadline_ms_nan_returns_error() {
    let err = parse_deadline_ms(Some(f64::NAN)).unwrap_err();
    assert!(
        !err.is_empty(),
        "NaN deadline must produce a non-empty error"
    );
}

#[test]
fn parse_deadline_ms_positive_infinity_returns_error() {
    let err = parse_deadline_ms(Some(f64::INFINITY)).unwrap_err();
    assert!(
        !err.is_empty(),
        "+Inf deadline must produce a non-empty error"
    );
}

#[test]
fn parse_deadline_ms_negative_infinity_returns_error() {
    let err = parse_deadline_ms(Some(f64::NEG_INFINITY)).unwrap_err();
    assert!(
        !err.is_empty(),
        "-Inf deadline must produce a non-empty error"
    );
}

#[test]
fn parse_deadline_ms_large_value_saturates_without_panic() {
    let result = parse_deadline_ms(Some(f64::MAX));
    assert!(result.is_ok(), "very large deadline must not panic");
}

#[test]
fn build_cache_key_is_none_for_default_config() {
    let cfg = default_wasm_config();
    assert_eq!(build_cache_key(&cfg).unwrap(), None);
}

#[test]
fn build_cache_key_is_none_for_empty_corrections() {
    let cfg = WasmConfig {
        corrections: Some(Default::default()),
        ..default_wasm_config()
    };
    assert_eq!(build_cache_key(&cfg).unwrap(), None);
}

#[test]
fn build_cache_key_is_none_for_deadline_only() {
    let cfg = WasmConfig {
        deadline_ms: Some(5000.0),
        ..default_wasm_config()
    };
    assert_eq!(build_cache_key(&cfg).unwrap(), None);
}

#[test]
fn build_cache_key_is_some_for_classifier_id() {
    let cfg = WasmConfig {
        classifier_id: Some("TEST-WASM-42".to_owned()),
        ..default_wasm_config()
    };
    assert!(build_cache_key(&cfg).unwrap().is_some());
}

#[test]
fn build_cache_key_is_some_for_classification_authority() {
    // issue #399: classification_authority rides on engine config, so
    // it participates in the cache key like classifier_id.
    let cfg = WasmConfig {
        classification_authority: Some("EO 13526".to_owned()),
        ..default_wasm_config()
    };
    assert!(build_cache_key(&cfg).unwrap().is_some());
}

#[test]
fn build_cache_key_is_none_for_signature_only() {
    // issue #399: signature is a per-call FixOptions field, NOT engine
    // config — it must never affect which cached engine is reused.
    let cfg = WasmConfig {
        signature: Some("SIG".to_owned()),
        ..default_wasm_config()
    };
    assert_eq!(build_cache_key(&cfg).unwrap(), None);
}

#[test]
fn build_cache_key_is_some_for_confidence_threshold() {
    let cfg = WasmConfig {
        confidence_threshold: Some(0.75),
        ..default_wasm_config()
    };
    assert!(build_cache_key(&cfg).unwrap().is_some());
}

#[test]
fn build_cache_key_is_some_for_nonempty_corrections() {
    let cfg = WasmConfig {
        corrections: Some(
            [("NF".to_owned(), "NOFORN".to_owned())]
                .into_iter()
                .collect(),
        ),
        ..default_wasm_config()
    };
    assert!(build_cache_key(&cfg).unwrap().is_some());
}

#[test]
fn build_cache_key_is_stable_for_equal_corrections() {
    use std::collections::HashMap;
    let mut m1: HashMap<String, String> = HashMap::new();
    m1.insert("NF".to_owned(), "NOFORN".to_owned());
    m1.insert("SI".to_owned(), "SPECIAL INTELLIGENCE".to_owned());

    let mut m2: HashMap<String, String> = HashMap::new();
    m2.insert("SI".to_owned(), "SPECIAL INTELLIGENCE".to_owned());
    m2.insert("NF".to_owned(), "NOFORN".to_owned());

    let k1 = build_cache_key(&WasmConfig {
        corrections: Some(m1),
        ..default_wasm_config()
    })
    .unwrap();

    let k2 = build_cache_key(&WasmConfig {
        corrections: Some(m2),
        ..default_wasm_config()
    })
    .unwrap();

    assert_eq!(k1, k2);
}

#[test]
fn build_cache_key_differs_for_different_classifier_ids() {
    let k1 = build_cache_key(&WasmConfig {
        classifier_id: Some("TEST-WASM-42".to_owned()),
        ..default_wasm_config()
    })
    .unwrap();

    let k2 = build_cache_key(&WasmConfig {
        classifier_id: Some("TEST-CLASSIFIER-42".to_owned()),
        ..default_wasm_config()
    })
    .unwrap();

    assert_ne!(k1, k2);
}

#[test]
fn r2_byte_identity_none_json() {
    let (_, _, cache_key) = parse_wasm_config(&None).expect("parse default");
    assert_eq!(cache_key, None);
}

#[test]
fn r2_byte_identity_empty_object() {
    let (_, _, cache_key) = parse_wasm_config(&Some("{}".to_owned())).expect("parse empty object");
    assert_eq!(cache_key, None);
}

#[test]
fn r2_byte_identity_empty_corrections() {
    let (_, _, cache_key) = parse_wasm_config(&Some(r#"{"corrections": {}}"#.to_owned()))
        .expect("parse empty corrections");
    assert_eq!(cache_key, None);
}

#[test]
fn r2_byte_identity_deadline_only() {
    let (_, _, cache_key) = parse_wasm_config(&Some(r#"{"deadline_ms": 1000}"#.to_owned()))
        .expect("parse deadline only");
    assert_eq!(cache_key, None);
}

#[test]
fn r2_byte_identity_classifier_id_only() {
    let (_, _, cache_key) = parse_wasm_config(&Some(r#"{"classifier_id": "agent42"}"#.to_owned()))
        .expect("parse classifier_id");
    assert_eq!(cache_key.as_deref(), Some(r#"{"classifier_id":"agent42"}"#));
}

#[test]
fn r2_byte_identity_threshold_only() {
    let (_, _, cache_key) =
        parse_wasm_config(&Some(r#"{"confidence_threshold": 0.85}"#.to_owned()))
            .expect("parse threshold");
    assert_eq!(
        cache_key.as_deref(),
        Some(r#"{"confidence_threshold":0.85}"#)
    );
}

#[test]
fn r2_byte_identity_corrections_sorted() {
    let (_, _, cache_key) = parse_wasm_config(&Some(
        r#"{"corrections": {"MGT": "MANAGEMENT", "DOC1": "DOCUMENT"}}"#.to_owned(),
    ))
    .expect("parse corrections");
    assert_eq!(
        cache_key.as_deref(),
        Some(r#"{"corrections":{"DOC1":"DOCUMENT","MGT":"MANAGEMENT"}}"#)
    );
}

#[test]
fn r2_byte_identity_all_three() {
    let (_, _, cache_key) = parse_wasm_config(&Some(
        r#"{"classifier_id": "agent42", "confidence_threshold": 0.85, "corrections": {"K": "V"}}"#
            .to_owned(),
    ))
    .expect("parse all three");
    assert_eq!(
        cache_key.as_deref(),
        Some(r#"{"classifier_id":"agent42","confidence_threshold":0.85,"corrections":{"K":"V"}}"#)
    );
}

#[test]
fn r2_byte_identity_deadline_plus_classifier() {
    let (_, _, cache_key) = parse_wasm_config(&Some(
        r#"{"deadline_ms": 1000, "classifier_id": "agent42"}"#.to_owned(),
    ))
    .expect("parse deadline + classifier");
    assert_eq!(cache_key.as_deref(), Some(r#"{"classifier_id":"agent42"}"#));
}

#[test]
fn r2_classifier_id_wrong_type_errors() {
    let result = parse_wasm_config(&Some(r#"{"classifier_id": 123}"#.to_owned()));
    assert!(result.is_err());
}

#[test]
fn r2_confidence_threshold_wrong_type_errors() {
    let result = parse_wasm_config(&Some(r#"{"confidence_threshold": "high"}"#.to_owned()));
    assert!(result.is_err());
}

#[test]
fn r2_corrections_array_errors() {
    let result = parse_wasm_config(&Some(r#"{"corrections": ["a", "b"]}"#.to_owned()));
    assert!(result.is_err());
}

#[test]
fn r2_corrections_non_string_value_errors() {
    let result = parse_wasm_config(&Some(r#"{"corrections": {"k": 123}}"#.to_owned()));
    assert!(result.is_err());
}

#[test]
fn r2_deadline_ms_wrong_type_errors() {
    let result = parse_wasm_config(&Some(r#"{"deadline_ms": "soon"}"#.to_owned()));
    assert!(result.is_err());
}

#[test]
fn r2_top_level_string_errors() {
    let result = parse_wasm_config(&Some(r#""hello""#.to_owned()));
    assert!(result.is_err());
}

#[test]
fn r2_top_level_array_errors() {
    let result = parse_wasm_config(&Some(r#"[1, 2]"#.to_owned()));
    assert!(result.is_err());
}

#[test]
fn r2_top_level_number_errors() {
    let result = parse_wasm_config(&Some(r#"42"#.to_owned()));
    assert!(result.is_err());
}

#[test]
fn r2_unknown_field_silently_ignored() {
    let (cfg, _, cache_key) = parse_wasm_config(&Some(
        r#"{"some_unknown_field": 42, "another_one": "value"}"#.to_owned(),
    ))
    .expect("unknown fields must NOT produce an error");
    assert_eq!(cfg.classifier_id, None);
    assert_eq!(cfg.confidence_threshold, None);
    assert!(cfg.corrections.is_none());
    assert_eq!(cfg.deadline_ms, None);
    assert_eq!(cache_key, None);
}

#[test]
fn r2_unknown_field_coexists_with_known() {
    let (cfg, _, cache_key) = parse_wasm_config(&Some(
        r#"{"future_field": true, "classifier_id": "agent42"}"#.to_owned(),
    ))
    .expect("unknown + known must not error");
    assert_eq!(cfg.classifier_id.as_deref(), Some("agent42"));
    assert_eq!(cache_key.as_deref(), Some(r#"{"classifier_id":"agent42"}"#));
}

#[test]
fn seconds_per_julian_year_constant_is_correct() {
    let expected = (365.25_f64 * 24.0 * 3600.0) as u64;
    assert_eq!(crate::banner::SECONDS_PER_JULIAN_YEAR, expected);
}

#[test]
fn current_year_is_plausible() {
    let year = crate::banner::current_year();
    assert!(year >= 2026, "current_year must be ≥ 2026, got {year}");
    assert!(year <= 2100, "current_year must be ≤ 2100, got {year}");
}
