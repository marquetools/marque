// SPDX-FileCopyrightText: 2026 Knitli Inc.
//
// SPDX-License-Identifier: LicenseRef-MarqueLicense-1.0

//! REL TO category rules.
//!
//! - [`MissingUsaTrigraphRule`] — REL TO list missing USA; fix the
//!   list to lead with USA.
//! - [`PreferTetragraphCollapseRule`] — page-level Suggest to
//!   collapse explicit-member REL TO lists into a tetragraph.
//! - [`CollapseUniformRelPortionsRule`] — page-level Suggest to
//!   collapse uniform per-portion REL TO into the compact `REL`
//!   form.
//! - [`BareRelPortionDivergenceRule`] — page-level Warn when bare
//!   REL and explicit-REL-TO portions diverge.
//!
//! Predicate IDs live on each rule's `RuleId::new(...)` — the wire
//! string is the single source of truth.

use std::collections::HashSet;

use marque_ism::{CanonicalAttrs, Span, TETRAGRAPH_MEMBERS, TokenKind, TokenSpan};
use marque_rules::{
    Confidence, Diagnostic, FixIntent, FixSource, Message, MessageArgs, MessageTemplate, Phase,
    Rule, RuleContext, RuleId, Severity,
};
use marque_scheme::{
    Citation, FactRef, RecanonScope, ReplacementIntent, Scope, SectionLetter, capco,
};

use crate::scheme::CapcoScheme;

// ---------------------------------------------------------------------------
// Rule: E002 — Missing USA in REL TO trigraph list
// ---------------------------------------------------------------------------

/// E002 detects missing or misplaced `USA` in the REL TO marking template
/// from CAPCO-2016 §H.8 (p150–151, "Additional Marking Instructions"):
///
/// - Line 3713: "'USA' must always appear first whenever the REL TO string
///   is used to communicate release decisions either by the US or a Non-US
///   entity."
///
/// When E002 fires, its fix also produces a canonical REL TO list in a
/// single pass by placing `USA` first and alphabetizing the remaining
/// trigraphs. That canonicalization aligns the output with p151:
///
/// - Line 3714: "After 'USA', list the required one or more trigraph country
///   codes in alphabetical order followed by tetragraph codes listed in
///   alphabetical order. Each code is separated by a comma and a space."
///
/// E002 does not, by itself, detect alphabetical-ordering errors when `USA`
/// is already present and first; those cases are handled by the renderer's
/// REL TO axis (`render_rel_to.rs`) per CAPCO-2016 §H.8 p150–151 (pre-PR-3c.B
/// the ordering check belonged to E020 / E060, both retired). The 0.97
/// confidence is predicated on single-pass canonicalization so an E002 fix
/// does not leave behind a latent alphabetical-ordering violation for a
/// second pass.
///
/// Scope boundaries:
/// - Tetragraph alphabetization is deferred. `CountryCode` (issue
///   #183 PR-A) now carries tetragraphs, but E002 still sorts the
///   list as a flat alphabetical sequence rather than the §H.8 p151
///   "trigraphs alpha, then tetragraphs alpha, USA first" form.
///   Separate follow-up — the canonicalizer should partition true
///   country trigraphs (`code.len() == 3`) from the remaining codes
///   (the 2-byte `EU`, the 4-byte tetragraphs, and 15-byte
///   `AUSTRALIA_GROUP` belong in the non-trigraph bucket) before
///   sorting, or ideally derive the buckets from the CVE schema
///   groups in `CVEnumISMCATRelTo.xsd`.
/// - "REL TO USA" alone (p151, a non-authorized marking with no
///   following country codes) is out of scope. E002 does not fire when
///   USA is present and first; a separate rule is needed for that case.
pub(super) struct MissingUsaTrigraphRule;

/// Citations E002 may emit on diagnostics. See
/// [`Rule::cited_authorities`] for the F.1 corpus-fidelity gate
/// contract.
///
/// §H.8 p151 is the precise authority for "USA always appears first"
/// in the REL TO list. §H.8 p150 is the section anchor (REL TO marking
/// template) but the verbatim USA-first rule sits in the
/// "Additional Marking Instructions" block on p151.
const MISSING_USA_TRIGRAPH_AUTHORITIES: &[Citation] = &[capco(SectionLetter::H, 8, 151)];

impl Rule<CapcoScheme> for MissingUsaTrigraphRule {
    fn id(&self) -> RuleId {
        RuleId::new("capco", "portion.dissem.rel-to-missing-usa")
    }
    fn name(&self) -> &'static str {
        "missing-usa-trigraph"
    }
    fn default_severity(&self) -> Severity {
        Severity::Fix
    }
    /// Phase::WholeMarking: rewrites the entire REL TO block (multi-token
    /// span covering first→last `RelToTrigraph` plus any trailing
    /// separators); requires whole-marking attrs to canonicalize.
    fn phase(&self) -> Phase {
        Phase::WholeMarking
    }
    fn trusted(&self) -> bool {
        true
    }
    fn cited_authorities(&self) -> &'static [Citation] {
        MISSING_USA_TRIGRAPH_AUTHORITIES
    }
    fn check(&self, attrs: &CanonicalAttrs, ctx: &RuleContext) -> Vec<Diagnostic<CapcoScheme>> {
        if attrs.rel_to.is_empty() {
            return vec![];
        }

        let has_usa = attrs.rel_to.contains(&marque_ism::CountryCode::USA);
        let usa_first = attrs
            .rel_to
            .first()
            .is_some_and(|t| *t == marque_ism::CountryCode::USA);

        if has_usa && usa_first {
            return vec![];
        }

        // PR 3c.2.C C5 / G13: drop the runtime string distinction;
        // `MessageTemplate::NonCanonicalOrder` with `category =
        // Some(CAT_REL_TO)` identifies the violation class. Both arms
        // (missing USA / USA not first) map to the same template
        // because both are "REL TO ordering violation" per
        // §H.8 p150-151. The narrower distinction lives in the
        // `MessageArgs` populated below.
        let message = Message::new(
            MessageTemplate::NonCanonicalOrder,
            MessageArgs {
                category: Some(crate::scheme::CAT_REL_TO),
                ..MessageArgs::default()
            },
        );
        // §H.8 p151 carries the verbatim USA-first rule (the
        // Additional Marking Instructions block on the REL TO page);
        // p150 is the section anchor for the REL TO marking template
        // generally. T044 reviewer pass corrected this from p150 to
        // p151 to match the precision of `cited_authorities()` —
        // declared and emitted citations must agree (F.1 corpus-
        // fidelity gate).
        let citation = capco(SectionLetter::H, 8, 151);

        // Locate the `RelToBlock` this diagnostic refers to. If the
        // marking has more than one REL TO block (e.g.,
        // `SECRET//REL TO GBR//NF//REL TO AUS`), a single first→last
        // splice would delete intervening `//...//` content. In that
        // case we emit a diagnostic with no FixProposal and let the
        // author resolve manually. The discriminator only needs to
        // distinguish 0 / 1 / many, so we pull two iterator items and
        // match on the shape rather than allocating a `Vec`.
        let mut rel_to_blocks_iter = attrs
            .token_spans
            .iter()
            .filter(|t| t.kind == TokenKind::RelToBlock);
        let block = match (rel_to_blocks_iter.next(), rel_to_blocks_iter.next()) {
            (Some(first), None) => first,
            (None, _) => {
                // No block tagging (defensive: `attrs.rel_to` non-empty
                // should imply at least one `RelToBlock` token). Emit
                // diagnostic without a fix rather than risk mis-splice.
                // Anchor to the candidate marking span (always a real
                // in-source location) rather than a meaningless `(0, 0)`;
                // fall back to the first token span if the candidate
                // span is somehow degenerate.
                let anchor = if !ctx.candidate_span.is_empty() {
                    ctx.candidate_span
                } else {
                    attrs
                        .token_spans
                        .first()
                        .map_or(ctx.candidate_span, |t| t.span)
                };
                return vec![Diagnostic::new(
                    self.id(),
                    self.default_severity(),
                    anchor,
                    message.clone(),
                    citation,
                    None,
                )];
            }
            (Some(first), Some(_)) => {
                // Multiple REL TO blocks present; the message template
                // is the same NonCanonicalOrder class, but the
                // recoverability differs (single-pass fix is unsafe).
                return vec![Diagnostic::new(
                    self.id(),
                    self.default_severity(),
                    first.span,
                    message.clone(),
                    citation,
                    None,
                )];
            }
        };

        // Collect RelToTrigraph spans that fall inside the single
        // RelToBlock. Filtering on block containment is defensive
        // against future parser changes that might surface trigraph
        // tokens outside their block.
        let rel_to_spans: Vec<&TokenSpan> = attrs
            .token_spans
            .iter()
            .filter(|t| {
                t.kind == TokenKind::RelToTrigraph
                    && t.span.start >= block.span.start
                    && t.span.end <= block.span.end
            })
            .collect();
        let (first, last) = match (rel_to_spans.first(), rel_to_spans.last()) {
            (Some(f), Some(l)) => (f, l),
            _ => {
                return vec![Diagnostic::new(
                    self.id(),
                    self.default_severity(),
                    block.span,
                    message.clone(),
                    citation,
                    None,
                )];
            }
        };

        // Span: first→last `RelToTrigraph` within this block, extended
        // through any trailing `,`/whitespace tail *only when* the
        // remainder of the RelToBlock after the last trigraph is
        // delimiter-only. This consumes stale delimiters like the
        // trailing `,` in `REL TO GBR, AUS,` so the splice leaves a
        // clean list. We gate on delimiter-only to preserve any
        // content we can't recognize (tokens outside the CVE
        // TRIGRAPHS list — `is_trigraph` returns false, so the parser
        // never emits a `RelToTrigraph` span for them; deleting them
        // would be wrong).
        let start = first.span.start;
        let mut end = last.span.end;
        let tail_offset = end - block.span.start;
        let block_bytes = block.text.as_bytes();
        if tail_offset <= block_bytes.len() {
            let tail = &block_bytes[tail_offset..];
            if tail.iter().all(|b| matches!(b, b',' | b' ' | b'\t')) {
                end = block.span.end;
            }
        }
        let span = Span::new(start, end);

        // Build the fully canonical list (USA first, non-USA entries
        // alphabetical per CAPCO-2016 §H.8 p151, no duplicates) via
        // [`canonicalize_trigraph_list`]. When USA is missing from
        // input we add it before canonicalizing so the output always
        // has USA first; the helper itself treats USA as "first if
        // present" without injecting it (the helper's contract is
        // "rearrange, don't synthesize"). Producing the canonical form
        // in a single pass is required because the renderer's REL TO
        // axis only canonicalizes when USA is already first — the
        // §H.8 p151 ordering invariant moved into `render_rel_to.rs`
        // when E020 / E060 retired. Dedup before canonicalize so
        // E002's fix output stays canonical when input also has
        // duplicates — under the C-1 overlap guard E002's narrow span
        // would not deduplicate other rules' edits, so we deduplicate
        // PR 3c.B Commit 10: structural FixIntent only. The engine's
        // synthesis path (`synthesize_fixes`) re-renders the canonical
        // bytes from the per-page projection at promotion time via
        // `apply_intent` + `render_canonical`. The rule emits the
        // structural intent only; no byte-precise replacement
        // computation lives on this path post-cutover (G13).
        //
        //   - USA missing → `FactAdd { USA, Scope::Portion }`
        //     (USA injection is a fact-set addition mandated by §H.8 p151).
        //   - USA not first → `Recanonicalize { Portion }` (the sort
        //     is renderer territory; `render_canonical` absorbs
        //     USA-first alpha by construction).
        let intent_scope_recanon = match ctx.marking_type {
            marque_ism::MarkingType::Portion => RecanonScope::Portion,
            _ => RecanonScope::Page,
        };
        let intent_scope_factadd = match ctx.marking_type {
            marque_ism::MarkingType::Portion => Scope::Portion,
            _ => Scope::Page,
        };
        let fix_intent = if !has_usa {
            FixIntent {
                replacement: ReplacementIntent::FactAdd {
                    token: FactRef::Cve(crate::scheme::TOK_USA),
                    scope: intent_scope_factadd,
                },
                confidence: Confidence::strict(0.97),
                feature_ids: Default::default(),
                message: Message::new(MessageTemplate::RequiredByPresence, MessageArgs::default()),
                source: FixSource::BuiltinRule,
                migration_ref: None,
            }
        } else {
            FixIntent {
                replacement: ReplacementIntent::Recanonicalize {
                    scope: intent_scope_recanon,
                },
                confidence: Confidence::strict(0.97),
                feature_ids: Default::default(),
                message: Message::new(MessageTemplate::NonCanonicalOrder, MessageArgs::default()),
                source: FixSource::BuiltinRule,
                migration_ref: None,
            }
        };
        vec![Diagnostic::with_fix_at_span(
            self.id(),
            self.default_severity(),
            span,
            ctx.candidate_span,
            message,
            citation,
            fix_intent,
        )]
    }
}
// ---------------------------------------------------------------------------
// Rule: S009 — prefer-tetragraph-collapse.
//
// Issue #250. Authority: CAPCO-2016 §H.8 p150 (canonical REL TO form;
// worked examples consistently use the compact tetragraph form when all
// members are present). ISMCAT Tetragraph Taxonomy
// V[`marque_ism::ISMCAT_TETRA_VERSION`] (membership tables).
//
// Default severity is `Off` — tetragraph vs. explicit-member form is a
// classification-authority / org style choice; neither form violates
// CAPCO-2016 §H.8. Users opt in via `[rules] S009 = "suggest"`.
// ---------------------------------------------------------------------------

/// Confidence scalar for S009 (`prefer-tetragraph-collapse`).
///
/// Mirrors `BARE_NATO_REQUIRES_REL_TO_CONFIDENCE = 0.85` — sufficient for the
/// suggestion channel. The collapse is purely additive (no
/// information loss: tetragraphs are decomposable), so 0.85 is
/// conservative; users who set `[rules] S009 = "fix"` will need
/// `confidence_threshold ≤ 0.84` to auto-apply.
const PREFER_TETRAGRAPH_COLLAPSE_CONFIDENCE: f32 = 0.85;

/// Rule **S009** — `prefer-tetragraph-collapse`.
///
/// When a REL TO list enumerates all individual members of a known
/// decomposable tetragraph (e.g., `AUS, CAN, GBR, NZL` for `FVEY`),
/// suggests replacing the explicit list with the compact tetragraph form.
///
/// Example: `REL TO USA, AUS, CAN, GBR, NZL` → `REL TO USA, FVEY`.
///
/// **Default severity: `Off`** — tetragraph vs. explicit-member form is
/// an org/classification-authority style choice, not a CAPCO mandate.
/// Enable via `[rules] S009 = "suggest"` (or `"warn"`) in `.marque.toml`.
///
/// **Algorithm**: Greedy set cover over decomposable tetragraphs —
/// tetragraphs with a non-empty member slice in the ISMCAT taxonomy
/// (opaque tetragraphs with no published membership, such as `EU`, are
/// skipped). Candidates are sorted by member-count descending, then alpha,
/// so larger groups (FVEY 5 members) are preferred over overlapping
/// sub-groups (ACGU 4 members). USA is **never** absorbed — §H.8 p150
/// worked examples always emit `USA` explicitly even when `FVEY` is the
/// tetragraph.
///
/// A no-op gate suppresses emission when every selected tetragraph is
/// already present in the REL TO list (input is already compact).
///
/// Authority: CAPCO-2016 §H.8 p150 (canonical REL TO form; USA-first,
/// trigraphs alpha, tetragraphs alpha — worked examples use compact
/// tetragraph form throughout).
pub(super) struct PreferTetragraphCollapseRule;

impl Rule<CapcoScheme> for PreferTetragraphCollapseRule {
    fn id(&self) -> RuleId {
        RuleId::new("capco", "page.dissem.prefer-tetragraph-collapse")
    }
    fn name(&self) -> &'static str {
        "prefer-tetragraph-collapse"
    }
    /// `Severity::Off` — disabled by default.
    fn default_severity(&self) -> Severity {
        Severity::Off
    }
    fn phase(&self) -> Phase {
        Phase::WholeMarking
    }
    fn trusted(&self) -> bool {
        true
    }
    fn check(&self, attrs: &CanonicalAttrs, _ctx: &RuleContext) -> Vec<Diagnostic<CapcoScheme>> {
        // Gate 1: nothing to collapse with an empty REL TO.
        if attrs.rel_to.is_empty() {
            return vec![];
        }

        // O(1)-lookup set for membership tests.
        let rel_to_codes: HashSet<&str> = attrs.rel_to.iter().map(|c| c.as_str()).collect();

        // Find decomposable tetragraphs (non-empty member slice per ISMCAT)
        // whose full member set is covered by the current REL TO list.
        // Opaque tetragraphs (empty member slice — e.g. EU) are skipped;
        // S005 handles opaque-tetragraph uncertainty separately.
        let mut candidates: Vec<(&str, &'static [&'static str])> = TETRAGRAPH_MEMBERS
            .iter()
            .filter_map(|(code, members)| {
                if members.is_empty() {
                    return None;
                }
                if members.iter().all(|m| rel_to_codes.contains(*m)) {
                    Some((*code, *members))
                } else {
                    None
                }
            })
            .collect();

        if candidates.is_empty() {
            return vec![];
        }

        // Greedy set cover: largest-member-set first, alpha tie-break.
        // Larger groups preferred (FVEY 5-member over ACGU 4-member).
        candidates.sort_unstable_by(|a, b| b.1.len().cmp(&a.1.len()).then_with(|| a.0.cmp(b.0)));

        // `collapsed` tracks non-USA trigraphs absorbed into a selected
        // tetragraph. USA is intentionally excluded — §H.8 p150 worked
        // examples always emit USA explicitly even when FVEY is selected.
        let mut collapsed: HashSet<&str> = HashSet::new();
        let mut selected: Vec<&str> = Vec::new();
        for (code, members) in &candidates {
            let overlaps = members
                .iter()
                .any(|m| *m != "USA" && collapsed.contains(*m));
            if overlaps {
                continue;
            }
            selected.push(code);
            for m in *members {
                if *m != "USA" {
                    collapsed.insert(m);
                }
            }
        }

        if selected.is_empty() {
            return vec![];
        }

        // No-op gate: every selected tetragraph already in the REL TO list
        // means the input is already in compact form — nothing to suggest.
        if selected.iter().all(|code| rel_to_codes.contains(*code)) {
            return vec![];
        }

        // Build the replacement using the canonical §H.8 p150 / §A.6 p16
        // sort: USA first, remaining trigraphs ascending alpha, tetragraphs
        // ascending alpha.
        let has_usa = rel_to_codes.contains("USA");
        let mut remaining_trigraphs: Vec<&str> = rel_to_codes
            .iter()
            .copied()
            .filter(|&c| c != "USA" && c.len() == 3 && !collapsed.contains(c))
            .collect();
        remaining_trigraphs.sort_unstable();
        let mut tetragraph_bucket: Vec<&str> = rel_to_codes
            .iter()
            .copied()
            .filter(|&c| c.len() != 3 && !collapsed.contains(c))
            .collect();
        tetragraph_bucket.extend_from_slice(&selected);
        tetragraph_bucket.sort_unstable();
        tetragraph_bucket.dedup();

        let mut replacement =
            String::with_capacity(7 + 5 * (remaining_trigraphs.len() + tetragraph_bucket.len()));
        replacement.push_str("REL TO");
        let mut first_code = true;
        let emit_code = |code: &str, out: &mut String, first: &mut bool| {
            if *first {
                out.push(' ');
                *first = false;
            } else {
                out.push_str(", ");
            }
            out.push_str(code);
        };
        if has_usa {
            emit_code("USA", &mut replacement, &mut first_code);
        }
        for code in &remaining_trigraphs {
            emit_code(code, &mut replacement, &mut first_code);
        }
        for code in &tetragraph_bucket {
            emit_code(code, &mut replacement, &mut first_code);
        }

        // Single RelToBlock span — same splice pattern as S007.
        let mut blocks = attrs
            .token_spans
            .iter()
            .filter(|t| t.kind == TokenKind::RelToBlock);
        let block = match (blocks.next(), blocks.next()) {
            (Some(b), None) => b,
            // Multiple RelToBlock tokens is a malformed shape; E002 /
            // parser-shape diagnostics own that case. Skip to avoid
            // cross-block splice damage.
            (Some(_), Some(_)) | (None, _) => return vec![],
        };

        if block.text.as_str() == replacement {
            return vec![];
        }

        vec![Diagnostic::text_correction(
            self.id(),
            Severity::Suggest,
            block.span,
            Message::new(
                MessageTemplate::NonCanonicalForm,
                MessageArgs {
                    category: Some(crate::scheme::CAT_REL_TO),
                    ..Default::default()
                },
            ),
            capco(SectionLetter::H, 8, 150),
            replacement,
            FixSource::BuiltinRule,
            Confidence::strict(PREFER_TETRAGRAPH_COLLAPSE_CONFIDENCE),
            None,
        )]
    }
}
// ---------------------------------------------------------------------------
// Rule: S010 — collapse-uniform-rel-portions
// ---------------------------------------------------------------------------
//
// Phase: PageFinalization. Off by default.
//
// CAPCO-2016 §H.8 p150: "Authorized Portion Mark (when the portion's
// country trigraphs and/or tetragraph list is the SAME as the banner line
// REL TO marking): REL". When ALL portions with an explicit REL TO list
// carry the same list as the projected banner REL TO, the compact `REL`
// form is equally valid. S010 suggests this compaction. Gate: only fires
// when EVERY explicit-REL-TO portion matches, so the suggested
// transformation replaces all of them uniformly.

/// Confidence scalar for S010.
const COLLAPSE_UNIFORM_REL_PORTIONS_CONFIDENCE: f32 = 0.85;
const COLLAPSE_UNIFORM_REL_PORTIONS_CITATION: Citation = capco(SectionLetter::H, 8, 150);

/// Rule **S010** — `collapse-uniform-rel-portions`.
///
/// Off by default. Enable via `[rules] S010 = "suggest"`.
/// Authority: CAPCO-2016 §H.8 p150.
pub(super) struct CollapseUniformRelPortionsRule;

impl Rule<CapcoScheme> for CollapseUniformRelPortionsRule {
    fn id(&self) -> RuleId {
        RuleId::new("capco", "page.dissem.collapse-uniform-rel-portions")
    }
    fn name(&self) -> &'static str {
        "collapse-uniform-rel-portions"
    }
    fn default_severity(&self) -> Severity {
        Severity::Off
    }
    fn phase(&self) -> Phase {
        Phase::PageFinalization
    }
    fn trusted(&self) -> bool {
        true
    }
    fn check(&self, attrs: &CanonicalAttrs, ctx: &RuleContext) -> Vec<Diagnostic<CapcoScheme>> {
        check_collapse_uniform_rel_portions(attrs, ctx)
    }
}

/// Expand a REL TO list into an atomic set of trigraphs.
///
/// Tetragraph codes in `rel_to` (e.g. `FVEY`, `NATO`) are replaced by their
/// member trigraphs. Opaque tetragraphs with no published membership (e.g.
/// `EU`) pass through unchanged. This normalizes both the banner-projected
/// set and per-portion sets to a common representation before comparison, so
/// `(S//REL TO USA, FVEY)` and `(S//REL TO USA, AUS, CAN, GBR, NZL)` are
/// treated as equivalent.
fn expand_rel_to_atomic(
    codes: &[marque_ism::CountryCode],
) -> std::collections::BTreeSet<marque_ism::CountryCode> {
    let mut out = std::collections::BTreeSet::new();
    for code in codes {
        if let Some(members) = crate::vocab::expand_tetragraph(code.as_str()) {
            for m in members {
                if let Some(cc) = marque_ism::CountryCode::try_new(m.as_bytes()) {
                    out.insert(cc);
                }
            }
        } else {
            out.insert(*code);
        }
    }
    out
}

fn check_collapse_uniform_rel_portions(
    _attrs: &CanonicalAttrs,
    ctx: &RuleContext,
) -> Vec<Diagnostic<CapcoScheme>> {
    let Some(page_portions) = ctx.page_portions.as_ref() else {
        return Vec::new();
    };
    let portions: &[CanonicalAttrs] = page_portions.as_ref();
    let Some(page_mark) = ctx.page_marking.as_ref() else {
        return Vec::new();
    };
    // No banner REL TO list projected — nothing to match against.
    if page_mark.rel_to.is_empty() {
        return Vec::new();
    }
    // NOFORN guard: REL TO superseded by NOFORN (mirrors S005).
    let any_noforn = portions.iter().any(|p| {
        p.dissem_iter()
            .any(|d| matches!(d, marque_ism::DissemControl::Nf))
    });
    if any_noforn {
        return Vec::new();
    }
    let banner_set = expand_rel_to_atomic(&page_mark.rel_to);
    let explicit_portions: Vec<&CanonicalAttrs> =
        portions.iter().filter(|p| !p.rel_to.is_empty()).collect();
    if explicit_portions.is_empty() {
        return Vec::new();
    }
    // Gate: EVERY explicit-REL-TO portion must match the banner list.
    let all_match = explicit_portions.iter().all(|p| {
        let portion_set = expand_rel_to_atomic(&p.rel_to);
        portion_set == banner_set
    });
    if !all_match {
        return Vec::new();
    }
    let mut diagnostics = Vec::new();
    for portion in explicit_portions {
        let Some(block) = portion
            .token_spans
            .iter()
            .find(|t| t.kind == TokenKind::RelToBlock)
        else {
            continue;
        };
        diagnostics.push(Diagnostic::text_correction(
            RuleId::new("capco", "page.dissem.collapse-uniform-rel-portions"),
            Severity::Suggest,
            block.span,
            Message::new(
                MessageTemplate::NonCanonicalForm,
                MessageArgs {
                    category: Some(crate::scheme::CAT_REL_TO),
                    ..Default::default()
                },
            ),
            COLLAPSE_UNIFORM_REL_PORTIONS_CITATION,
            "REL",
            FixSource::BuiltinRule,
            Confidence::strict(COLLAPSE_UNIFORM_REL_PORTIONS_CONFIDENCE),
            None,
        ));
    }
    diagnostics
}
// ---------------------------------------------------------------------------
// Rule: E072 — bare-rel-portion-divergence
// ---------------------------------------------------------------------------
//
// Phase: PageFinalization. Warn by default.
//
// CAPCO-2016 §H.8 p150-151: bare `REL` in a portion means "my releasability
// = the banner's REL TO list." When bare-REL portions and explicit-REL-TO
// portions with a different list coexist on the same page, extraction is
// ambiguous: bare-REL portions implicitly carry the banner list while the
// divergent explicit portions carry a different list. E072 warns on each
// divergent explicit portion.

const BARE_REL_PORTION_DIVERGENCE_CITATION: Citation = capco(SectionLetter::H, 8, 151);

/// Rule **E072** — `bare-rel-portion-divergence`.
///
/// Default severity: [`Severity::Warn`]. Authority: CAPCO-2016 §H.8 p150-151.
pub(super) struct BareRelPortionDivergenceRule;

impl Rule<CapcoScheme> for BareRelPortionDivergenceRule {
    fn id(&self) -> RuleId {
        RuleId::new("capco", "page.dissem.bare-rel-portion-divergence")
    }
    fn name(&self) -> &'static str {
        "bare-rel-portion-divergence"
    }
    fn default_severity(&self) -> Severity {
        Severity::Warn
    }
    fn phase(&self) -> Phase {
        Phase::PageFinalization
    }
    fn trusted(&self) -> bool {
        true
    }
    fn check(&self, attrs: &CanonicalAttrs, ctx: &RuleContext) -> Vec<Diagnostic<CapcoScheme>> {
        check_bare_rel_portion_divergence(attrs, ctx)
    }
}

fn check_bare_rel_portion_divergence(
    _attrs: &CanonicalAttrs,
    ctx: &RuleContext,
) -> Vec<Diagnostic<CapcoScheme>> {
    let Some(page_portions) = ctx.page_portions.as_ref() else {
        return Vec::new();
    };
    let portions: &[CanonicalAttrs] = page_portions.as_ref();
    let Some(page_mark) = ctx.page_marking.as_ref() else {
        return Vec::new();
    };
    // NOFORN guard: REL TO superseded by NOFORN (mirrors S005).
    let any_noforn = portions.iter().any(|p| {
        p.dissem_iter()
            .any(|d| matches!(d, marque_ism::DissemControl::Nf))
    });
    if any_noforn {
        return Vec::new();
    }
    // No projected banner REL TO — nothing to compare against.
    if page_mark.rel_to.is_empty() {
        return Vec::new();
    }
    // E072 only applies when at least one bare-REL portion exists.
    let has_bare_rel = portions.iter().any(|p| {
        p.rel_to.is_empty()
            && p.dissem_iter()
                .any(|d| matches!(d, marque_ism::DissemControl::Rel))
    });
    if !has_bare_rel {
        return Vec::new();
    }
    let banner_set = expand_rel_to_atomic(&page_mark.rel_to);
    let mut diagnostics = Vec::new();
    for portion in portions {
        if portion.rel_to.is_empty() {
            continue;
        }
        let portion_set = expand_rel_to_atomic(&portion.rel_to);
        if portion_set == banner_set {
            continue;
        }
        // Portion's explicit list diverges from what bare-REL portions imply.
        //
        // Parser invariant (verified against `parse_rel_to_with_spans`, the
        // sole producer of `rel_to` entries): every push into `rel_to` is
        // immediately preceded by a `TokenKind::RelToBlock` `TokenSpan` push
        // at the two call sites in `marque-core::parser`. The
        // `portion.rel_to.is_empty()` guard above means we reach this site
        // only when `rel_to` is non-empty, therefore the `find()` MUST
        // succeed. The `else` arm is defense-in-depth against future parser
        // changes that would violate the invariant; uses the same let-else
        // shape as S010, with a `debug_assert!` on the invariant itself
        // (not on a constant) so dev/test builds panic loud if the parser
        // ever drops the RelToBlock span while keeping `rel_to` populated.
        debug_assert!(
            portion
                .token_spans
                .iter()
                .any(|t| t.kind == TokenKind::RelToBlock),
            "E072: portion with non-empty rel_to has no RelToBlock token span \
             (parser invariant violation; see parse_rel_to_with_spans call sites \
             in marque-core::parser)"
        );
        let Some(block) = portion
            .token_spans
            .iter()
            .find(|t| t.kind == TokenKind::RelToBlock)
        else {
            continue;
        };
        let span = block.span;
        diagnostics.push(Diagnostic::new(
            RuleId::new("capco", "page.dissem.bare-rel-portion-divergence"),
            Severity::Warn,
            span,
            Message::new(
                MessageTemplate::BannerRollupMismatch,
                MessageArgs {
                    category: Some(crate::scheme::CAT_REL_TO),
                    ..Default::default()
                },
            ),
            BARE_REL_PORTION_DIVERGENCE_CITATION,
            None,
        ));
    }
    diagnostics
}
