//! **`http.server` 的 SIGSEGV 回归**（第 412 轮，task-24 ✓）——两侧逐行一致、且不许崩。
//!
//! 根因（gdb 钉出来的）：`itertools.chain` 的 stdlib 原生把**借用的实参**直接交给
//! `Instance::new_list`（那个接口是**接手新引用**的 `CM-4` 契约 ✗）⇒ list 释放时把帧手里
//! 那份也减掉 ⇒ 元素（`range` 对象）被提前释放 ⇒ 迭代时 `iterator_type_for` 读到
//! **已释放对象**的类型指针（毒化）⇒ SIGSEGV（`http.server` 的 `BaseHTTPRequestHandler`
//! 类体里 `str.maketrans({c: … for c in itertools.chain(range(32), range(127, 160))})`
//! 正是触发形状 ✓）。修法：chain 先把每个实参 `retain` 再进 list ✓。
//!
//! 抽例：chain 的四种最小形状（list／dict 推导／多参／嵌套）＋ `import http.server` 后
//! 类体语义（`BaseHTTPRequestHandler.responses`／版本属性／类对象都在）——两侧逐行一致 ✓。
//! 期望由本机 `python3`（3.14.4，`-S`＋`PYTHONPATH=本仓 Lib` 同源）跑同一段脚本取得 ✓。

fn run(name: &str, script: &str) -> String {
    let root = std::env::temp_dir().join(format!("pyawa-http-server-{}-{name}", std::process::id()));
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
        "不许中止（退出状态 {:?}，SIGSEGV＝139）\n--- stdout ---\n{}\n--- stderr ---\n{}",
        output.status,
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    String::from_utf8_lossy(&output.stdout).into_owned()
}

/// 参照（`python3` 3.14 实测 ✓，逐行）。
const EXPECTED: &str = "chain_list [1, 2, 3]\nchain_range [0, 1, 2]\nchain_comp {0: 0, 1: 2, 2: 4, 3: 6, 4: 8}\nchain_nested [[1], [2], [3]]\ngen_filter ['aa', 'cc']\ngen_filter_call 3\nresponses ('Not Found', 'Nothing matches the given URI') ('OK', 'Request fulfilled, document follows')\nattrs HTTP/1.0 text/html;charset=utf-8 BaseHTTP/0.6\nserver <class 'http.server.HTTPServer'> <class 'http.server.ThreadingHTTPServer'>\nmodule_ok ['BaseHTTPRequestHandler', 'HTTPServer', 'SimpleHTTPRequestHandler', 'ThreadingHTTPServer']\n";

const SCRIPT: &str = "import itertools\nimport http.server\n\nprint(\"chain_list\", list(itertools.chain([1, 2], [3])))\nprint(\"chain_range\", list(itertools.chain(range(3))))\nprint(\"chain_comp\", {c: c * 2 for c in itertools.chain(range(3), range(3, 5))})\nprint(\"chain_nested\", list(itertools.chain([[1], [2]], [[3]])))\n# 生成器表达式**条件里带调用**的形状（`STORE_FAST_LOAD_FAST` 融合次序真 bug 的回归 ✓）\nprint(\"gen_filter\", list(x for x in [\"aa\", \"bbb\", \"cc\"] if len(x) == 2))\nprint(\"gen_filter_call\", sum(len(x) for x in [\"a\", \"bb\"]))\n\nhandler = http.server.BaseHTTPRequestHandler\nprint(\"responses\", handler.responses[404], handler.responses[200])\nprint(\"attrs\", handler.protocol_version, handler.error_content_type, handler.server_version)\nprint(\"server\", http.server.HTTPServer, http.server.ThreadingHTTPServer)\nprint(\"module_ok\", sorted(name for name in (\"BaseHTTPRequestHandler\", \"HTTPServer\",\n      \"ThreadingHTTPServer\", \"SimpleHTTPRequestHandler\") if hasattr(http.server, name)))\n";

#[test]
fn itertools_chain_and_http_server_agree_with_the_reference() {
    let stdout = run("shapes", SCRIPT);
    assert_eq!(stdout, EXPECTED, "{stdout}");
}
