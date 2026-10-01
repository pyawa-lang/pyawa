//! `pyawa-abi` 已落地部分的性质（判据见 `docs/SPEC-c-abi.md` §13）：
//! 状态码取值（`AB-20`）、版本策略（`AB-39`…`AB-45`）、**有界读取**（`AB-43`）、
//! panic 边界（`AB-3`／`CX-11`，`T-AB-2` 的一半）。

use core::mem::size_of;

use pyawa_abi::status::*;
use pyawa_abi::*;

#[test]
fn status_codes_match_the_spec_table() {
    // AB-20：取值稳定，与 SPEC-c-abi.md §15.2 逐项对应
    assert_eq!(PA_OK, 0);
    assert_eq!(PA_ERR_RUNTIME, 1);
    assert_eq!(PA_ERR_SYNTAX, 2);
    assert_eq!(PA_ERR_MEMORY, 3);
    assert_eq!(PA_ERR_INTERRUPT, 4);
    assert_eq!(PA_ERR_NOTIMPLEMENTED, 5);
    assert_eq!(PA_ERR_INVALID, 6);
    assert_eq!(PA_ERR_ABI, 7);
    assert_eq!(PA_ERR_RESERVED_FIRST, 8);
    assert_eq!(PA_ERR_RESERVED_LAST, 31);
    assert!(PA_ERR_RESERVED_FIRST > PA_ERR_ABI, "AB-20：新增必须落在预留区");
}

#[test]
fn version_macros_agree_with_the_exported_functions() {
    assert_eq!(pa_abi_version(), PA_ABI_VERSION);
    assert_eq!(pa_abi_size(), PA_ABI_SIZE);
    assert_eq!(
        PA_ABI_SIZE,
        size_of::<pa_host>(),
        "AB-45：PA_ABI_SIZE 就是宿主结构体字节数"
    );
    assert_eq!(
        PA_ABI_VERSION,
        (PA_ABI_MAJOR << 16) | PA_ABI_MINOR,
        "版本编码：主版本在高 16 位"
    );
    // 版本字符串是 NUL 结尾的静态串
    // SAFETY: pa_version 返回静态内存的指针。
    let text = unsafe { core::ffi::CStr::from_ptr(pa_version()) };
    assert!(
        text.to_str()
            .unwrap()
            .chars()
            .all(|c| c.is_ascii_digit() || c == '.')
    );
}

#[test]
fn version_compatibility_is_major_only() {
    // AB-41：次版本差异（表尾追加）旧宿主仍可用；主版本不同必须失败
    assert!(version_compatible(PA_ABI_VERSION));
    assert!(version_compatible((PA_ABI_MAJOR << 16) | 7), "次版本高一点仍兼容");
    assert!(!version_compatible(((PA_ABI_MAJOR + 1) << 16) | PA_ABI_MINOR));
    assert!(!version_compatible(0));
}

#[test]
fn mismatch_message_is_diagnosable() {
    // T-AB-4：信息必须可诊断（含两侧版本）
    let mismatch = VersionMismatch {
        host_version: (2 << 16) | 3,
        runtime_version: PA_ABI_VERSION,
    };
    let message = mismatch.message();
    assert!(message.contains("2.3"), "含宿主版本：{message}");
    assert!(message.contains("1.0"), "含运行时版本：{message}");
}

#[test]
fn host_view_never_reads_beyond_the_declared_size() {
    // AB-43：以 min(宿主 size, 自身 size) 为界读取，禁止越界读。
    #[repr(C)]
    struct TinyHost {
        abi_size: usize,
    }
    let tiny = TinyHost {
        abi_size: size_of::<usize>(),
    };
    // SAFETY: 指针指向 TinyHost，可读一个 usize（正是它声明的尺寸）。
    let view = unsafe { view_host((&tiny as *const TinyHost).cast::<pa_host>()) };
    assert_eq!(view.abi_size, Some(size_of::<usize>()));
    assert_eq!(view.abi_version, None, "尺寸不够覆盖版本字段 ⇒ 不许读");
    assert_eq!(view.capabilities, None, "同上");

    // 完整的宿主结构：字段都读得到
    let full = pa_host {
        abi_size: size_of::<pa_host>(),
        abi_version: PA_ABI_VERSION,
        capabilities: core::ptr::null(),
    };
    // SAFETY: 指针指向完整的 pa_host。
    let view = unsafe { view_host(&full) };
    assert_eq!(view.abi_version, Some(PA_ABI_VERSION));
    assert!(view.capabilities.is_some());

    // NULL 宿主：全是 None（不 UB）
    // SAFETY: 允许传 NULL。
    let view = unsafe { view_host(core::ptr::null()) };
    assert!(view.abi_size.is_none() && view.abi_version.is_none());
}

#[test]
fn boundary_catches_panics() {
    // AB-3／CX-11：panic 绝不跨边界（T-AB-2 的一半）
    assert_eq!(boundary(|| PA_OK), PA_OK);
    let caught = boundary(|| panic!("宿主函数炸了"));
    assert_eq!(caught, PA_ERR_RUNTIME, "被捕获并转成状态码，进程不崩");
}
