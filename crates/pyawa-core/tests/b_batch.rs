//! **扩展特性 B 批**：`BC-62` 多语句 `lambda` body ＋ `BC-63` 空安全操作符 ＋ `TS-49` 类型规则。
//!
//! 验收口径（`docs/SPEC-bytecode.md` 的 `BC-62`／`BC-63`、`docs/SPEC-type-system.md` 的 `TS-49`）：
//!
//! - **`BC-62`**：扩展模式（`.pyawa`）下 `lambda` body 允许 `( 语句… 末表达式 )`；同一 body 写成
//!   `def` 版本（`.py` 下合法）后**逐项对拍**（扩展模式没有参照实现，`MS-13` ③）；单行表达式形态
//!   **产物不变**（与纯 Python 模式逐相同）；纯 Python 模式的新形态**必须** `Syntax`（`BC-15`）。
//! - **`BC-63`**：`?.`／`??`／`??=`／`?[` 四种，逐条与规格里的**等价纯 Python 写法**对拍
//!   （只求值一次、惰性）；纯 Python 模式四种写法**必须** `Syntax`。
//! - **`TS-49`**：`A?` 上 `?.b` ⇒ `B?`；`A`（非可空）⇒ `B`；`??` 取相容上界、任一侧平台类型 ⇒
//!   平台类型；`??=` 不改变标注类型。
//!
//! **运行期自检脚本的口径**：本文件用 `Vm::new()` 在**进程内**跑扩展模式模块（与
//! `tests/assignment_emission.rs` 同一手法）。这套脚手架**没有装 `builtins`**（`P3-14` 未落地）
//! ⇒ 脚本里**不用任何内建函数／异常**：对不上时写一个**未绑定名字**（`_bc62_xxx`）当表达式语句，
//! 自然抛 `NameError` 把检查点带进失败消息；计数用普通变量，不调 `len()`。
//! 带 `print`／`sorted`／`assert` 的端到端对拍在 `crates/pyawa-runtime/tests/b_batch_shapes.rs`
//! （真实 CLI，`.pyawa` vs `.py` def 版逐行比 stdout）。

mod common;

use pyawa_core::compile::{compile, instantiate, CheckTier, CompileError, CompiledUnit, Mode};
use pyawa_core::{ExecError, Frame};

use common::Vm;

/// 扩展模式编译（浅层档位、优化级 0：与本批的语义无关，固定死以消掉无关变量）。
fn extension(source: &str) -> CompiledUnit {
    compile(source, "probe.pyawa", Mode::Extension, CheckTier::Shallow, 0)
        .unwrap_or_else(|error| panic!("扩展模式应当编得过 {source:?}，却报了 {error:?}"))
}

/// 纯 Python 模式编译。
fn pure_python(source: &str) -> Result<CompiledUnit, CompileError> {
    compile(source, "probe.py", Mode::PurePython, CheckTier::Shallow, 0)
}

/// 跑一个模块；失败时把**待处理异常**的类型／消息带进 panic（便于定位脚本里哪条检查没过）。
fn run(source: &str, mode: Mode) -> Result<(), String> {
    let vm = Vm::new();
    let unit = compile(source, "probe", mode, CheckTier::Shallow, 0)
        .map_err(|error| format!("编译失败：{error:?}"))?;
    let code = instantiate(&vm.instance, &unit);
    let namespace = vm.instance.new_dict();
    // 类体序言会读 `__name__`（`LOAD_NAME __name__; STORE_NAME __module__`）——与 `tests/compiled_class.rs`
    // 同一手法：把模块名补进命名空间，否则 `class` 一出现就 `NameError`。
    let module_name = vm.instance.new_str("__main__");
    vm.instance.dict_set(namespace, "__name__", module_name);
    // `Frame::for_code_with_namespace` **接手**一份命名空间引用 ⇒ 先补一份（`OM-16`）
    // SAFETY: namespace 由本测试持有。
    unsafe { vm.instance.incref_object(namespace.as_ptr()) };
    let frame = Frame::for_code_with_namespace(vm.frame_type, &code, namespace);
    let frame = vm.instance.alloc(frame);
    match pyawa_core::execute(&vm.instance, &frame) {
        Ok(_) => Ok(()),
        Err(error) => {
            let pending = vm
                .pending_exception()
                .map(|(name, message)| format!("{name}: {message:?}"))
                .unwrap_or_else(|| "（没有异常状态）".to_owned());
            let kind = match error {
                ExecError::Raised { .. } => "Raised",
                other => {
                    return Err(format!("执行器内部错误 {other:?}（不是 Python 异常）"));
                }
            };
            Err(format!("{kind} / {pending}"))
        }
    }
}

/// 跑扩展模式脚本并断言**跑完**（脚本自带"未绑定名字"自检）。
fn run_extension(source: &str) {
    if let Err(message) = run(source, Mode::Extension) {
        panic!("扩展模式脚本应当跑完，却报了：{message}\n--- 源码 ---\n{source}");
    }
}

/// 跑纯 Python 模式脚本并断言**跑完**（`BC-63` 的等价写法对拍用）。
fn run_pure(source: &str) {
    if let Err(message) = run(source, Mode::PurePython) {
        panic!("纯 Python 模式脚本应当跑完，却报了：{message}\n--- 源码 ---\n{source}");
    }
}

/// 跑扩展模式脚本并断言**抛指定异常**。
fn expect_extension_raise(source: &str, type_name: &str) {
    let message = run(source, Mode::Extension).expect_err("这段应当抛异常");
    assert!(
        message.contains("Raised") && message.contains(type_name),
        "期望 {type_name}，实际：{message}"
    );
}

/// 断言扩展模式报**编译期类型错**（`TS-47`／`TS-49`），返回消息。
fn expect_type_assignment(source: &str) -> String {
    match compile(source, "probe.pyawa", Mode::Extension, CheckTier::Shallow, 0) {
        Err(CompileError::TypeAssignment(message)) => message,
        other => panic!(
            "能静态判定的不符必须在编译期报 TypeAssignment，实际 {:?}\n源码：{source:?}",
            other.map(|_| ())
        ),
    }
}

/// 断言扩展模式**编得过**（`TS-48` 平台类型／合法赋值的放行面）。
fn expect_extension_compiles(source: &str) {
    if let Err(error) = compile(source, "probe.pyawa", Mode::Extension, CheckTier::Shallow, 0) {
        panic!("扩展模式应当编得过 {source:?}，却报了 {error:?}");
    }
}

/// 断言纯 Python 模式下这段是 **`Syntax`**（`BC-15`：扩展语法在 `.py` 下必须拒绝）。
fn expect_pure_syntax(source: &str) {
    match pure_python(source) {
        Err(CompileError::Syntax(_)) => {}
        other => panic!(
            "纯 Python 模式下扩展语法必须报 Syntax，实际 {:?}\n源码：{source:?}",
            other.map(|_| ())
        ),
    }
}

// =========================================================================== #
// BC-62：多语句 lambda body
// =========================================================================== #

/// **`BC-62` ①**：规格示例里的四种形态，扩展模式 lambda 与**同 body 的 def 版**逐项对拍。
///
/// 扩展模式没有参照实现（`MS-13` ③）⇒ 对拍物是 `.py` 下合法的 `def` 版本；两边的期望值一致
/// （端到端 `sorted(key=…)` 的版本在 `crates/pyawa-runtime/tests/b_batch_shapes.rs` 里跑真实 CLI）。
#[test]
fn bc62_multi_statement_lambda_matches_the_def_version() {
    // ① 规格第一个示例的形状（当 key 用；这里直接调 key，`sorted` 留给 runtime e2e）。
    let lambda_form = r#"names = ["  Bob", "Alice ", "bob"]
key = lambda s: (
    t = s.strip()
    t.lower()
)
if key(names[0]) != "bob": _bc62_key_0
if key(names[1]) != "alice": _bc62_key_1
"#;
    let def_form = r#"names = ["  Bob", "Alice ", "bob"]
def key(s):
    t = s.strip()
    return t.lower()
if key(names[0]) != "bob": _bc62_key_0
if key(names[1]) != "alice": _bc62_key_1
"#;
    run_extension(lambda_form);
    run_pure(def_form);

    // ② 同一行：括号内的 `;` 与换行都白送（规格示例 `print((lambda x: (y = x * 2; y + 1))(10))`）。
    let lambda_form = "f = lambda x: (y = x * 2; y + 1)\nresult = f(10)\nif result != 21: _bc62_inline\n";
    let def_form = "def f(x):\n    y = x * 2\n    return y + 1\nresult = f(10)\nif result != 21: _bc62_inline\n";
    run_extension(lambda_form);
    run_pure(def_form);

    // ③ 放进表里。
    let lambda_form = r#"handlers = {"inc": lambda v: (
    n = v + 1
    n
)}
result = handlers["inc"](10)
if result != 11: _bc62_table
"#;
    let def_form = r#"def inc(v):
    n = v + 1
    return n
handlers = {"inc": inc}
result = handlers["inc"](10)
if result != 11: _bc62_table
"#;
    run_extension(lambda_form);
    run_pure(def_form);

    // ④ 无末表达式 ⇒ None。
    let lambda_form = "f = lambda x: (y = x + 1)\nresult = f(1)\nif result is not None: _bc62_none\n";
    let def_form = "def f(x):\n    y = x + 1\nresult = f(1)\nif result is not None: _bc62_none\n";
    run_extension(lambda_form);
    run_pure(def_form);

    // ⑤ 闭包：body 里用外层函数的局部（自由变量分析走 lambda 的体，含语句形态）。
    let lambda_form = r#"def make(k):
    return lambda v: (
        n = v + k
        n * 2
    )
f = make(3)
result = f(4)
if result != 14: _bc62_closure
"#;
    let def_form = r#"def make(k):
    def f(v):
        n = v + k
        return n * 2
    return f
f = make(3)
result = f(4)
if result != 14: _bc62_closure
"#;
    run_extension(lambda_form);
    run_pure(def_form);
}

/// **`BC-62` ②**：赋值在 lambda 体内是**自己的局部**（不能把外层同名局部写坏）。
#[test]
fn bc62_body_assignment_is_local_to_the_lambda() {
    let source = r#"def outer():
    x = 100
    f = lambda v: (
        x = v + 1
        x
    )
    return f(1), x
result = outer()
if result[0] != 2: _bc62_local
if result[1] != 100: _bc62_local_outer
"#;
    run_extension(source);
}

/// **`BC-62` ②b**：`pass`（无末表达式 ⇒ `None`）与**增强赋值**也是简单语句（`BC-62` 的"简单语句序列"）。
#[test]
fn bc62_pass_and_augmented_assignment_are_simple_statements() {
    run_extension(
        "f = lambda x: (pass)\nresult = f(1)\nif result is not None: _bc62_pass\n",
    );
    run_extension(
        "f = lambda x: (y = x; y += 1; y)\nresult = f(10)\nif result != 11: _bc62_augassign\n",
    );
}

/// **`BC-62` ③**：**单行表达式形态行为完全不变**——扩展模式与纯 Python 模式的产物相同
/// （`BC-62` 的"增量仅为 body 由 expression 扩充"＋ `TS-5` 的"既有构造读法不变"）。
#[test]
fn bc62_single_line_expression_form_is_unchanged() {
    let sources = [
        "f = lambda x: x + 1\n",
        "result = (lambda a, b: a * b)(2, 3)\n",
        "f = lambda s: s.strip().lower()\n",
        "handlers = {\"inc\": lambda v: v + 1}\n",
        "result = (lambda v: -v)(3)\n",
    ];
    for source in sources {
        let extension_unit = extension(source);
        let pure_unit = pure_python(source).expect("单行 lambda 在纯 Python 模式合法");
        assert_eq!(
            format!("{extension_unit:?}"),
            format!("{pure_unit:?}"),
            "单行表达式形态在扩展模式必须与纯 Python 模式产物一致：{source:?}"
        );
    }
}

/// **`BC-62` ④**：纯 Python 模式（`.py`）下新形态**必须**报 `SyntaxError`（`BC-15`）。
#[test]
fn bc62_new_form_is_syntax_error_in_pure_python() {
    let sources = [
        "f = lambda x: (y = x; y + 1)\n",
        "f = lambda x: (y = x)\n",
        "f = lambda x: (\n    y = x\n    y\n)\n",
        "f = lambda x: (y = x * 2; y + 1)\n",
        "print((lambda x: (y = x; y))(1))\n",
    ];
    for source in sources {
        expect_pure_syntax(source);
    }
}

/// **`BC-62` ⑤**：`( ... )` 里的**多行普通表达式**照旧是表达式（不因为体里出现物理换行就误当语句块）。
#[test]
fn bc62_multiline_expression_stays_an_expression() {
    let source = "f = lambda a, b: (a +\n    b)\n";
    assert_eq!(
        format!("{:?}", extension(source)),
        format!("{:?}", pure_python(source).expect("合法")),
        "跨行表达式形态必须仍走表达式通道"
    );
    run_extension(
        "f = lambda a, b: (a +\n    b)\nresult = f(2, 3)\nif result != 5: _bc62_multiline\n",
    );
}

// =========================================================================== #
// BC-63：空安全操作符（运行期，逐条与"等价纯 Python 写法"对拍）
// =========================================================================== #

/// **`BC-63` ① `a?.b`／`a?[i]`**：与规格的等价写法表逐条对拍（有值／`None`）。
#[test]
fn bc63_safe_access_and_subscript_match_the_equivalent_pure_python() {
    // `a?.b`
    let extension_form = r#"class C:
    def __init__(self, value):
        self.b = value
c = C(5)
r1 = c?.b
n = None
r2 = n?.b
if r1 != 5: _bc63_attr_value
if r2 is not None: _bc63_attr_none
"#;
    let pure_form = r#"class C:
    def __init__(self, value):
        self.b = value
c = C(5)
r1 = (lambda _t: _t.b if _t is not None else None)(c)
n = None
r2 = (lambda _t: _t.b if _t is not None else None)(n)
if r1 != 5: _bc63_attr_value
if r2 is not None: _bc63_attr_none
"#;
    run_extension(extension_form);
    run_pure(pure_form);

    // `a?[i]`（列表／字符串／`None` 三档）
    let extension_form = r#"xs = [10, 20]
s = "abc"
n = None
r1 = xs?[1]
r2 = s?[2]
r3 = n?[0]
if r1 != 20: _bc63_sub_list
if r2 != "c": _bc63_sub_str
if r3 is not None: _bc63_sub_none
"#;
    let pure_form = r#"xs = [10, 20]
s = "abc"
n = None
r1 = (lambda _t: _t[1] if _t is not None else None)(xs)
r2 = (lambda _t: _t[2] if _t is not None else None)(s)
r3 = (lambda _t: _t[0] if _t is not None else None)(n)
if r1 != 20: _bc63_sub_list
if r2 != "c": _bc63_sub_str
if r3 is not None: _bc63_sub_none
"#;
    run_extension(extension_form);
    run_pure(pure_form);
}

/// **`BC-63` ② `a?.b(c)`**：`a` 为 `None` ⇒ **实参不求值**（等价写法把调用放进条件分支 ⇒ 惰性）。
#[test]
fn bc63_safe_call_does_not_evaluate_arguments_when_none() {
    let extension_form = r#"log = [0]
def side(v):
    log[0] = v
    return v
class C:
    def b(self, x):
        return x + 1
c = C()
none = None
r1 = c?.b(side(4))
r2 = none?.b(side(9))
if r1 != 5: _bc63_call_value
if r2 is not None: _bc63_call_none
if log[0] != 4: _bc63_call_lazy
"#;
    let pure_form = r#"log = [0]
def side(v):
    log[0] = v
    return v
class C:
    def b(self, x):
        return x + 1
c = C()
none = None
r1 = (lambda _t: _t.b(side(4)) if _t is not None else None)(c)
r2 = (lambda _t: _t.b(side(9)) if _t is not None else None)(none)
if r1 != 5: _bc63_call_value
if r2 is not None: _bc63_call_none
if log[0] != 4: _bc63_call_lazy
"#;
    run_extension(extension_form);
    run_pure(pure_form);
}

/// **`BC-63` ③ `?.` 的"整条后缀链短路"**：`a?.b.c` 在 `a` 为 `None` 时结果是 `None`
/// （不是 `None.c` 的 `AttributeError`），且链上后续后缀**不再求值**。
#[test]
fn bc63_safe_access_short_circuits_the_whole_suffix_chain() {
    let source = r#"class B:
    def __init__(self):
        self.c = 7
class A:
    def __init__(self):
        self.b = B()
a = A()
none = None
r1 = a?.b.c
r2 = none?.b.c
if r1 != 7: _bc63_chain_value
if r2 is not None: _bc63_chain_none

# 规格示例：一路空安全 ＋ 末尾缺省
state = [0, 0]
def side(v):
    state[0] = state[0] + 1
    state[1] = v
    return v
class Row:
    def __init__(self, value):
        self.value = value
class Table:
    def __init__(self, row):
        self._row = row
    def row(self, k):
        state[0] = state[0] + 1
        state[1] = k
        return self._row
table = Table(Row("x"))
r3 = table?.row(1)?.value ?? "缺省"
empty = None
r4 = empty?.row(side(2))?.value ?? "缺省"
missing = Table(None)
r5 = missing?.row(3)?.value ?? "缺省"
if r3 != "x": _bc63_chain_default_value
if r4 != "缺省": _bc63_chain_default_none
if r5 != "缺省": _bc63_chain_default_missing
# `empty` 为 None ⇒ 实参 side(2) 不求值；总共只该有 row(1)、row(3) 两次调用
if state[0] != 2: _bc63_chain_lazy
if state[1] != 3: _bc63_chain_lazy_last
"#;
    run_extension(source);
}

/// **`BC-63` ④ 属性不存在仍是 `AttributeError`**（`?.` 只吞 `None`，不吞错）。
#[test]
fn bc63_missing_attribute_is_not_swallowed() {
    expect_extension_raise(
        "class C:\n    pass\nc = C()\nr = c?.missing\n",
        "AttributeError",
    );
}

/// **`BC-63` ⑤ `a ?? b`**：只判 `None`（`0`／`''`／`[]` 都算有值）、右结合、低于 `or`／`and`、
/// 左侧**只求值一次**、右侧**惰性**；与规格等价写法对拍。
#[test]
fn bc63_null_coalesce_matches_the_equivalent_pure_python() {
    // 只判 `None`：`0`／`''`／空列表都算"有值"。
    run_extension(
        r#"r1 = 0 ?? 5
r2 = "" ?? "x"
r3 = None ?? 5
if r1 != 0: _bc63_coalesce_zero
if r2 != "": _bc63_coalesce_empty
if r3 != 5: _bc63_coalesce_none
"#,
    );
    // 只求值一次 ＋ 右侧惰性（等价写法：`(lambda _t: _t if _t is not None else b)(a)`）。
    let extension_form = r#"state = [0, 0]
def side(name, value):
    state[0] = state[0] + 1
    state[1] = name
    return value
r1 = side(1, 3) ?? side(2, 4)
r2 = side(3, None) ?? side(4, 6)
if r1 != 3: _bc63_coalesce_value
if r2 != 6: _bc63_coalesce_default
if state[0] != 3: _bc63_coalesce_once
if state[1] != 4: _bc63_coalesce_order
"#;
    let pure_form = r#"state = [0, 0]
def side(name, value):
    state[0] = state[0] + 1
    state[1] = name
    return value
r1 = (lambda _t: _t if _t is not None else side(2, 4))(side(1, 3))
r2 = (lambda _t: _t if _t is not None else side(4, 6))(side(3, None))
if r1 != 3: _bc63_coalesce_value
if r2 != 6: _bc63_coalesce_default
if state[0] != 3: _bc63_coalesce_once
if state[1] != 4: _bc63_coalesce_order
"#;
    run_extension(extension_form);
    run_pure(pure_form);
    // 右结合／优先级：`??` 低于 `or`／`and` ⇒ `None or 0 ?? 5` ＝ `(None or 0) ?? 5` ＝ 0；
    // `None ?? 0 or 7` 的右侧是 `0 or 7` ⇒ 7。
    run_extension(
        r#"r1 = None or 0 ?? 5
r2 = None ?? 0 or 7
r3 = None ?? None ?? 9
if r1 != 0: _bc63_coalesce_precedence_or
if r2 != 7: _bc63_coalesce_precedence_right
if r3 != 9: _bc63_coalesce_right_assoc
"#,
    );
}

/// **`BC-63` ⑥ `a ??= b`**：只在 `a` 为 `None` 时赋值（等价写法 `if a is None: a = b`），
/// 右侧惰性、`a` 只读一次。
#[test]
fn bc63_null_coalesce_assignment_matches_the_equivalent_pure_python() {
    let extension_form = r#"state = [0, 0]
def side(name, value):
    state[0] = state[0] + 1
    state[1] = name
    return value
x = None
x ??= side(1, 1)
y = 5
y ??= side(2, 2)
z = None
z ??= None
if x != 1: _bc63_nullassign_x
if y != 5: _bc63_nullassign_y
if z is not None: _bc63_nullassign_z
if state[0] != 1: _bc63_nullassign_lazy
if state[1] != 1: _bc63_nullassign_order
"#;
    let pure_form = r#"state = [0, 0]
def side(name, value):
    state[0] = state[0] + 1
    state[1] = name
    return value
x = None
if x is None: x = side(1, 1)
y = 5
if y is None: y = side(2, 2)
z = None
if z is None: z = None
if x != 1: _bc63_nullassign_x
if y != 5: _bc63_nullassign_y
if z is not None: _bc63_nullassign_z
if state[0] != 1: _bc63_nullassign_lazy
if state[1] != 1: _bc63_nullassign_order
"#;
    run_extension(extension_form);
    run_pure(pure_form);
    // 函数局部与模块级都走同一个发射口径。
    run_extension(
        "def f():\n    x = None\n    x ??= 4\n    return x\nresult = f()\nif result != 4: _bc63_nullassign_local\n",
    );
}

/// **`BC-63` ⑦**：纯 Python 模式（`.py`）下四种写法**必须**报 `SyntaxError`（`BC-15`）。
#[test]
fn bc63_forms_are_syntax_error_in_pure_python() {
    let sources = [
        "a = obj?.b\n",
        "a = obj?.b(1)\n",
        "a = obj?.b.c\n",
        "a = xs?[0]\n",
        "a = x ?? y\n",
        "x ??= 1\n",
        "d = table?.row(k)?.value ?? \"缺省\"\n",
    ];
    for source in sources {
        expect_pure_syntax(source);
    }
}

/// **扩展模式下的畸形输入**：报编译错误（不是 panic／越界）。
#[test]
fn malformed_batch_extension_syntax_is_reported_not_a_panic() {
    let sources = [
        "a = obj?.\n",
        "a = obj?\n",
        "a = obj?[\n",
        "a = obj?[0\n",
        "a = x ??\n",
        "f = lambda x: (y = )\n",
        "f = lambda x: (\n",
    ];
    for source in sources {
        let result = compile(source, "probe.pyawa", Mode::Extension, CheckTier::Shallow, 0);
        assert!(
            matches!(
                result,
                Err(CompileError::Syntax(_))
                    | Err(CompileError::Unsupported(_))
                    | Err(CompileError::TypeAssignment(_))
            ),
            "畸形输入应报编译错误（不 panic）：{source:?} ⇒ {:?}",
            result.map(|_| ())
        );
    }
}

// =========================================================================== #
// TS-49：空安全操作符的类型规则
// =========================================================================== #

/// `TS-49`：`A?` 上 `?.b`（`b: B`）⇒ `B?`；`A`（非可空）⇒ `B`。
#[test]
fn ts49_safe_access_nullability_follows_the_receiver_annotation() {
    const CLASS: &str = "class Box:\n    value: int\n\n";
    // 可空接收者 ⇒ 结果 `int?` ⇒ 赋给 `int` 标注的变量是静态可判定的不符。
    let source =
        format!("{CLASS}def f(b: Box?) -> None:\n    y: int = b?.value\n    return None\n");
    let message = expect_type_assignment(&source);
    assert!(
        message.contains("`y`") && message.contains("got int | None"),
        "`A?` 上的 `?.b` 结果必须是 `B?`：{message}"
    );
    // 非可空接收者 ⇒ 结果 `B`（静态即可断言不为 None）⇒ 赋给 `int` 合法。
    expect_extension_compiles(&format!(
        "{CLASS}def f(b: Box) -> None:\n    y: int = b?.value\n    return None\n"
    ));
    // 属性**没有标注** ⇒ 平台类型（`TS-48`）⇒ 放行（判不出，交给运行期）。
    expect_extension_compiles(&format!(
        "{CLASS}def f(b: Box?) -> None:\n    y: int = b?.untyped\n    return None\n"
    ));
    // `A?` 上的 `?.b` 结果赋给**可空**标注合法（`int?` 接受 `int | None`）。
    expect_extension_compiles(&format!(
        "{CLASS}def f(b: Box?) -> None:\n    y: int? = b?.value\n    return None\n"
    ));
}

/// `TS-49`：`a ?? b` 取**相容上界**；任一侧是平台类型 ⇒ 结果平台类型；`TS-50` 收窄之后
/// `x ?? d` 在编译期即可判**恒取左值**（非可空）。
#[test]
fn ts49_null_coalesce_takes_the_compatible_upper_bound_and_narrows() {
    // 未收窄：`int? ?? int` 的上界含 `None`（`int?`）⇒ 赋给 `int` 是静态不符。
    let message =
        expect_type_assignment("def f(x: int?) -> None:\n    y: int = x ?? 0\n    return None\n");
    assert!(
        message.contains("got int | None"),
        "`int? ?? int` 的相容上界是 `int?`：{message}"
    );
    // 收窄之后（`if x is None: return` 让 None 那一支走掉）⇒ `x` 是 `int`（非可空）⇒ 结果 `int`。
    expect_extension_compiles(
        "def f(x: int?) -> None:\n    if x is None:\n        return None\n    y: int = x ?? 0\n    return None\n",
    );
    // 平台类型参与 ⇒ 结果平台类型（判不出，放行）。
    expect_extension_compiles("def f(x) -> None:\n    y: int = x ?? \"s\"\n    return None\n");
    expect_extension_compiles(
        "def f(x: int?) -> None:\n    y: int = x ?? hidden()\n    return None\n",
    );
    // `A`（非可空）上的安全链结果 ⇒ `B`；链尾 `??` 判恒取左值 ⇒ 整体仍是 `B`。
    const CLASS: &str = "class Box:\n    value: int\n\n";
    expect_extension_compiles(&format!(
        "{CLASS}def f(b: Box) -> None:\n    y: int = b?.value ?? 0\n    return None\n"
    ));
}

/// `TS-49`：`?[i]` 的元素类型来自已知容器标注（`list[T]` ⇒ `T`、可空接收者 ⇒ 结果可空）。
#[test]
fn ts49_safe_subscript_element_type() {
    // 可空容器 ⇒ 元素可空（`int?`）⇒ 赋给 `int` 是静态不符。
    let message = expect_type_assignment(
        "def f(xs: list[int]?) -> None:\n    y: int = xs?[0]\n    return None\n",
    );
    assert!(
        message.contains("got int | None"),
        "`list[int]?` 的 `?[0]` 结果必须是 `int?`：{message}"
    );
    // 非可空容器 ⇒ `int`。
    expect_extension_compiles(
        "def f(xs: list[int]) -> None:\n    y: int = xs?[0]\n    return None\n",
    );
}

/// `TS-49`：`a ??= b` **不改变 `a` 的标注类型**——赋值本身仍按 `TS-47` 查，标注沿用原类型。
#[test]
fn ts49_null_coalesce_assignment_keeps_the_annotation() {
    // 合法：`int` 赋给 `int?`（`??=` 不把标注收窄成 `int`）。
    expect_extension_compiles("def f(x: int?) -> None:\n    x ??= 0\n    return None\n");
    // 标注不变 ⇒ 之后 `y: int = x` 仍按 `int?` 判 ⇒ 静态不符。
    let message = expect_type_assignment(
        "def f(x: int?) -> None:\n    x ??= 0\n    y: int = x\n    return None\n",
    );
    assert!(
        message.contains("got int | None"),
        "`??=` 不改变标注类型（仍是 `int?`）：{message}"
    );
    // `??=` 的右值仍按 `TS-47` 查：`str` 赋给 `int?` 是静态不符。
    let message =
        expect_type_assignment("def f(x: int?) -> None:\n    x ??= \"s\"\n    return None\n");
    assert!(
        message.contains("got str"),
        "`??=` 的右值也要按 `TS-47` 检查：{message}"
    );
}
