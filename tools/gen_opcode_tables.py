#!/usr/bin/env python3
"""从本机 CPython 3.14 的**运行时**导出指令表，生成 `crates/pyawa-core/src/opcode_metadata.rs`。

`BC-38`：数值数据的唯一出处是实现；本脚本不保存任何数值，只做"探测 → 拟合 → 校验 → 落盘"。
`BC-30`：基线是与本机运行时**全等**的指令名与编号，禁止凭记忆或抄源码。

重生成::

    python3 tools/gen_opcode_tables.py

`stack_effect` 不是逐值抄表，而是先从 oracle 拟合出**规则**（常量／线性／奇偶／`UNPACK_EX`），
再在一个远大于夹具的域上逐点校验；拟合不出规则的组合会让本脚本直接失败。
"""

from __future__ import annotations

import pathlib
import sys

import _opcode
import opcode
from _opcode_metadata import HAVE_ARGUMENT, MIN_INSTRUMENTED_OPCODE, opmap

ROOT = pathlib.Path(__file__).resolve().parents[1]
OUTPUT = ROOT / "crates/pyawa-core/src/opcode_metadata.rs"

#: 拟合与校验用的 oparg 域：小值全扫 ＋ 边界与大值。远大于夹具的采样面。
#: 不含 `2^30`——CPython 对极端 oparg 的接受边界各指令不一致（`CALL` 在 `INT_MAX-1` 就报错，
#: `BUILD_TUPLE` 到 `INT_MAX` 仍接受），逐点对齐没有收益；Rust 侧只保证 `i32` 溢出即报错。
PROBE_DOMAIN = tuple(range(0, 260)) + (
    255, 256, 257, 511, 512, 1023, 1024, 4095, 4096,
    65535, 65536, 1 << 20,
)

#: `BC-1` 的 27 个必含名（缺一即 `import opcode` 崩）。
REQUIRED_NAMES = (
    "EXTENDED_ARG", "COMPARE_OP", "BINARY_OP", "CALL_INTRINSIC_1", "CALL_INTRINSIC_2",
    "CONTAINS_OP", "CONVERT_VALUE", "END_ASYNC_FOR", "ENTER_EXECUTOR", "FOR_ITER",
    "IMPORT_NAME", "IS_OP", "JUMP_BACKWARD", "LOAD_ATTR", "LOAD_COMMON_CONSTANT",
    "LOAD_FAST_BORROW_LOAD_FAST_BORROW", "LOAD_FAST_LOAD_FAST", "LOAD_GLOBAL",
    "LOAD_SMALL_INT", "LOAD_SPECIAL", "LOAD_SUPER_ATTR", "SEND", "SET_FUNCTION_ATTRIBUTE",
    "STORE_FAST_LOAD_FAST", "STORE_FAST_STORE_FAST", "STORE_GLOBAL", "STORE_NAME",
)

NAMES = {value: name for name, value in opmap.items()}


def stack_effect(op: int, oparg: int, jump: bool | None) -> int:
    try:
        return _opcode.stack_effect(op, oparg, jump=jump)
    except ValueError as error:  # pragma: no cover —— 出现即说明探测域越界，必须显式失败
        raise SystemExit(
            f"oracle 拒绝了 {NAMES.get(op, op)} oparg={oparg} jump={jump}：{error}"
        ) from error


def values(op: int, jump: bool | None) -> list[int]:
    return [stack_effect(op, oparg, jump) for oparg in PROBE_DOMAIN]


def fit_rule(values_: list[int]) -> dict[str, object]:
    """把 `oparg → 栈效应` 拟合成一条规则；拟合不出就抛异常（脚本失败）。"""
    if len(set(values_)) == 1:
        return {"kind": "const", "value": values_[0]}

    factor = values_[1] - values_[0] if len(values_) > 1 else 0
    base = values_[0]
    if all(value == factor * oparg + base for oparg, value in zip(PROBE_DOMAIN, values_)):
        return {"kind": "linear", "factor": factor, "base": base}

    even, odd = values_[0], values_[1]
    if all(value == (even if oparg % 2 == 0 else odd)
           for oparg, value in zip(PROBE_DOMAIN, values_)):
        return {"kind": "parity", "even": even, "odd": odd}

    if all(value == (oparg & 0xFF) + (oparg >> 8)
           for oparg, value in zip(PROBE_DOMAIN, values_)):
        return {"kind": "unpack_ex"}

    raise SystemExit(f"拟合不出规则：{values_[:8]}…（PROBE_DOMAIN 上的取值无已知形态）")


def fit_stack_effects() -> dict[int, tuple[dict, dict | None]]:
    """编号 →（规则，`jump=False` 时的规则或 None）。"""
    fitted: dict[int, tuple[dict, dict | None]] = {}
    for name, op in opmap.items():
        plain = values(op, None)
        with_jump = values(op, True)
        not_jump = values(op, False)
        rule = fit_rule(plain)
        if plain != with_jump:
            raise SystemExit(f"{name}: jump=None 与 jump=True 不一致，形态超出预期")
        if plain == not_jump:
            fitted[op] = (rule, None)
        else:
            fitted[op] = (rule, fit_rule(not_jump))
    return fitted


def free_opcodes() -> list[int]:
    """`BC-31`：空闲编号＝不在 `opmap` 里、且在 instrumented 区段之下。"""
    used = set(opmap.values())
    return [op for op in range(0, MIN_INSTRUMENTED_OPCODE) if op not in used]


def emit_items(items: list[str], per_line: int) -> str:
    lines = []
    for start in range(0, len(items), per_line):
        chunk = ", ".join(items[start:start + per_line])
        suffix = "," if start + per_line < len(items) else ","
        lines.append(f"    {chunk}{suffix}")
    return "\n".join(lines)


def emit_rule(rule: dict) -> str:
    kind = rule["kind"]
    if kind == "const":
        return f"StackRule::Const({rule['value']})"
    if kind == "linear":
        return f"StackRule::Linear {{ factor: {rule['factor']}, base: {rule['base']} }}"
    if kind == "parity":
        return f"StackRule::Parity {{ even: {rule['even']}, odd: {rule['odd']} }}"
    if kind == "unpack_ex":
        return "StackRule::UnpackEx"
    raise SystemExit(f"未知规则形态：{rule}")


def generate() -> str:
    for required in REQUIRED_NAMES:
        if required not in opmap:
            raise SystemExit(f"BC-1 失败：基线 opmap 缺 `{required}`")

    fitted = fit_stack_effects()
    free = free_opcodes()
    if len(free) < 2:
        raise SystemExit("BC-31 失败：instrumented 区段之下没有足够的空闲编号")
    boundary_in, boundary_out = free[-1], free[-2]

    # T-BC-11：`opmap` ＝ 基线 ∪ 专有指令（额外项**仅**这两条）
    proprietary = [("CHECK_BOUNDARY_IN", boundary_in), ("CHECK_BOUNDARY_OUT", boundary_out)]
    by_name = sorted(list(opmap.items()) + proprietary)
    by_opcode = sorted((op, name) for name, op in by_name)

    has: dict[str, list[int]] = {}
    for family in ("arg", "const", "name", "jump", "free", "local", "exc"):
        predicate = getattr(_opcode, f"has_{family}")
        has[family] = sorted(op for op in opmap.values() if predicate(op))
    # BC-24：两条专有指令都带 oparg（签名条目索引），因此进 `has_arg`——否则 `dis` 不会按带参格式化
    has["arg"] = sorted(set(has["arg"]) | {boundary_in, boundary_out})

    # 注意：`opcode._inline_cache_entries` 的键是**指令名**（str），不是编号。
    cache = sorted(
        (opmap[name], entries)
        for name, entries in opcode._inline_cache_entries.items()
        if entries and name in opmap
    )

    nb_ops = [[name, symbol] for name, symbol in _opcode.get_nb_ops()]
    intrinsic1 = list(_opcode.get_intrinsic1_descs())
    intrinsic2 = list(_opcode.get_intrinsic2_descs())
    special_methods = list(_opcode.get_special_method_names())

    # 校验：拟合出的规则必须在整个探测域上逐点等于 oracle
    for name, op in opmap.items():
        rule, not_jump_rule = fitted[op]
        for jump, chosen in ((None, rule), (True, rule), (False, not_jump_rule or rule)):
            for oparg in PROBE_DOMAIN:
                expected = stack_effect(op, oparg, jump)
                actual = evaluate_rule(chosen, oparg)
                if expected != actual:
                    raise SystemExit(
                        f"校验失败：{name} oparg={oparg} jump={jump} oracle={expected} 规则={actual}"
                    )

    version = f"{sys.version_info.major}.{sys.version_info.minor}.{sys.version_info.micro}"
    lines: list[str] = [
        "//! 指令表与元数据——**数值的唯一出处**（`BC-38`）。",
        "//!",
        "//! 由 `tools/gen_opcode_tables.py` 从本机 CPython 运行时探测生成：**禁止手改**；",
        f"//! 重生成：`python3 tools/gen_opcode_tables.py`（本文件生成时的基线：CPython {version}）。",
        "//!",
        "//! 基线＝CPython 3.14 的指令名与编号（`BC-30`），此处**不发明指令、不改名**。",
        "",
        "/// `BC-40`：指令集版本常量——增删指令、语义变化、cache 宽度变化**必须**递增；",
        "/// `BC-41` 要求它随 `.pyac` 头部一起判陈旧。取值由实现维护。",
        "pub const INSTRUCTION_SET_VERSION: u32 = 1;",
        "",
        f"/// `BC-30`（实测）：有参指令的编号下界。",
        f"pub const HAVE_ARGUMENT: u16 = {HAVE_ARGUMENT};",
        "",
        "/// `BC-30`／`BC-32`：instrumented 区段的下界；Pyawa **禁止发射**该区段（`BC-32`）。",
        f"pub const MIN_INSTRUMENTED_OPCODE: u16 = {MIN_INSTRUMENTED_OPCODE};",
        "",
        "/// 名字 → 编号，按名字升序：CPython 3.14 基线 ＋ Pyawa 专有指令。",
        "/// `T-BC-11`：**基线 ⊆ 本表**，且额外项**仅为**专有指令（取空闲编号，见 `PYAWA_SPECIFIC`）。",
        "pub static OPMAP: &[(&str, u16)] = &[",
        emit_items([f'("{name}", {op})' for name, op in by_name], 3),
        "];",
        "",
        "/// 编号 → 名字，按编号升序。",
        "pub static OPNAME: &[(u16, &str)] = &[",
        emit_items([f'({op}, "{name}")' for op, name in by_opcode], 3),
        "];",
        "",
        "/// `BC-31`：Pyawa 专有指令取**空闲编号**（不进 `OPMAP`——那是 CPython 基线）；",
        "/// 编号在「未占用且低于 instrumented 区段」的空隙里取，生成时校验。",
        "pub static PYAWA_SPECIFIC: &[(&str, u16)] = &[",
        f'    ("CHECK_BOUNDARY_IN", {boundary_in}),',
        f'    ("CHECK_BOUNDARY_OUT", {boundary_out}),',
        "];",
        "",
        "/// `BC-35`：inline cache 宽度（只列非零项；发射方**必须**留等宽零填充槽）。",
        "pub static INLINE_CACHE_ENTRIES: &[(u16, u32)] = &[",
        emit_items([f"({op}, {entries})" for op, entries in cache], 6),
        "];",
        "",
    ]
    for family in ("arg", "const", "name", "jump", "free", "local", "exc"):
        lines += [
            f"/// `BC-37`：`has_{family}` 为真的指令编号，升序。"
            + ("（＝基线 ＋ Pyawa 专有指令：`BC-24` 说它们带 oparg）" if family == "arg" else ""),
            f"pub static HAS_{family.upper()}: &[u16] = &[",
            emit_items([str(op) for op in has[family]], 16),
            "];",
            "",
        ]
    lines += [
        "/// `BC-39`：`BINARY_OP` 的 oparg 顺序（名字，运算符）；`NB_SUBSCR` 在最后。",
        "pub static NB_OPS: &[(&str, &str)] = &[",
        emit_items([f'("{name}", "{symbol}")' for name, symbol in nb_ops], 3),
        "];",
        "",
        "/// `_opcode.get_intrinsic1_descs()` 的返回值。",
        "pub static INTRINSIC1_DESCS: &[&str] = &[",
        emit_items([f'"{name}"' for name in intrinsic1], 3),
        "];",
        "",
        "/// `_opcode.get_intrinsic2_descs()` 的返回值。",
        "pub static INTRINSIC2_DESCS: &[&str] = &[",
        emit_items([f'"{name}"' for name in intrinsic2], 3),
        "];",
        "",
        "/// `_opcode.get_special_method_names()` 的返回值。",
        "pub static SPECIAL_METHOD_NAMES: &[&str] = &[",
        emit_items([f'"{name}"' for name in special_methods], 3),
        "];",
        "",
        "/// `BC-32`：Pyawa 不做 CPython 式特化，本表**必须为空**。",
        "/// （CPython 3.14 运行时的同名表非空，那是参照实现的特化，**不照抄**。）",
        "pub static SPECIALIZATIONS: &[(&str, &[&str])] = &[];",
        "",
        "/// `BC-32`：同上，**必须为空**（`opcode.py` 的 `opname` 构造对空值安全）。",
        "pub static SPECIALIZED_OPMAP: &[(&str, u16)] = &[];",
        "",
        "/// `BC-38`：栈效应的规则形态。",
        "#[derive(Clone, Copy, Debug, PartialEq, Eq)]",
        "pub enum StackRule {",
        "    /// 与 oparg 无关。",
        "    Const(i32),",
        "    /// `factor * oparg + base`。",
        "    Linear { factor: i32, base: i32 },",
        "    /// 按 oparg 的奇偶取两个值。",
        "    Parity { even: i32, odd: i32 },",
        "    /// `UNPACK_EX`：低 8 位是前置个数、高 8 位是后置个数。",
        "    UnpackEx,",
        "}",
        "",
        "/// `BC-38`：编号 → 栈效应规则（升序；`jump` 为 `None`／`True` 时用这一条）。",
        "pub static STACK_EFFECTS: &[(u16, StackRule)] = &[",
    ]
    entries = [(op, rule) for op, (rule, _) in sorted(fitted.items())]
    entries += [(boundary_out, {"kind": "const", "value": 0}),
                (boundary_in, {"kind": "const", "value": 0})]
    entries.sort()
    lines.append(emit_items([f"({op}, {emit_rule(rule)})" for op, rule in entries], 2))
    lines += [
        "];",
        "",
        "/// `BC-38`：`jump=False` 时与上面不同的指令（`SETUP_*` 一族）。",
        "pub static STACK_EFFECTS_NOT_JUMP: &[(u16, StackRule)] = &[",
    ]
    not_jump = sorted((op, rule) for op, (_, rule) in fitted.items() if rule is not None)
    lines.append(emit_items([f"({op}, {emit_rule(rule)})" for op, rule in not_jump], 2))
    lines += ["];", ""]
    return "\n".join(lines)


def evaluate_rule(rule: dict, oparg: int) -> int:
    kind = rule["kind"]
    if kind == "const":
        return int(rule["value"])
    if kind == "linear":
        return int(rule["factor"]) * oparg + int(rule["base"])
    if kind == "parity":
        return int(rule["even"]) if oparg % 2 == 0 else int(rule["odd"])
    if kind == "unpack_ex":
        return (oparg & 0xFF) + (oparg >> 8)
    raise SystemExit(f"未知规则：{rule}")


def main() -> int:
    text = generate()
    OUTPUT.write_text(text, encoding="utf-8")
    print(f"已写入 {OUTPUT.relative_to(ROOT)}：{len(text.splitlines())} 行")
    print(f"基线 CPython {sys.version.split()[0]}｜opmap {len(opmap)} 项｜"
          f"HAVE_ARGUMENT {HAVE_ARGUMENT}｜MIN_INSTRUMENTED {MIN_INSTRUMENTED_OPCODE}")
    return 0


if __name__ == "__main__":
    sys.exit(main())
