#!/usr/bin/env python3
"""从参照实现**探测** `sys` 的可观测面，生成：

- `crates/pyawa-stdlib/tests/fixtures/sys.rs`：对拍夹具

`CX-13` 要求 `sys.implementation.name` **必须**报 `pyawa`（不得伪装 CPython）；
`DESIGN.md` §9 把 `sys.version`／`sys.implementation` 列为**实现观测面**（"必然不同"）。
所以夹具里同时记下**参照实现的值**与**本实现的规则**：

- **必须一致**的：`version_info`／`hexversion`（语言级别——库用它做特性检测）、
  `maxunicode`／`maxsize`／`byteorder`（与实现无关的常量）
- **必须不同**的：`implementation.name`／`cache_tag`、`version` 串（身份项）

夹具生成 **Rust** 而不是 JSON：`pyawa-stdlib` 的测试里没有 JSON 解析器
（与 `fixtures/builtins.rs`、`fixtures/unicode.rs` 同一取舍）。

用法::

    python3 tools/gen_sys_fixture.py
"""

from __future__ import annotations

import pathlib
import sys

ROOT = pathlib.Path(__file__).resolve().parents[1]
FIXTURE = ROOT / "crates/pyawa-stdlib/tests/fixtures/sys.rs"


def main() -> None:
    implementation = sys.implementation
    lines = [
        "//! 由 `tools/gen_sys_fixture.py` 探测参照实现导出；**禁止手改**。",
        f"//! 参照实现：{sys.version.split()[0]}（`sys.version_info` ＝ {tuple(sys.version_info)!r}）",
        "//!",
        "//! 生成 Rust 而不是 JSON：`pyawa-stdlib` 的测试里没有 JSON 解析器",
        "//! （与 `fixtures/builtins.rs`、`fixtures/unicode.rs` 同一取舍）。",
        "",
        "/// 参照实现的 `sys.version_info`（**语言级别**：Pyawa 必须报同一组数，供特性检测）。",
        f"pub const REFERENCE_VERSION_INFO: (u32, u32, u32, &str, u32) = "
        f"({sys.version_info[0]}, {sys.version_info[1]}, {sys.version_info[2]}, "
        f'"{sys.version_info[3]}", {sys.version_info[4]});',
        "",
        f"/// 参照实现的 `sys.hexversion`（＝ 上一条的整数编码）。",
        f"pub const REFERENCE_HEXVERSION: i64 = {sys.hexversion};",
        "",
        f"/// 参照实现的 `sys.maxunicode`。",
        f"pub const REFERENCE_MAXUNICODE: i64 = {sys.maxunicode};",
        "",
        f"/// 参照实现的 `sys.maxsize`。",
        f"pub const REFERENCE_MAXSIZE: i64 = {sys.maxsize};",
        "",
        f'/// 参照实现的 `sys.byteorder`（宿主端序）。',
        f'pub const REFERENCE_BYTEORDER: &str = "{sys.byteorder}";',
        "",
        '/// 参照实现的 `sys.implementation.name`——Pyawa **必须**报 `pyawa`，'
        "它与本值**必须不同**（`CX-13`）。",
        f'pub const REFERENCE_IMPLEMENTATION_NAME: &str = "{implementation.name}";',
        "",
        "/// 参照实现的 `sys.implementation.cache_tag`——Pyawa 用自己的值，与此**必须不同**。",
        f'pub const REFERENCE_CACHE_TAG: &str = "{implementation.cache_tag}";',
        "",
        "/// 参照实现的 `sys.version` 串——Pyawa 的构建串**必须含 `pyawa`**，与此**必须不同**。",
        f'pub const REFERENCE_VERSION: &str = "{sys.version}";',
        "",
    ]
    FIXTURE.write_text("\n".join(lines))
    print(
        f"已写入 {FIXTURE.relative_to(ROOT)}（参照 {sys.version.split()[0]}，"
        f"version_info={tuple(sys.version_info)!r}，byteorder={sys.byteorder!r}）"
    )


if __name__ == "__main__":
    main()
