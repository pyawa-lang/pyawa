//! `sys.getfilesystemencoding()`／`sys.getfilesystemencodeerrors()`／`sys.getdefaultencoding()`
//! 的真值（task-61 收尾：修 `getfilesystemencodeerrors` 误报编码名的真 bug）。
//!
//! 期望值来自本机 `python3` 3.14.4 实测：编码名 `utf-8`、**错误处理器名** `surrogateescape`。
//! 旧实现把三个名字共用一个原生、靠"绑定 self 的函数名"区分 ⇒ 模块函数调用时拿不到名字 ⇒
//! `getfilesystemencodeerrors()` 误报 `'utf-8'` ⇒ `os.fsdecode` 报 `LookupError`。

use core::ptr::NonNull;

use pyawa_core::{Instance, NativeFn};
use pyawa_stdlib::sys_module;

fn value(instance: &Instance, namespace: NonNull<pyawa_core::Header>, name: &str) -> String {
    use pyawa_core::BuiltinFunctionObject;
    let function = instance
        .dict_get(namespace, name)
        .unwrap_or_else(|| panic!("`sys.{name}` 必须存在"));
    // SAFETY: 这些名字放的都是原生可调用对象。
    let handler: NativeFn = unsafe { (*function.as_ptr().cast::<BuiltinFunctionObject>()).function() };
    // SAFETY: 无实参调用（handler 只借用实参表）。
    let result = unsafe { handler(&instance, None, &[], &[]) }.expect("这些 sys 查询口不应失败");
    instance
        .text_of(result)
        .unwrap_or_else(|| panic!("`sys.{name}()` 必须返回 str"))
        .to_owned()
}

#[test]
fn filesystem_encoding_names_match_the_reference() {
    let instance = Instance::new();
    let namespace = sys_module::build(&instance);
    assert_eq!(value(&instance, namespace, "getfilesystemencoding"), "utf-8");
    assert_eq!(value(&instance, namespace, "getdefaultencoding"), "utf-8");
    // **错误处理器名**（不是编码名）：参照 3.14.4 ⇒ `surrogateescape`。
    assert_eq!(value(&instance, namespace, "getfilesystemencodeerrors"), "surrogateescape");
}
