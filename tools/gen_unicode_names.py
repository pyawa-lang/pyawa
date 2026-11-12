#!/usr/bin/env python3
"""**生成 `\\N{…}` 的 Unicode 名字表**（C2 冷门语义；PLAN §9.4 工单第 6 项）。

数据来源（如实登记 ✓）：
* **主表**：参照实现的 `unicodedata.name`（本机 CPython 3.14.4，Unicode 16.0.0）——遍历
  `0..0x110000` 取全部具名码点 ✓；
* **别名**（`\\N{NULL}`／`\\N{LF}`／`\\N{BYTE ORDER MARK}` 一类）：CPython 只暴露 `lookup`、
  **不暴露别名清单** ✗ ⇒ 从 Perl 的机器生成数据 `unicore/Name.pl`（Unicode 15.0.0）取**候选**，
  再**逐个用本机 `unicodedata.lookup` 复核**（只保留参照确实接受的单码点名 ✓ —— `LINE FEED (LF)`
  这类 Perl 有、参照不收的会被滤掉 ✓）；
* **命名序列**（`KEYCAP NUMBER SIGN` 一类多码点）：`\\N{}` 只收**单码点**名（参照实测对序列名报
  `unknown Unicode character name` ✗）⇒ 不收录 ✓。

压缩：
* **CJK 表意文字**（`CJK UNIFIED IDEOGRAPH-XXXX`／`CJK COMPATIBILITY IDEOGRAPH-XXXX`，约 9.9 万条、
  2.6 MB）是**算法名字** ⇒ 只存码点**区间**，查表时解析十六进制后缀 ✓；
* 其余（含 HANGUL SYLLABLE，约 1.1 万条）逐条存名字（按名字排序，供二分查找 ✓）。

二进制格式（小端）：
```
magic  b"PYUN"
u8     version = 1
u8     reserved = 0
u32    count            存表名字数
u32    cjk_range_count  CJK 区间数
u32    names_len        名字字节区长度
u32[count+1]  offsets   （相对名字字节区；升序，最后一个 = names_len）
u32[count]    codepoints
(cjk_range_count × (u32 start, u32 end))  CJK 区间（按 start 升序）
u8[names_len] names     （按名字字节升序；不含 NUL 分隔）
```

用法::

    python3 tools/gen_unicode_names.py            # 写 crates/pyawa-core/src/unicode_names.bin
    python3 tools/gen_unicode_names.py --check    # 只校验现有文件与本次生成一致
"""
import argparse
import glob
import pathlib
import struct
import sys
import unicodedata

ROOT = pathlib.Path(__file__).resolve().parent.parent
OUT = ROOT / "crates/pyawa-core/src/unicode_names.bin"

CJK_PREFIXES = ("CJK UNIFIED IDEOGRAPH-", "CJK COMPATIBILITY IDEOGRAPH-")

#: Perl 的 `unicore/Name.pl`（只用来**取别名候选**；缺文件就跳过别名，主表不受影响 ✓）。
ALIAS_CANDIDATES = (
    "/usr/share/perl/*/unicore/Name.pl",
    "/usr/lib/*/perl-base/unicore/Name.pl",
)


def collect_names():
    names = []
    for cp in range(0x110000):
        try:
            ch = chr(cp)
        except ValueError:
            continue
        name = unicodedata.name(ch, None)
        if name is not None:
            names.append((name, cp))
    return names


def collect_aliases(existing: set, verbose: bool = True):
    """Perl Name.pl ⇒ 别名候选 ⇒ **逐个用本机 CPython `lookup` 复核** ✓。"""
    paths = []
    for pattern in ALIAS_CANDIDATES:
        paths.extend(glob.glob(pattern))
    if not paths:
        if verbose:
            print("!! 找不到 Perl unicore/Name.pl ⇒ 跳过别名表（主表不受影响）", file=sys.stderr)
        return []
    path = sorted(paths)[0]
    lines = pathlib.Path(path).read_text(encoding="utf-8", errors="replace").splitlines()
    aliases = []
    seen = set()
    index = 0
    while index < len(lines):
        line = lines[index].strip()
        index += 1
        if not line:
            continue
        # 形式：码点行（4–6 位大写十六进制）后跟名字行（名字里允许空格/括号/连字符）。
        if len(line) >= 4 and all(ch in "0123456789ABCDEF" for ch in line):
            codepoint = int(line, 16)
            if index >= len(lines):
                break
            name = lines[index].strip()
            index += 1
            if not name or name in existing or name in seen:
                continue
            # **用参照自己复核**：它只收单码点、且必须就是我们猜的那个码点 ✓。
            try:
                found = unicodedata.lookup(name)
            except KeyError:
                continue
            if len(found) != 1 or ord(found) != codepoint:
                continue
            seen.add(name)
            aliases.append((name, codepoint))
    if verbose:
        print(f"别名：{path} ⇒ 复核后 {len(aliases)} 条 ✓")
    return aliases


def is_cjk_algorithmic(name: str, codepoint: int) -> bool:
    for prefix in CJK_PREFIXES:
        if name.startswith(prefix):
            suffix = name[len(prefix):]
            if suffix == f"{codepoint:04X}":
                return True
    return False


def build_payload():
    names = collect_names()
    existing = {name for name, _ in names}
    aliases = collect_aliases(existing)
    names.extend(aliases)
    print(f"名字总数：{len(names)}（主表 {len(existing)} ＋ 别名 {len(aliases)}）")

    stored = []
    cjk = []
    for name, codepoint in names:
        if is_cjk_algorithmic(name, codepoint):
            cjk.append(codepoint)
        else:
            stored.append((name, codepoint))
    # CJK 码点压成连续区间（按码点升序）
    cjk.sort()
    ranges = []
    for codepoint in cjk:
        if ranges and codepoint == ranges[-1][1] + 1:
            ranges[-1] = (ranges[-1][0], codepoint)
        else:
            ranges.append((codepoint, codepoint))
    stored.sort(key=lambda item: item[0].encode("utf-8"))
    # 名字唯一性自检（同一个名字不许映射到两个码点）
    for index in range(1, len(stored)):
        assert stored[index - 1][0] != stored[index][0], f"重名: {stored[index][0]}"
    print(f"存表名字：{len(stored)}（{sum(len(n.encode()) for n, _ in stored)} 字节）；"
          f"CJK 算法名字：{len(cjk)} 条压成 {len(ranges)} 段")

    name_blob = bytearray()
    offsets = []
    codepoints = []
    for name, codepoint in stored:
        offsets.append(len(name_blob))
        encoded = name.encode("utf-8")
        name_blob.extend(encoded)
        codepoints.append(codepoint)
    offsets.append(len(name_blob))

    payload = bytearray()
    payload.extend(b"PYUN")
    payload.extend(bytes((1, 0)))
    payload.extend(struct.pack("<I", len(stored)))
    payload.extend(struct.pack("<I", len(ranges)))
    payload.extend(struct.pack("<I", len(name_blob)))
    payload.extend(struct.pack(f"<{len(offsets)}I", *offsets))
    payload.extend(struct.pack(f"<{len(codepoints)}I", *codepoints))
    for start, end in ranges:
        payload.extend(struct.pack("<II", start, end))
    payload.extend(name_blob)
    return bytes(payload), stored, ranges


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--check", action="store_true", help="只校验现有文件与本次生成一致（不写）")
    args = parser.parse_args()
    print(f"unicodedata：{unicodedata.unidata_version}（{sys.version.split()[0]}）")
    payload, _, _ = build_payload()
    if args.check:
        if not OUT.is_file():
            print(f"缺文件：{OUT}", file=sys.stderr)
            return 2
        current = OUT.read_bytes()
        if current != payload:
            print("不一致 ✗（重新生成）", file=sys.stderr)
            return 1
        print("一致 ✓")
        return 0
    OUT.parent.mkdir(parents=True, exist_ok=True)
    OUT.write_bytes(payload)
    print(f"已写 {OUT}（{len(payload)} 字节 ≈ {len(payload) / 1024 / 1024:.2f} MB）")
    return 0


if __name__ == "__main__":
    sys.exit(main())
