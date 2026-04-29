// SPDX-FileCopyrightText: 2026 Knitli Inc.
//
// SPDX-License-Identifier: Apache-2.0

/**
 * Marque Interactive Demo (worker-backed)
 *
 * Architecture:
 *   - The Marque WASM engine runs in a Web Worker (worker.js). The main
 *     thread never blocks on lint/fix/banner — typing latency stays at
 *     frame rate even on large inputs.
 *   - On debounce, the editor posts {type:'fix', seq, text, threshold,
 *     config} to the worker. The worker returns {fixedText, banner,
 *     applied[], remaining[]}.
 *   - Stale results are dropped via sequence numbers — only the most
 *     recent request's reply updates the editor.
 *
 * Surface:
 *   - Real-time banner roll-up from portion markings (computed against
 *     the post-fix text the engine returns).
 *   - Correct-as-you-type via fix() — applies each fix as a CodeMirror
 *     change at its byte offset, so the cursor never jumps.
 *   - Squiggly underlines for remaining diagnostics, with hover
 *     tooltips that show rule ID + message + CAPCO citation.
 *   - Inline audit stream — each fix produces a styled entry below the
 *     document, blending into the page background.
 *   - Faded placeholder + idle autoplay: an empty editor invites the
 *     user to type, and after ~6s of inactivity an autoplay sequence
 *     demonstrates the engine end-to-end. Any input pauses autoplay
 *     immediately.
 */

import {
  EditorView,
  Decoration,
  ViewPlugin,
  hoverTooltip,
  StateEffect,
  StateField,
  EditorState,
} from './vendor.js';

// ---------------------------------------------------------------------------
// Engine configuration (passed through to the worker)
// ---------------------------------------------------------------------------

const DEMO_CONFIG = JSON.stringify({
  corrections: {
    'SERCET':       'SECRET',
    'SECERT':       'SECRET',
    'SECRECT':      'SECRET',
    'SCERET':       'SECRET',
    'SECRTE':       'SECRET',
    'CONFIDETIAL':  'CONFIDENTIAL',
    'CONFIENTIAL':  'CONFIDENTIAL',
    'CONFIDENTAL':  'CONFIDENTIAL',
    'UNCALSSIFIED': 'UNCLASSIFIED',
    'UNCLASSFIED':  'UNCLASSIFIED',
    'UNCLASSIFED':  'UNCLASSIFIED',
    'NOFON':        'NOFORN',
    'NOFRON':       'NOFORN',
  },
});

const DEBOUNCE_MS   = 80;
const AUTOPLAY_IDLE_MS = 6_000;
const AUTOPLAY_CHAR_MS = 70;
const HISTOGRAM_BUCKETS = 20;     // 0.05-wide buckets across [0, 1]
const PLACEHOLDER_TEXT =
  'Try typing — for example: This is a (sercet//noforn) test memo.';

// Mutable: bound to the slider in the side rail. Changing this re-issues a
// fix request so the user can watch how few diagnostics actually drop out
// when the bar is raised — that's the robustness story.
let currentThreshold = 0.0;

// Mutable: bound to the deep-scan toggle in the side rail. When true, the
// worker calls `fix_deep_scan` (Phase D probabilistic recognizer) instead of
// strict `fix`. Mangled markings recover via the decoder; audit records carry
// V2 provenance (recognition × rule axes, runner_up_ratio, features).
let deepScanMode = false;

// Autoplay script: a short representative passage that exercises typo
// correction (SERCET → SECRET → S), portion abbreviation, and banner
// roll-up. Newlines are part of the script.
const AUTOPLAY_SCRIPT =
  'This memo summarizes the program review.\n\n' +
  '(SERCET//NOFORN) Initial findings indicate the system is operating ' +
  'within nominal parameters.\n\n' +
  '(C) Status reports will continue on a weekly cadence.\n';

// ---------------------------------------------------------------------------
// Worker boot
// ---------------------------------------------------------------------------

const worker = new Worker(new URL('./worker.js', import.meta.url), { type: 'module' });
let workerReady = false;
const workerReadyPromise = new Promise(resolve => {
  worker.addEventListener('message', function onReady(ev) {
    if (ev.data && ev.data.type === 'ready') {
      workerReady = true;
      worker.removeEventListener('message', onReady);
      resolve();
    }
  });
});
worker.postMessage({ type: 'configure', config: DEMO_CONFIG });

// ---------------------------------------------------------------------------
// CodeMirror state plumbing — diagnostics and placeholder
// ---------------------------------------------------------------------------

const setDiagnosticsEffect = StateEffect.define();

const diagnosticsField = StateField.define({
  create() { return Decoration.none; },
  update(decorations, tr) {
    decorations = decorations.map(tr.changes);
    for (const e of tr.effects) {
      if (e.is(setDiagnosticsEffect)) decorations = e.value;
    }
    return decorations;
  },
  provide: f => EditorView.decorations.from(f),
});

class PlaceholderWidget {
  constructor(text) { this.text = text; }
  toDOM() {
    const el = document.createElement('span');
    el.className = 'cm-placeholder';
    el.textContent = this.text;
    return el;
  }
  eq(other) { return other.text === this.text; }
  ignoreEvent() { return true; }
}

const placeholderPlugin = ViewPlugin.fromClass(class {
  constructor(view) { this.decorations = this.compute(view); }
  update(update) {
    if (update.docChanged || update.viewportChanged) {
      this.decorations = this.compute(update.view);
    }
  }
  compute(view) {
    if (view.state.doc.length === 0) {
      return Decoration.set([
        Decoration.widget({
          widget: new PlaceholderWidget(PLACEHOLDER_TEXT),
          side: 1,
        }).range(0),
      ]);
    }
    return Decoration.none;
  }
}, { decorations: v => v.decorations });

// ---------------------------------------------------------------------------
// Page-break decoration — make form-feed (\f) visible as a horizontal rule.
// ---------------------------------------------------------------------------

class PageBreakWidget {
  toDOM() {
    const el = document.createElement('span');
    el.className = 'cm-pagebreak';
    el.setAttribute('aria-label', 'Page break');
    el.textContent = '— page break —';
    return el;
  }
  eq() { return true; }
  ignoreEvent() { return true; }
}

const pageBreakPlugin = ViewPlugin.fromClass(class {
  constructor(view) { this.decorations = this.compute(view); }
  update(update) {
    if (update.docChanged || update.viewportChanged) {
      this.decorations = this.compute(update.view);
    }
  }
  compute(view) {
    const ranges = [];
    const text = view.state.doc.toString();
    for (let i = 0; i < text.length; i++) {
      if (text.charCodeAt(i) === 0x0c /* \f */) {
        ranges.push(
          Decoration.replace({ widget: new PageBreakWidget() }).range(i, i + 1),
        );
      }
    }
    return Decoration.set(ranges);
  }
}, { decorations: v => v.decorations });

// ---------------------------------------------------------------------------
// Banner classification → CSS class
// ---------------------------------------------------------------------------

const LEVEL_CLASSES = [
  ['TOP SECRET', 'level-ts'],
  ['SECRET',     'level-secret'],
  ['CONFIDENTIAL','level-confidential'],
];

function classificationClass(banner) {
  const b = banner.toUpperCase();
  for (const [prefix, cls] of LEVEL_CLASSES) {
    if (b.startsWith(prefix)) {
      if (prefix === 'TOP SECRET' && b.includes('//') && b.length > 10) {
        return 'level-ts-sci';
      }
      return cls;
    }
  }
  return 'level-unclassified';
}

const ALL_LEVEL_CLASSES = [
  'level-unclassified', 'level-confidential', 'level-secret',
  'level-ts', 'level-ts-sci', 'level-empty',
];

function applyBanner(banner, topEl, bottomEl) {
  const cls = classificationClass(banner);
  topEl.classList.remove(...ALL_LEVEL_CLASSES);
  bottomEl.classList.remove(...ALL_LEVEL_CLASSES);
  topEl.classList.add(cls);
  bottomEl.classList.add(cls);
  topEl.textContent = banner;
  bottomEl.textContent = banner;
}

// ---------------------------------------------------------------------------
// Audit stream
// ---------------------------------------------------------------------------

let auditEntryCount = 0;

function prependAuditEntry(record, stream, emptyEl) {
  if (emptyEl && emptyEl.parentNode === stream) {
    stream.removeChild(emptyEl);
  }

  const now = new Date();
  const hh = String(now.getHours()).padStart(2, '0');
  const mm = String(now.getMinutes()).padStart(2, '0');
  const ss = String(now.getSeconds()).padStart(2, '0');
  const timeStr = `${hh}:${mm}:${ss}`;

  const sourceLabel = record.source === 'CorrectionsMap' ? 'corrections-map'
    : record.source === 'BuiltinRule'    ? 'rule'
    : record.source === 'MigrationTable' ? 'migration'
    : record.source === 'DecoderPosterior' ? 'decoder'
    : String(record.source).toLowerCase();

  const pct = Math.round((record.confidence ?? 1) * 100);

  const entry = document.createElement('div');
  entry.className = 'audit-entry';
  entry.setAttribute('role', 'log');

  const fields = [
    ['audit-time',        timeStr],
    ['audit-rule',        record.rule],
    ['audit-original',    record.original],
    ['audit-arrow',       '→'],
    ['audit-replacement', record.replacement],
    ['audit-source',      sourceLabel],
    ['audit-confidence',  `${pct}%`],
  ];
  for (const [cls, text] of fields) {
    const span = document.createElement('span');
    span.className = cls;
    span.textContent = text;
    entry.appendChild(span);
  }

  // Phase D provenance — V2 audit schema. Surface recognition score and
  // runner-up posterior margin when present (deep-scan path only).
  if (typeof record.recognition === 'number' && record.recognition < 1) {
    const prov = document.createElement('div');
    prov.className = 'audit-provenance';
    const recLabel = document.createElement('span');
    recLabel.className = 'audit-prov-key';
    recLabel.textContent = 'recognition';
    prov.appendChild(recLabel);
    const recVal = document.createElement('span');
    recVal.className = 'audit-prov-val';
    recVal.textContent = record.recognition.toFixed(3);
    prov.appendChild(recVal);

    if (typeof record.runner_up_ratio === 'number') {
      const runnerLabel = document.createElement('span');
      runnerLabel.className = 'audit-prov-key';
      runnerLabel.textContent = 'runner-up';
      prov.appendChild(runnerLabel);
      const runnerVal = document.createElement('span');
      runnerVal.className = 'audit-prov-val';
      runnerVal.textContent = record.runner_up_ratio.toFixed(3);
      prov.appendChild(runnerVal);
    }

    if (Array.isArray(record.features) && record.features.length > 0) {
      const featLabel = document.createElement('span');
      featLabel.className = 'audit-prov-key';
      featLabel.textContent = 'features';
      prov.appendChild(featLabel);
      const featVal = document.createElement('span');
      featVal.className = 'audit-prov-val audit-prov-features';
      featVal.textContent = record.features.join(' · ');
      prov.appendChild(featVal);
    }
    entry.appendChild(prov);
  }

  if (stream.firstChild) stream.insertBefore(entry, stream.firstChild);
  else stream.appendChild(entry);
  auditEntryCount++;
}

function prependAuditSeparator(stream) {
  if (auditEntryCount === 0) return;
  const hr = document.createElement('hr');
  hr.className = 'audit-separator';
  if (stream.firstChild) stream.insertBefore(hr, stream.firstChild);
  else stream.appendChild(hr);
}

// ---------------------------------------------------------------------------
// Update loop — debounced post-to-worker / receive / apply
// ---------------------------------------------------------------------------

let debounceTimer = null;
let nextSeq = 1;
let activeSeq = 0;          // last-issued request seq
let lastSettledText = null; // last text we've successfully processed

function scheduleUpdate(view, refs) {
  clearTimeout(debounceTimer);
  debounceTimer = setTimeout(() => requestUpdate(view, refs), DEBOUNCE_MS);
}

function requestUpdate(view, refs, { force = false } = {}) {
  const text = view.state.doc.toString();
  if (!force && text === lastSettledText) return;
  activeSeq = nextSeq++;
  if (deepScanMode) {
    // Phase D path — no threshold, no config (decoder uses compile-time priors).
    worker.postMessage({
      type: 'fix:deep',
      seq: activeSeq,
      text,
      config: DEMO_CONFIG,
    });
  } else {
    worker.postMessage({
      type: 'fix',
      seq: activeSeq,
      text,
      threshold: currentThreshold,
      config: DEMO_CONFIG,
    });
  }
}

function applyFixResult(view, refs, msg) {
  // Drop stale results — a newer request has already been issued.
  if (msg.seq !== activeSeq) return;

  const currentText = view.state.doc.toString();

  // The seqs match, but the user may have typed in the gap between the
  // request being sent and the reply arriving. If the editor's current
  // text differs from what we'd get by applying these fixes, we re-issue
  // a fresh request rather than apply stale changes.
  // (Cheap check: if there are applied fixes, their spans must still be
  // in-bounds for the current text.)
  if (msg.applied.length > 0) {
    const inBounds = msg.applied.every(f =>
      f.span.start >= 0 && f.span.end <= currentText.length
    );
    if (!inBounds) {
      scheduleUpdate(view, refs);
      return;
    }
  }

  // Build CodeMirror change set from the applied-fix spans.
  // The two-pass fix pipeline can produce multiple applied fixes targeting
  // the same span (e.g., pass-1 C001 'SERCET'→'SECRET' then pass-2 E009
  // 'SECRET'→'S' at the same offset). CodeMirror cannot apply two changes
  // at the same range in one transaction, so we deduplicate by span key,
  // keeping only the LAST fix per span for the change set. Each individual
  // fix still gets its own audit entry below — the audit log shows what
  // the engine did; the change set shows the net effect.
  if (msg.applied.length > 0) {
    const bySpan = new Map();
    for (const f of msg.applied) {
      bySpan.set(`${f.span.start}:${f.span.end}`, f);
    }
    const changes = [...bySpan.values()]
      .map(f => ({ from: f.span.start, to: f.span.end, insert: f.replacement }))
      .sort((a, b) => a.from - b.from);
    view.dispatch({ changes });
  }

  // After patching, the editor's text equals msg.fixedText.
  lastSettledText = msg.fixedText;

  // Decorations from the remaining diagnostics — operate on the post-fix text.
  const fixedText = msg.fixedText;
  const decorationRanges = [];
  const diagData = [];
  for (const d of (msg.remaining || [])) {
    const from = d.span?.start ?? 0;
    const to   = d.span?.end   ?? from;
    if (from >= to || to > fixedText.length) continue;
    const cls = d.severity === 'error' ? 'marque-error' : 'marque-warn';
    decorationRanges.push(Decoration.mark({ class: cls }).range(from, to));
    diagData.push({ from, to, rule: d.rule, message: d.message, citation: d.citation });
  }
  decorationRanges.sort((a, b) => a.from - b.from);
  view.dispatch({ effects: setDiagnosticsEffect.of(Decoration.set(decorationRanges)) });
  view._marqueDiagData = diagData;

  // Banner.
  applyBanner(msg.banner || 'UNCLASSIFIED', refs.topBanner, refs.bottomBanner);
  refs.lastBanner = msg.banner || 'UNCLASSIFIED';
  refs.lastFixedText = fixedText;

  // Audit entries — one row per applied fix, separator between batches.
  if (msg.applied.length > 0) {
    prependAuditSeparator(refs.auditStream);
    for (const f of msg.applied) {
      prependAuditEntry(f, refs.auditStream, refs.auditEmpty);
    }
  }

  // NOFORN-clears-REL-TO supersession callout. The PageRewrite that
  // implements `capco/noforn-clears-rel-to` doesn't surface as a fix —
  // it changes the banner roll-up. Detect heuristically: any portion
  // marking on the page contains NOFORN/NF AND any portion contains
  // REL TO, but the post-rewrite banner has dropped the REL TO list.
  maybeEmitNofornCallout(fixedText, refs.lastBanner, refs);

  // Side rail: threshold counter + histogram + remaining-diagnostics list.
  updateThresholdCounter(msg.applied, msg.remaining || [], refs);
  updateConfidenceHistogram(msg.applied, msg.remaining || [], refs);
  updateRemainingPanel(view, msg.remaining || [], refs);

  // Multi-page indicator + CAB refresh — both derive from the post-fix text.
  updatePageCount(fixedText, refs);
  requestCab(refs);
}

// ---------------------------------------------------------------------------
// Side-rail: threshold counter, histogram, remaining-diagnostics panel
// ---------------------------------------------------------------------------

function fixableCount(applied, remaining) {
  // Total diagnostics that have a fix proposal at all (regardless of where
  // they landed against the threshold).
  let total = applied.length;
  for (const r of remaining) if (r.fix) total++;
  return total;
}

function updateThresholdCounter(applied, remaining, refs) {
  const total = fixableCount(applied, remaining);
  const pct = Math.round(currentThreshold * 100);
  if (total === 0) {
    refs.thresholdCounter.textContent = '—';
    return;
  }
  refs.thresholdCounter.textContent =
    `${applied.length} of ${total} applied @ ${pct}%`;
}

function updateConfidenceHistogram(applied, remaining, refs) {
  const root = refs.histogram;
  const buckets = new Array(HISTOGRAM_BUCKETS).fill(0);

  const push = (c) => {
    if (typeof c !== 'number' || !Number.isFinite(c)) return;
    const clamped = Math.max(0, Math.min(1, c));
    let idx = Math.floor(clamped * HISTOGRAM_BUCKETS);
    if (idx === HISTOGRAM_BUCKETS) idx = HISTOGRAM_BUCKETS - 1;
    buckets[idx]++;
  };
  for (const a of applied) push(a.confidence);
  for (const r of remaining) if (r.fix) push(r.fix.confidence);

  const max = Math.max(1, ...buckets);
  const tIdx = Math.min(
    HISTOGRAM_BUCKETS - 1,
    Math.floor(currentThreshold * HISTOGRAM_BUCKETS),
  );

  // Reuse / create bar elements.
  while (root.children.length < HISTOGRAM_BUCKETS) {
    const bar = document.createElement('span');
    bar.className = 'histo-bar';
    root.appendChild(bar);
  }
  while (root.children.length > HISTOGRAM_BUCKETS) {
    root.removeChild(root.lastChild);
  }

  for (let i = 0; i < HISTOGRAM_BUCKETS; i++) {
    const bar = root.children[i];
    const h = (buckets[i] / max) * 100;
    bar.style.height = buckets[i] === 0 ? '2px' : `${Math.max(6, h)}%`;
    bar.classList.remove('is-above-threshold', 'is-below-threshold', 'is-at-threshold');
    if (i === tIdx) bar.classList.add('is-at-threshold');
    else if (i > tIdx) bar.classList.add('is-above-threshold');
    else bar.classList.add('is-below-threshold');
    bar.title = `${(i / HISTOGRAM_BUCKETS).toFixed(2)}–${((i + 1) / HISTOGRAM_BUCKETS).toFixed(2)}: ${buckets[i]}`;
  }
}

function updateRemainingPanel(view, remaining, refs) {
  const list = refs.remainingList;
  const empty = refs.remainingEmpty;
  const count = refs.remainingCount;

  // Wipe and rebuild — small lists, simple wins.
  list.replaceChildren();

  if (remaining.length === 0) {
    list.appendChild(empty);
    count.textContent = 'none';
    return;
  }
  count.textContent = `${remaining.length}`;

  for (const d of remaining) {
    const item = document.createElement('li');
    item.className = `remaining-item sev-${d.severity || 'info'}`;
    item.tabIndex = 0;

    const head = document.createElement('div');
    head.className = 'remaining-head';
    const ruleEl = document.createElement('span');
    ruleEl.className = 'remaining-rule';
    ruleEl.textContent = d.rule;
    head.appendChild(ruleEl);
    if (d.fix && typeof d.fix.confidence === 'number') {
      const conf = document.createElement('span');
      conf.className = 'remaining-confidence';
      conf.textContent = `${Math.round(d.fix.confidence * 100)}% conf`;
      head.appendChild(conf);
    }
    item.appendChild(head);

    const message = document.createElement('div');
    message.className = 'remaining-message';
    message.textContent = d.message || '';
    item.appendChild(message);

    if (d.citation) {
      const cite = document.createElement('div');
      cite.className = 'remaining-citation';
      cite.textContent = d.citation;
      item.appendChild(cite);
    }

    item.addEventListener('click', () => focusEditorSpan(view, d.span));
    item.addEventListener('keydown', (ev) => {
      if (ev.key === 'Enter' || ev.key === ' ') {
        ev.preventDefault();
        focusEditorSpan(view, d.span);
      }
    });
    list.appendChild(item);
  }
}

function focusEditorSpan(view, span) {
  if (!span) return;
  const len = view.state.doc.length;
  const from = Math.max(0, Math.min(len, span.start ?? 0));
  const to   = Math.max(from, Math.min(len, span.end ?? from));
  view.focus();
  view.dispatch({
    selection: { anchor: from, head: to },
    scrollIntoView: true,
  });
}

// ---------------------------------------------------------------------------
// NOFORN-supersession callout
//
// `capco/noforn-clears-rel-to` is a PageRewrite, not a FixProposal — it
// changes the banner roll-up rather than rewriting source bytes. Detect
// heuristically: any portion contains NOFORN/NF, any portion contains
// REL TO, and the resulting banner has dropped the REL TO list.
// ---------------------------------------------------------------------------

let lastNofornState = false;

const PORTION_RE = /\(([^)]+)\)/g;

function maybeEmitNofornCallout(fixedText, banner, refs) {
  let hasNoforn = false;
  let hasRelTo  = false;
  for (const m of fixedText.matchAll(PORTION_RE)) {
    const inner = m[1].toUpperCase();
    if (/\bNOFORN\b|\bNF\b/.test(inner)) hasNoforn = true;
    if (/REL\s+TO\b/.test(inner)) hasRelTo = true;
  }
  const bannerLacksRelTo = !banner.toUpperCase().includes('REL TO');
  const fired = hasNoforn && hasRelTo && bannerLacksRelTo;

  // Only emit when transitioning into the fired state — avoids spamming
  // the audit stream with one callout per keystroke.
  if (fired && !lastNofornState) {
    prependNofornCallout(refs.auditStream, refs.auditEmpty);
  }
  lastNofornState = fired;
}

function prependNofornCallout(stream, emptyEl) {
  if (emptyEl && emptyEl.parentNode === stream) {
    stream.removeChild(emptyEl);
  }
  const el = document.createElement('div');
  el.className = 'audit-callout';
  el.setAttribute('role', 'note');

  const tag = document.createElement('span');
  tag.className = 'audit-callout-tag';
  tag.textContent = 'lattice';
  el.appendChild(tag);

  const body = document.createElement('span');
  body.className = 'audit-callout-body';
  body.textContent =
    'NOFORN supersedes REL TO at the page level — REL TO list cleared from the banner.';
  el.appendChild(body);

  const rule = document.createElement('span');
  rule.className = 'audit-callout-rule';
  rule.textContent = 'capco/noforn-clears-rel-to';
  el.appendChild(rule);

  if (stream.firstChild) stream.insertBefore(el, stream.firstChild);
  else stream.appendChild(el);
}

// ---------------------------------------------------------------------------
// Banner-derivation tooltip
//
// On focus or hover of either banner, parse all portion markings out of the
// post-fix text and list them. Doesn't reach into the engine — the surface
// area we need (which portions contributed) is already visible in the source.
// ---------------------------------------------------------------------------

function attachBannerTooltip(refs) {
  const tt = refs.bannerTooltip;
  if (!tt) return;

  function show(target) {
    const banner = refs.lastBanner || 'UNCLASSIFIED';
    const text   = refs.lastFixedText || '';
    const portions = [...text.matchAll(PORTION_RE)].map(m => m[1].trim());

    tt.replaceChildren();
    const title = document.createElement('div');
    title.className = 'banner-tooltip-title';
    title.textContent = 'Banner derivation';
    tt.appendChild(title);

    const ruleLine = document.createElement('div');
    ruleLine.className = 'banner-tooltip-rule';
    ruleLine.textContent =
      'max(classification) ∪ caveats across all portions on the page';
    tt.appendChild(ruleLine);

    if (portions.length === 0) {
      const empty = document.createElement('div');
      empty.className = 'banner-tooltip-empty';
      empty.textContent = 'No portion markings yet — banner is the default.';
      tt.appendChild(empty);
    } else {
      const list = document.createElement('div');
      list.className = 'banner-tooltip-portions';
      const seen = new Set();
      for (const p of portions) {
        if (seen.has(p)) continue;
        seen.add(p);
        const li = document.createElement('span');
        li.textContent = `(${p})`;
        list.appendChild(li);
        if (seen.size >= 8) break;
      }
      tt.appendChild(list);
      if (portions.length > seen.size) {
        const more = document.createElement('span');
        more.className = 'banner-tooltip-empty';
        more.textContent = `…and ${portions.length - seen.size} more.`;
        tt.appendChild(more);
      }
    }

    const rect = target.getBoundingClientRect();
    // Position centered horizontally just under the banner.
    tt.setAttribute('aria-hidden', 'false');
    tt.dataset.open = 'true';
    // Render once to measure, then position.
    const ttRect = tt.getBoundingClientRect();
    let left = rect.left + rect.width / 2 - ttRect.width / 2;
    left = Math.max(8, Math.min(window.innerWidth - ttRect.width - 8, left));
    let top = rect.bottom + 6;
    if (top + ttRect.height > window.innerHeight - 8) {
      top = rect.top - ttRect.height - 6;
    }
    tt.style.left = `${left + window.scrollX}px`;
    tt.style.top  = `${top + window.scrollY}px`;
  }

  function hide() {
    tt.setAttribute('aria-hidden', 'true');
    tt.dataset.open = 'false';
  }

  for (const el of [refs.topBanner, refs.bottomBanner]) {
    el.addEventListener('mouseenter', () => show(el));
    el.addEventListener('mouseleave', hide);
    el.addEventListener('focus',      () => show(el));
    el.addEventListener('blur',       hide);
  }
}

// ---------------------------------------------------------------------------
// Threshold control wiring
// ---------------------------------------------------------------------------

function attachThresholdControls(view, refs) {
  const slider = refs.thresholdSlider;
  const valueEl = refs.thresholdValue;
  const presets = refs.thresholdPresets;

  function setThreshold(t, opts = {}) {
    currentThreshold = Math.max(0, Math.min(1, Number(t) || 0));
    valueEl.textContent = currentThreshold.toFixed(2);
    if (slider.value !== String(currentThreshold)) {
      slider.value = String(currentThreshold);
    }
    // Update the active preset chip — exact match wins, otherwise nothing.
    for (const chip of presets) {
      const v = parseFloat(chip.dataset.threshold);
      chip.classList.toggle('is-active', Math.abs(v - currentThreshold) < 1e-6);
    }
    if (!opts.silent) {
      // Force a fresh request even if the document hasn't changed — only
      // the threshold has, and we want the side rail to reflect that.
      requestUpdate(view, refs, { force: true });
    }
  }

  slider.addEventListener('input', () => setThreshold(slider.value));
  for (const chip of presets) {
    chip.addEventListener('click', () => setThreshold(chip.dataset.threshold));
  }

  // Seed the readout — but don't force-issue a request before the worker
  // has even processed the initial empty-doc lint.
  setThreshold(currentThreshold, { silent: true });
}

// ---------------------------------------------------------------------------
// Deep-scan toggle wiring
// ---------------------------------------------------------------------------

function attachDeepScanToggle(view, refs) {
  const cb = refs.deepScanToggle;
  const note = refs.deepScanNote;
  if (!cb) return;

  cb.addEventListener('change', () => {
    deepScanMode = !!cb.checked;
    if (refs.docCard) {
      refs.docCard.classList.toggle('is-deepscan', deepScanMode);
    }
    if (note) {
      note.innerHTML = deepScanMode
        ? 'On — probabilistic decoder. Synthetic <code>R001</code> diagnostics carry decoder posteriors.'
        : 'Off — strict header-only recognizer. Try <code>SECRT//NOFRN</code> with this on.';
    }
    requestUpdate(view, refs, { force: true });
  });
}

// ---------------------------------------------------------------------------
// Multi-page support — page-break insertion + per-page banner ladder
// ---------------------------------------------------------------------------

const PAGE_BREAK = '\f';

function splitPages(text) {
  // Pages are separated by form-feed (\f) — what the engine treats as a hard
  // page break. Empty trailing pages are dropped so the page count matches
  // what the user sees.
  const raw = text.split(PAGE_BREAK);
  const pages = [];
  for (let i = 0; i < raw.length; i++) {
    const page = raw[i];
    if (i === raw.length - 1 && page.length === 0) continue;
    pages.push(page);
  }
  return pages.length > 0 ? pages : [''];
}

function attachPageBreakButton(view, refs) {
  const btn = refs.insertPageBreak;
  if (!btn) return;
  btn.addEventListener('click', () => {
    const sel = view.state.selection.main;
    const insert = (sel.from === 0 ? '' : '\n') + PAGE_BREAK + '\n';
    view.dispatch({
      changes: { from: sel.from, to: sel.to, insert },
      selection: { anchor: sel.from + insert.length },
    });
    view.focus();
  });
}

function updatePageCount(text, refs) {
  if (!refs.docPageCount) return;
  const pages = splitPages(text);
  if (pages.length <= 1) {
    refs.docPageCount.textContent = '';
    return;
  }
  refs.docPageCount.textContent = `${pages.length} pages · banner resets per page`;
}

// ---------------------------------------------------------------------------
// CAB generator wiring
// ---------------------------------------------------------------------------

let cabSeq = 0;

function attachCabPanel(refs) {
  const toggleBtn = refs.toggleCab;
  const panel = refs.cabPanel;
  if (!toggleBtn || !panel) return;

  toggleBtn.addEventListener('click', () => {
    const isHidden = panel.hidden;
    panel.hidden = !isHidden;
    toggleBtn.classList.toggle('is-active', isHidden);
    const label = toggleBtn.querySelector('.doc-tool-label');
    if (label) label.textContent = isHidden ? 'Hide CAB' : 'Show CAB';
    if (isHidden) requestCab(refs);
  });

  for (const input of [refs.cabClassifiedBy, refs.cabDerivedFrom]) {
    if (!input) continue;
    input.addEventListener('input', () => requestCab(refs));
  }
}

function requestCab(refs) {
  if (!refs.cabPanel || refs.cabPanel.hidden) return;
  cabSeq++;
  worker.postMessage({
    type: 'cab',
    seq: cabSeq,
    text: refs.lastFixedText || '',
    classifiedBy: (refs.cabClassifiedBy?.value || '').trim() || null,
    derivedFrom:  (refs.cabDerivedFrom?.value  || '').trim() || null,
    config: DEMO_CONFIG,
  });
}

function applyCabResult(refs, msg) {
  if (msg.seq !== cabSeq) return;
  if (refs.cabOutput) {
    refs.cabOutput.textContent = msg.cab || '(no portion markings yet)';
  }
}

// ---------------------------------------------------------------------------
// Scenario tabs — U.S. memo vs FGI / JOINT
// ---------------------------------------------------------------------------

const SCENARIOS = {
  us: {
    note: 'Standard U.S. classification with SCI, SAR, and dissem controls.',
    seed:
      'This memo summarizes the program review.\n\n' +
      '(SERCET//NOFORN) Initial findings indicate the system is operating ' +
      'within nominal parameters.\n\n' +
      '(C) Status reports will continue on a weekly cadence.\n',
  },
  fgi: {
    note:
      'Foreign Government Information, JOINT classification, and NATO ' +
      'tetragraphs. CAPCO §H.7 country-code ordering applies.',
    seed:
      'JOINT memo — coalition program coordination.\n\n' +
      '(//FGI GBR S) Analytical assessment shared by the United Kingdom.\n\n' +
      '(//JOINT SECRET USA, GBR, AUS) Combined operational summary; ' +
      'releasability follows the lowest classifier.\n\n' +
      '(//SECRET//REL TO USA, FVEY) Distribution limited to Five Eyes ' +
      'partners — note tetragraph form.\n',
  },
};

function attachScenarioTabs(view, refs) {
  const tabs = refs.scenarioTabs;
  if (!tabs || tabs.length === 0) return;

  for (const tab of tabs) {
    tab.addEventListener('click', () => {
      const key = tab.dataset.scenario;
      const scenario = SCENARIOS[key];
      if (!scenario) return;
      for (const t of tabs) {
        const active = t === tab;
        t.classList.toggle('is-active', active);
        t.setAttribute('aria-selected', active ? 'true' : 'false');
      }
      if (refs.scenarioNote) refs.scenarioNote.textContent = scenario.note;

      // Replace the document content with the scenario seed. This is a hard
      // reset: we want to demonstrate a different domain, not splice into
      // existing prose.
      const len = view.state.doc.length;
      view.dispatch({
        changes: { from: 0, to: len, insert: scenario.seed },
        selection: { anchor: scenario.seed.length },
      });
      view.focus();
    });
  }
}

// ---------------------------------------------------------------------------
// Idle autoplay
// ---------------------------------------------------------------------------

function makeAutoplay(view) {
  let charIdx = 0;
  let typingTimer = null;
  let idleTimer   = null;
  let aborted     = false;

  function tick() {
    if (aborted || charIdx >= AUTOPLAY_SCRIPT.length) return;
    const ch = AUTOPLAY_SCRIPT[charIdx++];
    const pos = view.state.doc.length;
    view.dispatch({
      changes: { from: pos, to: pos, insert: ch },
      // Keep the cursor at the inserted position so subsequent inserts append.
      selection: { anchor: pos + ch.length },
    });
    typingTimer = setTimeout(tick, AUTOPLAY_CHAR_MS);
  }

  function start() {
    if (aborted) return;
    if (view.state.doc.length > 0) return; // user already typed
    charIdx = 0;
    tick();
  }

  function abort() {
    if (aborted) return;
    aborted = true;
    clearTimeout(typingTimer);
    clearTimeout(idleTimer);
  }

  idleTimer = setTimeout(start, AUTOPLAY_IDLE_MS);

  return { abort };
}

// ---------------------------------------------------------------------------
// Init
// ---------------------------------------------------------------------------

async function main() {
  await workerReadyPromise;

  const refs = {
    topBanner:    document.getElementById('banner-top'),
    bottomBanner: document.getElementById('banner-bottom'),
    docCard:      document.getElementById('document-card'),
    auditStream:  document.getElementById('audit-stream'),
    auditEmpty:   document.getElementById('audit-empty'),
    bannerTooltip:    document.getElementById('banner-tooltip'),
    thresholdSlider:  document.getElementById('threshold-slider'),
    thresholdValue:   document.getElementById('threshold-value'),
    thresholdCounter: document.getElementById('threshold-counter'),
    thresholdPresets: document.querySelectorAll('.preset-chip'),
    histogram:        document.getElementById('confidence-histogram'),
    remainingList:    document.getElementById('remaining-list'),
    remainingEmpty:   document.getElementById('remaining-empty'),
    remainingCount:   document.getElementById('remaining-count'),
    contrastStrip:    document.getElementById('contrast-strip'),
    contrastDismiss:  document.getElementById('contrast-dismiss'),
    deepScanToggle:   document.getElementById('deepscan-toggle'),
    deepScanNote:     document.getElementById('deepscan-note'),
    insertPageBreak:  document.getElementById('insert-page-break'),
    docPageCount:     document.getElementById('doc-page-count'),
    toggleCab:        document.getElementById('toggle-cab'),
    cabPanel:         document.getElementById('cab-panel'),
    cabClassifiedBy:  document.getElementById('cab-classified-by'),
    cabDerivedFrom:   document.getElementById('cab-derived-from'),
    cabOutput:        document.getElementById('cab-output'),
    scenarioTabs:     document.querySelectorAll('.scenario-tab'),
    scenarioNote:     document.getElementById('scenario-note'),
    lastBanner: 'UNCLASSIFIED',
    lastFixedText: '',
  };

  // Contrast strip: dismissable, persisted in localStorage.
  if (refs.contrastStrip && refs.contrastDismiss) {
    try {
      if (localStorage.getItem('marque-demo:contrast-dismissed') === '1') {
        refs.contrastStrip.hidden = true;
      }
    } catch { /* localStorage may be unavailable */ }
    refs.contrastDismiss.addEventListener('click', () => {
      refs.contrastStrip.hidden = true;
      try { localStorage.setItem('marque-demo:contrast-dismissed', '1'); }
      catch { /* ignore */ }
    });
  }

  // Initial empty banner — UNCLASSIFIED.
  applyBanner('UNCLASSIFIED', refs.topBanner, refs.bottomBanner);

  // Banner-derivation tooltip — hover/focus on either banner.
  attachBannerTooltip(refs);

  // Worker → main: receive results, drop stale ones.
  worker.addEventListener('message', (ev) => {
    const msg = ev.data;
    if (!msg) return;
    if (msg.type === 'fix:result')      applyFixResult(view, refs, msg);
    else if (msg.type === 'cab:result') applyCabResult(refs, msg);
    else if (msg.type === 'error')      console.error('[marque worker]', msg);
  });

  // Tooltip extension reads from view._marqueDiagData.
  const tooltip = hoverTooltip((view, pos) => {
    const diags = view._marqueDiagData || [];
    const match = diags.find(d => pos >= d.from && pos < d.to);
    if (!match) return null;
    return {
      pos: match.from,
      end: match.to,
      above: true,
      create() {
        const dom = document.createElement('div');
        dom.className = 'marque-tooltip';
        const rule = document.createElement('div');
        rule.className = 'tip-rule'; rule.textContent = match.rule;
        const msg  = document.createElement('div');
        msg.className = 'tip-message'; msg.textContent = match.message;
        const cite = document.createElement('div');
        cite.className = 'tip-citation'; cite.textContent = match.citation;
        dom.append(rule, msg, cite);
        return { dom };
      },
    };
  });

  let autoplay; // forward declaration so updateListener can abort

  const startState = EditorState.create({
    doc: '',
    extensions: [
      diagnosticsField,
      placeholderPlugin,
      pageBreakPlugin,
      tooltip,
      EditorView.lineWrapping,
      EditorView.theme({
        '&': { background: 'transparent', color: 'var(--color-text-primary)' },
        '.cm-content': {
          fontFamily: 'var(--font-body)',
          fontSize: '14px',
          lineHeight: '1.85',
          caretColor: 'var(--color-text-primary)',
        },
        '.cm-cursor': { borderLeftColor: 'var(--color-text-primary)' },
        '.cm-selectionBackground': { background: 'rgba(126, 152, 218, 0.3) !important' },
        '&.cm-focused .cm-selectionBackground': {
          background: 'rgba(126, 152, 218, 0.4) !important',
        },
        '.cm-gutters': { display: 'none' },
        '.cm-activeLine': { background: 'transparent' },
        '.cm-activeLineGutter': { background: 'transparent' },
      }, { dark: false }),
      EditorView.updateListener.of(update => {
        if (!update.docChanged) return;
        const newText = update.view.state.doc.toString();
        if (newText === lastSettledText) return;
        scheduleUpdate(update.view, refs);
      }),
    ],
  });

  const view = new EditorView({
    state: startState,
    parent: document.getElementById('editor-mount'),
  });

  // Threshold controls — wire after the view exists so chips/slider can
  // re-issue requestUpdate against the editor.
  attachThresholdControls(view, refs);

  // Tier 3 wiring — deep-scan, page-break, CAB, scenario tabs.
  attachDeepScanToggle(view, refs);
  attachPageBreakButton(view, refs);
  attachCabPanel(refs);
  attachScenarioTabs(view, refs);

  // Pause autoplay on any user input (keystroke, paste, click that sets focus).
  // The autoplay's own dispatches set the 'autoplay' annotation, so we
  // detect human input by absence — abort whenever a non-tagged docChange
  // occurs after autoplay started. To keep this simple, we abort on the
  // first user-driven 'keydown' or 'beforeinput' event in the editor.
  const abortAutoplay = () => { if (autoplay) autoplay.abort(); };
  view.dom.addEventListener('keydown',     abortAutoplay, { once: true });
  view.dom.addEventListener('beforeinput', abortAutoplay, { once: true });
  view.dom.addEventListener('paste',       abortAutoplay, { once: true });

  autoplay = makeAutoplay(view);
}

main().catch(err => {
  console.error('[marque demo] init failed:', err);
});
