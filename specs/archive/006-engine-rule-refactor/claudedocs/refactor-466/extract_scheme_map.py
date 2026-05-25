#!/usr/bin/env python3
# SPDX-FileCopyrightText: 2026 Adam Poulemanos
#
# SPDX-License-Identifier: MIT OR Apache-2.0
"""
extract_scheme_map.py — structural map of crates/capco/src/scheme.rs for issue #466.

Parses scheme.rs with a hand-rolled brace-depth tracker (depth 0 = top level),
emits per-block metadata as JSON, and assigns each block to a proposed module
in the planned scheme/ submodule layout. Deterministic; stdlib only.

Invariants:
 - String/char literals and /* */ block comments do NOT count toward brace depth.
 - A block's line range starts at the first line of its preceding doc/attr cluster
   and ends at the matching closing brace (or terminating semicolon for `use`,
   `const`, `static`, `type`).
 - `within_file_deps` is a whole-word match against the set of names actually
   defined at top level in this file (excluding self-references).
 - `external_deps` is a deduped sorted list of (crate_or_module, symbol) tuples
   from `use` statements that the block body actually references.

This script does NOT modify scheme.rs.
"""

from __future__ import annotations

import json
import re
import sys
from dataclasses import dataclass, field
from pathlib import Path

# ---------------------------------------------------------------------------
# Configuration
# ---------------------------------------------------------------------------

REPO_ROOT = Path("/home/knitli/marque")
SOURCE_FILE = REPO_ROOT / "crates" / "capco" / "src" / "scheme.rs"
OUT_DIR = REPO_ROOT / "claudedocs" / "refactor-466"
JSON_OUT = OUT_DIR / "scheme_map.json"
MD_OUT = OUT_DIR / "scheme_map.md"
PROPOSAL_OUT = OUT_DIR / "split_proposal.md"

# Maximum LOC per proposed module per issue #466.
LOC_CEILING = 800

# Recommended module layout from issue #466. Used as the starting point;
# the actual assignment in `assign_module` may deviate when the coupling
# graph warrants — those deviations are surfaced in split_proposal.md.
PROPOSED_MODULES = [
    "scheme.rs (proper)",
    "scheme/rewrites.rs",
    "scheme/constraints.rs",
    "scheme/predicates.rs",
    "scheme/actions.rs",
    "scheme/shared.rs",
    "scheme/tests.rs",
]


# ---------------------------------------------------------------------------
# Rust lexer-lite: brace depth + comment/string skip
# ---------------------------------------------------------------------------


@dataclass
class StripResult:
    """Result of stripping comments / strings from one logical line.

    `code` is the line with string contents and comments replaced by spaces
    (length preserved so column counts elsewhere stay aligned, though we
    don't actually use column counts). `state` is the multi-line-comment /
    string state to carry into the next line.
    """

    code: str
    state: str  # "code" | "block_comment" | "raw_string:<hashes>" | "string"


def strip_one_line(line: str, state_in: str) -> StripResult:
    """Strip comments and string contents from one line.

    Brace and bracket tokens inside strings, char literals, // line comments,
    or /* */ block comments MUST NOT contribute to depth tracking.
    """
    out: list[str] = []
    i = 0
    n = len(line)
    state = state_in

    while i < n:
        ch = line[i]

        if state == "block_comment":
            if ch == "*" and i + 1 < n and line[i + 1] == "/":
                out.append("  ")
                i += 2
                state = "code"
                continue
            out.append(" ")
            i += 1
            continue

        if state.startswith("raw_string:"):
            # raw string r#"..."#  — terminator is `"` followed by N `#`.
            hashes = int(state.split(":", 1)[1])
            if ch == '"':
                if all(i + 1 + k < n and line[i + 1 + k] == "#" for k in range(hashes)):
                    out.append(" " * (1 + hashes))
                    i += 1 + hashes
                    state = "code"
                    continue
            out.append(" ")
            i += 1
            continue

        if state == "string":
            if ch == "\\" and i + 1 < n:
                out.append("  ")
                i += 2
                continue
            if ch == '"':
                out.append('"')
                i += 1
                state = "code"
                continue
            out.append(" ")
            i += 1
            continue

        # state == "code"
        if ch == "/" and i + 1 < n and line[i + 1] == "/":
            # line comment — rest of line is a comment.
            out.append(" " * (n - i))
            break
        if ch == "/" and i + 1 < n and line[i + 1] == "*":
            out.append("  ")
            i += 2
            state = "block_comment"
            continue
        if ch == '"':
            out.append('"')
            i += 1
            state = "string"
            continue
        if ch == "r" and i + 1 < n and (line[i + 1] == '"' or line[i + 1] == "#"):
            # raw string. Count leading `#`s before the opening `"`.
            j = i + 1
            hashes = 0
            while j < n and line[j] == "#":
                hashes += 1
                j += 1
            if j < n and line[j] == '"':
                out.append(" " * (j - i + 1))
                i = j + 1
                state = f"raw_string:{hashes}"
                continue
        if ch == "'":
            # Char literal OR lifetime. Distinguishing is tricky but only the
            # char-literal case carries content we must mask. We try to match
            # a char literal first; if it doesn't parse, treat as lifetime
            # (passthrough).
            if i + 2 < n and line[i + 1] == "\\" and i + 3 < n:
                # escaped char like '\n' or '\\' or '\''
                # find the closing quote within ~6 chars to handle '\x7F'
                # and '\u{...}' forms.
                for k in range(i + 2, min(n, i + 12)):
                    if line[k] == "'":
                        out.append("'" + " " * (k - i - 1) + "'")
                        i = k + 1
                        break
                else:
                    # malformed — fall through.
                    out.append(ch)
                    i += 1
                continue
            if i + 2 < n and line[i + 2] == "'":
                # simple char literal 'x'
                out.append("' '")
                i += 3
                continue
            # otherwise lifetime — passthrough
            out.append(ch)
            i += 1
            continue

        out.append(ch)
        i += 1

    return StripResult(code="".join(out), state=state)


# ---------------------------------------------------------------------------
# Block detection
# ---------------------------------------------------------------------------


# Patterns for top-level item KEYWORDS that start a block on a line at depth 0.
# We capture (visibility, keyword) and extract the name in a second step.
TOP_ITEM_RE = re.compile(
    r"^\s*"
    r"(?P<vis>pub(?:\([^)]+\))?\s+)?"
    r"(?P<kw>use|mod|const|static|type|struct|enum|trait|impl|fn|unsafe\s+fn|async\s+fn|unsafe\s+impl)"
    r"\b"
)

# A `use` or `mod ...;` or `const X: T = ...;` etc. ends with `;` on first line
# OR with `}` for `mod foo { ... }`.
NAME_AFTER_KW = re.compile(r"\b(?P<name>[A-Za-z_][A-Za-z_0-9]*)\b")
IMPL_RE = re.compile(r"^\s*(?:unsafe\s+)?impl(?:\s*<[^>]*>)?\s+(?P<rest>.+?)\s*(?:\{|where\b)")
# Top-level macro invocations have NO leading whitespace; macros inside a fn
# body show up indented, and must NOT be misclassified as top-level items.
MACRO_INVOKE_RE = re.compile(r"^(?P<name>[A-Za-z_][A-Za-z_0-9]*)!\s*[\[\(\{]")

DOC_LINE_RE = re.compile(r"^\s*///")
INNER_DOC_LINE_RE = re.compile(r"^\s*//!")
ATTR_LINE_RE = re.compile(r"^\s*#\[")
ATTR_OPEN_RE = re.compile(r"^\s*#\[")  # multi-line attr support
CFG_TEST_MOD_RE = re.compile(r"^\s*#\[cfg\(test\)\]")


@dataclass
class Block:
    id: str
    kind: str
    name: str
    visibility: str
    line_start: int
    line_end: int
    loc: int
    doc: str
    attributes: list[str]
    signature: str
    within_file_deps: list[str] = field(default_factory=list)
    external_deps: list[list[str]] = field(default_factory=list)  # [crate, symbol]
    methods: list[dict] | None = None


# ---------------------------------------------------------------------------
# Parser
# ---------------------------------------------------------------------------


def parse_file(path: Path) -> dict:
    raw = path.read_text(encoding="utf-8")
    lines = raw.splitlines()
    total_lines = len(lines)

    # Pre-strip every line for brace/depth analysis.
    stripped: list[str] = []
    state = "code"
    for line in lines:
        r = strip_one_line(line, state)
        stripped.append(r.code)
        state = r.state

    # Verify our state machine returned to code at EOF.
    assert state == "code", f"trailing tokenizer state: {state!r}"

    # ------------------------------------------------------------------
    # Pass 1: identify top-level blocks (depth 0 → 0 transitions).
    # ------------------------------------------------------------------

    blocks_raw: list[dict] = []
    module_doc_lines: list[str] = []
    imports: list[dict] = []

    depth = 0
    i = 0
    # First: capture module-level //! doc comments.
    while i < total_lines:
        if INNER_DOC_LINE_RE.match(lines[i]):
            module_doc_lines.append(lines[i].lstrip()[3:].lstrip())
            i += 1
        elif lines[i].strip() == "" or lines[i].lstrip().startswith("//"):
            # blank line or // (non-doc) comment between license header and //!
            i += 1
            # stop after we've passed the //! cluster
            if module_doc_lines:
                # peek ahead — if next non-blank is not //!, stop.
                j = i
                while j < total_lines and lines[j].strip() == "":
                    j += 1
                if j < total_lines and not INNER_DOC_LINE_RE.match(lines[j]):
                    break
        else:
            break

    i = 0
    while i < total_lines:
        s = stripped[i]

        if depth == 0:
            # Gather a contiguous doc+attr cluster preceding any item.
            doc_lines: list[str] = []
            attr_lines: list[str] = []

            # Skip leading blank lines between blocks
            while i < total_lines and lines[i].strip() == "":
                i += 1
            cluster_start = i

            # Collect doc-comments and attributes.
            while i < total_lines:
                raw_line = lines[i]
                if DOC_LINE_RE.match(raw_line):
                    doc_lines.append(raw_line.lstrip()[3:].rstrip())
                    i += 1
                    continue
                if ATTR_OPEN_RE.match(raw_line):
                    # Attribute may span multiple lines; consume until `]` at depth 0.
                    attr_start = i
                    bracket_depth = 0
                    started = False
                    while i < total_lines:
                        s_line = stripped[i]
                        for ch in s_line:
                            if ch == "[":
                                bracket_depth += 1
                                started = True
                            elif ch == "]":
                                bracket_depth -= 1
                        i += 1
                        if started and bracket_depth == 0:
                            break
                    attr_lines.append("\n".join(lines[attr_start:i]).strip())
                    continue
                # blank inside the cluster?
                if raw_line.strip() == "":
                    # If the next non-blank is a doc or attr, keep gathering;
                    # otherwise this cluster ends here and the blank is between.
                    j = i
                    while j < total_lines and lines[j].strip() == "":
                        j += 1
                    if j < total_lines and (
                        DOC_LINE_RE.match(lines[j]) or ATTR_OPEN_RE.match(lines[j])
                    ):
                        i = j
                        continue
                    # Blank line — not part of an item cluster. Reset cluster.
                    if not doc_lines and not attr_lines:
                        i += 1
                        cluster_start = i
                        continue
                    # We had a cluster but it isn't followed by an item — drop.
                    doc_lines = []
                    attr_lines = []
                    i = j
                    cluster_start = i
                    continue
                break

            if i >= total_lines:
                break

            s = stripped[i]
            raw_line = lines[i]
            mo = TOP_ITEM_RE.match(raw_line)
            mac = MACRO_INVOKE_RE.match(raw_line)
            if mo is None and mac is None:
                # not a top-level item start — track depth and continue
                for ch in s:
                    if ch == "{":
                        depth += 1
                    elif ch == "}":
                        depth -= 1
                i += 1
                continue

            item_start = cluster_start

            # Decide block kind/name/end-line.
            if mac is not None:
                # macro invocation block (e.g., lazy_static! { ... })
                name = mac.group("name") + "!"
                kind = "macro_invoke"
                end_line = consume_to_balanced(stripped, i, opener=None)
                signature = raw_line.strip()
            else:
                kw = mo.group("kw").strip()

                if kw == "use":
                    name = extract_use_name(raw_line)
                    kind = "use"
                    end_line = consume_to_semicolon(stripped, i)
                    # Join with newlines so single-line // comments inside the
                    # use block don't swallow the rest of the block.
                    signature = "\n".join(
                        lines[i : end_line + 1]
                    ).strip()
                    # Capture import for the imports list.
                    parsed_imports = parse_use_block(
                        "\n".join(lines[i : end_line + 1])
                    )
                    for crate, syms in parsed_imports:
                        imports.append(
                            {"crate": crate, "symbols": syms, "line": i + 1}
                        )
                elif kw in ("const", "static", "type"):
                    nm = NAME_AFTER_KW.search(raw_line[mo.end() :])
                    name = nm.group("name") if nm else "?"
                    kind = kw
                    end_line = consume_to_semicolon(stripped, i)
                    # Keep newlines so multi-line const bodies stay readable.
                    signature = "\n".join(
                        lines[i : end_line + 1]
                    ).strip()
                elif kw == "mod":
                    nm = NAME_AFTER_KW.search(raw_line[mo.end() :])
                    name = nm.group("name") if nm else "?"
                    kind = "mod"
                    # mod foo;  OR mod foo { ... }
                    if ";" in stripped[i]:
                        end_line = consume_to_semicolon(stripped, i)
                    else:
                        end_line = consume_to_balanced(stripped, i, opener="{")
                    signature = lines[i].strip()
                elif kw == "impl" or kw == "unsafe impl":
                    impl_m = IMPL_RE.match(raw_line)
                    if impl_m:
                        rest = impl_m.group("rest").strip()
                    else:
                        rest = raw_line.strip()
                    name = "impl " + rest
                    kind = "impl"
                    end_line = consume_to_balanced(stripped, i, opener="{")
                    signature = name
                elif kw in ("struct", "enum", "trait"):
                    nm = NAME_AFTER_KW.search(raw_line[mo.end() :])
                    name = nm.group("name") if nm else "?"
                    kind = kw
                    if kw == "struct":
                        # struct body can be:
                        #   `{...}`            normal struct
                        #   `(...);`           tuple struct (multi-line ok)
                        #   `;`                unit struct
                        # We scan forward to find the first of `{` `(` `;` at depth 0.
                        end_line = consume_struct_body(stripped, i)
                    else:
                        # enum, trait — always `{...}` at depth 0.
                        end_line = consume_to_balanced(stripped, i, opener=None)
                    signature = lines[i].strip()
                elif kw.endswith("fn"):
                    nm = NAME_AFTER_KW.search(raw_line[mo.end() :])
                    name = nm.group("name") if nm else "?"
                    kind = "fn"
                    # signature ends at the `{` (or `;` for trait fn decls)
                    end_line = consume_to_balanced_or_semicolon(stripped, i)
                    # Build a one-line signature: first line up to `{`
                    sig_text = []
                    for k in range(i, min(end_line + 1, total_lines)):
                        sig_text.append(lines[k].strip())
                        if "{" in stripped[k] or ";" in stripped[k]:
                            break
                    signature = " ".join(sig_text)
                    # Pull `{` index for sig truncation
                    if "{" in signature:
                        signature = signature.split("{", 1)[0].strip() + " { ... }"
                    elif ";" in signature:
                        signature = signature.rstrip(";").strip() + ";"
                else:
                    # Unknown keyword — skip
                    end_line = i
                    name = "?"
                    kind = "unknown"
                    signature = raw_line.strip()

                if (mo.group("vis") or "").strip().startswith("pub"):
                    visibility = "pub" if (mo.group("vis") or "").strip() == "pub" else mo.group("vis").strip()
                else:
                    visibility = "private"

            block_id = f"{kind}:{name}"
            doc_text = "\n".join(doc_lines).strip()

            blocks_raw.append(
                {
                    "id": block_id,
                    "kind": kind,
                    "name": name,
                    "visibility": visibility,
                    "line_start": item_start + 1,  # 1-based
                    "line_end": end_line + 1,  # 1-based, inclusive
                    "doc": doc_text,
                    "attributes": attr_lines,
                    "signature": signature,
                }
            )

            # Advance past this block at depth 0.
            i = end_line + 1
            # Reset depth tracking to 0 — we've consumed the whole block.
            depth = 0
            continue

        # depth > 0 — shouldn't happen at top level loop entry, but be safe.
        for ch in s:
            if ch == "{":
                depth += 1
            elif ch == "}":
                depth -= 1
        i += 1

    # ------------------------------------------------------------------
    # Pass 2: deduplicate impl block IDs so JSON keys stay unique.
    # ------------------------------------------------------------------
    seen_ids: dict[str, int] = {}
    for b in blocks_raw:
        key = b["id"]
        if key in seen_ids:
            seen_ids[key] += 1
            b["id"] = f"{key}#{seen_ids[key]}"
        else:
            seen_ids[key] = 1

    # ------------------------------------------------------------------
    # Pass 3: enumerate impl-block methods.
    # ------------------------------------------------------------------
    for b in blocks_raw:
        if b["kind"] != "impl":
            continue
        b_methods = extract_impl_methods(
            lines, stripped, b["line_start"] - 1, b["line_end"] - 1
        )
        b["methods"] = b_methods

    # ------------------------------------------------------------------
    # Pass 4: build the set of "names defined in this file" for dep tracking.
    # ------------------------------------------------------------------
    file_names: set[str] = set()
    for b in blocks_raw:
        if "methods" not in b:
            b["methods"] = None
        if b["kind"] in ("fn", "const", "static", "type", "struct", "enum", "trait", "mod"):
            if b["name"] and b["name"] != "?":
                file_names.add(b["name"])
        # Intentionally do NOT add impl-method names to file_names — methods
        # are called via `.method()` syntax and our word-boundary scan can't
        # distinguish a real intra-file reference to a free fn `foo` from a
        # method call `something.foo()`. Including method names produces
        # noisy outbound-coupling reports without informational gain.

    # ------------------------------------------------------------------
    # Pass 5: build import lookup table — symbol → crate root.
    # ------------------------------------------------------------------
    import_table: dict[str, str] = {}
    for imp in imports:
        for sym in imp["symbols"]:
            # If symbol is renamed via `as`, the local name is after `as`.
            # parse_use_block already normalized.
            import_table[sym] = imp["crate"]

    # ------------------------------------------------------------------
    # Pass 6: compute deps per block.
    # ------------------------------------------------------------------
    for b in blocks_raw:
        body_lines = lines[b["line_start"] - 1 : b["line_end"]]
        # Strip the doc/attr cluster from body for dep scanning — we only
        # care about code references.
        body = "\n".join(body_lines)
        # Strip strings and comments cheaply.
        body_cleaned = strip_strings_and_comments(body)

        words = set(re.findall(r"\b([A-Za-z_][A-Za-z_0-9]*)\b", body_cleaned))
        # Within-file deps: words that match file_names, excluding self.
        wf = sorted(w for w in words if w in file_names and w != b["name"])
        b["within_file_deps"] = wf

        # External deps: imported symbols referenced in this body.
        ext: set[tuple[str, str]] = set()
        for sym, crate in import_table.items():
            if sym in words:
                ext.add((crate, sym))
        b["external_deps"] = sorted([list(p) for p in ext])

    # ------------------------------------------------------------------
    # Pass 7: compute LOC and total coverage.
    # ------------------------------------------------------------------
    for b in blocks_raw:
        b["loc"] = b["line_end"] - b["line_start"] + 1

    return {
        "file": str(SOURCE_FILE.relative_to(REPO_ROOT)),
        "total_lines": total_lines,
        "module_doc": "\n".join(module_doc_lines).strip(),
        "imports": imports,
        "blocks": blocks_raw,
        "file_names": sorted(file_names),
    }


# ---------------------------------------------------------------------------
# Helpers — brace/depth navigation
# ---------------------------------------------------------------------------


def consume_to_semicolon(stripped: list[str], start: int) -> int:
    """Return the line index (0-based) on which the first `;` at depth 0 occurs."""
    depth = 0
    for k in range(start, len(stripped)):
        for ch in stripped[k]:
            if ch == "{":
                depth += 1
            elif ch == "}":
                depth -= 1
            elif ch == ";" and depth == 0:
                return k
    return len(stripped) - 1


def consume_to_balanced(stripped: list[str], start: int, opener: str | None) -> int:
    """Return the line index of the matching closing brace.

    `opener`: if "{", we expect the first `{` to be on or after `start`. If None,
    we scan for the first `{` then match.
    """
    depth = 0
    started = False
    for k in range(start, len(stripped)):
        for ch in stripped[k]:
            if ch == "{":
                depth += 1
                started = True
            elif ch == "}":
                depth -= 1
                if started and depth == 0:
                    return k
    return len(stripped) - 1


def consume_struct_body(stripped: list[str], start: int) -> int:
    """Determine the end line of a `struct` item.

    Handles three shapes:
      - `struct Foo;`              unit struct — ends at `;` on `start`.
      - `struct Foo { ... }`       brace body — balance the braces.
      - `struct Foo(...);`         tuple struct — balance the parens, then
                                   require a trailing `;` (possibly on a
                                   later line).

    Important: `pub(crate)` / `pub(super)` etc. contain a paren that MUST
    NOT be mistaken for the start of a tuple-struct body. We skip past the
    `struct` keyword and the struct name before scanning for the opener.
    Generic parameter lists `<T: Bound>` also live between the name and the
    body opener — we treat any `<...>` cluster as opaque by tracking angle
    depth at the same time.
    """
    paren_depth = 0
    brace_depth = 0
    angle_depth = 0
    started = None  # "brace" | "paren" | None
    seen_struct_kw = False
    seen_name = False
    for k in range(start, len(stripped)):
        s = stripped[k]
        # Use a regex to fast-forward past `struct` keyword + name on the
        # first line where they appear.
        if not seen_struct_kw:
            m = re.search(r"\bstruct\b", s)
            if m:
                seen_struct_kw = True
                # Skip past the struct keyword.
                offset = m.end()
                s_after = s[offset:]
                # Skip whitespace, then the struct name identifier.
                rest_m = re.match(r"\s*([A-Za-z_][A-Za-z_0-9]*)", s_after)
                if rest_m:
                    seen_name = True
                    s = s_after[rest_m.end() :]
                else:
                    s = s_after
            else:
                continue

        for ch in s:
            if not seen_name:
                # Still scanning for the struct name on a continuation line.
                # Names can't start with `(`, `{`, `;`, `<`. The next identifier
                # token is the name. Cheapest fix: skip non-name chars until
                # an alnum/_ char appears.
                if ch.isalpha() or ch == "_":
                    seen_name = True
                continue

            if started is None:
                if ch == "<":
                    angle_depth += 1
                    continue
                if ch == ">":
                    if angle_depth > 0:
                        angle_depth -= 1
                    continue
                if angle_depth > 0:
                    continue
                if ch == "{":
                    brace_depth = 1
                    started = "brace"
                elif ch == "(":
                    paren_depth = 1
                    started = "paren"
                elif ch == ";":
                    return k  # unit struct or end of `pub struct Foo;`
            elif started == "brace":
                if ch == "{":
                    brace_depth += 1
                elif ch == "}":
                    brace_depth -= 1
                    if brace_depth == 0:
                        return k
            elif started == "paren":
                if ch == "(":
                    paren_depth += 1
                elif ch == ")":
                    paren_depth -= 1
                elif ch == ";" and paren_depth == 0:
                    return k
    return len(stripped) - 1


def consume_to_balanced_or_semicolon(stripped: list[str], start: int) -> int:
    """For fn: either `{ ... }` body or `;` (trait decl). Whichever comes first
    at depth 0 wins, but we scan for the `{` and balance it once found.
    """
    depth = 0
    for k in range(start, len(stripped)):
        for ch in stripped[k]:
            if ch == "{":
                depth += 1
                # found body — switch to balance mode
                return consume_to_balanced(stripped, k, opener="{")
            elif ch == ";" and depth == 0:
                return k
    return len(stripped) - 1


def extract_use_name(line: str) -> str:
    """`use foo::bar::{...};` → `foo::bar` for the block id."""
    m = re.match(r"\s*use\s+(?P<path>[^;{]+)", line)
    if not m:
        return "use"
    path = m.group("path").strip()
    # If it's `foo::bar::{...`, return the prefix before `{`.
    if "{" in path:
        path = path.split("{", 1)[0].rstrip(":").rstrip()
    return path.rstrip(";").strip()


def parse_use_block(text: str) -> list[tuple[str, list[str]]]:
    """Parse a `use crate::path::{Sym, Other};` block into [(crate, [Sym,Other])].

    Returns the FIRST path segment as the crate, and the leaf symbol names.
    Handles `as` renames (`Foo as Bar` → local name `Bar`), `self`, nested braces,
    and multi-line layouts.
    """
    # Strip `use ` prefix and trailing `;`.
    text = text.strip()
    if text.startswith("pub "):
        text = text[len("pub ") :].strip()
    if text.startswith("use "):
        text = text[len("use ") :].strip()
    if text.endswith(";"):
        text = text[:-1].strip()

    if "{" not in text:
        # Simple `use foo::bar::Baz;` or `use foo::bar::*;`
        parts = text.split("::")
        crate = parts[0].strip() if parts else "?"
        leaf = parts[-1].strip() if parts else "?"
        if " as " in leaf:
            leaf = leaf.split(" as ", 1)[1].strip()
        if leaf == "*":
            return [(crate, [])]
        return [(crate, [leaf])]

    # Has `{ ... }`. Find prefix before `{`.
    brace_pos = text.find("{")
    prefix = text[:brace_pos].rstrip(":").rstrip()
    body = text[brace_pos + 1 :]
    # Trim trailing `}` only if balanced.
    # We accept the simple case: one outer `{ ... }`.
    if body.endswith("}"):
        body = body[:-1]

    parts = prefix.split("::") if prefix else []
    crate = parts[0].strip() if parts else "?"

    symbols: list[str] = []
    # Split on commas at brace depth 0 within the body.
    depth = 0
    buf: list[str] = []
    pieces: list[str] = []
    for ch in body:
        if ch == "{":
            depth += 1
            buf.append(ch)
        elif ch == "}":
            depth -= 1
            buf.append(ch)
        elif ch == "," and depth == 0:
            pieces.append("".join(buf).strip())
            buf = []
        else:
            buf.append(ch)
    if buf:
        pieces.append("".join(buf).strip())

    for piece in pieces:
        if not piece:
            continue
        # Strip line comments inside multi-line use block (already stripped
        # at line-level, but defensive).
        piece = re.sub(r"//.*$", "", piece, flags=re.MULTILINE).strip()
        if not piece:
            continue
        # `Self` / `self` references the prefix itself; skip.
        if piece in ("self", "Self"):
            if parts:
                symbols.append(parts[-1].strip())
            continue
        if "{" in piece:
            # nested — recurse into the nested set
            inner_brace = piece.find("{")
            inner_prefix = piece[:inner_brace].rstrip(":").rstrip()
            inner_body = piece[inner_brace:]
            inner_text = f"use {prefix}::{inner_prefix}::{inner_body};"
            for c, syms in parse_use_block(inner_text):
                symbols.extend(syms)
            continue
        # `Foo as Bar`
        if " as " in piece:
            piece = piece.split(" as ", 1)[1].strip()
        # `path::Leaf` — take leaf
        if "::" in piece:
            piece = piece.rsplit("::", 1)[1].strip()
        if piece and piece != "*":
            symbols.append(piece)

    return [(crate, symbols)]


def extract_impl_methods(
    lines: list[str], stripped: list[str], i_start: int, i_end: int
) -> list[dict]:
    """Enumerate `fn` items at the inner-depth-1 level of an `impl { ... }` block."""
    methods: list[dict] = []
    depth = 0
    i = i_start
    impl_brace_found = False
    while i <= i_end:
        s = stripped[i]
        # Compute depth *before* applying this line's contribution. A method
        # signature line `    pub fn foo() -> Bar {` opens a new brace; we must
        # match it at the depth it's nested in (1 inside an impl), not at the
        # depth after the `{` opens (2).
        depth_before = depth
        for ch in s:
            if ch == "{":
                depth += 1
                impl_brace_found = True
            elif ch == "}":
                depth -= 1

        # We want fns at the impl's inner level: depth_before == 1.
        if impl_brace_found and depth_before == 1:
            raw = lines[i]
            # Match a method: optional pub, optional async/unsafe, fn NAME
            mm = re.match(
                r"^\s*(?P<doc_or_attr>)?"
                r"(?P<vis>pub(?:\([^)]+\))?\s+)?"
                r"(?:(?:unsafe|async|const|default)\s+)*"
                r"fn\s+(?P<name>[A-Za-z_][A-Za-z_0-9]*)",
                raw,
            )
            if mm and raw.lstrip().startswith(
                ("pub fn", "pub(", "fn ", "unsafe ", "async ", "const fn", "default ")
            ):
                # Gather preceding doc cluster.
                doc_lines: list[str] = []
                j = i - 1
                while j >= i_start:
                    line_j = lines[j]
                    if DOC_LINE_RE.match(line_j):
                        doc_lines.insert(0, line_j.lstrip()[3:].rstrip())
                        j -= 1
                    elif ATTR_OPEN_RE.match(line_j):
                        j -= 1
                    elif line_j.strip() == "":
                        if doc_lines:
                            break
                        j -= 1
                    else:
                        break

                # Find method end: balanced `{ ... }` or `;` (trait decl style).
                end = consume_to_balanced_or_semicolon(stripped, i)
                # Build signature
                sig_text = []
                for k in range(i, min(end + 1, len(lines))):
                    sig_text.append(lines[k].strip())
                    if "{" in stripped[k] or ";" in stripped[k]:
                        break
                signature = " ".join(sig_text)
                if "{" in signature:
                    signature = signature.split("{", 1)[0].strip() + " { ... }"
                elif ";" in signature:
                    signature = signature.rstrip(";").strip() + ";"

                methods.append(
                    {
                        "name": mm.group("name"),
                        "visibility": (mm.group("vis") or "private").strip() or "private",
                        "line_start": i + 1,
                        "line_end": end + 1,
                        "loc": end - i + 1,
                        "doc": "\n".join(doc_lines).strip(),
                        "signature": signature,
                    }
                )
                i = end + 1
                # Recompute depth after jump.
                depth = 0
                # Re-scan stripped lines from i_start up to end inclusive to
                # restore depth. This is robust but quadratic worst-case; the
                # typical impl has only a handful of methods so it's fine.
                for k in range(i_start, min(i, i_end + 1)):
                    for ch2 in stripped[k]:
                        if ch2 == "{":
                            depth += 1
                        elif ch2 == "}":
                            depth -= 1
                continue
        i += 1
    return methods


def strip_strings_and_comments(text: str) -> str:
    """One-shot strip for dep scanning. Faster than the line-by-line walker."""
    out: list[str] = []
    state = "code"
    i = 0
    n = len(text)
    while i < n:
        ch = text[i]
        if state == "code":
            if ch == "/" and i + 1 < n and text[i + 1] == "/":
                # to end of line
                nl = text.find("\n", i)
                if nl == -1:
                    break
                out.append("\n")
                i = nl + 1
                continue
            if ch == "/" and i + 1 < n and text[i + 1] == "*":
                state = "block"
                i += 2
                continue
            if ch == '"':
                state = "string"
                out.append(" ")
                i += 1
                continue
            if ch == "'":
                # char or lifetime — handled cheaply: skip 'x' or '\x' patterns
                # If next char is alnum + _ followed by NOT ', it's a lifetime.
                if i + 1 < n and text[i + 1] != "\\" and i + 2 < n and text[i + 2] == "'":
                    out.append("' '")
                    i += 3
                    continue
                if i + 1 < n and text[i + 1] == "\\":
                    end = text.find("'", i + 2)
                    if end != -1 and end - i < 10:
                        out.append("' '")
                        i = end + 1
                        continue
                # lifetime — passthrough one char
                out.append(ch)
                i += 1
                continue
            out.append(ch)
            i += 1
        elif state == "string":
            if ch == "\\" and i + 1 < n:
                i += 2
                continue
            if ch == '"':
                state = "code"
                out.append(" ")
                i += 1
                continue
            i += 1
        elif state == "block":
            if ch == "*" and i + 1 < n and text[i + 1] == "/":
                state = "code"
                i += 2
                continue
            i += 1
    return "".join(out)


# ---------------------------------------------------------------------------
# Module assignment
# ---------------------------------------------------------------------------


def assign_module(block: dict) -> str:
    """Decide which proposed module a block belongs to.

    Heuristics — applied in order:
      1. `cfg(test)` mod tests → scheme/tests.rs
      2. `use`, top-level `const`/`static` for category/token ids and metadata,
         the `CapcoMarking` struct + its core impls, the `CapcoScheme` struct
         + `Debug`/`Default` impls + `MarkingScheme` trait impl, plus the
         struct/enum definitions → scheme.rs (proper)
      3. Functions / impl methods named `build_page_rewrites`, helpers that
         construct PageRewrite rows, and `merge_fgi_markers` /
         `extract_foreign_sources` (rewrite-payload helpers) →
         scheme/rewrites.rs
      4. `build_constraints` + helpers referenced only from it →
         scheme/constraints.rs
      5. Top-level free functions whose name pattern suggests predicate
         (`*_trigger`, `is_*`, `has_*`, `dissem_*`, `never_fires`,
         `evaluate_custom_by_attrs`, `satisfies_attrs`) →
         scheme/predicates.rs
      6. Top-level free functions whose name pattern suggests action
         (`noop_action`, `apply_*`, `capco_category_*`) →
         scheme/actions.rs
      7. Anything else (shared / fallthrough) → scheme/shared.rs

    The classifier in #466 is offered as a recommendation. We deviate by
    spinning `scheme/shared.rs` for blocks that don't fit and by routing
    `CompanionForm` / open-vocab helpers there. The split proposal flags
    every block whose category is ambiguous.
    """
    name = block["name"]
    kind = block["kind"]
    attrs = " ".join(block["attributes"])

    if kind == "mod" and "tests" in name and "cfg(test)" in attrs:
        return "scheme/tests.rs"
    if kind == "mod" and name == "tests":
        return "scheme/tests.rs"

    # use statements
    if kind == "use":
        return "scheme.rs (proper)"

    # Top-level const/static for category / token / sentinel ids and metadata.
    if kind in ("const", "static"):
        return "scheme.rs (proper)"

    # Struct / enum definitions for the scheme.
    if kind in ("struct", "enum", "trait", "type"):
        return "scheme.rs (proper)"

    # impl blocks: where does the impl belong?
    if kind == "impl":
        n = name
        # The MarkingScheme trait impl is the surface — keep with the scheme.
        if "MarkingScheme" in n or "Default" in n or "Debug" in n or "PartialEq" in n or "Eq for" in n or "From<" in n or "Lattice" in n:
            return "scheme.rs (proper)"
        # CompanionForm impl
        if "CompanionForm" in n:
            return "scheme/shared.rs"
        # impl CapcoMarking { ... } — small helper impls
        if "impl CapcoMarking" == n:
            return "scheme.rs (proper)"
        # impl CapcoScheme { ... } — inherent impl. Multiple inherent impls per
        # type are legal in Rust. The current file has THREE such blocks:
        # (a) the small `Default` shim, (b) a 2k+ LOC builder block hosting
        # new() + build_page_rewrites + build_categories + build_constraints,
        # (c) helpers ~lines 4829-5165 (fix_intent_by_name, bridges), and
        # (d) a tiny test-only block (with_rewrites, with_extra_rewrite_for_tests).
        # We classify the impl SHELL by which destination its dominant method
        # belongs to; the proposal explains the cleaner shape (option 2: lift
        # builders to pub(crate) free fns).
        if n == "impl CapcoScheme":
            # Inspect the block's methods (if computed yet by the caller).
            meths = block.get("methods") or []
            if any(m["name"] == "build_page_rewrites" for m in meths):
                return "scheme/rewrites.rs"
            if any(m["name"] == "build_constraints" for m in meths):
                return "scheme/constraints.rs"
            if any(m["name"] == "fix_intent_by_name" for m in meths):
                return "scheme.rs (proper)"
            if any(m["name"] == "with_rewrites" for m in meths):
                return "scheme/tests.rs"
            return "scheme.rs (proper)"
        return "scheme/shared.rs"

    # Free functions
    if kind == "fn":
        n = name

        # ACTIONS: any fn whose name advertises a write/replace/clear/strip/apply/emit.
        action_exact = {
            "noop_action",
            "extract_foreign_sources",
            "merge_fgi_markers",
            "apply_intent_to_marking",
            "apply_fact_add",
            "apply_fact_remove",
            "capco_category_contains",
            "capco_category_has_values",
            "capco_category_clear",
            "capco_category_replace",
            "page_context_to_attrs",
            "strip_dod_ucni_action",
            "strip_doe_ucni_action",
        }
        if n in action_exact:
            return "scheme/actions.rs"
        action_prefixes = ("emit_", "strip_", "apply_")
        if any(n.startswith(p) for p in action_prefixes):
            return "scheme/actions.rs"

        # PREDICATES (read-only, return bool/option/answer).
        predicate_exact = {
            "satisfies_attrs",
            "evaluate_custom_by_attrs",
            "anchors_on",
            "rel_to_covers",
            "joint_requires_usa",
            "compartment_has_sub",
            "first_sci_span",
            "us_level",
            "last_dissem_span",
            "infer_companion_form",
            "first_span_of_optional",
            "class_floor_row_by_name",
            "class_floor_satisfied",
            "class_floor_anchor_span",
            "class_floor_catalog_eval",
            "sci_per_system_row_by_name",
            "sci_per_system_catalog_eval",
            "capco_token_category",
            "collect_present_tokens",
            "hcs_system_constraints",
        }
        if n in predicate_exact:
            return "scheme/predicates.rs"
        predicate_prefixes = (
            "is_",
            "has_",
            "dissem_",
            "presence_",
        )
        predicate_suffixes = (
            "_trigger",
            "_satisfied",
            "_covers",
            "_requires_usa",
        )
        if any(n.startswith(p) for p in predicate_prefixes):
            return "scheme/predicates.rs"
        if any(n.endswith(s) for s in predicate_suffixes):
            return "scheme/predicates.rs"
        if n in ("never_fires",):
            return "scheme/predicates.rs"

        # CONSTRAINT-rule emitters: free fns named after a rule ID (E012, W002,
        # S004, etc.) are the diagnostic-emit bodies; they belong in constraints.
        if re.match(r"^[eEwWsScC]\d{3}_", n):
            return "scheme/constraints.rs"
        if n.startswith("class_floor_") or n.startswith("sci_per_system_"):
            return "scheme/constraints.rs"

        # Unknown free fn — shared.
        return "scheme/shared.rs"

    if kind == "macro_invoke":
        return "scheme/shared.rs"

    return "scheme/shared.rs"


# ---------------------------------------------------------------------------
# Report rendering
# ---------------------------------------------------------------------------


def doc_one_liner(doc: str) -> str:
    if not doc:
        return ""
    for line in doc.splitlines():
        line = line.strip()
        if line:
            return line
    return ""


def render_markdown_report(data: dict) -> str:
    blocks = data["blocks"]
    for b in blocks:
        b["proposed_module"] = assign_module(b)

    # Group by proposed module.
    grouped: dict[str, list[dict]] = {m: [] for m in PROPOSED_MODULES}
    for b in blocks:
        grouped.setdefault(b["proposed_module"], []).append(b)

    # Build name → module index for coupling analysis.
    name_to_module: dict[str, str] = {}
    for m, bs in grouped.items():
        for b in bs:
            if b["name"] and b["name"] != "?":
                name_to_module[b["name"]] = m
            if b.get("methods"):
                for meth in b["methods"]:
                    if meth["name"]:
                        name_to_module.setdefault(meth["name"], m)

    out: list[str] = []
    out.append("<!--")
    out.append("SPDX-FileCopyrightText: 2026 Adam Poulemanos")
    out.append("SPDX-License-Identifier: MIT OR Apache-2.0")
    out.append("-->")
    out.append("")
    out.append("# scheme.rs structural map — refactor #466")
    out.append("")
    out.append(f"Source: `{data['file']}` — {data['total_lines']} lines, {len(blocks)} top-level blocks.")
    out.append("")
    out.append("Generated by `extract_scheme_map.py`. Re-run to refresh.")
    out.append("")

    # Per-module summary table.
    out.append("## Per-module summary")
    out.append("")
    out.append("| Proposed module | Block count | LOC sum | Over 800-LOC ceiling? |")
    out.append("|---|---:|---:|---|")
    for m in PROPOSED_MODULES:
        bs = grouped.get(m, [])
        loc_sum = sum(b["loc"] for b in bs)
        over = "**YES**" if loc_sum > LOC_CEILING else "no"
        out.append(f"| `{m}` | {len(bs)} | {loc_sum} | {over} |")
    out.append("")

    # Per-module detail sections.
    for m in PROPOSED_MODULES:
        bs = grouped.get(m, [])
        if not bs:
            continue
        loc_sum = sum(b["loc"] for b in bs)
        out.append(f"## `{m}` — {loc_sum} LOC, {len(bs)} blocks")
        out.append("")

        # Block table.
        out.append("| Name | Kind | Lines | LOC | Doc (one-liner) |")
        out.append("|---|---|---|---:|---|")
        for b in sorted(bs, key=lambda x: x["line_start"]):
            doc1 = doc_one_liner(b["doc"]).replace("|", "\\|")
            out.append(
                f"| `{b['name']}` | {b['kind']} | {b['line_start']}–{b['line_end']} | {b['loc']} | {doc1} |"
            )
        out.append("")

        # Inbound coupling: which OTHER modules call into names defined here.
        names_here = {b["name"] for b in bs if b["name"] and b["name"] != "?"}
        for b in bs:
            if b.get("methods"):
                for meth in b["methods"]:
                    if meth["name"]:
                        names_here.add(meth["name"])

        inbound: dict[str, set[str]] = {}
        for ob in blocks:
            if ob["proposed_module"] == m:
                continue
            shared = set(ob["within_file_deps"]) & names_here
            if shared:
                inbound.setdefault(ob["proposed_module"], set()).update(shared)

        outbound: dict[str, set[str]] = {}
        for b in bs:
            for dep in b["within_file_deps"]:
                dep_mod = name_to_module.get(dep)
                if dep_mod and dep_mod != m:
                    outbound.setdefault(dep_mod, set()).add(dep)

        out.append("### Inbound coupling")
        if not inbound:
            out.append("")
            out.append("None — nothing in other modules calls into this module.")
        else:
            out.append("")
            for mod, syms in sorted(inbound.items()):
                out.append(f"- from `{mod}`: " + ", ".join(f"`{s}`" for s in sorted(syms)))
        out.append("")
        out.append("### Outbound coupling")
        if not outbound:
            out.append("")
            out.append("None — this module is self-contained.")
        else:
            out.append("")
            for mod, syms in sorted(outbound.items()):
                out.append(f"- into `{mod}`: " + ", ".join(f"`{s}`" for s in sorted(syms)))
        out.append("")

    # Module-level doc.
    if data["module_doc"]:
        out.append("## Source module doc (verbatim)")
        out.append("")
        out.append("```text")
        out.append(data["module_doc"])
        out.append("```")
        out.append("")

    return "\n".join(out) + "\n"


def assign_method_module(method_name: str) -> str:
    """Where would this impl-method live if lifted to a free pub(crate) fn?"""
    n = method_name
    if n in ("new", "with_rewrites", "with_extra_rewrite_for_tests"):
        return "scheme.rs (proper)"
    if n == "build_page_rewrites":
        return "scheme/rewrites.rs"
    if n == "build_constraints":
        return "scheme/constraints.rs"
    if n == "build_categories":
        return "scheme.rs (proper)"
    if n == "fix_intent_by_name":
        return "scheme/constraints.rs"
    if n.startswith("bridge_") or n == "evaluate_named_constraint" or n == "has_diagnostic_constraints":
        return "scheme/constraints.rs"
    # Trait methods on `impl MarkingScheme for CapcoScheme` — these can't be
    # moved out (they implement a trait), but they CAN delegate to free fns.
    if n in ("apply_intent", "evaluate_custom", "render_canonical", "iter_present_tokens",
             "name", "schema_version", "categories", "constraints", "templates",
             "parse", "satisfies", "category_of"):
        return "scheme.rs (proper)"
    return "scheme.rs (proper)"


def render_split_proposal(data: dict) -> str:
    blocks = data["blocks"]
    for b in blocks:
        if "proposed_module" not in b:
            b["proposed_module"] = assign_module(b)

    grouped: dict[str, list[dict]] = {m: [] for m in PROPOSED_MODULES}
    for b in blocks:
        grouped.setdefault(b["proposed_module"], []).append(b)

    # Compute "option 2 projection": each impl method counted against its own
    # destination module (as if lifted to pub(crate) fn). The impl shell drops
    # to just the trait/inherent surface (no large bodies).
    option2_loc: dict[str, int] = {m: 0 for m in PROPOSED_MODULES}
    for b in blocks:
        if b["kind"] == "impl" and b.get("methods"):
            # Count the impl's methods individually.
            for m in b["methods"]:
                option2_loc[assign_method_module(m["name"])] += m["loc"]
            # Plus a small per-impl shell overhead (the `impl X { ... }` lines).
            # Estimate: 2 lines for opening/closing braces + space between methods.
            shell = max(2, b["loc"] - sum(m["loc"] for m in b["methods"]))
            option2_loc[b["proposed_module"]] += shell
        else:
            option2_loc[b["proposed_module"]] += b["loc"]

    out: list[str] = []
    out.append("<!--")
    out.append("SPDX-FileCopyrightText: 2026 Adam Poulemanos")
    out.append("SPDX-License-Identifier: MIT OR Apache-2.0")
    out.append("-->")
    out.append("")
    out.append("# scheme.rs split proposal — issue #466")
    out.append("")
    out.append(
        "This is a *draft* split. The actual extraction is a follow-up PR. "
        "The goal here is to surface where the recommended layout from #466 "
        "(rewrites / constraints / predicates / actions / scheme proper) holds, "
        "and where the coupling graph contradicts it."
    )
    out.append("")

    # LOC vs ceiling assessment.
    out.append("## LOC targets vs. 800-line ceiling")
    out.append("")
    out.append(
        "Two columns: **as-routed** counts every impl block whole "
        "(charged to whichever module the impl shell goes to); "
        "**post option-2** projects what the LOC distribution would look "
        "like if the giant `impl CapcoScheme` were broken into "
        "free `pub(crate) fn` builders living in their respective modules."
    )
    out.append("")
    out.append("| Module | As-routed LOC | vs. ceiling | Post option-2 LOC | vs. ceiling | Assessment |")
    out.append("|---|---:|---|---:|---|---|")
    for m in PROPOSED_MODULES:
        bs = grouped.get(m, [])
        loc_sum = sum(b["loc"] for b in bs)
        o2 = option2_loc.get(m, 0)
        if loc_sum == 0 and o2 == 0:
            assess = "empty under current heuristic — fold into a sibling"
            ratio = "—"
            o2_ratio = "—"
        else:
            ratio = f"{loc_sum / LOC_CEILING:.1f}×" if loc_sum > LOC_CEILING else f"{loc_sum / LOC_CEILING:.0%}"
            o2_ratio = f"{o2 / LOC_CEILING:.1f}×" if o2 > LOC_CEILING else f"{o2 / LOC_CEILING:.0%}"
            if o2 > LOC_CEILING:
                assess = "**over ceiling even after option-2 lift — needs sub-split**"
            elif o2 > LOC_CEILING * 0.85:
                assess = "near ceiling post-lift, no growth headroom"
            else:
                assess = "fits cleanly after option-2 lift"
        out.append(f"| `{m}` | {loc_sum} | {ratio} | {o2} | {o2_ratio} | {assess} |")
    out.append("")

    # Risks & deviations.
    out.append("## Risks and deviations from #466's recommended layout")
    out.append("")

    # The biggest known-risk: the giant `impl CapcoScheme` block hosts
    # build_page_rewrites + build_constraints + build_categories in one
    # syntactic shell. We can't put one `impl CapcoScheme` block in two
    # files; the proposal must address this.
    out.append("### Risk 1: a single `impl CapcoScheme` block hosts multiple build-* methods")
    out.append("")
    big_impls = [b for b in blocks if b["kind"] == "impl" and b["name"] == "impl CapcoScheme"]
    for b in big_impls:
        meths = b.get("methods") or []
        out.append(
            f"- `impl CapcoScheme` at lines {b['line_start']}–{b['line_end']} "
            f"({b['loc']} LOC) hosts: " +
            ", ".join(f"`{m['name']}` ({m['loc']} LOC)" for m in meths)
        )
    out.append("")
    out.append(
        "Rust doesn't allow splitting one `impl Foo { ... }` block across files. "
        "Two clean fixes:"
    )
    out.append("")
    out.append(
        "1. **Split into multiple `impl CapcoScheme` blocks** (Rust allows multiple "
        "inherent impl blocks per type) — one block per file. Each helper method "
        "becomes `pub(crate)` in its sibling module if cross-block calls exist; "
        "`new()` lives in `scheme.rs` proper and references the per-file builders."
    )
    out.append("")
    out.append(
        "2. **Lift builders out of `impl CapcoScheme` entirely** — make "
        "`build_page_rewrites()`, `build_constraints()`, `build_categories()` "
        "free `pub(crate) fn` items in their respective modules, and have "
        "`CapcoScheme::new()` call them as free functions. This is the cleaner "
        "shape for a refactor focused on file-size discipline."
    )
    out.append("")
    out.append(
        "Recommendation: option 2. It makes the file boundary structural (each "
        "module owns its own builder) instead of cosmetic (each module owns part "
        "of one impl block)."
    )
    out.append("")

    # Risk 2: hard-case blocks.
    out.append("### Risk 2: hard-case blocks (no clean home)")
    out.append("")
    hard_cases: list[tuple[dict, str, set[str]]] = []
    name_to_module = {b["name"]: b["proposed_module"] for b in blocks}
    # Methods also resolve names — index them too.
    for b in blocks:
        if b.get("methods"):
            for meth in b["methods"]:
                # Methods route based on assign_method_module under option 2,
                # but we keep the impl's home module as the fallback for the
                # `as-routed` projection.
                name_to_module.setdefault(meth["name"], b["proposed_module"])
    for b in blocks:
        # Skip impl blocks — they are the trait/inherent surface and reach
        # across the codebase by design. They are addressed under Risk 1.
        if b["kind"] == "impl":
            continue
        # Skip the test module — its job is to exercise the whole API.
        if b["kind"] == "mod" and b["name"] == "tests":
            continue
        # A block is "hard" if its outbound deps span 2+ non-self modules
        # OR if it was classified as scheme/shared.rs by exclusion.
        modules = {
            name_to_module.get(d)
            for d in b["within_file_deps"]
            if name_to_module.get(d) and name_to_module.get(d) != b["proposed_module"]
        }
        modules.discard(None)
        if b["proposed_module"] == "scheme/shared.rs" or len(modules) >= 2:
            hard_cases.append((b, "outbound deps span", modules))

    if not hard_cases:
        out.append("None.")
    else:
        out.append("The following non-impl blocks reach across module boundaries. "
                   "Each is a candidate to keep in `scheme/shared.rs` as `pub(crate)`, "
                   "or to inline at one call site if the caller is the only consumer.")
        out.append("")
        for b, _, modules in hard_cases:
            other_mods = ", ".join(sorted(m for m in modules if m))
            out.append(
                f"- **`{b['name']}`** ({b['kind']}, lines {b['line_start']}–{b['line_end']}, "
                f"{b['loc']} LOC) — currently routed to `{b['proposed_module']}`; "
                f"reaches into: {other_mods if other_mods else '(no cross-module deps)'}."
            )
            doc1 = doc_one_liner(b["doc"])
            if doc1:
                out.append(f"  - doc: \"{doc1}\"")
            if b["loc"] > 30:
                out.append(
                    "  - **recommendation**: lift to `scheme/shared.rs` as "
                    "`pub(crate) fn`; large enough that duplication isn't viable."
                )
            elif b["loc"] > 10:
                out.append(
                    "  - **recommendation**: either `pub(crate)` in shared.rs, "
                    "or inline if the only cross-module caller is one specific block."
                )
            else:
                out.append(
                    "  - **recommendation**: small enough to duplicate per call site, "
                    "or keep as `pub(crate)` helper in shared.rs."
                )
    out.append("")

    # Risk 3: the recommended LOC targets in #466.
    out.append("### Risk 3: #466's LOC estimates vs. measured")
    out.append("")
    out.append("Issue #466 estimates and what we actually measure (post option-2):")
    out.append("")
    out.append("| Module | #466 estimate | Measured (post option-2) | Verdict |")
    out.append("|---|---|---:|---|")
    estimates = {
        "scheme/rewrites.rs": (2000, 3000),
        "scheme/constraints.rs": (1500, 2000),
        "scheme/predicates.rs": (500, 1000),
        "scheme/actions.rs": (500, 800),
        "scheme.rs (proper)": (500, 1000),
    }
    for m, (lo, hi) in estimates.items():
        measured = option2_loc.get(m, 0)
        if measured > hi:
            verdict = "**over high-end — sub-split recommended**"
        elif measured < lo:
            verdict = "under low-end — may be too small for a dedicated file"
        else:
            verdict = "within estimate range"
        out.append(f"| `{m}` | {lo}–{hi} LOC | {measured} | {verdict} |")
    out.append("")
    out.append(
        "Several modules exceed even the high-end estimate. Each over-ceiling "
        "module needs a concrete sub-split plan. Candidate sub-splits, by module:"
    )
    out.append("")

    # Sub-split candidates per oversized module.
    over_modules = [m for m, loc in option2_loc.items() if loc > LOC_CEILING]
    for m in over_modules:
        bs = grouped.get(m, [])
        # Pull in methods of impls that route here under option 2.
        method_items: list[tuple[str, int, int, int]] = []  # (name, loc, line_start, line_end)
        for b in bs:
            if b["kind"] == "impl" and b.get("methods"):
                for meth in b["methods"]:
                    if assign_method_module(meth["name"]) == m:
                        method_items.append((meth["name"], meth["loc"], meth["line_start"], meth["line_end"]))
            else:
                method_items.append((b["name"], b["loc"], b["line_start"], b["line_end"]))
        # Also add methods from OTHER impls that would migrate here.
        for b in blocks:
            if b["proposed_module"] == m:
                continue
            if b["kind"] == "impl" and b.get("methods"):
                for meth in b["methods"]:
                    if assign_method_module(meth["name"]) == m:
                        method_items.append((meth["name"], meth["loc"], meth["line_start"], meth["line_end"]))
        method_items.sort(key=lambda x: -x[1])

        out.append(f"- **`{m}`** ({option2_loc[m]} LOC post-lift)")
        out.append(f"  - largest items:")
        for name, loc, ls, le in method_items[:6]:
            out.append(f"    - `{name}` ({loc} LOC, lines {ls}–{le})")
        # Heuristic sub-split suggestion
        if m == "scheme/rewrites.rs":
            out.append(
                "  - sub-split candidates: by §-section (`rewrites/h6.rs` AEA, "
                "`rewrites/h8.rs` dissem, `rewrites/h9.rs` non-IC dissem) "
                "OR by pattern (`rewrites/pattern_a_noforn_supremacy.rs`, "
                "`rewrites/pattern_b_fouo_eviction.rs`, "
                "`rewrites/pattern_c_classified_strip.rs`, "
                "`rewrites/pattern_d_caveated_to_noforn.rs`). The "
                "pattern-based split aligns with the existing build_page_rewrites "
                "doc-comment grouping (see lines 2222–2273)."
            )
        elif m == "scheme/predicates.rs":
            out.append(
                "  - sub-split candidates: by predicate family — "
                "`predicates/presence.rs` (the ~25 `presence_*` fns), "
                "`predicates/triggers.rs` (the `*_trigger` family), "
                "`predicates/satisfies.rs` (`satisfies_attrs` + helpers), "
                "`predicates/class_floor.rs` (the class-floor catalog evaluator)."
            )
        elif m == "scheme/actions.rs":
            out.append(
                "  - sub-split candidates: `actions/intent.rs` "
                "(`apply_intent_to_marking` + `apply_fact_add` + "
                "`apply_fact_remove`), `actions/category_ops.rs` "
                "(`capco_category_*` helpers), `actions/companions.rs` "
                "(`emit_*_companions` + `emit_companion_insert`), "
                "`actions/strip.rs` (`strip_*_ucni_action`)."
            )
        elif m == "scheme/constraints.rs":
            out.append(
                "  - sub-split candidates: split `build_constraints` from its "
                "helpers (`e0XX_*` rule emitters into `constraints/rule_emitters.rs`, "
                "class-floor catalog into `constraints/class_floor.rs`, "
                "SCI per-system catalog into `constraints/sci_per_system.rs`)."
            )
        elif m == "scheme.rs (proper)":
            out.append(
                "  - scheme.rs is too big at 2.5×; pull token/category id "
                "constants into `scheme/ids.rs` (cuts ~120 LOC), and consider "
                "moving the `impl MarkingScheme for CapcoScheme` block to "
                "`scheme/marking_scheme_impl.rs` (cuts ~552 LOC). What remains "
                "(constants + struct defs + `new()` + the small Debug/Default/"
                "PartialEq/Eq/From/Lattice impls) fits under the ceiling."
            )
        out.append("")

    # Risk 4: pub-surface preservation.
    out.append("### Risk 4: `pub` surface preservation")
    out.append("")
    out.append(
        "Issue #466's acceptance criteria forbid new `pub` symbols. The proposed "
        "split keeps every helper currently inside `impl CapcoScheme` as a "
        "free `pub(crate)` fn in the destination module (option 2 above). "
        "`pub(crate)` is fine; `pub` is not. Below is the list of currently-"
        "private free functions that the proposed split would need to elevate "
        "to `pub(crate)` so a sibling module can call them:"
    )
    out.append("")
    cross_module_calls: set[str] = set()
    for b in blocks:
        for dep in b["within_file_deps"]:
            dep_mod = name_to_module.get(dep)
            if dep_mod and dep_mod != b["proposed_module"]:
                # Find the dep block to check visibility.
                for db in blocks:
                    if db["name"] == dep and db["visibility"] == "private" and db["kind"] == "fn":
                        cross_module_calls.add(dep)
                        break
    if cross_module_calls:
        for sym in sorted(cross_module_calls):
            out.append(f"- `{sym}`")
    else:
        out.append("None — every currently-private free fn is referenced only within its own proposed module.")
    out.append("")

    # Per-module detailed write-up.
    out.append("## Per-module write-up")
    out.append("")
    for m in PROPOSED_MODULES:
        bs = grouped.get(m, [])
        loc_sum = sum(b["loc"] for b in bs)
        if not bs:
            continue
        out.append(f"### `{m}`")
        out.append("")
        out.append(f"Target LOC: **{loc_sum}** across {len(bs)} blocks.")
        out.append("")
        # Top-3 largest blocks for context.
        top = sorted(bs, key=lambda x: -x["loc"])[:5]
        out.append("Top blocks by LOC:")
        for b in top:
            out.append(
                f"- `{b['name']}` ({b['kind']}, {b['loc']} LOC, "
                f"lines {b['line_start']}–{b['line_end']})"
            )
        out.append("")

    return "\n".join(out) + "\n"


# ---------------------------------------------------------------------------
# License sidecar writer
# ---------------------------------------------------------------------------

LICENSE_TEXT = (
    "SPDX-FileCopyrightText: 2026 Adam Poulemanos\n"
    "\n"
    "SPDX-License-Identifier: MIT OR Apache-2.0\n"
)


def write_license_sidecar(path: Path) -> None:
    sidecar = path.with_suffix(path.suffix + ".license")
    sidecar.write_text(LICENSE_TEXT, encoding="utf-8")


# ---------------------------------------------------------------------------
# Main
# ---------------------------------------------------------------------------


def main() -> int:
    if not SOURCE_FILE.exists():
        print(f"ERROR: source file not found: {SOURCE_FILE}", file=sys.stderr)
        return 1

    OUT_DIR.mkdir(parents=True, exist_ok=True)

    data = parse_file(SOURCE_FILE)

    # Sanity check: sum of LOC + blank lines between blocks should equal
    # total_lines. We report a warning if not.
    block_loc_sum = sum(b["loc"] for b in data["blocks"])
    last_end = 0
    inter_block_lines = 0
    for b in sorted(data["blocks"], key=lambda x: x["line_start"]):
        if b["line_start"] > last_end + 1:
            inter_block_lines += b["line_start"] - last_end - 1
        last_end = max(last_end, b["line_end"])
    trailing = max(0, data["total_lines"] - last_end)
    accounted = block_loc_sum + inter_block_lines + trailing
    if accounted != data["total_lines"]:
        print(
            f"WARNING: line accounting mismatch: blocks={block_loc_sum} "
            f"inter={inter_block_lines} trailing={trailing} "
            f"sum={accounted} vs total={data['total_lines']} "
            f"(delta={data['total_lines'] - accounted})",
            file=sys.stderr,
        )

    # Annotate proposed_module on each block before serialization.
    for b in data["blocks"]:
        b["proposed_module"] = assign_module(b)

    # JSON output — sort keys for determinism.
    JSON_OUT.write_text(
        json.dumps(data, indent=2, sort_keys=True, ensure_ascii=False) + "\n",
        encoding="utf-8",
    )
    write_license_sidecar(JSON_OUT)

    # Markdown reports.
    MD_OUT.write_text(render_markdown_report(data), encoding="utf-8")
    write_license_sidecar(MD_OUT)

    PROPOSAL_OUT.write_text(render_split_proposal(data), encoding="utf-8")
    write_license_sidecar(PROPOSAL_OUT)

    # Print summary.
    grouped: dict[str, int] = {}
    for b in data["blocks"]:
        grouped.setdefault(b["proposed_module"], 0)
        grouped[b["proposed_module"]] += b["loc"]
    print(f"Total blocks: {len(data['blocks'])}")
    print(f"Total LOC:    {data['total_lines']}")
    print()
    print("LOC per proposed module (as-routed):")
    for m in PROPOSED_MODULES:
        print(f"  {m:32s} {grouped.get(m, 0):5d}")
    # Count hard cases: non-impl, non-test blocks whose outbound deps
    # span 2+ other modules, plus anything routed to scheme/shared.rs
    # by exclusion.
    name_to_module = {b["name"]: b["proposed_module"] for b in data["blocks"]}
    hard = 0
    for b in data["blocks"]:
        if b["kind"] == "impl":
            continue
        if b["kind"] == "mod" and b["name"] == "tests":
            continue
        modules = {
            name_to_module.get(d)
            for d in b["within_file_deps"]
            if name_to_module.get(d) and name_to_module.get(d) != b["proposed_module"]
        }
        modules.discard(None)
        if b["proposed_module"] == "scheme/shared.rs" or len(modules) >= 2:
            hard += 1
    print()
    print(f"Hard cases (cross-module fns + shared.rs catch-all): {hard}")
    print()
    print(f"JSON:     {JSON_OUT}")
    print(f"Markdown: {MD_OUT}")
    print(f"Proposal: {PROPOSAL_OUT}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
