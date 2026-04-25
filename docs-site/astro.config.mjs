// SPDX-FileCopyrightText: 2026 Knitli Inc.
//
// SPDX-License-Identifier: MIT OR Apache-2.0

import { defineConfig, fontProviders } from 'astro/config';

export default defineConfig({
  site: 'https://marque.rs',
  compressHTML: true,
  build: {
    assets: 'assets',
  },
  fonts: [
    // OCR-B — local file, the brand display font.
    // Used for marking badges, section labels, the (M) in the logo.
    {
      provider: fontProviders.local(),
      name: 'OCR B',
      cssVariable: '--font-display',
      fallbacks: ['OCR-B', 'Courier Prime', 'Courier New', 'monospace'],
      options: {
        variants: [
          {
            src: ['./src/assets/OCR-B/font/OCR-B.otf', './src/assets/OCR-B/font/OCR-B.ttf'],
            weight: 'normal',
            style: 'normal',
          },
        ],
      },
    },
    // Fira Code — monospace for code blocks, terminal output, rule IDs.
    // Vendored locally under `src/assets/Fira-Code/font/` (SIL OFL 1.1) so the
    // build does not depend on `api.fontsource.org` / `cdn.jsdelivr.net` at
    // build time. See `src/assets/Fira-Code/README.md` and whitepaper §8.6
    // (gap register #18, closed in v0.4).
    {
      provider: fontProviders.local(),
      name: 'Fira Code',
      cssVariable: '--font-mono',
      fallbacks: ['JetBrains Mono', 'Cascadia Code', 'Consolas', 'monospace'],
      options: {
        variants: [
          { src: ['./src/assets/Fira-Code/font/Fira-Code-300.woff2'], weight: 300, style: 'normal' },
          { src: ['./src/assets/Fira-Code/font/Fira-Code-400.woff2'], weight: 400, style: 'normal' },
          { src: ['./src/assets/Fira-Code/font/Fira-Code-500.woff2'], weight: 500, style: 'normal' },
          { src: ['./src/assets/Fira-Code/font/Fira-Code-600.woff2'], weight: 600, style: 'normal' },
          { src: ['./src/assets/Fira-Code/font/Fira-Code-700.woff2'], weight: 700, style: 'normal' },
        ],
      },
    },
    // IBM Plex Sans — body text across all three site surfaces.
    // Vendored locally under `src/assets/IBM-Plex-Sans/font/` (SIL OFL 1.1)
    // for the same reason as Fira Code above. See
    // `src/assets/IBM-Plex-Sans/README.md`.
    {
      provider: fontProviders.local(),
      name: 'IBM Plex Sans',
      cssVariable: '--font-body',
      fallbacks: ['Inter', 'Segoe UI', 'system-ui', '-apple-system', 'sans-serif'],
      options: {
        variants: [
          { src: ['./src/assets/IBM-Plex-Sans/font/IBM-Plex-Sans-300.woff2'],        weight: 300, style: 'normal' },
          { src: ['./src/assets/IBM-Plex-Sans/font/IBM-Plex-Sans-300-italic.woff2'], weight: 300, style: 'italic' },
          { src: ['./src/assets/IBM-Plex-Sans/font/IBM-Plex-Sans-400.woff2'],        weight: 400, style: 'normal' },
          { src: ['./src/assets/IBM-Plex-Sans/font/IBM-Plex-Sans-400-italic.woff2'], weight: 400, style: 'italic' },
          { src: ['./src/assets/IBM-Plex-Sans/font/IBM-Plex-Sans-500.woff2'],        weight: 500, style: 'normal' },
          { src: ['./src/assets/IBM-Plex-Sans/font/IBM-Plex-Sans-500-italic.woff2'], weight: 500, style: 'italic' },
          { src: ['./src/assets/IBM-Plex-Sans/font/IBM-Plex-Sans-600.woff2'],        weight: 600, style: 'normal' },
          { src: ['./src/assets/IBM-Plex-Sans/font/IBM-Plex-Sans-600-italic.woff2'], weight: 600, style: 'italic' },
          { src: ['./src/assets/IBM-Plex-Sans/font/IBM-Plex-Sans-700.woff2'],        weight: 700, style: 'normal' },
          { src: ['./src/assets/IBM-Plex-Sans/font/IBM-Plex-Sans-700-italic.woff2'], weight: 700, style: 'italic' },
        ],
      },
    },
  ],
});
