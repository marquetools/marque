#!/usr/bin/env python3
import re
import sys

def clean_ocr(text):
    """
    Cleans common OCR artifacts from text, specifically targeted at CAPCO manuals.
    Includes case-aware rejoining of broken words and clusters.
    """
    # Specific broken words and clusters - PRESERVE CASE
    replacements = [
        # (pattern, replacement, flags)
        (r'com pa rtmen t', 'compartment', re.I),
        (r'com p artmen t', 'compartment', re.I),
        (r'com pa rtment', 'compartment', re.I),
        (r'com p artment', 'compartment', re.I),
        (r'tri gra ph', 'trigraph', re.I),
        (r'tri gra phs', 'trigraphs', re.I),
        (r'tetr ag r ap h', 'tetragraph', re.I),
        (r'tetr ag r ap hs', 'tetragraphs', re.I),
        (r'al p hanumeric', 'alphanumeric', re.I),
        (r'al ph abetic', 'alphabetic', re.I),
        (r'pos tp onement', 'postponement', re.I),
        (r'cha ng es', 'changes', re.I),
        (r'sy stem', 'system', re.I),
        (r'ex pl oitation', 'exploitation', re.I),
        (r'ex pl icit', 'explicit', re.I),
        (r'hi gh', 'high', re.I),
        (r'si g nal', 'signal', re.I),
        (r'un ca v e a t e d', 'uncaveated', re.I),
        (r'res pe ctive', 'respective', re.I),
        (r'orga nization', 'organization', re.I),
        (r'p o rtion', 'portion', re.I),
        (r'p ortion', 'portion', re.I),
        (r'ap p l i c a b l e', 'applicable', re.I),
        (r'dis semination', 'dissemination', re.I),
        (r'clas sification', 'classification', re.I),
        (r'de cl assify', 'declassify', re.I),
        (r'identif y', 'identify', re.I),
        (r're p resents', 'represents', re.I),
        (r're p resented', 'represented', re.I),
        (r're p eating', 'repeating', re.I),
        (r're p eat', 'repeat', re.I),
        (r're q uired', 'required', re.I),
        (r're q uires', 'requires', re.I),
        (r're q uirement', 'requirement', re.I),
        (r'va lues', 'values', re.I),
        (r'notyet', 'not yet', re.I),
        (r'use d', 'used', re.I),
        (r'document s', 'documents', re.I),
        (r'materialson', 'materials on', re.I),
        (r'ill eg al', 'illegal', re.I),
        (r'thef t', 'theft', re.I),
        (r'sabot ag e', 'sabotage', re.I),
        (r'e qu i pm ent', 'equipment', re.I),
        (r'weap on', 'weapon', re.I),
        (r'com po nent', 'component', re.I),
        (r'acknowled ge d', 'acknowledged', re.I),
        (r'im pl ementi ng', 'implementing', re.I),
        (r'orig inati ng', 'originating', re.I),
        (r'orig inator', 'originator', re.I),
        (r'im pl ementation', 'implementation', re.I),
        (r'pr ocedures', 'procedures', re.I),
        (r're po rts', 'reports', re.I),
        (r'po wers', 'powers', re.I),
        (r'ap prov al', 'approval', re.I),
        (r'Warni ng s', 'Warnings', re.I),
        (r'warni ng', 'warning', re.I),
        (r'dis pl a y ed', 'displayed', re.I),
        (r'se pa ratel y', 'separately', re.I),
        (r'di sc l os ure', 'disclosure', re.I),
        (r'r e l eas e', 'release', re.I),
        (r'd ete rm i na ti o n', 'determination', re.I),
        (r'nee de d', 'needed', re.I),
        (r'existi ng', 'existing', re.I),
        (r'pr ovisions', 'provisions', re.I),
        (r'comm i ng li ng', 'commingling', re.I),
        (r'prec edence', 'precedence', re.I),
        (r'orga nized', 'organized', re.I),
        (r'orga nizational', 'organizational', re.I),
        (r'D ep uty', 'Deputy', re.I),
        (r'O p erations', 'Operations', re.I),
        (r'po lic y', 'policy', re.I),
        (r'Pos tp onement', 'Postponement', re.I),
        (r'Cha ng es', 'Changes', re.I),
        (r'S y stem', 'System', re.I),
        (r'National S y stem', 'National System', re.I),
        (r'Atomic Ener g y', 'Atomic Energy', re.I),
        (r'Excha ng e', 'Exchange', re.I),
        (r'Grou p', 'Group', re.I),
        (r'Forei g n', 'Foreign', re.I),
        (r'Warni ng', 'Warning', re.I),
        (r'firs t', 'first', re.I),
        (r'p a ge', 'page', re.I),
        (r'go vernments', 'governments', re.I),
        (r'Im pl ementi ng', 'Implementing', re.I),
        (r'Forei g n Disclosure', 'Foreign Disclosure', re.I),
        (r'ap pr oved', 'approved', re.I),
        (r'ap pro val', 'approval', re.I),
        (r'pe rmitted', 'permitted', re.I),
        (r'prio r', 'prior', re.I),
        (r'p laced', 'placed', re.I),
        (r't op', 'top', re.I),
        (r'lef t', 'left', re.I),
        (r'j ustified', 'justified', re.I),
        (r'do c um en t', 'document', re.I),
        (r'c on ta i ns', 'contains', re.I),
        (r'r ef eren c es', 'references', re.I),
        (r'S e ns i t i v e', 'Sensitive', re.I),
        (r'A na l y t i c al', 'Analytical', re.I),
        (r'T ec hn i qu es', 'Techniques', re.I),
        (r'Forei gn', 'Foreign', re.I),
        (r'abbreviation s', 'abbreviations', re.I),
        (r'S IGMA', 'SIGMA', re.I),
    ]

    def case_aware_replace(pattern, replacement, flags):
        def _replace(match):
            orig = match.group(0)
            if orig.isupper():
                return replacement.upper()
            if orig.islower():
                return replacement.lower()
            if orig[0].isupper() and (len(orig) == 1 or orig[1:].islower() or orig[1] == ' '):
                return replacement.capitalize()
            return replacement
        return _replace

    for pattern, replacement, flags in replacements:
        text = re.sub(pattern, case_aware_replace(pattern, replacement, flags), text, flags=flags)

    # Letter-by-letter spaced words (3+ letters)
    # This matches a letter followed by a space, repeated 2+ times, followed by a letter.
    # It avoids matching common portion marks.
    text = re.sub(r'\b(?!(?:U|S|C|TS)\b)[A-Za-z](?:\s[A-Za-z]){2,}\b', lambda m: m.group(0).replace(" ", ""), text)

    # Fix "Roll - Up"
    text = re.sub(r'Roll\s*-\s*Up', 'Roll-Up', text)

    # Fix space before punctuation (but avoid breaking table formatting if possible)
    # Be more specific: only if it's "word ," or "word ."
    text = re.sub(r'(\w)\s([,.;:])', r'\1\2', text)

    # Fix (U For -> (U) For
    text = re.sub(r'\(U For', '(U) For', text)

    # Fix common cases like "T he" -> "The"
    text = re.sub(r'\bT\she\b', 'The', text)
    text = re.sub(r'\bt\she\b', 'the', text)

    # Re-fix specific acronym/word merges that might have happened
    text = re.sub(r'USClassification', 'US Classification', text)
    text = re.sub(r'USAtomic', 'US Atomic', text)
    text = re.sub(r'SBUportions', 'SBU portions', text)
    text = re.sub(r'SBusupersedes', 'SBU supersedes', text)
    text = re.sub(r'elemenTheads', 'element heads', text)
    text = re.sub(r'HUMINtoperations', 'HUMINT operations', text)

    return text

if __name__ == "__main__":
    if len(sys.argv) > 1:
        file_path = sys.argv[1]
        with open(file_path, 'r', encoding='utf-8') as f:
            content = f.read()
        cleaned = clean_ocr(content)
        with open(file_path, 'w', encoding='utf-8') as f:
            f.write(cleaned)
    else:
        print("Usage: scripts/clean_ocr.py <file_path>")
        sys.exit(1)
