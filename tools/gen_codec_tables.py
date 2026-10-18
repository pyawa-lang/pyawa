#!/usr/bin/env python3
"""**从参照 `python3` 生成多字节编码的映射表**（CJK `_codecs_*` 族的数据源 ✓）。

动因 ✓：`_codecs_jp`／`_codecs_cn`／`_codecs_kr`／`_codecs_tw`… 的映射表在 CPython 的 **C 模块**里 ✗，
`Lib/` 里拿不到 ✗（实测：`Lib/encodings/cp932.py` 已同步 ✓，但 `import encodings.cp932` ⇒
`No module named '_codecs_jp'` ✗）⇒ 只能**从参照 dump** ✓ ⇒ 生成 Rust 侧的数据文件 ✓
（与 `unicode_tables.rs` 同源的思路 ✓）。

## 生成物（**禁止手改**；改参照/算法后重跑本工具 ✓）

表落在 `crates/pyawa-stdlib/src/codec_tables/<family>.txt`，由 `pyawa-stdlib` 的
`multibyte_module.rs` 用 `include_str!` 读入、**首次用到时**解析成有序数组（编码方向二分查找 ✓）。
生成 → 落文件的可复现命令（**一处真相：这里**）::

    python3 tools/gen_codec_tables.py --family jp --emit        # 写 crates/pyawa-stdlib/src/codec_tables/jp.txt
    python3 tools/gen_codec_tables.py --family jp               # 只打印统计＋自检（不写文件）
    python3 tools/gen_codec_tables.py --family all --emit       # 已支持的全部族

文本格式（**只看数据，不含状态机** ✓；`E`/`D` 两个方向都 dump ⇒ Rust 侧不做任何反向推导 ✓）::

    # 头注释（生成命令、参照解释器版本、逐编码计数）
    @ <codec>                          # 编码块开始
    E <码位:hex> <编码字节:hex>         # str→bytes：该码位的**唯一规范编码**（参照 encode 的结果）
    D <字节序列:hex> <码位:hex>[ ...]   # bytes→str：该字节序列的**完整解码结果**（JIS X 0213 的
                                       #   组合字符会解出 2 个码位 ⇒ 一行里可有两个码位）
    P <码位:hex> <码位:hex> <字节:hex>  # 两码位 → bytes：JIS X 0213 的**组合字符编码**
                                       #   （`'か' + '\u309a'` 合成一条字节序列；单码位表装不下）
    X <前缀:hex> <期望总长>            # 前缀状态 → 该状态**还差几个字节**（错误分型用：
                                       #   字节不够 ⇒ `incomplete`，够但配不上 ⇒ `illegal`）

## 口径与边界（如实标注）

- **编码方向**：对每个码位调参照 `str.encode(codec)`，成功则记一条 ✓（失败 = 不可编码 ⇒ 不记）。
  另：JIS X 0213 的**两码位组合字符**（`'か'+'\u309a'` 之类，共 25 对）单码位表装不下 ⇒ 对解码表里
  每条「解出 2 个码位」的序列，再调参照 `text.encode(codec)` 得到它的规范编码，记成 `P` 行。
- **解码方向**：从**合法前缀**出发做 BFS ✓ —— 对每条候选字节序列调 `bytes.decode(codec)`：
  成功 → 记一条完整解码结果；报 `incomplete multibyte sequence` → 它仍是前缀，继续加一个字节；
  其余失败（`illegal multibyte sequence`）→ 不是前缀，剪枝。这样得到的是**参照解码器的精确映射** ✓
  （不是编码表的反推 —— 反向会丢掉 cp932／big5 的重复映射与 JIS X 0213 的组合字符 ✗）。
- **只出数据，不实现状态机** ✗：`_codecs_*` 的编解码循环（`errors=strict/ignore/replace`、
  `incomplete`／`illegal` 分型、错误文案）在 `multibyte_module.rs` ✓；本工具只保证**表**与参照一致。
- **错误分型的结构信息**（`X` 行）：CPython 的 C 解码器对"字节不够"（`incomplete`）与"够但配不上"
  （`illegal`）分得比前缀集合更细 —— 例如 `euc_jp` 的 `b'\x8f\xa1'` 在输入末尾报 `incomplete`
  （结构上还要第 3 字节），而 `b'\x8f\xa1\x20'` 报 `illegal`。本工具对每个**合法前缀**用两个
  填充字节探出「该状态期望的总字节数」（`X` 行；只支持**定长状态机**，状态依赖的编码会明确报错）。
- **吸收态长前缀**（`L` 行；round 12 为 `euc_kr` 引进）：某些前缀之后任何字节都还报
  `incomplete`（CPython 在等一条定长长序列，如 `euc_kr` 的 8 字节谚文分解）⇒ BFS 直接展开会
  指数爆炸，本工具把它记成「长前缀 → 期望总长」，映射取参照 `encode` 反向并逐条 `decode` 核对。
- **未支持**（本工具会明确拒绝，不静默生成半张表）：`gb18030`（4 字节区间 ≈ 110 万码位，
  需要专用线性表示）、`hz`（有状态 7-bit 编解码）、`big5hkscs`（属 `_codecs_hk`，另一族）。

用法::

    python3 tools/gen_codec_tables.py --family jp            # 统计＋逐编码自检
    python3 tools/gen_codec_tables.py --family jp --emit     # 写表（可复现路径见上）
    python3 tools/gen_codec_tables.py cp949                   # 单编码统计（旧行为，不写表）
    python3 tools/gen_codec_tables.py cp949 --out target/recon/cp949.tsv   # 单编码 TSV（旧行为）
"""

from __future__ import annotations

import argparse
import pathlib
import sys

WORKSPACE = pathlib.Path(__file__).resolve().parent.parent
TABLE_DIR = WORKSPACE / "crates" / "pyawa-stdlib" / "src" / "codec_tables"

#: 各族的编码名（**顺序即生成顺序**；与 `Lib/encodings/*.py` 的 `getcodec(...)` 参数一致 ✓）。
FAMILIES: dict[str, list[str]] = {
    "jp": [
        "cp932",
        "euc_jp",
        "euc_jis_2004",
        "euc_jisx0213",
        "shift_jis",
        "shift_jis_2004",
        "shift_jisx0213",
    ],
    "tw": ["big5", "cp950"],
    "kr": ["euc_kr", "cp949", "johab"],
    "cn": ["gb2312", "gbk"],
}

#: 本工具**不支持**的编码（理由必须写在这里；生成时拒绝而不是出半张表 ✓）。
UNSUPPORTED: dict[str, str] = {
    "gb18030": "4 字节区间映射（≈110 万码位）需要专用的区间线性表示，尚未落地",
    "hz": "有状态 7-bit 编解码（`~{…~}`）需要状态机，尚未落地",
    "big5hkscs": "属 `_codecs_hk`（另一族，未排期）",
}

#: 抽查用的样例（只用于打印，不参与断言）。
KNOWN = {
    "cp949": "가",
    "euc_kr": "가",
    "shift_jis": "あ",
    "euc_jp": "あ",
    "cp932": "あ",
    "gb2312": "中",
    "gbk": "中",
    "big5": "中",
    "cp950": "中",
}

#: 解码 BFS 的深度上限（多字节编码最长 8 字节：`euc_kr` 的谚文分解；超过即视为表异常 ✓）。
MAX_SEQUENCE = 8


def encode_map(codec: str) -> dict[int, bytes]:
    """参照的**编码方向**：码位 → 该码位的规范编码字节（不可编码的码位不记）。"""
    table: dict[int, bytes] = {}
    for codepoint in range(0x110000):
        if 0xD800 <= codepoint <= 0xDFFF:
            continue
        try:
            table[codepoint] = chr(codepoint).encode(codec)
        except UnicodeEncodeError:
            continue
    return table


def decode_map(
    codec: str, encoded: dict[int, bytes]
) -> tuple[dict[bytes, str], list[bytes], dict[bytes, int]]:
    """参照的**解码方向**：字节序列 → 完整解码结果（BFS 只走合法前缀 ✓）。

    返回 `(表, 全部合法前缀, 长前缀表)`：

    - **合法前缀**（`incomplete` 的那些）——错误分型（`incomplete`／`illegal`）要用它们推出
      每个状态的期望字节数（见 [`expectation_map`]）。
    - **长前缀**（`long_prefixes`）：**吸收态** —— 该前缀的 256 个单字节扩展**全部**仍报
      `incomplete`，**且再扩一层也还全部** `incomplete`（否则只是"还差几个字节"的普通状态，
      如 `euc_jp` 的 `8f`）。那是 CPython 状态机在等一条**定长**长序列（本族只有 `euc_kr` 的
      谚文分解：`a4d4` 之后还要 6 字节、整整 8 字节才查表）。吸收态若照常 BFS 展开会指数爆炸
      （256^6）✗ ⇒ 记成「长前缀 → 期望总长」，由 Rust 引擎**一步跳到总长**再查表 ✓。
      长分支的**按位字母表**从参照 encode 结果里取（每个偏移上出现过哪些字节），再对**全部跨积**
      逐条问参照 `decode` ⇒ 规范组合与"仅解码"组合都收 ✓（`euc_kr`：11172 条组合里 8822 条来自
      encode、其余是仅解码项，**不能只抄 encode 反向** ✗）。该分支的"字母表完备"由 `euc_kr` 的
      逐位替换闭包探测（把每位的双字节码在 `a1a1..fefe` 全范围替换，可解码的组合恰好落在字母表内）
      交叉验证；`check_maps` 守住"每条 encode 结果都在长分支解码表里"。
    """
    table: dict[bytes, str] = {}
    all_prefixes: list[bytes] = []
    long_prefixes: dict[bytes, int] = {}

    def probe(prefix: bytes) -> tuple[list[tuple[bytes, str]], list[bytes]]:
        """一个前缀的 256 个单字节扩展：`(成功解码的, 仍 incomplete 的)`。"""
        successes: list[tuple[bytes, str]] = []
        incompletes: list[bytes] = []
        for byte in range(256):
            sequence = prefix + bytes([byte])
            if len(sequence) > MAX_SEQUENCE:
                raise SystemExit(f"✗ {codec}: 序列超过 {MAX_SEQUENCE} 字节：{sequence.hex()}")
            try:
                successes.append((sequence, sequence.decode(codec)))
            except UnicodeDecodeError as error:
                if "incomplete multibyte sequence" in str(error):
                    incompletes.append(sequence)
        return successes, incompletes

    def is_absorbing(prefix: bytes) -> bool:
        """真吸收态：自己的扩展全 incomplete，**且**每个子前缀的扩展也全 incomplete。"""
        _, incompletes = probe(prefix)
        if len(incompletes) != 256:
            return False
        for child in incompletes:
            _, child_incompletes = probe(child)
            if len(child_incompletes) != 256:
                return False
        return True

    prefixes = [b""]
    while prefixes:
        next_prefixes: list[bytes] = []
        for prefix in prefixes:
            successes, incompletes = probe(prefix)
            for sequence, decoded in successes:
                table[sequence] = decoded
            if prefix and len(incompletes) == 256:
                if is_absorbing(prefix):
                    # 真吸收态：不再展开（`probe` 已经确认 256 个扩展全 incomplete）
                    long_prefixes[prefix] = 0  # 真实总长待下面按参照 encode 结果定
                    continue
                # 只是"字节不够"的普通状态（如 euc_jp 的 8f）⇒ 照常展开 ✓
            next_prefixes.extend(incompletes)
        all_prefixes.extend(next_prefixes)
        prefixes = next_prefixes
    for prefix in list(long_prefixes):
        encoded_candidates = [
            sequence
            for sequence in encoded.values()
            if len(sequence) > len(prefix) and sequence.startswith(prefix)
        ]
        if not encoded_candidates:
            raise SystemExit(f"✗ {codec}: 吸收态前缀 {prefix.hex()} 在编码表里没有候选长序列")
        expected = max(len(sequence) for sequence in encoded_candidates)
        if any(len(sequence) != expected for sequence in encoded_candidates):
            raise SystemExit(
                f"✗ {codec}: 吸收态前缀 {prefix.hex()} 的长序列长度不一致（本工具只支持定长）"
            )
        long_prefixes[prefix] = expected
        # **按位字母表**：长分支的每个偏移上有哪些字节，从参照 encode 结果里取。解码器对每位的
        # 取值集合做校验 ⇒ **全部跨积**逐条问参照 decode（**不能只抄 encode 反向** ✗：`euc_kr`
        # 的 8 字节分支有 11172 条组合，encode 只覆盖其中 8822 条规范组合，其余是"仅解码"项）。
        alphabets = [
            sorted({sequence[offset] for sequence in encoded_candidates})
            for offset in range(len(prefix), expected)
        ]
        combinations = 1
        for alphabet in alphabets:
            combinations *= len(alphabet)
        if combinations > 2_000_000:
            raise SystemExit(
                f"✗ {codec}: 吸收态前缀 {prefix.hex()} 的跨积 {combinations} 太大（不支持）"
            )
        decoded_long: set[bytes] = set()

        def enumerate_long(offset: int, current: list[int]) -> None:
            if offset == len(alphabets):
                sequence = prefix + bytes(current)
                try:
                    text = sequence.decode(codec)
                except UnicodeDecodeError as error:
                    if "incomplete multibyte sequence" in str(error):
                        raise SystemExit(
                            f"✗ {codec}: 长序列 {sequence.hex()} 仍报 incomplete（吸收态假设被打破）"
                        )
                    return
                if len(text) != 1:
                    raise SystemExit(f"✗ {codec}: 长序列 {sequence.hex()} 解出 {text!r}（不是单码位）")
                table[sequence] = text
                decoded_long.add(sequence)
                return
            for byte in alphabets[offset]:
                current.append(byte)
                enumerate_long(offset + 1, current)
                current.pop()

        enumerate_long(0, [])
        missing = [sequence for sequence in encoded_candidates if sequence not in decoded_long]
        if missing:
            raise SystemExit(
                f"✗ {codec}: 编码结果 {missing[0].hex()} 不在长分支解码表里（跨积字母表不全）"
            )
    return table, all_prefixes, long_prefixes


def expectation_map(codec: str, prefixes: list[bytes]) -> dict[bytes, int]:
    """每个**合法前缀状态**的期望总字节数（CPython 的 C 解码器用它区分 `incomplete`／`illegal`）。

    口径：从 `len(prefix)+1` 起，用 `0x00`／`0xFF` 两个填充字节补到总长 N，看参照还报不报
    `incomplete`；两个填充都不再 incomplete ⇒ 该状态期望 N 字节。两个填充结论不同 ⇒ 状态依赖
    （本表只支持定长状态机）⇒ 明确报错，不生成半张表 ✓。
    """
    expectations: dict[bytes, int] = {}
    for prefix in prefixes:
        depth = len(prefix)
        expected = None
        for total in range(depth + 1, MAX_SEQUENCE + 1):
            verdicts: list[str] = []
            for filler in (0x00, 0xFF):
                candidate = prefix + bytes([filler]) * (total - depth)
                try:
                    candidate.decode(codec)
                    verdicts.append("done")
                except UnicodeDecodeError as error:
                    verdicts.append(
                        "incomplete" if "incomplete multibyte sequence" in str(error) else "illegal"
                    )
            if "incomplete" not in verdicts:
                expected = total
                break
            if len(set(verdicts)) > 1:
                raise SystemExit(
                    f"✗ {codec}: 前缀 {prefix.hex()} 在总长 {total} 处状态依赖（{verdicts}）"
                    " —— 本工具只支持定长状态机"
                )
        if expected is None:
            raise SystemExit(f"✗ {codec}: 前缀 {prefix.hex()} 到 {MAX_SEQUENCE} 字节仍报 incomplete")
        expectations[prefix] = expected
    return expectations


def pair_encode_map(codec: str, decoded: dict[bytes, str]) -> dict[tuple[int, int], bytes]:
    """**两码位组合字符**的编码方向（JIS X 0213；单码位表装不下 ✓）。

    只对解码表里解出 2 个码位的序列做：把它们（`str`）交给参照 `encode`，得到规范编码字节。
    自检：编码结果必须回到原序列、且原序列解码结果仍是这两个码位（否则参照两侧不自洽 ⇒ 停）。
    """
    pairs: dict[tuple[int, int], bytes] = {}
    for sequence, text in decoded.items():
        if len(text) != 2:
            continue
        encoded = text.encode(codec)
        if encoded != sequence:
            raise SystemExit(
                f"✗ {codec}: 组合字符 {text!r} 的编码 {encoded.hex()} 与解码序列 {sequence.hex()} 不一致"
            )
        pairs[(ord(text[0]), ord(text[1]))] = encoded
    return pairs


def check_maps(codec: str, encoded: dict[int, bytes], decoded: dict[bytes, str]) -> list[str]:
    """对两张表做交叉自检（**不拿我们自己当标准** ✓——两张表各自都来自参照）。

    自检项：① 每条编码结果的字节序列应能被解码器认出（长度方向自洽）；
    ② 解码结果的每个码位应仍可编码（组合字符的第二个码位除外——那是 JIS X 0213 的既有形态）；
    ③ 记录多码位解码条数（Rust 表要能装下）。
    """
    problems: list[str] = []
    encoded_values = set(encoded.values())
    # **合法前缀集合**：吸收态下编码结果可以是"解码器眼中的前缀"（参照自身就不对称：`euc_kr` 的
    # `U+3164` 编成 `a4d4`，而 `b'\xa4\xd4'` 解出来是 `incomplete`）⇒ 编码值只要"在解码表里
    # **或**是解码表某条序列的真前缀"就算自洽 ✓（这是参照的既有形态，不是我们的偏差）。
    decoded_prefixes: set[bytes] = set()
    for sequence in decoded:
        for length in range(1, len(sequence)):
            decoded_prefixes.add(sequence[:length])
    for codepoint, sequence in encoded.items():
        if sequence not in decoded and sequence not in decoded_prefixes:
            problems.append(f"编码 {codepoint:04X}→{sequence.hex()} 不在解码表里（也不是前缀）")
        if len(problems) > 5:
            break
    # **如实口径**：解码表允许**比编码表多**（参照固有：`euc_jp` 的 `8fa2b7` 解 `~`、`johab` 有 17 条
    # 仅解码项；吸收态长分支已逐条核对回 encode 反向 ✓）⇒ 这里**不**要求 decode ⊂ encode。
    for sequence, text in decoded.items():
        if len(text) > 2:
            multi += 1
            problems.append(f"解码 {sequence.hex()} → {text!r}（{len(text)} 个码位，Rust 表只装 2 个）")
        if not text:
            problems.append(f"解码 {sequence.hex()} → 空串")
    return problems


def family_text(family: str, codecs: list[str]) -> tuple[str, str]:
    """生成一族的数据文件文本；返回 `(文本, 统计摘要)`。"""
    lines: list[str] = []
    lines.append("# pyawa 多字节编码表 —— **生成物，禁止手改**。")
    lines.append(f"# 生成：python3 tools/gen_codec_tables.py --family {family} --emit")
    lines.append(
        f"# 参照解释器：CPython {sys.version_info.major}.{sys.version_info.minor}."
        f"{sys.version_info.micro}"
    )
    lines.append("# 格式：`@ <codec>` 块开始；`E <码位:hex> <字节:hex>` 编码方向；")
    lines.append("#       `D <字节:hex> <码位:hex>[ <码位:hex>]` 解码方向（JIS X 0213 组合字符可两个码位）；")
    lines.append("#       `P <码位1:hex> <码位2:hex> <字节:hex>` 两码位组合字符的编码方向；")
    lines.append("#       `X <前缀:hex> <期望总长>` 前缀状态的结构长度（错误分型 incomplete／illegal 用）；")
    lines.append("#       `L <前缀:hex> <总长>` 吸收态长前缀（euc_kr 谚文 8 字节：引擎一步跳到总长再查表）。")
    summary: list[str] = []
    bodies: list[tuple[str, list[str], list[str]]] = []
    for codec in codecs:
        encoded = encode_map(codec)
        decoded, prefixes, long_prefixes = decode_map(codec, encoded)
        expectations = expectation_map(codec, prefixes)
        pairs = pair_encode_map(codec, decoded)
        problems = check_maps(codec, encoded, decoded)
        if problems:
            for problem in problems:
                print(f"✗ {codec}: {problem}", file=sys.stderr)
            raise SystemExit(f"✗ {codec} 自检失败（{len(problems)} 条）")
        multi = sum(1 for text in decoded.values() if len(text) > 1)
        summary.append(
            f"  {codec}: 可编码码位 {len(encoded)}；可解码序列 {len(decoded)}；组合字符 {len(pairs)} 对；"
            f"前缀状态 {len(expectations)} 个；长前缀（吸收态）{len(long_prefixes)} 个（最长 "
            f"{max((len(sequence) for sequence in decoded), default=0)} 字节）"
        )
        encoded_lines = [
            f"E {codepoint:04X} {encoded[codepoint].hex()}"
            for codepoint in sorted(encoded)
        ]
        decoded_lines = [
            f"D {sequence.hex()} " + " ".join(f"{ord(character):04X}" for character in decoded[sequence])
            for sequence in sorted(decoded, key=lambda value: (len(value), value))
        ]
        pair_lines = [
            f"P {first:04X} {second:04X} {pairs[(first, second)].hex()}"
            for first, second in sorted(pairs)
        ]
        expectation_lines = [
            f"X {prefix.hex()} {expectations[prefix]}"
            for prefix in sorted(expectations, key=lambda value: (len(value), value))
        ]
        long_lines = [
            f"L {prefix.hex()} {long_prefixes[prefix]}"
            for prefix in sorted(long_prefixes, key=lambda value: (len(value), value))
        ]
        bodies.append((codec, encoded_lines, decoded_lines, pair_lines, expectation_lines, long_lines))
    for codec, encoded_lines, decoded_lines, pair_lines, expectation_lines, long_lines in bodies:
        lines.append("")
        lines.append(f"@ {codec}")
        lines.extend(encoded_lines)
        lines.extend(decoded_lines)
        lines.extend(pair_lines)
        lines.extend(expectation_lines)
        lines.extend(long_lines)
    lines.append("")
    return "\n".join(lines), "\n".join(summary)


def main() -> int:
    parser = argparse.ArgumentParser(description="从参照 python3 dump 多字节编码映射表")
    parser.add_argument("codec", nargs="?", default="", help="单个编码名（旧行为：只打印统计）")
    parser.add_argument("--out", default="", help="单编码 TSV 输出路径（旧行为；只出编码方向）")
    parser.add_argument("--family", default="", choices=sorted(FAMILIES) + ["all"], help="按族生成/统计")
    parser.add_argument("--emit", action="store_true", help="把表写进 crates/pyawa-stdlib/src/codec_tables/")
    args = parser.parse_args()

    if not args.codec and not args.family:
        parser.error("要么给一个编码名（统计），要么用 `--family`（按族生成）")

    if args.codec:
        if args.codec in UNSUPPORTED:
            print(f"✗ {args.codec}：{UNSUPPORTED[args.codec]}", file=sys.stderr)
            return 2
        encoded = encode_map(args.codec)
        print(
            f"codec={args.codec}：可编码码位 {len(encoded)} 个；表内最大字节数 "
            f"{max((len(value) for value in encoded.values()), default=0)}"
        )
        spot = KNOWN.get(args.codec)
        if spot is not None:
            print(f"抽查：{spot!r} ⇒ {spot.encode(args.codec)!r}")
        if args.out:
            out = pathlib.Path(args.out)
            out.parent.mkdir(parents=True, exist_ok=True)
            with out.open("w", encoding="utf-8") as handle:
                for codepoint in sorted(encoded):
                    handle.write(f"{codepoint:04X}\t{encoded[codepoint].hex()}\n")
            print(f"已写出 {out}（{len(encoded)} 行）")
        return 0

    families = sorted(FAMILIES) if args.family == "all" else [args.family]
    TABLE_DIR.mkdir(parents=True, exist_ok=True)
    for family in families:
        text, summary = family_text(family, FAMILIES[family])
        print(f"family={family}")
        print(summary)
        if args.emit:
            target = TABLE_DIR / f"{family}.txt"
            target.write_text(text, encoding="utf-8")
            print(f"已写出 {target}（{text.count(chr(10)) + 1} 行，{len(text.encode('utf-8'))} 字节）")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
