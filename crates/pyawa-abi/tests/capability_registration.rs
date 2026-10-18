//! **`CP-30` 注册时校验**（＋ `AB-41` 的版本兼容矩阵在能力接口这一面的落地）。
//!
//! `pa_setcapability` 在**注册那一刻**读宿主域 vtable 的公共头 `(version, size)`（所有域 vtable
//! 的前两字段，`CP-30`）并校验：
//!
//! - **主版本相同**才可用（`AB-41`：改既有槽位语义／删函数 ⇒ 主版本 +1，旧宿主 `create` 必须失败）；
//! - 宿主**次版本较低** ⇒ 仍可用（它只是表尾少了几个较新的追加槽位）；**次版本较高** ⇒ 失败
//!   （运行时没有较新的追加槽位可提供）；
//! - `size` 必须 `>=` 公共头字节数、且不超过合理上限（防荒唐尺寸）。
//!
//! 失败返回 `PA_ERR_ABI`，原因经 `pa_errmsg` 可诊断；**不落半截状态**（旧实现不被覆盖）。
//!
//! 本文件只造**最小 header vtable**（`repr(C) { version, size }`）：注册入口不解释域形状
//! （`AB-32`：形状归 `CP-`），所以这里不需要 `pyawa-capabilities` 的真结构体。

use core::ffi::{c_void, CStr};
use core::mem::size_of;

use pyawa_abi::capability::{PA_ASYNC_OK, PA_DOMAIN_CLOCK, PA_DOMAIN_FS};
use pyawa_abi::status::{PA_ERR_ABI, PA_ERR_INVALID, PA_OK};
use pyawa_abi::{
    capability_version_compatible, pa_create, pa_destroy, pa_errmsg, pa_host, pa_setcapability,
    pa_setcapability_async, pa_state, version_string, PA_ABI_MAJOR, PA_ABI_MINOR, PA_ABI_SIZE,
    PA_ABI_VERSION, PA_CAPABILITY_HEADER_SIZE, PA_CAPABILITY_MAX_SIZE,
};

/// 测试宿主能给出的**最小** vtable：只按 `CP-30` 放公共头（注册入口只需要这两个字段）。
#[repr(C)]
struct HeaderVtable {
    version: u32,
    size: u32,
}

const fn header(version: u32, size: u32) -> HeaderVtable {
    HeaderVtable { version, size }
}

/// 合法的公共头（`version`＝三份契约共用号，`size`＝本表字节数）。
fn valid_header() -> HeaderVtable {
    header(PA_ABI_VERSION, size_of::<HeaderVtable>() as u32)
}

/// 建一个可用实例（本文件不测 `pa_create` 失败路径）。
fn new_state() -> *mut pa_state {
    let host = pa_host {
        abi_size: PA_ABI_SIZE,
        abi_version: PA_ABI_VERSION,
        capabilities: core::ptr::null(),
    };
    let mut state: *mut pa_state = core::ptr::null_mut();
    // SAFETY: 按 `AB-8`／`AB-43` 的契约传宿主结构；出参是本函数的局部量。
    let created = unsafe { pa_create(&host, &mut state) };
    assert_eq!(created, PA_OK, "建实例失败");
    assert!(!state.is_null(), "`pa_create` 成功了却不交回实例");
    state
}

/// 声明 `fs` 域的异步分类（`CP-25`：注册实现之前**必须**先声明）。
fn declare_fs_async_mut(state: *mut pa_state) {
    // SAFETY: `state` 由 `new_state` 交回且尚未销毁。
    assert_eq!(
        unsafe { pa_setcapability_async(state, PA_DOMAIN_FS, PA_ASYNC_OK) },
        PA_OK,
        "声明 `fs` 域异步分类失败"
    );
}

/// 注册一个 `fs` 域实现（返回状态码）。
fn register_fs(state: *mut pa_state, table: &HeaderVtable) -> i32 {
    let implementation = (table as *const HeaderVtable).cast::<c_void>();
    // SAFETY: `state` 存活；`table` 由调用方保证比 `state` 活得久（本文件的表都是测试函数的局部，
    // 且在销毁 state 之后才离开作用域）。
    unsafe { pa_setcapability(state, PA_DOMAIN_FS, implementation) }
}

/// 读当前错误消息（`pa_errmsg` 的借用约定：**只在本调用之后立刻读**）。
fn error_message(state: *mut pa_state) -> Option<String> {
    // SAFETY: `state` 由 `new_state` 交回、尚未销毁。
    let pointer = unsafe { pa_errmsg(state) };
    if pointer.is_null() {
        return None;
    }
    // SAFETY: 非空 ⇒ 指向实例持有的 NUL 结尾消息（借用期到下一次 API 调用为止）。
    Some(unsafe { CStr::from_ptr(pointer) }.to_string_lossy().into_owned())
}

/// 实例当前登记的 `fs` 域实现指针（`None` ＝ 未注册／已被 `null` 注册）。
fn stored_fs_implementation(state: *mut pa_state) -> Option<*mut c_void> {
    // SAFETY: `state` 存活；`instance()` 只借用。
    let instance = unsafe { &*state }.instance();
    instance
        .capability(PA_DOMAIN_FS as usize)
        .map(|pointer| pointer.cast_mut())
}

/// `AB-41` 的兼容判定本身（纯函数面）也守一条，避免"测试只测了注册、规则却写反"。
#[test]
fn the_ab_41_compatibility_rule_is_explicit() {
    // 主版本不同 ⇒ 一律不兼容（无论次版本）。
    assert!(!capability_version_compatible((PA_ABI_MAJOR + 1) << 16));
    assert!(!capability_version_compatible(
        ((PA_ABI_MAJOR + 1) << 16) | 0xFFFF
    ));
    // 主版本相同、次版本不高于运行时 ⇒ 兼容（旧宿主仍可用）。
    assert!(capability_version_compatible(PA_ABI_VERSION));
    assert!(capability_version_compatible(PA_ABI_MAJOR << 16));
    // 主版本相同、次版本高于运行时 ⇒ 不兼容。
    assert!(!capability_version_compatible(
        (PA_ABI_MAJOR << 16) | (PA_ABI_MINOR + 1)
    ));
}

/// ① 合法版本＋尺寸 ⇒ 注册成功（且 `NULL` 仍表示"整域未实现"）。
#[test]
fn a_valid_header_registers_successfully() {
    let table = valid_header();
    let state = new_state();
    declare_fs_async_mut(state);
    assert_eq!(
        register_fs(state, &table),
        PA_OK,
        "合法公共头必须注册成功（CP-30）"
    );
    assert!(
        error_message(state).is_none(),
        "成功路径不该留下错误消息"
    );
    let stored = stored_fs_implementation(state).expect("注册成功后实例里应存到实现指针");
    assert_eq!(
        stored,
        (&table as *const HeaderVtable).cast_mut().cast::<c_void>(),
        "注册的必须是宿主那份 vtable 的指针"
    );

    // `CP-2`：`NULL` vtable ＝ 整域未实现 —— 仍允许注册（不解释、不校验）。
    // SAFETY: `state` 存活。
    assert_eq!(
        unsafe { pa_setcapability(state, PA_DOMAIN_FS, core::ptr::null()) },
        PA_OK,
        "CP-2：null vtable 允许注册（创建期不拒绝）"
    );
    assert!(
        stored_fs_implementation(state).is_none(),
        "null vtable 注册后实例里不该有实现指针"
    );

    // SAFETY: `state` 由 `new_state` 交回、尚未销毁。
    assert_eq!(unsafe { pa_destroy(state) }, PA_OK);
}

/// ② 主版本不符 ⇒ 注册失败（`PA_ERR_ABI`）且**可诊断**；状态不被半截改动。
#[test]
fn a_major_version_mismatch_fails_the_registration_diagnosably() {
    let state = new_state();
    declare_fs_async_mut(state);

    // 先注册一份**合法**实现，作为"失败不得覆盖旧实现"的基线。
    let good = valid_header();
    assert_eq!(register_fs(state, &good), PA_OK);
    let good_pointer = stored_fs_implementation(state).expect("基线实现应当登记上");
    let baseline = error_message(state);

    for host_version in [
        (PA_ABI_MAJOR + 1) << 16,
        (PA_ABI_MAJOR + 7) << 16 | 3,
        // 主版本 0（`version: 1` 这类"未填"哨兵的编码）同样是主版本不符。
        (PA_ABI_MAJOR - 1) << 16 | 1,
    ] {
        let bad = header(host_version, size_of::<HeaderVtable>() as u32);
        assert_eq!(
            register_fs(state, &bad),
            PA_ERR_ABI,
            "主版本不符必须失败（AB-41／AB-40），host_version={host_version:#x}"
        );
        let message = error_message(state).expect("失败必须留下可诊断消息（AB-40）");
        assert!(
            message.contains("ABI 版本不兼容") && message.contains("主版本不符"),
            "消息要说明是主版本问题：{message}"
        );
        assert!(
            message.contains(&version_string(host_version))
                && message.contains(&version_string(PA_ABI_VERSION)),
            "消息要同时带宿主与运行时版本：{message}"
        );
        assert_eq!(
            stored_fs_implementation(state),
            Some(good_pointer),
            "注册失败不得覆盖已登记的实现（禁止半截状态）"
        );
    }

    assert!(
        baseline.is_none(),
        "合法注册本身不该产生错误消息（失败后再检查可诊断性才有意义）"
    );
    // 失败之后**合法**注册仍应成功（实例没被弄坏）。
    assert_eq!(register_fs(state, &good), PA_OK, "失败后合法注册仍应成功");
    // SAFETY: `state` 由 `new_state` 交回、尚未销毁。
    assert_eq!(unsafe { pa_destroy(state) }, PA_OK);
}

/// ③ `size` 非法（过小／荒唐大）⇒ 注册失败，且**不越界读**（只读公共头，不按声明尺寸再读）。
#[test]
fn an_invalid_header_size_fails_the_registration() {
    let state = new_state();
    declare_fs_async_mut(state);

    let too_small = [
        0u32,
        1,
        PA_CAPABILITY_HEADER_SIZE - 1,
    ];
    for size in too_small {
        let bad = header(PA_ABI_VERSION, size);
        assert_eq!(
            register_fs(state, &bad),
            PA_ERR_ABI,
            "size={size} 小于公共头必须失败（CP-30）"
        );
        let message = error_message(state).expect("失败必须可诊断");
        assert!(
            message.contains("size") && message.contains("最小"),
            "消息要说明尺寸小于公共头：{message}"
        );
        assert!(
            stored_fs_implementation(state).is_none(),
            "失败不得留下实现"
        );
    }

    let too_large = [PA_CAPABILITY_MAX_SIZE + 1, u32::MAX];
    for size in too_large {
        let bad = header(PA_ABI_VERSION, size);
        assert_eq!(
            register_fs(state, &bad),
            PA_ERR_ABI,
            "size={size} 超过合理上限必须失败（CP-30）"
        );
        let message = error_message(state).expect("失败必须可诊断");
        assert!(
            message.contains("size") && message.contains("上限"),
            "消息要说明尺寸超过上限：{message}"
        );
    }

    // 公共头正好 8 字节 ⇒ 合法（最小头就是它）。
    let minimal = header(PA_ABI_VERSION, PA_CAPABILITY_HEADER_SIZE);
    assert_eq!(
        register_fs(state, &minimal),
        PA_OK,
        "size 恰为公共头字节数应当合法"
    );

    // SAFETY: `state` 由 `new_state` 交回、尚未销毁。
    assert_eq!(unsafe { pa_destroy(state) }, PA_OK);
}

/// ④ 次版本：宿主**较低** ⇒ 可用（旧宿主仍可用）；宿主**较高** ⇒ 失败。
#[test]
fn minor_versions_follow_the_ab_41_direction() {
    let state = new_state();
    declare_fs_async_mut(state);

    let older = header((PA_ABI_MAJOR << 16) | PA_ABI_MINOR.saturating_sub(1), 8);
    if PA_ABI_MINOR > 0 {
        assert_eq!(
            register_fs(state, &older),
            PA_OK,
            "AB-41：旧宿主（次版本较低）仍可用"
        );
    }

    let newer = header((PA_ABI_MAJOR << 16) | (PA_ABI_MINOR + 1), 8);
    assert_eq!(
        register_fs(state, &newer),
        PA_ERR_ABI,
        "AB-41：宿主次版本高于运行时必须失败"
    );
    let message = error_message(state).expect("失败必须可诊断");
    assert!(
        message.contains("次版本"),
        "消息要说明是次版本方向问题：{message}"
    );

    // 与运行时完全同版本 ⇒ 成功。
    assert_eq!(register_fs(state, &header(PA_ABI_VERSION, 8)), PA_OK);

    // SAFETY: `state` 由 `new_state` 交回、尚未销毁。
    assert_eq!(unsafe { pa_destroy(state) }, PA_OK);
}

/// ⑤ 顺序判据：**分类先于实现**（`CP-25`／`T-AB-6`）的既有语义不变，且未声明分类时**不读**
/// 宿主 vtable（避免为一次必败的调用去碰宿主内存）。
#[test]
fn classification_is_checked_before_the_header_and_never_defaults() {
    let table = valid_header();
    let state = new_state();
    let implementation = (&table as *const HeaderVtable).cast::<c_void>();

    // 未声明分类 ⇒ 即使公共头合法也失败；且这属于宿主用法错误（`PA_ERR_INVALID`）。
    // SAFETY: `state` 存活；`table` 活到测试末尾。
    assert_eq!(
        unsafe { pa_setcapability(state, PA_DOMAIN_FS, implementation) },
        PA_ERR_INVALID,
        "CP-25：缺失异步分类即注册失败"
    );
    assert!(
        stored_fs_implementation(state).is_none(),
        "CP-25：失败不得落默认值"
    );

    // 未声明分类时**不去读**不合法的指针也不会崩：域号越界在最前面被拒（`AB-33`）。
    // SAFETY: 这里故意传一个不可解引用的指针，但域号非法 ⇒ 应当在读它之前就返回。
    assert_eq!(
        unsafe { pa_setcapability(state, -1, 0x1234usize as *const c_void) },
        PA_ERR_INVALID,
        "域号非法必须拒绝（且不得解引用实现指针）"
    );

    // 声明分类后同一张表注册成功（同一实例，证明上面的失败没有污染状态）。
    declare_fs_async_mut(state);
    assert_eq!(register_fs(state, &table), PA_OK);

    // `clock` 域同样走这条校验（抽查：非 `fs` 域也共用同一公共头规则）。
    // SAFETY: `state` 存活。
    assert_eq!(
        unsafe { pa_setcapability_async(state, PA_DOMAIN_CLOCK, PA_ASYNC_OK) },
        PA_OK
    );
    let clock_table = valid_header();
    // SAFETY: `state`／`clock_table` 都活到测试末尾。
    assert_eq!(
        unsafe {
            pa_setcapability(
                state,
                PA_DOMAIN_CLOCK,
                (&clock_table as *const HeaderVtable).cast::<c_void>(),
            )
        },
        PA_OK,
        "clock 域同样按 CP-30 公共头校验"
    );

    // SAFETY: `state` 由 `new_state` 交回、尚未销毁。
    assert_eq!(unsafe { pa_destroy(state) }, PA_OK);
}
