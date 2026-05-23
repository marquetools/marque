use super::*;

/// Quick gate for "does this block start with an FGI marker, in either
/// the abbreviation or long-form?".
///
/// Mirrors the prefix-check that gates
/// [`parse_fgi_marker_with_spans`] dispatch in the block-walking
/// parser. Centralizing it here keeps the abbreviation/long-form set
/// in lock-step between the gate and the parser. Long-form authority:
/// CAPCO-2016 §H.7 p123 (Authorized Banner Line Marking Title:
/// `FOREIGN GOVERNMENT INFORMATION [LIST]` for the acknowledged form
/// / `FOREIGN GOVERNMENT INFORMATION` for the concealed form). Pair
/// with the `fgi_gate_lock_steps_with_parser` integration test in
/// `crates/core/tests/banner_long_forms.rs`, which exercises the
/// end-to-end contract: every input that should reach
/// [`parse_fgi_marker_with_spans`] must first pass this gate.
pub(super) fn starts_with_fgi_prefix(s: &str) -> bool {
    s == "FGI"
        || s.starts_with("FGI ")
        || s == "FOREIGN GOVERNMENT INFORMATION"
        || s.starts_with("FOREIGN GOVERNMENT INFORMATION ")
}

/// Parse an FGI marker block in a US-classified marking.
///
/// **This is the production entry point** — called directly from the
/// block walker in [`Parser::parse_marking_string`] (around
/// `parser.rs:704` inside `Parser::parse_marking_string`). The
/// `#[cfg(test)]` [`parse_fgi_marker`] wrapper below delegates here
/// and discards the span buffer; production paths thread real
/// `block_offset` + `token_spans` arguments through this function so
/// per-country [`TokenKind::FgiOwnershipTrigraph`] spans land in the
/// AST for `FgiOwnershipTrigraphSuggestRule` (issue #545) to consume.
///
/// This is the FGI block between SAR and dissem controls in a
/// US-classified marking (e.g., `SECRET//FGI DEU//NOFORN`). Not to
/// be confused with [`parse_fgi_classification`] which parses a
/// non-US classification.
///
/// # Span emission
///
/// Emits one [`TokenKind::FgiOwnershipTrigraph`] span per
/// shape-admitted country token in the ownership list, appended into
/// `token_spans` alongside the block-level [`TokenKind::FgiMarker`]
/// span the caller pushes on its own. Dual emission mirrors
/// [`parse_rel_to_with_spans`] (`RelToBlock` + `RelToTrigraph`).
///
/// `block_offset` is the absolute byte offset of `block` within the
/// original source buffer. Per-country sub-spans are computed by
/// walking `block` with a hand-rolled ASCII-whitespace cursor (NOT
/// `split_whitespace`, which discards byte positions) so the offsets
/// land exactly on the country token, not on a leading or trailing
/// space.
///
/// The function stages per-country spans into a local `pending`
/// buffer and flushes them to `token_spans` only after
/// [`FgiMarker::acknowledged`] confirms a non-empty acknowledged
/// result. The FR-016 closure (any token failing the shape gate, or
/// an empty list after the prefix, returns `None` with no per-country
/// spans leaked) is preserved through this staging step.
///
/// # Three return cases (FR-015 / FR-016 closure, GH #280)
///
/// Per CAPCO-2016 §H.7 p122 the FGI marker has exactly two lawful
/// banner forms: bare `FGI` (source-concealed) and `FGI [LIST]`
/// (source-acknowledged). Anything else is a parse failure, not a
/// degraded lawful form. This function enforces that as three
/// disjoint return cases:
///
/// | Input shape | Return |
/// |-------------|--------|
/// | `"FGI"` exactly (no whitespace, no suffix) | `Some(SourceConcealed)` |
/// | `"FGI " + tokens` where every token is a 2-, 3-, or 4-letter ASCII upper code (registered exception / Annex B trigraph / Annex A tetragraph) | `Some(Acknowledged { countries })` |
/// | Anything else (malformed prefix, any token fails the country-token shape gate, OR empty list after `"FGI "`) | `None` |
///
/// The third row is the FR-016 closure: a post-failure shape MUST
/// be `None`, never a degraded `Some(SourceConcealed)`. The
/// transitional T094 fallback (`...unwrap_or(SourceConcealed)`) was
/// removed in T088+T093 so a parse failure surfaces honestly to the
/// rule layer instead of being silently re-cast as lawful
/// concealment. A diagnostic for malformed FGI input is the rule
/// layer's job; the parser's job is to refuse to mint a misleading
/// AST.
///
/// # Country-token shape gate
///
/// Token admission goes through
/// [`marque_ism::CountryCode::admits_fgi_ownership_token`] — a
/// shape-only FGI-ownership predicate that admits any 2- or 3-byte
/// ASCII-upper token OR the literal `NATO` tetragraph. This is
/// intentionally narrower than the FGI/REL TO list-token predicate
/// [`marque_ism::CountryCode::admits_country_token`] (which admits
/// any 2-4 ASCII upper token) on the tetragraph axis only: issue
/// #280 narrowed the FGI ownership slot so distribution-list
/// tetragraphs (`FVEY`, `CFIUS`, `ACGU`, `ISAF`) reject — per §H.7
/// they describe who may receive a marking, not who owns it; they
/// are lawful in REL TO list slots, not FGI ownership slots. `NATO`
/// is the only alliance tetragraph CAPCO treats as an ownership
/// identifier in this slot.
///
/// The 2- and 3-byte branches are shape-only — any conformant
/// uppercase token admits, including unregistered ones like `XX`
/// or `ZZZ`. Registry validation is the rule layer's job (S004
/// trigraph-suggest, E008 unknown-token); see the predicate's
/// doc-comment for the full rationale. This matches the project's
/// established parser/rule split (the parser produces well-formed
/// AST nodes; the rule layer flags unknown tokens with actionable
/// diagnostics). Decoder coordination tracked at #496.
///
/// This is a deliberate FR-015 surface mismatch with the broader
/// `Vocabulary<CapcoScheme>::shape_admits(CAT_FGI_MARKER, _)` arm
/// in `marque-capco/src/vocabulary.rs` — the vocabulary surface
/// continues to call `admits_country_token` for round-trip
/// compatibility with the cross-axis admission contract. The parser
/// is the FGI-ownership-narrowed path; vocabulary stays broader.
///
/// CAPCO-2016 §A.6 p16 spells out the list shape and gives the
/// canonical multi-country example: "Multiple FGI trigraph country
/// codes or tetragraph codes must be separated by a single space
/// ... An example may appear as: `SECRET//FGI GBR JPN NATO//REL TO
/// USA, GBR, JPN, NATO`." §H.7 p122 carries the ownership semantic
/// and the `FGI [LIST]` Register form that drives the surface this
/// predicate gates. The order invariant (trigraphs alphabetic, then
/// tetragraphs alphabetic) is rule-layer, not admission — real-
/// world inputs arrive in any order and a dedicated rule normalizes
/// them. Registry membership (Annex B for trigraphs) is also
/// rule-layer.
///
/// `CountryCode::try_new` is a strictly weaker predicate at this
/// site (it admits 2-15 byte values including digits and underscore
/// for `AX2` / `AX3` / `AUSTRALIA_GROUP`), so going through
/// `admits_fgi_ownership_token` first guarantees the subsequent
/// `try_new` succeeds; the construct call is therefore infallible.
/// That ordering is what lets the parser remain zero-allocation on
/// the failure path (Constitution Principle II): `?` returns `None`
/// immediately on any token-shape failure, no temporary allocation
/// needed.
///
/// # Edge cases (driven via the [`parse_fgi_marker`] test wrapper)
///
/// - `parse_fgi_marker("")` → `None` (empty input has no `FGI` prefix).
/// - `parse_fgi_marker("FGI")` → `Some(SourceConcealed)` — the bare
///   lawful concealed form.
/// - `parse_fgi_marker("FGI ")` (trailing whitespace, no tokens) →
///   `None`. The strict `"FGI "` prefix followed by zero tokens is
///   malformed input. Bare `"FGI"` (no trailing space) is the lawful
///   concealed form; the trailing space disambiguates the two
///   surfaces.
/// - `parse_fgi_marker("FGI deu")` → `None` (lowercase fails the
///   country-token shape gate; admission requires uniform ASCII upper).
/// - `parse_fgi_marker("FGI USA NATO")` →
///   `Some(Acknowledged { countries: [USA, NATO] })`. `NATO` is the
///   only ownership-context tetragraph per §H.7. Order normalization
///   (trigraph-then-tetragraph) is a rule-layer concern; the parser
///   preserves source order.
/// - `parse_fgi_marker("FGI EU")` →
///   `Some(Acknowledged { countries: [EU] })`. The 2-byte branch
///   admits per Council Decision 2013/488/EU (EU CONFIDENTIAL /
///   EU SECRET / EU TOP SECRET); EU is registered in ISMCAT
///   `CVEnumISMCATRelTo`.
/// - `parse_fgi_marker("FGI XX")` / `parse_fgi_marker("FGI ZZZ")`
///   → `Some(Acknowledged { countries: [...] })`. Shape-only
///   admission: unregistered uppercase tokens admit at the parser;
///   downstream rules (S004 / E008) carry the registry-validation
///   responsibility. See predicate doc-comment for the full
///   architectural rationale.
/// - `parse_fgi_marker("FGI DEUX")` / `parse_fgi_marker("FGI BLAH")`
///   → `None` (4-char non-`NATO` tetragraphs are distribution-list
///   markers per §H.7, not ownership identifiers; the FGI ownership
///   slot rejects them per issue #280).
/// - `parse_fgi_marker("FGI FVEY")` → `None` (FVEY is a
///   distribution-list tetragraph — lawful in REL TO, not in FGI
///   ownership context per §H.7).
/// - `parse_fgi_marker("FGI USAGB")` → `None` (5-byte token rejected
///   by the shape gate; `AUSTRALIA_GROUP`-class codes are out of
///   scope here per the §H.7 "exception is granted" carve-out).
/// - `parse_fgi_marker("foo FGI USA")` → `None` (no `FGI ` prefix
///   on the input).
///
/// # Authority
///
/// CAPCO-2016 §H.7 p122 (FGI banner forms — concealed vs.
/// acknowledged; trigraph-OR-`NATO`-tetragraph list grammar for the
/// ownership slot) + §A.6 p16 ("Multiple FGI trigraph country codes
/// or tetragraph codes must be separated by a single space"). The
/// ownership-token predicate's authority chain is documented at
/// [`marque_ism::CountryCode::admits_fgi_ownership_token`].
pub(super) fn parse_fgi_marker_with_spans(
    block: &str,
    block_offset: usize,
    token_spans: &mut SmallVec<[TokenSpan; 16]>,
) -> Option<FgiMarker> {
    // Case 1: bare `FGI` (banner abbreviation) or `FOREIGN GOVERNMENT
    // INFORMATION` (long-form title) is the lawful source-concealed
    // banner form per CAPCO-2016 §H.7 p123. No per-country spans.
    if block == "FGI" || block == "FOREIGN GOVERNMENT INFORMATION" {
        return Some(FgiMarker::SourceConcealed);
    }

    // Case 2 / Case 3 dispatch. Two prefix forms: `"FGI "` (4 bytes,
    // abbreviation) and `"FOREIGN GOVERNMENT INFORMATION "` (31
    // bytes, long form) per CAPCO-2016 §H.7 p123. `strip_prefix`
    // returning `None` on missing prefix is the Case 3 short-circuit
    // for inputs like `"FGIDEU"`, `"foo FGI USA"`, or anything else
    // that doesn't lead with the canonical separator. The actual
    // stripped length is needed for per-country sub-span offset
    // arithmetic — handle each branch explicitly rather than via a
    // chained `or_else` so the prefix length stays in scope.
    let (prefix_len, rest) = if let Some(rest) = block.strip_prefix("FGI ") {
        (4_usize, rest)
    } else if let Some(rest) = block.strip_prefix("FOREIGN GOVERNMENT INFORMATION ") {
        (31_usize, rest)
    } else {
        return None;
    };

    // Build the country list directly into the inline-4
    // `SmallVec` shape `FgiMarker::Acknowledged` carries — typical
    // FGI lists are ≤4 codes per CAPCO §H.7, so the common cases
    // (`FGI USA`, `FGI USA GBR`, the §H.7 canonical example
    // `FGI GBR JPN NATO`) stay heap-free. Walk `rest` with a
    // hand-rolled cursor so per-token byte offsets are recoverable;
    // `split_whitespace` would lose the position information.
    //
    // `pending` stages per-country spans so a parse failure later
    // in the loop does not commit half a country list. Inline-4
    // matches `countries`.
    let mut countries: SmallVec<[CountryCode; 4]> = SmallVec::new();
    let mut pending: SmallVec<[TokenSpan; 4]> = SmallVec::new();
    let base = block_offset + prefix_len;
    let bytes = rest.as_bytes();
    let mut idx = 0_usize;
    while idx < bytes.len() {
        // Skip ASCII whitespace (CAPCO §A.6 p16 specifies single
        // space, but the parser tolerates multi-space and tab
        // between tokens for resilience; a future style rule may
        // flag the non-canonical separator).
        while idx < bytes.len() && bytes[idx].is_ascii_whitespace() {
            idx += 1;
        }
        if idx >= bytes.len() {
            break;
        }
        let tok_start = idx;
        while idx < bytes.len() && !bytes[idx].is_ascii_whitespace() {
            idx += 1;
        }
        let tok_end = idx;
        let token = &rest[tok_start..tok_end];
        // Issue #280: FGI ownership context — route every token
        // through the FGI-ownership shape predicate. A 2- or
        // 3-byte ASCII-upper token (admits sovereign-state
        // trigraphs, the EU 2-byte code, AND unregistered shape-
        // conformant tokens — registry validation is the rule
        // layer's job) or the literal `NATO` tetragraph admit;
        // distribution-list tetragraphs (`FVEY`, `CFIUS`, `ACGU`,
        // `ISAF`), lowercase, digits, 5+-byte codes, and junk are
        // parse failures that return `None` — never silently dropped.
        if !CountryCode::admits_fgi_ownership_token(token.as_bytes()) {
            return None;
        }
        // `admits_fgi_ownership_token` (2- or 3-byte ASCII upper OR
        // literal `NATO`) is strictly stronger than `try_new`
        // (2-15 alphanumeric/underscore), so this construction
        // cannot fail. The `?` is here only as a type-system
        // safeguard; it is unreachable for any input that passed
        // the shape gate above.
        let code = CountryCode::try_new(token.as_bytes())?;
        countries.push(code);
        let abs_start = base + tok_start;
        let abs_end = base + tok_end;
        pending.push(TokenSpan {
            kind: TokenKind::FgiOwnershipTrigraph,
            span: Span::new(abs_start, abs_end),
            text: token.into(),
        });
    }

    // Case 3 closure: `"FGI "` followed by zero shape-admitted
    // tokens (e.g., trailing whitespace only, or input like
    // `"FGI \t"`). `FgiMarker::acknowledged` returns `None` on an
    // empty country list, which is exactly the FR-016 contract —
    // propagate it directly. This is the line that retired the
    // transitional `unwrap_or(SourceConcealed)` fallback (#280).
    //
    // Only commit the staged per-country spans on a successful
    // acknowledged parse — keeps the failure path span-clean.
    let marker = FgiMarker::acknowledged(countries)?;
    token_spans.extend(pending);
    Some(marker)
}

/// Test-only thin wrapper that delegates to
/// [`parse_fgi_marker_with_spans`] with a discarded
/// `SmallVec<[TokenSpan; 16]>` and zero base offset. The
/// `SmallVec` capacity matches the production call site, keeping the
/// wrapper heap-free for the typical FGI block (≤ 16 countries). The
/// production call site is
/// [`Parser::parse_marking_string`] (around `parser.rs:704`, inside
/// the block-walker's FGI-marker arm) and uses
/// [`parse_fgi_marker_with_spans`] directly so per-country
/// [`TokenKind::FgiOwnershipTrigraph`] spans land in the AST.
///
/// Preserved only for ergonomic inline test fixtures that assert
/// `FgiMarker` shape without driving token-span emission. See the
/// production function's doc-comment for the full grammar, edge
/// cases, FR-015 / FR-016 closure, and CAPCO §H.7 / §A.6 authority.
#[cfg(test)]
pub(super) fn parse_fgi_marker(s: &str) -> Option<FgiMarker> {
    let mut discarded: SmallVec<[TokenSpan; 16]> = SmallVec::new();
    parse_fgi_marker_with_spans(s, 0, &mut discarded)
}
