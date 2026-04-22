# capco-validators

Specialized CAPCO/ISM validator agents and orchestration skill for the `marque` rule engine. Provides 13 focused expert agents covering every marking category, plus a skill for coordinating validation workflows.

## Purpose

These validators enforce citation discipline and predicate correctness in `marque-capco` rule implementations. Before any new rule (E###/W###/C###) is opened for review, the appropriate validator should sign off that:

- The CAPCO citation is real and accurately reflects the source
- The CVE predicate values match ISM-v2022-DEC
- The rule logic covers the edge cases the source spells out

## Installation

```bash
# Add the local development marketplace
/plugin marketplace add /path/to/marque/tools/capco-validators

# Install the plugin
/plugin install capco-validators@capco-validators-dev

# Restart Claude Code to activate
```

## Usage

Invoke the orchestration skill to find the right validator:

```
/capco-validate
```

Or delegate directly to a specific agent:

```
Use capco-sci-validator to check: Rule E032, citation §H.4 p15,
checks that compartment ordering is numeric-first then alpha.
Test: "SI-123 G ABCD" fails, "SI-G 123 ABCD" passes.
```

## The 13 Agents

### Category Validators (10)

| Agent | Domain | CAPCO § |
|-------|--------|----------|
| `capco-foundational` | Portion marks, banners, general syntax | §A–D |
| `capco-sci-validator` | SCI controls, compartments, sub-compartments | §H.4, §A.6 |
| `capco-sar-validator` | SAR/SAP markings | §H.5 |
| `capco-dissem-validator` | Dissemination controls | §H.8–9 |
| `capco-aea-validator` | Atomic Energy Act markings | §H.6 |
| `capco-fgi-validator` | Foreign Government Information | §H.7 |
| `capco-classification-validator` | US classification levels | §H.1–2 |
| `capco-declassification-validator` | CAB declassification guidance | §E |
| `capco-non-ic-validator` | Non-IC dissemination control markings | §H.9 |
| `capco-legacy-validator` | Deprecated markings | §F |

### Specialist Validators (3)

| Agent | Domain |
|-------|--------|
| `capco-declassification-markings-specialist` | Historical banner declassification syntax + modern CAB mappings |
| `capco-cab-specialist` | CAB structure, authority chains, multi-source resolution |
| `capco-deprecated-historical-specialist` | Legacy marking context, FOUO→CUI, commingling rules |

## Structure

```
capco-validators/
├── .claude-plugin/
│   ├── plugin.json
│   └── marketplace.json
├── agents/                    (13 agent .md files)
├── skills/
│   └── capco-validate/
│       └── SKILL.md           (orchestration guide + quick selection)
├── references/
│   ├── capco/                 (extracted CAPCO-2016 sections)
│   ├── ism-enums/             (ISM-v2022-DEC CVE data)
│   └── README.md              (original validator system documentation)
└── README.md
```

## Authority

**CAPCO-2016** — Intelligence Community Markings System Register and Manual  
**ISM-v2022-DEC** — ODNI Information Security Marking schema package (June 2023)
