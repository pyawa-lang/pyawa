//! **Unicode 名字表**（C2 冷门语义：`\N{…}` 具名转义）—— 数据由 `tools/gen_unicode_names.py` 生成。
//!
//! 口径（全照参照 CPython 3.14.4 实测 ✓）：
//! * 名字查询**大小写不敏感**（`\N{bullet}`／`\N{Bullet}` 都收 ✓）⇒ 查询前把 ASCII 折叠成大写 ✓；
//! * **算法名字**（`CJK UNIFIED IDEOGRAPH-XXXX`／`CJK COMPATIBILITY IDEOGRAPH-XXXX`，约 9.9 万条）
//!   只存码点**区间**：解析十六进制后缀后要求**规范写法**（`f"{cp:04X}"` 与后缀逐字节相同，
//!   因此 `04E00` 这类多前导零的名字**不**被接受 ✓）＋区间命中 ✓；
//! * 其余名字按 UTF-8 字节升序存表（约 5.1 万条，含 `HANGUL SYLLABLE …` 与别名）⇒ 二分查找 ✓。
//!
//! 数据格式见 `tools/gen_unicode_names.py` 的文件头（`magic`／`version`／偏移表／码点表／
//! CJK 区间表／名字字节区）；本模块只读不写，越界一律当"没有"（数据是构建期生成的常量 ✓）。

/// 生成数据（`tools/gen_unicode_names.py` 的产物；1.56 MB）。
static DATA: &[u8] = include_bytes!("unicode_names.bin");

/// 头：`magic[4] + version(u8) + reserved(u8) + count(u32) + cjk_count(u32) + names_len(u32)`。
/// 注意：版式由 `tools/gen_unicode_names.py` 写定 —— `magic` 后紧跟两个字节版本/保留位，
/// 三个 `u32` 从**偏移 6** 开始（不是 8！）
const HEADER_LEN: usize = 4 + 1 + 1 + 4 + 4 + 4;
const COUNT_OFFSET: usize = 6;
const CJK_COUNT_OFFSET: usize = 10;
const NAMES_LEN_OFFSET: usize = 14;

/// 解析后的表布局（每次查询都从常量数据现算，字段少、开销可忽略 ✓）。
struct Table {
    count: usize,
    cjk_count: usize,
    names_len: usize,
    offsets_base: usize,
    codepoints_base: usize,
    ranges_base: usize,
    names_base: usize,
}

fn read_u32(offset: usize) -> Option<u32> {
    let bytes = DATA.get(offset..offset + 4)?;
    Some(u32::from_le_bytes(bytes.try_into().ok()?))
}

fn table() -> Option<Table> {
    if DATA.get(..4)? != b"PYUN" || DATA.get(4).copied()? != 1 {
        return None;
    }
    let count = read_u32(COUNT_OFFSET)? as usize;
    let cjk_count = read_u32(CJK_COUNT_OFFSET)? as usize;
    let names_len = read_u32(NAMES_LEN_OFFSET)? as usize;
    let offsets_base = HEADER_LEN;
    let codepoints_base = offsets_base + (count + 1) * 4;
    let ranges_base = codepoints_base + count * 4;
    let names_base = ranges_base + cjk_count * 8;
    if names_base + names_len != DATA.len() {
        return None;
    }
    Some(Table {
        count,
        cjk_count,
        names_len,
        offsets_base,
        codepoints_base,
        ranges_base,
        names_base,
    })
}

/// 存表名字的字节区间（第 `index` 条）。
fn name_bytes(table: &Table, index: usize) -> Option<&'static [u8]> {
    let start = read_u32(table.offsets_base + index * 4)? as usize;
    let end = read_u32(table.offsets_base + (index + 1) * 4)? as usize;
    if end < start || end > table.names_len {
        return None;
    }
    DATA.get(table.names_base + start..table.names_base + end)
}

fn codepoint_at(table: &Table, index: usize) -> Option<u32> {
    read_u32(table.codepoints_base + index * 4)
}

/// CJK 码点是否落在生成数据记录的区间里（区间按 start 升序、互不相交 ✓）。
fn cjk_range_contains(table: &Table, codepoint: u32) -> bool {
    let mut low = 0usize;
    let mut high = table.cjk_count;
    while low < high {
        let middle = (low + high) / 2;
        let start = read_u32(table.ranges_base + middle * 8).unwrap_or(0);
        let end = read_u32(table.ranges_base + middle * 8 + 4).unwrap_or(0);
        if codepoint < start {
            high = middle;
        } else if codepoint > end {
            low = middle + 1;
        } else {
            return true;
        }
    }
    false
}

/// 解析 `PREFIX-XXXX` 的十六进制后缀（要求与 `f"{cp:04X}"` 逐字节相同 —— 规范写法 ✓）。
fn algorithmic_codepoint(suffix: &str) -> Option<u32> {
    if suffix.is_empty() || suffix.len() > 6 || !suffix.bytes().all(|byte| byte.is_ascii_hexdigit()) {
        return None;
    }
    let codepoint = u32::from_str_radix(suffix, 16).ok()?;
    let canonical = format!("{codepoint:04X}");
    if canonical != suffix {
        return None;
    }
    Some(codepoint)
}

const CJK_PREFIXES: [&str; 2] = [
    "CJK UNIFIED IDEOGRAPH-",
    "CJK COMPATIBILITY IDEOGRAPH-",
];

/// **按名字查码点**（`\N{…}` 的唯一入口）—— 大小写不敏感 ✓。
pub fn lookup(name: &str) -> Option<char> {
    // 参照的名字全是 ASCII（含空格/括号/连字符/数字）；含非 ASCII 一定不是合法名字 ✓。
    if !name.is_ascii() {
        return None;
    }
    // 折叠成大写（数据里存的是大写 ✓）；同时保持字节切片以免额外分配两次。
    let mut upper: Vec<u8> = name.bytes().map(|byte| byte.to_ascii_uppercase()).collect();
    let table = table()?;
    // ① 算法名字（CJK）：前缀＋规范十六进制后缀＋区间命中 ✓。
    for prefix in CJK_PREFIXES {
        let prefix_bytes = prefix.as_bytes();
        if upper.len() > prefix_bytes.len() && upper.starts_with(prefix_bytes) {
            let suffix = core::str::from_utf8(&upper[prefix_bytes.len()..]).ok()?;
            let codepoint = algorithmic_codepoint(suffix)?;
            if cjk_range_contains(&table, codepoint) {
                return char::from_u32(codepoint);
            }
        }
    }
    // ② 逐条名字：按 UTF-8 字节序二分查找 ✓。
    let mut low = 0usize;
    let mut high = table.count;
    while low < high {
        let middle = (low + high) / 2;
        let candidate = name_bytes(&table, middle)?;
        match candidate.cmp(&upper[..]) {
            core::cmp::Ordering::Less => low = middle + 1,
            core::cmp::Ordering::Greater => high = middle,
            core::cmp::Ordering::Equal => {
                upper.clear();
                return char::from_u32(codepoint_at(&table, middle)?);
            }
        }
    }
    upper.clear();
    None
}

#[cfg(test)]
mod tests {
    use super::lookup;

    #[test]
    fn looks_up_ordinary_names_case_insensitively() {
        assert_eq!(lookup("BULLET"), Some('•'));
        assert_eq!(lookup("bullet"), Some('•'));
        assert_eq!(lookup("Bullet"), Some('•'));
        assert_eq!(lookup("SNOWMAN"), Some('☃'));
        assert_eq!(lookup("LATIN CAPITAL LETTER A"), Some('A'));
        assert_eq!(lookup("GREEK SMALL LETTER ALPHA"), Some('α'));
        assert_eq!(lookup("NO SUCH NAME"), None);
        assert_eq!(lookup(""), None);
    }

    #[test]
    fn looks_up_aliases() {
        assert_eq!(lookup("NULL"), Some('\u{0}'));
        assert_eq!(lookup("LF"), Some('\n'));
        assert_eq!(lookup("BYTE ORDER MARK"), Some('\u{FEFF}'));
    }

    #[test]
    fn looks_up_algorithmic_and_hangul_names() {
        assert_eq!(lookup("CJK UNIFIED IDEOGRAPH-4E00"), Some('一'));
        assert_eq!(lookup("cjk unified ideograph-4e00"), Some('一'));
        assert_eq!(lookup("CJK UNIFIED IDEOGRAPH-04E00"), None, "多前导零不是规范名字");
        assert_eq!(lookup("HANGUL SYLLABLE GA"), Some('가'));
    }
}
