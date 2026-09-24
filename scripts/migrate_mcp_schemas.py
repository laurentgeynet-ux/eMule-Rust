#!/usr/bin/env python3
# -*- coding: utf-8 -*-
"""Migration one-shot : compacte les input_schema json! des outils MCP.

- `{"type":"T","description":"D"}` -> prop("T","D")
- `{"type":"object","required":[...],"properties":{P}}` -> obj_schema(json!({P}), &[...])

Usage : python scripts/migrate_mcp_schemas.py <fichier.rs> [...]
Le script est volontairement conservateur : tout schéma non reconnu est laissé
tel quel et signalé sur stderr.
"""

import re
import sys
from pathlib import Path

PROP_RE = re.compile(
    r'"(?P<name>[A-Za-z_][A-Za-z0-9_]*)"\s*:\s*\{\s*'
    r'"type"\s*:\s*"(?P<ty>[^"]+)"\s*,\s*'
    r'"description"\s*:\s*"(?P<desc>(?:[^"\\]|\\.)*)"\s*'
    r'\}',
    re.S,
)


def simple_props(text: str) -> str:
    return PROP_RE.sub(
        lambda m: f'"{m.group("name")}": prop("{m.group("ty")}", "{m.group("desc")}")',
        text,
    )


def split_top_level(body: str, sep: str) -> list[str]:
    """Split `body` on `sep` occurring at bracket depth 0 outside strings."""
    parts, cur = [], []
    depth = 0
    in_str = esc = False
    for ch in body:
        if in_str:
            cur.append(ch)
            if esc:
                esc = False
            elif ch == "\\":
                esc = True
            elif ch == '"':
                in_str = False
            continue
        if ch == '"':
            in_str = True
            cur.append(ch)
            continue
        if ch in "{[(":
            depth += 1
        elif ch in "}])":
            depth -= 1
        if ch == sep and depth == 0:
            parts.append("".join(cur))
            cur = []
        else:
            cur.append(ch)
    parts.append("".join(cur))
    return parts


def top_key(part: str) -> tuple[str, str] | None:
    """Split `"key": value` at the first top-level colon."""
    depth = 0
    in_str = esc = False
    for i, ch in enumerate(part):
        if in_str:
            if esc:
                esc = False
            elif ch == "\\":
                esc = True
            elif ch == '"':
                in_str = False
            continue
        if ch == '"':
            in_str = True
            continue
        if ch in "{[(":
            depth += 1
        elif ch in "}])":
            depth -= 1
        elif ch == ":" and depth == 0:
            return part[:i].strip().strip('"'), part[i + 1 :].strip()
    return None


def find_matching(text: str, start: int, open_ch: str, close_ch: int | str) -> int:
    """Index of the `close_ch` matching `open_ch` at `start` (strings skipped)."""
    depth = 0
    in_str = esc = False
    for i in range(start, len(text)):
        ch = text[i]
        if in_str:
            if esc:
                esc = False
            elif ch == "\\":
                esc = True
            elif ch == '"':
                in_str = False
            continue
        if ch == '"':
            in_str = True
            continue
        if ch == open_ch:
            depth += 1
        elif ch == close_ch:
            depth -= 1
            if depth == 0:
                return i
    return -1


def transform(text: str) -> tuple[str, int]:
    text = simple_props(text)
    out = []
    i = 0
    marker = "input_schema: json!("
    converted = 0
    while True:
        idx = text.find(marker, i)
        if idx == -1:
            out.append(text[i:])
            break
        j = idx + len(marker)
        if j >= len(text) or text[j] != "{":
            out.append(text[i:j])
            i = j
            continue
        end_obj = find_matching(text, j, "{", "}")
        end_paren = find_matching(text, j - 1, "(", ")")
        if end_obj == -1 or end_paren == -1 or end_obj > end_paren:
            print(f"warn: schema non parseable @ {idx}", file=sys.stderr)
            out.append(text[i:j])
            i = j
            continue
        obj_body = text[j + 1 : end_obj]
        tail = text[end_obj + 1 : end_paren]
        parts = [p for p in split_top_level(obj_body, ",") if p.strip()]
        fields = {}
        ok = True
        for p in parts:
            kv = top_key(p)
            if kv is None:
                ok = False
                break
            fields[kv[0]] = kv[1]
        if not ok or fields.get("type", "").strip('"') != "object" or tail.strip():
            print(f"warn: schema non standard @ {idx} ({fields.keys()})", file=sys.stderr)
            out.append(text[i:j])
            i = j
            continue
        props = fields.get("properties", "{}").strip()
        if props.startswith("{") and props.endswith("}"):
            props_inner = props[1:-1].strip()
        else:
            props_inner = props
        required = fields.get("required", "[]").strip()
        req_items = [s.strip() for s in split_top_level(required.strip()[1:-1], ",") if s.strip()]
        req = ", ".join(req_items)
        inner = f"{{{props_inner}}}" if props_inner else "{}"
        out.append(text[i:idx])
        out.append(f"input_schema: obj_schema(json!({inner}), &[{req}]),")
        i = end_paren + 1
        converted += 1
    return "".join(out), converted


def main() -> int:
    total = 0
    for arg in sys.argv[1:]:
        path = Path(arg)
        src = path.read_text(encoding="utf-8")
        dst, n = transform(src)
        path.write_text(dst, encoding="utf-8")
        total += n
        print(f"{path}: {n} schemas convertis")
    print(f"total: {total}")
    return 0


if __name__ == "__main__":
    sys.exit(main())
