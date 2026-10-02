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

    # ---- `TS-45` ①：`int`↔`str` 的位数上限（`sys` 侧的两个入口）----
    #
    # 转换本身的**消息**归 `tools/gen_int_fixture.py`（那是 `int` 的行为）；
    # 这里只记 `sys` 的 API 面：默认值、阈值与五种非法形态的**实测消息**。

    def error_of(operation) -> str | None:
        try:
            operation()
            return None
        except Exception as error:  # 探测：把异常原样记下来
            return f"{type(error).__name__}: {error}"

    def rust_string(text: str | None) -> str:
        if text is None:
            return "None"
        escaped = text.replace("\\", "\\\\").replace('"', '\\"')
        return f'Some("{escaped}")'

    # ---- `float_info`／`int_info`（第 216 轮按 §5.2.3 的新口径落地）----
    #
    # `float_info`：我们与参照**同一个物**（IEEE-754 `f64`）⇒ 逐字段值必须相同；
    # `int_info`：`bits_per_digit`／`sizeof_digit` 是**实现观测面**（参照的 30／4 描述的是
    # **它内部**的表示，我们的表示是实现自选）⇒ 夹具只记参照值，用来断言"我们**没有**照抄"；
    # 两个位数上限（`TS-45` 的 4300 与参照实测的 640）**必须**一致。
    float_fields = sorted(
        name for name in dir(sys.float_info) if not name.startswith("_") and not callable(getattr(sys.float_info, name))
    )
    int_info = sys.int_info

    default_limit = sys.get_int_max_str_digits()
    threshold = sys.int_info.str_digits_check_threshold
    below = error_of(lambda: sys.set_int_max_str_digits(threshold - 1))
    negative = error_of(lambda: sys.set_int_max_str_digits(-1))
    not_integer = error_of(lambda: sys.set_int_max_str_digits("x"))
    huge = error_of(lambda: sys.set_int_max_str_digits(2**40))
    no_args = error_of(lambda: sys.set_int_max_str_digits())
    two_args = error_of(lambda: sys.set_int_max_str_digits(1000, 2000))
    get_with_args = error_of(lambda: sys.get_int_max_str_digits(1))
    setting_zero = error_of(lambda: sys.set_int_max_str_digits(0))
    zero_means_unlimited = sys.get_int_max_str_digits() == 0
    sys.set_int_max_str_digits(default_limit)

    lines += [
        "/// 参照实现的 `sys.float_info` 逐字段（`repr`；我们就是 IEEE-754 `f64` ⇒ 值**必须**相同）。",
        "pub static REFERENCE_FLOAT_INFO: &[(&str, &str)] = &[",
    ]
    for name in float_fields:
        lines.append(f'    ("{name}", "{getattr(sys.float_info, name)!r}"),')
    lines += [
        "];",
        "",
        "/// 参照实现的 `sys.int_info.bits_per_digit`——**实现观测面**：参照内部是 2^30 进制（30），",
        "/// 我们内部是 2^32 进制 ⇒ 本值用来断言我们**没有**照抄参照（`MS-17`）。",
        f"pub const REFERENCE_INT_INFO_BITS_PER_DIGIT: i64 = {int_info.bits_per_digit};",
        "",
        "/// 参照实现的 `sys.int_info.sizeof_digit`（实现观测面；参照为 4）。",
        f"pub const REFERENCE_INT_INFO_SIZEOF_DIGIT: i64 = {int_info.sizeof_digit};",
        "",
        "/// 参照实现的 `sys.int_info.default_max_str_digits`（`TS-45`：**必须**一致）。",
        f"pub const REFERENCE_INT_INFO_DEFAULT_MAX_STR_DIGITS: i64 = {int_info.default_max_str_digits};",
        "",
        "/// 参照实现的 `sys.int_info.str_digits_check_threshold`（**必须**一致）。",
        f"pub const REFERENCE_INT_INFO_STR_DIGITS_THRESHOLD: i64 = {int_info.str_digits_check_threshold};",
        "",
        "/// 参照实现的 `sys.get_int_max_str_digits()` 默认值（`TS-45` ①）。",
        f"pub const REFERENCE_INT_MAX_STR_DIGITS: i64 = {default_limit};",
        "",
        "/// 参照实现的 `sys.int_info.str_digits_check_threshold`（`set_` 允许的最小非零值）。",
        f"pub const REFERENCE_STR_DIGITS_THRESHOLD: i64 = {threshold};",
        "",
        "/// `sys.set_int_max_str_digits(threshold - 1)` 的实测消息。",
        f"pub const REFERENCE_SET_BELOW_MESSAGE: Option<&str> = {rust_string(below)};",
        "",
        "/// `sys.set_int_max_str_digits(-1)` 的实测消息（与上面同一句）。",
        f"pub const REFERENCE_SET_NEGATIVE_MESSAGE: Option<&str> = {rust_string(negative)};",
        "",
        "/// `sys.set_int_max_str_digits('x')` 的实测消息。",
        f"pub const REFERENCE_SET_NOT_INTEGER_MESSAGE: Option<&str> = {rust_string(not_integer)};",
        "",
        "/// `sys.set_int_max_str_digits(2**40)` 的实测消息（超出 C `int`）。",
        f"pub const REFERENCE_SET_HUGE_MESSAGE: Option<&str> = {rust_string(huge)};",
        "",
        "/// `sys.set_int_max_str_digits()` 少给实参的实测消息。",
        f"pub const REFERENCE_SET_NO_ARGS_MESSAGE: Option<&str> = {rust_string(no_args)};",
        "",
        "/// `sys.set_int_max_str_digits(1000, 2000)` 多给实参的实测消息。",
        f"pub const REFERENCE_SET_TWO_ARGS_MESSAGE: Option<&str> = {rust_string(two_args)};",
        "",
        "/// `sys.get_int_max_str_digits(1)` 给了实参的实测消息。",
        f"pub const REFERENCE_GET_WITH_ARGS_MESSAGE: Option<&str> = {rust_string(get_with_args)};",
        "",
        "/// `sys.set_int_max_str_digits(0)` 是否成功（`0` ＝ 不限）。",
        f"pub const REFERENCE_SETTING_ZERO_SUCCEEDS: bool = {str(setting_zero is None).lower()};",
        "",
        "/// `set_(0)` 之后 `get_()` 是否报 `0`。",
        f"pub const REFERENCE_ZERO_MEANS_UNLIMITED: bool = {str(zero_means_unlimited).lower()};",
        "",
    ]

    FIXTURE.write_text("\n".join(lines))
    print(
        f"已写入 {FIXTURE.relative_to(ROOT)}（参照 {sys.version.split()[0]}，"
        f"version_info={tuple(sys.version_info)!r}，byteorder={sys.byteorder!r}）"
    )


if __name__ == "__main__":
    main()
