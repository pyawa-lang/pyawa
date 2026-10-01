//! `.pyac` 容器格式（`IM-18`…`IM-21`）的可测性质。
//!
//! 这是**自有**格式（`IM-18`：与 CPython 的 `.pyc` 无关且不兼容）⇒ 期望值不可能来自参照实现，
//! 只能来自规格 ＋ 本层选定的编码。所以这里除了性质断言，还有一条**黄金字节**用例把布局钉住
//! （改布局就得改它——那正是我们要的提醒）。

use std::path::Path;

use pyawa_runtime::pyac::{
    self, PyacError, Staleness, ARTIFACT_DIRECTORY, HEADER_LEN, MAGIC, MODE_EXTENDED, MODE_PURE,
};

#[test]
fn artifact_path_follows_the_spec() {
    // `IM-18`：`<pkgdir>/__pyawa__/<源文件名>.<指令集版本>.pyac`
    let path = pyac::artifact_path(Path::new("/pkg"), "foo.py", 1);
    assert_eq!(
        path,
        Path::new("/pkg").join(ARTIFACT_DIRECTORY).join("foo.py.1.pyac")
    );
    // 模式体现在源文件名里 ⇒ 换模式必然换名（`IM-18` 末尾那句）
    assert_ne!(
        pyac::file_name("foo.py", 1),
        pyac::file_name("foo.pyawa", 1),
        "模式不同 ⇒ 产物名不同"
    );
    // 指令集版本体现在名字里 ⇒ 升版本必然换名（`BC-29`／`IM-20` ①）
    assert_ne!(pyac::file_name("foo.py", 1), pyac::file_name("foo.py", 2));
}

#[test]
fn encode_decode_round_trips() {
    let source = b"x = 1\n";
    let code = vec![0xAA, 0xBB, 0xCC];
    let bytes = pyac::encode(MODE_PURE, 0, source, &code, 1);
    assert_eq!(bytes.len(), HEADER_LEN + code.len());
    assert_eq!(&bytes[..8], &MAGIC);
    let product = pyac::decode(&bytes, 1).expect("应当解得出来");
    assert_eq!(product.instruction_set_version, 1);
    assert_eq!(product.mode, MODE_PURE);
    assert_eq!(product.optimization, 0);
    assert_eq!(product.source_length, source.len() as u64);
    assert_eq!(product.source_fingerprint, pyac::fingerprint(source));
    assert_eq!(product.code, code);
}

#[test]
fn the_product_is_a_pure_function_of_its_four_inputs() {
    // `IM-21`：只由「源码 ＋ 模式 ＋ 优化级 ＋ 指令集版本」决定
    let source = b"x = 1\n";
    let first = pyac::encode(MODE_PURE, 0, source, b"code", 1);
    let second = pyac::encode(MODE_PURE, 0, source, b"code", 1);
    assert_eq!(first, second, "同样四样输入 ⇒ 同样的字节");
    // 四样里换哪一样都会变
    assert_ne!(first, pyac::encode(MODE_EXTENDED, 0, source, b"code", 1));
    assert_ne!(first, pyac::encode(MODE_PURE, 1, source, b"code", 1));
    assert_ne!(first, pyac::encode(MODE_PURE, 0, b"x = 2\n", b"code", 1));
    assert_ne!(first, pyac::encode(MODE_PURE, 0, source, b"code", 2));
}

#[test]
fn decode_rejects_bad_or_mismatched_products() {
    let source = b"x = 1\n";
    let bytes = pyac::encode(MODE_PURE, 0, source, b"code", 1);
    // 版本不符 ⇒ `BC-29` 判陈旧、**不**加载
    assert_eq!(
        pyac::decode(&bytes, 2),
        Err(PyacError::VersionMismatch {
            found: 1,
            expected: 2
        })
    );
    // 太短
    assert_eq!(pyac::decode(&bytes[..4], 1), Err(PyacError::Truncated));
    // magic 不对
    let mut broken = bytes.clone();
    broken[0] = b'X';
    assert_eq!(pyac::decode(&broken, 1), Err(PyacError::BadMagic));
    // 代码段越界
    let mut broken = bytes.clone();
    broken[34..38].copy_from_slice(&u32::MAX.to_le_bytes());
    assert_eq!(pyac::decode(&broken, 1), Err(PyacError::BadCodeSection));
}

#[test]
fn staleness_is_two_steps_and_never_uses_mtime() {
    let root = std::env::temp_dir().join(format!("pyawa-pyac-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&root);
    std::fs::create_dir_all(&root).expect("建临时目录");
    let source = b"x = 1\n";

    // ① 名字不匹配（这里是**别的指令集版本**留下的产物）⇒ 精确查找找不到它
    let other = pyac::write(&root, "foo.py", 2, MODE_PURE, 0, source, b"old").expect("写别的版本");
    assert!(other.is_file());
    assert_eq!(
        pyac::find_artifact(&root, "foo.py", 1),
        None,
        "版本不匹配的产物必须被忽略（连读都不读）"
    );

    // ② 名字匹配 ⇒ 比指纹
    let path = pyac::write(&root, "foo.py", 1, MODE_PURE, 0, source, b"code").expect("写产物");
    assert_eq!(
        pyac::staleness(&path, MODE_PURE, 0, source, 1),
        Staleness::Fresh,
        "同样源码 ⇒ 不陈旧"
    );
    // 长度相同而哈希不同 ⇒ 陈旧（`IM-20` 特意点出的情形）
    let same_length = b"x = 2\n";
    assert_eq!(same_length.len(), source.len());
    assert_eq!(
        pyac::staleness(&path, MODE_PURE, 0, same_length, 1),
        Staleness::Stale,
        "长度相同、哈希不同 ⇒ 陈旧"
    );
    // 长度不同 ⇒ 陈旧
    assert_eq!(
        pyac::staleness(&path, MODE_PURE, 0, b"x = 1\n\n", 1),
        Staleness::Stale
    );
    // 模式／优化级对不上也算陈旧
    assert_eq!(
        pyac::staleness(&path, MODE_EXTENDED, 0, source, 1),
        Staleness::Stale
    );
    assert_eq!(pyac::staleness(&path, MODE_PURE, 1, source, 1), Staleness::Stale);

    let _ = std::fs::remove_dir_all(&root);
}

#[test]
fn the_header_layout_is_locked_by_golden_bytes() {
    // 自有格式 ⇒ 期望值只能来自本层选定的编码；这一条把布局钉住
    let bytes = pyac::encode(MODE_EXTENDED, 2, b"hi", b"\x01\x02", 1);
    assert_eq!(
        bytes,
        vec![
            b'P', b'Y', b'A', b'W', b'A', b'C', 0, 0, // magic
            1, 0, 0, 0, // 指令集版本（小端 u32）
            1, // 模式：扩展
            2, // 优化级
            2, 0, 0, 0, 0, 0, 0, 0, // 源码长度（小端 u64）
            // 源码指纹（'h' = 0x68、'i' = 0x69 的 FNV-1a，小端）
            (pyac::fingerprint(b"hi") & 0xFF) as u8,
            ((pyac::fingerprint(b"hi") >> 8) & 0xFF) as u8,
            ((pyac::fingerprint(b"hi") >> 16) & 0xFF) as u8,
            ((pyac::fingerprint(b"hi") >> 24) & 0xFF) as u8,
            ((pyac::fingerprint(b"hi") >> 32) & 0xFF) as u8,
            ((pyac::fingerprint(b"hi") >> 40) & 0xFF) as u8,
            ((pyac::fingerprint(b"hi") >> 48) & 0xFF) as u8,
            ((pyac::fingerprint(b"hi") >> 56) & 0xFF) as u8,
            38, 0, 0, 0, // 代码段偏移
            2, 0, 0, 0, // 代码段长度
            1, 2, // 代码
        ]
    );
}
