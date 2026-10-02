#!/usr/bin/env python3
"""从参照实现**探测** `_imp` 的可观测面，生成：

- `crates/pyawa-stdlib/tests/fixtures/imp.rs`：对拍夹具

`SPEC-c-modules.md` §6 写明 `_imp` **必须**提供 `pyc_magic_number_token`（`_bootstrap_external.py`
用它算 `MAGIC_NUMBER`，取低 16 位），且**其值由 Pyawa 自定**。所以夹具记的是**参照的值与形状**
（用来断言"我们必须不同"），以及 `is_builtin` 的三态取值与 `_imp` 的公开名字清单。

夹具生成 **Rust** 而不是 JSON（与其它夹具同一取舍：`pyawa-stdlib` 的测试里没有 JSON 解析器）。

用法::

    python3 tools/gen_imp_fixture.py
"""

from __future__ import annotations

import _imp
import pathlib

ROOT = pathlib.Path(__file__).resolve().parents[1]
FIXTURE = ROOT / "crates/pyawa-stdlib/tests/fixtures/imp.rs"


def main() -> None:
    token = _imp.pyc_magic_number_token
    names = sorted(name for name in dir(_imp) if not name.startswith("__"))
    lines = [
        "//! 由 `tools/gen_imp_fixture.py` 探测参照实现导出；**禁止手改**。",
        "//!",
        "//! 生成 Rust 而不是 JSON：`pyawa-stdlib` 的测试里没有 JSON 解析器"
        "（与其它夹具同一取舍）。",
        "",
        "/// 参照实现的 `pyc_magic_number_token`——**Pyawa 必须用自定值**，与此不同。",
        f"pub const REFERENCE_PYC_MAGIC_NUMBER_TOKEN: i64 = {token};",
        "",
        "/// 参照实现 `pyc_magic_number_token` 的**低 16 位**"
        "（`_bootstrap_external` 算 `MAGIC_NUMBER` 用它）。",
        f"pub const REFERENCE_MAGIC_LOW_16: i64 = {token & 0xFFFF};",
        "",
        "/// 参照实现里 `is_builtin('sys')` 的三态值（`-1` ＝ 内建）。",
        f"pub const REFERENCE_IS_BUILTIN_SYS: i64 = {_imp.is_builtin('sys')};",
        "",
        "/// 参照实现里 `is_builtin('nope')` 的值（`0` ＝ 不是内建）。",
        f"pub const REFERENCE_IS_BUILTIN_UNKNOWN: i64 = {_imp.is_builtin('nope')};",
        "",
        "/// 参照实现的 `_imp.__doc__`（与参照同源的一句话）。",
        f'pub const REFERENCE_DOC: &str = "{_imp.__doc__}";',
        "",
        "/// 参照实现的公开名字清单（本层只落地其中一小部分，逐条见 §5.2.4）。",
        "pub static REFERENCE_NAMES: &[&str] = &[",
    ]
    for name in names:
        lines.append(f'    "{name}",')
    lines.append("];")
    lines.append("")
    FIXTURE.write_text("\n".join(lines))
    print(
        f"已写入 {FIXTURE.relative_to(ROOT)}（token={token}（低 16 位 {token & 0xFFFF:#x}）、"
        f"公开名 {len(names)} 个）"
    )


if __name__ == "__main__":
    main()
