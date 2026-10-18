//! **CJK 多字节编解码族的对拍护栏**（round 10 task-6 ＋ round 11 task-9）。
//! 契约见 `docs/SPEC-c-modules.md` §5.2.18。期望值**全部取自参照 `python3` 3.14.4 对同一脚本的
//! 实际输出**（不拿我们自己的输出当标准 ✓）：把下面的 `*_SCRIPT` 原样交给参照解释器，取其 stdout
//! 钉成 `*_OUTPUT`，再让 Pyawa 跑同一脚本逐字节比较。
//!
//! **三个 test** 分别守三代落地面：
//!
//! 1. `multibyte_codec_family_matches_the_reference_byte_for_byte`（round 10）：`_multibytecodec`
//!    ＋ `_codecs_jp` 一族 —— 7 个 `Lib/encodings/*` 真 import；`getcodec` 的
//!    `(bytes, 消耗码位数)`／`(str, 消耗字节数)` 返回形态；`errors` 三态与错误文案
//!    （`illegal`／`incomplete` 分型、`bytes in position 0-1` 复数形态、字符字面量转义）；
//!    JIS X 0213 两码位组合字符（解码 2 码位／编码合成 1 条序列，含跨 chunk 留尾）；
//!    `codecs.lookup`／别名／增量编解码器／流 Reader・Writer。
//! 2. `core_encoding_delegation_matches_the_reference_byte_for_byte`（round 11）：**core 的编码
//!    委派** —— `str.encode`／`bytes.decode`／`str(bytes, enc, errors)` 对 7 个 jp 编码与内置四档
//!    （utf-8／ascii／latin-1／cp437）的 `errors` 三态与错误文案；未知编码的 `LookupError`；
//!    `bytes.decode` 的关键字形态；`encodings` **charmap 族**（cp1252）经委派可用；第二族
//!    `_codecs_cn`（`gb2312`／`gbk`，`cp936` 别名）；`import codecs` 后 `codecs.lookup('cp932')`
//!    的 **lazy `encodings`** 语义（脚本里此前没导入 encodings）。
//! 3. `tw_and_kr_families_match_the_reference_byte_for_byte`（round 12）：**`_codecs_tw`**（`big5`／
//!    `cp950`）＋**`_codecs_kr`**（`euc_kr`／`cp949`／`johab`）—— 5 个模块真 import、`getcodec`
//!    返回形态与**按族限定**（`_codecs_tw.getcodec('gbk')` ⇒ `LookupError: no such codec is
//!    supported.`）、`str.encode`／`bytes.decode` 委派与 errors 三态、`incomplete`／`illegal`
//!    分型、别名、非文本编码器守卫；另含 **`euc_kr` 的 8 字节谚文分解**（吸收态长前缀：截断报
//!    `incomplete`、凑满 8 字节不匹配报 `illegal`、`ignore`／`replace` 的跨度，以及参照自身的
//!    `U+3164`→`a4d4` 不对称）。
//!
//! 表数据与引擎的**生成路径**见 `crates/pyawa-stdlib/src/multibyte_module.rs` 模块头
//! （`python3 tools/gen_codec_tables.py --family {jp,cn,tw,kr} --emit`）；
//! 委派口径见 `crates/pyawa-core/src/instance/convert.rs` 的 `builtin_encoding` 注释。

fn run(name: &str, script: &str) -> String {
    let root = std::env::temp_dir().join(format!("pyawa-multibyte-{}-{name}", std::process::id()));
    let _ = std::fs::remove_dir_all(&root);
    std::fs::create_dir_all(&root).expect("建临时目录");
    let path = root.join("probe.py");
    std::fs::write(&path, script).expect("写脚本");
    let output = std::process::Command::new(env!("CARGO_BIN_EXE_pyawa"))
        .arg(&path)
        .output()
        .expect("跑 CLI");
    let _ = std::fs::remove_dir_all(&root);
    assert!(
        output.status.success(),
        "脚本应当成功（exit={:?}）；stderr={}",
        output.status,
        String::from_utf8_lossy(&output.stderr)
    );
    String::from_utf8_lossy(&output.stdout).into_owned()
}

// ---- round 10：`_multibytecodec` ＋ `_codecs_jp` 一族 -------------------------------------------

/// 参照与 Pyawa 用同一份脚本（只有两侧都有的公开面：`_codecs_jp`／`encodings`／`codecs`／`io`）。
const FAMILY_SCRIPT: &str = r##""""CJK 多字节编解码族（`_codecs_jp`）的**参照对拍探针**：同一份脚本在参照 `python3` 与 Pyawa 上跑，
输出必须逐字节一致（测试里把参照输出钉成期望值）。只用两侧都有的公开面：`_codecs_jp`、
`encodings.*`、`codecs`、`io.BytesIO`。"""

import codecs
import io

# ---- ① 7 个模块都能 import，且 getcodec 交回的 codec 能编解码 ----
import encodings.cp932
import encodings.euc_jp
import encodings.euc_jis_2004
import encodings.euc_jisx0213
import encodings.shift_jis
import encodings.shift_jis_2004
import encodings.shift_jisx0213

SAMPLES = [
    ("cp932", "日本語テスト"),
    ("euc_jp", "日本語テスト"),
    ("euc_jis_2004", "日本語テスト"),
    ("euc_jisx0213", "日本語テスト"),
    ("shift_jis", "日本語テスト"),
    ("shift_jis_2004", "日本語テスト"),
    ("shift_jisx0213", "日本語テスト"),
]
for name, text in SAMPLES:
    module = getattr(encodings, name)
    encoded = module.codec.encode(text)
    decoded = module.codec.decode(encoded[0])
    print(name, encoded, decoded)

# ---- ② 逐编码往返（不同内容：ASCII／假名／汉字／符号／半角） ----
ROUNDTRIP = [
    ("cp932", "hello, world"),
    ("euc_jp", "あいうえお"),
    ("euc_jis_2004", "漢字とかな"),
    ("euc_jisx0213", "①②③④⑤"),
    ("shift_jis", "ｱｲｳｴｵ"),
    ("shift_jis_2004", "〜−"),
    ("shift_jisx0213", "か\u309aき\u309a"),
]
for name, text in ROUNDTRIP:
    codec = __import__("_codecs_jp").getcodec(name)
    encoded, used = codec.encode(text)
    decoded, eaten = codec.decode(encoded)
    print(name, encoded, used, decoded, eaten, decoded == text)

# ---- ③ 错误处理三条（strict／ignore／replace）与错误文案／异常类型 ----
ERRORS = [
    ("cp932", "あ\u2603い"),
    ("euc_jp", "あ\u2603い"),
    ("euc_jis_2004", "あ\u00a5い"),
    ("euc_jisx0213", "\u203e"),
    ("shift_jis", "あ\uff5eい"),
    ("shift_jis_2004", "あ\uff5eい"),
    ("shift_jisx0213", "\u00a5"),
]
for name, text in ERRORS:
    codec = __import__("_codecs_jp").getcodec(name)
    for errors in ("ignore", "replace"):
        print(name, errors, codec.encode(text, errors))
    try:
        codec.encode(text)
    except UnicodeEncodeError as error:
        print(name, "encode-strict", str(error))
    except Exception as error:  # noqa: BLE001 - 对拍要钉住类型
        print(name, "encode-strict-other", type(error).__name__, str(error))

DECODE_ERRORS = [
    ("cp932", b"\x82\xa0\x81"),
    ("euc_jp", b"\x82\xa0\x8f\xa1"),
    ("euc_jis_2004", b"\xa4\xa2\xff"),
    ("euc_jisx0213", b"\xa4\xa2\x8f"),
    ("shift_jis", b"\x82\xa0\x80"),
    ("shift_jis_2004", b"\x82\xa0\xff"),
    ("shift_jisx0213", b"\x82\xa0\x81"),
]
for name, raw in DECODE_ERRORS:
    codec = __import__("_codecs_jp").getcodec(name)
    for errors in ("ignore", "replace"):
        print(name, "decode", errors, codec.decode(raw, errors))
    try:
        codec.decode(raw)
    except UnicodeDecodeError as error:
        print(name, "decode-strict", str(error))
    except Exception as error:  # noqa: BLE001
        print(name, "decode-strict-other", type(error).__name__, str(error))

# ---- ④ JIS X 0213 的组合字符（一条字节序列解出两个码位） ----
COMBINING = [
    ("euc_jis_2004", b"\xa4\xf7"),
    ("euc_jisx0213", b"\xa4\xf7"),
    ("shift_jis_2004", b"\x82\xf5"),
    ("shift_jisx0213", b"\x82\xf5"),
]
for name, raw in COMBINING:
    codec = __import__("_codecs_jp").getcodec(name)
    decoded, eaten = codec.decode(raw)
    print(name, "combining", repr(decoded), len(decoded), eaten)

# ---- ④b 组合字符的**编码方向**（两码位合成一条序列；单码位不可编码） ----
PAIRS = [
    ("euc_jis_2004", "か\u309a"),
    ("euc_jisx0213", "か\u309a"),
    ("shift_jis_2004", "か\u309a"),
    ("shift_jisx0213", "か\u309a"),
]
for name, text in PAIRS:
    codec = __import__("_codecs_jp").getcodec(name)
    print(name, "pair", codec.encode(text), codec.encode(text, "ignore"), codec.encode(text, "replace"))
    print(name, "pair-mixed", codec.encode("X" + text + "Y"), codec.encode(text + text))
    try:
        codec.encode("\u309a")
    except UnicodeEncodeError as error:
        print(name, "pair-single-strict", str(error))
encoder = codecs.getincrementalencoder("shift_jis_2004")()
print("incremental-pair", encoder.encode("か", False), encoder.encode("\u309a", False))
encoder.reset()
print("incremental-pair-final", encoder.encode("か", True))

# ---- ⑤ 参数形态：非 str 的 encode／非 bytes 的 decode／未知编码／未知错误处理器 ----
codec = __import__("_codecs_jp").getcodec("cp932")
print("encode-bytes-arg", codec.encode(b"ab"))
print("encode-empty", codec.encode(""), codec.decode(b""))
try:
    codec.decode("あ")
except TypeError as error:
    print("decode-str-arg", str(error))
try:
    codec.decode(123)
except TypeError as error:
    print("decode-int-arg", str(error))
try:
    __import__("_codecs_jp").getcodec("jisx0213")
except LookupError as error:
    print("getcodec-unknown", str(error))
try:
    codec.encode("あ", "bogus-handler")
except LookupError as error:
    print("encode-bad-handler", str(error))

# ---- ⑥ `codecs` 注册表路径（lookup／别名／增量编解码器／流包装） ----
info = codecs.lookup("cp932")
print("lookup", info.name, codecs.lookup("932").name, codecs.lookup("ms932").name)
print("lookup-encode", info.encode("あい"), info.decode(b"\x82\xa0\x82\xa2"))
encoder = codecs.getincrementalencoder("cp932")()
print("incremental-encode", encoder.encode("あ"), encoder.encode("い", True), repr(encoder.getstate()))
decoder = codecs.getincrementaldecoder("cp932")()
print("incremental-decode", repr(decoder.decode(b"\x82", False)), repr(decoder.decode(b"\xa0", False)))
print("incremental-final", repr(decoder.decode(b"", True)), repr(decoder.getstate()))
decoder.reset()
print("incremental-reset", repr(decoder.decode(b"\x82\xa0", True)))
reader = codecs.getreader("euc_jp")(io.BytesIO(b"\xa4\xa2\xa4\xa4"))
print("stream-reader", reader.read())
buffer = io.BytesIO()
writer = codecs.getwriter("euc_jp")(buffer)
writer.write("あい")
print("stream-writer", buffer.getvalue())

# ---- ⑦ 解码方向的边界（截断前缀的 ignore／replace 口径） ----
for raw in (b"\x8f", b"\x8f\xa1", b"\x8f\xa1\x20"):
    codec = __import__("_codecs_jp").getcodec("euc_jp")
    for errors in ("ignore", "replace"):
        print("truncated", raw, errors, repr(codec.decode(raw, errors)))
    try:
        codec.decode(raw)
    except UnicodeDecodeError as error:
        print("truncated", raw, "strict", str(error))"##;

/// 参照 `python3` 3.14.4 对 `FAMILY_SCRIPT` 的实际 stdout（**逐字节**对拍）。
const FAMILY_OUTPUT: &str = r##"cp932 (b'\x93\xfa\x96{\x8c\xea\x83e\x83X\x83g', 6) ('日本語テスト', 12)
euc_jp (b'\xc6\xfc\xcb\xdc\xb8\xec\xa5\xc6\xa5\xb9\xa5\xc8', 6) ('日本語テスト', 12)
euc_jis_2004 (b'\xc6\xfc\xcb\xdc\xb8\xec\xa5\xc6\xa5\xb9\xa5\xc8', 6) ('日本語テスト', 12)
euc_jisx0213 (b'\xc6\xfc\xcb\xdc\xb8\xec\xa5\xc6\xa5\xb9\xa5\xc8', 6) ('日本語テスト', 12)
shift_jis (b'\x93\xfa\x96{\x8c\xea\x83e\x83X\x83g', 6) ('日本語テスト', 12)
shift_jis_2004 (b'\x93\xfa\x96{\x8c\xea\x83e\x83X\x83g', 6) ('日本語テスト', 12)
shift_jisx0213 (b'\x93\xfa\x96{\x8c\xea\x83e\x83X\x83g', 6) ('日本語テスト', 12)
cp932 b'hello, world' 12 hello, world 12 True
euc_jp b'\xa4\xa2\xa4\xa4\xa4\xa6\xa4\xa8\xa4\xaa' 5 あいうえお 10 True
euc_jis_2004 b'\xb4\xc1\xbb\xfa\xa4\xc8\xa4\xab\xa4\xca' 5 漢字とかな 10 True
euc_jisx0213 b'\xad\xa1\xad\xa2\xad\xa3\xad\xa4\xad\xa5' 5 ①②③④⑤ 10 True
shift_jis b'\xb1\xb2\xb3\xb4\xb5' 5 ｱｲｳｴｵ 5 True
shift_jis_2004 b'\x81`\x81|' 2 〜− 4 True
shift_jisx0213 b'\x82\xf5\x82\xf6' 4 か゚き゚ 4 True
cp932 ignore (b'\x82\xa0\x82\xa2', 3)
cp932 replace (b'\x82\xa0?\x82\xa2', 3)
cp932 encode-strict 'cp932' codec can't encode character '\u2603' in position 1: illegal multibyte sequence
euc_jp ignore (b'\xa4\xa2\xa4\xa4', 3)
euc_jp replace (b'\xa4\xa2?\xa4\xa4', 3)
euc_jp encode-strict 'euc_jp' codec can't encode character '\u2603' in position 1: illegal multibyte sequence
euc_jis_2004 ignore (b'\xa4\xa2\xa4\xa4', 3)
euc_jis_2004 replace (b'\xa4\xa2?\xa4\xa4', 3)
euc_jis_2004 encode-strict 'euc_jis_2004' codec can't encode character '\xa5' in position 1: illegal multibyte sequence
euc_jisx0213 ignore (b'', 1)
euc_jisx0213 replace (b'?', 1)
euc_jisx0213 encode-strict 'euc_jisx0213' codec can't encode character '\u203e' in position 0: illegal multibyte sequence
shift_jis ignore (b'\x82\xa0\x82\xa2', 3)
shift_jis replace (b'\x82\xa0?\x82\xa2', 3)
shift_jis encode-strict 'shift_jis' codec can't encode character '\uff5e' in position 1: illegal multibyte sequence
shift_jis_2004 ignore (b'\x82\xa0\x82\xa2', 3)
shift_jis_2004 replace (b'\x82\xa0?\x82\xa2', 3)
shift_jis_2004 encode-strict 'shift_jis_2004' codec can't encode character '\uff5e' in position 1: illegal multibyte sequence
shift_jisx0213 ignore (b'\\', 1)
shift_jisx0213 replace (b'\\', 1)
cp932 decode ignore ('あ', 3)
cp932 decode replace ('あ�', 3)
cp932 decode-strict 'cp932' codec can't decode byte 0x81 in position 2: incomplete multibyte sequence
euc_jp decode ignore ('', 4)
euc_jp decode replace ('���', 4)
euc_jp decode-strict 'euc_jp' codec can't decode byte 0x82 in position 0: illegal multibyte sequence
euc_jis_2004 decode ignore ('あ', 3)
euc_jis_2004 decode replace ('あ�', 3)
euc_jis_2004 decode-strict 'euc_jis_2004' codec can't decode byte 0xff in position 2: incomplete multibyte sequence
euc_jisx0213 decode ignore ('あ', 3)
euc_jisx0213 decode replace ('あ�', 3)
euc_jisx0213 decode-strict 'euc_jisx0213' codec can't decode byte 0x8f in position 2: incomplete multibyte sequence
shift_jis decode ignore ('あ', 3)
shift_jis decode replace ('あ�', 3)
shift_jis decode-strict 'shift_jis' codec can't decode byte 0x80 in position 2: illegal multibyte sequence
shift_jis_2004 decode ignore ('あ', 3)
shift_jis_2004 decode replace ('あ�', 3)
shift_jis_2004 decode-strict 'shift_jis_2004' codec can't decode byte 0xff in position 2: illegal multibyte sequence
shift_jisx0213 decode ignore ('あ', 3)
shift_jisx0213 decode replace ('あ�', 3)
shift_jisx0213 decode-strict 'shift_jisx0213' codec can't decode byte 0x81 in position 2: incomplete multibyte sequence
euc_jis_2004 combining 'か゚' 2 2
euc_jisx0213 combining 'か゚' 2 2
shift_jis_2004 combining 'か゚' 2 2
shift_jisx0213 combining 'か゚' 2 2
euc_jis_2004 pair (b'\xa4\xf7', 2) (b'\xa4\xf7', 2) (b'\xa4\xf7', 2)
euc_jis_2004 pair-mixed (b'X\xa4\xf7Y', 4) (b'\xa4\xf7\xa4\xf7', 4)
euc_jis_2004 pair-single-strict 'euc_jis_2004' codec can't encode character '\u309a' in position 0: illegal multibyte sequence
euc_jisx0213 pair (b'\xa4\xf7', 2) (b'\xa4\xf7', 2) (b'\xa4\xf7', 2)
euc_jisx0213 pair-mixed (b'X\xa4\xf7Y', 4) (b'\xa4\xf7\xa4\xf7', 4)
euc_jisx0213 pair-single-strict 'euc_jisx0213' codec can't encode character '\u309a' in position 0: illegal multibyte sequence
shift_jis_2004 pair (b'\x82\xf5', 2) (b'\x82\xf5', 2) (b'\x82\xf5', 2)
shift_jis_2004 pair-mixed (b'X\x82\xf5Y', 4) (b'\x82\xf5\x82\xf5', 4)
shift_jis_2004 pair-single-strict 'shift_jis_2004' codec can't encode character '\u309a' in position 0: illegal multibyte sequence
shift_jisx0213 pair (b'\x82\xf5', 2) (b'\x82\xf5', 2) (b'\x82\xf5', 2)
shift_jisx0213 pair-mixed (b'X\x82\xf5Y', 4) (b'\x82\xf5\x82\xf5', 4)
shift_jisx0213 pair-single-strict 'shift_jisx0213' codec can't encode character '\u309a' in position 0: illegal multibyte sequence
incremental-pair b'' b'\x82\xf5'
incremental-pair-final b'\x82\xa9'
encode-bytes-arg (b"b'ab'", 5)
encode-empty (b'', 0) ('', 0)
decode-str-arg a bytes-like object is required, not 'str'
decode-int-arg a bytes-like object is required, not 'int'
getcodec-unknown no such codec is supported.
lookup cp932 cp932 cp932
lookup-encode (b'\x82\xa0\x82\xa2', 2) ('あい', 4)
incremental-encode b'\x82\xa0' b'\x82\xa2' 0
incremental-decode '' 'あ'
incremental-final '' (b'', 0)
incremental-reset 'あ'
stream-reader あい
stream-writer b'\xa4\xa2\xa4\xa4'
truncated b'\x8f' ignore ('', 1)
truncated b'\x8f' replace ('�', 1)
truncated b'\x8f' strict 'euc_jp' codec can't decode byte 0x8f in position 0: incomplete multibyte sequence
truncated b'\x8f\xa1' ignore ('', 2)
truncated b'\x8f\xa1' replace ('�', 2)
truncated b'\x8f\xa1' strict 'euc_jp' codec can't decode bytes in position 0-1: incomplete multibyte sequence
truncated b'\x8f\xa1 ' ignore (' ', 3)
truncated b'\x8f\xa1 ' replace ('�� ', 3)
truncated b'\x8f\xa1 ' strict 'euc_jp' codec can't decode byte 0x8f in position 0: illegal multibyte sequence
"##;

#[test]
fn multibyte_codec_family_matches_the_reference_byte_for_byte() {
    let stdout = run("family", FAMILY_SCRIPT);
    assert_eq!(
        stdout, FAMILY_OUTPUT,
        "`_codecs_jp` 一族的输出与参照不一致（逐字节对拍；先看上面 diff 的期望/实际）"
    );
}

// ---- round 11：core 编码委派 ＋ `codecs.lookup` lazy encodings ---------------------------------

/// 同一脚本两侧跑；**不预先** `import encodings` —— 第 ⑨ 段就是在验 lazy 语义。
const DELEGATION_SCRIPT: &str = r##""""round 11 对拍：core 编码委派（`str.encode`／`bytes.decode`／`str(bytes, enc, errors)`）＋
`codecs.lookup` lazy encodings。同一脚本两侧跑，输出必须逐字节一致。"""

import codecs
import sys

CODECS = [
    "cp932",
    "euc_jp",
    "euc_jis_2004",
    "euc_jisx0213",
    "shift_jis",
    "shift_jis_2004",
    "shift_jisx0213",
]

# ---- ① 7 个 jp 编码走 str.encode／bytes.decode 的往返 ----
CASES = [
    ("cp932", "日本語テスト"),
    ("euc_jp", "あいうえお"),
    ("euc_jis_2004", "漢字とかな"),
    ("euc_jisx0213", "①②③④⑤"),
    ("shift_jis", "ｱｲｳｴｵ"),
    ("shift_jis_2004", "〜−"),
    ("shift_jisx0213", "か\u309aき\u309a"),
]
for name, text in CASES:
    encoded = text.encode(name)
    decoded = encoded.decode(name)
    print(name, encoded, decoded, decoded == text)

# ---- ② errors 三态：编码方向（可编码＋不可编码混合） ----
ENCODE_CASES = [
    ("cp932", "あ\u2603い"),
    ("euc_jp", "あ\u2603い"),
    ("euc_jis_2004", "あ\u00a5い"),
    ("euc_jisx0213", "\u203e"),
    ("shift_jis", "あ\uff5eい"),
    ("shift_jis_2004", "あ\uff5eい"),
    ("shift_jisx0213", "\u00a5"),
]
for name, text in ENCODE_CASES:
    for errors in ("ignore", "replace"):
        print(name, "encode", errors, text.encode(name, errors))
    try:
        text.encode(name)
    except UnicodeEncodeError as error:
        print(name, "encode strict", str(error))
    except Exception as error:  # noqa: BLE001
        print(name, "encode strict other", type(error).__name__, str(error))

# ---- ③ errors 三态：解码方向（含截断前缀与非法字节） ----
DECODE_CASES = [
    ("cp932", b"\x82\xa0\x81"),
    ("euc_jp", b"\x82\xa0\x8f\xa1"),
    ("euc_jis_2004", b"\xa4\xa2\xff"),
    ("euc_jisx0213", b"\xa4\xa2\x8f"),
    ("shift_jis", b"\x82\xa0\x80"),
    ("shift_jis_2004", b"\x82\xa0\xff"),
    ("shift_jisx0213", b"\x82\xa0\x81"),
]
for name, raw in DECODE_CASES:
    for errors in ("ignore", "replace"):
        print(name, "decode", errors, raw.decode(name, errors))
    try:
        raw.decode(name)
    except UnicodeDecodeError as error:
        print(name, "decode strict", str(error))
    except Exception as error:  # noqa: BLE001
        print(name, "decode strict other", type(error).__name__, str(error))

# ---- ④ 内置四档的 errors 三态（快路径）与错误文案 ----
BUILTIN_ENCODE = [
    ("ascii", "abc\xe9"),
    ("latin-1", "caf\u00e9\u20ac"),
    ("cp437", "\u00c7\u3042"),
    ("utf-8", "\u3042\u2603"),
]
for name, text in BUILTIN_ENCODE:
    for errors in ("ignore", "replace"):
        print("builtin", name, "encode", errors, text.encode(name, errors))
    try:
        text.encode(name)
    except UnicodeEncodeError as error:
        print("builtin", name, "encode strict", str(error))
BUILTIN_DECODE = [
    ("ascii", b"ab\xe9"),
    ("latin-1", b"\xe9\xff"),
    ("cp437", b"\x80\xff"),
    ("utf-8", b"ab\xe3\x81"),
    ("utf-8", b"ab\xe3\x28\xa1"),
    ("utf-8", b"ab\xff"),
]
for name, raw in BUILTIN_DECODE:
    for errors in ("ignore", "replace"):
        print("builtin", name, "decode", errors, repr(raw.decode(name, errors)))
    try:
        raw.decode(name)
    except UnicodeDecodeError as error:
        print("builtin", name, "decode strict", str(error))

# ---- ⑤ `str(bytes, encoding, errors)`（第三个实参真接） ----
for raw, encoding, errors in [
    (b"\xe9", "ascii", "replace"),
    (b"\xe9", "ascii", "ignore"),
    (b"\xe9", "latin-1", "strict"),
    (b"\x82\xa0", "cp932", "strict"),
    (b"\x82\xa0\x81", "cp932", "replace"),
    (b"\x8f", "euc_jp", "ignore"),
]:
    print("str()", raw, encoding, errors, str(raw, encoding, errors))

# ---- ⑥ 未知编码的 LookupError 文案／类型（保留调用方原样大小写） ----
for name in ["NoSuch", "no such", "CP99999"]:
    try:
        "a".encode(name)
    except LookupError as error:
        print("unknown-encode", type(error).__name__, ":", error)
    try:
        b"a".decode(name)
    except LookupError as error:
        print("unknown-decode", type(error).__name__, ":", error)

# ---- ⑦ bytes.decode 的关键字形态 ----
print("kwargs-string", b"\x82\xa0".decode(encoding="cp932", errors="strict"))
print("kwargs-errors", b"a".decode(errors="strict"))
print("kwargs-positional-both", b"\x82\xa0".decode("cp932", "ignore"))

# ---- ⑧ 委派给 `encodings` 的**非 jp** 编码（charmap 族） ----
print("cp1252", "\u00e9\u20ac".encode("cp1252"), b"\xe9\x80".decode("cp1252"))
print("cp1252-replace", "\u3042".encode("cp1252", "replace"))

# ---- ⑨ `codecs.lookup` 的 lazy `encodings`（本进程此前**没**导入 encodings） ----
print("lazy-before", "encodings" in sys.modules)
print("lazy-name", codecs.lookup("cp932").name)
print("lazy-after", "encodings" in sys.modules)
print("lazy-encode", codecs.lookup("cp932").encode("\u3042"))
print("lazy-decode", codecs.lookup("cp932").decode(b"\x82\xa0"))
print("lazy-alias", codecs.lookup("932").name, codecs.lookup("mskanji").name)

# ---- ⑩ errors 处理器名：未注册 ⇒ 照参照 LookupError（内置快路径与委派两路） ----
for text, name in [("\u00e9", "ascii"), ("\u2603", "cp932")]:
    try:
        text.encode(name, "bogus-handler")
    except LookupError as error:
        print("bad-handler", name, type(error).__name__, ":", error)

# ---- ⑪ 第二族 `_codecs_cn`（gb2312／gbk；`cp936` 是别名）走同一委派 ----
import encodings.gb2312
import encodings.gbk

for name, text in [("gb2312", "中文测试"), ("gbk", "中文测试")]:
    encoded = text.encode(name)
    print("cn", name, encoded, encoded.decode(name), encoded.decode(name) == text)
    for errors in ("ignore", "replace"):
        print("cn", name, "encode", errors, "中\u2603文".encode(name, errors))
        print("cn", name, "decode", errors, b"\xd6\xd0\x81".decode(name, errors))
    try:
        "中\u2603文".encode(name)
    except UnicodeEncodeError as error:
        print("cn", name, "encode strict", str(error))
    try:
        b"\xd6\xd0\x81".decode(name)
    except UnicodeDecodeError as error:
        print("cn", name, "decode strict", str(error))
print("cn module-codec", encodings.gbk.codec.encode("中文"), encodings.gb2312.codec.decode(b"\xd6\xd0"))
print("cn alias", codecs.lookup("cp936").name, codecs.lookup("936").name, "中文".encode("cp936"))
try:
    print("cn bytes-value", b"\x80".decode("gbk"))
except UnicodeDecodeError as error:
    print("cn bytes-value", "decode-err", type(error).__name__, ":", error)
try:
    print("cn euro", "\u20ac".encode("gbk"))
except UnicodeEncodeError as error:
    print("cn euro", "encode-err", type(error).__name__, ":", error)

# ---- ⑫ 非文本编解码器（rot13／base64 一类）照参照报 LookupError（名字用调用方原文） ----
for expr_name, action in [
    ("rot13", "encode"),
    ("ROT13", "encode"),
    ("rot-13", "encode"),
    ("base64", "encode"),
    ("Hex", "decode"),
]:
    try:
        if action == "encode":
            "abc".encode(expr_name)
        else:
            b"abc".decode(expr_name)
    except LookupError as error:
        print("non-text", expr_name, action, type(error).__name__, ":", error)"##;

/// 参照 `python3` 3.14.4 对 `DELEGATION_SCRIPT` 的实际 stdout。
const DELEGATION_OUTPUT: &str = r##"cp932 b'\x93\xfa\x96{\x8c\xea\x83e\x83X\x83g' 日本語テスト True
euc_jp b'\xa4\xa2\xa4\xa4\xa4\xa6\xa4\xa8\xa4\xaa' あいうえお True
euc_jis_2004 b'\xb4\xc1\xbb\xfa\xa4\xc8\xa4\xab\xa4\xca' 漢字とかな True
euc_jisx0213 b'\xad\xa1\xad\xa2\xad\xa3\xad\xa4\xad\xa5' ①②③④⑤ True
shift_jis b'\xb1\xb2\xb3\xb4\xb5' ｱｲｳｴｵ True
shift_jis_2004 b'\x81`\x81|' 〜− True
shift_jisx0213 b'\x82\xf5\x82\xf6' か゚き゚ True
cp932 encode ignore b'\x82\xa0\x82\xa2'
cp932 encode replace b'\x82\xa0?\x82\xa2'
cp932 encode strict 'cp932' codec can't encode character '\u2603' in position 1: illegal multibyte sequence
euc_jp encode ignore b'\xa4\xa2\xa4\xa4'
euc_jp encode replace b'\xa4\xa2?\xa4\xa4'
euc_jp encode strict 'euc_jp' codec can't encode character '\u2603' in position 1: illegal multibyte sequence
euc_jis_2004 encode ignore b'\xa4\xa2\xa4\xa4'
euc_jis_2004 encode replace b'\xa4\xa2?\xa4\xa4'
euc_jis_2004 encode strict 'euc_jis_2004' codec can't encode character '\xa5' in position 1: illegal multibyte sequence
euc_jisx0213 encode ignore b''
euc_jisx0213 encode replace b'?'
euc_jisx0213 encode strict 'euc_jisx0213' codec can't encode character '\u203e' in position 0: illegal multibyte sequence
shift_jis encode ignore b'\x82\xa0\x82\xa2'
shift_jis encode replace b'\x82\xa0?\x82\xa2'
shift_jis encode strict 'shift_jis' codec can't encode character '\uff5e' in position 1: illegal multibyte sequence
shift_jis_2004 encode ignore b'\x82\xa0\x82\xa2'
shift_jis_2004 encode replace b'\x82\xa0?\x82\xa2'
shift_jis_2004 encode strict 'shift_jis_2004' codec can't encode character '\uff5e' in position 1: illegal multibyte sequence
shift_jisx0213 encode ignore b'\\'
shift_jisx0213 encode replace b'\\'
cp932 decode ignore あ
cp932 decode replace あ�
cp932 decode strict 'cp932' codec can't decode byte 0x81 in position 2: incomplete multibyte sequence
euc_jp decode ignore 
euc_jp decode replace ���
euc_jp decode strict 'euc_jp' codec can't decode byte 0x82 in position 0: illegal multibyte sequence
euc_jis_2004 decode ignore あ
euc_jis_2004 decode replace あ�
euc_jis_2004 decode strict 'euc_jis_2004' codec can't decode byte 0xff in position 2: incomplete multibyte sequence
euc_jisx0213 decode ignore あ
euc_jisx0213 decode replace あ�
euc_jisx0213 decode strict 'euc_jisx0213' codec can't decode byte 0x8f in position 2: incomplete multibyte sequence
shift_jis decode ignore あ
shift_jis decode replace あ�
shift_jis decode strict 'shift_jis' codec can't decode byte 0x80 in position 2: illegal multibyte sequence
shift_jis_2004 decode ignore あ
shift_jis_2004 decode replace あ�
shift_jis_2004 decode strict 'shift_jis_2004' codec can't decode byte 0xff in position 2: illegal multibyte sequence
shift_jisx0213 decode ignore あ
shift_jisx0213 decode replace あ�
shift_jisx0213 decode strict 'shift_jisx0213' codec can't decode byte 0x81 in position 2: incomplete multibyte sequence
builtin ascii encode ignore b'abc'
builtin ascii encode replace b'abc?'
builtin ascii encode strict 'ascii' codec can't encode character '\xe9' in position 3: ordinal not in range(128)
builtin latin-1 encode ignore b'caf\xe9'
builtin latin-1 encode replace b'caf\xe9?'
builtin latin-1 encode strict 'latin-1' codec can't encode character '\u20ac' in position 4: ordinal not in range(256)
builtin cp437 encode ignore b'\x80'
builtin cp437 encode replace b'\x80?'
builtin cp437 encode strict 'charmap' codec can't encode character '\u3042' in position 1: character maps to <undefined>
builtin utf-8 encode ignore b'\xe3\x81\x82\xe2\x98\x83'
builtin utf-8 encode replace b'\xe3\x81\x82\xe2\x98\x83'
builtin ascii decode ignore 'ab'
builtin ascii decode replace 'ab�'
builtin ascii decode strict 'ascii' codec can't decode byte 0xe9 in position 2: ordinal not in range(128)
builtin latin-1 decode ignore 'éÿ'
builtin latin-1 decode replace 'éÿ'
builtin cp437 decode ignore 'Ç\xa0'
builtin cp437 decode replace 'Ç\xa0'
builtin utf-8 decode ignore 'ab'
builtin utf-8 decode replace 'ab�'
builtin utf-8 decode strict 'utf-8' codec can't decode bytes in position 2-3: unexpected end of data
builtin utf-8 decode ignore 'ab('
builtin utf-8 decode replace 'ab�(�'
builtin utf-8 decode strict 'utf-8' codec can't decode byte 0xe3 in position 2: invalid continuation byte
builtin utf-8 decode ignore 'ab'
builtin utf-8 decode replace 'ab�'
builtin utf-8 decode strict 'utf-8' codec can't decode byte 0xff in position 2: invalid start byte
str() b'\xe9' ascii replace �
str() b'\xe9' ascii ignore 
str() b'\xe9' latin-1 strict é
str() b'\x82\xa0' cp932 strict あ
str() b'\x82\xa0\x81' cp932 replace あ�
str() b'\x8f' euc_jp ignore 
unknown-encode LookupError : unknown encoding: NoSuch
unknown-decode LookupError : unknown encoding: NoSuch
unknown-encode LookupError : unknown encoding: no such
unknown-decode LookupError : unknown encoding: no such
unknown-encode LookupError : unknown encoding: CP99999
unknown-decode LookupError : unknown encoding: CP99999
kwargs-string あ
kwargs-errors a
kwargs-positional-both あ
cp1252 b'\xe9\x80' é€
cp1252-replace b'?'
lazy-before True
lazy-name cp932
lazy-after True
lazy-encode (b'\x82\xa0', 1)
lazy-decode ('あ', 2)
lazy-alias cp932 cp932
bad-handler ascii LookupError : unknown error handler name 'bogus-handler'
bad-handler cp932 LookupError : unknown error handler name 'bogus-handler'
cn gb2312 b'\xd6\xd0\xce\xc4\xb2\xe2\xca\xd4' 中文测试 True
cn gb2312 encode ignore b'\xd6\xd0\xce\xc4'
cn gb2312 decode ignore 中
cn gb2312 encode replace b'\xd6\xd0?\xce\xc4'
cn gb2312 decode replace 中�
cn gb2312 encode strict 'gb2312' codec can't encode character '\u2603' in position 1: illegal multibyte sequence
cn gb2312 decode strict 'gb2312' codec can't decode byte 0x81 in position 2: incomplete multibyte sequence
cn gbk b'\xd6\xd0\xce\xc4\xb2\xe2\xca\xd4' 中文测试 True
cn gbk encode ignore b'\xd6\xd0\xce\xc4'
cn gbk decode ignore 中
cn gbk encode replace b'\xd6\xd0?\xce\xc4'
cn gbk decode replace 中�
cn gbk encode strict 'gbk' codec can't encode character '\u2603' in position 1: illegal multibyte sequence
cn gbk decode strict 'gbk' codec can't decode byte 0x81 in position 2: incomplete multibyte sequence
cn module-codec (b'\xd6\xd0\xce\xc4', 2) ('中', 2)
cn alias gbk gbk b'\xd6\xd0\xce\xc4'
cn bytes-value decode-err UnicodeDecodeError : 'gbk' codec can't decode byte 0x80 in position 0: incomplete multibyte sequence
cn euro encode-err UnicodeEncodeError : 'gbk' codec can't encode character '\u20ac' in position 0: illegal multibyte sequence
non-text rot13 encode LookupError : 'rot13' is not a text encoding; use codecs.encode() to handle arbitrary codecs
non-text ROT13 encode LookupError : 'ROT13' is not a text encoding; use codecs.encode() to handle arbitrary codecs
non-text rot-13 encode LookupError : 'rot-13' is not a text encoding; use codecs.encode() to handle arbitrary codecs
non-text base64 encode LookupError : 'base64' is not a text encoding; use codecs.encode() to handle arbitrary codecs
non-text Hex decode LookupError : 'Hex' is not a text encoding; use codecs.decode() to handle arbitrary codecs
"##;

#[test]
fn core_encoding_delegation_matches_the_reference_byte_for_byte() {
    let stdout = run("delegation", DELEGATION_SCRIPT);
    assert_eq!(
        stdout, DELEGATION_OUTPUT,
        "core 的编码委派（str.encode／bytes.decode／str()）或 `codecs.lookup` lazy 语义与参照不一致"
    );
}


// ---- round 12：`_codecs_tw`（big5／cp950）＋ `_codecs_kr`（euc_kr／cp949／johab） ------------

/// 同一脚本两侧跑（含 `euc_kr` 的 8 字节谚文分解与按族限定的 `getcodec`）。
const TW_KR_SCRIPT: &str = r##""""round 12 对拍：`_codecs_tw`（big5／cp950）＋`_codecs_kr`（euc_kr／cp949／johab）——
同引擎加族后，`import encodings.*`、`codecs.lookup(...)`、`str.encode`／`bytes.decode` 委派路径
与参照 `python3` 逐字节一致。"""

import codecs

import encodings.big5
import encodings.cp949
import encodings.cp950
import encodings.euc_kr
import encodings.johab

# ---- ① 5 个模块真 import＋module-level codec 编解码 ----
CASES = [
    ("big5", "中文測試"),
    ("cp950", "中文測試"),
    ("euc_kr", "한국어테스트"),
    ("cp949", "한국어테스트"),
    ("johab", "한국어테스트"),
]
for name, text in CASES:
    module = getattr(encodings, name)
    encoded = module.codec.encode(text)
    decoded = module.codec.decode(encoded[0])
    print("module", name, encoded, decoded, decoded == text)

# ---- ② `_codecs_tw`／`_codecs_kr` 的 getcodec 返回形态与往返 ----
import _codecs_kr
import _codecs_tw

for family, name, text in [
    (_codecs_tw, "big5", "中文測試"),
    (_codecs_tw, "cp950", "中文測試"),
    (_codecs_kr, "euc_kr", "한국어"),
    (_codecs_kr, "cp949", "한국어"),
    (_codecs_kr, "johab", "한국어"),
]:
    codec = family.getcodec(name)
    encoded, used = codec.encode(text)
    decoded, eaten = codec.decode(encoded)
    print("getcodec", name, encoded, used, decoded, eaten)
try:
    _codecs_tw.getcodec("gbk")
except LookupError as error:
    print("getcodec-unknown", type(error).__name__, ":", error)

# ---- ③ `str.encode`／`bytes.decode` 委派路径＋errors 三态 ----
ENCODE_ERRORS = [
    ("big5", "中\u2603文"),
    ("cp950", "中\u2603文"),
    ("euc_kr", "한\u2603국"),
    ("cp949", "한\u2603국"),
    ("johab", "한\u2603국"),
]
for name, text in ENCODE_ERRORS:
    for errors in ("ignore", "replace"):
        print("encode", name, errors, text.encode(name, errors))
    try:
        text.encode(name)
    except UnicodeEncodeError as error:
        print("encode", name, "strict", str(error))
    except Exception as error:  # noqa: BLE001
        print("encode", name, "strict-other", type(error).__name__, str(error))

DECODE_ERRORS = [
    ("big5", b"\xa4\xa4\x81"),
    ("cp950", b"\xa4\xa4\x81"),
    ("euc_kr", b"\xb0\xa1\x81"),
    ("cp949", b"\xb0\xa1\x81"),
    ("johab", b"\xb0\xa1\x81"),
]
for name, raw in DECODE_ERRORS:
    for errors in ("ignore", "replace"):
        print("decode", name, errors, raw.decode(name, errors))
    try:
        raw.decode(name)
    except UnicodeDecodeError as error:
        print("decode", name, "strict", str(error))
    except Exception as error:  # noqa: BLE001
        print("decode", name, "strict-other", type(error).__name__, str(error))

# 单字节非法/截断的分型（incomplete vs illegal）
for name in ("big5", "cp950", "euc_kr", "cp949", "johab"):
    for raw in (b"\x81", b"\x81\x20", b"\xff"):
        try:
            print("decode-shape", name, raw, raw.decode(name))
        except UnicodeDecodeError as error:
            print("decode-shape", name, raw, str(error))
        for errors in ("ignore", "replace"):
            print("decode-shape", name, raw, errors, repr(raw.decode(name, errors)))

# ---- ④ euc_kr 的 8 字节谚文分解（吸收态长前缀） ----
codec = _codecs_kr.getcodec("euc_kr")
jamo = "\uac02"
encoded = jamo.encode("euc_kr")
print("jamo", encoded, len(encoded), encoded.decode("euc_kr"))
valid = bytes((0xA4, 0xD4, 0xA4, 0xA1, 0xA4, 0xBF, 0xA4, 0xA2))
print("jamo-valid", valid.decode("euc_kr"), codec.decode(valid))
for raw in (valid, valid[:7], valid[:6], valid[:2], bytes((0xA4, 0xD4, 0, 0, 0, 0, 0, 0))):
    for errors in ("strict", "ignore", "replace"):
        try:
            print("jamo", raw.hex(), errors, repr(codec.decode(raw, errors)))
        except UnicodeDecodeError as error:
            print("jamo", raw.hex(), errors, str(error))
# 参照自身的不对称：U+3164 编成 a4d4，而 b'\\xa4\\xd4' 解出来是 incomplete
filler = "\u3164"
print("jamo-asym", filler.encode("euc_kr"))
try:
    bytes((0xA4, 0xD4)).decode("euc_kr")
except UnicodeDecodeError as error:
    print("jamo-asym", str(error))

# ---- ⑤ 注册表路径（lookup／别名）与 charmap 之外的委派 ----
for name in ("big5", "cp950", "euc_kr", "cp949", "johab"):
    info = codecs.lookup(name)
    text = "中文" if name in ("big5", "cp950") else "한국"
    encoded = info.encode(text)
    print("lookup", name, info.name, encoded, info.decode(encoded[0]))
print("alias", codecs.lookup("csbig5").name, codecs.lookup("uhc").name, codecs.lookup("ksc5601").name)

# ---- ⑥ 非文本编码器守卫（委派路径的边界仍然照参照） ----
for name, action in [("rot13", "encode"), ("hex", "decode")]:
    try:
        "abc".encode(name) if action == "encode" else b"abc".decode(name)
    except LookupError as error:
        print("non-text", name, type(error).__name__, ":", error)"##;

/// 参照 `python3` 3.14.4 对 `TW_KR_SCRIPT` 的实际 stdout。
const TW_KR_OUTPUT: &str = r##"module big5 (b'\xa4\xa4\xa4\xe5\xb4\xfa\xb8\xd5', 4) ('中文測試', 8) False
module cp950 (b'\xa4\xa4\xa4\xe5\xb4\xfa\xb8\xd5', 4) ('中文測試', 8) False
module euc_kr (b'\xc7\xd1\xb1\xb9\xbe\xee\xc5\xd7\xbd\xba\xc6\xae', 6) ('한국어테스트', 12) False
module cp949 (b'\xc7\xd1\xb1\xb9\xbe\xee\xc5\xd7\xbd\xba\xc6\xae', 6) ('한국어테스트', 12) False
module johab (b'\xd0e\x8a\x82\xb4\xe1\xc9A\xafa\xcba', 6) ('한국어테스트', 12) False
getcodec big5 b'\xa4\xa4\xa4\xe5\xb4\xfa\xb8\xd5' 4 中文測試 8
getcodec cp950 b'\xa4\xa4\xa4\xe5\xb4\xfa\xb8\xd5' 4 中文測試 8
getcodec euc_kr b'\xc7\xd1\xb1\xb9\xbe\xee' 3 한국어 6
getcodec cp949 b'\xc7\xd1\xb1\xb9\xbe\xee' 3 한국어 6
getcodec johab b'\xd0e\x8a\x82\xb4\xe1' 3 한국어 6
getcodec-unknown LookupError : no such codec is supported.
encode big5 ignore b'\xa4\xa4\xa4\xe5'
encode big5 replace b'\xa4\xa4?\xa4\xe5'
encode big5 strict 'big5' codec can't encode character '\u2603' in position 1: illegal multibyte sequence
encode cp950 ignore b'\xa4\xa4\xa4\xe5'
encode cp950 replace b'\xa4\xa4?\xa4\xe5'
encode cp950 strict 'cp950' codec can't encode character '\u2603' in position 1: illegal multibyte sequence
encode euc_kr ignore b'\xc7\xd1\xb1\xb9'
encode euc_kr replace b'\xc7\xd1?\xb1\xb9'
encode euc_kr strict 'euc_kr' codec can't encode character '\u2603' in position 1: illegal multibyte sequence
encode cp949 ignore b'\xc7\xd1\xb1\xb9'
encode cp949 replace b'\xc7\xd1?\xb1\xb9'
encode cp949 strict 'cp949' codec can't encode character '\u2603' in position 1: illegal multibyte sequence
encode johab ignore b'\xd0e\x8a\x82'
encode johab replace b'\xd0e?\x8a\x82'
encode johab strict 'johab' codec can't encode character '\u2603' in position 1: illegal multibyte sequence
decode big5 ignore 中
decode big5 replace 中�
decode big5 strict 'big5' codec can't decode byte 0x81 in position 2: incomplete multibyte sequence
decode cp950 ignore 中
decode cp950 replace 中�
decode cp950 strict 'cp950' codec can't decode byte 0x81 in position 2: incomplete multibyte sequence
decode euc_kr ignore 가
decode euc_kr replace 가�
decode euc_kr strict 'euc_kr' codec can't decode byte 0x81 in position 2: incomplete multibyte sequence
decode cp949 ignore 가
decode cp949 replace 가�
decode cp949 strict 'cp949' codec can't decode byte 0x81 in position 2: incomplete multibyte sequence
decode johab ignore 쌰
decode johab replace 쌰�
decode johab strict 'johab' codec can't decode byte 0x81 in position 2: incomplete multibyte sequence
decode-shape big5 b'\x81' 'big5' codec can't decode byte 0x81 in position 0: incomplete multibyte sequence
decode-shape big5 b'\x81' ignore ''
decode-shape big5 b'\x81' replace '�'
decode-shape big5 b'\x81 ' 'big5' codec can't decode byte 0x81 in position 0: illegal multibyte sequence
decode-shape big5 b'\x81 ' ignore ' '
decode-shape big5 b'\x81 ' replace '� '
decode-shape big5 b'\xff' 'big5' codec can't decode byte 0xff in position 0: incomplete multibyte sequence
decode-shape big5 b'\xff' ignore ''
decode-shape big5 b'\xff' replace '�'
decode-shape cp950 b'\x81' 'cp950' codec can't decode byte 0x81 in position 0: incomplete multibyte sequence
decode-shape cp950 b'\x81' ignore ''
decode-shape cp950 b'\x81' replace '�'
decode-shape cp950 b'\x81 ' 'cp950' codec can't decode byte 0x81 in position 0: illegal multibyte sequence
decode-shape cp950 b'\x81 ' ignore ' '
decode-shape cp950 b'\x81 ' replace '� '
decode-shape cp950 b'\xff' 'cp950' codec can't decode byte 0xff in position 0: incomplete multibyte sequence
decode-shape cp950 b'\xff' ignore ''
decode-shape cp950 b'\xff' replace '�'
decode-shape euc_kr b'\x81' 'euc_kr' codec can't decode byte 0x81 in position 0: incomplete multibyte sequence
decode-shape euc_kr b'\x81' ignore ''
decode-shape euc_kr b'\x81' replace '�'
decode-shape euc_kr b'\x81 ' 'euc_kr' codec can't decode byte 0x81 in position 0: illegal multibyte sequence
decode-shape euc_kr b'\x81 ' ignore ' '
decode-shape euc_kr b'\x81 ' replace '� '
decode-shape euc_kr b'\xff' 'euc_kr' codec can't decode byte 0xff in position 0: incomplete multibyte sequence
decode-shape euc_kr b'\xff' ignore ''
decode-shape euc_kr b'\xff' replace '�'
decode-shape cp949 b'\x81' 'cp949' codec can't decode byte 0x81 in position 0: incomplete multibyte sequence
decode-shape cp949 b'\x81' ignore ''
decode-shape cp949 b'\x81' replace '�'
decode-shape cp949 b'\x81 ' 'cp949' codec can't decode byte 0x81 in position 0: illegal multibyte sequence
decode-shape cp949 b'\x81 ' ignore ' '
decode-shape cp949 b'\x81 ' replace '� '
decode-shape cp949 b'\xff' 'cp949' codec can't decode byte 0xff in position 0: incomplete multibyte sequence
decode-shape cp949 b'\xff' ignore ''
decode-shape cp949 b'\xff' replace '�'
decode-shape johab b'\x81' 'johab' codec can't decode byte 0x81 in position 0: incomplete multibyte sequence
decode-shape johab b'\x81' ignore ''
decode-shape johab b'\x81' replace '�'
decode-shape johab b'\x81 ' 'johab' codec can't decode byte 0x81 in position 0: illegal multibyte sequence
decode-shape johab b'\x81 ' ignore ' '
decode-shape johab b'\x81 ' replace '� '
decode-shape johab b'\xff' 'johab' codec can't decode byte 0xff in position 0: incomplete multibyte sequence
decode-shape johab b'\xff' ignore ''
decode-shape johab b'\xff' replace '�'
jamo b'\xa4\xd4\xa4\xa1\xa4\xbf\xa4\xa2' 8 갂
jamo-valid 갂 ('갂', 8)
jamo a4d4a4a1a4bfa4a2 strict ('갂', 8)
jamo a4d4a4a1a4bfa4a2 ignore ('갂', 8)
jamo a4d4a4a1a4bfa4a2 replace ('갂', 8)
jamo a4d4a4a1a4bfa4 strict 'euc_kr' codec can't decode bytes in position 0-6: incomplete multibyte sequence
jamo a4d4a4a1a4bfa4 ignore ('', 7)
jamo a4d4a4a1a4bfa4 replace ('�', 7)
jamo a4d4a4a1a4bf strict 'euc_kr' codec can't decode bytes in position 0-5: incomplete multibyte sequence
jamo a4d4a4a1a4bf ignore ('', 6)
jamo a4d4a4a1a4bf replace ('�', 6)
jamo a4d4 strict 'euc_kr' codec can't decode bytes in position 0-1: incomplete multibyte sequence
jamo a4d4 ignore ('', 2)
jamo a4d4 replace ('�', 2)
jamo a4d4000000000000 strict 'euc_kr' codec can't decode byte 0xa4 in position 0: illegal multibyte sequence
jamo a4d4000000000000 ignore ('\x00\x00\x00\x00\x00\x00', 8)
jamo a4d4000000000000 replace ('��\x00\x00\x00\x00\x00\x00', 8)
jamo-asym b'\xa4\xd4'
jamo-asym 'euc_kr' codec can't decode bytes in position 0-1: incomplete multibyte sequence
lookup big5 big5 (b'\xa4\xa4\xa4\xe5', 2) ('中文', 4)
lookup cp950 cp950 (b'\xa4\xa4\xa4\xe5', 2) ('中文', 4)
lookup euc_kr euc_kr (b'\xc7\xd1\xb1\xb9', 2) ('한국', 4)
lookup cp949 cp949 (b'\xc7\xd1\xb1\xb9', 2) ('한국', 4)
lookup johab johab (b'\xd0e\x8a\x82', 2) ('한국', 4)
alias big5 cp949 euc_kr
non-text rot13 LookupError : 'rot13' is not a text encoding; use codecs.encode() to handle arbitrary codecs
non-text hex LookupError : 'hex' is not a text encoding; use codecs.decode() to handle arbitrary codecs
"##;

#[test]
fn tw_and_kr_families_match_the_reference_byte_for_byte() {
    let stdout = run("tw-kr", TW_KR_SCRIPT);
    assert_eq!(
        stdout, TW_KR_OUTPUT,
        "`_codecs_tw`／`_codecs_kr` 一族的输出与参照不一致（逐字节对拍）"
    );
}
