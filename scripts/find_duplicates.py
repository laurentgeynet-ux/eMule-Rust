#!/usr/bin/env python3
# -*- coding: utf-8 -*-
"""Detecteur de duplication pour le workspace Rust eMuleRust.

Trois angles d'analyse, tous hors-ligne :
  A. Symboles homonymes (fn/struct/enum/trait/const definis dans >=2 fichiers)
  B. Corps de fonctions dupliques (hash du corps normalise : identique ou
     identique apres renommage des identifiants)
  C. Fenetres de clones (blocs de >=WINDOW lignes nettoyees identiques,
     style CPD)

Usage : python scripts/find_duplicates.py [--window N] [--root crates]
Sortie : rapport texte sur stdout (rediriger vers un fichier pour archiver).
"""

import argparse
import hashlib
import re
import sys
from collections import defaultdict
from pathlib import Path

KEYWORDS = {
    "as", "async", "await", "break", "const", "continue", "crate", "dyn",
    "else", "enum", "extern", "false", "fn", "for", "if", "impl", "in",
    "let", "loop", "match", "mod", "move", "mut", "pub", "ref", "return",
    "self", "Self", "static", "struct", "super", "trait", "true", "type",
    "unsafe", "use", "where", "while", "macro_rules", "Box", "Some", "None",
    "Ok", "Err", "Vec", "String", "Option", "Result", "Copy", "Clone",
}

SYMBOL_RE = re.compile(
    r"(?:^|\s)(?:pub(?:\([^)]*\))?\s+)?(?:async\s+)?(?:unsafe\s+)?"
    r"(fn|struct|enum|trait|const|static|mod)\s+([A-Za-z_][A-Za-z0-9_]*)"
)

ATTR_LINE_RE = re.compile(r"^\s*#?\s*!\s*\[.*$")


def clean_source(text: str) -> str:
    """Remplace commentaires et litteraux par des espaces, conserve la structure."""
    out = list(text)
    i, n = 0, len(text)
    state = "code"
    block_depth = 0
    raw_hashes = 0
    while i < n:
        c = text[i]
        if state == "code":
            if c == "/" and i + 1 < n and text[i + 1] == "/":
                j = text.find("\n", i)
                j = n if j == -1 else j
                for k in range(i, j):
                    out[k] = " "
                i = j
            elif c == "/" and i + 1 < n and text[i + 1] == "*":
                state = "block"
                block_depth = 1
                out[i] = out[i + 1] = " "
                i += 2
            elif c == "r" and i + 1 < n and text[i + 1] in ('"', "#"):
                j = i + 1
                hashes = 0
                while j < n and text[j] == "#":
                    hashes += 1
                    j += 1
                if j < n and text[j] == '"':
                    state = "raw"
                    raw_hashes = hashes
                    for k in range(i, j + 1):
                        out[k] = " "
                    i = j + 1
                else:
                    i += 1
            elif c == '"':
                state = "str"
                out[i] = " "
                i += 1
            elif c == "'":
                # litteral char 'x' ou '\n' — distinguer du lifetime 'a
                if i + 2 < n and text[i + 2] == "'":
                    out[i] = out[i + 1] = out[i + 2] = " "
                    i += 3
                elif i + 3 < n and text[i + 1] == "\\" and text[i + 3] == "'":
                    for k in range(i, i + 4):
                        out[k] = " "
                    i += 4
                else:
                    i += 1  # lifetime
            else:
                i += 1
        elif state == "block":
            if c == "/" and i + 1 < n and text[i + 1] == "*":
                block_depth += 1
                out[i] = out[i + 1] = " "
                i += 2
            elif c == "*" and i + 1 < n and text[i + 1] == "/":
                block_depth -= 1
                out[i] = out[i + 1] = " "
                i += 2
                if block_depth == 0:
                    state = "code"
            else:
                if c != "\n":
                    out[i] = " "
                i += 1
        elif state == "str":
            if c == "\\" and i + 1 < n:
                out[i] = out[i + 1] = " "
                i += 2
            elif c == '"':
                out[i] = " "
                state = "code"
                i += 1
            else:
                if c != "\n":
                    out[i] = " "
                i += 1
        elif state == "raw":
            if c == '"':
                j = i + 1
                ok = True
                for h in range(raw_hashes):
                    if j + h >= n or text[j + h] != "#":
                        ok = False
                        break
                if ok:
                    for k in range(i, j + raw_hashes):
                        out[k] = " "
                    i = j + raw_hashes
                    state = "code"
                else:
                    if c != "\n":
                        out[i] = " "
                    i += 1
            else:
                if c != "\n":
                    out[i] = " "
                i += 1
    return "".join(out)


def find_matching_brace(cleaned: str, open_idx: int) -> int:
    depth = 0
    i = open_idx
    n = len(cleaned)
    while i < n:
        c = cleaned[i]
        if c == "{":
            depth += 1
        elif c == "}":
            depth -= 1
            if depth == 0:
                return i
        i += 1
    return -1


def extract_functions(cleaned: str):
    """Retourne [(name, body_text)] pour chaque fn avec corps (pas les decls trait)."""
    fns = []
    for m in re.finditer(r"\bfn\s+([A-Za-z_][A-Za-z0-9_]*)", cleaned):
        # cherche la fin de signature : premier '{' ou ';' a profondeur 0
        i = m.end()
        depth_paren = depth_brack = depth_angle = 0
        brace_idx = -1
        while i < len(cleaned):
            c = cleaned[i]
            if c == "(":
                depth_paren += 1
            elif c == ")":
                depth_paren -= 1
            elif c == "[":
                depth_brack += 1
            elif c == "]":
                depth_brack -= 1
            elif c == "<":
                depth_angle += 1
            elif c == ">":
                depth_angle = max(0, depth_angle - 1)
            elif c == "{" and depth_paren == 0 and depth_brack == 0:
                brace_idx = i
                break
            elif c == ";" and depth_paren == 0 and depth_brack == 0:
                break  # declaration sans corps
            i += 1
        if brace_idx == -1:
            continue
        end = find_matching_brace(cleaned, brace_idx)
        if end == -1:
            continue
        fns.append((m.group(1), cleaned[brace_idx:end + 1]))
    return fns


def norm_body(body: str, rename_idents: bool) -> str:
    toks = re.findall(r"[A-Za-z_][A-Za-z0-9_]*|[^A-Za-z_0-9\s]", body)
    if rename_idents:
        toks = [t if (not re.match(r"^[A-Za-z_]", t) or t in KEYWORDS) else "ID"
                for t in toks]
    return hashlib.sha1(" ".join(toks).encode()).hexdigest()


def collect_rs_files(root: Path):
    files = []
    for p in sorted(root.rglob("*.rs")):
        if "target" in p.parts:
            continue
        files.append(p)
    return files


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--root", default="crates")
    ap.add_argument("--window", type=int, default=8,
                    help="taille de fenetre en lignes nettoyees (defaut 8)")
    ap.add_argument("--min-body-tokens", type=int, default=25,
                    help="taille min d'un corps de fn (en tokens) pour le hash")
    args = ap.parse_args()

    root = Path(args.root)
    files = collect_rs_files(root)
    total_lines = 0
    symbols = defaultdict(list)           # name -> [(file, line, kind)]
    fn_exact = defaultdict(list)          # hash -> [(file, name, line)]
    fn_renamed = defaultdict(list)
    windows = defaultdict(list)           # hash -> [(file, win_start_clean_idx)]
    clean_lines_of = {}                   # file -> [(orig_lineno, cleaned_line)]

    for p in files:
        text = p.read_text(encoding="utf-8", errors="replace")
        total_lines += text.count("\n") + 1
        cleaned = clean_source(text)
        rel = str(p).replace("\\", "/")
        clines = []
        for idx, line in enumerate(cleaned.splitlines(), start=1):
            s = " ".join(line.split())
            clines.append((idx, s))
        clean_lines_of[rel] = clines
        nonempty = [(n_, s) for n_, s in clines if s and s not in ("{", "}", "};", "{;", ")")]
        for i in range(0, max(0, len(nonempty) - args.window + 1)):
            chunk = nonempty[i:i + args.window]
            h = hashlib.sha1("\n".join(s for _, s in chunk).encode()).hexdigest()
            windows[h].append((rel, chunk[0][0]))

        for m in SYMBOL_RE.finditer(cleaned):
            kind, name = m.group(1), m.group(2)
            line = cleaned.count("\n", 0, m.start()) + 1
            symbols[name].append((rel, line, kind))

        for name, body in extract_functions(cleaned):
            ntok = len(re.findall(r"[A-Za-z_][A-Za-z0-9_]*|\S", body))
            if ntok < args.min_body_tokens:
                continue
            line = cleaned.count("\n", 0, cleaned.find(body)) + 1
            fn_exact[norm_body(body, False)].append((rel, name, line))
            fn_renamed[norm_body(body, True)].append((rel, name, line))

    print(f"== Baseline ==  {len(files)} fichiers, {total_lines} lignes\n")

    print("== A. Symboles homonymes (meme nom, >=2 fichiers) ==")
    shown = 0
    for name in sorted(symbols, key=lambda k: -len({f for f, _, _ in symbols[k]})):
        locs = symbols[name]
        fset = sorted({f for f, _, _ in locs})
        if len(fset) < 2:
            continue
        if name in ("new", "default", "fmt", "drop", "from", "clone",
                    "as_str", "len", "is_empty", "fmt", "eq", "hash",
                    "serialize", "deserialize", "borrow", "deref", "into"):
            continue  # impls de traits standard — bruit
        kinds0 = {k for _, _, k in locs}
        if kinds0 == {"mod"} and name in ("tests", "types"):
            continue  # idiome Rust : modules tests/types par fichier — legitime
        shown += 1
        kinds = {k for _, _, k in locs}
        print(f"  {name}  x{len(locs)} ({'/'.join(sorted(kinds))})")
        for f, ln, k in locs:
            print(f"      {f}:{ln} [{k}]")
    if not shown:
        print("  (aucun)")

    print("\n== B1. Corps de fonctions strictement identiques ==")
    shown = 0
    for h, locs in sorted(fn_exact.items(), key=lambda kv: -len(kv[1])):
        uniq = {(f, l) for f, _, l in locs}
        if len(uniq) < 2:
            continue
        shown += 1
        names = {n for _, n, _ in locs}
        print(f"  [{'/'.join(sorted(names))}] x{len(uniq)}")
        for f, n, l in locs:
            print(f"      {f}:{l}  fn {n}")
    if not shown:
        print("  (aucun)")

    print("\n== B2. Corps identiques apres renommage d'identifiants ==")
    shown = 0
    for h, locs in sorted(fn_renamed.items(), key=lambda kv: -len(kv[1])):
        uniq = {(f, l) for f, _, l in locs}
        if len(uniq) < 2:
            continue
        names = {n for _, n, _ in locs}
        if len(names) < 2 and len({f for f, _, _ in locs}) < 2:
            continue
        shown += 1
        print(f"  [{'/'.join(sorted(names))}] x{len(uniq)}")
        for f, n, l in locs:
            print(f"      {f}:{l}  fn {n}")
    if not shown:
        print("  (aucun)")

    print(f"\n== C. Blocs clones inter-fichiers (>={args.window} lignes nettoyees) ==")
    # Fusionne les fenetres glissantes en blocs : deux fenetres identiques a
    # (f1,l1) et (f2,l2) appartiennent au meme clone si meme delta l2-l1.
    diagonals = defaultdict(set)  # (f1,f2,delta) -> set de l1
    for occs in windows.values():
        locs = sorted(set(occs))
        for i in range(len(locs)):
            for j in range(i + 1, len(locs)):
                f1, l1 = locs[i]
                f2, l2 = locs[j]
                if f1 == f2:
                    continue
                diagonals[(f1, f2, l2 - l1)].add(l1)

    blocks = []
    for (f1, f2, delta), starts in diagonals.items():
        starts = sorted(starts)
        i = 0
        while i < len(starts):
            j = i
            while j + 1 < len(starts) and starts[j + 1] <= starts[j] + 1:
                j += 1
            nlines = (starts[j] - starts[i]) + args.window
            if nlines >= args.window + 2:  # au moins un vrai bloc fusionne
                blocks.append((nlines, f1, starts[i], f2, starts[i] + delta))
            i = j + 1

    blocks.sort(key=lambda b: -b[0])
    shown = 0
    seen = set()
    for nlines, f1, l1, f2, l2 in blocks:
        key = (f1, l1, f2, l2)
        if key in seen:
            continue
        seen.add(key)
        shown += 1
        print(f"  ~{nlines} lignes : {f1}:{l1}  <->  {f2}:{l2}")
        if shown >= 200:
            print("  ... (tronque)")
            break
    if not shown:
        print("  (aucun)")


if __name__ == "__main__":
    main()
