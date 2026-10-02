#!/usr/bin/env python3
"""从参照实现**探测** errno 的平台数据与异常映射，生成：

- `crates/pyawa-runtime/src/platform_errno.rs`：**宿主平台**的 errno 数字（名字 → 值）
- `crates/pyawa-stdlib/src/errno_map.rs`：errno **名字** → `OSError` 子类名（`CM-19` 的表）
- `crates/pyawa-stdlib/tests/fixture-errno.json`：对拍夹具

`CM-19` 要求映射表**必须**由本机参照实现探测导出（**禁止凭记忆手写**）；
`CM-20` 要求模块暴露**宿主平台**的数字、映射**按名字**匹配（**禁止**把某平台的数字
硬编码进映射）。因此本脚本不写死任何常数：名字、数字、别名、`errorcode` 与异常映射
全部现取。换平台时重跑本脚本即可。

用法::

    python3 tools/gen_errno.py
"""

from __future__ import annotations

import errno
import json
import pathlib

ROOT = pathlib.Path(__file__).resolve().parents[1]
RUNTIME_OUT = ROOT / "crates/pyawa-runtime/src/platform_errno.rs"
MAP_OUT = ROOT / "crates/pyawa-stdlib/src/errno_map.rs"
FIXTURE = ROOT / "crates/pyawa-stdlib/tests/fixture-errno.json"


def collect() -> tuple[list[tuple[str, int]], dict[int, str], list[tuple[str, str]]]:
    """探测：常量（名字, 值）、`errorcode`（值 → 规范名）、异常映射（名字 → 异常类名）。"""
    names = sorted(
        name
        for name in dir(errno)
        if not name.startswith("_") and name.startswith("E") and name.isupper()
    )
    constants = [(name, int(getattr(errno, name))) for name in names]
    # `errorcode` 的键是数字、值是**规范名**（别名里只留一个——留哪个由探测给出，不猜）
    errorcode = {int(key): str(value) for key, value in errno.errorcode.items()}
    # 异常映射：`OSError(errno, ...)` 由参照实现自己挑子类 ⇒ 直接问它，不抄表
    mapping = []
    for name, value in constants:
        try:
            cls = type(OSError(value, "probe")).__name__
        except Exception:  # pragma: no cover - 参照实现不会失败
            cls = "OSError"
        mapping.append((name, cls))
    return constants, errorcode, mapping


def emit_runtime(constants: list[tuple[str, int]]) -> str:
    lines = [
        "//! **宿主平台**的 `errno` 数字（由 `tools/gen_errno.py` 探测参照实现生成）。",
        "//!",
        "//! `CM-20`：`errno` 模块**必须**暴露宿主平台的数字，映射**按名字**匹配。",
        "//! 换平台时重跑生成脚本；**禁止**手改本文件。",
        "",
        "/// 宿主平台的 errno 常量（名字 → 值，按名字排序）。",
        "pub const HOST_ERRNO: &[(&str, i64)] = &[",
    ]
    for name, value in constants:
        lines.append(f'    ("{name}", {value}),')
    lines.append("];")
    lines.append("")
    return "\n".join(lines)


def emit_map(
    constants: list[tuple[str, int]],
    errorcode: dict[int, str],
    mapping: list[tuple[str, str]],
) -> str:
    lines = [
        "//! errno → `OSError` 子类的映射（由 `tools/gen_errno.py` 探测参照实现生成）。",
        "//!",
        "//! `CM-5`：机器错误 → 对应 Python 异常；`CM-19`：表**必须**由探测导出；",
        "//! `CM-20`：**按名字**匹配（数字随平台，禁止硬编码进映射）。**禁止**手改本文件。",
        "",
        "/// `CM-19` 的映射表：errno **名字** → 异常类名（按名字排序）。",
        "/// 表外的 errno 一律落 `OSError`（`CM-19`）。",
        "pub const ERRNO_TO_CLASS: &[(&str, &str)] = &[",
    ]
    for name, cls in mapping:
        lines.append(f'    ("{name}", "{cls}"),')
    lines.append("];")
    lines.append("")
    lines.append("/// `errno.errorcode` 的内容（值 → 规范名；别名只留探测给出的那一个）。")
    lines.append("pub const ERRNO_ERRORCODE: &[(i64, &str)] = &[")
    for value, name in sorted(errorcode.items()):
        lines.append(f'    ({value}, "{name}"),')
    lines.append("];")
    lines.append("")
    return "\n".join(lines)


def main() -> int:
    constants, errorcode, mapping = collect()
    RUNTIME_OUT.write_text(emit_runtime(constants), encoding="utf-8")
    MAP_OUT.write_text(emit_map(constants, errorcode, mapping), encoding="utf-8")
    FIXTURE.write_text(
        json.dumps(
            {
                "constants": [{"name": n, "value": v} for n, v in constants],
                "errorcode": {str(k): v for k, v in sorted(errorcode.items())},
                "errno_to_class": [{"name": n, "class": c} for n, c in mapping],
            },
            ensure_ascii=False,
            indent=2,
            sort_keys=False,
        )
        + "\n",
        encoding="utf-8",
    )
    distinct = sorted({cls for _, cls in mapping})
    print(f"常量 {len(constants)} 个；errorcode {len(errorcode)} 条；异常类 {len(distinct)} 种")
    print("异常类：" + "、".join(distinct))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
