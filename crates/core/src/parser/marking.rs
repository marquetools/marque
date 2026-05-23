use super::*;

impl<'t> Parser<'t> {
    pub(super) fn parse_marking_string<'src>(
        &self,
        s: &'src str,
        context: MarkingType,
        s_offset: usize,
        origin: SourceOrigin,
    ) -> Result<ParsedAttrs<'src>, CoreError> {
        if s.is_empty() {
            return Err(CoreError::MalformedMarking(s.to_owned()));
        }

        let mut classification: Option<ParsedClassification<'src>> = None;
        let mut fgi_marker: Option<ParsedFgiMarker<'src>> = None;
        let mut declassify_on: Option<ParsedDeclassifyOn<'src>> = None;
        let mut declass_exemption: Option<DeclassExemption> = None;
        let mut sar_markings: Option<ParsedSarMarking<'src>> = None;

        let separators: SmallVec<[usize; 4]> = s.match_indices("//").map(|(i, _)| i).collect();
        let mut block_ranges: SmallVec<[(usize, usize); 4]> =
            SmallVec::with_capacity(separators.len() + 1);
        let mut prev_end = 0usize;
        for &sep_start in &separators {
            block_ranges.push((prev_end, sep_start));
            prev_end = sep_start + 2; // skip the `//`
        }
        block_ranges.push((prev_end, s.len()));

        let mut token_spans: SmallVec<[TokenSpan; 16]> = SmallVec::new();

        let mut sci: SmallVec<[SciControl; 4]> = SmallVec::new();
        let mut sci_markings: SmallVec<[ParsedSciMarking<'src>; 2]> = SmallVec::new();
        let mut sar_captured = false;
        let mut aea: SmallVec<[ParsedAea<'src>; 2]> = SmallVec::new();
        let mut dissem: SmallVec<[ParsedDissem<'src>; 4]> = SmallVec::new();
        let mut non_ic: SmallVec<[ParsedNonIcDissem<'src>; 2]> = SmallVec::new();
        let mut rel_to: SmallVec<[ParsedRelToEntry<'src>; 8]> = SmallVec::new();
        let mut display_only_to: SmallVec<[ParsedDisplayOnlyEntry<'src>; 4]> = SmallVec::new();

        let is_non_us = s.starts_with("//");

        for (idx, &(rel_start, rel_end)) in block_ranges.iter().enumerate() {
            let raw = &s[rel_start..rel_end];
            let trimmed = raw.trim();
            if trimmed.is_empty() {
                continue;
            }
            let trim_lead = raw.len() - raw.trim_start().len();
            let abs_start = s_offset + rel_start + trim_lead;
            let abs_end = abs_start + trimmed.len();
            let span = Span::new(abs_start, abs_end);

            if idx == 0 && !is_non_us {
                if let Some(c) = parse_classification(trimmed) {
                    classification = Some(ParsedClassification::new(
                        MarkingClassification::Us(c),
                        trimmed,
                        span,
                    ));
                }
                token_spans.push(TokenSpan {
                    kind: TokenKind::Classification,
                    span,
                    text: trimmed.into(),
                });
                continue;
            }

            if idx == 1 && is_non_us {
                let parsed_cls = if let Some(nato_block) = parse_nato_classification(trimmed) {
                    let NatoBlock { class, companion } = nato_block;
                    match companion {
                        NatoCompanion::Bare => {}
                        NatoCompanion::Aea(aea_marking) => {
                            aea.push(ParsedAea::new(aea_marking, trimmed, span));
                        }
                        NatoCompanion::Sci(nato_sap) => {
                            let sci_marking = SciMarking::new(
                                SciControlSystem::NatoSap(nato_sap),
                                Box::new([]),
                                None,
                            );
                            sci_markings.push(ParsedSciMarking::new(sci_marking, trimmed, span));
                        }
                    }
                    Some(MarkingClassification::Nato(class))
                } else if let Some(joint) = parse_joint_classification(trimmed) {
                    Some(MarkingClassification::Joint(joint))
                } else {
                    parse_fgi_classification(trimmed).map(MarkingClassification::Fgi)
                };
                if let Some(value) = parsed_cls {
                    classification = Some(ParsedClassification::new(value, trimmed, span));
                } else {
                    token_spans.push(TokenSpan {
                        kind: TokenKind::Unknown,
                        span,
                        text: trimmed.into(),
                    });
                    continue;
                }
                token_spans.push(TokenSpan {
                    kind: TokenKind::Classification,
                    span,
                    text: trimmed.into(),
                });
                continue;
            }

            if trimmed.starts_with("SAR-") || trimmed.starts_with("SPECIAL ACCESS REQUIRED-") {
                if sar_captured {
                    token_spans.push(TokenSpan {
                        kind: TokenKind::Unknown,
                        span,
                        text: trimmed.into(),
                    });
                    continue;
                }
                if let Some((marking, sar_spans)) = parse_sar_category(trimmed, abs_start) {
                    sar_markings = Some(ParsedSarMarking::new(marking, trimmed, span));
                    token_spans.extend(sar_spans);
                    sar_captured = true;
                    continue;
                }
                token_spans.push(TokenSpan {
                    kind: TokenKind::Unknown,
                    span,
                    text: trimmed.into(),
                });
                continue;
            }

            if trimmed.starts_with("REL TO") || trimmed.starts_with("REL ") {
                token_spans.push(TokenSpan {
                    kind: TokenKind::RelToBlock,
                    span,
                    text: trimmed.into(),
                });
                let parsed =
                    parse_rel_to_with_spans(trimmed, abs_start, self.tokens, &mut token_spans);
                rel_to.extend(parsed.countries);
                dissem.extend(parsed.trailing_dissem);
                non_ic.extend(parsed.trailing_non_ic);
                display_only_to.extend(parsed.trailing_display_only);
            } else if trimmed.starts_with("DISPLAY ONLY ") || trimmed == "DISPLAY ONLY" {
                token_spans.push(TokenSpan {
                    kind: TokenKind::DisplayOnlyBlock,
                    span,
                    text: trimmed.into(),
                });
                let parsed = parse_display_only_with_spans(
                    trimmed,
                    abs_start,
                    self.tokens,
                    &mut token_spans,
                );
                display_only_to.extend(parsed.countries);
                dissem.extend(parsed.trailing_dissem);
                non_ic.extend(parsed.trailing_non_ic);
            } else if let Some(long_form) = recognize_deprecated_sci_long_form(trimmed) {
                let compartments: Box<[SciCompartment]> = match &long_form.compartment {
                    Some(comp) => Box::new([SciCompartment::new(comp.as_str(), Box::new([]))]),
                    None => Box::new([]),
                };
                let canonical_enum = if compartments.is_empty() {
                    SciControl::parse(long_form.system.as_str())
                } else {
                    compartments.first().and_then(|c| {
                        let composite = format!("{}-{}", long_form.system.as_str(), c.identifier);
                        SciControl::parse(&composite)
                    })
                };
                let marking = SciMarking::new(
                    SciControlSystem::Published(long_form.system),
                    compartments,
                    canonical_enum,
                );
                if let Some(ctrl) = marking.canonical_enum {
                    sci.push(ctrl);
                }
                sci_markings.push(ParsedSciMarking::new(marking, trimmed, span));
                token_spans.push(TokenSpan {
                    kind: TokenKind::SciControl,
                    span,
                    text: trimmed.into(),
                });
                token_spans.push(TokenSpan {
                    kind: TokenKind::SciSystem,
                    span,
                    text: trimmed.into(),
                });
            } else if recognize_eyes_only_block(trimmed, self.tokens).is_some() {
                dissem.push(ParsedDissem::new(DissemControl::Eyes, trimmed, span));
                token_spans.push(TokenSpan {
                    kind: TokenKind::DissemControl,
                    span,
                    text: trimmed.into(),
                });
            } else if (trimmed.contains('-')
                || trimmed.contains('/')
                || is_bare_cve_value(trimmed)
                || (is_valid_custom_control(trimmed)
                    && trimmed.bytes().any(|b| b.is_ascii_digit())
                    && !is_known_non_sci_token(trimmed)
                    && !is_declass_date(trimmed))
                || recognize_nato_sap(trimmed).is_some())
                && let Some(markings) = parse_sci_block(trimmed, abs_start, &mut token_spans)
            {
                for marking in &markings {
                    if let Some(ctrl) = marking.canonical_enum {
                        sci.push(ctrl);
                    }
                }
                for marking in markings {
                    sci_markings.push(ParsedSciMarking::new(marking, trimmed, span));
                }
            } else if let Some(ctrl) =
                SciControl::parse(trimmed).or_else(|| parse_sci_control_full_form(trimmed))
            {
                sci.push(ctrl);
                token_spans.push(TokenSpan {
                    kind: TokenKind::SciControl,
                    span,
                    text: trimmed.into(),
                });
            } else if starts_with_fgi_prefix(trimmed)
                && matches!(
                    classification.as_ref().map(|c| &c.value),
                    Some(MarkingClassification::Us(_))
                )
            {
                if let Some(marker) =
                    parse_fgi_marker_with_spans(trimmed, abs_start, &mut token_spans)
                {
                    fgi_marker = Some(ParsedFgiMarker::new(marker, trimmed, span));
                    token_spans.push(TokenSpan {
                        kind: TokenKind::FgiMarker,
                        span,
                        text: trimmed.into(),
                    });
                } else {
                    token_spans.push(TokenSpan {
                        kind: TokenKind::Unknown,
                        span,
                        text: trimmed.into(),
                    });
                }
            } else if let Some(ctrl) =
                DissemControl::parse(trimmed).or_else(|| parse_dissem_full_form(trimmed))
            {
                dissem.push(ParsedDissem::new(ctrl, trimmed, span));
                token_spans.push(TokenSpan {
                    kind: TokenKind::DissemControl,
                    span,
                    text: trimmed.into(),
                });
            } else if let Some(nic) = parse_non_ic_full_form(trimmed) {
                non_ic.push(ParsedNonIcDissem::new(nic, trimmed, span));
                token_spans.push(TokenSpan {
                    kind: TokenKind::NonIcDissem,
                    span,
                    text: trimmed.into(),
                });
            } else if let Some(aea_marking) = parse_aea_full_form(trimmed) {
                aea.push(ParsedAea::new(aea_marking, trimmed, span));
                token_spans.push(TokenSpan {
                    kind: TokenKind::AeaMarking,
                    span,
                    text: trimmed.into(),
                });
            } else if let Some(exemption) = DeclassExemption::parse(trimmed) {
                declass_exemption = Some(exemption);
                token_spans.push(TokenSpan {
                    kind: TokenKind::DeclassExemption,
                    span,
                    text: trimmed.into(),
                });
            } else if is_declass_date(trimmed) {
                if let Ok(date) = IsmDate::from_str(trimmed) {
                    declassify_on = Some(ParsedDeclassifyOn::new(date, trimmed, span));
                }
                token_spans.push(TokenSpan {
                    kind: TokenKind::DeclassDate,
                    span,
                    text: trimmed.into(),
                });
            } else if let Some(foreign) = try_parse_foreign_classification(trimmed) {
                let prior_us = match classification.as_ref().map(|c| &c.value) {
                    Some(MarkingClassification::Us(level)) => Some(*level),
                    _ => None,
                };
                if let Some(us_level) = prior_us {
                    let foreign_equiv = match &foreign {
                        ForeignClassification::Nato(n) => n.us_equivalent(),
                        ForeignClassification::Fgi(f) => f.level,
                        ForeignClassification::Joint(j) => j.level,
                    };
                    let max_level = us_level.max(foreign_equiv);
                    classification = Some(ParsedClassification::new(
                        MarkingClassification::Conflict {
                            us: max_level,
                            foreign: Box::new(foreign),
                        },
                        trimmed,
                        span,
                    ));
                    token_spans.push(TokenSpan {
                        kind: TokenKind::Classification,
                        span,
                        text: trimmed.into(),
                    });
                } else {
                    token_spans.push(TokenSpan {
                        kind: TokenKind::Unknown,
                        span,
                        text: trimmed.into(),
                    });
                }
            } else if trimmed.contains('/') && !trimmed.starts_with("REL") {
                #[derive(Clone, Copy, PartialEq, Eq)]
                enum SubKind {
                    Sci,
                    Dissem,
                    RelTo,
                    DisplayOnly,
                    NonIc,
                    Aea,
                    Unknown,
                }

                fn category_family(k: SubKind) -> SubKind {
                    match k {
                        SubKind::RelTo | SubKind::DisplayOnly => SubKind::Dissem,
                        other => other,
                    }
                }

                struct SubResult<'a> {
                    kind: SubKind,
                    tok: &'a str,
                    span: Span,
                    sci: Option<SciControl>,
                    dissem: Option<DissemControl>,
                    nic: Option<NonIcDissem>,
                    aea: Option<AeaMarking>,
                }

                let (token_offsets, slash_offsets) = split_slash_with_separator_offsets(trimmed);
                let mut results: SmallVec<[SubResult<'_>; 4]> = SmallVec::new();
                for (sub_off, sub_tok) in token_offsets {
                    let sub_abs_start = abs_start + sub_off;
                    let sub_span = Span::new(sub_abs_start, sub_abs_start + sub_tok.len());
                    if sub_tok.starts_with("REL TO ") || sub_tok == "REL TO" {
                        results.push(SubResult {
                            kind: SubKind::RelTo,
                            tok: sub_tok,
                            span: sub_span,
                            sci: None,
                            dissem: None,
                            nic: None,
                            aea: None,
                        });
                    } else if sub_tok.starts_with("DISPLAY ONLY ") || sub_tok == "DISPLAY ONLY" {
                        results.push(SubResult {
                            kind: SubKind::DisplayOnly,
                            tok: sub_tok,
                            span: sub_span,
                            sci: None,
                            dissem: None,
                            nic: None,
                            aea: None,
                        });
                    } else if let Some(ctrl) =
                        SciControl::parse(sub_tok).or_else(|| parse_sci_control_full_form(sub_tok))
                    {
                        results.push(SubResult {
                            kind: SubKind::Sci,
                            tok: sub_tok,
                            span: sub_span,
                            sci: Some(ctrl),
                            dissem: None,
                            nic: None,
                            aea: None,
                        });
                    } else if let Some(ctrl) =
                        DissemControl::parse(sub_tok).or_else(|| parse_dissem_full_form(sub_tok))
                    {
                        results.push(SubResult {
                            kind: SubKind::Dissem,
                            tok: sub_tok,
                            span: sub_span,
                            sci: None,
                            dissem: Some(ctrl),
                            nic: None,
                            aea: None,
                        });
                    } else if let Some(nic) = parse_non_ic_full_form(sub_tok) {
                        results.push(SubResult {
                            kind: SubKind::NonIc,
                            tok: sub_tok,
                            span: sub_span,
                            sci: None,
                            dissem: None,
                            nic: Some(nic),
                            aea: None,
                        });
                    } else if let Some(aea_marking) = parse_aea_full_form(sub_tok) {
                        results.push(SubResult {
                            kind: SubKind::Aea,
                            tok: sub_tok,
                            span: sub_span,
                            sci: None,
                            dissem: None,
                            nic: None,
                            aea: Some(aea_marking),
                        });
                    } else if context == MarkingType::Banner
                        && let Some(ws_off) = sub_tok.find(char::is_whitespace)
                        && let Some(recovered) = {
                            let word = &sub_tok[..ws_off];
                            let trailing = sub_tok[ws_off..].trim();
                            let trailing_is_all_unrecognized = trailing.is_empty()
                                || trailing.split_whitespace().all(|t| {
                                    SciControl::parse(t).is_none()
                                        && parse_sci_control_full_form(t).is_none()
                                        && DissemControl::parse(t).is_none()
                                        && parse_dissem_full_form(t).is_none()
                                        && parse_non_ic_full_form(t).is_none()
                                        && parse_aea_full_form(t).is_none()
                                });
                            if !trailing_is_all_unrecognized {
                                None
                            } else {
                                let word_span =
                                    Span::new(sub_abs_start, sub_abs_start + word.len());
                                SciControl::parse(word)
                                    .or_else(|| parse_sci_control_full_form(word))
                                    .map(|ctrl| SubResult {
                                        kind: SubKind::Sci,
                                        tok: word,
                                        span: word_span,
                                        sci: Some(ctrl),
                                        dissem: None,
                                        nic: None,
                                        aea: None,
                                    })
                                    .or_else(|| {
                                        DissemControl::parse(word)
                                            .or_else(|| parse_dissem_full_form(word))
                                            .map(|ctrl| SubResult {
                                                kind: SubKind::Dissem,
                                                tok: word,
                                                span: word_span,
                                                sci: None,
                                                dissem: Some(ctrl),
                                                nic: None,
                                                aea: None,
                                            })
                                    })
                                    .or_else(|| {
                                        parse_non_ic_full_form(word).map(|nic| SubResult {
                                            kind: SubKind::NonIc,
                                            tok: word,
                                            span: word_span,
                                            sci: None,
                                            dissem: None,
                                            nic: Some(nic),
                                            aea: None,
                                        })
                                    })
                                    .or_else(|| {
                                        parse_aea_full_form(word).map(|aea_marking| SubResult {
                                            kind: SubKind::Aea,
                                            tok: word,
                                            span: word_span,
                                            sci: None,
                                            dissem: None,
                                            nic: None,
                                            aea: Some(aea_marking),
                                        })
                                    })
                            }
                        }
                    {
                        results.push(recovered);
                    } else {
                        results.push(SubResult {
                            kind: SubKind::Unknown,
                            tok: sub_tok,
                            span: sub_span,
                            sci: None,
                            dissem: None,
                            nic: None,
                            aea: None,
                        });
                    }
                }

                let first_parsed_kind = results
                    .iter()
                    .find(|r| r.kind != SubKind::Unknown)
                    .map(|r| r.kind);
                let all_same_category = first_parsed_kind.is_some_and(|first| {
                    let first_family = category_family(first);
                    results
                        .iter()
                        .filter(|r| r.kind != SubKind::Unknown)
                        .all(|r| category_family(r.kind) == first_family)
                });

                if first_parsed_kind.is_none() || !all_same_category {
                    token_spans.push(TokenSpan {
                        kind: TokenKind::Unknown,
                        span,
                        text: trimmed.into(),
                    });
                } else {
                    let mut committed_idx: usize = 0;
                    for sep in &slash_offsets {
                        let left_idx = if sep.left_nonempty {
                            let idx = committed_idx;
                            committed_idx += 1;
                            Some(idx)
                        } else {
                            None
                        };
                        let right_idx = if sep.right_nonempty {
                            Some(committed_idx)
                        } else {
                            None
                        };
                        let emit = match (left_idx, right_idx) {
                            (Some(li), Some(ri)) => {
                                let left = results.get(li);
                                let right = results.get(ri);
                                matches!(
                                    (left, right),
                                    (Some(l), Some(r))
                                        if l.kind != SubKind::Unknown
                                            && r.kind != SubKind::Unknown
                                            && category_family(l.kind) == category_family(r.kind)
                                )
                            }
                            _ => false,
                        };
                        if emit {
                            let abs_slash_start = abs_start + sep.start;
                            let abs_slash_end = abs_start + sep.end;
                            token_spans.push(TokenSpan {
                                kind: TokenKind::Separator,
                                span: Span::new(abs_slash_start, abs_slash_end),
                                text: "/".into(),
                            });
                        }
                    }
                    for r in results {
                        match r.kind {
                            SubKind::Sci => {
                                sci.push(r.sci.unwrap());
                                token_spans.push(TokenSpan {
                                    kind: TokenKind::SciControl,
                                    span: r.span,
                                    text: r.tok.into(),
                                });
                            }
                            SubKind::Dissem => {
                                dissem.push(ParsedDissem::new(r.dissem.unwrap(), r.tok, r.span));
                                token_spans.push(TokenSpan {
                                    kind: TokenKind::DissemControl,
                                    span: r.span,
                                    text: r.tok.into(),
                                });
                            }
                            SubKind::RelTo => {
                                token_spans.push(TokenSpan {
                                    kind: TokenKind::RelToBlock,
                                    span: r.span,
                                    text: r.tok.into(),
                                });
                                let parsed = parse_rel_to_with_spans(
                                    r.tok,
                                    r.span.start,
                                    self.tokens,
                                    &mut token_spans,
                                );
                                rel_to.extend(parsed.countries);
                                debug_assert!(
                                    parsed.trailing_dissem.is_empty()
                                        && parsed.trailing_non_ic.is_empty()
                                        && parsed.trailing_display_only.is_empty(),
                                    "multi-token RelTo path should never observe trailing \
                                     controls (sub-token splitting peels them first)"
                                );
                            }
                            SubKind::DisplayOnly => {
                                token_spans.push(TokenSpan {
                                    kind: TokenKind::DisplayOnlyBlock,
                                    span: r.span,
                                    text: r.tok.into(),
                                });
                                let parsed = parse_display_only_with_spans(
                                    r.tok,
                                    r.span.start,
                                    self.tokens,
                                    &mut token_spans,
                                );
                                display_only_to.extend(parsed.countries);
                                debug_assert!(
                                    parsed.trailing_dissem.is_empty()
                                        && parsed.trailing_non_ic.is_empty(),
                                    "multi-token DisplayOnly path should never observe trailing \
                                     controls (sub-token splitting peels them first)"
                                );
                            }
                            SubKind::NonIc => {
                                non_ic.push(ParsedNonIcDissem::new(r.nic.unwrap(), r.tok, r.span));
                                token_spans.push(TokenSpan {
                                    kind: TokenKind::NonIcDissem,
                                    span: r.span,
                                    text: r.tok.into(),
                                });
                            }
                            SubKind::Aea => {
                                aea.push(ParsedAea::new(r.aea.unwrap(), r.tok, r.span));
                                token_spans.push(TokenSpan {
                                    kind: TokenKind::AeaMarking,
                                    span: r.span,
                                    text: r.tok.into(),
                                });
                            }
                            SubKind::Unknown => {
                                token_spans.push(TokenSpan {
                                    kind: TokenKind::Unknown,
                                    span: r.span,
                                    text: r.tok.into(),
                                });
                            }
                        }
                    }
                }
            } else {
                token_spans.push(TokenSpan {
                    kind: TokenKind::Unknown,
                    span,
                    text: trimmed.into(),
                });
            }
        }

        for &sep_start in &separators {
            token_spans.push(TokenSpan {
                kind: TokenKind::Separator,
                span: Span::new(s_offset + sep_start, s_offset + sep_start + 2),
                text: "//".into(),
            });
        }
        token_spans.sort_unstable_by_key(|ts| ts.span.start);

        let _ = context; // used for future context-aware validation

        let mut attrs = ParsedAttrs::new(
            classification,
            sci_markings.into_boxed_slice(),
            sci.into_boxed_slice(),
            sar_markings,
            aea.into_boxed_slice(),
            fgi_marker,
            dissem.into_boxed_slice(),
            Box::new([]),
            non_ic.into_boxed_slice(),
            rel_to.into_boxed_slice(),
            display_only_to.into_boxed_slice(),
            declassify_on,
            None,
            None,
            declass_exemption,
            token_spans.into_boxed_slice(),
            origin,
        );
        marque_ism::attribute_dissems(&mut attrs, self.default_origin);
        marque_ism::dedup_companions(&mut attrs);
        Ok(attrs)
    }
}
