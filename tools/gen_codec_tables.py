#!/usr/bin/env python3
"""**从参照 `python3` 生成多字节编码的映射表**（新目标"多字节编解码族"的工作件 ✓）。

动因 ✓：`_codecs_jp`／`_codecs_cn`／`_codecs_kr`… 的映射表在 CPython 的 **C 模块**里 ✗，
`Lib/` 里拿不到 ✗（实测：`Lib/encodings/cp932.py` 已同步 ✓，但 `import encodings.cp932` ⇒
`No module named '_codecs_jp'` ✗）⇒ 只能**从参照 dump** ✓ ⇒ 生成 Rust 表 ✓（与 `unicode_tables.rs`
同源的思路 ✓）。

用法::

    python3 tools/gen_codec_tables.py cp949            # 打印统计 ＋ 抽查
    python3 tools/gen_codec_tables.py cp949 --out target/recon/cp949.tsv

口径 ✓：对**每个码位**调参照的 `chr(cp).encode(codec)` ⇒ 成功则记 `cp<TAB>hexbytes` ✓
（单向表 ✓；解码方向可由该表反转 ＋ 查参照的兼容映射 ✓ —— 这一步留待接线时按需补齐 ✓）。
**如实标注** ✗：本工具只生成**数据** ✓，不实现 `_codecs_*` 的编解码状态机 ✗（那是接线的工作 ✓）。
"""

from __future__ import annotations

import argparse
import pathlib
import sys

KNOWN = {"cp949": "가", "euc_kr": "가", "shift_jis": "あ", "euc_jp": "あ", "gb2312": "中", "big5": "中"}


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("codec")
    parser.add_argument("--out", default="")
    args = parser.parse_args()

    rows: list[tuple[int, bytes]] = []
    for codepoint in range(0x110000):
        if 0xD800 <= codepoint <= 0xDFFF:
            continue
        text = chr(codepoint)
        try:
            encoded = text.encode(args.codec)
        except (UnicodeEncodeError, LookupError):
            continue
        rows.append((codepoint, encoded))

    print(f"codec={args.codec}：可编码码位 {len(rows)} 个；表内最大字节数 "
          f"{max((len(b) for _, b in rows), default=0)}")
    spot = KNOWN.get(args.codec)
    if spot is not None:
        print(f"抽查：{spot!r} ⇒ {spot.encode(args.codec)!r}")
    if args.out:
        out = pathlib.Path(args.out)
        out.parent.mkdir(parents=True, exist_ok=True)
        with out.open("w", encoding="utf-8") as handle:
            for codepoint, encoded in rows:
                handle.write(f"{codepoint:04X}\t{encoded.hex()}\n")
        print(f"已写出 {out}（{len(rows)} 行）")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
