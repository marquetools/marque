<!--
SPDX-FileCopyrightText: 2026 Knitli Inc.

SPDX-License-Identifier: MIT OR Apache-2.0
-->

# marque-config

Layered configuration loading for Marque.

`marque-config` resolves a single, validated `Config` from up to four sources
and hard-fails fast on misuse. The engine never reads files itself — it
takes the resolved `Config` as a constructor argument.

## Role in Marque

```
.marque.toml ─┐
.marque.local.toml ─┼─→ marque-config ─→ Config ─→ marque-engine
env vars ─┘            (validates, merges)
CLI flags ─────────────┘ (caller-applied)
```

## Precedence

Highest wins:

```
CLI flags  >  env vars  >  .marque.local.toml  >  .marque.toml
```

`load(start_dir)` walks upward from `start_dir` and stops at the first
`.marque.toml`, `.git/`, or filesystem root. `load_with_explicit_config(path)`
short-circuits the walk and uses `path` directly.

## Usage

```rust
use marque_config::load;

let cwd = std::env::current_dir()?;
let config = load(&cwd)?;

println!("threshold = {}", config.confidence_threshold());
# Ok::<(), Box<dyn std::error::Error>>(())
```

## Config files

`.marque.toml` — committed, project/org policy:

```toml
[capco]
version = "ISM-v2022-DEC"

[rules]
E001 = "fix"                    # portion-mark-in-banner; off | info | warn | error | fix
E002 = "warn"                   # missing-usa-trigraph
"missing-usa-trigraph" = "warn" # equivalent — rule names are accepted as aliases

[corrections]
"SERCET" = "SECRET"
```

`.marque.local.toml` — gitignored, per-user identity. **Never** commit:

```toml
[user]
classifier_id = "TEST-12345"
classification_authority = "EO 13526"
default_reason = "1.4(c)"              # optional
derived_from_default = "Multiple Sources"  # optional
```

## Rule keys: IDs *or* names

The `[rules]` section accepts **either** the canonical rule ID (`E001`) **or**
the rule name (`portion-mark-in-banner`) as the key. Both forms resolve to
the same rule at engine construction time. Using the name form makes configs
more self-documenting; the ID form is shorter and stable across renames.

Writing **both** forms with **different** severities for the same rule is
rejected at engine construction time — one form would have silently won the
HashMap iteration race and the other would have been dropped. Writing both
forms with the **same** severity is accepted silently.

## Hard-fail validators

The loader (or engine constructor) refuses to produce a `Config` / `Engine`
(exit code in parens) when:

- `.marque.toml` contains a `[user]` section — identity must live only in
  `.marque.local.toml` or env vars (FR-010, exit `65`).
- `[capco] version` does not match the compiled `marque_ism::SCHEMA_VERSION`
  (FR-011, exit `65`).
- `confidence_threshold` is outside `[0.0, 1.0]` (exit `65`).
- A rule severity string is not one of `off`, `info`, `warn`, `error`, `fix`
  (exit `65`).
- The config file cannot be read (exit `74`).
- A `[rules]` key is not a registered rule ID or name — the engine emits
  `EngineConstructionError::UnknownRuleOverride` with a best-effort
  "did you mean…" suggestion (exit `65`).
- A `[rules]` section specifies the same rule two different ways with
  conflicting severities — the engine emits
  `EngineConstructionError::ConflictingRuleOverride` (exit `65`).

`ConfigError::exit_code()` and `EngineConstructionError::exit_code()`
return the appropriate code for the CLI.

## Public types

| Type | Role |
|---|---|
| `Config` | The resolved, merged configuration. |
| `UserConfig` | Classifier identity (from `.marque.local.toml` or env). |
| `RuleConfig` | Per-rule severity overrides. |
| `CapcoConfig` | Schema version pin. |
| `ConfigError` | Validation failures, with `exit_code()` mapping. |

## License

Marque License 1.0 (`LicenseRef-MarqueLicense-1.0`). See [LICENSE.md](./LICENSE.md).
