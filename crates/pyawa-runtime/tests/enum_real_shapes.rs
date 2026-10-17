//! **enum 换真货战役 B1–B3 的回归抽例**（第 410 轮，task-21 ✓）——两侧逐行一致。
//!
//! 覆盖：
//! * **B1／G1**：`if` 块里的 `for`／`while` 带 `break`（退出路径先前被换成**作用域 return** ✗；
//!   模块级同形会让后半段静默不执行 ✗）＋ 外层循环夹一层 `if` 的嵌套形（先前会跳过块后代码／死循环 ✗）；
//! * **B2／G2**：内建类型**自己的** `__new__`（`int/str/bytes/list/dict/set` 一族；
//!   `str.__new__(子类, "abc")` 造子类型实例、错参文案照参照 ✓）＋ `str` 子类**按值比较** ＋
//!   `set` 子类 `repr` 用子类名 ✓（`enum.py` 的 `StrEnum.__new__`／`HTTPMethod.__new__` 压在这格）；
//! * **B3／G5**：`__set_name__` 里 `type(owner)` 是**元类** ✓、元类方法可见 ✓；
//!   附带钉**两遍走**（`__set_name__` 能看到**后面才定义**的类属性 ✓ —— 真 `enum.py` 的
//!   `_proto_member.__set_name__` 要写 `_member_names_`，那个键在它之后才进字典 ✓）；
//! * 顺带钉掉 B 批暴露的**类对象专用通道**：类对象 `repr`／`str` 按**元类 MRO**（含元类 `__repr__` 覆写）✓、
//!   迭代类对象走元类 `__iter__` ✓、`function.__get__` 描述符 ✓（`enum.py` 的 `_is_descriptor` 靠它 ✓）。
//!
//! 期望输出由本机 `python3`（3.14.4）跑**同一段脚本**取得（不是手写 ✓）；
//! `repr` 里的类地址／`<__main__.…>` 之类按参照原样（`__main__` 两侧一致 ✓）。

fn run(name: &str, script: &str) -> String {
    let root = std::env::temp_dir().join(format!("pyawa-enum-real-{}-{name}", std::process::id()));
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
const EXPECTED: &str = "after if 1\nafter if None\nif_break 1 None\nafter while w\nafter while None\nwhile_break w None\nnested_break [3, 1, 2]\nmodule_loop 1\nmodule_after\nnew_is False False False\nnew_str S abc\nnew_int I 5\nnew_bytes B b'xy'\nnew_list L []\nnew_empty '' 0\nconstruct S I 6\nstr_eq True True\nset_repr St() St({1})\nwrong_owner -> TypeError str.__new__(I): I is not a subtype of str\nbad_cls -> TypeError str.__new__(X): X is not a type object (str)\nkw -> TypeError str() got an unexpected keyword argument 'bad'\nsetname M ('helper', 1) C\nlater_attr a 7\nclass_repr <class '__main__.C'> <class '__main__.C'> <class '__main__.Holder'>\nmeta_repr <meta 'MR'>\nmeta_iter ['MI', 3]\nfunc_get True method 1\n";

const SCRIPT: &str = "def c(flag):\n    out = None\n    if flag:\n        for i in (1, 2):\n            out = i\n            break\n    print(\"after if\", out)\n    return out\n\n\nprint(\"if_break\", c(True), c(False))\n\n\ndef w(flag):\n    out = None\n    if flag:\n        while True:\n            out = \"w\"\n            break\n    print(\"after while\", out)\n    return out\n\n\nprint(\"while_break\", w(True), w(False))\n\n\ndef nested():\n    out = []\n    for outer in (1, 2):\n        if outer == 1:\n            for i in (3, 4):\n                out.append(i)\n                break\n        out.append(outer)\n    return out\n\n\nprint(\"nested_break\", nested())\n\nif True:\n    for i in (1, 2):\n        print(\"module_loop\", i)\n        break\nprint(\"module_after\")\n\n\nclass S(str):\n    pass\n\n\nclass I(int):\n    pass\n\n\nclass B(bytes):\n    pass\n\n\nclass St(set):\n    pass\n\n\nclass L(list):\n    pass\n\n\nprint(\"new_is\", str.__new__ is object.__new__, int.__new__ is object.__new__,\n      str.__new__ is int.__new__)\nprint(\"new_str\", type(str.__new__(S, \"abc\")).__name__, str.__new__(S, \"abc\"))\nprint(\"new_int\", type(int.__new__(I, 5)).__name__, int.__new__(I, 5))\nprint(\"new_bytes\", type(bytes.__new__(B, b\"xy\")).__name__, bytes.__new__(B, b\"xy\"))\nprint(\"new_list\", type(list.__new__(L)).__name__, list.__new__(L))\nprint(\"new_empty\", repr(str.__new__(S)), int.__new__(I))\nprint(\"construct\", type(S(\"abc\")).__name__, type(I(5)).__name__, I(5) + 1)\nprint(\"str_eq\", S(\"abc\") == \"abc\", \"abc\" == S(\"abc\"))\nprint(\"set_repr\", repr(St()), repr(St([1])))\n\n\ndef show(label, fn):\n    try:\n        print(label, \"->\", repr(fn()))\n    except Exception as error:\n        print(label, \"->\", type(error).__name__, error)\n\n\nshow(\"wrong_owner\", lambda: str.__new__(I, \"x\"))\nshow(\"bad_cls\", lambda: str.__new__(\"notaclass\", \"x\"))\nshow(\"kw\", lambda: str.__new__(S, \"x\", bad=1))\n\n\nclass M(type):\n    def helper(cls, value):\n        return (\"helper\", value)\n\n\nclass Descriptor:\n    def __set_name__(self, owner, name):\n        print(\"setname\", type(owner).__name__, owner.helper(1), owner.__name__)\n\n\nclass C(metaclass=M):\n    d = Descriptor()\n\n\nclass Track:\n    def __set_name__(self, owner, name):\n        print(\"later_attr\", name, owner.LATER)\n\n\nclass Holder:\n    a = Track()\n    LATER = 7\n\n\nprint(\"class_repr\", repr(C), str(C), repr(Holder))\n\n\nclass MetaRepr(type):\n    def __repr__(cls):\n        return \"<meta %r>\" % cls.__name__\n\n\nclass MR(metaclass=MetaRepr):\n    pass\n\n\nprint(\"meta_repr\", repr(MR))\n\n\nclass MetaIter(type):\n    def __iter__(cls):\n        return iter((cls.__name__, 3))\n\n\nclass MI(metaclass=MetaIter):\n    pass\n\n\nprint(\"meta_iter\", list(MI))\n\n\ndef f(x):\n    return x\n\n\nprint(\"func_get\", f.__get__(None, object) is f, type(f.__get__(1, object)).__name__,\n      f.__get__(1, object)())\n";

#[test]
fn enum_real_campaign_shapes_agree_with_the_reference() {
    let stdout = run("shapes", SCRIPT);
    assert_eq!(stdout, EXPECTED, "{stdout}");
}
