#!/usr/bin/env python3
"""生成任意精度整数（`TS-45`／`P1-11`）的**参照夹具**。

**为什么有它**：`TS-45` 把"任意精度"定为**可观察语义**（不是实现自由），并要求两处易漏的
连带：① `int`↔`str` 的**位数上限**（默认 4300，超出报 `ValueError`，消息以探测为准）；
② `hash` 与参照一致。数值与消息一律**现场实测导出**，禁手写、禁回忆。

**用法**：

    python3 tools/gen_int_fixture.py            # 只打印摘要（默认）
    python3 tools/gen_int_fixture.py --emit     # 写出 crates/pyawa-core/tests/fixture-int-3.14.json

夹具里的整数一律编码成**十进制字符串**（`i64` 装不下；JSON 数字会失真）。

导出内容：
- `arithmetic`：`add`／`sub`／`mul`／`floordiv`／`mod`／`pow` 的 `(a, b) → 结果`
  （含**负除数的 floor 语义**——Rust 的 `div_euclid` 在这里是错的，实测打出来钉住）
- `unary`：`neg`／`abs`
- `compare`：`<`／`<=`／`==`／`>`／`>=`／`!=` 六个布尔
- `text`：`str(x)` 与 `repr(x)`
- `hash`：`hash(x)`（`i64`）
- `float`：`float(x)`（十进制字符串）与超出范围时的异常
- `limits`：4300 位上限的三条实测（边界内／超出）与消息原文
"""

from __future__ import annotations

import json
import subprocess
import sys
from pathlib import Path

#: 参与交叉组合的取值（含 `i64` 边界两侧与几个大数）。
VALUES: list[str] = [
    "0",
    "1",
    "-1",
    "2",
    "-2",
    "256",
    "257",
    "-5",
    "-6",
    "9223372036854775807",      # i64::MAX
    "9223372036854775808",      # i64::MAX + 1
    "-9223372036854775808",     # i64::MIN
    "-9223372036854775809",     # i64::MIN - 1
    "18446744073709551616",     # 2**64
    "1267650600228229401496703205376",          # 2**100
    "-1267650600228229401496703205376",         # -(2**100)
    "1000000000000000000000000000000",          # 10**30
]

#: 四则用的 `(a, b)` 对：正负号 × 量级的组合，外加 `i64` 边界两侧。
#: **除数为 0 的**单列（`zero_divisor`）——那几条要抓异常消息，不能混进正常结果表。
PAIRS: list[tuple[str, str]] = [
    ("0", "1"), ("1", "1"), ("-1", "1"), ("1", "-1"),
    ("-1", "-1"), ("7", "2"), ("7", "-2"), ("-7", "2"), ("-7", "-2"),
    ("2", "10"), ("10", "2"), ("-10", "3"), ("10", "-3"), ("-10", "-3"),
    ("9223372036854775807", "1"), ("9223372036854775807", "-1"),
    ("-9223372036854775808", "1"), ("-9223372036854775808", "-1"),
    ("9223372036854775807", "9223372036854775807"),
    ("-9223372036854775808", "-9223372036854775808"),
    ("1000000000000000000000000000000", "7"),
    ("-1000000000000000000000000000000", "7"),
    ("1000000000000000000000000000000", "-7"),
    ("-1000000000000000000000000000000", "-7"),
    ("1267650600228229401496703205376", "9223372036854775807"),
    ("-1267650600228229401496703205376", "4294967296"),
    ("18446744073709551616", "-3"),
]

#: 除数为 0 的取样（`//` 与 `%` 的消息实测导出）。
ZERO_DIVISORS: list[str] = ["0", "1", "-1", "1267650600228229401496703205376"]

#: 幂：指数取小，压住夹具体积（底数含大数）。
POW_CASES: list[tuple[str, int]] = [
    ("0", 0), ("1", 0), ("-1", 0), ("0", 5), ("1", 100), ("-1", 101), ("-1", 100),
    ("2", 10), ("2", 64), ("2", 100), ("-2", 65), ("-2", 64), ("10", 30),
    ("9223372036854775807", 2), ("-9223372036854775808", 2), ("7", 0),
]

#: `hash` 的取样（`TS-45` 点名 `hash(2**100)`）。
HASH_VALUES: list[str] = [
    "0", "1", "-1", "2", "256", "257", "-5", "-2",
    "9223372036854775807", "-9223372036854775808",
    "1267650600228229401496703205376",
    "-1267650600228229401496703205376",
    "1000000000000000000000000000000",
]

#: `float` 的取样（超出范围的那条单列）。
FLOAT_VALUES: list[str] = ["0", "-1", "2", "1267650600228229401496703205376", "1000000000000000000000000000000"]

#: 探针程序：**全部在本机参照上真跑**，跑不动就硬失败（不静默跳过）。
PROBE = r'''
import json, sys

def s(value):
    return str(value)

values = [int(text) for text in sys.argv[1].split(",")]
pairs = [(int(a), int(b)) for a, b in (item.split(":") for item in sys.argv[2].split(","))]
powers = [(int(a), int(b)) for a, b in (item.split(":") for item in sys.argv[3].split(","))]
hashes = [int(text) for text in sys.argv[4].split(",")]
floats = [int(text) for text in sys.argv[5].split(",")]
zero_divisors = [int(text) for text in sys.argv[6].split(",")]

operators = [
    ("add", lambda a, b: a + b),
    ("sub", lambda a, b: a - b),
    ("mul", lambda a, b: a * b),
    ("floordiv", lambda a, b: a // b),
    ("mod", lambda a, b: a % b),
]
arithmetic = []
for a, b in pairs:
    for name, operation in operators:
        arithmetic.append({"op": name, "a": s(a), "b": s(b), "result": s(operation(a, b))})
for a, exponent in powers:
    arithmetic.append({"op": "pow", "a": s(a), "b": s(exponent), "result": s(a ** exponent)})

unary = []
for value in values:
    unary.append({"op": "neg", "a": s(value), "result": s(-value)})
    unary.append({"op": "abs", "a": s(value), "result": s(abs(value))})

compare = []
for a, b in pairs:
    compare.append({
        "a": s(a), "b": s(b),
        "lt": a < b, "le": a <= b, "eq": a == b,
        "gt": a > b, "ge": a >= b, "ne": a != b,
    })

text = [{"value": s(value), "str": str(value), "repr": repr(value)} for value in values]
hash_rows = [{"value": s(value), "hash": hash(value)} for value in hashes]

# `& | ^`：负数走**补码**语义（无限符号扩展）——照参照真跑，别自己推
bitwise = []
for a, b in pairs:
    bitwise.append({
        "a": s(a), "b": s(b),
        "and": s(a & b), "or": s(a | b), "xor": s(a ^ b),
    })

# `<< >>`：位移量取 0／1／7／31／32／64／100（跨 limb 边界）
SHIFT_COUNTS = (0, 1, 7, 31, 32, 64, 100)
shifts = []
for value in values:
    for count in SHIFT_COUNTS:
        shifts.append({
            "a": s(value), "n": count,
            "lshift": s(value << count), "rshift": s(value >> count),
        })

invert = [{"a": s(value), "result": s(~value)} for value in values]

def error_of(operation):
    """跑一下，回 `"类名: 消息"`；没出错回 `None`（`ZeroDivisionError` 那条另有一套）。"""
    try:
        operation()
        return None
    except Exception as error:
        return f"{type(error).__name__}: {error}"


def message_of(operation):
    try:
        operation()
        return None
    except ZeroDivisionError as error:
        return str(error)

zero_rows = [
    {
        "a": s(value),
        "floordiv_message": message_of(lambda value=value: value // 0),
        "mod_message": message_of(lambda value=value: value % 0),
    }
    for value in zero_divisors
]

float_rows = [{"value": s(value), "float": repr(float(value))} for value in floats]

# `format(大整数, 规格)`：整数码走任意精度；浮点码会不会先转 double、`c` 超范围怎么报，
# 以及 **4300 位上限管不管 `format`**——全部照参照真跑（别自己推）
format_rows = []
FORMAT_CASES = [
    (10 ** 30, ""), (10 ** 30, ","), (10 ** 30, "_"), (10 ** 30, "030"), (10 ** 30, ">40"),
    (10 ** 30, "=+040"), (10 ** 30, "b"), (10 ** 30, "o"), (10 ** 30, "x"), (10 ** 30, "X"),
    (10 ** 30, "#x"), (10 ** 30, "#b"), (10 ** 30, "#o"), (10 ** 30, "<+45,"),
    (2 ** 100, "x"), (2 ** 100, "#X"), (2 ** 100, ","), (-(2 ** 100), "_x"),
    (-(10 ** 30), "030"), (0, "x"), (0, "#o"),
    (10 ** 30, ".2f"), (10 ** 30, "e"), (10 ** 30, "%"), (10 ** 30, "g"), (10 ** 30, "E"),
    # `0` 与**显式** `=` 对齐一起出现时填充仍是 `0`（小整数也放几条，免得只在超大值上验）
    (42, "=+040"), (42, "=040"), (-42, "=+040"), (42, "0=+40"), (42, "=+8"), (42, "05"),
]
for value, spec in FORMAT_CASES:
    row = {"value": s(value), "spec": spec}
    try:
        row["result"] = format(value, spec)
    except Exception as error:
        row["error"] = f"{type(error).__name__}: {error}"
    format_rows.append(row)

format_errors = {
    # `c` 超出 Unicode 范围；负值也算超范围
    "char_too_large": error_of(lambda: format(2 ** 100, "c")),
    "char_negative": error_of(lambda: format(-1, "c")),
    "char_ok": format(42, "c"),
    # 4300 位上限**管不管** `format`（十进制 vs 十六进制分别探）
    "decimal_over_limit": error_of(lambda: format(10 ** 5000)),
    "hex_over_limit": error_of(lambda: format(10 ** 5000, "x")),
    # 十六进制**不受**位数上限约束 ⇒ 把那条结果也带回来，好逐字对拍（别在测试里猜前缀）
    "hex_over_limit_text": format(10 ** 5000, "x"),
    "float_code_over_limit": error_of(lambda: format(10 ** 5000, "e")),
    "float_code_over_double": error_of(lambda: format(10 ** 400, ".2f")),
}

try:
    float(10 ** 400)
    overflow = None
except OverflowError as error:
    overflow = str(error)

# `int(float)`：向零截断；inf／nan 各有实测消息
# `1e300` 那两条是**故意**的：double 并不精确等于 10^300，它的精确值就是这些位 ⇒ 借道 `i64`
# 会静默饱和（`TS-45` 明禁），拿它当"精确转换"的判据
float_to_int = {
    "truncate_positive": s(int(2.5)),
    "truncate_negative": s(int(-2.5)),
    "from_negative_zero": s(int(-0.0)),
    "from_infinity": error_of(lambda: int(float("inf"))),
    "from_nan": error_of(lambda: int(float("nan"))),
    "exact_from_large_double": s(int(1e300)),
    "exact_from_negative_large_double": s(int(-1e300)),
    "exact_from_denormal": s(int(5e-324)),
    "exact_from_half": s(int(0.5)),
}

limit = sys.get_int_max_str_digits()
inside = 10 ** (limit - 1)          # 恰好 limit 位
outside = 10 ** limit               # limit + 1 位
try:
    str(outside)
    to_str_message = None
except ValueError as error:
    to_str_message = str(error)
try:
    int("1" + "0" * limit)
    from_str_message = None
except ValueError as error:
    from_str_message = str(error)


# `sys` 的两个入口与它们的取值域（`TS-45` ①点名 `sys.set_int_max_str_digits()` 可改）
default_limit = sys.get_int_max_str_digits()
threshold = sys.int_info.str_digits_check_threshold
set_zero = error_of(lambda: sys.set_int_max_str_digits(0))
zero_means_unlimited = sys.get_int_max_str_digits() == 0
sys.set_int_max_str_digits(default_limit)          # 复原，免得影响下面的探测
# 边界：前导零算不算、带符号的正好 limit 位算不算
leading_zeros = error_of(lambda: int("0" * (default_limit + 1)))
sign_plus_limit = error_of(lambda: int("-" + "1" * default_limit))

print(json.dumps({
    "arithmetic": arithmetic,
    "unary": unary,
    "bitwise": bitwise,
    "shifts": shifts,
    "invert": invert,
    "shift_errors": {
        "negative_left": error_of(lambda: 1 << -1),
        "negative_right": error_of(lambda: 1 >> -1),
        "huge_left": error_of(lambda: 1 << (2 ** 62)),
        "huge_right": error_of(lambda: 1 >> (2 ** 62)),
    },
    "compare": compare,
    "text": text,
    "hash": hash_rows,
    "zero_divisor": zero_rows,
    "float": float_rows,
    "format": format_rows,
    "format_errors": format_errors,
    "float_overflow": overflow,
    "float_to_int": float_to_int,
    "limits": {
        "max_str_digits": limit,
        "inside_digits": limit,
        "inside_value": s(inside),
        "outside_digits": limit + 1,
        # 不要在探针里 `str(outside)`：那正是要触发 `ValueError` 的那一下；
        # 这里直接拼出十进制字面量（字符串拼接不受上限约束）
        "outside_value": "1" + "0" * limit,
        "to_str_message": to_str_message,
        "from_str_message": from_str_message,
    },
    # `sys.set/get_int_max_str_digits` 的**消息与取值域**归 `tools/gen_sys_fixture.py`
    # （API 面的夹具放 stdlib 那边）；这里只留"转换本身"的边界事实
    "sys_limits": {
        "default": default_limit,
        "threshold": threshold,
        "setting_zero_succeeds": set_zero is None,
        "zero_means_unlimited": zero_means_unlimited,
        "leading_zeros_over_limit_message": leading_zeros,
        "sign_plus_exactly_limit_is_ok": sign_plus_limit is None,
    },
    "small_int_range": {"min": -5, "max": 256},
}, ensure_ascii=False))
'''


def probe() -> dict:
    """在参照实现上跑探针，交回夹具对象。"""
    pairs = ",".join(f"{a}:{b}" for a, b in PAIRS)
    powers = ",".join(f"{a}:{e}" for a, e in POW_CASES)
    done = subprocess.run(
        [
            sys.executable, "-c", PROBE,
            ",".join(VALUES), pairs, powers,
            ",".join(HASH_VALUES), ",".join(FLOAT_VALUES), ",".join(ZERO_DIVISORS),
        ],
        capture_output=True,
        text=True,
    )
    if done.returncode != 0:
        raise SystemExit(f"参照探针失败（不静默跳过）：\n{done.stderr}")
    return json.loads(done.stdout)


def main() -> None:
    fixture = probe()
    version = subprocess.run(
        [sys.executable, "--version"], capture_output=True, text=True
    ).stdout.strip()
    fixture = {"reference": version, **fixture}
    arithmetic = len(fixture["arithmetic"])
    compare = len(fixture["compare"])
    text = len(fixture["text"])
    hashes = len(fixture["hash"])
    if "--emit" in sys.argv:
        target = Path("crates/pyawa-core/tests/fixture-int-3.14.json")
        target.write_text(json.dumps(fixture, ensure_ascii=False, indent=1) + "\n", encoding="utf-8")
        print(f"导出 → {target}")
    else:
        print(json.dumps(fixture, ensure_ascii=False, indent=1))
    print(
        f"摘要：算术 {arithmetic} 条 · 比较 {compare} 条 · 文本 {text} 条 · hash {hashes} 条 · "
        f"上限 {fixture['limits']['max_str_digits']} 位",
        file=sys.stderr,
    )


if __name__ == "__main__":
    main()
