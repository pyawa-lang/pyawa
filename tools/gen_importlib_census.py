#!/usr/bin/env python3
"""普查：**引导路径上的三个上游文件**需要哪些模块（`_bootstrap`／`_bootstrap_external`／`site`）。

用途（`docs/PLAN-milestones.md` §9.4 前置链的下一环"让 `importlib` 能在 VM 里跑"）：把"缺什么"
变成一张**可复现**的表，而不是印象。参照源取本机 CPython（与 `REQUIREMENTS.md` 同版本 ✓），
只读、不入库（`CX-8`：`Lib/` 一律到 M3 才引入）。

分类口径（**由参照实现自己决定**，不靠手写名单）：
  · 纯 Python：`<root>/<mod>.py` 或 `<root>/<mod>/__init__.py` 存在；
  · 否则：C 模块 / 冻结模块（`sys`、`builtins`、`_io`、`_imp`、`_warnings`、`posix` …）。

用法：`python3 tools/gen_importlib_census.py [--root <stdlib 目录>] [--out <tsv>]`
"""
from __future__ import annotations

import argparse
import pathlib
import re
import sys
import sysconfig

FILES = ("importlib/_bootstrap.py", "importlib/_bootstrap_external.py", "site.py")

# Pyawa 侧已实现的 C 模块（`crates/pyawa-stdlib/src/*_module.rs`；**本脚本自己的一份清单** ✓）
IMPLEMENTED = {
    "sys": "sys_module.rs",
    "builtins": "builtins_module.rs",
    "_io": "_io_module.rs",
    "_imp": "imp_module.rs",
    "errno": "errno_module.rs",
    "marshal": "marshal_module.rs",
    "itertools": "itertools_module.rs",
    "operator": "operator_module.rs",
}

# **必须整行像一条真的导入语句**：`_bootstrap.py` 的散文里也有 "import implementation is
# desired." ✗ ⇒ 宽松正则会咬出假阳性（第一版实测两条 ✓）。两条正则分别管 `import a, b as c`
# 与 `from a.b import c` ✓。
IMPORT_RE = re.compile(r"^\s*import\s+([A-Za-z_][A-Za-z0-9_]*(?:\.[A-Za-z_][A-Za-z0-9_]*)*)(?:\s+as\s+\w+)?\s*(?:,.*?)?\s*(?:#.*)?$")
FROM_RE = re.compile(r"^\s*from\s+([A-Za-z_][A-Za-z0-9_]*(?:\.[A-Za-z_][A-Za-z0-9_]*)*)\s+import\b")
MULTI_RE = re.compile(r"^(?:import\s+([A-Za-z_][A-Za-z0-9_]*(?:\.[A-Za-z_][A-Za-z0-9_]*)*)(?:\s+as\s+\w+)?\s*,\s*)+")


def module_root(name: str) -> str:
    return name.split(".")[0]


def is_pure_python(root: pathlib.Path, module: str) -> bool:
    return (root / f"{module}.py").is_file() or (root / module / "__init__.py").is_file()


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--root", default=sysconfig.get_paths()["stdlib"])
    parser.add_argument("--out", default="tools/importlib-bootstrap-census.tsv")
    options = parser.parse_args()
    root = pathlib.Path(options.root)

    rows: dict[tuple[str, str], list[str]] = {}
    for relative in FILES:
        path = root / relative
        if not path.is_file():
            print(f"缺文件：{path}", file=sys.stderr)
            return 1
        for number, line in enumerate(path.read_text(encoding="utf-8", errors="replace").splitlines(), 1):
            match = IMPORT_RE.match(line) or FROM_RE.match(line)
            if not match:
                continue
            module = module_root(match.group(1))
            if module.startswith("_frozen_"):
                # **冻结模块**：那正是本文件自己在 `sys.modules` 里的名字 ⇒ 不是"缺的依赖" ✓
                kind = "冻结（自指）"
            elif is_pure_python(root, module):
                kind = "纯 Python"
            else:
                kind = "C／冻结"
            rows.setdefault((kind, module), []).append(f"{relative}:{number}")

    lines = ["# 引导路径的模块需求（生成器：tools/gen_importlib_census.py；参照源只读，不入库 ✓）",
             "# 列：类别\t模块\tPyawa 现状\t出现的处数\t出处"]
    implemented = pure = cmodules = 0
    for (kind, module), places in sorted(rows.items()):
        state = IMPLEMENTED.get(module, "—")
        if kind == "纯 Python":
            pure += 1
        elif kind == "冻结（自指）":
            pass
        else:
            cmodules += 1
            if module in IMPLEMENTED:
                implemented += 1
        lines.append(f"{kind}\t{module}\t{state}\t{len(places)}\t{','.join(places[:4])}")
    pathlib.Path(options.out).write_text("\n".join(lines) + "\n", encoding="utf-8")

    print(f"参照源：{root}（{len(FILES)} 个文件）")
    frozen = sum(1 for (k, _) in rows if k == "冻结（自指）")
    print(f"纯 Python 依赖：{pure} 个；C／冻结依赖：{cmodules} 个（其中 Pyawa 已有 {implemented} 个）；冻结自指：{frozen} 个")
    missing = sorted(m for (k, m) in rows if k == "C／冻结" and m not in IMPLEMENTED)
    print("缺的 C／冻结依赖：" + "、".join(missing))
    print(f"写出：{options.out}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
