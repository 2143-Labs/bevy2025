#!/usr/bin/env python3
"""Check docs/design.html: every in-page href="#X" resolves and there is no <script>."""
import re
import sys
from pathlib import Path

path = Path(__file__).resolve().parent.parent / "docs" / "design.html"
html = path.read_text(encoding="utf-8")

errors = []
if re.search(r"<script", html, re.IGNORECASE):
    errors.append("design.html: <script> not allowed")

ids = set(re.findall(r'\bid="([^"]+)"', html))
anchors = re.findall(r'href="#([^"]*)"', html)
for anchor in sorted(set(anchors)):
    if anchor not in ids:
        errors.append(f"design.html: broken anchor #{anchor}")

if errors:
    print("\n".join(errors), file=sys.stderr)
    sys.exit(1)
print(f"design.html OK ({len(set(anchors))} anchors)")
