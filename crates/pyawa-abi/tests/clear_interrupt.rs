//! **AB-92**：`pa_clear_interrupt` 让“一个任务被中断”不阻塞同实例的其他任务。

use core::mem::size_of;

use pyawa_abi::status::*;
use pyawa_abi::*;

fn compatible_host() -> pa_host {
    pa_host {
        abi_size: size_of::<pa_host>(),
        abi_version: PA_ABI_VERSION,
        capabilities: core::ptr::null(),
    }
}

#[test]
fn clear_interrupt_unblocks_other_tasks() {
    let host = compatible_host();
    let mut state: *mut pa_state = core::ptr::null_mut();
    assert_eq!(unsafe { pa_create(&host, &mut state) }, PA_OK);
    unsafe { pyawa_stdlib::install((&*state).instance(), "probe", &[]) };
    let src = b"def g():\n    yield 1\n    yield 2\n    return 3\ng1 = g()\ng2 = g()\n";
    assert_eq!(
        unsafe {
            pa_exec_string(
                state,
                src.as_ptr().cast(),
                src.len() as isize,
                c"clear-interrupt".as_ptr(),
                c"python".as_ptr(),
                core::ptr::null(),
            )
        },
        PA_OK
    );

    // g1 首次让出 1。
    assert_eq!(unsafe { pa_getglobal(state, c"g1".as_ptr()) }, PA_OK);
    assert_eq!(unsafe { pa_resume(state, 0, 1) }, PA_YIELDED);
    assert_eq!(unsafe { pa_settop(state, 0) }, PA_OK);

    // 中断：g1 的 resume 返回 PA_ERR_INTERRUPT；实例级标志仍在。
    assert_eq!(unsafe { pa_interrupt(state) }, PA_OK);
    assert_eq!(unsafe { pa_getglobal(state, c"g1".as_ptr()) }, PA_OK);
    assert_eq!(unsafe { pa_resume(state, 0, 1) }, PA_ERR_INTERRUPT);
    assert_eq!(unsafe { pa_settop(state, 0) }, PA_OK);

    // 清中断后，g2 可继续驱动（不被 g1 的中断拖住）。
    assert_eq!(unsafe { pa_clear_interrupt(state) }, PA_OK);
    assert_eq!(unsafe { pa_getglobal(state, c"g2".as_ptr()) }, PA_OK);
    assert_eq!(unsafe { pa_resume(state, 0, 1) }, PA_YIELDED);
    let mut value = 0i64;
    assert_eq!(unsafe { pa_tointeger(state, -1, &mut value) }, PA_OK);
    assert_eq!(value, 1);
    assert_eq!(unsafe { pa_settop(state, 0) }, PA_OK);
    assert_eq!(unsafe { pa_getglobal(state, c"g2".as_ptr()) }, PA_OK);
    assert_eq!(unsafe { pa_resume(state, 0, 1) }, PA_YIELDED);
    assert_eq!(unsafe { pa_tointeger(state, -1, &mut value) }, PA_OK);
    assert_eq!(value, 2);
    unsafe { pa_destroy(state) };
}
