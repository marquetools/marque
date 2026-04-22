# CAPCO/ISM Validator Agents

Specialized validator agents for ISM/CAPCO rule and token validation. These agents are the authoritative fact-checkers to prevent rule/token errors from making it into production.

## Quick Start

Invoke any validator directly from the terminal:

```bash
# Category validator: Validate a rule within a marking category
invoke capco-sci-validator <<'EOF'
Rule: check_sci_compartment_ordering
Citation: "SI-G ABCD should pass, SI-ABCD G should fail"
Test case: ...
EOF

# Category validator: Validate a token against ISM data
invoke capco-classification-validator <<'EOF'
Token: TOP SECRET
Check against: CVEnumISMClassificationAll
Expected value: "TOP SECRET"
EOF

# Specialist validator: Validate CAB authority chain
invoke capco-cab-specialist <<'EOF'
Source 1: "Classified by: Secretary of Defense"
Source 2: "Classified by: Agency XYZ"
Question: Whose authority do I cite when deriving from both?
EOF

# Specialist validator: Check historical marking equivalents
invoke capco-deprecated-historical-specialist <<'EOF'
Found: FOUO//REL TO USA (legacy document)
Question: What's the modern CUI equivalent?
EOF
```

## The 13 Validator Agents

### 10 Category Validators

Each agent is a focused expert on one marking category:

| Agent | Focus | CAPCO § | Key CVE Enums |
|-------|-------|---------|---------------|
| **capco-sci-validator** | SCI controls, compartments, sub-compartments, grammar | H.4, A.6 | CVEnumISMSCIControls |
| **capco-sar-validator** | SAR programs, SAP structure, compartments | H.5 | CVEnumISMSAR, SARAuthorities |
| **capco-dissem-validator** | REL TO, NOFORN, FOUO, DECON, RELIDO, FISA | H.8–9 | CVEnumISMDissem* |
| **capco-aea-validator** | Restricted Data, CNWDI, SIGMA, UCNI | H.6 | CVEnumISMAtomicEnergyMarkings |
| **capco-fgi-validator** | FGI codes, trigraphs, tetragraphs | H.7 | CVEnumISMNonUSControls |
| **capco-nato-validator** | JOINT, NATO, allied markings, 25X codes | H.3 | CVEnumISMHighWaterNATO, ISM25X |
| **capco-classification-validator** | TOP SECRET, SECRET, CONFIDENTIAL, rules | H.1–2 | CVEnumISMClassification* |
| **capco-declassification-validator** | Declassify-on, exemptions, authority blocks | E | CVEnumISMExemptFrom |
| **capco-cui-validator** | CUI BASIC/SPECIFIED, legacy FOUO | F | CVEnumISMCUI* |
| **capco-legacy-validator** | Deprecated markings, FOUO migration | F | — |

### 3 Specialized Cross-Cutting Validators

These agents provide deep expertise on concerns that span multiple marking categories:

| Agent | Focus | CAPCO § | Key CVE Enums |
|-------|-------|---------|---------------|
| **capco-declassification-markings-specialist** | Historical banner declassification syntax, modern CAB declassification, historical→modern mappings | D, E, I | CVEnumISMExemptFrom |
| **capco-cab-specialist** | Classification Authority Block structure, authority chains, multi-source resolution, precedence rules | B, D, E | CVEnumISMExemptFrom, CVEnumISMClassification* |
| **capco-deprecated-historical-specialist** | Legacy marking context, deprecation rationale, FOUO→CUI mappings, commingling rules, system evolution | F, I | CVEnumISMCUI* |

### 1 Foundational Validator

**capco-foundational** — foundational syntax rules (portions, banners, marking requirements, terminology)

## Validator Architecture: Horizontal and Vertical Expertise

The 13 validators form a two-layer expertise model:

**Horizontal (Category) Expertise**: The 10 category validators provide deep, focused knowledge of one marking system or category:
- **SCI Validator**: All things SCI — compartments, sub-compartments, ordering rules, grammar per §A.6 + §H.4
- **SAR Validator**: All things SAR — programs, structure, hierarchy per §H.5
- **Dissemination Validator**: All dissem controls — REL TO, NOFORN, FOUO, etc. per §H.8–9
- *... (7 more category validators)*

Use category validators when your question is focused on one marking domain: "Is this SCI marking valid?", "What dissem controls apply here?"

**Vertical (Specialized) Expertise**: The 3 specialized cross-cutting validators provide deep knowledge on concerns that span multiple categories:

1. **Declassification Markings Specialist**: Bridges historical declassification syntax (obsolete banner-line format) to modern CAB declassification rules. Use when: "What did this old declassification marking mean?", "How do I convert legacy banner declassification to modern CAB format?"

2. **CAB Specialist**: Authority chains, multi-source resolution, precedence rules. Use when: "How do I handle conflicting authorities from multiple classified sources?", "What's the correct derivation statement for this CAB?"

3. **Deprecated/Historical Specialist**: Legacy markings, deprecation context, FOUO→CUI mappings, commingling rules. Use when: "Why was this marking deprecated?", "What's the CUI equivalent of FOUO?", "How do I handle commingled legacy/modern markings?"

## Validation Workflow

### 1. **When adding a new rule E###**

Before PR review, validate against the appropriate agent:

```bash
invoke capco-sci-validator <<'EOF'
Rule ID: E032
Name: check_sci_compartment_order
CAPCO citation: §H.4 p15, "compartments must be sorted numerically first"
Implementation: matches on compartment ordering
Test: "SI-123 G ABCD" should fail; "SI-G 123 ABCD" should pass
EOF
```

Expected output: ✓ PASS with citation verification, or detailed issues + fixes

### 2. **When auditing token definitions**

Check tokens against ISM CVE enums:

```bash
invoke capco-classification-validator <<'EOF'
Token name: ClassificationLevel::TopSecret
CVE source: CVEnumISMClassificationAll
Expected value: "TOP SECRET"
Context: Used in banner line generation
EOF
```

### 3. **When migrating rules**

If moving rule logic between crates or versions, validate against source:

```bash
invoke capco-dissem-validator <<'EOF'
Original rule: "NOFORN implies REL TO USA only"
New implementation: RelTo intersection with noforn_clears_rel_to policy
CAPCO citation check: §H.8-9 p145
Does the new logic maintain equivalence to the source?
EOF
```

## Authority & Citation Discipline

**Key principle**: Every rule, token, and fix must be traceable to a real passage in CAPCO-2016 or the ISM XML/JSON enums.

Each validator:
- ✓ Has the authoritative CAPCO section(s) embedded in its system prompt
- ✓ Has ISM CVE enumeration data (Markdown tables) included
- ✓ Will cite §X.Y page Z for every claim
- ✓ Will flag fabricated, drifted, or misattributed citations
- ✓ Will catch predicates that don't match the source

**You cannot**: ask a validator to approve something without source grounding. If no citation exists, the validator will say so.

## Data Organization

```
~/.claude/agents/
├── capco-sci-validator.json
├── capco-sar-validator.json
├── ... (9 more agents)
└── validators/
    ├── README.md (this file)
    ├── capco/
    │   ├── foundational.md       (§A, C, D from CAPCO)
    │   ├── sci.md               (§H.4 + §A.6)
    │   ├── sar.md               (§H.5)
    │   ├── dissem.md            (§H.8-9)
    │   ├── aea.md               (§H.6)
    │   ├── fgi.md               (§H.7)
    │   ├── nato.md              (§H.3)
    │   ├── classification.md     (§H.1-2)
    │   ├── declassification.md   (§E)
    │   ├── legacy.md            (§F)
    │   └── README.md
    └── data/ism-enums/
        ├── capco-sci-validator.md
        ├── capco-sar-validator.md
        ├── ... (9 more)
        └── README.md
```

## Preventing the "39 hardcoded rules" problem

The original system had errors because:
- Rules were written without checking against CAPCO
- Citations were stale, wrong, or hallucinated
- Predicates didn't match CVE enums
- No systematic validation before commit

**With these validators**:
1. Every new rule goes through the appropriate agent first
2. Citations are verified in real-time
3. Predicates are validated against ISM XML data
4. PRs that bypass validation are caught in code review
5. Migration errors (rule logic → predicate) are caught before testing

## Best Practices

### DO:
- ✓ Invoke the validator for your category *before* you open a PR
- ✓ Reference the validator output in your PR description ("validated by capco-sci-validator")
- ✓ Use the validator's suggested fixes (they cite CAPCO)
- ✓ Ask the validator to re-check after you edit a rule

### DON'T:
- ✗ Skip validation to "move faster"
- ✗ Assume a citation is correct without validator sign-off
- ✗ Commit a rule if the validator flags issues
- ✗ Edit the validators themselves — if the data is wrong, the source (CAPCO, ISM XML) is wrong

## Extending the System

### To add a new marking category validator (e.g., NATO, NARA CUI):

1. Extract the authoritative manual section → `validators/capco/<category>.md`
2. Convert any XML/JSON enums → `validators/data/ism-enums/<category>.md`
3. Create agent definition → `~/.claude/agents/<category>-validator.json`
4. Update this README under "10 Category Validators"
5. Announce it in the project CLAUDE.md

### To add a new specialized cross-cutting validator (e.g., audit trail specialist):

1. Identify which CAPCO sections apply (may span multiple normative sections)
2. Extract relevant sections → `validators/capco/<specialist>.md`
3. Identify which ISM enums apply → `validators/data/ism-enums/<specialist>.md`
4. Create agent definition → `~/.claude/agents/<specialist>.json`
5. Update this README under "3 Specialized Cross-Cutting Validators"
6. Announce it in the project CLAUDE.md

The tool scales cleanly because each agent is independent, focused, and self-contained. New validators do not require changes to existing ones.

## Updating Agent Data

When CAPCO is updated or ISM schemas change:

1. Replace the relevant file in `validators/capco/` or `validators/data/ism-enums/`
2. The agent system prompts automatically use the latest files
3. No need to regenerate agent definitions (they reference the files dynamically)
4. Test the agent immediately to ensure it picks up changes

## Questions?

Validators are single-purpose: they check rule/token correctness against authoritative sources. They're not:
- Advisors on broader system design
- Writers of new rules (they validate existing ones)
- Documenters of architecture

For those tasks, use the broader `marque` project agents or documentation processes.

---

**Authority**: CAPCO-2016, ISM-v2022-DEC  
**Created**: 2026-04-21  
**Last Updated**: 2026-04-21
