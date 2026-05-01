<!--
SPDX-FileCopyrightText: 2026 Knitli Inc.
SPDX-License-Identifier: MIT OR Apache-2.0

Font Software (the .woff2 files in `font/`) is licensed under the SIL
Open Font License 1.1 — see `LICENSE` in this directory. The license
header above governs only this README and surrounding project metadata.
-->

# IBM Plex Sans

IBM Plex Sans is the corporate typeface designed by Mike Abbink for
IBM. The marque docs site uses it for body text across all three
site surfaces (home, blog, docs).

## What is vendored here

`font/` contains the Latin-subset WOFF2 files at the weights and
styles referenced by `site/astro.config.mjs`:

| File | Weight | Style |
|---|---|---|
| `IBM-Plex-Sans-300.woff2` | 300 (light) | normal |
| `IBM-Plex-Sans-300-italic.woff2` | 300 (light) | italic |
| `IBM-Plex-Sans-400.woff2` | 400 (regular) | normal |
| `IBM-Plex-Sans-400-italic.woff2` | 400 (regular) | italic |
| `IBM-Plex-Sans-500.woff2` | 500 (medium) | normal |
| `IBM-Plex-Sans-500-italic.woff2` | 500 (medium) | italic |
| `IBM-Plex-Sans-600.woff2` | 600 (semibold) | normal |
| `IBM-Plex-Sans-600-italic.woff2` | 600 (semibold) | italic |
| `IBM-Plex-Sans-700.woff2` | 700 (bold) | normal |
| `IBM-Plex-Sans-700-italic.woff2` | 700 (bold) | italic |

Source: extracted from the `@fontsource/ibm-plex-sans` npm package
(SIL OFL 1.1) at version 5.2.8. Re-extract by running
`pnpm add @fontsource/ibm-plex-sans` and copying the
`latin-<weight>-(normal|italic).woff2` files out of
`node_modules/@fontsource/ibm-plex-sans/files/`.

## Why local-vendored

Previously the site declared `fontProviders.fontsource()` in
`astro.config.mjs`, which causes Astro to fetch font files from the
Fontsource API and the jsDelivr CDN at build time. Vendoring the
font files removes the build-time CDN dependency entirely — the build
is fully reproducible offline, and the bytes used in production are
the bytes committed to this repo. See whitepaper §8.6 (gap register
row 18, closed in v0.6).

## License

IBM Plex Sans is licensed under the SIL Open Font License 1.1. See
`LICENSE` in this directory for the full license text.
