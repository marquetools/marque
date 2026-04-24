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
    {
      provider: fontProviders.fontsource(),
      name: 'Fira Code',
      cssVariable: '--font-mono',
      weights: [300, 400, 500, 600, 700],
      styles: ['normal'],
      fallbacks: ['JetBrains Mono', 'Cascadia Code', 'Consolas', 'monospace'],
    },
    // IBM Plex Sans — body text across all three site surfaces.
    {
      provider: fontProviders.fontsource(),
      name: 'IBM Plex Sans',
      cssVariable: '--font-body',
      weights: [300, 400, 500, 600, 700],
      styles: ['normal', 'italic'],
      fallbacks: ['Inter', 'Segoe UI', 'system-ui', '-apple-system', 'sans-serif'],
    },
  ],
});
