//! **`pyexpat` 形状面**（`P3-37` 剩余，round 38）。
//!
//! 落地（`CM-6`：名字齐、真做不了的调用时如实报未实现）：
//! * 模块级：`EXPAT_VERSION`／`native_encoding`／`version_info`／`features`／三个
//!   `XML_PARAM_ENTITY_PARSING_*`／`ErrorString(code)`（44 条码表，码序照参照实测）；
//! * **子模块** `pyexpat.errors`（44 条名字→消息）／`pyexpat.model`（10 条常量）——
//!   `xml.parsers.expat` 的 `sys.modules[...] = model/errors` 重导出要它们是**模块对象**；
//! * `ExpatError`／`error`（同对象；本层按 `OSError` 家族落名，参照是 `Exception` 子类，如实记）；
//! * `XMLParserType`（类名 `xmlparser`，照参照）＋ `ParserCreate(...)`：实例可建、处理器属性可读写、
//!   `SetParamEntityParsing` 返回 1；**`Parse`／`ParseFile` 如实 `NotImplementedError`** ✗
//!   （整块 expat C 扩展：XML/DTD/实体/命名空间/错误码语义，属独立评估）。
//!
//! 期望值由本机 `python3`（3.14.4）跑**同一段脚本**取得（`MS-17`），逐字节一致；
//! `Parse` 的未实现面是 pyawa 侧钉住（参照侧会真解析）。

fn run(name: &str, script: &str) -> String {
    let root = std::env::temp_dir().join(format!("pyawa-pyexpat-{}-{name}", std::process::id()));
    let _ = std::fs::remove_dir_all(&root);
    std::fs::create_dir_all(&root).expect("建临时目录");
    let path = root.join("probe.py");
    std::fs::write(&path, script).expect("写脚本");
    let output = std::process::Command::new(env!("CARGO_BIN_EXE_pyawa"))
        .arg(&path)
        .current_dir(&root)
        .output()
        .expect("跑 CLI");
    let _ = std::fs::remove_dir_all(&root);
    assert!(
        output.status.success(),
        "不许中止（退出状态 {:?}）\n--- stdout ---\n{}\n--- stderr ---\n{}",
        output.status,
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    String::from_utf8_lossy(&output.stdout).into_owned()
}

/// 形状面（与参照逐字节一致）。
const PYEXPAT_SHAPE_SCRIPT: &str = r#"import pyexpat
import xml.parsers.expat

print("version", pyexpat.EXPAT_VERSION, pyexpat.version_info, pyexpat.native_encoding)
print("errors-submodule", xml.parsers.expat.errors.XML_ERROR_ABORTED)
print("model-submodule", xml.parsers.expat.model.XML_CTYPE_ANY)
print("ErrorString", pyexpat.ErrorString(1), pyexpat.ErrorString(3), pyexpat.ErrorString(35), pyexpat.ErrorString(0))
print("error-alias", pyexpat.error is pyexpat.ExpatError, issubclass(pyexpat.ExpatError, Exception))
p = pyexpat.ParserCreate()
print("parser", type(p).__name__, p.StartElementHandler, p.ordered_attributes, p.buffer_size)
p.StartElementHandler = "handler"
print("handler-set", p.StartElementHandler)
print("set-param", p.SetParamEntityParsing(pyexpat.XML_PARAM_ENTITY_PARSING_NEVER))
print("features", len(pyexpat.features), pyexpat.features[0])
try:
    pyexpat.ParserCreate(namespace_separator="::")
except ValueError as error:
    print("bad-sep", str(error))
try:
    pyexpat.ParserCreate(namespace_separator=1)
except TypeError as error:
    print("bad-sep-type", str(error))"#;

/// 参照输出（逐行）。
const PYEXPAT_SHAPE_EXPECTED: &str = "version expat_2.7.4 (2, 7, 4) UTF-8\nerrors-submodule parsing aborted\nmodel-submodule 2\nErrorString out of memory no element found parsing aborted None\nerror-alias True True\nparser xmlparser None False 8192\nhandler-set handler\nset-param 1\nfeatures 10 ('sizeof(XML_Char)', 1)\nbad-sep namespace_separator must be at most one character, omitted, or None\nbad-sep-type ParserCreate() argument 'namespace_separator' must be str or None, not int\n";

/// `Parse`／`ParseFile` 的**如实未实现**（pyawa 侧钉住；消息含"移植"）。
const PYEXPAT_PARSE_SCRIPT: &str = r#"import pyexpat

parser = pyexpat.ParserCreate()
for name in ("Parse", "ParseFile"):
    try:
        if name == "Parse":
            getattr(parser, name)("<a/>", True)
        else:
            getattr(parser, name)(None)
    except NotImplementedError as error:
        print(name, "未实现" if "移植" in str(error) else "?")
"#;

const PYEXPAT_PARSE_EXPECTED: &str = "Parse 未实现\nParseFile 未实现\n";

#[test]
fn pyexpat_shape_matches_the_reference() {
    assert_eq!(run("shape", PYEXPAT_SHAPE_SCRIPT), PYEXPAT_SHAPE_EXPECTED);
    assert_eq!(run("parse", PYEXPAT_PARSE_SCRIPT), PYEXPAT_PARSE_EXPECTED);
}
