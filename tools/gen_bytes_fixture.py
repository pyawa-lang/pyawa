#!/usr/bin/env python3
"""从参照实现**探测** `bytes` 的可观测面，导出 `crates/pyawa-core/tests/fixture-bytes-3.14.json`。

`P1-12`（`TS-42` 的"M2 之后、M3 之前"档）：`bytes` 要挡住 `marshal`（`CM-27`）——所以它的
**形状、消息、哈希、比较**都得是**实测**的，不许手写（`MS-19`）。

探针全部在**本机参照**上真跑；跑不动就硬失败（不静默跳过）。

用法::

    python3 tools/gen_bytes_fixture.py            # 只打印摘要
    python3 tools/gen_bytes_fixture.py --emit     # 写出夹具
"""

from __future__ import annotations

import json
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
FIXTURE = ROOT / "crates/pyawa-core/tests/fixture-bytes-3.14.json"

#: `repr` 取样：空的、可打印 ASCII、需要转义的、非 ASCII／高位字节、引号冲突、控制字符。
REPR_VALUES: list[bytes] = [
    b"",
    b"abc",
    b"it's",
    b'has "double"',
    b"both ' and \"",
    b"tab\tnewline\n",
    b"\x00\x01\x1f",
    b"\x7f",
    b"\x80\xff",
    b"\\backslash\\",
    b"caf\xc3\xa9",  # UTF-8 的 é（bytes 层面是高位字节）
    b"\r\n",
]

#: 构造取样：`(写法, 说明)` —— 写法在探针里 eval。
#: 注意 `bytes(256)`：**不是**错误，是 256 个零字节（实测）——所以它列在这儿而不是错误表里。
CONSTRUCT_CASES: list[str] = [
    "bytes()",
    "bytes(0)",
    "bytes(3)",
    "bytes(256)",
    "bytes(b'ab')",
    "bytes([0, 1, 255])",
    "bytes('abc', 'utf-8')",
    "bytes('é', 'utf-8')",
    "bytes((1, 2, 3))",
]

#: 构造**失败**取样（消息要逐字进夹具）。
#: `bytes(bytearray(...))` 这类**不进**：`bytearray` 在 `TS-42` 里是 M3+，本层还没有 ⇒ 放进来
#: 只会得到一条"本层未实现"，那是缺口不是语义（`MS-19` 的适用范围）。
CONSTRUCT_ERRORS: list[str] = [
    "bytes(-1)",
    "bytes([256])",
    "bytes(['a'])",
    "bytes('abc')",
    "bytes(1.5)",
    "bytes('abc', 'nope')",
]

#: 索引取样（**整数**下标；`bytes` 下标给整数，实测 `b'abc'[0] == 97`）。
INDEX_CASES: list[str] = [
    "b'abc'[0]",
    "b'abc'[-1]",
]

#: **切片**取样：`bytes` 的切片要 `slice` 类型（`TS-42` 里是 M3+）⇒ 先**记录**不测，
#: 等切片接线时拿它当判据。
SLICE_CASES: list[str] = [
    "b'abc'[1:]",
    "b'abc'[:2]",
    "b'abc'[::-1]",
    "b'abc'[5:]",
    "b'abc'[1:2]",
]

#: 索引**失败**取样。
INDEX_ERRORS: list[str] = [
    "b'abc'[3]",
    "b'abc'[-4]",
]

#: 比较取样（`bytes` 之间按字典序）。
COMPARE_VALUES: list[bytes] = [b"", b"a", b"ab", b"abc", b"abd", b"b", b"\x00", b"\xff"]

#: 哈希取样（参照：`bytes` 的哈希与同内容的 ASCII `str` **相同**）。
HASH_VALUES: list[bytes] = [b"", b"a", b"abc", b"hello world", b"\x00\xff"]

#: **方法面**取样：接收者 ＋ 方法名 ＋ 实参（实参的形态在两侧都表达得出来，好逐条对拍）。
#: 结果一律用参照的 `repr` 记（`bytes`／`str`／`int`／`bool`／`list` 都能落进同一个通道）。
METHOD_CASES: list[dict] = [
    {"receiver": "616263", "method": "hex", "args": []},
    {"receiver": "636166c3a9", "method": "decode", "args": []},
    {"receiver": "616263", "method": "decode", "args": [{"kind": "str", "value": "utf-8"}]},
    {"receiver": "616263", "method": "decode", "args": [{"kind": "str", "value": "nope"}]},
    {"receiver": "616263", "method": "startswith", "args": [{"kind": "bytes", "value": "61"}]},
    {"receiver": "616263", "method": "startswith", "args": [{"kind": "bytes", "value": "62"}]},
    {"receiver": "616263", "method": "endswith", "args": [{"kind": "bytes", "value": "63"}]},
    {"receiver": "6162633463", "method": "find", "args": [{"kind": "bytes", "value": "63"}]},
    {"receiver": "6162633463", "method": "find", "args": [{"kind": "bytes", "value": "7a"}]},
    {"receiver": "6162633463", "method": "count", "args": [{"kind": "bytes", "value": "63"}]},
    {"receiver": "61626334", "method": "replace", "args": [{"kind": "bytes", "value": "61"},
                                                          {"kind": "bytes", "value": "78"}]},
    {"receiver": "61624344", "method": "upper", "args": []},
    {"receiver": "41426364", "method": "lower", "args": []},
    {"receiver": "202061622020", "method": "strip", "args": []},
    {"receiver": "612c622c63", "method": "split", "args": [{"kind": "bytes", "value": "2c"}]},
    {"receiver": "2c", "method": "join", "args": [{"kind": "bytes_list", "value": ["61", "62"]}]},
    # 下面四条是**容易静默写错**的那几个：带实参的 strip、空 old 的 replace、
    # 空分隔符的 split、以及 join 收到非 bytes 的项
    {"receiver": "202061622020", "method": "strip", "args": [{"kind": "bytes", "value": "61"}]},
    {"receiver": "616263", "method": "replace",
     "args": [{"kind": "bytes", "value": ""}, {"kind": "bytes", "value": "78"}]},
    {"receiver": "616263", "method": "split", "args": [{"kind": "bytes", "value": ""}]},
    {"receiver": "2c", "method": "join", "args": [{"kind": "int_list", "value": [1]}]},
]

#: **字面量**取样：源码文本 → 值（词法＋转义的判据；转义用 `raw` 串写，免得被 Python 先吃掉）。
LITERAL_CASES: list[str] = [
    "b''",
    "b'abc'",
    'b"abc"',
    r"b'\n'",
    r"b'\t'",
    r"b'\r'",
    r"b'\x00\xff'",
    r"b'\\'",
    r"b'\''",
    'b"\\""',
    r"b'\101'",
    r"b'it\'s'",
    r"b'caf\xc3\xa9'",
]

#: 字面量**失败**取样（消息逐字进夹具）。
LITERAL_ERRORS: list[str] = [
    "b'é'",       # 非 ASCII 字符直接写在 bytes 字面量里
    r"b'\x1'",    # `\x` 后面要两位十六进制
    "b'abc",      # 没有收尾引号
]

PROBE = r'''
import json, sys

def s_bytes(value):
    return value.hex()

def from_hex(text):
    # 空 `bytes` 的十六进制是空串，用 `-` 占位（否则在参数里会被当成"没有这一项"）
    return b"" if text == "-" else bytes.fromhex(text)

def error_of(operation):
    try:
        operation()
        return None
    except Exception as error:
        return f"{type(error).__name__}: {error}"

repr_values = [from_hex(text) for text in sys.argv[1].split(",") if text != ""]
construct_cases = sys.argv[2].split("\n") if sys.argv[2] else []
construct_errors = sys.argv[3].split("\n") if sys.argv[3] else []
index_cases = sys.argv[4].split("\n") if sys.argv[4] else []
index_errors = sys.argv[5].split("\n") if sys.argv[5] else []
slice_cases = sys.argv[8].split("\n") if len(sys.argv) > 8 and sys.argv[8] else []
method_cases = json.loads(sys.argv[11]) if len(sys.argv) > 11 and sys.argv[11] else []

def build_argument(spec):
    if spec["kind"] == "bytes":
        return bytes.fromhex(spec["value"]) if spec["value"] else b""
    if spec["kind"] == "bytes_list":
        return [bytes.fromhex(item) if item else b"" for item in spec["value"]]
    if spec["kind"] == "int_list":
        return list(spec["value"])
    if spec["kind"] == "str":
        return spec["value"]
    if spec["kind"] == "int":
        return spec["value"]
    raise AssertionError(spec["kind"])

method_rows = []
for case in method_cases:
    receiver = bytes.fromhex(case["receiver"]) if case["receiver"] else b""
    arguments = [build_argument(spec) for spec in case["args"]]
    row = dict(case)
    try:
        row["repr"] = repr(getattr(receiver, case["method"])(*arguments))
    except Exception as error:
        row["error"] = f"{type(error).__name__}: {error}"
    method_rows.append(row)

literal_cases = sys.argv[9].split("\n") if len(sys.argv) > 9 and sys.argv[9] else []
literal_errors = sys.argv[10].split("\n") if len(sys.argv) > 10 and sys.argv[10] else []
compare_values = [from_hex(text) for text in sys.argv[6].split(",") if text != ""]
hash_values = [from_hex(text) for text in sys.argv[7].split(",") if text != ""]

# `repr` 逐条实测（`repr` 的**文本**就是夹具里要比的那一项）
repr_rows = [{"hex": s_bytes(value), "repr": repr(value), "str": str(value)} for value in repr_values]

construct = []
for text in construct_cases:
    value = eval(text)
    construct.append({"call": text, "hex": s_bytes(value), "repr": repr(value)})

construct_errors_rows = [{"call": text, "error": error_of(lambda text=text: eval(text))} for text in construct_errors]

index_rows = []
for text in index_cases:
    result = eval(text)
    if isinstance(result, int):
        index_rows.append({"expr": text, "int": result})
    else:
        index_rows.append({"expr": text, "hex": s_bytes(result)})

slice_rows = []
for text in slice_cases:
    result = eval(text)
    slice_rows.append({"expr": text, "hex": s_bytes(result)})

index_error_rows = [{"expr": text, "error": error_of(lambda text=text: eval(text))} for text in index_errors]

compare = []
for left in compare_values:
    for right in compare_values:
        compare.append({
            "a": s_bytes(left), "b": s_bytes(right),
            "lt": left < right, "le": left <= right, "eq": left == right,
            "gt": left > right, "ge": left >= right, "ne": left != right,
        })

hashes = []
for value in hash_values:
    row = {"hex": s_bytes(value), "hash": hash(value)}
    # 参照实测：同内容的 ASCII `str` 与 `bytes` 的哈希**相同**（这条容易被想当然写错）
    try:
        row["text_hash"] = hash(value.decode("ascii"))
    except Exception:
        row["text_hash"] = None
    hashes.append(row)

# 迭代与包含（`in`）
iteration = []
for value in repr_values:
    iteration.append({
        "hex": s_bytes(value),
        "items": list(value),
        "contains_97": 97 in value,
    })

# 长度
lengths = [{"hex": s_bytes(value), "len": len(value)} for value in repr_values]

print(json.dumps({
    "repr": repr_rows,
    "construct": construct,
    "construct_errors": construct_errors_rows,
    "index": index_rows,
    "slice": slice_rows,
    "index_errors": index_error_rows,
    "compare": compare,
    "hash": hashes,
    "iteration": iteration,
    "length": lengths,
    "methods": method_rows,
    # 字面量：`eval` 成功给值，失败给**编译期**消息（`SyntaxError` 一族）
    "literal": [
        {"source": text, "hex": s_bytes(eval(text)), "repr": repr(eval(text))}
        for text in literal_cases
    ],
    "literal_errors": [
        {"source": text, "error": error_of(lambda text=text: eval(text))}
        for text in literal_errors
    ],
    "hash_equals_ascii_str": all(
        row["text_hash"] is None or row["text_hash"] == row["hash"] for row in hashes
    ),
}, ensure_ascii=False))
'''


def main() -> int:
    emit = "--emit" in sys.argv

    def encode(values: list[bytes]) -> str:
        return ",".join(value.hex() if value else "-" for value in values)

    argv = [
        encode(REPR_VALUES),
        "\n".join(CONSTRUCT_CASES),
        "\n".join(CONSTRUCT_ERRORS),
        "\n".join(INDEX_CASES),
        "\n".join(INDEX_ERRORS),
        encode(COMPARE_VALUES),
        encode(HASH_VALUES),
        "\n".join(SLICE_CASES),
        "\n".join(LITERAL_CASES),
        "\n".join(LITERAL_ERRORS),
        json.dumps(METHOD_CASES),
    ]
    completed = __import__("subprocess").run(
        [sys.executable, "-c", PROBE, *argv], capture_output=True, check=False
    )
    if completed.returncode != 0:
        sys.stderr.write(completed.stderr.decode())
        return completed.returncode
    data = json.loads(completed.stdout.decode())

    if emit:
        FIXTURE.write_text(json.dumps(data, ensure_ascii=False, indent=1) + "\n")
    print(
        f"repr {len(data['repr'])} 条 · 构造 {len(data['construct'])} 条 · "
        f"索引 {len(data['index'])} 条 · 字面量 {len(data['literal'])} 条 · 方法 {len(data['methods'])} 条 · 比较 {len(data['compare'])} 条 · "
        f"hash {len(data['hash'])} 条 · 参照实测 hash(bytes) == hash(ascii str)："
        f"{data['hash_equals_ascii_str']}"
    )
    if emit:
        print(f"导出 → {FIXTURE.relative_to(ROOT)}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
