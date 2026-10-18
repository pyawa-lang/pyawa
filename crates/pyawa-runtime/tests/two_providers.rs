//! **M5 判据②** 的可执行验收：**同一脚本、两种 provider、写法不变**
//! （`SPEC-capabilities.md` §11 的 `T-CP-11`；也覆盖 `T-CP-4`／`T-CP-5`／`T-CP-12`／`T-CP-7` 的真实现面）。
//!
//! # 两档 provider
//!
//! | 档 | 类型 | 语义 |
//! |---|---|---|
//! | **真实** | [`PosixFs`]（`pyawa_runtime::fs_posix`） | 真实机器：路径原样交给 OS |
//! | **受限** | [`RestrictedFs`]（同文件；本轮新增） | 根能力 ＝ 一个目录（chroot）：`..`／符号链接出不了根；`/proc`、`/dev`、`/sys` 就是根下的同名路径（不存在 ⇒ `ENOENT`）；额外只读根（工作区 `Lib/`）供 import |
//!
//! **写法不变**的判据：脚本源码是**同一个文件、逐字节相同**（本测试把同一份源码写进两个根并断言
//! 字节相等），脚本里**没有**任何"按 provider 分支"的东西；比较**退出码 ＋ stdout ＋ 未捕获异常
//! 类型／消息**，归一规则按 `MS-9`（路径前缀／`0x` 地址／行尾与末尾换行）。
//!
//! # 如实记的两件事（不藏）
//!
//! 1. **观察通道**：`write` 槽被测试包了一层——句柄 `1`／`2` 记进 sink（`MS-8` 的 stdout 通道），
//!    其余句柄原样委托给 provider 自己的 `write`（**不是**把 provider 的写能力替换掉）。
//!    两个档走**同一条**观察通道 ⇒ 比较是对称的。
//! 2. **`CP-21`／`CP-32` 的缺口**：规格要求路径解析在 VM 侧（`CP-7`"权威在 provider、判定在 VM"）。
//!    现在**没有** VM 侧解析器 ⇒ 受限档的解析在 provider 内（`RestrictedFs` 的文档里写明是过渡档）。
//!    ⇒ `T-CP-11` 可以绿，但 `CP-21`／`CP-32` **不**因此算满足（M5 矩阵里单列）。
//!
//! 跑法：`cargo test -p pyawa-runtime --test two_providers`。

use std::path::{Path, PathBuf};
use std::sync::Mutex;

use pyawa_abi::capability::{PA_ASYNC_OK, PA_DOMAIN_FS};
use pyawa_abi::status::PA_OK;
use pyawa_abi::{
    pa_create, pa_destroy, pa_errmsg, pa_exec_string, pa_host, pa_setcapability,
    pa_setcapability_async, pa_state, PA_ABI_SIZE, PA_ABI_VERSION,
};
use pyawa_capabilities::fs::{CapStatus, CpFsVtable, Handle};
use pyawa_core::CapabilityCallError;
use pyawa_runtime::fs_posix::{PosixFs, RestrictedFs};

// --------------------------------------------------------------------------- #
// 夹具与观察通道
// --------------------------------------------------------------------------- #

/// 所有 `run` 共用一个锁：拿它的时候就顺手改进程 cwd（真实档要按 cwd 解析相对名字），
/// 所以两个档的跑必须串行（`SINK`／`INNER_WRITE` 也是进程内共享状态）。
static RUN_LOCK: Mutex<()> = Mutex::new(());

/// 脚本自己的 stdout／stderr（句柄 `1`／`2`）落点——**两档共用**（`MS-8` 的观测通道）。
static SINK: Mutex<Vec<u8>> = Mutex::new(Vec::new());

/// 被包住的 provider `write` 槽：`(原函数指针, 原 state 指针的整数形态)`。
///
/// `Mutex<Option<(fn, usize)>>` 而不是 `*mut c_void`：`static` 要求 `Sync`，裸指针不是 `Send`。
static INNER_WRITE: Mutex<
    Option<(
        extern "C" fn(*mut core::ffi::c_void, Handle, *const u8, usize, *mut usize, *mut i32) -> CapStatus,
        usize,
    )>,
> = Mutex::new(None);

/// 观察通道的 `write` 槽：标准流记进 [`SINK`]，其它句柄**原样委托**给真 provider。
extern "C" fn recording_write(
    _state: *mut core::ffi::c_void,
    handle: Handle,
    bytes: *const u8,
    len: usize,
    out_len: *mut usize,
    errno_out: *mut i32,
) -> CapStatus {
    if handle.0 == 1 || handle.0 == 2 {
        // SAFETY: 契约同 `SPEC-capabilities.md` §9.1（缓冲区可读 `len` 字节）。
        let slice = unsafe { core::slice::from_raw_parts(bytes, len) };
        SINK.lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .extend_from_slice(slice);
        if !out_len.is_null() {
            // SAFETY: 出参由调用方提供。
            unsafe { *out_len = len };
        }
        return CapStatus::Ok;
    }
    let (inner, state_number) = INNER_WRITE
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
        .expect("`run` 必须先装好原 `write` 槽");
    // 内层 `write` 要的是**内层** `state`（`PosixFs` 或 `RestrictedFs` 自己）⇒ 用装进来的那一个；
    // 观察通道收到的 `_state` 是外层 vtable 的 state，句柄类槽位不需要它。
    // SAFETY: 原函数指针与 state 都由当前 provider 的 vtable 给出，且 provider 活到本次调用之后。
    inner(
        state_number as *mut core::ffi::c_void,
        handle,
        bytes,
        len,
        out_len,
        errno_out,
    )
}

/// 一次脚本执行的可观察结果（`MS-8` 的三项：退出码／stdout／未捕获异常）。
#[derive(Debug, Clone, PartialEq, Eq)]
struct Observation {
    exit: i32,
    /// 脚本自己的 stdout（逐行；**行尾与末尾换行已归一**，`MS-9`）。
    stdout: String,
    exception: Option<(String, String)>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ProviderKind {
    Real,
    Restricted,
}

/// 工作区的 `Lib/`（受限档的**只读额外根**：import 要按绝对路径读模块文件）。
fn lib_directory() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../Lib")
        .canonicalize()
        .expect("工作区 `Lib/` 应当存在")
}

/// 建两棵**初始内容相同**的根：`<base>/real`（真实档 cwd）与 `<base>/restricted`（受限档根能力）。
struct Fixture {
    base: PathBuf,
    real: PathBuf,
    restricted: PathBuf,
}

fn fixture(name: &str) -> Fixture {
    let base = std::env::temp_dir().join(format!(
        "pyawa-two-providers-{}-{name}",
        std::process::id()
    ));
    let _ = std::fs::remove_dir_all(&base);
    let real = base.join("real");
    let restricted = base.join("restricted");
    for directory in [&real, &restricted] {
        std::fs::create_dir_all(directory).expect("建根目录");
        std::fs::write(directory.join("greeting.txt"), "hello from the capability layer\n")
            .expect("写 greeting.txt");
        std::os::unix::fs::symlink("greeting.txt", directory.join("link")).expect("建符号链接");
        std::os::unix::fs::symlink("../outside.txt", directory.join("escape-link"))
            .expect("建逃逸符号链接");
    }
    // 根**外面**的机密文件：真实档读得到，受限档必须看不见。
    std::fs::write(base.join("outside.txt"), "outside-secret\n").expect("写根外文件");
    Fixture { base, real, restricted }
}

impl Fixture {
    /// 把同一份脚本源码写进两棵根，并断言**逐字节相同**（"写法不变"的判据）。
    fn plant_script(&self, source: &str) {
        for directory in [&self.real, &self.restricted] {
            std::fs::write(directory.join("probe.py"), source).expect("写脚本");
        }
        let real_bytes = std::fs::read(self.real.join("probe.py")).expect("读真实档脚本");
        let restricted_bytes = std::fs::read(self.restricted.join("probe.py")).expect("读受限档脚本");
        assert_eq!(
            real_bytes, restricted_bytes,
            "`T-CP-11`：两档必须是**同一份脚本**（逐字节相同）"
        );
    }
}

impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.base);
    }
}

/// 跑一次脚本（`kind` 选 provider；`root` 同时是该次运行的 cwd）。
fn run(kind: ProviderKind, root: &Path) -> Observation {
    let _serialized = RUN_LOCK.lock().unwrap_or_else(|poisoned| poisoned.into_inner());
    let source = std::fs::read_to_string(root.join("probe.py")).expect("读脚本");
    let previous_directory = std::env::current_dir().expect("取 cwd");
    std::env::set_current_dir(root).expect("切 cwd（真实档按 cwd 解析相对名字）");
    SINK.lock().unwrap_or_else(|poisoned| poisoned.into_inner()).clear();

    // provider 必须在 state 之前声明（它被 vtable 的 `state` 裸指针借用；本函数末尾 state 先销毁）。
    let provider = match kind {
        ProviderKind::Real => ActualProvider::Real(PosixFs::new()),
        ProviderKind::Restricted => ActualProvider::Restricted(
            RestrictedFs::new(root, vec![lib_directory()]).expect("建受限 provider（根必须存在）"),
        ),
    };
    let mut table = provider.vtable();
    let original_write = table.write.expect("provider 必须实现 `write`");
    *INNER_WRITE.lock().unwrap_or_else(|poisoned| poisoned.into_inner()) =
        Some((original_write, table.state as usize));
    table.write = Some(recording_write);

    let host = pa_host {
        abi_size: PA_ABI_SIZE,
        abi_version: PA_ABI_VERSION,
        capabilities: core::ptr::null(),
    };
    let mut state: *mut pa_state = core::ptr::null_mut();
    // SAFETY: 按 `AB-8`／`AB-43` 的契约传宿主结构；`table` 活到 state 销毁之后。
    let created = unsafe { pa_create(&host, &mut state) };
    assert_eq!(created, PA_OK, "建实例失败");

    // SAFETY: `state` 刚建成功，活到本函数末尾；`table` 的地址在 state 存活期内有效。
    unsafe {
        assert_eq!(
            pa_setcapability_async(state, PA_DOMAIN_FS, PA_ASYNC_OK),
            PA_OK,
            "声明 `fs` 域异步分类失败"
        );
        assert_eq!(
            pa_setcapability(state, PA_DOMAIN_FS, (&table as *const CpFsVtable).cast()),
            PA_OK,
            "注册 `fs` 域实现失败"
        );
        // **组合根装配**（`CM-14`）：与 CLI 同款 ⇒ 脚本可以用 `print`／`import`。
        pyawa_stdlib::install((&*state).instance(), "probe", &[]);
        pyawa_stdlib::set_module_search_path(
            (&*state).instance(),
            &[lib_directory().to_string_lossy().into_owned()],
        );
    }

    let mode = b"python\0";
    // SAFETY: 源码指针＋长度由 `source` 的借用保证；`chunkname`／`options` 传 NULL（`AB-60`／`AB-61`）。
    let status = unsafe {
        pa_exec_string(
            state,
            source.as_ptr().cast(),
            source.len() as isize,
            core::ptr::null(),
            mode.as_ptr().cast(),
            core::ptr::null(),
        )
    };
    let exit = if status == PA_OK { 0 } else { 1 };
    let message = if status == PA_OK {
        String::new()
    } else {
        // SAFETY: 借用只在本次调用之后有效 ⇒ 当场复制（`AB-48`）。
        unsafe {
            let pointer = pa_errmsg(state);
            if pointer.is_null() {
                String::new()
            } else {
                core::ffi::CStr::from_ptr(pointer).to_string_lossy().into_owned()
            }
        }
    };
    let exception = message.split_once(": ").map(|(kind, text)| {
        (
            normalize_text(kind.trim(), root),
            normalize_text(text.trim(), root),
        )
    });

    let recorded = {
        let mut sink = SINK.lock().unwrap_or_else(|poisoned| poisoned.into_inner());
        core::mem::take(&mut *sink)
    };
    // SAFETY: `state` 由 `pa_create` 交回、尚未销毁；provider 活到它的 vtable 使用结束之后。
    unsafe { pa_destroy(state) };
    *INNER_WRITE.lock().unwrap_or_else(|poisoned| poisoned.into_inner()) = None;
    std::env::set_current_dir(previous_directory).expect("恢复 cwd");

    Observation {
        exit,
        stdout: normalize_bytes(&recorded, root),
        exception,
    }
}

enum ActualProvider {
    Real(PosixFs),
    Restricted(RestrictedFs),
}

impl ActualProvider {
    fn vtable(&self) -> CpFsVtable {
        match self {
            Self::Real(provider) => provider.vtable(),
            Self::Restricted(provider) => provider.vtable(),
        }
    }
}

// --------------------------------------------------------------------------- #
// `MS-9` 的归一：路径前缀 / `0x…` 地址 / 行尾与末尾换行
// --------------------------------------------------------------------------- #

fn normalize_text(text: &str, root: &Path) -> String {
    let root_text = root.to_string_lossy().into_owned();
    let replaced = text.replace(&root_text, "<ROOT>");
    mask_addresses(&replaced)
}

fn normalize_bytes(bytes: &[u8], root: &Path) -> String {
    let text = String::from_utf8_lossy(bytes).into_owned();
    let normalized = normalize_text(&text, root);
    // 行尾与末尾换行归一（`MS-9`）：逐行去行尾空白，再按 `\n` 拼回。
    normalized.lines().map(str::trim_end).collect::<Vec<_>>().join("\n")
}

/// 把 `0x` 后跟十六进制数字的串换成 `0xADDR`（与 `crates/pyawa-abi/tests/conformance.rs` 同一口径）。
fn mask_addresses(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    let mut rest = text;
    while let Some(position) = rest.find("0x") {
        out.push_str(&rest[..position]);
        let after = &rest[position + 2..];
        let digits = after.chars().take_while(|character| character.is_ascii_hexdigit()).count();
        if digits == 0 {
            out.push_str("0x");
            rest = after;
        } else {
            out.push_str("0xADDR");
            rest = &after[digits..];
        }
    }
    out.push_str(rest);
    out
}

// --------------------------------------------------------------------------- #
// T-CP-11：同一脚本、两档观察相同
// --------------------------------------------------------------------------- #

/// `T-CP-11` 的脚本：只用 Python 与 `os`／内建（**不**含任何按 provider 分支的写法）。
///
/// 覆盖面：`print`（`fs.write` 句柄 1）、文本读、`os.open`／`read`／`write`／`close`（含**两次
/// close**，`CP-17` 幂等）、`os.readlink`、`os.stat`、`fd` 是 `int`（`CP-14`）、以及一个
/// **两档都会** `FileNotFoundError` 的相对路径。
const SAME_SCRIPT: &str = r#"import os

print("same-script")
print(open("greeting.txt").read().strip())
fd = os.open("greeting.txt", 0)
print(type(fd).__name__, isinstance(fd, int), fd >= 3)
print(len(os.read(fd, 5)))
os.close(fd)
os.close(fd)
print("double-close-ok")
print(os.readlink("link"))
print(os.stat("greeting.txt").st_size)
writer = os.open("created.txt", 65)
print(os.write(writer, b"payload\n"))
os.close(writer)
print(open("created.txt").read().strip())
try:
    open("missing.txt")
except FileNotFoundError as exc:
    print(type(exc).__name__, str(exc))
print("done")
raise ValueError("boom")
"#;

#[test]
fn the_same_script_has_identical_observations_on_both_providers() {
    let fixture = fixture("same-script");
    fixture.plant_script(SAME_SCRIPT);

    let real = run(ProviderKind::Real, &fixture.real);
    let restricted = run(ProviderKind::Restricted, &fixture.restricted);

    // 两档都应当以**同一个未捕获异常**收场（`MS-8` 的"异常类型＋消息"也进入比较）。
    assert_eq!(real.exit, 1, "脚本末尾故意抛异常 ⇒ 退出码 1：{real:?}");
    assert_eq!(restricted.exit, 1, "受限档同样：{restricted:?}");
    assert_eq!(
        real.exception,
        Some(("ValueError".to_owned(), "boom".to_owned())),
        "真实档的未捕获异常类型／消息不对：{real:?}"
    );
    assert_eq!(
        restricted.exception, real.exception,
        "`T-CP-11`：未捕获异常的类型／消息两档必须一致"
    );
    assert_eq!(
        restricted.stdout, real.stdout,
        "`T-CP-11`：同一脚本在两种 provider 下的 stdout 必须一致（已按 `MS-9` 归一）\n\
         --- 真实 ---\n{}\n--- 受限 ---\n{}",
        real.stdout, restricted.stdout
    );
    assert!(
        real.stdout.contains("same-script") && real.stdout.contains("FileNotFoundError"),
        "脚本要真的跑到（否则比较是空转）：{real:?}"
    );
    // 两份观察原样打出来（`cargo test … -- --nocapture` 里看得到；默认被 libtest 收着）。
    eprintln!("[T-CP-11] 真实档观察：\n{real:#?}");
    eprintln!("[T-CP-11] 受限档观察：\n{restricted:#?}");
}

// --------------------------------------------------------------------------- #
// T-CP-4：`..`／符号链接出不了根（真实档做对照：它读得到根外）
// --------------------------------------------------------------------------- #

const ESCAPE_SCRIPT: &str = r#"print("escape-script")
for label, path in (("dotdot", "../outside.txt"), ("symlink", "escape-link"), ("absolute", "/etc/passwd")):
    try:
        data = open(path).read()
        print(label, "read", len(data))
    except FileNotFoundError:
        print(label, "FileNotFoundError")
    except OSError as exc:
        print(label, type(exc).__name__)
print("done")
"#;

#[test]
fn the_restricted_provider_cannot_escape_its_root() {
    let fixture = fixture("escape");
    fixture.plant_script(ESCAPE_SCRIPT);

    let real = run(ProviderKind::Real, &fixture.real);
    let restricted = run(ProviderKind::Restricted, &fixture.restricted);
    assert_eq!(real.exit, 0);
    assert_eq!(restricted.exit, 0);

    // 对照：真实档真的读到了根外的东西（说明夹具的"逃逸"确实有效，不是空测）。
    for label in ["dotdot", "symlink", "absolute"] {
        assert!(
            real.stdout.contains(&format!("{label} read")),
            "真实档应当读到 {label}（否则对照无意义）：\n{}",
            real.stdout
        );
    }
    // `CP-21`／`T-CP-4`：受限档一律"看不见"——`..` 被夹在根内、符号链接目标在根外、
    // 宿主机绝对路径按 chroot 落在根下 ⇒ 三种都是 `FileNotFoundError`（`CP-23` 形态）。
    for label in ["dotdot", "symlink", "absolute"] {
        assert!(
            restricted.stdout.contains(&format!("{label} FileNotFoundError")),
            "受限档的 {label} 必须 FileNotFoundError：\n{}",
            restricted.stdout
        );
    }
    assert!(
        !restricted.stdout.contains(" read "),
        "受限档不得读到根外任何东西：\n{}",
        restricted.stdout
    );
    eprintln!("[T-CP-4] 真实档（对照，读得到根外）：\n{}", real.stdout);
    eprintln!("[T-CP-4] 受限档（出不了根）：\n{}", restricted.stdout);
}

// --------------------------------------------------------------------------- #
// T-CP-5：受限档的 `/proc`／`/dev`／`/sys` ⇒ `FileNotFoundError`
// --------------------------------------------------------------------------- #

const SPECIAL_ROOTS_SCRIPT: &str = r#"print("special-roots")
for path in ("/proc/self/status", "/dev/null", "/sys"):
    try:
        data = open(path).read()
        print(path, "read", len(data))
    except FileNotFoundError:
        print(path, "FileNotFoundError")
    except OSError as exc:
        print(path, type(exc).__name__)
print("done")
"#;

#[test]
fn the_restricted_provider_reports_special_roots_as_file_not_found() {
    let fixture = fixture("special-roots");
    fixture.plant_script(SPECIAL_ROOTS_SCRIPT);

    let real = run(ProviderKind::Real, &fixture.real);
    let restricted = run(ProviderKind::Restricted, &fixture.restricted);
    assert_eq!(real.exit, 0);
    assert_eq!(restricted.exit, 0);

    // 真实档：这台机器确实有 `/proc`、`/dev`（否则这条对照无意义）。
    for path in ["/proc/self/status", "/dev/null"] {
        assert!(
            real.stdout.contains(&format!("{path} read")),
            "真实档应当读到 {path}：\n{}",
            real.stdout
        );
    }
    // `CP-23`／`T-CP-5`：受限档没有这三处 ⇒ 必须是 `FileNotFoundError`（**不是**"未实现"）。
    for path in ["/proc/self/status", "/dev/null", "/sys"] {
        assert!(
            restricted.stdout.contains(&format!("{path} FileNotFoundError")),
            "受限档的 {path} 必须 FileNotFoundError：\n{}",
            restricted.stdout
        );
    }
    assert!(
        !restricted.stdout.contains("未实现"),
        "`CP-23`：禁止表现为未实现：\n{}",
        restricted.stdout
    );
    eprintln!("[T-CP-5] 真实档：\n{}", real.stdout);
    eprintln!("[T-CP-5] 受限档：\n{}", restricted.stdout);
}

// --------------------------------------------------------------------------- #
// T-CP-12：真 provider 的 `close` 幂等 ＋ 关闭后操作报机器错误
// --------------------------------------------------------------------------- #

/// 宿主 errno 表的唯一真相在 `pyawa-abi`（不在测试里写第二个数字）。
fn host_errno(name: &str) -> i32 {
    pyawa_abi::platform_errno::HOST_ERRNO
        .iter()
        .find(|(key, _)| *key == name)
        .map(|(_, value)| *value as i32)
        .unwrap_or_else(|| panic!("宿主 errno 表里没有 {name}"))
}

/// 建实例并注册一个 `fs` 域实现（**不**装 stdlib：这里只打 VM 适配层的调用）。
///
/// # Safety
///
/// `state` 必须先于 `table`／provider 销毁（本函数只负责建，销毁由调用方做）。
unsafe fn state_with_fs(table: *const CpFsVtable) -> *mut pa_state {
    let host = pa_host {
        abi_size: PA_ABI_SIZE,
        abi_version: PA_ABI_VERSION,
        capabilities: core::ptr::null(),
    };
    let mut state: *mut pa_state = core::ptr::null_mut();
    // SAFETY: 调用方契约（见函数注释）。
    unsafe {
        assert_eq!(pa_create(&host, &mut state), PA_OK, "建实例失败");
        assert_eq!(pa_setcapability_async(state, PA_DOMAIN_FS, PA_ASYNC_OK), PA_OK);
        assert_eq!(
            pa_setcapability(state, PA_DOMAIN_FS, table.cast()),
            PA_OK,
            "注册 `fs` 域实现失败"
        );
    }
    state
}

#[test]
fn close_is_idempotent_and_closed_handles_fail_on_both_providers() {
    let base = std::env::temp_dir().join(format!(
        "pyawa-two-providers-close-{}",
        std::process::id()
    ));
    let _ = std::fs::remove_dir_all(&base);
    std::fs::create_dir_all(&base).expect("建临时目录");
    std::fs::write(base.join("close.txt"), "close-me\n").expect("写文件");

    // ---- 真实档 ----
    let real = PosixFs::new();
    let real_table = real.vtable();
    // SAFETY: `real_table` 活到 state 销毁之后。
    let real_state = unsafe { state_with_fs(&real_table) };
    // SAFETY: 只借用；`real_state` 尚未销毁。
    let real_instance = unsafe { &*real_state }.instance();
    let real_path = base.join("close.txt").to_string_lossy().into_owned();
    let handle = real_instance
        .fs_open(real_path.as_bytes(), 0, 0)
        .expect("真实档 open 应当成功");
    assert_eq!(real_instance.fs_read(handle, &mut [0u8; 8]).map(|_| ()), Ok(()));
    assert_eq!(real_instance.fs_close(handle), Ok(()), "第一次 close 成功");
    assert_eq!(
        real_instance.fs_close(handle),
        Ok(()),
        "`CP-17`：真 provider 的 close 必须幂等"
    );
    assert_eq!(
        real_instance.fs_read(handle, &mut [0u8; 8]),
        Err(CapabilityCallError::Machine(host_errno("EBADF"))),
        "`CP-18`：关闭后 read 必须是 EBADF 形状的机器错误"
    );
    // SAFETY: `real_state` 由 `state_with_fs` 交回、尚未销毁。
    assert_eq!(unsafe { pa_destroy(real_state) }, PA_OK);

    // ---- 受限档（同一套断言，证明两档的 `CP-17`／`CP-18` 语义一致）----
    let restricted = RestrictedFs::new(&base, vec![]).expect("建受限 provider");
    let restricted_table = restricted.vtable();
    // SAFETY: `restricted_table` 活到 state 销毁之后。
    let restricted_state = unsafe { state_with_fs(&restricted_table) };
    // SAFETY: 只借用；`restricted_state` 尚未销毁。
    let restricted_instance = unsafe { &*restricted_state }.instance();
    let handle = restricted_instance
        .fs_open(b"close.txt", 0, 0)
        .expect("受限档 open 应当成功");
    assert_eq!(restricted_instance.fs_read(handle, &mut [0u8; 8]).map(|_| ()), Ok(()));
    assert_eq!(restricted_instance.fs_close(handle), Ok(()));
    assert_eq!(
        restricted_instance.fs_close(handle),
        Ok(()),
        "`CP-17`：受限 provider 的 close 同样必须幂等"
    );
    assert_eq!(
        restricted_instance.fs_read(handle, &mut [0u8; 8]),
        Err(CapabilityCallError::Machine(host_errno("EBADF"))),
        "`CP-18`：受限档同样返回 EBADF"
    );
    // SAFETY: `restricted_state` 由 `state_with_fs` 交回、尚未销毁。
    assert_eq!(unsafe { pa_destroy(restricted_state) }, PA_OK);

    let _ = std::fs::remove_dir_all(&base);
}

// --------------------------------------------------------------------------- #
// T-CP-7 后半（`CP-19`）的**缺口探针**：当前红，如实留证据
// --------------------------------------------------------------------------- #

/// **`T-CP-7` 后半的缺口探针**（`CP-19`／`CP-31；**当前红**）。
///
/// 现状：VM 侧**没有**能力表（`fd → 能力对象` 表还在 provider 里）⇒ `Instance`／`pa_destroy`
/// 不知道这个实例开过哪些句柄 ⇒ 实例销毁**不会**关闭它们。本探针打开一个句柄、**不 close**
/// 就销毁实例，然后断言 provider 手里还留着它——**这条断言通过 ＝ 缺口仍在**。
///
/// 标 `#[ignore]`：它表达的是"当前已知的红"，不是验收项。缺口修好后这条会失败 ⇒ 按新语义改成
/// 正当断言（`== 0`）并去掉 `#[ignore]`。
///
/// 跑法：`cargo test -p pyawa-runtime --test two_providers -- --ignored`
#[test]
#[ignore = "`CP-19` 缺口探针：当前红（实例销毁不关闭 provider 句柄；等 `CP-31` 的 VM 侧能力表）"]
fn gap_probe_instance_destroy_leaves_provider_handles_open() {
    let base = std::env::temp_dir().join(format!(
        "pyawa-two-providers-residual-{}",
        std::process::id()
    ));
    let _ = std::fs::remove_dir_all(&base);
    std::fs::create_dir_all(&base).expect("建临时目录");
    std::fs::write(base.join("leak.txt"), "payload\n").expect("写文件");

    let provider = PosixFs::new();
    let table = provider.vtable();
    // SAFETY: `table` 活到 state 销毁之后。
    let state = unsafe { state_with_fs(&table) };
    let path = base.join("leak.txt").to_string_lossy().into_owned();
    // SAFETY: 只借用；`state` 尚未销毁。
    let handle = unsafe { &*state }
        .instance()
        .fs_open(path.as_bytes(), 0, 0)
        .expect("open 应当成功");
    assert!(handle >= 3, "真文件句柄从 3 起（1／2 是标准流）");
    assert_eq!(provider.open_handle_count(), 1, "打开后 provider 里有 1 个句柄");
    // 故意**不** close 就销毁实例：`CP-19` 要求这里兜底释放。
    // SAFETY: `state` 由 `state_with_fs` 交回、尚未销毁。
    assert_eq!(unsafe { pa_destroy(state) }, PA_OK);
    assert!(
        provider.open_handle_count() > 0,
        "缺口已修（实例销毁把句柄关了）⇒ 把这条探针改成 `== 0` 的正当断言并去掉 #[ignore]"
    );
    let _ = std::fs::remove_dir_all(&base);
}

// --------------------------------------------------------------------------- #
// T-CP-3 的**缺口探针**：当前红（如实留证据，不伪造绿）
// --------------------------------------------------------------------------- #

/// **`T-CP-3` 缺口探针**（`CP-14`／`CP-31`；**当前红**）。
///
/// 今天成立的一半：`_io.FileIO.fileno()` 返回的确实是 `int`（`CP-14` 的 `fd` 形状）。
/// 仍红的两处：`os.dup2` 不存在（`CP-31` 的"能力表复制"没落地）、`select.select` 调用如实报
/// `NotImplementedError`（`net` 域的 `poll` 槽 + 真 `select(2)` 未接线）。
///
/// 探针断言的是"**缺口仍在**"（因此通过 ＝ 红仍成立）；修好后这条会失败 ⇒ 改成正当断言并去掉
/// `#[ignore]`。
///
/// 跑法：`cargo test -p pyawa-runtime --test two_providers -- --ignored`
#[test]
#[ignore = "`T-CP-3` 缺口探针：当前红（`os.dup2` 缺、`select.select` 未接线）"]
fn gap_probe_tcp3_fd_int_surface() {
    let fixture = fixture("tcp3");
    let script = r#"import os
import select

print("dup2", hasattr(os, "dup2"))
try:
    select.select([], [], [], 0)
except BaseException as exc:
    print("select", type(exc).__name__)
handle = open("greeting.txt")
print("fileno-int", isinstance(handle.fileno(), int))
handle.close()
print("done")
"#;
    fixture.plant_script(script);
    let real = run(ProviderKind::Real, &fixture.real);
    assert_eq!(real.exit, 0, "探针脚本应当正常跑完：{real:?}");
    assert!(
        real.stdout.contains("dup2 False"),
        "缺口已修（`os.dup2` 有了）⇒ 改成正当断言并去掉 #[ignore]：\n{}",
        real.stdout
    );
    assert!(
        real.stdout.contains("select NotImplementedError"),
        "缺口已修（`select.select` 接线了）⇒ 改成正当断言并去掉 #[ignore]：\n{}",
        real.stdout
    );
    // 已经成立的一半：`fd` 就是 `int`（`CP-14`）——这一条**不**是缺口。
    assert!(
        real.stdout.contains("fileno-int True"),
        "`CP-14`：`fd` 必须是 `int`：\n{}",
        real.stdout
    );
}
