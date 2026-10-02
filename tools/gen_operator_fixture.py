#!/usr/bin/env python3
"""从**本机参照实现**导出 `operator` 的公开面与若干实测结果/消息（禁手写）。

命名与格式照 `tools/gen_itertools_fixture.py` 的成例：只导出 Rust 侧能直接用的常量。
"""
from __future__ import annotations

import operator
import pathlib

OUTPUT = pathlib.Path("crates/pyawa-stdlib/tests/fixtures/operator.rs")


def rust_bool(value: bool) -> str:
    """Rust 侧的字面量拼写（Python 的 `True`／`False` 在 Rust 里编不过）。"""
    return "true" if value else "false"


def message(call) -> str:
    try:
        call()
    except Exception as exc:  # noqa: BLE001 —— 探测就是要拿到消息
        return f"{type(exc).__name__}: {exc}"
    raise AssertionError("这次调用本该抛错，却成功了")


def main() -> int:
    names = sorted(name for name in dir(operator) if not name.startswith("_"))
    lines = [
        "//! 由 `tools/gen_operator_fixture.py` 从本机参照实现导出；**禁止手改**。",
        "",
        f"/// 参照实现里 `operator` 的非下划线公开名（共 {len(names)} 个）。",
        "pub static REFERENCE_NAMES: &[&str] = &[",
    ]
    lines += [f'    "{name}",' for name in names]
    lines += ["];", ""]
    # 第一刀（本段要实现的 6 个）的实测结果
    lines += [
        "/// 实测结果：`(函数名, 左, 右, 期望)`——`None` 表示只调一个实参。",
        "pub static RESULTS: &[(&str, i64, Option<i64>, i64)] = &[",
        f'    ("eq", 1, Some(1), {int(operator.eq(1, 1))}),',
        f'    ("eq", 1, Some(2), {int(operator.eq(1, 2))}),',
        f'    ("ne", 1, Some(1), {int(operator.ne(1, 1))}),',
        f'    ("ne", 1, Some(2), {int(operator.ne(1, 2))}),',
        f'    ("is_", 1, Some(1), {int(operator.is_(None, None))}),',
        f'    ("lt", 1, Some(2), {int(operator.lt(1, 2))}),',
        f'    ("lt", 2, Some(1), {int(operator.lt(2, 1))}),',
        f'    ("le", 1, Some(1), {int(operator.le(1, 1))}),',
        f'    ("ge", 1, Some(1), {int(operator.ge(1, 1))}),',
        f'    ("gt", 1, Some(2), {int(operator.gt(1, 2))}),',
        f'    ("add", 1, Some(2), {operator.add(1, 2)}),',
        f'    ("sub", 3, Some(1), {operator.sub(3, 1)}),',
        f'    ("mul", 2, Some(3), {operator.mul(2, 3)}),',
        '];',
        '',
        "/// 实测：单实参那几个（`truth`／`not_`）对 `0` 与 `1` 的结果。",
        f"pub const TRUTH_ZERO: bool = {rust_bool(operator.truth(0))};",
        f"pub const TRUTH_ONE: bool = {rust_bool(operator.truth(1))};",
        f"pub const NOT_ZERO: bool = {rust_bool(operator.not_(0))};",
        f"pub const NOT_ONE: bool = {rust_bool(operator.not_(1))};",
        '',
        "/// 实测：参数个数不对时的消息。",
        f'pub const REFERENCE_EQ_MISSING: &str = "{message(lambda: operator.eq(1))}";',
        f'pub const REFERENCE_TRUTH_TOO_MANY: &str = "{message(lambda: operator.truth(1, 2))}";',
        f'pub const REFERENCE_LT_NOT_SUPPORTED: &str = "{message(lambda: operator.lt(1, "a"))}";',
        f'pub const REFERENCE_LT_MISSING: &str = "{message(lambda: operator.lt(1))}";',
        f'pub const REFERENCE_ADD_NOT_SUPPORTED: &str = "{message(lambda: operator.add(1, "a"))}";',
        f'pub const REFERENCE_ADD_MISSING: &str = "{message(lambda: operator.add(1))}";',
        '',
    ]
    OUTPUT.write_text("\n".join(lines))
    print(f"导出 {len(names)} 个名字 → {OUTPUT}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
