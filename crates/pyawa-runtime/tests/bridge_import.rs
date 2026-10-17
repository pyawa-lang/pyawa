//! **过渡桥的公开入口**（第 216 轮）：`pyawa_core::executor::import::import_through_bridge` ✓ ——
//! ①a「Python 层 finder／loader」的**前置** ✓：finder 要能借它真装出模块 ✓，且**找不到时必须如实给
//! `None`** ✓（不许编假模块 ✗、也不许把"找不到"当异常抛 ✗ —— 第 215 轮实测到的正是后者 ✗）。

use pyawa_runtime::PaState;

#[test]
fn bridge_import_entry_loads_registered_modules_and_reports_missing_ones() {
    let state = PaState::new().expect("建实例");
    let raw = state.as_ptr();
    // SAFETY: `raw` 由 `PaState::new` 交回且活到本函数末尾。
    let instance = unsafe { &*raw }.instance();
    pyawa_stdlib::install(instance, "probe", &[]);

    let registered = pyawa_core::executor::import::import_through_bridge(instance, "errno")
        .expect("已登记的模块不该报错");
    assert!(registered.is_some(), "已登记的模块应当能装出 ✓（finder 要拿它当 spec 的凭据 ✓）");

    let missing = pyawa_core::executor::import::import_through_bridge(instance, "pyawa_no_such_module")
        .expect("找不到**不是**异常 ✓（finder 的 `find_spec` 要返回 `None` ✓）");
    assert!(missing.is_none(), "找不到必须如实给 `None` ✓");
}
