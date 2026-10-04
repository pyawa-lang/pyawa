#!/usr/bin/env python3
"""夹具用例的**一致性守卫**（第 109 轮立；缘由：第 105–107 轮我把条目加到了 `SOURCES` 之外 ✗，
语法合法但完全惰性 ⇒ 三轮"全量比对绿灯"其实是**空跑** ✗）。

三条判据（任一不成立 ⇒ 退出码 1，并打印证据）：

1. **文件里每一条用例都在 `SOURCES` 之内**：任何 `(str, bool, str)` 形状的三元组，其行号必须落在
   `SOURCES = [ … ]` 的行区间里 ✓（这条正是当年能当场抓住那个 bug 的那条 ✓）；
2. **`SOURCES` 与产物 JSON 的用例集合完全一致**（不多不少 ✓）；
3. **`covered=True` 的条目都在 JSON 里且 `covered=True`** ✓。

用法：`python3 tools/check_fixture_cases.py`（`gen_compile_fixture.py --emit` 末尾也会调它 ✓）。
"""
from __future__ import annotations

import ast
import json
import pathlib
import sys

ROOT = pathlib.Path(__file__).resolve().parents[1]
GENERATOR = ROOT / "tools" / "gen_compile_fixture.py"
OUTPUT = ROOT / "crates" / "pyawa-core" / "tests" / "fixture-compile-3.14.json"


def main() -> int:
    source = GENERATOR.read_text(encoding="utf-8")
    tree = ast.parse(source)

    sources_node = None
    for node in tree.body:
        if isinstance(node, ast.Assign) and getattr(node.targets[0], "id", "") == "SOURCES":
            sources_node = node.value
    if sources_node is None:
        print("✗ 找不到 `SOURCES`（生成器结构变了，守卫需要更新 ✓）")
        return 1

    low = sources_node.lineno
    high = max(getattr(element, "end_lineno", element.lineno) for element in sources_node.elts)
    # 只认**字面量**三元组；程序拼的（`BinOp` 等）交给 `GENERATED_SOURCES` 那条路 ✓
    cases_in_list = {
        element.elts[0].value: element.elts[1].value
        for element in sources_node.elts
        if isinstance(element, ast.Tuple)
        and len(element.elts) == 3
        and all(isinstance(part, ast.Constant) for part in element.elts)
        and isinstance(element.elts[0].value, str)
        and isinstance(element.elts[1].value, bool)
    }

    # 判据 1：文件里任何同形状的三元组都必须在 SOURCES 行区间内
    stray: list[tuple[int, str]] = []
    for node in ast.walk(tree):
        if not isinstance(node, ast.Tuple) or len(node.elts) != 3:
            continue
        if not all(isinstance(part, ast.Constant) for part in node.elts):
            continue
        if not isinstance(node.elts[0].value, str) or not isinstance(node.elts[1].value, bool):
            continue
        if not (low <= node.lineno <= high):
            stray.append((node.lineno, node.elts[0].value))
    if stray:
        print("✗ 有下列用例在 `SOURCES` **之外**（它们永远不会被生成 ⇒ 空跑 ✗）：")
        for line, text in stray:
            print(f"    第 {line} 行：{text!r}")
        print(f"  `SOURCES` 的行区间是 {low}–{high} ✓")
        return 1

    # 判据 2/3：与产物 JSON 完全一致
    payload = json.loads(OUTPUT.read_text(encoding="utf-8"))
    produced = payload["cases"] if "cases" in payload else payload
    # **程序生成的用例**（`GENERATED_SOURCES` ✓，第 121 轮）：把它们并进预期集合 ✓
    # 用导入的方式取，避免"手抄一遍"变成第二个真相 ✓。
    import importlib.util

    spec = importlib.util.spec_from_file_location("_gen_fixture", GENERATOR)
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    expected = set(cases_in_list)
    for source, covered, _ in getattr(module, "GENERATED_SOURCES", []):
        expected.add(source)
        cases_in_list[source] = covered
    if set(produced) != expected:
        only_json = sorted(set(produced) - expected)
        only_source = sorted(expected - set(produced))
        print("✗ `SOURCES` 与 JSON 的用例集合不一致：")
        for text in only_json[:5]:
            print(f"    只在 JSON：{text!r}")
        for text in only_source[:5]:
            print(f"    只在 SOURCES：{text!r}")
        return 1
    mismatched = [
        text
        for text, covered in cases_in_list.items()
        if covered and not produced[text].get("covered")
    ]
    if mismatched:
        print(f"✗ 下列条目在 `SOURCES` 里标了 `covered=True`，JSON 里却不是：{mismatched[:5]}")
        return 1

    print(f"✓ 夹具守卫：{len(cases_in_list)} 条用例全部在 `SOURCES` 内、与 JSON 一致 ✓")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
