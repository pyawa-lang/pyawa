#!/usr/bin/env python3
"""**§12.17 A1/A3**：从 Rust `repr(C)` 形状生成 `pa_capabilities.h` 与布局探针（单一方向生成）。

- 输入：`crates/pyawa-capabilities/src/{fs,clock,net}.rs` 的 `#[repr(C)]` 结构体/枚举；
- 输出：`crates/pyawa-abi/include/pa_capabilities.h`（3 域 vtable ＋ 嵌套结构 ＋ `pa_cap_status`）
  与 `crates/pyawa-abi/tests/capability_layout.rs`（打印 Rust 尺寸/偏移，供 C↔Rust 对照）；
- `--check`：重新生成并与仓库文件逐字节比较（漂移即红；A3 的"故意改字段必红"）；
- `--emit-c-probe PATH`：生成 C 探针源码（打印同一口徑的尺寸/偏移），供 A3 机检编译运行。

**为什么生成而不是手抄**：字段名/顺序/类型只该有一处真相（Rust `repr(C)`）；手抄漂移
（改名/漏字段/换类型）正是本单要消灭的冻结前风险。C 侧仍由**编译器**验证布局与探针一致。
"""

from __future__ import annotations

import argparse
import pathlib
import re
import sys

ROOT = pathlib.Path(__file__).resolve().parents[1]
CAP = ROOT / "crates" / "pyawa-capabilities" / "src"
HEADER = ROOT / "crates" / "pyawa-abi" / "include" / "pa_capabilities.h"
RUST_TEST = ROOT / "crates" / "pyawa-abi" / "tests" / "capability_layout.rs"

# Rust struct -> C 名（嵌套结构先于 vtable；顺序即输出顺序）
STRUCTS = [
    ("fs", "Handle", "pa_cap_handle"),
    ("net", "SocketHandle", "pa_cap_socket_handle"),
    ("fs", "FileInfo", "pa_cap_file_info"),
    ("fs", "FileTimes", "pa_cap_file_times"),
    ("clock", "CpTm", "pa_cap_tm"),
    ("net", "PollFd", "pa_cap_poll_fd"),
    ("fs", "CpFsVtable", "pa_fs_vtable"),
    ("clock", "CpClockVtable", "pa_clock_vtable"),
    ("net", "CpNetVtable", "pa_net_vtable"),
]
SOURCE_LABEL = {"fs": "fs", "clock": "clock", "net": "net"}
BASE_TYPES = {
    "u8": "uint8_t",
    "u16": "uint16_t",
    "u32": "uint32_t",
    "u64": "uint64_t",
    "i32": "int32_t",
    "i64": "int64_t",
    "usize": "size_t",
    "bool": "bool",
    "c_void": "void",
}
NAMED = {
    "Handle": "pa_cap_handle",
    "SocketHandle": "pa_cap_socket_handle",
    "FileInfo": "pa_cap_file_info",
    "FileTimes": "pa_cap_file_times",
    "CpTm": "pa_cap_tm",
    "PollFd": "pa_cap_poll_fd",
    "CapStatus": "pa_cap_status",
}


def strip_comments(text: str) -> str:
    text = re.sub(r"/\*.*?\*/", "", text, flags=re.S)
    text = re.sub(r"//[^\n]*", "", text)
    return text


def balanced(text: str, start: int, open_ch: str, close_ch: str) -> int:
    depth = 0
    i = start
    while i < len(text):
        c = text[i]
        if c == open_ch:
            depth += 1
        elif c == close_ch:
            depth -= 1
            if depth == 0:
                return i
        i += 1
    raise ValueError("unbalanced")


def split_top(text: str, sep: str = ",") -> list[str]:
    """按顶层分隔符切；跟踪 ()[]{}<>（`->` 的 `>` 不算）。"""
    out: list[str] = []
    cur: list[str] = []
    depth = 0
    i = 0
    while i < len(text):
        c = text[i]
        if c in "([{":
            depth += 1
        elif c in ")]}":
            depth -= 1
        elif c == "<":
            depth += 1
        elif c == ">":
            if i > 0 and text[i - 1] == "-":
                pass  # `->`
            else:
                depth -= 1
        if c == sep and depth == 0:
            out.append("".join(cur))
            cur = []
        else:
            cur.append(c)
        i += 1
    if cur:
        out.append("".join(cur))
    return out


def extract_struct(source: str, name: str) -> list[tuple[str, str]]:
    m = re.search(rf"pub\s+struct\s+{re.escape(name)}\s*\{{", source)
    if not m:
        # tuple 结构（`pub struct Handle(pub u64);`）：字段名固定 `value`（与 C 探针/头一致）。
        tm = re.search(rf"pub\s+struct\s+{re.escape(name)}\s*\(\s*pub\s+([^)]+)\)\s*;", source)
        if not tm:
            raise ValueError(f"找不到 struct {name}")
        return [("value", re.sub(r"\s+", " ", tm.group(1)).strip())]
    start = source.index("{", m.start())
    end = balanced(source, start, "{", "}")
    body = strip_comments(source[start + 1 : end])
    fields = []
    for chunk in split_top(body):
        chunk = chunk.strip()
        if not chunk:
            continue
        fm = re.match(r"pub\s+([A-Za-z_][A-Za-z0-9_]*)\s*:\s*(.+)$", chunk, re.S)
        if not fm:
            continue
        fields.append((fm.group(1), re.sub(r"\s+", " ", fm.group(2)).strip()))
    return fields


def extract_enum(source: str, name: str) -> list[tuple[str, int]]:
    m = re.search(rf"pub\s+enum\s+{re.escape(name)}\s*\{{", source)
    if not m:
        raise ValueError(f"找不到 enum {name}")
    start = source.index("{", m.start())
    end = balanced(source, start, "{", "}")
    body = strip_comments(source[start + 1 : end])
    out = []
    for chunk in split_top(body):
        cm = re.match(r"\s*([A-Za-z_][A-Za-z0-9_]*)\s*=\s*(\d+)", chunk)
        if cm:
            out.append((cm.group(1), int(cm.group(2))))
    return out


def c_type(ty: str) -> str:
    ty = ty.strip()
    if ty.startswith("*mut "):
        return c_type(ty[len("*mut ") :]) + " *"
    if ty.startswith("*const "):
        return "const " + c_type(ty[len("*const ") :]) + " *"
    if ty.startswith("["):
        m = re.match(r"\[([A-Za-z0-9_]+);\s*(\d+)\]", ty)
        if not m:
            raise ValueError(f"数组类型不认识：{ty}")
        return f"{c_type(m.group(1))}[{m.group(2)}]"
    if ty in BASE_TYPES:
        return BASE_TYPES[ty]
    if ty in NAMED:
        return NAMED[ty]
    if ty == "()":
        return "void"
    raise ValueError(f"类型不认识：{ty}")


def c_function_pointer(name: str, ty: str) -> str | None:
    if not ty.startswith("Option<"):
        return None
    inner = ty[len("Option<") :].strip()
    if inner.endswith(">"):
        inner = inner[:-1].strip()
    inner = inner.rstrip().rstrip(",").strip()
    if not inner.startswith('extern "C" fn('):
        return None
    open_pos = inner.index("fn(") + len("fn")
    args_start = open_pos + 1
    args_end = balanced(inner, open_pos, "(", ")")
    raw_params = inner[args_start:args_end]
    params = [c_type(p) for p in split_top(raw_params) if p.strip()]
    ret = inner[inner.index("->", args_end) + 2 :].strip().rstrip(",").strip()
    ret = c_type(ret)
    return f"{ret} (*{name})({', '.join(params)})"


def parse_all() -> tuple[list[tuple[str, str, str, list[tuple[str, str]]]], list[tuple[str, int]]]:
    texts = {key: strip_comments((CAP / f"{key}.rs").read_text(encoding="utf-8")) for key in SOURCE_LABEL}
    structs = []
    for key, rust_name, c_name in STRUCTS:
        structs.append((key, rust_name, c_name, extract_struct(texts[key], rust_name)))
    status = extract_enum(texts["fs"], "CapStatus")
    if status != [("Ok", 0), ("Machine", 1), ("Unimplemented", 2)]:
        raise ValueError(f"CapStatus 取值漂移：{status}")
    return structs, status


def render_header(structs, status) -> str:
    lines = [
        "/* Pyawa 能力域 vtable 的 C 布局（AB-45 允许的唯一伴随头；`pa.h` include 它）。",
        " *",
        " * **自动生成，勿手改**：`tools/gen_pa_capabilities.py` 从 `crates/pyawa-capabilities/src/`",
        " * 的 `#[repr(C)]` 形状生成；`--check` 漂移即红（PLAN §12.17 A1/A3）。",
        " * **只含已有 Rust 布局的 3 域**（fs/clock/net）；其余 6 域本 ABI 不定义布局（只能传 NULL）。",
        " */",
        "#ifndef PAWA_PA_CAPABILITIES_H",
        "#define PAWA_PA_CAPABILITIES_H",
        "",
        "#include <stdbool.h>",
        "#include <stddef.h>",
        "#include <stdint.h>",
        "",
        "#ifdef __cplusplus",
        'extern "C" {',
        "#endif",
        "",
        "/* CP-5：能力槽位的三态结果（不是 `pa_status`；0/1/2 与 Rust `CapStatus` 同值）。 */",
        "typedef enum pa_cap_status {",
    ]
    for name, value in status:
        lines.append(f"    PA_CAP_{name.upper()} = {value},")
    lines += [
        "} pa_cap_status;",
        "/* Rust `CapStatus` 是 repr(C) enum（4 字节）；C99 下用编译期负数组长度断言。 */",
        "typedef char pa_cap_status_must_be_4_bytes[(sizeof(pa_cap_status) == 4) ? 1 : -1];",
        "",
    ]
    for _key, _rust_name, c_name, fields in structs:
        lines.append(f"typedef struct {c_name} {{")
        for fname, fty in fields:
            fn = c_function_pointer(fname, fty)
            if fn is not None:
                lines.append(f"    {fn};")
            elif fty.startswith("["):
                base, size = re.match(r"\[([A-Za-z0-9_]+);\s*(\d+)\]", fty).groups()
                lines.append(f"    {c_type(base)} {fname}[{size}];")
            else:
                lines.append(f"    {c_type(fty)} {fname};")
        lines += [f"}} {c_name};", ""]
    lines += ["#ifdef __cplusplus", "}", "#endif", "", "#endif /* PAWA_PA_CAPABILITIES_H */", ""]
    return "\n".join(lines)


def render_rust_test(structs) -> str:
    lines = [
        "//! **自动生成，勿手改**（`tools/gen_pa_capabilities.py`）—— §12.17 A3 的 Rust 侧布局探针。",
        "//!",
        "//! 打印 `layout|C 结构名|字段|offset|size`（`@size` 行给整结构尺寸），供",
        "//! `tests/ci/check_capability_layout.py` 与 C 探针逐行对照。只打印、不 assert。",
        "",
        "#![allow(dead_code)]",
        "",
        "use core::ffi::c_void;",
        "use pyawa_capabilities::{",
        "    clock::{CpTm, CpClockVtable},",
        "    fs::{CapStatus, CpFsVtable, FileInfo, FileTimes, Handle},",
        "    net::{CpNetVtable, PollFd, SocketHandle},",
        "};",
        "",
        "#[test]",
        "fn print_layout() {",
    ]
    rust_names = {"pa_fs_vtable": "CpFsVtable", "pa_clock_vtable": "CpClockVtable", "pa_net_vtable": "CpNetVtable"}
    for _key, rust_name, c_name, fields in structs:
        if c_name in rust_names:
            rn = rust_names[c_name]
            lines.append(f"    println!(\"layout|{c_name}|@size|0|{{}}\", core::mem::size_of::<{rn}>());")
            for fname, fty in fields:
                lines.append(
                    f"    println!(\"layout|{c_name}|{fname}|{{}}|{{}}\", core::mem::offset_of!({rn}, {fname}), core::mem::size_of::<{fty}>());"
                )
    lines += ["}", ""]
    return "\n".join(lines)


def render_c_probe() -> str:
    lines = [
        "/* 自动生成（tools/gen_pa_capabilities.py --emit-c-probe）：C 侧布局探针。 */",
        '#include "pa_capabilities.h"',
        "#include <stdio.h>",
        "int main(void) {",
    ]
    # 与 Rust 测试同一行格式；字段列表从 Rust 解析结果生成
    structs, _status = parse_all()
    for _key, _rust_name, c_name, fields in structs:
        if c_name not in ("pa_fs_vtable", "pa_clock_vtable", "pa_net_vtable"):
            continue
        lines.append(f'    printf("layout|{c_name}|@size|0|%zu\\n", sizeof({c_name}));')
        for fname, _fty in fields:
            lines.append(
                f'    printf("layout|{c_name}|{fname}|%zu|%zu\\n", offsetof({c_name}, {fname}), sizeof((({c_name} *)0)->{fname}));'
            )
    lines += ["    return 0;", "}", ""]
    return "\n".join(lines)


def main(argv: list[str]) -> int:
    ap = argparse.ArgumentParser()
    ap.add_argument("--check", action="store_true")
    ap.add_argument("--emit-c-probe", metavar="PATH")
    args = ap.parse_args(argv)
    structs, status = parse_all()
    header = render_header(structs, status)
    rust_test = render_rust_test(structs)
    if args.emit_c_probe:
        pathlib.Path(args.emit_c_probe).write_text(render_c_probe(), encoding="utf-8")
        return 0
    if args.check:
        failures = []
        if not HEADER.is_file() or HEADER.read_text(encoding="utf-8") != header:
            failures.append(f"{HEADER.relative_to(ROOT)} 与生成结果不一致（重跑 tools/gen_pa_capabilities.py）")
        if not RUST_TEST.is_file() or RUST_TEST.read_text(encoding="utf-8") != rust_test:
            failures.append(f"{RUST_TEST.relative_to(ROOT)} 与生成结果不一致（重跑 tools/gen_pa_capabilities.py）")
        for failure in failures:
            print(f"[FAIL] {failure}", file=sys.stderr)
        if failures:
            return 1
        print("[PASS] capability layout generated artifacts 与 Rust repr(C) 一致")
        return 0
    HEADER.parent.mkdir(parents=True, exist_ok=True)
    HEADER.write_text(header, encoding="utf-8")
    RUST_TEST.write_text(rust_test, encoding="utf-8")
    print(f"wrote {HEADER.relative_to(ROOT)}")
    print(f"wrote {RUST_TEST.relative_to(ROOT)}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main(sys.argv[1:]))
