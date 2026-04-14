/**
 * Marque Interactive Demo
 *
 * Loads the Marque WASM module and wires up the classified-memo editor:
 * - Real-time banner rollup from portion markings (compute_banner)
 * - Auto-fix via fix() at threshold 0.0 (applies all suggestions)
 * - Squiggly underlines via CodeMirror 6 Decoration API (lint)
 * - Hover tooltips showing rule ID, message, CAPCO citation
 * - CAB generation on demand (generate_cab)
 * - Playground section for ad-hoc testing
 */

import init, { lint, fix, compute_banner, generate_cab }
  from '../crates/marque-wasm/pkg/marque_wasm.js';

import {
  EditorView,
  Decoration,
  hoverTooltip,
  StateEffect,
  StateField,
  EditorState,
} from './vendor.js';

// ---------------------------------------------------------------------------
// Pre-loaded document body
// ---------------------------------------------------------------------------

const INITIAL_BODY = `(TS//SI/TK//NF) This paragraph covers the program's overall status and top-line findings from the most recent collection cycle.

(S//NF) Supporting analysis has been coordinated across the relevant agencies and is reflected in the assessment below.

(U) This paragraph contains general background information and does not require special handling.

(SERCET//NF) This paragraph contains a classification typo — watch it correct automatically.`;

// ---------------------------------------------------------------------------
// CodeMirror decoration effects and state field
// ---------------------------------------------------------------------------

const setDiagnosticsEffect = StateEffect.define();

const diagnosticsField = StateField.define({
  create() { return Decoration.none; },
  update(decorations, tr) {
    decorations = decorations.map(tr.changes);
    for (const e of tr.effects) {
      if (e.is(setDiagnosticsEffect)) {
        decorations = e.value;
      }
    }
    return decorations;
  },
  provide: f => EditorView.decorations.from(f),
});


// ---------------------------------------------------------------------------
// Classification level → CSS class
// ---------------------------------------------------------------------------

function classificationClass(banner) {
  const b = banner.toUpperCase();
  if (b.startsWith('TOP SECRET') && (b.includes('//') && !b.endsWith('TOP SECRET'))) {
    // TS with SCI
    return 'level-ts-sci';
  }
  if (b.startsWith('TOP SECRET')) return 'level-ts';
  if (b.startsWith('SECRET')) return 'level-secret';
  if (b.startsWith('CONFIDENTIAL')) return 'level-confidential';
  if (b === 'UNCLASSIFIED') return 'level-unclassified';
  return 'level-unclassified';
}

function escapeHtml(s) {
  return s.replace(/&/g, '&amp;').replace(/</g, '&lt;').replace(/>/g, '&gt;');
}

// ---------------------------------------------------------------------------
// Banner update
// ---------------------------------------------------------------------------

function updateBanners(text, topEl, bottomEl) {
  let banner;
  try {
    banner = compute_banner(text);
  } catch (e) {
    banner = 'UNCLASSIFIED';
  }
  const cls = classificationClass(banner);

  // Remove old level classes
  const levels = ['level-unclassified','level-confidential','level-secret','level-ts','level-ts-sci','level-empty'];
  topEl.classList.remove(...levels);
  bottomEl.classList.remove(...levels);

  topEl.classList.add(cls);
  bottomEl.classList.add(cls);
  topEl.textContent = banner;
  bottomEl.textContent = banner;
}

// ---------------------------------------------------------------------------
// Issues panel update
// ---------------------------------------------------------------------------

function updateIssues(diagList, issuesList, issuesHeader) {
  issuesList.innerHTML = '';

  if (diagList.length === 0) {
    issuesList.innerHTML = '<li class="issues-empty">No issues found.</li>';
    issuesHeader.querySelector('.badge').textContent = '✓';
    issuesHeader.querySelector('.badge').className = 'badge badge-ok';
    return;
  }

  const errorCount = diagList.filter(d => d.severity === 'error').length;
  const warnCount  = diagList.filter(d => d.severity === 'warning').length;

  const badge = issuesHeader.querySelector('.badge');
  badge.textContent = String(errorCount + warnCount);
  badge.className = errorCount > 0 ? 'badge badge-error' : 'badge badge-warn';

  for (const d of diagList) {
    const li = document.createElement('li');
    const ruleSpan = document.createElement('span');
    ruleSpan.className = `issue-rule severity-${d.severity}`;
    ruleSpan.textContent = d.rule;

    const msgSpan = document.createElement('span');
    msgSpan.className = 'issue-message';
    msgSpan.textContent = d.message;

    li.appendChild(ruleSpan);
    li.appendChild(msgSpan);
    issuesList.appendChild(li);
  }
}

// ---------------------------------------------------------------------------
// Main debounced update loop
// ---------------------------------------------------------------------------

let debounceTimer = null;
const DEBOUNCE_MS = 50;

function parseNdjson(ndjson) {
  return ndjson.trim().split('\n')
    .filter(Boolean)
    .map(line => { try { return JSON.parse(line); } catch { return null; } })
    .filter(Boolean);
}

function runUpdate(view, topBanner, bottomBanner, issuesList, issuesHeader) {
  let text = view.state.doc.toString();

  // 1. Apply all fixes (threshold 0.0 → apply everything including E003 reorders)
  let fixResult;
  try {
    fixResult = JSON.parse(fix(text, 0.0, null));
  } catch (e) {
    fixResult = null;
  }

  if (fixResult && fixResult.applied && fixResult.applied.length > 0) {
    const fixed = fixResult.fixed_text;
    if (fixed !== text) {
      // Replace editor contents, preserving undo history
      view.dispatch({
        changes: { from: 0, to: view.state.doc.length, insert: fixed },
      });
      text = fixed;
    }
  }

  // 2. Lint for remaining diagnostics (squiggles)
  let diagList = [];
  try {
    diagList = parseNdjson(lint(text, null));
  } catch (e) {
    diagList = [];
  }

  // 3. Build CodeMirror decorations from span info
  const decorationRanges = [];
  const diagData = [];

  for (const d of diagList) {
    const from = d.span?.start ?? 0;
    const to   = d.span?.end   ?? from;
    if (from >= to || to > text.length) continue;

    const cls = d.severity === 'error' ? 'marque-error' : 'marque-warn';
    decorationRanges.push(Decoration.mark({ class: cls }).range(from, to));
    diagData.push({ from, to, rule: d.rule, message: d.message, citation: d.citation });
  }

  decorationRanges.sort((a, b) => a.from - b.from);
  const decos = Decoration.set(decorationRanges);

  // Dispatch decoration update to CodeMirror
  view.dispatch({
    effects: setDiagnosticsEffect.of(decos),
  });

  // Store diagnostic data on the view instance for tooltip access.
  // The hoverTooltip callback reads view._marqueDiagData directly.
  view._marqueDiagData = diagData;

  // 4. Update banners
  updateBanners(text, topBanner, bottomBanner);

  // 5. Update issues panel
  updateIssues(diagList, issuesList, issuesHeader);
}

// ---------------------------------------------------------------------------
// Init
// ---------------------------------------------------------------------------

async function main() {
  await init();

  const topBanner    = document.getElementById('banner-top');
  const bottomBanner = document.getElementById('banner-bottom');
  const issuesList   = document.getElementById('issues-list');
  const issuesHeader = document.getElementById('issues-header');
  const cabContent   = document.getElementById('cab-content');
  const btnCab       = document.getElementById('btn-generate-cab');

  // Build a simpler tooltip that reads from view._marqueDiagData
  const simpleTip = hoverTooltip((view, pos) => {
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
        dom.innerHTML = `
          <div class="tip-rule">${escapeHtml(match.rule)}</div>
          <div class="tip-message">${escapeHtml(match.message)}</div>
          <div class="tip-citation">${escapeHtml(match.citation)}</div>
        `;
        return { dom };
      },
    };
  });

  // Create CodeMirror editor with document-style theme
  const startState = EditorState.create({
    doc: INITIAL_BODY,
    extensions: [
      diagnosticsField,
      simpleTip,
      EditorView.lineWrapping,
      EditorView.theme({
        '&': {
          background: 'transparent',
          color: '#222',
        },
        '.cm-content': {
          fontFamily: "Georgia, 'Times New Roman', serif",
          fontSize: '14px',
          lineHeight: '1.85',
          caretColor: '#222',
        },
        '.cm-cursor': { borderLeftColor: '#222' },
        '.cm-selectionBackground': { background: '#b3d4fc !important' },
        '&.cm-focused .cm-selectionBackground': { background: '#b3d4fc !important' },
        '.cm-gutters': { display: 'none' },
        '.cm-activeLine': { background: 'transparent' },
        '.cm-activeLineGutter': { background: 'transparent' },
      }, { dark: false }),
      EditorView.updateListener.of(update => {
        if (update.docChanged || update.focusChanged) {
          clearTimeout(debounceTimer);
          debounceTimer = setTimeout(() => {
            runUpdate(update.view, topBanner, bottomBanner, issuesList, issuesHeader);
          }, DEBOUNCE_MS);
        }
      }),
    ],
  });

  const view = new EditorView({
    state: startState,
    parent: document.getElementById('editor-mount'),
  });

  // Run initial update
  runUpdate(view, topBanner, bottomBanner, issuesList, issuesHeader);

  // CAB button handler
  btnCab.addEventListener('click', () => {
    const text = view.state.doc.toString();
    let cabText;
    try {
      cabText = generate_cab(text, null, null);
    } catch (e) {
      cabText = 'Error generating CAB.';
    }
    if (cabText) {
      cabContent.textContent = cabText;
      cabContent.classList.remove('cab-placeholder');
    } else {
      // UNCLASSIFIED document — no CAB required.
      cabContent.textContent = 'No Classification Authority Block required for UNCLASSIFIED documents.';
      cabContent.classList.add('cab-placeholder');
    }
  });

  // ---------------------------------------------------------------------------
  // Playground section
  // ---------------------------------------------------------------------------
  const playgroundInput  = document.getElementById('playground-input');
  const playgroundOutput = document.getElementById('playground-output');

  function runPlayground() {
    const text = playgroundInput.value;
    if (!text.trim()) {
      playgroundOutput.textContent = '';
      return;
    }
    try {
      const ndjson = lint(text, null);
      playgroundOutput.textContent = ndjson || '(no diagnostics)';
    } catch (e) {
      playgroundOutput.textContent = `Error: ${e.message || e}`;
    }
  }

  playgroundInput.addEventListener('input', () => {
    clearTimeout(playgroundInput._timer);
    playgroundInput._timer = setTimeout(runPlayground, 150);
  });
}

main().catch(console.error);
