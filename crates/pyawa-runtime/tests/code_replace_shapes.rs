//! **`CodeObject.replace`**（第 409 轮，task-17 ✓）——两侧逐行一致。
//!
//! 语义照参照 `code.replace()`：**仅关键字**、缺省＝原值、返回**新** code object（原对象不动）；
//! 支持本层已存的字段（`co_name`/`co_qualname`/`co_filename`/`co_firstlineno`/`co_argcount`/
//! `co_posonlyargcount`/`co_kwonlyargcount`/`co_nlocals`/`co_stacksize`/`co_flags`/`co_code`/
//! `co_consts`/`co_names`/`co_varnames`/`co_freevars`/`co_cellvars`/`co_exceptiontable`）。
//!
//! **如实登记的偏差** ✗（不进两侧对拍）：`co_linetable`／`co_weakref` 等本层**没存**的字段
//! 给不了（报 unexpected keyword ✓，参照是支持的）；`co_firstlineno` 的**原值**本层的编译器
//! 固定为 1（既有偏差，抽例只钉**替换后**的值）；`types.coroutine` 那条路还差
//! `function.__code__` 的**写**属性（下一条真 bug，见汇报）。
//!
//! 抽例只钉**属性值**（code object 的 `repr`／身份不钉 ✓）；期望输出由本机 `python3`
//! （3.14.4）跑**同一段脚本**取得（不是手写 ✓）。

fn run(name: &str, script: &str) -> String {
    let root = std::env::temp_dir().join(format!("pyawa-code-replace-{}-{name}", std::process::id()));
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

/// 参照（`python3` 3.14 实测 ✓，逐行）。
const EXPECTED: &str = "copy True f 2 0 2 3\nflags 259 3\nname zz f\nqualname Q f\nfilename F.py\nfirstline 42\nstack 9 1\nconsts (1, 2) (None,)\nnames ('a', 'b') ()\nvarnames ('x', 'y', 'z')\nvarnames_keep ('a', 'b') 2\ncode b'\\x00\\x00' True\nexc b''\nargcount 1\nposonly 1\nkwonly 1\nfreevars ()\ncellvars ('c',)\nuntouched f 3 ('a', 'b') 2 True\npositional -> TypeError replace() takes no positional arguments\nbad_kw -> TypeError replace() got an unexpected keyword argument 'bad'\nflags_type -> TypeError 'str' object cannot be interpreted as an integer\nname_type -> TypeError replace() argument 'co_name' must be str, not int\nconsts_list -> TypeError replace() argument 'co_consts' must be tuple, not list\nargcount_type -> TypeError 'str' object cannot be interpreted as an integer\nneg_argcount -> ValueError co_argcount must be a positive integer\nneg_stacksize -> ValueError co_stacksize must be a positive integer\nnlocals_mismatch -> ValueError code: co_nlocals != len(co_varnames)\nvarnames_short -> ValueError code: co_varnames is too small\ncode_str -> TypeError replace() argument 'co_code' must be bytes, not str\nnames_nonstr -> SystemError non-string found in code slot\n";

const SCRIPT: &str = "_CODES = {}\n\n\ndef f(a, b=1):\n    return a + b\n\n\nco = f.__code__\nn = co.replace()\nprint(\"copy\", n is not co, n.co_name, n.co_argcount, n.co_kwonlyargcount, n.co_nlocals, n.co_flags)\nprint(\"flags\", co.replace(co_flags=co.co_flags | 0x100).co_flags, co.co_flags)\nprint(\"name\", co.replace(co_name=\"zz\").co_name, co.co_name)\nprint(\"qualname\", co.replace(co_qualname=\"Q\").co_qualname, co.co_qualname)\nprint(\"filename\", co.replace(co_filename=\"F.py\").co_filename)\nprint(\"firstline\", co.replace(co_firstlineno=42).co_firstlineno)\nprint(\"stack\", co.replace(co_stacksize=9).co_stacksize, co.replace(co_stacksize=0).co_stacksize)\nprint(\"consts\", co.replace(co_consts=(1, 2)).co_consts, co.co_consts)\nprint(\"names\", co.replace(co_names=(\"a\", \"b\")).co_names, co.co_names)\nprint(\"varnames\", co.replace(co_varnames=(\"x\", \"y\", \"z\"), co_nlocals=3).co_varnames)\nprint(\"varnames_keep\", co.co_varnames, co.co_nlocals)\nprint(\"code\", co.replace(co_code=b\"\\x00\\x00\").co_code, co.co_code != b\"\")\nprint(\"exc\", co.replace(co_exceptiontable=b\"\").co_exceptiontable)\nprint(\"argcount\", co.replace(co_argcount=1).co_argcount)\nprint(\"posonly\", co.replace(co_posonlyargcount=1, co_argcount=2).co_posonlyargcount)\nprint(\"kwonly\", co.replace(co_kwonlyargcount=1, co_argcount=1).co_kwonlyargcount)\nprint(\"freevars\", co.replace(co_freevars=()).co_freevars)\nprint(\"cellvars\", co.replace(co_cellvars=(\"c\",)).co_cellvars)\nprint(\"untouched\", co.co_name, co.co_flags, co.co_varnames, co.co_argcount, n.co_name == co.co_name)\n\n\ndef show(label, fn):\n    try:\n        print(label, \"->\", fn())\n    except Exception as error:\n        print(label, \"->\", type(error).__name__, error)\n\n\nshow(\"positional\", lambda: co.replace(1))\nshow(\"bad_kw\", lambda: co.replace(bad=1))\nshow(\"flags_type\", lambda: co.replace(co_flags=\"x\"))\nshow(\"name_type\", lambda: co.replace(co_name=5))\nshow(\"consts_list\", lambda: co.replace(co_consts=[1]))\nshow(\"argcount_type\", lambda: co.replace(co_argcount=\"x\"))\nshow(\"neg_argcount\", lambda: co.replace(co_argcount=-1))\nshow(\"neg_stacksize\", lambda: co.replace(co_stacksize=-1))\nshow(\"nlocals_mismatch\", lambda: co.replace(co_nlocals=5))\nshow(\"varnames_short\", lambda: co.replace(co_varnames=(\"only\",)))\nshow(\"code_str\", lambda: co.replace(co_code=\"x\"))\nshow(\"names_nonstr\", lambda: co.replace(co_names=(1,)))\n";

#[test]
fn code_object_replace_semantics_agree_with_the_reference() {
    let stdout = run("replace", SCRIPT);
    assert_eq!(stdout, EXPECTED, "{stdout}");
}
