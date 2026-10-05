"""类型族纯搬移（按行区间 + 事务式 + 自愈补 import）。用法：python3 target/split_family.py dict"""
import re, subprocess, sys, pathlib, shutil, os

family = sys.argv[1]
root = pathlib.Path("crates/pyawa-core/src")
bp = root / os.environ.get("SPLIT_SRC", "builtin_objects.rs")
DIR = os.environ.get("SPLIT_DIR", "builtin")
DECL = root / os.environ.get("SPLIT_DECL", "builtin.rs")
OLD = os.environ.get("SPLIT_OLD", "builtin_objects")
NEWPATH = os.environ.get("SPLIT_NEW", "builtin")
orig = bp.read_text()
lines = orig.splitlines(keepends=True)

def line_of(pos): return orig.count("\n", 0, pos)

def body_end(text, s):
    i = text.index("(", s); d = 0
    while i < len(text):
        if text[i] == "(": d += 1
        elif text[i] == ")":
            d -= 1
            if d == 0: break
        i += 1
    j = text.index("{", i); d = 0; k = j
    while k < len(text):
        if text[k] == "{": d += 1
        elif text[k] == "}":
            d -= 1
            if d == 0: return k + 1
        k += 1
    raise ValueError("函数体没配平")

def doc_start_line(text, pos):
    """往上吃掉紧邻的文档注释**与属性** ✓（`#[allow(dead_code)]` 一类必须跟函数一起搬 ✓ ——
    第 129 轮 `context` 族就是因为只认 `///` ✗ ⇒ 属性留在原文件、函数搬走 ⇒ 4 条"无人引用"警告 ✗）。"""
    ln = line_of(pos)
    while ln - 1 >= 0:
        s = lines[ln - 1].strip()
        if s.startswith("///") or s.startswith("#[") or s == "":
            ln -= 1
        else:
            break
    return ln

picked = []
for m in re.finditer(r"^(?:pub |pub\(crate\) )?(?:unsafe )?fn (" + os.environ.get("SPLIT_ALT", family + r"_[a-z_0-9]+") + r")\(", orig, re.M):
    if m.group(1).endswith("_method_native"):
        print("跳过派发表:", m.group(1)); continue
    picked.append((m.group(1), doc_start_line(orig, m.start()), line_of(body_end(orig, m.start())) + 1))
picked.sort(key=lambda p: p[1])
for a, b in zip(picked, picked[1:]):
    assert a[2] <= b[1], f"行区间重叠：{a} / {b}"
if not picked: sys.exit(f"没找到 {family}_* 函数")
moved = [p[0] for p in picked]
moved_text = "".join("".join(lines[p[1]:p[2]]) for p in picked)

defs = {m.group(2) for m in re.finditer(r"^(?:pub |pub\(crate\) )?(?:unsafe )?(fn|const|static) ([A-Za-z_][A-Za-z_0-9]*)", orig, re.M)}
refs = sorted({x for x in set(re.findall(r"\b([A-Za-z_][A-Za-z_0-9]*)\b", moved_text)) if x in defs and x not in moved})

macros = sorted({x for x in set(re.findall(r"\b([A-Za-z_][A-Za-z_0-9]*)\s*!", moved_text)) if ("macro_rules! " + x) in orig})

drop = {ln for p in picked for ln in range(p[1], p[2])}
keep = []
for idx, line in enumerate(lines):
    if idx in drop: continue
    if refs and re.match(r"^(unsafe )?(fn|const|static) (" + "|".join(map(re.escape, refs)) + r")\b", line):
        line = "pub(crate) " + line if line.startswith(("fn ", "const ", "static ")) else "pub(crate) " + line
    keep.append(line)
new_bp = "".join(keep)
for mac in macros:                      # 宏要导出才能跨模块用 ✓
    mm = re.search(r"^macro_rules! " + mac + r"\b", new_bp, re.M)
    if mm:
        i = new_bp.index("{", mm.end()); d = 0; k = i
        while k < len(new_bp):
            if new_bp[k] == "{": d += 1
            elif new_bp[k] == "}":
                d -= 1
                if d == 0: break
            k += 1
        new_bp = new_bp[:k + 1] + "\npub(crate) use " + mac + ";" + new_bp[k + 1:]
USE_ANCHOR = "use crate::header::Header;" if "use crate::header::Header;" in new_bp else new_bp.split("\n")[[i for i, l in enumerate(new_bp.split("\n")) if l.startswith("use ")][0]]
assert USE_ANCHOR in new_bp
_has_pub = any(re.search(r"^pub (?:unsafe )?fn " + re.escape(n) + r"\b", orig, re.M) for n in moved)
new_bp = new_bp.replace(USE_ANCHOR, ("pub use crate::" if _has_pub else "pub(crate) use crate::") + NEWPATH + "::" + family + "::*;\nuse crate::header::Header;", 1)

orig_uses = "".join(l for l in lines if re.match(r"^(pub )?use .*;\n$", l))
use_line = ("use crate::" + OLD + "::{" + ", ".join(refs) + "};\n" if len(refs) > 1 else
            ("use crate::" + OLD + "::" + refs[0] + ";\n" if refs else ""))
head = ("//! **`" + family + "` 族** —— 从 `builtin_objects.rs` **原样搬来** ✓（纯移动 ✓、零逻辑改动 ✓）。\n//!\n"
        "//! * 派发表 `" + family + "_method_native` **不搬** ✓（族之间的粘合层 ✓）；\n"
        "//! * 仍在原处的被引用项：" + ("、".join("`" + r + "`" for r in refs) if refs else "（无）") + " ✓；\n"
        "//! * `use` 块照抄原文件 ✓（`cargo fix` 随后删多余的 ✓），缺失项由自愈循环补 ✓。\n\n"
        + use_line + "\n")
if macros:
    head += "use crate::" + OLD + "::{" + ", ".join(macros) + "};\n"
new_mod = head + re.sub(r"^(unsafe )?fn ", r"pub(crate) \1fn ", moved_text, flags=re.M)

bak = pathlib.Path("target/split-backup"); bak.mkdir(parents=True, exist_ok=True)
shutil.copy(bp, bak / bp.name)
mods, out = DECL, root / DIR / (family + ".rs")
shutil.copy(mods, bak / mods.name)
external = {}
for f in pathlib.Path("crates").rglob("*.rs"):
    s = f.read_text(); o = s
    for name in moved:
        s = re.sub(OLD + r"::" + name + r"\b", NEWPATH + "::" + family + "::" + name, s)
    if s != o:
        external[f] = o; shutil.copy(f, bak / f.name); f.write_text(s)
out.parent.mkdir(parents=True, exist_ok=True)
try:
    bp.write_text(new_bp); out.write_text(new_mod)
    if ("mod " + family + ";") not in mods.read_text():
        mods.write_text(mods.read_text() + "pub mod " + family + ";\n")
except Exception:
    shutil.copy(bak / bp.name, bp); shutil.copy(bak / mods.name, mods)
    for f, o in external.items(): f.write_text(o)
    raise

def build():
    return subprocess.run(["cargo", "build", "-p", "pyawa-core"], capture_output=True, text=True)

r = build()
added = set()


STD_MAP = {
    "NonNull": "use core::ptr::NonNull;",
    "Cell": "use core::cell::Cell;",
    "RefCell": "use core::cell::RefCell;",
    "Ordering": "use core::sync::atomic::Ordering;",
    "AtomicU32": "use core::sync::atomic::AtomicU32;",
    "HashSet": "use std::collections::HashSet;",
    "HashMap": "use std::collections::HashMap;",
}


def _find_def(name):
    _sib = [f for f in sorted((bp.parent / bp.stem).glob("*.rs"))] if (bp.parent / bp.stem).is_dir() else []
    if re.search(r"^(?:pub(?:\(crate\))? )?(?:unsafe )?(?:fn|const|static|struct|enum|type|trait) " + re.escape(name) + r"\b", text, re.M):
        return "use crate::" + bp.stem + "::" + name + ";"
    for _f in _sib:
        if re.search(r"^(?:pub(?:\(crate\))? )?(?:unsafe )?(?:fn|const|static|struct|enum|type|trait) " + re.escape(name) + r"\b", _f.read_text(), re.M):
            return "use crate::" + bp.stem + "::" + name + ";"
    if (pathlib.Path("crates/pyawa-core/src") / (name + ".rs")).exists() or (pathlib.Path("crates/pyawa-core/src") / name / "mod.rs").exists():
        return "use crate::" + name + ";"
    if name in STD_MAP:                      # std/core 的名字不在 crate 里 ✓（第 144 轮）
        return STD_MAP[name]
    src_root = pathlib.Path("crates/pyawa-core/src")

    def path_of(f):
        parts = list(f.relative_to(src_root).with_suffix("").parts)
        if parts[-1] == "mod":
            parts = parts[:-1]
        if parts == ["lib"]:                 # lib.rs 就是 crate 根 ✓（第 145 轮：曾推成 crate::lib ✗）
            return "crate"
        return "crate::" + "::".join(parts)

    files = sorted(src_root.rglob("*.rs"))
    # 先找**定义处** ✓，再退到**再导出** ✓（第 145 轮：先撞到 lib.rs 的 `pub use` ✗ ⇒ 指到了不存在的 crate::lib）
    # 定义处**不要求 `pub`** ✓（第 146 轮：`enum Attribute` 是私有的 ✓）；找到就把它放宽成 pub(crate) ✓
    for f in files:
        txt = f.read_text()
        if re.search(r"^[ \t]*pub(?:\((?:crate|super)\))? (?:unsafe )?(?:struct|enum|type|trait|fn|const|static|union|mod) " + name + r"\b", txt, re.M):
            return "use " + path_of(f) + "::" + name + ";"
        m = re.search(r"^([ \t]*)(?:unsafe )?(struct|enum|type|trait|fn|const|static|union) " + name + r"\b", txt, re.M)
        if m:
            f.write_text(txt[:m.start()] + m.group(1) + "pub(crate) " + txt[m.start() + len(m.group(1)):])
            return "use " + path_of(f) + "::" + name + ";"
    for f in files:
        if re.search(r"^pub use [^;]*\b" + name + r"\b", f.read_text(), re.M):
            return "use " + path_of(f) + "::" + name + ";"
    return None


for _ in range(6):
    if r.returncode == 0:
        break
    names = set(re.findall(r"cannot find (?:type|value|function) `([A-Za-z_][A-Za-z_0-9]*)`", r.stderr))
    names |= set(re.findall(r"cannot find module or crate `([A-Za-z_][A-Za-z_0-9]*)`", r.stderr))
    names = {n for n in names if n not in added and n not in moved}
    if not names:
        break
    lines_cur = out.read_text().splitlines(keepends=True)
    last_use = max(i for i, l in enumerate(lines_cur) if l.startswith("use "))
    ins = [x for x in (_find_def(n) for n in sorted(names)) if x]
    added |= names
    out.write_text("".join(lines_cur[:last_use + 1]) + "".join(x + "\n" for x in ins) + "".join(lines_cur[last_use + 1:]))
    print("自愈补 import:", ins)
    r = build()

if r.returncode != 0 and not os.environ.get("SPLIT_KEEP"):
    print("编译失败 ⇒ 还原 ✓")
    shutil.copy(bak / bp.name, bp); shutil.copy(bak / mods.name, mods)
    out.unlink(missing_ok=True)
    for f, o in external.items(): f.write_text(o)
    print("\n".join([l for l in r.stderr.splitlines() if "error" in l or "-->" in l][:10])); sys.exit(1)
print("搬走 " + str(len(moved)) + " 个；" + str(out) + " " + str(len(new_mod.splitlines())) + " 行；被引用项 " + str(refs) + "；自愈补 " + str(sorted(added)))
