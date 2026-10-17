//! `.pyac` 容器格式（`IM-18`…`IM-21`）的可测性质。
//!
//! 这是**自有**格式（`IM-18`：与 CPython 的 `.pyc` 无关且不兼容）⇒ 期望值不可能来自参照实现，
//! 只能来自规格 ＋ 本层选定的编码。所以这里除了性质断言，还有一条**黄金字节**用例把布局钉住
//! （改布局就得改它——那正是我们要的提醒）。

use std::path::Path;

use pyawa_core::compile::CheckTier;
use pyawa_runtime::pyac::{
    self, PyacError, Staleness, ARTIFACT_DIRECTORY, HEADER_LEN, MAGIC, MODE_EXTENDED, MODE_PURE,
};

/// 测试默认用**浅层**档位（`TS-31` 的编译期参数；深层档位另有专门用例）。
const SHALLOW: u8 = CheckTier::Shallow.as_byte();
/// 深层档位（`TS-31` 的可选档位）。
const DEEP: u8 = CheckTier::Deep.as_byte();

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
    let bytes = pyac::encode(MODE_PURE, 0, SHALLOW, source, &code, 1);
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
    let first = pyac::encode(MODE_PURE, 0, SHALLOW, source, b"code", 1);
    let second = pyac::encode(MODE_PURE, 0, SHALLOW, source, b"code", 1);
    assert_eq!(first, second, "同样四样输入 ⇒ 同样的字节");
    // 四样里换哪一样都会变
    assert_ne!(first, pyac::encode(MODE_EXTENDED, 0, SHALLOW, source, b"code", 1));
    assert_ne!(first, pyac::encode(MODE_PURE, 1, SHALLOW, source, b"code", 1));
    assert_ne!(first, pyac::encode(MODE_PURE, 0, SHALLOW, b"x = 2\n", b"code", 1));
    assert_ne!(first, pyac::encode(MODE_PURE, 0, SHALLOW, source, b"code", 2));
}

#[test]
fn decode_rejects_bad_or_mismatched_products() {
    let source = b"x = 1\n";
    let bytes = pyac::encode(MODE_PURE, 0, SHALLOW, source, b"code", 1);
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
fn the_check_tier_is_a_compilation_input() {
    // `TS-31`：档位是**编译期参数**，且是 `IM-21` 五要素之一（`IM-20` ②：档位不同即陈旧）
    let source = b"x = 1\n";
    let shallow_artifact = pyac::encode(MODE_PURE, 0, SHALLOW, source, b"code", 1);
    let deep_artifact = pyac::encode(MODE_PURE, 0, DEEP, source, b"code", 1);
    assert_ne!(
        shallow_artifact, deep_artifact,
        "档位不同 ⇒ 产物必须不同（IM-21）"
    );
    assert_eq!(
        pyac::decode(&shallow_artifact, 1).expect("解得开").tier,
        SHALLOW
    );
    assert_eq!(
        pyac::decode(&deep_artifact, 1).expect("解得开").tier,
        DEEP
    );

    let root = std::env::temp_dir().join(format!("pyawa-pyac-tier-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&root);
    std::fs::create_dir_all(&root).expect("建临时目录");
    let path = pyac::write(&root, "t.py", 1, MODE_PURE, 0, SHALLOW, source, b"code")
        .expect("写产物");
    assert_eq!(
        pyac::staleness(&path, MODE_PURE, 0, SHALLOW, source, 1),
        Staleness::Fresh,
        "档位相同、源码相同 ⇒ 不陈旧"
    );
    assert_eq!(
        pyac::staleness(&path, MODE_PURE, 0, DEEP, source, 1),
        Staleness::Stale,
        "IM-20 ②：档位不同即陈旧"
    );
    let _ = std::fs::remove_dir_all(&root);
}

#[test]
fn staleness_is_two_steps_and_never_uses_mtime() {
    let root = std::env::temp_dir().join(format!("pyawa-pyac-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&root);
    std::fs::create_dir_all(&root).expect("建临时目录");
    let source = b"x = 1\n";

    // ① 名字不匹配（这里是**别的指令集版本**留下的产物）⇒ 精确查找找不到它
    let other = pyac::write(&root, "foo.py", 2, MODE_PURE, 0, SHALLOW, source, b"old").expect("写别的版本");
    assert!(other.is_file());
    assert_eq!(
        pyac::find_artifact(&root, "foo.py", 1),
        None,
        "版本不匹配的产物必须被忽略（连读都不读）"
    );

    // ② 名字匹配 ⇒ 比指纹
    let path = pyac::write(&root, "foo.py", 1, MODE_PURE, 0, SHALLOW, source, b"code").expect("写产物");
    assert_eq!(
        pyac::staleness(&path, MODE_PURE, 0, SHALLOW, source, 1),
        Staleness::Fresh,
        "同样源码 ⇒ 不陈旧"
    );
    // 长度相同而哈希不同 ⇒ 陈旧（`IM-20` 特意点出的情形）
    let same_length = b"x = 2\n";
    assert_eq!(same_length.len(), source.len());
    assert_eq!(
        pyac::staleness(&path, MODE_PURE, 0, SHALLOW, same_length, 1),
        Staleness::Stale,
        "长度相同、哈希不同 ⇒ 陈旧"
    );
    // 长度不同 ⇒ 陈旧
    assert_eq!(
        pyac::staleness(&path, MODE_PURE, 0, SHALLOW, b"x = 1\n\n", 1),
        Staleness::Stale
    );
    // 模式／优化级对不上也算陈旧
    assert_eq!(
        pyac::staleness(&path, MODE_EXTENDED, 0, SHALLOW, source, 1),
        Staleness::Stale
    );
    assert_eq!(pyac::staleness(&path, MODE_PURE, 1, SHALLOW, source, 1), Staleness::Stale);

    let _ = std::fs::remove_dir_all(&root);
}

#[test]
fn the_header_layout_is_locked_by_golden_bytes() {
    // 自有格式 ⇒ 期望值只能来自本层选定的编码；这一条把布局钉住。
    // `TS-31` 之后头部多一字节「检查档位」（排在优化级之后，`IM-19`）⇒ 偏移整体后移 1。
    let bytes = pyac::encode(MODE_EXTENDED, 2, DEEP, b"hi", b"\x01\x02", 1);
    assert_eq!(
        bytes,
        vec![
            b'P', b'Y', b'A', b'W', b'A', b'C', 0, 0, // magic（IM-19 的字段顺序见 pyac.rs 的文档表）
            1, 0, 0, 0, // 指令集版本（小端 u32）
            1, // 模式：扩展
            2, // 优化级
            1, // 检查档位：深层（TS-31；0 ＝ 浅层、1 ＝ 深层）
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
            39, 0, 0, 0, // 代码段偏移（头部 39 字节：TS-31 之后多一字节档位）
            2, 0, 0, 0, // 代码段长度
            1, 2, // 代码
        ]
    );
}

#[test]
fn compiled_units_survive_the_code_section() {
    // `IM-28`／`IM-31`：代码段装的就是 `P1-10` 的编译产物。编译 → 序列化 → 反序列化 → 必须
    // **逐字段相同**（含嵌套 code object、关键字名元组、位置表）。
    let sources = [
        "x = 1",
        "x = 200 + 100",
        "def f(a, b):\n    return a + b\n",
        "if a:\n    x = 1\n",
        "for i in s:\n    x = i\n",
        "x = f(a=1, b=2)",
        "x = f(*s, **d)",
        // **任意精度字面量**（第 285 轮）：新标签 13（十进制串）也要往返 ✓
        "x = 0xFFFFFFFFFFFFFFFF",
        "x = 10000000000000000000000",
    ];
    for source in sources {
        let unit = pyawa_core::compile::compile(
            source,
            "<t>",
            pyawa_core::compile::Mode::PurePython,
                CheckTier::Shallow,
                0,
        )
        .expect("编得过");
        let bytes = pyac::encode_unit(&unit);
        let back = pyac::decode_unit(&bytes).expect("解得开");
        assert_eq!(back, unit, "{source:?} 的产物应当在往返后完全相同");
    }
}

#[test]
fn the_code_section_is_deterministic() {
    // `IM-21`：产物只由「源码 ＋ 模式 ＋ 优化级 ＋ 指令集版本」决定 ⇒
    //   ① 同一份产物编两次给同一串字节；
    //   ② 反序列化后再编一次也给同一串字节（不为解析路径留痕）。
    let source = "def f(a):\n    return a + 1\nx = f(2)\n";
    let unit =
        pyawa_core::compile::compile(source, "<t>", pyawa_core::compile::Mode::PurePython, CheckTier::Shallow, 0)
            .expect("编得过");
    let first = pyac::encode_unit(&unit);
    let second = pyac::encode_unit(&unit);
    assert_eq!(first, second, "同一份产物必须给同一串字节");
    let back = pyac::decode_unit(&first).expect("解得开");
    assert_eq!(pyac::encode_unit(&back), first, "往返后再编也必须一致");

    // 整份 `.pyac` 同理（头部 ＋ 代码段都由那四样决定）
    let whole_a = pyac::encode(MODE_PURE, 0, SHALLOW, source.as_bytes(), &first, 1);
    let whole_b = pyac::encode(MODE_PURE, 0, SHALLOW, source.as_bytes(), &second, 1);
    assert_eq!(whole_a, whole_b, "整份产物必须一致");
    let product = pyac::decode(&whole_a, 1).expect("解得开");
    assert_eq!(
        pyac::decode_unit(&product.code).expect("解得开"),
        unit,
        "从整份产物里取回的产物应当相同"
    );
}

#[test]
fn a_broken_code_section_is_reported_not_guessed() {
    // 半截字节／空字节都不许 panic，一律 `BadCodeSection`
    assert!(matches!(
        pyac::decode_unit(&[]),
        Err(pyac::PyacError::BadCodeSection)
    ));
    let unit = pyawa_core::compile::compile("x = 1", "<t>", pyawa_core::compile::Mode::PurePython, CheckTier::Shallow, 0)
        .expect("编得过");
    let mut bytes = pyac::encode_unit(&unit);
    bytes.truncate(bytes.len() - 3);
    assert!(matches!(
        pyac::decode_unit(&bytes),
        Err(pyac::PyacError::BadCodeSection)
    ));
    let mut extra = pyac::encode_unit(&unit);
    extra.push(0);
    assert!(matches!(
        pyac::decode_unit(&extra),
        Err(pyac::PyacError::BadCodeSection)
    ));
}

#[test]
fn a_compiled_unit_lands_as_a_real_artifact() {
    // 把 `P1-10` 的产物经代码段落成磁盘上的真产物，再按 `IM-18`／`IM-20` 的两步找回：
    //   ① `find_artifact` 按**精确文件名**（含模式与指令集版本）找；
    //   ② `staleness` 比源码指纹。
    let root = std::env::temp_dir().join(format!("pyawa-pyac-unit-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&root);
    std::fs::create_dir_all(&root).expect("建临时目录");
    let source = "def f(a):\n    return a + 1\nx = f(2)\n";
    let unit =
        pyawa_core::compile::compile(source, "<t>", pyawa_core::compile::Mode::PurePython, CheckTier::Shallow, 0)
            .expect("编得过");
    let code = pyac::encode_unit(&unit);
    let path = pyac::write(
        &root,
        "m.py",
        1,
        MODE_PURE,
        0,
        SHALLOW,
        source.as_bytes(),
        &code,
    )
    .expect("写产物");
    assert_eq!(
        pyac::find_artifact(&root, "m.py", 1),
        Some(path.clone()),
        "① 按精确名字找得到"
    );
    assert_eq!(
        pyac::staleness(&path, MODE_PURE, 0, SHALLOW, source.as_bytes(), 1),
        Staleness::Fresh,
        "② 同一份源码 ⇒ 不陈旧"
    );
    let bytes = std::fs::read(&path).expect("读得回");
    let product = pyac::decode(&bytes, 1).expect("解得开");
    assert_eq!(
        pyac::decode_unit(&product.code).expect("解得开"),
        unit,
        "取回的产物与编译出来的一模一样"
    );
    let _ = std::fs::remove_dir_all(&root);
}

/// **容器 ＋ 陈旧判定在**运行路径**上真的被用**（第 405 轮；`IM-18`…`IM-21`）：
/// ① 第一次跑 ⇒ 写出产物；② 再跑（新鲜）⇒ **复用**（产物字节不变）；③ 改源码 ⇒ **陈旧 ⇒ 刷新** ✓。
#[test]
fn cli_writes_reuses_and_refreshes_the_pyac_artifact() {
    let root = std::env::temp_dir().join(format!("pyawa-pyac-cli-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&root);
    std::fs::create_dir_all(&root).expect("建临时目录");
    let script = root.join("t.py");
    std::fs::write(&script, "answer = 1\n").expect("写脚本");
    let binary = env!("CARGO_BIN_EXE_pyawa");
    let artifact = pyawa_runtime::pyac::artifact_path(
        &root,
        "t.py",
        pyawa_core::opcode_metadata::INSTRUCTION_SET_VERSION,
    );

    let first = std::process::Command::new(binary)
        .arg(&script)
        .output()
        .expect("跑第一次");
    assert!(first.status.success(), "第一次应当成功：{:?}", first);
    assert!(artifact.exists(), "第一次运行应当写出产物：{}", artifact.display());
    let after_first = std::fs::read(&artifact).expect("读产物");

    let second = std::process::Command::new(binary)
        .arg(&script)
        .output()
        .expect("跑第二次");
    assert!(second.status.success(), "第二次应当成功：{:?}", second);
    assert_eq!(
        after_first,
        std::fs::read(&artifact).expect("读产物"),
        "源码没变 ⇒ 产物应当**照原样复用**（不重写）"
    );

    std::fs::write(&script, "answer = 2\n").expect("改脚本");
    let third = std::process::Command::new(binary)
        .arg(&script)
        .output()
        .expect("跑第三次");
    assert!(third.status.success(), "第三次应当成功：{:?}", third);
    assert_ne!(
        after_first,
        std::fs::read(&artifact).expect("读产物"),
        "源码变了 ⇒ 陈旧判定必须**刷新**产物（`IM-20` ②）"
    );

    let _ = std::fs::remove_dir_all(&root);
}
