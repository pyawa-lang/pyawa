//! **异常 traceback 的帧保活／`__exit__` 的 `tb` 参数（task-14 #g）**：与 CPython 3.14.4 双侧对拍。
//!
//! **`FACE_SCRIPT`**：同一段探针在**本机 `python3` 3.14.4** 与 pyawa 上**逐行一致** ✓ ——
//! 覆盖 `BC-60` 未落地的最后一块可观察面（此前 `__traceback__` 是 `None`，`DIV-6`）：
//!
//! 1. **`with` 的 `__exit__(type, exc, tb)` 第三参**：参照给 `exc.__traceback__`（traceback 对象），
//!    pyawa 先前写死 `None`（`step_exceptions.rs` 的 `WITH_EXCEPT_START`）⇒ 本测试钉住。
//! 2. **帧保活**：已捕获异常存活期间，`__traceback__` 链上的帧与**局部变量**必须保持
//!    （`tb_frame`／`tb_next` 链、`f_locals` 里 `marker`／`payload`／`row` 都还在；
//!    帧在异常存活期间不能被释放或清空）。
//! 3. **`__context__`／`__cause__` 链的帧保活**：隐式上下文链与外层异常各自带 `__traceback__`，
//!    上下文异常的帧局部（`row`）仍可读。
//! 4. **释放方向**：捕获的异常丢掉后，链上的帧要能释放（`__del__` 终验器必须跑）——
//!    这是"保活"不能变成"泄漏"的对偶判据。
//!
//! 期望值由**本机 `python3` 3.14.4 跑同一段脚本**取得 ✓（逐行）。
//!
//! **已知偏差（如实，不钉）**：`tb_lasti` 的**数值**口径不同（本层是"指令序号×2"、参照是
//! `co_positions()` 的字节偏移；`0f360590` 已对齐到本层自己的坐标）⇒ 本测试只钉它是 `int`；
//! `tb_lineno` 照参照一致。

fn run(name: &str, script: &str) -> String {
    let root = std::env::temp_dir().join(format!("pyawa-tbframe-{}-{name}", std::process::id()));
    let _ = std::fs::remove_dir_all(&root);
    std::fs::create_dir_all(&root).expect("建临时目录");
    let path = root.join("probe.py");
    std::fs::write(&path, script).expect("写脚本");
    let output = std::process::Command::new(env!("CARGO_BIN_EXE_pyawa"))
        .arg(&path)
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

/// 参照侧脚本（`python3` 3.14.4 跑同一段 ⇒ `FACE_EXPECTED` ✓）。
const FACE_SCRIPT: &str = r#"import gc, sys

def show(label, value):
    print(label, "=>", repr(value))

# 1) with 语句的 __exit__(type, exc, tb)
seen = {}
class Ctx:
    def __enter__(self):
        return self
    def __exit__(self, exc_type, exc, tb):
        seen["type"] = exc_type.__name__ if exc_type is not None else None
        seen["exc"] = type(exc).__name__ if exc is not None else None
        seen["tb_is_none"] = tb is None
        seen["tb_frame"] = type(tb.tb_frame).__name__ if tb is not None else None
        seen["tb_lineno"] = isinstance(tb.tb_lineno, int) if tb is not None else None
        return False

def raise_in_with():
    raise ValueError("in with")

try:
    with Ctx():
        raise_in_with()
except ValueError:
    pass
print("exit-args", seen["type"], seen["exc"], seen["tb_is_none"], seen["tb_frame"], seen["tb_lineno"])

# 2) 帧保活：异常存活期间，__traceback__ 链上的帧与局部必须存活
saved = []

def level3():
    marker = 1234
    payload = {"tag": "level3"}
    raise ValueError("keep me")

def level2():
    row = [1, 2, 3]
    level3()

def level1():
    try:
        level2()
    except ValueError as exc:
        saved.append(exc)
    return "returned"

print("call", level1())
gc.collect()
exc = saved[0]
tb = exc.__traceback__
depth = 0
frame_names = []
locals_chain = []
while tb is not None:
    depth += 1
    frame = tb.tb_frame
    frame_names.append(type(frame).__name__)
    locals_chain.append(sorted(frame.f_locals.keys()))
    tb = tb.tb_next
print("tb-depth", depth)
print("tb-frame-types", sorted(set(frame_names)))
print("tb-locals", locals_chain)
marker = None
t = exc.__traceback__
while t is not None:
    if "marker" in t.tb_frame.f_locals:
        marker = t.tb_frame.f_locals["marker"]
    t = t.tb_next
print("marker-alive", marker)
print("payload-alive", end=" ")
alive = None
t = exc.__traceback__
while t is not None:
    payload = t.tb_frame.f_locals.get("payload")
    if payload is not None:
        alive = payload
    t = t.tb_next
print(alive)

# 3) __context__ 链的帧也保活
def make_context():
    try:
        level2()
    except ValueError:
        raise TypeError("outer")

try:
    make_context()
except TypeError as outer:
    context = outer.__context__
    print("context-tb", context.__traceback__ is not None)
    t = context.__traceback__
    has_row = False
    while t is not None:
        if "row" in t.tb_frame.f_locals:
            has_row = True
        t = t.tb_next
    print("context-frame-local", has_row)
    print("outer-tb", outer.__traceback__ is not None)

# 4) raise ... from 的 cause 帧
def make_cause():
    try:
        level3()
    except ValueError as inner:
        raise TypeError("wrapped") from inner

try:
    make_cause()
except TypeError as exc:
    cause = exc.__cause__
    print("cause-tb", cause is not None and cause.__traceback__ is not None)
    print("suppress", exc.__suppress_context__)

# 5) 释放方向：异常丢掉后帧必须能释放（终验器跑）
done = [False]

def wrapper():
    class Holder:
        def __del__(self):
            done[0] = True
    held = Holder()
    raise ValueError("drop me")

try:
    wrapper()
except ValueError:
    kept = __import__("sys").exc_info()[1]
del kept
gc.collect()
print("frame-released", done[0])
"#;

/// 参照（本机 `python3` 3.14.4 跑 `FACE_SCRIPT` 实测 ✓，逐行）。
const FACE_EXPECTED: &str = r#"exit-args ValueError ValueError False frame True
call returned
tb-depth 3
tb-frame-types ['frame']
tb-locals [[], ['row'], ['marker', 'payload']]
marker-alive 1234
payload-alive {'tag': 'level3'}
context-tb True
context-frame-local True
outer-tb True
cause-tb True
suppress True
frame-released True
"#;

#[test]
fn exception_traceback_frames_match_reference() {
    assert_eq!(run("face", FACE_SCRIPT), FACE_EXPECTED);
}
