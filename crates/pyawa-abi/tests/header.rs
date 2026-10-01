//! **`AB-46`**：`pa.h` 必须能被 **C 与 C++** 同时包含。
//!
//! 这里做的是**真编译**（`cc -c` 与 `c++ -c`），不是文本扫描：`extern "C"` 守卫写漏了，
//! C++ 那边照样能编过（链接期才出问题），所以"能包含"这条只能靠两边都编一次来钉。
//! 版本宏在 C 下也必须可用（`AB-45`／`AB-46`），故两边各加一条编译期断言。

use std::path::{Path, PathBuf};
use std::process::Command;

/// 读目标文件里的**未定义符号**（`nm`）。C++ 侧若缺 `extern "C"`，`pa_create` 会被 mangle
/// （变成 `_Z9pa_create...` 之类）——那正是本检查要抓的东西（`AB-46`）。
fn undefined_symbols(object: &Path) -> Vec<String> {
    let output = Command::new("nm")
        .arg("--undefined-only")
        .arg("--format=just-symbols")
        .arg(object)
        .output()
        .expect("nm 起不来（AB-46 的符号检查需要 binutils）");
    assert!(output.status.success(), "nm 失败：{:?}", output);
    String::from_utf8_lossy(&output.stdout)
        .lines()
        .map(|line| line.trim().to_owned())
        .filter(|line| !line.is_empty())
        .collect()
}

/// 头文件所在目录（`AB-45`：单一 `pa.h`，禁止分层头）。
fn include_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("include")
}

/// 造一个临时源文件并返回它的路径。
fn write_source(name: &str, text: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("pyawa-pa-header-{}", std::process::id()));
    std::fs::create_dir_all(&dir).expect("建临时目录");
    let path = dir.join(name);
    std::fs::write(&path, text).expect("写临时源文件");
    path
}

/// 编译一个源文件（只编译，不链接——`AB-46` 管的是"能否包含"）。
fn compile(compiler: &str, language: &str, source: &Path, object: &Path) -> Result<(), String> {
    let output = Command::new(compiler)
        .arg("-c")
        .arg("-x")
        .arg(language)
        .arg("-I")
        .arg(include_dir())
        .arg("-o")
        .arg(object)
        .arg(source)
        .output()
        .map_err(|error| format!("{compiler} 起不来（AB-46 的检查需要 C 与 C++ 编译器）：{error}"))?;
    if output.status.success() {
        return Ok(());
    }
    Err(format!(
        "{compiler} 编译失败：\n{}",
        String::from_utf8_lossy(&output.stderr)
    ))
}

#[test]
fn the_header_is_includable_from_c() {
    let source = write_source(
        "probe.c",
        r#"#include "pa.h"

/* AB-46：版本宏在 C 下也可用（不依赖 C++ 特性） */
_Static_assert(PA_ABI_MAJOR >= 1, "PA_ABI_MAJOR 在 C 下必须可用");
_Static_assert(PA_ABI_SIZE > 0, "PA_ABI_SIZE 在 C 下必须可用");
_Static_assert(sizeof(pa_host) == PA_ABI_SIZE, "AB-43：尺寸宏就是宿主结构体的字节数");

int main(void) {
    return 0;
}
"#,
    );
    let object = source.with_extension("o");
    compile("cc", "c", &source, &object).expect("C 侧必须能包含 pa.h");
}

#[test]
fn the_header_is_includable_from_cxx_and_keeps_c_linkage() {
    let source = write_source(
        "probe.cc",
        r#"#include "pa.h"

/* AB-46：C++ 侧同样要能包含，且版本宏同样可用 */
static_assert(PA_ABI_MAJOR >= 1, "PA_ABI_MAJOR 在 C++ 下必须可用");
static_assert(sizeof(pa_host) == PA_ABI_SIZE, "AB-43：尺寸宏就是宿主结构体的字节数");

/* 取几个导出函数的地址 ⇒ 目标文件里留下**未定义符号**，供 `nm` 检查有没有被 mangle */
extern "C" int pa_create(const pa_host *, pa_state **);
static void *keep[] = {
    reinterpret_cast<void *>(&pa_create),
    reinterpret_cast<void *>(&pa_destroy),
    reinterpret_cast<void *>(&pa_errmsg),
};

int main() {
    return keep[0] == nullptr;
}
"#,
    );
    let object = source.with_extension("o");
    compile("c++", "c++", &source, &object).expect("C++ 侧必须能包含 pa.h");
    let symbols = undefined_symbols(&object);
    assert!(
        symbols.iter().any(|symbol| symbol == "pa_create"),
        "C++ 侧编译后 `pa_create` 应当是**未 mangle** 的 C 符号（AB-46 的 extern \"C\" 守卫）；\
         实际未定义符号：{symbols:?}"
    );
    assert!(
        symbols.iter().all(|symbol| !symbol.starts_with("_Z")),
        "不该出现 mangle 过的名字（说明守卫漏了）：{symbols:?}"
    );
}
