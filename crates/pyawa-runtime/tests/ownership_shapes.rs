//! **值语义的引用计数 ＋ 终结器语义**（`P3-41` round 43）。
//!
//! round 43 用「对拍 refcount 增量」量出四类**每次操作漏一份引用**的真 bug（此前一直被
//! `frame_clear` 不漏形参的旧模型掩盖／放大）：
//! * **名字加载**（`LOAD_NAME`／`LOAD_GLOBAL`）：`lookup_in_mapping` 本已 `incref` 交"新引用"，
//!   而调用点一律 `push(...)`（`push` 自己再 `own()` 一次）⇒ 每次名字加载漏 1 ⇒ `c = a` 循环
//!   100 次 `a` 的 refcount **+100**（`test_weakref` 的 70000 个键值全留在堆上、GC 日志
//!   `freed≈0`、`live` 涨到 66.7 万 ⇒ 单次回收 372 ms ⇒ 超时）。改为**借用**语义 ✓；
//! * **方法接收者**：`CALL` 的 `self_or_null` 槽是借用，`call_callable` 契约"借用" ⇒ 调用完要还，
//!   先前漏还 ⇒ `a.m()` 每调一次 `a` 的 refcount +1 ✓；
//! * **返回值**：返回值是**新引用**，`push(instance, …)` 又 `own()` 一次 ⇒ 每调用漏一份；
//!   改 `Frame::push`（接管语义）直接收 ✓；
//! * `__call__` 转发**借用**的 `self` 当实参、`CALL_FUNCTION_EX` 漏还 `bound_self` ✓。
//!
//! 另修**终结器语义**（照参照 `tp_finalize`）：① 终结窗口里**临时自持一份**（`__del__` 的帧
//! 持有 `self` 形参，返回时会 decref ⇒ 之前计数在终结器返回前回到 0 ⇒ 内层 `release_one`
//! 重入、把正在终结的对象释放掉 ✗）；② **每个对象至多终结一次**（复活后不再跑 ✓，用
//! `finalized` 旁表记账，`OM-7` 的保留位不占 ✓）；③ 清引用**前**先摘出 GC 链表（照
//! `PyObject_GC_UnTrack` ✓，防 `__del__` 里的 `gc.collect()` 把 0 计数的自己再 `clear` 一次 ✗）。
//!
//! **如实未修（下一轮）** ✗：**位置形参**仍漏（`f(a)` 每调用 `a` 的 refcount +1）——那要动
//! "帧的形参槽所有权"模型；本轮试过（`frame_clear` 释放形参槽）⇒ refcount 全归零 ✓ 但在
//! `__del__` ＋ `gc.collect()` 的用例上暴露潜伏 UAF（`test_dict`／`test_weakref` SIGABRT/SEGV；
//! gdb：`release_object ← instance_attribute_set`）⇒ **已撤回**，留作独立一轮（需要同时审计
//! 属性写／终结器／异常回溯三条重入路径）。
//!
//! 前 4 行与 `python3` 3.14.4 逐字节一致（`MS-17`）；`finalized` 与 `done` 两行照参照语义钉住。

fn run(name: &str, script: &str) -> String {
    let root = std::env::temp_dir().join(format!("pyawa-own-{}-{}", std::process::id(), name));
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

/// `getrefcount` 增量（引用计数不能漏；与 python3 逐字节一致）＋ 终结器只跑一次。
const OWNERSHIP_SCRIPT: &str = r#"import gc
import sys

class A:
    pass

a = A()
f = len
g = A

def delta(label, value, body):
    before = sys.getrefcount(value)
    body()
    print(label, sys.getrefcount(value) - before)

def attr_read():
    for i in range(100):
        x = a.__class__

def name_load():
    for i in range(100):
        x = g

def builtin_call():
    for i in range(100):
        f([1])

def plain_fn_call():
    for i in range(100):
        abs(i)

delta("attr-read", a, attr_read)
delta("name-load", g, name_load)
delta("builtin-call", f, builtin_call)
delta("plain-fn", abs, plain_fn_call)

# 终结器只跑一次（即便复活）
class D:
    count = 0
    def __del__(self):
        D.count += 1
        globals()["keep"] = self

d = D()
keep = None
del d
gc.collect()
d2 = None
del keep
gc.collect()
print("finalized", D.count)"#;

const OWNERSHIP_EXPECTED: &str = "attr-read 0\nname-load 0\nbuiltin-call 0\nplain-fn 0\nfinalized 1\n";

/// `__del__` 里"删同一 dict 的键 ＋ `gc.collect()`"：不崩、不死循环（参照同款语义）。
const EVIL_SCRIPT: &str = r#"import gc

class EvilAttr:
    def __init__(self, d):
        self.d = d
    def __del__(self):
        if 'attr' in self.d:
            del self.d['attr']
        gc.collect()

class Obj:
    pass

obj = Obj()
obj.__dict__ = {}
for _ in range(10):
    obj.attr = EvilAttr(obj.__dict__)
print("done")"#;

const EVIL_EXPECTED: &str = "done\n";

#[test]
fn ownership_and_finalizer_semantics_match_the_reference() {
    assert_eq!(run("own", OWNERSHIP_SCRIPT), OWNERSHIP_EXPECTED);
    assert_eq!(run("evil", EVIL_SCRIPT), EVIL_EXPECTED);
}
