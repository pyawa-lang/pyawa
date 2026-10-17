//! **`sys.meta_path` 已就位**（护栏 ✓，守第 293 轮的 `install_meta_path`）：
//! finder 由调 `module` 类型对象造出 ✓、挂在 `install` 末尾 ✓，`find_spec` **只定位** ✓（`IM-31` ✓）。
//! **本文件只断言"finder 就位且行为是只定位"** ✓ —— **不**声称 `import` 已经走它 ✗
//! （第 237 轮实测"装表 ≠ 被用"✗：① a 仍未判成 ✓）。

fn run(name: &str, script: &str) -> String {
    let root = std::env::temp_dir().join(format!("pyawa-metapath-{}-{name}", std::process::id()));
    let _ = std::fs::remove_dir_all(&root);
    std::fs::create_dir_all(&root).expect("建临时目录");
    let path = root.join("probe.py");
    std::fs::write(&path, script).expect("写脚本");
    let output = std::process::Command::new(env!("CARGO_BIN_EXE_pyawa"))
        .arg(&path)
        .output()
        .expect("跑 CLI");
    let _ = std::fs::remove_dir_all(&root);
    assert!(output.status.success(), "应当成功：{output:?}");
    String::from_utf8_lossy(&output.stdout).into_owned()
}

#[test]
fn the_finder_is_installed_and_only_locates() {
    let stdout = run(
        "locate",
        "import sys\n\
         print('count:', len(sys.meta_path))\n\
         finder = sys.meta_path[0]\n\
         print('has find_spec:', hasattr(finder, 'find_spec'))\n\
         print('locatable:', finder.find_spec('os') is not None)\n\
         print('missing:', finder.find_spec('zzz_no_such_module_pyawa') is None)\n",
    );
    // 本层设计：表里就是我们那一个 finder ✓。
    assert!(stdout.contains("count: 1"), "{stdout}");
    assert!(stdout.contains("has find_spec: True"), "{stdout}");
    // **只定位** ✓：可定位的给 spec ✓，定位不到的给 None ✓（不是异常 ✓）。
    assert!(stdout.contains("locatable: True"), "{stdout}");
    assert!(stdout.contains("missing: True"), "{stdout}");
}
