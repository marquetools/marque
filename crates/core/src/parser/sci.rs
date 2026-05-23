use super::*;

/// Structural subparser for the SCI category block per CAPCO-2016 §A.6.
///
/// Grammar (spec 003-sci-compartments §R2):
///
/// ```text
/// SCI_BLOCK      := SCI_SYSTEM ("/" SCI_SYSTEM)*
/// SCI_SYSTEM     := CONTROL (-COMPARTMENT)*
/// CONTROL        := BARE_CONTROL | CUSTOM_CONTROL
/// BARE_CONTROL   := any bare CVE value (via is_bare_cve_value)
/// CUSTOM_CONTROL := [A-Z0-9]{2,5} (not matching a BARE_CONTROL)
/// COMPARTMENT    := COMP_ID (SPACE SUB_COMP)*
/// COMP_ID        := [A-Z0-9]+
/// SUB_COMP       := [A-Z0-9]+
/// ```
///
/// Returns `Some(markings)` on successful structural parse, `None` on any
/// grammar violation (dangling hyphens, leading hyphens, lowercase,
/// empty compartments, invalid custom shape). On `None`, the caller falls
/// back to the existing `SciControl::parse` exact-match path.
///
/// `canonical_enum` is populated via `format!("{ctrl}-{first_comp}").parse::<SciControl>()`
/// ONLY when the matching compartment has no sub-compartments — sub-comps
/// imply the compound is a structural anchor, not an atomic CVE value.
///
/// On success, emits TokenSpan entries (SciSystem / SciCompartment /
/// SciSubCompartment) at byte-precise offsets relative to `base`.
pub(super) fn parse_sci_block(
    text: &str,
    base: usize,
    tokens: &mut SmallVec<[TokenSpan; 16]>,
) -> Option<SmallVec<[SciMarking; 2]>> {
    if text.is_empty() {
        return None;
    }

    // Buffer tokens into a local scratch so we can discard them if any
    // system fails to parse (all-or-nothing success semantics per spec).
    // Inline-16 mirrors the outer `tokens` budget — even a multi-system
    // block like `SI-G ABCD DEFG-MMM AACD` emits well under 16 spans.
    let mut local_tokens: SmallVec<[TokenSpan; 16]> = SmallVec::new();
    // Inline-2: most SCI blocks carry one system; the §A.6 grammar example
    // `123/SI-G ABCD DEFG-MMM AACD` is two. Three- or four-system blocks
    // spill cleanly.
    let mut markings: SmallVec<[SciMarking; 2]> = SmallVec::new();

    // Split on `/` into per-system chunks, tracking byte offsets so each
    // TokenSpan's `span` is accurate relative to the original source.
    // Inline-4: same scale as `markings` (one chunk per `/`-separated
    // system); inline-4 covers up to four systems without spilling.
    let mut chunk_start = 0usize;
    let chunks: SmallVec<[(usize, &str); 4]> = {
        let mut v: SmallVec<[(usize, &str); 4]> = SmallVec::new();
        for (i, ch) in text.char_indices() {
            if ch == '/' {
                v.push((chunk_start, &text[chunk_start..i]));
                chunk_start = i + 1;
            }
        }
        v.push((chunk_start, &text[chunk_start..]));
        v
    };

    for (chunk_off, chunk) in chunks {
        // No trim — grammar is strict; whitespace inside a chunk is
        // meaningful only between sub-compartments (see below).
        if chunk.is_empty() {
            return None;
        }
        // Leading hyphen rejects immediately (e.g., `-SI`).
        if chunk.starts_with('-') {
            return None;
        }

        // Deprecated SCI long-form per-chunk recognition (PR 9a Copilot R4,
        // PR #416). When the chunk matches a deprecated long-form
        // (`HUMINT`, `COMINT`, `KDK-BLUEFISH`, `ECI ABC`, ...), route it
        // through the canonical-system enum path BEFORE the structural
        // CVE-bare / custom-control checks below — otherwise:
        //
        // - `HUMINT` / `COMINT` / `KLONDIKE-IDIT` (>5 chars) would fall
        //   through to `is_valid_custom_control`, fail (length cap), and
        //   reject the whole block, so multi-system inputs like
        //   `(TS//COMINT/TK//NF)` get tagged Unknown and the E065 walker
        //   never sees them.
        // - `KDK-BLUEFISH` (3-char prefix → fits `is_valid_custom_control`)
        //   currently lands as `SciControlSystem::Custom("KDK")` with a
        //   `BLUEFISH` compartment; the walker still fires via the
        //   `TokenKind::SciControl` text-prefix match, but the resulting
        //   `SciMarking` is structurally Custom (triggering W034
        //   unpublished-control noise) instead of canonical `Published(Tk)`
        //   with the canonical `BLFH` compartment.
        // - `ECI ABC` / `EL ECRU` (space inside the chunk) parses neither
        //   as CVE-bare nor custom-control and rejects the whole block.
        //
        // Routing through `recognize_deprecated_sci_long_form` here yields
        // a canonical `SciMarking`, dual-emits `TokenKind::SciControl` +
        // `TokenKind::SciSystem` spans with the chunk source bytes
        // verbatim (so the E065 walker can identify the deprecated form
        // via `TokenSpan.text`), and preserves the per-chunk
        // all-or-nothing parse semantic — if the recognizer doesn't match,
        // we fall through to the existing CVE-bare / custom-control
        // path; if it does match, we `continue` to the next chunk.
        //
        // Authority: CAPCO-2016 §H.4 pp 61, 62, 74, 76, 78, 85 — see
        // per-row citations in `recognize_deprecated_sci_long_form`.
        if let Some(long_form) = recognize_deprecated_sci_long_form(chunk) {
            // Build compartments from the recognizer's optional compartment
            // slot. The recognizer guarantees `is_alnum_upper(comp)` for
            // every PrefixSpace/PrefixHyphen variant (see the
            // `recognize_deprecated_sci_long_form` body), so this is safe.
            let compartments: Box<[SciCompartment]> = match &long_form.compartment {
                Some(comp) => Box::new([SciCompartment::new(comp.as_str(), Box::new([]))]),
                None => Box::new([]),
            };
            // Canonical-enum lookup mirrors the whole-block long-form path
            // at parser.rs ~line 422: bare control → `SciControl::parse`
            // on the system name (`HCS` / `SI` / `TK`); compound form →
            // `{ctrl}-{comp}` lookup (e.g., `TK-BLFH`).
            let canonical_enum = if compartments.is_empty() {
                SciControl::parse(long_form.system.as_str())
            } else {
                compartments.first().and_then(|c| {
                    let composite = format!("{}-{}", long_form.system.as_str(), c.identifier);
                    SciControl::parse(&composite)
                })
            };
            // Source bytes preserved verbatim — the E065 walker reads
            // `TokenSpan.text` to identify the deprecated form. Dual-emit
            // SciControl + SciSystem spans matching the whole-block
            // long-form path's invariant (parser.rs ~lines 441-475) and
            // the structural path's pattern (lines 1107-1118 below).
            let chunk_abs = base + chunk_off;
            let chunk_span = Span::new(chunk_abs, chunk_abs + chunk.len());
            local_tokens.push(TokenSpan {
                kind: TokenKind::SciControl,
                span: chunk_span,
                text: chunk.into(),
            });
            local_tokens.push(TokenSpan {
                kind: TokenKind::SciSystem,
                span: chunk_span,
                text: chunk.into(),
            });
            markings.push(SciMarking::new(
                SciControlSystem::Published(long_form.system),
                compartments,
                canonical_enum,
            ));
            continue;
        }

        // NATO SAP per-chunk recognition (PR 9c.1 R1 — canonical-form
        // round-trip closure). BOHEMIA / BALK render standalone in the
        // SCI block with no compartments per §G.2 p40 + §H.7 p127. The
        // bare token IS the chunk (no `-` separator, no compartments),
        // so we recognize it here BEFORE the `SciControlBare::parse` /
        // `is_valid_custom_control` path — `BOHEMIA` would otherwise
        // fail both branches (no CVE entry; 7 chars exceeds the 5-char
        // custom-control cap), and `BALK` would land as
        // `SciControlSystem::Custom("BALK")` (wrong axis).
        //
        // Combined forms like `BALK/BOHEMIA` flow through this branch
        // per chunk — the outer `/`-split has already broken the block
        // into single-token chunks before per-chunk dispatch.
        //
        // Authority: CAPCO-2016 §G.2 p40 (Table 5 registers BOHEMIA /
        // BALK as standalone control markings — no compartments, no
        // sub-compartments); §H.7 p127 (worked example
        // `TOP SECRET//BOHEMIA//FGI AUS CAN DEU NATO//NOFORN` and the
        // matching portion `(//CTS//BOHEMIA//REL TO USA, NATO)` —
        // BOHEMIA travels alone in the SCI block).
        if let Some(sap) = recognize_nato_sap(chunk) {
            let chunk_abs = base + chunk_off;
            let chunk_span = Span::new(chunk_abs, chunk_abs + chunk.len());
            // Dual-emit SciControl + SciSystem spans matching the
            // deprecated-long-form and CVE-bare paths so rules that
            // anchor on `TokenKind::SciSystem` find a matching span.
            local_tokens.push(TokenSpan {
                kind: TokenKind::SciControl,
                span: chunk_span,
                text: chunk.into(),
            });
            local_tokens.push(TokenSpan {
                kind: TokenKind::SciSystem,
                span: chunk_span,
                text: chunk.into(),
            });
            markings.push(SciMarking::new(
                SciControlSystem::NatoSap(sap),
                Box::new([]),
                None,
            ));
            continue;
        }

        // Split chunk on first `-` into (control, rest). If no `-`, the
        // whole chunk is the control with no compartments.
        let (ctrl_str, rest_opt) = match chunk.find('-') {
            Some(i) => (&chunk[..i], Some(&chunk[i + 1..])),
            None => (chunk, None),
        };

        if ctrl_str.is_empty() {
            return None;
        }

        // Recognize control: bare CVE first, then long-form Authorized
        // Banner Line Marking Title (e.g. `TALENT KEYHOLE` → `TK`,
        // `MARVEL` → `MVL`, `KLAMATH` → `KLM`) via MARKING_FORMS, then
        // custom [A-Z0-9]{2,5}. Long-form acceptance is required so a
        // chunk like `TALENT KEYHOLE` inside an `SI/TALENT KEYHOLE`
        // block routes through the structural path and produces a
        // canonical SciMarking, rather than falling through to the
        // flat within-block sub-token chain (which populates
        // `sci_controls` only). A custom control must not collide
        // with any other known category (Dissem / NonIcDissem / Sar
        // / Aea / DeclassExemption) — otherwise a block like `SI/NF`
        // would be mis-claimed as SCI instead of flagged as a stray
        // `/` by E004.
        //
        // Authority: CAPCO-2016 §D.1 p27 (any control marking in the
        // banner line may be spelled out per the Marking Title);
        // §H.4 p85 (TALENT KEYHOLE is the registered title for TK).
        let system: SciControlSystem = if let Some(bare) = SciControlBare::parse(ctrl_str) {
            SciControlSystem::Published(bare)
        } else if let Some(portion) = marque_ism::marking_forms::title_to_portion(ctrl_str)
            && let Some(bare) = SciControlBare::parse(portion)
        {
            SciControlSystem::Published(bare)
        } else if is_valid_custom_control(ctrl_str) && !is_known_non_sci_token(ctrl_str) {
            SciControlSystem::Custom(ctrl_str.into())
        } else {
            return None;
        };

        // Emit a block-level SciControl span covering the full system
        // chunk (control + compartments + sub-compartments), mirroring the
        // existing exact-match path so rule consumers (E010, E011, and
        // audit tooling that reads TokenKind::SciControl) continue to see
        // one span per marking. The granular SciSystem/SciCompartment/
        // SciSubCompartment spans below provide finer-grained structure
        // for spec 003 rules (E032–E035).
        let chunk_abs = base + chunk_off;
        local_tokens.push(TokenSpan {
            kind: TokenKind::SciControl,
            span: Span::new(chunk_abs, chunk_abs + chunk.len()),
            text: chunk.into(),
        });
        // Emit SciSystem token for the control identifier itself.
        let ctrl_abs = base + chunk_off;
        local_tokens.push(TokenSpan {
            kind: TokenKind::SciSystem,
            span: Span::new(ctrl_abs, ctrl_abs + ctrl_str.len()),
            text: ctrl_str.into(),
        });

        // Parse compartments. `rest` is the substring after the first `-`.
        // Each additional compartment is preceded by another `-`, and
        // sub-compartments within a compartment are space-separated.
        // Inline-4: the §A.6 grammar example `SI-G ABCD DEFG-MMM AACD`
        // shows two compartments (G, MMM) per system; CAPCO real-world
        // markings typically cap at ~4 compartments per system.
        let mut compartments: SmallVec<[SciCompartment; 4]> = SmallVec::new();
        if let Some(rest) = rest_opt {
            // Split `rest` on `-` into compartment segments. Strict grammar:
            // empty segment (trailing or consecutive hyphen) → reject.
            let rest_abs_base = base + chunk_off + ctrl_str.len() + 1; // +1 skips the `-`
            let mut seg_start = 0usize;
            // Inline-4: same cardinality as `compartments` (one segment per
            // `-`-separated compartment).
            let mut seg_offs: SmallVec<[(usize, &str); 4]> = SmallVec::new();
            for (i, ch) in rest.char_indices() {
                if ch == '-' {
                    seg_offs.push((seg_start, &rest[seg_start..i]));
                    seg_start = i + 1;
                }
            }
            seg_offs.push((seg_start, &rest[seg_start..]));

            for (seg_off, seg) in seg_offs {
                if seg.is_empty() {
                    return None; // dangling `-` or consecutive `--`
                }
                // Each compartment segment = COMP_ID (SPACE SUB_COMP)*
                // Split on space.
                let mut parts = seg.split(' ');
                let comp_id_src = parts.next().unwrap(); // at least one part
                if comp_id_src.is_empty() || !is_alnum_upper(comp_id_src) {
                    // Long-form compartment fallback: if the segment
                    // isn't alnum_upper (e.g. `GAMMA ABCD`'s
                    // `GAMMA` does pass; this branch handles future
                    // mixed-case input), reject. The canonicalization
                    // step below picks up alnum-upper long forms
                    // like `GAMMA`/`BLUEFISH`/`IDITAROD`/`KANDIK`.
                    return None;
                }
                // Long-form compartment canonicalization
                // (CAPCO-2016 §G.1 Table 4 + §H.4 p61, p87, p91, p95).
                // The MARKING_FORMS table records the long-form title
                // (GAMMA, BLUEFISH, IDITAROD, KANDIK) → short-form
                // portion (G, BLFH, IDIT, KAND); when the source token
                // is the long form, store the canonical short form on
                // the SciCompartment so page-level equality with
                // portion-form `SI-G ABCD` succeeds. The TokenSpan
                // retains the source bytes verbatim so audit-record
                // anchoring and the byte-identity round-trip stay
                // accurate.
                let comp_id =
                    marque_ism::marking_forms::title_to_portion(comp_id_src).unwrap_or(comp_id_src);

                let comp_abs = rest_abs_base + seg_off;
                local_tokens.push(TokenSpan {
                    kind: TokenKind::SciCompartment,
                    span: Span::new(comp_abs, comp_abs + comp_id_src.len()),
                    text: comp_id_src.into(),
                });

                // Inline-4: the §A.6 grammar example shows 2 sub-comps
                // per compartment (`G ABCD DEFG`, `MMM AACD`); real-world
                // markings rarely exceed 4 per compartment.
                let mut subs: SmallVec<[SmolStr; 4]> = SmallVec::new();
                // Track cursor within segment for sub-compartment offsets.
                // Use the SOURCE length (`comp_id_src.len()`) — the
                // cursor walks the source bytes, while `comp_id` may
                // be the canonicalized short form after long-form
                // lookup (e.g. `GAMMA` source maps to `G` canonical,
                // but the sub-compartment span starts after the source
                // `GAMMA` token, not after a 1-byte `G`).
                let mut sub_cursor = comp_id_src.len() + 1; // +1 skips the space
                for sub in parts {
                    if sub.is_empty() || !is_alnum_upper(sub) {
                        return None;
                    }
                    let sub_abs = rest_abs_base + seg_off + sub_cursor;
                    local_tokens.push(TokenSpan {
                        kind: TokenKind::SciSubCompartment,
                        span: Span::new(sub_abs, sub_abs + sub.len()),
                        text: sub.into(),
                    });
                    subs.push(sub.into());
                    sub_cursor += sub.len() + 1;
                }

                compartments.push(SciCompartment::new(comp_id, subs.into_boxed_slice()));
            }
        }

        // canonical_enum population (per data-model §canonical_enum):
        // - No compartments → the bare control itself may be a CVE value
        //   (e.g., `SI`, `TK`, `HCS`). Preserves pre-spec behavior.
        // - One or more compartments → try `{ctrl}-{first_comp}` ONLY when
        //   the first compartment has no sub-compartments. Sub-comps mean
        //   the compound is a structural anchor, not an atomic CVE atom.
        //
        // Resolve the control's canonical short form for CVE lookup —
        // `ctrl_str` may be a long-form title (e.g. `TALENT KEYHOLE`)
        // after the long-form gate at the system-resolution step above.
        // `marking_forms::title_to_portion` returns the canonical
        // portion form when the input is a long form; otherwise the
        // source token already IS the canonical form.
        let ctrl_canonical =
            marque_ism::marking_forms::title_to_portion(ctrl_str).unwrap_or(ctrl_str);
        let canonical_enum = if compartments.is_empty() {
            SciControl::parse(ctrl_canonical)
        } else {
            compartments
                .first()
                .filter(|c| c.sub_compartments.is_empty())
                .and_then(|c| {
                    let composite = format!("{}-{}", ctrl_canonical, c.identifier);
                    SciControl::parse(&composite)
                })
        };

        markings.push(SciMarking::new(
            system,
            compartments.into_boxed_slice(),
            canonical_enum,
        ));
    }

    tokens.extend(local_tokens);
    Some(markings)
}

/// Custom control shape check: `[A-Z0-9]{2,5}` per spec §R1. Must not match
/// a bare CVE value (caller dispatches to Published first, so this check is
/// strictly the shape constraint).
pub(super) fn is_valid_custom_control(s: &str) -> bool {
    let len = s.len();
    (2..=5).contains(&len) && is_alnum_upper(s)
}

/// Returns true if `s` is non-empty and every byte is ASCII uppercase or digit.
pub(super) fn is_alnum_upper(s: &str) -> bool {
    !s.is_empty()
        && s.bytes()
            .all(|b| b.is_ascii_uppercase() || b.is_ascii_digit())
}

/// Guard for the SCI structural subparser: returns true if `s` is a known
/// non-SCI token (dissem, non-IC dissem, AEA marking, or declass exemption).
/// Prevents `parse_sci_block` from claiming mixed-category slash blocks
/// like `SI/NF` that should surface as stray-slash errors. SAR is
/// structural (not CVE-backed) and handled by `parse_sar_category`.
pub(super) fn is_known_non_sci_token(s: &str) -> bool {
    DissemControl::parse(s).is_some()
        || parse_dissem_full_form(s).is_some()
        || parse_non_ic_full_form(s).is_some()
        || AeaMarking::parse(s).is_some()
        || DeclassExemption::parse(s).is_some()
}

/// Recognize a bare NATO Special Access Program token (`BOHEMIA` or
/// `BALK`). Returns the matching [`NatoSap`] variant when `s` is one of
/// the two registered NATO SAPs; returns `None` otherwise.
///
/// NATO SAPs travel in the SCI block position (`//CTS//BOHEMIA`,
/// `//CTS//BALK`) and render standalone with no compartments or
/// sub-compartments — they are CAPCO-only tokens with no ODNI CVE
/// registration, so the structural SCI block parser needs an explicit
/// recognizer rather than going through the bare-CVE / custom-control
/// fallbacks.
///
/// Matching is strict on case (uppercase only, matching every other
/// CAPCO token recognizer in this file).
///
/// # Authority
///
/// - CAPCO-2016 §G.2 p40 (Table 5: ARH by Registered Marking —
///   registers BOHEMIA / BALK as standalone control markings, not
///   classification suffixes).
/// - CAPCO-2016 §H.7 p127 (FGI worked example
///   `TOP SECRET//BOHEMIA//FGI AUS CAN DEU NATO//NOFORN` and the
///   matching portion `(//CTS//BOHEMIA//REL TO USA, NATO)` — places
///   BOHEMIA in the SCI block position alongside the FGI block).
pub(super) fn recognize_nato_sap(s: &str) -> Option<NatoSap> {
    match s {
        "BOHEMIA" => Some(NatoSap::Bohemia),
        "BALK" => Some(NatoSap::Balk),
        _ => None,
    }
}
