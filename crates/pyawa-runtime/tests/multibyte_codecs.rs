//! **CJK 多字节编解码族（`_codecs_jp`）的对拍护栏**（task-6）：契约见 `docs/SPEC-c-modules.md` §5.2.18。
//!
//! 期望值**全部取自参照 `python3` 3.14.4 对同一脚本的实际输出**（不拿我们自己的输出当标准 ✓）：
//! 生成方式＝把下面的 `REFERENCE_SCRIPT` 原样交给参照解释器，取其 stdout 钉成 `REFERENCE_OUTPUT`。
//!
//! 覆盖面（**真 import ＋真编解码**）：
//!
//! - `Lib/encodings/` 的 **7** 个多字节模块逐字同步后真 import：`cp932`／`euc_jp`／
//!   `euc_jis_2004`／`euc_jisx0213`／`shift_jis`／`shift_jis_2004`／`shift_jisx0213`；
//! - `_codecs_jp.getcodec(name)` 的 `(bytes, 消耗码位数)`／`(str, 消耗字节数)` 返回形态；
//! - `errors=strict`／`ignore`／`replace` 三条 ＋ 错误文案（`illegal`／`incomplete` 分型、
//!   `bytes in position 0-1` 复数形态、`\xNN`／`\uNNNN`／`\UNNNNNNNN` 字符字面量）；
//! - JIS X 0213 的两码位组合字符（解码出 2 个码位；编码两码位合成一条序列）；
//! - `codecs.lookup`／别名／增量编解码器（含跨 chunk 的组合字符留尾）／流 Reader／Writer。
//!
//! 表数据与引擎的**生成路径**见 `crates/pyawa-stdlib/src/multibyte_module.rs` 模块头
//! （`python3 tools/gen_codec_tables.py --family jp --emit`）。

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

/// 参照与 Pyawa 用同一份脚本（只有两侧都有的公开面：`_codecs_jp`／`encodings`／`codecs`／`io`）。
const REFERENCE_SCRIPT: &str = r##""""CJK 多字节编解码族（`_codecs_jp`）的**参照对拍探针**：同一份脚本在参照 `python3` 与 Pyawa 上跑，
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

/// 参照 `python3` 3.14.4 对 `REFERENCE_SCRIPT` 的实际 stdout（**逐字节**对拍）。
const REFERENCE_OUTPUT: &str = r##"cp932 (b'\x93\xfa\x96{\x8c\xea\x83e\x83X\x83g', 6) ('日本語テスト', 12)
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
    let stdout = run("family", REFERENCE_SCRIPT);
    assert_eq!(
        stdout, REFERENCE_OUTPUT,
        "`_codecs_jp` 一族的输出与参照不一致（逐字节对拍；先看上面 diff 的期望/实际）"
    );
}
