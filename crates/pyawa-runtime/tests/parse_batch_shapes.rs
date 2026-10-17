//! **P3-31 解析器缺口批的运行期抽例**（第 405 轮）——两侧逐行一致。
//!
//! 与 `crates/pyawa-core/tests/parse_batch_shapes.rs`（编译期逐条指令）配对：这一条走**运行期**，
//! 把每格的**可观察语义**（值、求值次序、tab 缩进、注解不求值、`*` 先于关键字求值）钉住 ✓。
//! 期望输出由本机 `python3`（3.14.4）跑**同一段脚本**取得（不是手写 ✓）。

fn run(name: &str, script: &str) -> String {
    let root = std::env::temp_dir().join(format!("pyawa-parse-batch-{}-{name}", std::process::id()));
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
const EXPECTED: &str = "nested 1 2 3 4\nnested_fn (5, 6, 7, 8, 9, 10, [11, 12])\nnested_mixed 13 14 15\nwith_tuple 16 17 18 19 20\nwith_fn (21, 22)\ncomp_bracket [1, 2, 3]\ncomp_paren [5, 7]\ncomp_nested [19]\ncomp_bare [3, 7]\ngenexpr_nested (23,)\nsubscript_call {'a': {'b': 23}}\nsubscript_order ['value', 'container', 'key']\nleading_dot 0.05 0.5 50.0 0.05\nleading_dot_var 0.5\nstar_tuple (1, 25, 26)\nstar_tuple_first (25, 26, 27)\nstar_tuple_middle (1, 25, 26, 28, 29)\nstar_unpack 1 [30, 31] 32\ninline_else 3 4\ninline_then 5 6\nannotated 33 [34]\ntabs 35 36\nstar_after_kw ((38,), {'obj': 37})\nstar_after_kw2 ((40, 41), {'a': 39, 'b': 42})\nstar_kw_order ((44, 46), {'a': 43, 'b': 45}) ['star1', 'star2', 'kw1', 'kw2']\n";

/// 十个缺口形状的脚本：② with 元组目标 ③ 推导式模式目标 ④ 调用基底下标 …… ⑩ `*` 后置。
const SCRIPT: &str = "# P3-31 解析器缺口批的运行期抽例：每段都打印可观察结果（参照值由本机 python3 3.14 导出）。\nlog = []\n\n\ndef t(name, value):\n    log.append(name)\n    return value\n\n\n# ① 嵌套元组目标\n(a, b), (c, d) = (1, 2), (3, 4)\nprint(\"nested\", a, b, c, d)\n\n\ndef nested_fn():\n    (e, f), (g, h) = (5, 6), (7, 8)\n    i, (j, *k) = 9, (10, 11, 12)\n    return e, f, g, h, i, j, k\n\n\nprint(\"nested_fn\", nested_fn())\n[p, [q, r]] = [13, [14, 15]]\nprint(\"nested_mixed\", p, q, r)\n\n# ② with … as 元组目标\nclass Ctx:\n    def __init__(self, value):\n        self.value = value\n\n    def __enter__(self):\n        return self.value\n\n    def __exit__(self, *args):\n        return False\n\n\nwith Ctx((16, 17)) as (s, u), Ctx((18, (19, 20))) as (v, (w, x)):\n    print(\"with_tuple\", s, u, v, w, x)\n\n\ndef with_fn():\n    with Ctx((21, 22)) as (y, z):\n        return y, z\n\n\nprint(\"with_fn\", with_fn())\n\n# ③ 推导式模式目标\ndata = [(1,), (2,), (3,)]\nprint(\"comp_bracket\", [off for [off] in data])\nprint(\"comp_paren\", [v for (k2, v) in [(4, 5), (6, 7)]])\nprint(\"comp_nested\", [x2 + y2 for a2, (x2, y2) in [(8, (9, 10))]])\nprint(\"comp_bare\", [m + n for m, n in [(1, 2), (3, 4)]])\nprint(\"genexpr_nested\", tuple(a3 + b3 for a3, (b3, c3) in [(11, (12, 13))]))\n\n# ④ 下标赋值的目标基底是调用\nd2 = {}\nd2.setdefault(\"a\", {})[\"b\"] = 23\nprint(\"subscript_call\", d2)\norder = []\n\n\ndef container():\n    order.append(\"container\")\n    return {}\n\n\ndef key():\n    order.append(\"key\")\n    return \"k\"\n\n\ndef value():\n    order.append(\"value\")\n    return 24\n\n\ncontainer()[key()] = value()\nprint(\"subscript_order\", order)\n\n# ⑤ 前导点浮点\nprint(\"leading_dot\", 0.05, 0.5, .5e2, min(1, .05))\ny = .5\nprint(\"leading_dot_var\", y)\n\n# ⑥ 元组显示里的 *\nargs = [25, 26]\nprint(\"star_tuple\", (1, *args))\nprint(\"star_tuple_first\", (*args, 27))\nprint(\"star_tuple_middle\", (1, *args, 28, *[29]))\nrest = [30, 31]\np2, *q2, r2 = 1, *rest, 32\nprint(\"star_unpack\", p2, q2, r2)\n\n# ⑦ 行内 if 分号表 ＋ 次行 else\nflag = False\nif flag: a4 = 1; b4 = 2\nelse: a4 = 3; b4 = 4\nprint(\"inline_else\", a4, b4)\nif True: a4 = 5; b4 = 6\nelse: a4 = 7; b4 = 8\nprint(\"inline_then\", a4, b4)\n\n# ⑧ 带注解赋值的非名字目标（注解**不求值**；若求值这里的名字会 NameError）\nclass Holder:\n    def __init__(self):\n        self.x: never_defined_name = 33\n        self.items = [0]\n        self.items[0]: never_defined_name = 34\n\n\nholder = Holder()\nprint(\"annotated\", holder.x, holder.items)\n\n# ⑨ 制表符缩进（代码行 tab、注释行 tab）\ndef tab_code():\n\treturn 35\n\n\ndef tab_comment():\n    n = 36\n\t# 这行是以 tab 开头的注释（不算缩进）\n    return n\n\n\nprint(\"tabs\", tab_code(), tab_comment())\n\n# ⑩ `*` 后置关键字实参\ndef collect(*a, **k):\n    return a, k\n\n\nprint(\"star_after_kw\", collect(obj=37, *[38]))\nprint(\"star_after_kw2\", collect(a=39, *[40, 41], b=42))\norder2 = []\n\n\ndef tv(name, value):\n    order2.append(name)\n    return value\n\n\nprint(\"star_kw_order\", collect(a=tv(\"kw1\", 43), *[tv(\"star1\", 44)], b=tv(\"kw2\", 45), *[tv(\"star2\", 46)]), order2)\n";

#[test]
fn parser_batch_semantics_agree_with_the_reference() {
    let stdout = run("semantics", SCRIPT);
    assert_eq!(stdout, EXPECTED, "{stdout}");
}
