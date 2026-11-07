#!/usr/bin/env python3
"""生成**规范化**所需的组合表（`unicode_composition.rs`）。

原则（照 `tools/gen_unicode.py` 的 `CM-13`／`CM-22`）：数据**只能**从参照实现导出，禁止手写、禁止
换 Unicode crate。这里从 `unicodedata` 探测：

- **规范组合对** `(前导码点, 组合码点) -> 组合结果码点`：对每个码点取 `decomposition()`（无 `<tag>`
  的一级规范分解），二元对再用 `normalize('NFC', …)` 验证——**能组合**才收（这一步天然排除了
  规范排除表里的码点 ✓，不必另导出排除表）。
- 组合对按 `(a, b)` 升序排好，运行期二分查找 ✓。

用法：`python3 tools/gen_unicode_normalization.py > crates/pyawa-stdlib/src/unicode_composition.rs`
"""

import sys
import unicodedata


def main() -> int:
    pairs: dict[tuple[int, int], int] = {}
    for code_point in range(0x110000):
        character = chr(code_point)
        decomposition = unicodedata.decomposition(character)
        if not decomposition or decomposition.startswith("<"):
            continue
        parts = decomposition.split()
        if len(parts) != 2:
            continue
        first, second = int(parts[0], 16), int(parts[1], 16)
        if unicodedata.normalize("NFC", chr(first) + chr(second)) == character:
            pairs[(first, second)] = code_point
    items = sorted(pairs.items())
    out = sys.stdout
    out.write(
        "//! **规范化组合表**（生成产物；`round 107` 引入）。\n"
        "//!\n"
        "//! 由 `tools/gen_unicode_normalization.py` 探测参照实现导出——**禁止手写**、**禁止**改用\n"
        "//! Rust 生态的 Unicode crate（与 `unicode_tables.rs` 同一口径：版本不由我们控制 ✗）。\n"
        f"//! 本文件由脚本生成于参照实现 `unicodedata.unidata_version == {unicodedata.unidata_version}` ✓。\n"
        "//!\n"
        "//! 语义：**规范组合对** `(前导, 组合) -> 结果` ✓；已用 `normalize('NFC', …)` 验证（天然排除\n"
        "//! 规范排除码点 ✓）。运行期二分查找 ✓。\n"
        "\n"
        "/// 本表的 Unicode 版本（与 `unicode_tables::UNIDATA_VERSION` 必须一致 ✓）。\n"
        f'pub const UNIDATA_VERSION: &str = "{unicodedata.unidata_version}";\n'
        "\n"
        "/// 规范组合对（按 `(a, b)` 升序 ✓）。\n"
        "pub static COMPOSITION_PAIRS: &[(u32, u32, u32)] = &[\n"
    )
    for (a, b), composed in items:
        out.write(f"    ({a:#x}, {b:#x}, {composed:#x}),\n")
    out.write("];\n\n")
    out.write(
        "/// 查规范组合对（`None` ＝ 不组合 ✓）。\n"
        "pub fn compose_pair(a: u32, b: u32) -> Option<u32> {\n"
        "    let mut low = 0usize;\n"
        "    let mut high = COMPOSITION_PAIRS.len();\n"
        "    while low < high {\n"
        "        let middle = (low + high) / 2;\n"
        "        let (first, second, composed) = COMPOSITION_PAIRS[middle];\n"
        "        if (first, second) < (a, b) {\n"
        "            low = middle + 1;\n"
        "        } else if (first, second) > (a, b) {\n"
        "            high = middle;\n"
        "        } else {\n"
        "            return Some(composed);\n"
        "        }\n"
        "    }\n"
        "    None\n"
        "}\n"
    )
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
