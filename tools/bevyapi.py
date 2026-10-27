#!/usr/bin/env python3
"""Offline API lookup for bevy crates at an exact version.

Ground truth is docs.rs rustdoc JSON (the compiler's own view of the API), not
model memory. Fetches once, caches a compact index, answers in a few hundred
tokens.

    ./bevyapi.py fetch                 # index every crate in CRATES at BEVY_VER
    ./bevyapi.py search 'Query::iter'  # find items by regex on module::name
    ./bevyapi.py sig Query::iter       # full signatures + doc line
    ./bevyapi.py mod bevy_ecs::query   # everything in one module
    ./bevyapi.py crates                # what is cached
"""
import argparse, json, os, re, sys, urllib.request, urllib.error
from pathlib import Path

BEVY_VER = os.environ.get("BEVY_VER", "0.20.0")
CACHE = Path(os.environ.get("BEVYAPI_CACHE", Path.home() / ".cache" / "bevyapi"))

# Everything a bevy 3d game + this workspace's deps can plausibly touch.
# crate -> version (None means "same major.minor as BEVY_VER")
CRATES = {
    "bevy": None, "bevy_ecs": None, "bevy_app": None, "bevy_asset": None,
    "bevy_render": None, "bevy_pbr": None, "bevy_mesh": None, "bevy_image": None,
    "bevy_camera": None, "bevy_light": None, "bevy_shader": None, "bevy_ui": None,
    "bevy_ui_render": None, "bevy_sprite": None, "bevy_input": None,
    "bevy_time": None, "bevy_transform": None, "bevy_reflect": None,
    "bevy_state": None, "bevy_window": None, "bevy_scene": None,
    "bevy_animation": None, "bevy_audio": None, "bevy_log": None,
    "bevy_math": None, "bevy_color": None, "bevy_platform": None,
    "bevy_tasks": None, "bevy_picking": None, "bevy_gizmos": None,
    "avian3d": "0.8.0",  # versioned independently of bevy
}


# ---------------------------------------------------------------- type render
# id -> short name, filled per crate while indexing (format 61 gives resolved_path
# a `path` string but no `name`, so cross-crate types need resolving)
NAMES = {}


def _short(path):
    return (path or "").split("::")[-1]


def _bound(b):
    if "trait_bound" in b:
        tb, tr = b["trait_bound"], b["trait_bound"]["trait"]
        s = _short(tr.get("path")) + _args(tr.get("args"))
        return "?" + s if tb.get("modifier") == "maybe" else s
    if "outlives" in b:
        return b["outlives"]
    if "use" in b:
        return "use<...>"
    return ""


def _args(a):
    if not a or "angle_bracketed" not in a:
        return ""
    out = []
    for x in a["angle_bracketed"].get("args", []):
        if "lifetime" in x:
            out.append(x["lifetime"])
        elif "type" in x:
            out.append(_ty(x["type"]))
        elif "const" in x:
            out.append(str(x["const"].get("expr", "..")))
        elif "infer" in x:
            out.append("_")
    return "<" + ", ".join(out) + ">" if out else ""


def _ty(t):
    try:
        return _ty_inner(t)
    except Exception:
        return ".."  # never crash a whole lookup on one exotic type


def _ty_inner(t):
    if t is None:
        return "()"
    if not isinstance(t, dict):
        return ".."
    if "primitive" in t:
        return t["primitive"]
    if "generic" in t:
        return t["generic"]
    if "resolved_path" in t:
        rp = t["resolved_path"]
        nm = _short(rp.get("path")) or NAMES.get(str(rp.get("id"))) or "?"
        return nm + _args(rp.get("args"))
    if "borrowed_ref" in t:
        br = t["borrowed_ref"]
        lt = (br.get("lifetime") + " ") if br.get("lifetime") else ""
        return f"&{lt}{'mut ' if br.get('is_mutable') else ''}{_ty(br['type'])}"
    if "raw_pointer" in t:
        rp = t["raw_pointer"]
        return f"*{'mut' if rp.get('is_mutable') else 'const'} {_ty(rp['type'])}"
    if "tuple" in t:
        return "(" + ", ".join(_ty(x) for x in t["tuple"]) + ")"
    if "slice" in t:
        return "[" + _ty(t["slice"]) + "]"
    if "array" in t:
        return "[" + _ty(t["array"]["type"]) + "; " + str(t["array"].get("len")) + "]"
    if "impl_trait" in t:
        return "impl " + " + ".join(_bound(b) for b in t["impl_trait"])
    if "dyn_trait" in t:
        return "dyn " + " + ".join(_bound(p["trait"]) for p in t["dyn_trait"]
                                   if isinstance(p, dict) and "trait" in p)
    if "qualified_path" in t:
        qp = t["qualified_path"]
        tr = qp.get("trait")
        pre = (tr["path"] + _args(tr.get("args")) + "::") if tr and tr.get("path") else ""
        return f"<{_ty(qp['self_type'])} as {pre}{qp['name']}>{_args(qp.get('args'))}"
    if "function_pointer" in t:
        fp = t["function_pointer"]
        ins = ", ".join(_ty(a[1]) for a in fp["sig"]["inputs"])
        o = _ty(fp["sig"]["output"])
        return f"fn({ins})" + ("" if o == "()" else f" -> {o}")
    if "infer" in t:
        return "_"
    if "pat" in t:
        return _ty(t["pat"].get("type", {}))
    return ".."


def _generics(g):
    ps = []
    for p in (g or {}).get("params", []):
        k = p["kind"]
        if "type" in k:
            if p["name"].startswith("impl "):
                ps.append(p["name"])  # `impl Trait` param: the name *is* the bound
                continue
            b = [x for x in (_bound(y) for y in k["type"].get("bounds", [])) if x]
            ps.append(p["name"] + (": " + " + ".join(b) if b else ""))
        elif "lifetime" in k:
            ps.append(p["name"])
        elif "const" in k:
            ps.append(f"const {p['name']}: {_ty(k['const']['type'])}")
    return "<" + ", ".join(ps) + ">" if ps else ""


def _sig(item, name=None):
    f = item["inner"]["function"]
    hd = f["header"]
    out = []
    if hd.get("is_const"):
        out.append("const ")
    if hd.get("is_async"):
        out.append("async ")
    if hd.get("is_unsafe"):
        out.append("unsafe ")
    out.append("fn " + (name or item.get("name") or "?") + _generics(f.get("generics")))
    ins = []
    for n, ty in f["sig"]["inputs"]:
        if n == "self":
            if ty == {"generic": "Self"}:
                ins.append("self")
            elif isinstance(ty, dict) and "borrowed_ref" in ty and ty["borrowed_ref"]["type"] == {"generic": "Self"}:
                ins.append("&mut self" if ty["borrowed_ref"].get("is_mutable") else "&self")
            else:
                ins.append("self: " + _ty(ty))
        else:
            ins.append(f"{n}: {_ty(ty)}")
    out.append("(" + ", ".join(ins) + ")")
    o = _ty(f["sig"].get("output"))
    if o != "()":
        out.append(" -> " + o)
    return "".join(out)


def _doc(raw, cap=150):
    if not raw:
        return ""
    t = re.sub(r"\[`?([^\]]+)`?\]\([^)]*\)", r"\1", raw)
    t = re.sub(r"[`*#]", "", t)
    t = re.sub(r"\s+", " ", t).strip()
    return t[:cap] + ("..." if len(t) > cap else "")


# ---------------------------------------------------------------- fetch/index
def fetch_one(crate, ver):
    url = f"https://docs.rs/crate/{crate}/{ver}/json"
    CACHE.mkdir(parents=True, exist_ok=True)
    dest = CACHE / f"{crate}-{ver}.json"
    if dest.exists():
        return dest
    try:
        with urllib.request.urlopen(url, timeout=60) as r:
            blob = r.read()
    except urllib.error.HTTPError as e:
        print(f"  {crate}@{ver}: HTTP {e.code} (no rustdoc JSON at that version)", file=sys.stderr)
        return None
    if blob[:4] == b"\x28\xb5\x2f\xfd":  # zstd magic
        try:
            import compression.zstd as zstd  # py>=3.14
            blob = zstd.decompress(blob)
        except Exception:
            import subprocess, tempfile
            with tempfile.NamedTemporaryFile(suffix=".zst", delete=False) as fh:
                fh.write(blob)
                tmp = fh.name
            blob = subprocess.run(["zstd", "-dc", tmp], capture_output=True, check=True).stdout
            os.unlink(tmp)
    raw = json.loads(blob)
    items = []
    idx, paths = raw["index"], raw["paths"]
    NAMES.clear()
    for pid, pp in paths.items():
        if pp.get("path"):
            NAMES[pid] = pp["path"][-1]
    for pid, ex in (raw.get("external_crates") or {}).items():
        NAMES.setdefault(pid, ex.get("name") or "?")
    for sid, p in paths.items():
        if p.get("crate_id") != 0:
            continue  # re-exports from other crates; they have their own crate
        it = idx.get(str(sid))
        if it is None:
            continue
        path, kind = p["path"], p["kind"]
        mod = "::".join(path[:-1]) or crate
        if kind == "function":
            items.append({"m": mod, "k": "fn", "n": it.get("name") or "?", "s": _sig(it), "d": _doc(it.get("docs"))})
        elif kind in ("struct", "enum", "trait", "union"):
            body = it["inner"].get(kind) or {}
            g = _generics(body.get("generics"))
            label = {"struct": "struct", "enum": "enum", "trait": "trait", "union": "union"}[kind] + " "
            if kind == "trait":
                b = [x for x in (_bound(y) for y in body.get("bounds", [])) if x]
                label += (it.get("name") or "?") + g + (": " + " + ".join(b) if b else "")
            else:
                label += (it.get("name") or "?") + g
            items.append({"m": mod, "k": kind, "n": it.get("name") or "?", "s": label, "d": _doc(it.get("docs"))})
            container = f"{mod}::{it.get('name')}"
            if kind == "trait":
                for aid in body.get("items", []):
                    a = idx.get(str(aid))
                    if a and "function" in a["inner"]:
                        items.append({"m": container, "k": "traitfn", "n": a.get("name") or "?", "s": _sig(a), "d": _doc(a.get("docs"))})
            for iid in body.get("impls") or []:
                imp = idx.get(str(iid))
                if not imp or "impl" not in imp["inner"]:
                    continue
                ib = imp["inner"]["impl"]
                if ib.get("trait") is not None or ib.get("is_synthetic"):
                    continue
                for aid in ib.get("items", []):
                    a = idx.get(str(aid))
                    if a and "function" in a["inner"]:
                        items.append({"m": container, "k": "method", "n": a.get("name") or "?", "s": _sig(a), "d": _doc(a.get("docs"))})
        elif kind in ("macro", "constant", "type_alias", "static", "primitive"):
            items.append({"m": mod, "k": kind, "n": it.get("name") or "?", "s": f"{kind} {it.get('name') or '?'}", "d": _doc(it.get("docs"))})
        elif kind == "module":
            items.append({"m": "::".join(path), "k": "mod", "n": path[-1], "s": f"module {path[-1]}", "d": _doc(it.get("docs"), 100)})

    # Prelude contents: which names are already in scope with `use X::prelude::*`,
    # and where each one actually lives. Answers the most common bevy question.
    stack = [(crate, idx.get(str(raw["root"])))]
    while stack:
        mpath, mod = stack.pop()
        body = (mod or {}).get("inner", {}).get("module") or {}
        for cid in body.get("items", []):
            ch = idx.get(str(cid))
            if not ch:
                continue
            if "module" in ch["inner"]:
                stack.append((mpath + "::" + (ch.get("name") or "?"), ch))
            elif "use" in ch["inner"]:
                u = ch["inner"]["use"]
                if u.get("is_glob") or not mpath.endswith("prelude"):
                    continue
                nm = u.get("name") or _short(u.get("source"))
                src = u.get("source") or "?"
                items.append({"m": mpath, "k": "use", "n": nm, "s": f"re-export of {src}", "d": ""})

    # rustdoc lists assoc items both in `paths` (as functions) and through their
    # impl block; collapse those, but keep trait methods distinct from inherent ones.
    seen, uniq = set(), []
    for x in items:
        key = (x["m"], "trait" if x["k"] == "traitfn" else "", x["n"], x["s"])
        if key in seen:
            continue
        seen.add(key)
        uniq.append(x)
    dest.write_text(json.dumps({"crate": crate, "version": ver, "items": uniq}))
    return dest


def cached():
    return sorted(CACHE.glob("*.json"))


def load(args):
    files = cached()
    if args.crate:
        files = [f for f in files if f.name.split("-")[0] in args.crate]
    out = []
    for f in files:
        out.extend(json.loads(f.read_text())["items"])
    return out


def main():
    ap = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    ap.add_argument("-n", "--limit", type=int, default=40)
    ap.add_argument("--crate", action="append", help="restrict to these crates (repeatable)")
    ap.add_argument("cmd", choices=["fetch", "search", "sig", "mod", "crates"])
    ap.add_argument("arg", nargs="?", help="regex / item name / module path")
    args = ap.parse_args()

    if args.cmd == "fetch":
        targets = args.arg.split(",") if args.arg else list(CRATES)
        for c in targets:
            ver = CRATES.get(c) or BEVY_VER
            got = fetch_one(c, ver)
            if got:
                n = len(json.loads(got.read_text())["items"])
                print(f"  {c}@{ver}: {n} items")
        return

    if args.cmd == "crates":
        for f in cached():
            meta = json.loads(f.read_text())
            print(f"{meta['crate']:16} {meta['version']:10} {len(meta['items']):>6} items")
        return

    if not cached():
        sys.exit("nothing cached yet: run `bevyapi.py fetch`")

    items = load(args)
    if args.cmd == "search":
        if not args.arg:
            sys.exit("search needs a regex, e.g. 'Query::iter'")
        rx = re.compile(args.arg, re.I)
        hits = [x for x in items if rx.search(x["m"] + "::" + x["n"])]
        hits.sort(key=lambda x: (len(x["m"]), x["m"], x["n"]))
        for x in hits[:args.limit]:
            print(f"{x['m']}::{x['n']}\t{x['s'][:100]}")
        if len(hits) > args.limit:
            print(f"... {len(hits) - args.limit} more (raise -n, or tighten the regex)")
        if not hits:
            print("no match")
        return

    if args.cmd == "sig":
        if not args.arg:
            sys.exit("sig needs an item, e.g. 'Query::iter'")
        parts = [p for p in args.arg.split("::") if p]
        member, container = parts[-1], (parts[-2] if len(parts) > 1 else None)
        want = [re.compile(re.escape(w), re.I) for w in args.arg.replace("::", " ").split()]
        hits = [x for x in items if all(r.search(x["m"] + "::" + x["n"]) for r in want)]

        def rank(x):
            exact_n = x["n"].lower() == member.lower()
            exact_c = container is None or x["m"].lower() == container.lower() or \
                x["m"].lower().endswith("::" + container.lower())
            return (0 if (exact_n and exact_c) else 1, len(x["m"]), x["n"])

        hits.sort(key=rank)
        if hits and rank(hits[0])[0] == 0:  # narrow to the exact item when we have one
            hits = [x for x in hits if rank(x)[0] == 0]
        for x in hits[:args.limit]:
            print(f"// {x['m']}")
            print(x["s"])
            if x["d"]:
                print(f"// {x['d']}")
            print()
        if not hits:
            print("no match")
        return

    if args.cmd == "mod":
        if not args.arg:
            sys.exit("mod needs a path, e.g. 'bevy_ecs::query'")
        rx = re.compile(r"^" + re.escape(args.arg) + r"(:|$)", re.I)
        hits = [x for x in items if rx.match(x["m"])]
        hits.sort(key=lambda x: (x["m"], x["n"]))
        for x in hits[:args.limit]:
            print(f"{x['m']}::{x['n']}\t{x['s'][:100]}")
        if not hits:
            print("no match")
        return


if __name__ == "__main__":
    main()
