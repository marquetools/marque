<!--
SPDX-FileCopyrightText: 2026 Knitli Inc.
SPDX-License-Identifier: MIT OR Apache-2.0

Font Software (the .woff2 files in `font/`) is licensed under the SIL
Open Font License 1.1 — see `LICENSE` in this directory. The license
header above governs only this README and surrounding project metadata.
-->

# Fira Code

Fira Code is a monospace font with programming ligatures, originally
developed by Nikita Prokopov. The marque docs site uses it for code
blocks, terminal output, and rule IDs.

## What is vendored here

`font/` contains the Latin-subset WOFF2 files at the five weights
referenced by `docs-site/astro.config.mjs`:

| File | Weight |
|---|---|
| `Fira-Code-300.woff2` | 300 (light) |
| `Fira-Code-400.woff2` | 400 (regular) |
| `Fira-Code-500.woff2` | 500 (medium) |
| `Fira-Code-600.woff2` | 600 (semibold) |
| `Fira-Code-700.woff2` | 700 (bold) |

Source: extracted from the `@fontsource/fira-code` npm package
(SIL OFL 1.1) at version 5.2.7. Re-extract by running
`pnpm add @fontsource/fira-code` and copying the `latin-<weight>-normal.woff2`
files out of `node_modules/@fontsource/fira-code/files/`.

## Why local-vendored

Previously the docs-site declared `fontProviders.fontsource()` in
`astro.config.mjs`, which causes Astro to fetch font files from the
Fontsource API and the jsDelivr CDN at build time. Vendoring the
font files removes the build-time CDN dependency entirely — the build
is fully reproducible offline, and the bytes used in production are
the bytes committed to this repo. See whitepaper §8.6 (gap register
row 18 in v0.3 / closed in v0.4).

## License

Fira Code is licensed under the SIL Open Font License 1.1. See
`LICENSE` in this directory for the full license text.
