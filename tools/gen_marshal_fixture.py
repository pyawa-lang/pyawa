#!/usr/bin/env python3
"""从参照实现**探测** `marshal` 的可观测面，导出 `crates/pyawa-stdlib/tests/fixtures/marshal.rs`。

**`CM-27`** 说得很清楚：`marshal` **必须存在且自洽**（**自有**二进制格式 ＋ 自己的版本号），
`loads(dumps(x))` 必须往返；**不追**与参照的字节兼容。所以这个夹具只记**我们也要照做的**那些
可观测事实（版本号是"参照的值"、错误消息、哪些类型往返得动），不记参照的字节流。

生成 Rust 而不是 JSON：`pyawa-stdlib` 的测试里没有 JSON 解析器（与 `gen_sys_fixture.py` 同款）。

用法::

    python3 tools/gen_marshal_fixture.py           # 只打印摘要
    python3 tools/gen_marshal_fixture.py --emit     # 写出夹具
"""

from __future__ import annotations

import marshal
import pathlib
import sys

ROOT = pathlib.Path(__file__).resolve().parents[1]
FIXTURE = ROOT / "crates/pyawa-stdlib/tests/fixtures/marshal.rs"

#: 参与"往返得动吗"探测的值（**不**含 code object：那要另一套字段，见 `CM-27` 的"存在且自洽"）。
VALUES: list[tuple[str, object]] = [
    ("none", None),
    ("true", True),
    ("false", False),
    ("int_small", 42),
    ("int_negative", -7),
    ("int_i64_max", 2**63 - 1),
    ("int_big", 2**100),
    ("float", 1.5),
    ("float_nan", float("nan")),
    ("str_ascii", "abc"),
    ("str_unicode", "café"),
    ("bytes", b"\x00\xff"),
    ("tuple", (1, 2)),
    ("list", [1, "a"]),
    ("dict", {"a": 1}),
    ("set", {1, 2}),
    ("nested", {"k": [1, (2, b"x")]}),
]


def error_of(operation) -> str | None:
    try:
        operation()
        return None
    except Exception as error:  # 探测：原样记下来
        return f"{type(error).__name__}: {error}"


def main() -> int:
    emit = "--emit" in sys.argv
    round_trips = []
    for name, value in VALUES:
        try:
            encoded = marshal.dumps(value)
            decoded = marshal.loads(encoded)
            equal = repr(decoded) == repr(value) or (
                isinstance(value, float) and value != value and decoded != decoded
            )
            round_trips.append({"name": name, "ok": bool(equal), "length": len(encoded)})
        except Exception as error:
            round_trips.append({"name": name, "ok": False, "error": f"{type(error).__name__}: {error}"})

    circular = []
    cyclic_list: list[object] = []
    cyclic_list.append(cyclic_list)
    circular.append({"name": "list", "error": error_of(lambda: marshal.dumps(cyclic_list))})
    cyclic_dict: dict[str, object] = {}
    cyclic_dict["self"] = cyclic_dict
    circular.append({"name": "dict", "error": error_of(lambda: marshal.dumps(cyclic_dict))})

    errors = {
        "empty_loads": error_of(lambda: marshal.loads(b"")),
        "garbage_loads": error_of(lambda: marshal.loads(b"not marshal data")),
        "truncated": error_of(lambda: marshal.loads(marshal.dumps([1, 2, 3])[:-1])),
        "loads_bad_type": error_of(lambda: marshal.loads(b"\x7f")),
    }

    lines = [
        "//! 由 `tools/gen_marshal_fixture.py` 探测参照实现导出；**禁止手改**。",
        f"//! 参照实现：{sys.version.split()[0]}",
        "//!",
        "//! `CM-27`：`marshal` **自有**格式、`loads(dumps(x))` 必须往返、**不追**字节兼容。",
        "//! 所以这里记的是**参照的可观测面**（版本号、错误消息、哪些类型往返得动），",
        "//! **不是**参照的字节流。",
        "",
        "/// 参照实现的 `marshal.version`（**它的**格式版本）。",
        "///",
        "/// **`CM-27`**：Pyawa 的 marshal 用自己的版本号 ⇒ 与本值**必须不同**才算守住「自有格式」。",
        f"pub const REFERENCE_VERSION: i64 = {marshal.version};",
        "",
        "/// 循环引用：参照实测的消息（`CM-27` 没要求逐字相同，但我们照它报）。",
        f'pub const REFERENCE_CIRCULAR_MESSAGE: Option<&str> = {rust_option(circular[0]["error"])};',
        "",
        "/// `loads(b'')` 的实测消息。",
        f'pub const REFERENCE_EMPTY_MESSAGE: Option<&str> = {rust_option(errors["empty_loads"])};',
        "",
        "/// `loads(b'not marshal data')` 的实测消息。",
        f'pub const REFERENCE_GARBAGE_MESSAGE: Option<&str> = {rust_option(errors["garbage_loads"])};',
        "",
        "/// 被截断的输入的实测消息。",
        f'pub const REFERENCE_TRUNCATED_MESSAGE: Option<&str> = {rust_option(errors["truncated"])};',
        "",
        "/// 未知类型码的实测消息。",
        f'pub const REFERENCE_BAD_TYPE_MESSAGE: Option<&str> = {rust_option(errors["loads_bad_type"])};',
        "",
        "/// 参照实测**往返得动**的类型名（我们的义务面至少覆盖这些）。",
        "pub const REFERENCE_ROUND_TRIP: &[&str] = &[",
    ]
    for row in round_trips:
        if row["ok"]:
            lines.append(f'    "{row["name"]}",')
    lines += [
        "];",
        "",
    ]
    FIXTURE.write_text("\n".join(lines))
    ok = [row["name"] for row in round_trips if row["ok"]]
    failed = [(row["name"], row.get("error")) for row in round_trips if not row["ok"]]
    print(f"版本 {marshal.version} · 往返得动 {len(ok)}/{len(VALUES)} 个值")
    if failed:
        print("往返失败（照实记下来）：")
        for name, error in failed:
            print(f"  {name}: {error}")
    if emit:
        print(f"导出 → {FIXTURE.relative_to(ROOT)}")
    return 0


def rust_option(text: str | None) -> str:
    if text is None:
        return "None"
    escaped = text.replace("\\", "\\\\").replace('"', '\\"')
    return f'Some("{escaped}")'


if __name__ == "__main__":
    raise SystemExit(main())
