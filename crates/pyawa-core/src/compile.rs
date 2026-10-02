//! **源码 → 字节码**（`P1-10`；契约 `BC-14`…`BC-18`，族级映射见 `SPEC-bytecode.md` §11）。
//!
//! 词法 → 语法 → 发射；产物是**纯数据**（[`CompiledUnit`]：不带实例／路径／时间 ⇒
//! `BC-16` 的纯函数性结构上成立），过 `validate`（`BC-36` 的零填充缓存槽也照做），
//! 并与参照实现**逐条指令（含字节偏移）**对拍（`tools/gen_compile_fixture.py` 的夹具）。
//!
//! # 已接线
//!
//! - **模块级**：`NAME = <表达式>`（`;`／换行分隔）、`def NAME(形参…):` ＋ 缩进体
//! - **函数体**：`NAME = <表达式>`、`return <表达式>`（缩进块，`varnames` 先形参后局部）
//! - **表达式**：十进制整数字面量、单引号字符串字面量、名字、`+`（左结合）、比较
//!   （`<`／`<=`／`==`／`!=`／`>`／`>=`；`COMPARE_OP` 的 oparg **逐运算符实测**）
//! - **调用**：`f(...)`（位置实参；实参先用 `+`／比较／字面量／名字／再套一层调用）
//!   - 实测形状：`<可调用>; PUSH_NULL; <实参…>; CALL <个数>`；`PUSH_NULL` 取**被调用者**的
//!     跨度、`CALL` 取**整段调用**；表达式语句（`f()`）算完 `POP_TOP` 丢掉
//! - **关键字实参**：`f(a=1)`／`f(1, a=2)`／`f(b=2, a=1)`。实测形状：
//!   `<可调用>; PUSH_NULL; <位置实参…>; <关键字值…>; LOAD_CONST <名元组>; CALL_KW <位置+关键字数>`
//!   ——名元组是**紧邻 `CALL_KW` 之前**那条 `LOAD_CONST`（常量表里排在关键字值之后），
//!   名序照**源码顺序**
//! - **注解与边界检查的发射**（`BC-23`…`BC-25`／`TS-31`）：`def f(x: int) -> int:` 的形参注解与
//!   返回注解都可解析（`int`／`Any`／`None`／`list[int]` 这类一层或多层下标）；当
//!   **`mode == Extension` 且 `tier == Deep`** 且该函数带注解时，序言发 `CHECK_BOUNDARY_IN
//!   <签名常量>`、每个 `return` 前发 `CHECK_BOUNDARY_OUT <签名常量>`（`BC-25`②＋`TS-31`）
//!   - 签名条目＝**标签元组**：形参按顺序一个标签一条，未注解的形参记 `Any`；返回注解单发一条
//!   - `list[int]` ⇒ 复合标签 `(list, int)`（深层档位按它递归；`TS-30` 的不变性落在外类型上）
//!   - ⚠ **`BC-25`①的缺口**："只在标注／未标注的交界处发射；两侧都标注时禁止发射"要**跨模块**
//!     的静态信息，本层现在没有 ⇒ 暂按"该函数带标注"**保守**发射（宁可多查也不放过）
//!   - 参照实现在 3.14 用 **PEP 649** 的 `__annotate__` ＋ `SET_FUNCTION_ATTRIBUTE` 传注解，
//!     那是**另一族**（注解对象的求值）；该族**已落地**（`__annotate__` 单元 ＋ 属性通道 ＋
//!     `__annotations__`／`__doc__`），与本层的边界检查各自独立
//! - **形参**：位置默认值、`*args`／`**kw`、**仅关键字形参**（`def f(a, *, c=3)`）**都已落地**，
//!   且都与参照**逐字节**一致：
//!   - `varnames` 顺序＝位置参数 → 仅关键字 → `*args` → `**kw`；`argcount` 只数位置参数；
//!     `kwonlyargcount` 数仅关键字；flags ＝ `0x3 | varargs<<2 | varkw<<3`
//!   - 默认值：位置那条是**元组**（字面量时折叠成一条 `LOAD_CONST`，且**字面量与那个元组的
//!     入池次序都照参照**——元组排在常量表最后，故走"延迟入池"）；仅关键字那条是 `BUILD_MAP`
//!   - 挂载次序（实测）：`SET_FUNCTION_ATTRIBUTE` **16 → 2 → 1**
//!   - `/`（**仅位置**形参）也已落地：只改元数据（`co_posonlyargcount` 是前缀个数，
//!     **不产生指令**），绑定规则由 `bind_arguments` 负责
//!   - **仍未接**：形参默认值里的**算术折叠痕渍**（本层只折叠直接字面量）
//! - **字面量默认值的常量表次序**：实测 `def f(a, b=2)` 会在常量表里多出一个参照内部的槽
//!   （常量折叠的痕迹），本层暂时只对拍"名字默认值"那种干净形状
//! - **`*`／`**` 实参**（`CALL_FUNCTION_EX`，实测四种形状）：
//!   - 位置部分：没有 `*` 但有关键字 ⇒ `LOAD_CONST ()`；只有一个 `*` 且无前置位置实参 ⇒
//!     直接把那个可迭代对象交上去；有一个 `*` 且有前置位置实参 ⇒ `BUILD_LIST n`（前置实参
//!     已经压栈）＋ `<* 对象>` ＋ `LIST_EXTEND 1` ＋ `CALL_INTRINSIC_1 6`（`LIST_TO_TUPLE`）
//!   - 关键字部分：`名字=值` 逐对压栈后 `BUILD_MAP <对数>`（一对都没有就先 `BUILD_MAP 0`），
//!     随后每个 `**` 压栈 ＋ `DICT_MERGE 1`；一个关键字都没有就压 `PUSH_NULL`
//!   - 那个空元组常量是**收尾之后**才登记（`x = f(**d)` ⇒ `[None, ()]`），与折叠常量同一条路
//! - **循环的 `else`**：`while … else` 的 else 体**紧接退出标签**（没有额外跳转）；
//!   `for … else` 的 else 体**紧接 `POP_ITER`**（正常耗尽才走到）。`break`／`continue`
//!   仍需跳转修补，如实报未接线
//! - **`for` 循环**：`for <名字> in <可迭代>:` ＋ 缩进体。实测形状：
//!   `GET_ITER; FOR_ITER →耗尽; <目标存入>; <体>; JUMP_BACKWARD →FOR_ITER; END_FOR; POP_ITER`
//!   （注意 `END_FOR` 在 `POP_ITER` **之前**）；`for … else` 如实报未接线
//! - **循环**：`while <条件>:` ＋ 缩进体。回边用 `JUMP_BACKWARD`，oparg 是**往回**的距离
//!   （实测 `当前码元 + 占用码元数 − 目标码元`；方向由 opcode 定）；条件是**比较**时
//!   **不再**补 `TO_BOOL`（比较自带的 `bool(...)` 位已经是布尔），裸名字才补
//! - **控制流**：`if <条件>:` ＋ 缩进体，可带 `else:`（跳转目标按 `BC-55` 的公式回填，
//!   含缓存宽度；`if` 指令要 `TO_BOOL` ＋ `POP_JUMP_IF_FALSE` ＋ `NOT_TAKEN`）
//!   - 实测两条：末尾 `if` 的**每个分支**末尾各补一条隐式 `LOAD_CONST None; RETURN_VALUE`；
//!     末尾 `if/else` 两分支都 return ⇒ 模块**不再**补收尾（没有可落到末尾的路径）
//!
//! # 发射细节全为实测
//!
//! | 实测口径 | 说明 |
//! |---|---|
//! | `RESUME 0` 起头 | 模块与函数都一样 |
//! | 小整数 `0..=255` | 走 `LOAD_SMALL_INT`（`oparg` 就是值），但**常量表里照样登记** |
//! | 名字表 | 按**发射顺序**登记（`x = y` ⇒ `('y','x')`：值先于目标） |
//! | `co_qualname`（`BC-4`） | 作用域链：模块 ⇒ `<module>`、模块级 `def f` ⇒ `f`、函数里的函数 ⇒ `f.<locals>.g`（实测） |
//! | 模块收尾 | `None` **最后**登记 ＋ `LOAD_CONST <None>` ＋ `RETURN_VALUE` |
//! | 缓存槽 | 带缓存的指令后补**等宽零填充码元**（`BC-35`／`BC-36`）——补对了偏移才逐字相同 |
//! | `def` | `LOAD_CONST <嵌套下标>` ＋ `MAKE_FUNCTION`（**无 oparg**）＋ `STORE_NAME` |
//! | 函数 | `flags = 0x3`、`argcount` ＝ 形参个数、读局部 `LOAD_FAST_BORROW <槽>` |
//! | 赋值右值最外层是局部 | 用 `LOAD_FAST <槽>`（**不**借入）——实测 |
//! | `+` 两侧都是局部借入 | 打成 `LOAD_FAST_BORROW_LOAD_FAST_BORROW <高4位先压 | 低4位后压>`（实测 `b + a` ⇒ 16） |
//! | 函数常量表的 `None` | **只有该函数自己没有别的常量时**才登记（6 个形状都吻合；原因不明，规则照实写下来） |
//! | 嵌套调用的位置 | **未对齐**：实测 `x = f(g(1))` 的外层 `CALL`／存入／收尾都取**内层调用**的跨度（参照实现的位置传播细节）⇒ 该段如实标为未覆盖 |
//! | `CALL_FUNCTION_EX` 形态的位置 | **未对齐**：存入／收尾取**目标**（与普通 `CALL` 不同）⇒ 那 7 段语料如实标注，指令流照常对拍 |
//! | `for` 的位置 | **未对齐**：同 `if`／`while` ⇒ 该段语料如实标注，指令流照常对拍 |
//! | `while` 的位置 | **未对齐**：同 `if`（体与收尾另取一套）⇒ 两段 `while` 语料如实标注，指令流照常对拍 |
//! | `if` 的位置 | 实测：`if` 的**全部指令**（含分支里的）取**条件**的跨度 ⇒ 已实现；但模块收尾那两条在 `if` 形态下另取一套（跟着分支体最后一条的两半走）⇒ **未对齐**，夹具里 4 段 `if` 形态如实标注（指令流照常对拍） |
//! | **位置表**（`BC-18`） | 与指令一一对应；**逐形态实测**：模块 `RESUME` ⇒ `(0,1,0,0)`、函数 `RESUME` ⇒ `(def 行, def 行, 0, 0)`、字面量/名字取自身跨度、`BINARY_OP` 取整段 `a + b`、超指令取**先压的那个**名字、`STORE_NAME` 在"未折叠的 `+`"时取整段表达式否则取目标、`STORE_FAST` 总取目标、`def` 三条指令取整个 `def`、模块收尾两条取最后一条指令的位置；**`RETURN_VALUE` 四种形态四种值**（字面量／未折叠 `+`／折叠结果／裸名字） |
//!
//! | 常量折叠 | 只**最左叶子**进常量表（`1 + 2 + 3` ⇒ 表里只有 1）；结果是小整数走 `LOAD_SMALL_INT` 不进表；否则该常量**收尾之后**才登记（`x = 200 + 100` ⇒ `[200, None, 300]`）；字符串也折叠 |
//!
//! # 未接线（照实报 `Unsupported`／`Syntax`，不猜）
//!
//! 制表符缩进、嵌套函数定义、负数常量、字符串转义、任意精度整数（`i64` 溢出）、
//! `EXTENDED_ARG`（`oparg > 255`）、位置表（`BC-18`）、扩展模式的边界检查指令（`BC-23`…`BC-28`）。

use crate::opcode;

/// 编译模式（`BC-14`：**显式入参**，禁止从全局或环境推断）。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Mode {
    /// 纯 Python 模式（`IM-1`）。
    PurePython,
    /// 扩展模式（`IM-1`）。
    Extension,
}

/// **`TS-31`**：检查档位——**编译期参数**（与模式、优化级同层）。
///
/// `TS-31` 已裁定的口径：输入通道是**编译期参数**（深层检查 ＝ 对容器元素的**递归检查**，
/// 是**代码生成差异**而非运行期开关）；档位**必须**是可编码的有限集合，**至少**含
/// **浅层（默认）**与**深层**两种，具体编码由实现自选（本层：`0` ＝ 浅层、`1` ＝ 深层）。
///
/// 因为它是编译输入，产物**必须**带上它（`IM-19` 的头部、`IM-20` 的陈旧判定、
/// `IM-21` 的决定要素）——见 `pyawa-runtime` 的 `pyac`。
///
/// **两档的差别是"标签里带不带内层"**（`TS-31`："深层检查 ＝ 对容器元素的递归检查"，
/// 是**代码生成差异**）：
///
/// - **浅层（默认）**：只发**裸**类型标签（`int`／`list`）⇒ 执行器只判外类型（`TS-13`）
/// - **深层**：容器标签带内层（`(list, int)`）⇒ 执行器**递归**比元素（`TS-31`）
///
/// ⚠ **现状**：档位已是显式编译输入、也进产物（`IM-19`／`IM-20`／`IM-21`），但**编译器还没有
/// 标注支持** ⇒ 目前两者的代码段相同（差别只体现在头部字节）；"按标注发检查指令"（`BC-25`）
/// 随后接。执行器那半（带内层的标签就递归）**已落地**，见 `tests/boundary.rs`。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CheckTier {
    /// 浅层（默认）：边界处只做类型标签检查（`TS-13`）。
    Shallow,
    /// 深层：对容器元素**递归检查**（`TS-31` 的可选档位）。
    Deep,
}

impl CheckTier {
    /// 头部里那一字节的编码（实现自选；`IM-19` 只固定**字段顺序**）。
    pub const fn as_byte(self) -> u8 {
        match self {
            CheckTier::Shallow => 0,
            CheckTier::Deep => 1,
        }
    }

    /// 从头部字节解回档位；未知取值给 `None`（调用方按"读不出来"处理）。
    pub const fn from_byte(byte: u8) -> Option<CheckTier> {
        match byte {
            0 => Some(CheckTier::Shallow),
            1 => Some(CheckTier::Deep),
            _ => None,
        }
    }
}

/// 常量表里的一项。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Constant {
    /// `None`。
    None,
    /// `True`／`False`（实测：走 `LOAD_CONST`，常量表里就是它们）。
    Bool(bool),
    /// 整数。
    Int(i64),
    /// 字符串。
    Str(String),
    /// **`bytes` 字面量**（`P1-12`；不进 `co_consts` 的文本形态，实例化时建 `BytesObject`）。
    Bytes(Vec<u8>),
    /// 嵌套的 code object（本层只有函数体那一种）。
    Code(Box<CompiledUnit>),
    /// **关键字名元组**（`CALL_KW` 之前那条 `LOAD_CONST`；实测紧邻它、名序照源码顺序）。
    Names(Vec<String>),
    /// **类型对象**（按名字引用；`TS-31` 的边界检查标签用）。
    ///
    /// 编译器不认识运行期的类型对象，只能按名字指——实例化时由 `type_named` 解析；
    /// 解析不到就跳过这一条检查（`instantiate` 的注释里写明）。
    Type(String),
    /// **标签元组**（`TS-31`／`TS-30` 的复合标签：`list[int]` ⇒ `(list, int)`）。
    Tuple(Vec<Constant>),
}

/// 编译产物（**纯数据**）。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CompiledUnit {
    /// `co_name`。
    pub name: String,
    /// `BC-4` 的 `co_qualname`：作用域链上的名字（模块是 `<module>`，模块级 `def f` 是 `f`，
    /// 类体是 `C`、其方法 `C.m`，函数里的函数是 `f.<locals>.g`）。**实测**口径见测试。
    pub qualname: String,
    /// `co_argcount`。
    pub argcount: usize,
    /// `co_posonlyargcount`。
    pub posonlyargcount: usize,
    /// `co_kwonlyargcount`。
    pub kwonlyargcount: usize,
    /// `co_nlocals`。
    pub nlocals: usize,
    /// `co_flags`。
    pub flags: u32,
    /// `co_names`。
    pub names: Vec<String>,
    /// `co_varnames`。
    pub varnames: Vec<String>,
    /// `BC-45` 的 `co_cellvars`（类体的 `__classdict__` 就在这里）。
    pub cellvars: Vec<String>,
    /// `BC-45` 的 `co_freevars`。
    pub freevars: Vec<String>,
    /// `co_consts`。
    pub constants: Vec<Constant>,
    /// 字节码（每码元 2 字节：`opcode` ＋ `oparg`；带缓存的指令后跟等宽零填充）。
    pub code: Vec<u8>,
    /// **`BC-18` 的位置表**：与指令一一对应（起始行／结束行／起始列／结束列；行从 1 起、列从 0 起）。
    pub positions: Vec<(u32, u32, u32, u32)>,
}

/// 编译失败。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CompileError {
    /// 语法不认识（消息是本层自己的，**还没**对齐参照实现的 `SyntaxError` 文本）。
    Syntax(String),
    /// 认识但还没接线（消息说明是哪一件）。
    Unsupported(String),
}

/// 源码里的一段跨度（行从 1 起、列从 0 起）。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct Span {
    line_start: u32,
    line_end: u32,
    col_start: u32,
    col_end: u32,
}

impl Span {
    const fn new(line_start: u32, line_end: u32, col_start: u32, col_end: u32) -> Span {
        Span { line_start, line_end, col_start, col_end }
    }

    const fn synthetic() -> Span {
        // 模块 `RESUME` 的位置（实测 `(0, 1, 0, 0)`）
        Span::new(0, 1, 0, 0)
    }

    const fn tuple(self) -> (u32, u32, u32, u32) {
        (self.line_start, self.line_end, self.col_start, self.col_end)
    }

    /// 从 `self` 到 `other` 的整段（"整个表达式／整条语句"的位置）。
    fn to(self, other: Span) -> Span {
        Span {
            line_start: self.line_start,
            line_end: other.line_end,
            col_start: self.col_start,
            col_end: other.col_end,
        }
    }
}

/// 编译一段源码（`BC-16`：纯函数——同样的入参给同样的产物）。
///
/// `mode`（`BC-14`）、`tier`（`TS-31`）与 `optimization`（`IM-19`）都是**显式编译输入**，
/// **禁止**取默认值；它们与源码、指令集版本一起决定产物（`IM-21` 的五要素）。
///
/// `optimization` 的取值范围由**调用方**的契约给（ABI 侧见 `AB-61`）；本层**还没有优化器**，
/// 取值目前**不改变发射**——它作为输入在此**显式声明为暂不参与发射**（见下面的 `let _`），
/// 不是漏用。等 Pyawa 的 `-O` 语义有规格时，这一格就是落点。
pub fn compile(
    source: &str,
    filename: &str,
    mode: Mode,
    tier: CheckTier,
    optimization: u8,
) -> Result<CompiledUnit, CompileError> {
    // `BC-14`：模式是显式入参。`BC-15` 要求纯 Python 模式拒绝扩展语法——而 `§13-12` 已决
    // "扩展特性清单为空"，所以此刻两种模式的产物相同（`filename` 也还不进产物）。
    // `TS-31`：档位同层显式传入；深层档位目前不改发射（见 `CheckTier` 的说明）。
    // `IM-19`／`IM-21`：优化级同层显式传入，同样暂不改发射。
    let _ = (filename, optimization);
    let lexed = lex(source)?;
    let statements = parse_module(&lexed)?;
    if statements.is_empty() {
        return Err(CompileError::Syntax("没有语句".to_owned()));
    }
    compile_scope(
        "<module>",
        "<module>",
        &[],
        &[],
        None,
        None,
        None,
        mode,
        tier,
        &statements,
        ScopeKind::Module,
        Span::synthetic(),
    )
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum ScopeKind {
    Module,
    Function,
    /// **类体**（`class C: …`）：作用域与函数不同——无参、无局部槽、flags 0；
    /// 序言要铺 `__module__`／`__qualname__`／`__firstlineno__`，收尾要铺 `__static_attributes__`。
    Class,
}

/// **`__static_attributes__` 的静态收集**（实测 3.14 的规则）：
///
/// - 只收**赋值**形态 `self.名字 = …`；只读 `self.名字` 不算
/// - **按字母序输出**且**去重**（同一条里先写 `self.b` 再写 `self.a` ⇒ `('a', 'b')`）
/// - 类体层的普通赋值（`x = 1`）不算
/// - **嵌套函数里也算**（方法里的 `def inner(): self.z = 1` ⇒ 收到的 `z`）
///
/// - `if`／`while`／`for` 的体（含各自的 `else` 体）也走进去（实测：`if x: self.a = 1` ⇒ `('a',)`）
fn collect_static_attributes(statements: &[Statement], out: &mut Vec<String>) {
    for statement in statements {
        match statement {
            Statement::AssignAttr { object, name, .. } => {
                if matches!(object, Expression::Name(base, _) if base == "self") {
                    out.push(name.clone());
                }
            }
            // 方法与嵌套函数
            Statement::Def { body, .. } => collect_static_attributes(body, out),
            // 复合语句的体（实测：`if`／`while`／`for` 体里写 `self.X` 同样会收）
            Statement::For {
                body, else_body, ..
            }
            | Statement::While {
                body, else_body, ..
            } => {
                collect_static_attributes(body, out);
                collect_static_attributes(else_body, out);
            }
            Statement::If {
                then_body, else_body, ..
            } => {
                collect_static_attributes(then_body, out);
                collect_static_attributes(else_body, out);
            }
            _ => {}
        }
    }
}

/// 编译一个**类体**（`class C: …`）⇒ 一个 [`CompiledUnit`]。
///
/// 形态逐条实测（`co_name`／`co_qualname` 都是类名、`flags = 0`、`argcount = 0`、无局部槽）：
///
/// ```text
/// RESUME
/// LOAD_NAME __name__;   STORE_NAME __module__
/// LOAD_CONST 'C';       STORE_NAME __qualname__      （这个常量在**下标 0**）
/// LOAD_SMALL_INT <首行>; STORE_NAME __firstlineno__
/// [有文档串时：LOAD_CONST <文档串>; STORE_NAME __doc__]（紧随 `__qualname__` 那个常量之后）
/// <体里的语句>
/// LOAD_CONST ();        STORE_NAME __static_attributes__
/// LOAD_CONST None;      RETURN_VALUE
/// ```
///
/// **注意**：体里只有**赋值/表达式**语句时是这个形状；一旦体里出现 `def`，参照还会多出
/// `__classdict__` 这个 cell（`MAKE_CELL`／`LOAD_LOCALS`／`STORE_DEREF`／`__classdictcell__`）
/// ⇒ 那一支**尚未接线**（`def` 在本层仍报"嵌套的函数定义尚未接线"），别照这个形状硬拼。
fn compile_class_scope(
    name: &str,
    qualname: &str,
    mode: Mode,
    tier: CheckTier,
    statements: &[Statement],
    first_line: u32,
) -> Result<CompiledUnit, CompileError> {
    let span = Span::new(first_line, first_line, 0, 0);
    let docstring: Option<(String, Span)> = match statements.first() {
        Some(Statement::Expression(Expression::Str(text, span), _)) => {
            Some((text.clone(), *span))
        }
        _ => None,
    };
    let body: &[Statement] = if docstring.is_some() {
        &statements[1..]
    } else {
        statements
    };
    let mut emitter = Emitter {
        mode,
        tier,
        qualname: qualname.to_owned(),
        boundary_out: None,
        deferred: Vec::new(),
        pending: Vec::new(),
        jumps: Vec::new(),
        labels: Vec::new(),
        if_implicit_return: false,
        in_condition: false,
        // 类体的收尾由本函数**显式**发（`__static_attributes__` ＋ 隐式 return）
        epilogue_needed: false,
        epilogue_span: span,
        last_span: span,
        kind: ScopeKind::Class,
        unit: CompiledUnit {
            name: name.to_owned(),
            qualname: qualname.to_owned(),
            argcount: 0,
            posonlyargcount: 0,
            kwonlyargcount: 0,
            nlocals: 0,
            flags: 0,
            names: Vec::new(),
            varnames: Vec::new(),
            cellvars: Vec::new(),
            freevars: Vec::new(),
            constants: Vec::new(),
            code: Vec::new(),
            positions: Vec::new(),
        },
    };
    // **体里有 `def` 时**参照会多铺一个 `__classdict__` cell（实测）：
    //   `MAKE_CELL 0`（在 `RESUME` **之前**！）
    //   ＋ 序言之后 `LOAD_LOCALS; STORE_DEREF 0`
    //   ＋ 收尾前 `LOAD_FAST_BORROW 0; STORE_NAME __classdictcell__`
    // `co_cellvars = ("__classdict__",)`；`nlocals` 仍是 0（cell 不占局部槽）。
    let has_def = body
        .iter()
        .any(|statement| matches!(statement, Statement::Def { .. }));
    if has_def {
        emitter.unit.cellvars = vec!["__classdict__".to_owned()];
        emitter.emit_named(span, "MAKE_CELL", 0);
    }
    emitter.emit_named(span, "RESUME", 0);
    let module_name = emitter.intern_name("__name__");
    emitter.emit_named(span, "LOAD_NAME", module_name as u8);
    let module_attr = emitter.intern_name("__module__");
    emitter.emit_named(span, "STORE_NAME", module_attr as u8);
    // 类名常量：**下标 0**（实测）
    let qualname_const = emitter.intern_constant(Constant::Str(qualname.to_owned()));
    debug_assert_eq!(qualname_const, 0, "类体里 `__qualname__` 用的常量必须在 0");
    emitter.emit_named(span, "LOAD_CONST", qualname_const as u8);
    let qualname_attr = emitter.intern_name("__qualname__");
    emitter.emit_named(span, "STORE_NAME", qualname_attr as u8);
    emitter.emit_named(span, "LOAD_SMALL_INT", first_line as u8);
    let firstline_attr = emitter.intern_name("__firstlineno__");
    emitter.emit_named(span, "STORE_NAME", firstline_attr as u8);
    if has_def {
        // 序言之后：把**类命名空间**（`LOAD_LOCALS`）存进那个 cell
        emitter.emit_named(span, "LOAD_LOCALS", 0);
        emitter.emit_named(span, "STORE_DEREF", 0);
    }
    if let Some((text, doc_span)) = docstring.as_ref() {
        let index = emitter.intern_constant(Constant::Str(text.clone()));
        emitter.emit_named(*doc_span, "LOAD_CONST", index as u8);
        let doc_attr = emitter.intern_name("__doc__");
        emitter.emit_named(*doc_span, "STORE_NAME", doc_attr as u8);
    }
    for statement in body {
        emitter.emit_statement(statement)?;
    }
    // 收尾四条的位置取**体末句**（实测：`class C(B): x = 1` ⇒ `(2, 2, 4, 5)` —— 即最后一条
    // 指令的位点，与模块收尾"跟整段"不同）
    let tail_span = emitter.last_span;
    // `__static_attributes__`：**静态收集**方法体里写过的 `self.X`（实测：字母序去重）
    let mut static_names: Vec<String> = Vec::new();
    collect_static_attributes(body, &mut static_names);
    static_names.sort();
    static_names.dedup();
    let attributes = emitter.intern_constant(Constant::Tuple(
        static_names
            .into_iter()
            .map(Constant::Str)
            .collect::<Vec<_>>(),
    ));
    emitter.emit_named(tail_span, "LOAD_CONST", attributes as u8);
    let static_attr = emitter.intern_name("__static_attributes__");
    emitter.emit_named(tail_span, "STORE_NAME", static_attr as u8);
    if has_def {
        // 收尾前把 cell 里的命名空间也挂成 `__classdictcell__`（实测）
        emitter.emit_named(tail_span, "LOAD_FAST_BORROW", 0);
        let cell_attr = emitter.intern_name("__classdictcell__");
        emitter.emit_named(tail_span, "STORE_NAME", cell_attr as u8);
    }
    let none_index = emitter.intern_constant(Constant::None);
    emitter.emit_named(tail_span, "LOAD_CONST", none_index as u8);
    emitter.emit_named(tail_span, "RETURN_VALUE", 0);
    emitter.flush_jumps();
    Ok(emitter.unit)
}

/// 编译一个作用域（模块或函数）⇒ 一个 [`CompiledUnit`]。
///
/// `first_line` 是这个作用域第一行的行号（模块是 1，函数是 `def` 那一行——实测函数
/// `RESUME` 的位置是 `(def 行, def 行, 0, 0)`）。
fn compile_scope(
    name: &str,
    qualname: &str,
    parameters: &[Parameter],
    kwonly: &[Parameter],
    returns: Option<&Constant>,
    varargs: Option<&str>,
    varkw: Option<&str>,
    mode: Mode,
    tier: CheckTier,
    statements: &[Statement],
    kind: ScopeKind,
    resume_span: Span,
) -> Result<CompiledUnit, CompileError> {
    // **文档字符串**（实测）：作用域里**第一条**语句是字符串字面量时它就是文档串——
    // 它进**常量 0**、**不产生指令**；函数的 `co_flags` 还要置 `0x4000000`（"有文档串"标志），
    // 函数对象的 `__doc__` 就靠这个标志区分"常量 0 恰好是字符串"（`def f(): return "x"`
    // 的 `__doc__` 是 `None` 而 `co_consts[0]` 是 `'x'`——实测）。
    // 模块那半另有形态：`LOAD_CONST <0>; STORE_NAME __doc__`（实测）。
    let docstring: Option<(String, Span)> = match statements.first() {
        Some(Statement::Expression(Expression::Str(text, span), _)) => {
            Some((text.clone(), *span))
        }
        _ => None,
    };
    let body: &[Statement] = if docstring.is_some() {
        &statements[1..]
    } else {
        statements
    };
    let mut emitter = Emitter {
        mode,
        tier,
        qualname: qualname.to_owned(),
        boundary_out: None,
        deferred: Vec::new(),
        pending: Vec::new(),
        jumps: Vec::new(),
        labels: Vec::new(),
        if_implicit_return: false,
        in_condition: false,
        epilogue_needed: true,
        epilogue_span: resume_span,
        last_span: resume_span,
        unit: CompiledUnit {
            name: name.to_owned(),
            qualname: qualname.to_owned(),
            argcount: parameters.len(),
            // 仅位置形参是**前缀**（解析时保证）
            posonlyargcount: parameters
                .iter()
                .take_while(|parameter| parameter.posonly)
                .count(),
            kwonlyargcount: kwonly.len(),
            cellvars: Vec::new(),
            freevars: Vec::new(),
            // `varnames` 的顺序（实测／`argbind.rs` 记着）：位置参数 → 仅关键字 → `*args` → `**kw`
            nlocals: parameters.len()
                + kwonly.len()
                + usize::from(varargs.is_some())
                + usize::from(varkw.is_some()),
            flags: if kind == ScopeKind::Function {
                0x3 | (u32::from(varargs.is_some()) << 2) | (u32::from(varkw.is_some()) << 3)
                    | if docstring.is_some() { 0x400_0000 } else { 0 }
            } else {
                0
            },
            names: Vec::new(),
            varnames: parameters
                .iter()
                .chain(kwonly.iter())
                .map(|parameter| parameter.name.clone())
                .chain(varargs.map(str::to_owned))
                .chain(varkw.map(str::to_owned))
                .collect(),
            constants: Vec::new(),
            code: Vec::new(),
            positions: Vec::new(),
        },
        kind,
    };
    emitter.emit_at(
        resume_span,
        opcode::opcode("RESUME").expect("RESUME 在表里"),
        0,
    );
    // 文档串进**常量 0**（实测）；模块那半还要把它存进 `__doc__`
    if let Some((text, span)) = docstring.as_ref() {
        let index = emitter.intern_constant(Constant::Str(text.clone()));
        debug_assert_eq!(index, 0, "文档串必须是常量 0");
        if kind == ScopeKind::Module {
            // 实测：这**两条**的位置是文档串语句自己的跨度
            let name = emitter.intern_name("__doc__");
            emitter.emit_named(*span, "LOAD_CONST", index as u8);
            emitter.emit_named(*span, "STORE_NAME", name as u8);
        }
    }
    // **`BC-25`②＋`TS-31`**：边界检查指令**只**在扩展模式编译出的代码里发，且**只在深层档位**下。
    // `BC-25`①的"只在标注／未标注的交界处发射"要**跨模块**的静态信息（当前没有）⇒ 暂按
    // "该函数带标注"**保守**发射（宁可多查也不放过）；缺口记在模块文档里。
    if kind == ScopeKind::Function && mode == Mode::Extension && tier == CheckTier::Deep {
        let annotated = parameters
            .iter()
            .any(|parameter| parameter.annotation.is_some())
            || returns.is_some();
        if annotated {
            if !parameters.is_empty() {
                let labels: Vec<Constant> = parameters
                    .iter()
                    .map(|parameter| {
                        parameter
                            .annotation
                            .clone()
                            .unwrap_or_else(|| Constant::Str("Any".to_owned()))
                    })
                    .collect();
                let index = emitter.intern_constant(Constant::Tuple(labels));
                emitter.emit_at(
                    resume_span,
                    opcode::opcode("CHECK_BOUNDARY_IN").expect("专有指令在表里"),
                    index as u8,
                );
            }
            if let Some(label) = returns {
                let index = emitter.intern_constant(Constant::Tuple(vec![label.clone()]));
                emitter.boundary_out = Some(index);
            }
        }
    }
    let last_index = body.len().saturating_sub(1);
    for (index, statement) in body.iter().enumerate() {
        emitter.if_implicit_return = kind == ScopeKind::Module
            && index == last_index
            && matches!(statement, Statement::If { .. });
        emitter.emit_statement(statement)?;
        emitter.if_implicit_return = false;
    }
    // 收尾顺序照实测：
    //   模块：先登记 `None`（`LOAD_CONST <None>` ＋ `RETURN_VALUE`，位置取**最后一条指令**的），
    //         然后才把折叠出来的常量追加进表尾（`x = 200 + 100` ⇒ `[200, None, 300]`）
    //   函数：先冲刷折叠常量（`return 200 + 100` ⇒ `[200, 300]`），再判"表还空着就登记 None"
    if kind == ScopeKind::Module && emitter.epilogue_needed {
        let none_index = emitter.intern_constant(Constant::None);
        let tail = emitter.epilogue_span;
        emitter.emit_at(
            tail,
            opcode::opcode("LOAD_CONST").expect("LOAD_CONST 在表里"),
            none_index as u8,
        );
        emitter.emit_at(tail, opcode::opcode("RETURN_VALUE").expect("RETURN_VALUE 在表里"), 0);
        emitter.flush_pending();
        // **延迟入池**的常量（字面量默认值折出来的元组）：参照把它们排在常量表最后
    for (offset, constant) in core::mem::take(&mut emitter.deferred) {
        let index = emitter.intern_constant(constant);
        emitter.unit.code[offset] = index as u8;
    }
    emitter.flush_jumps();
    } else if kind == ScopeKind::Module {
        // 不需要收尾（末尾 `if/else` 两分支都 return）
        emitter.flush_pending();
        emitter.flush_jumps();
    } else {
        emitter.flush_pending();
        // **函数的隐式返回**：函数体可以"落到末尾"时，参照会补 `LOAD_CONST None; RETURN_VALUE`
        // （`epilogue_needed` 初值为真，遇到 `return` 会置假）。位置取**最后一条真指令**的跨度，
        // 与类体那条规则一致。**之前这里只登记了 `None` 常量、从不发这两条指令** ⇒
        // "没有显式 `return` 的函数"执行时必然 `FellOffEnd`（第 100 轮抓到的根因）。
        // 只有**语句体末尾不是 `return`** 时才补（末尾是 `return` 就没有可落到末尾的路径）；
        // `if/else` 两条分支都 return 的情形已由 `epilogue_needed` 置假覆盖。
        let falls_through = !matches!(body.last(), Some(Statement::Return(_, _)));
        if emitter.epilogue_needed && falls_through {
            let tail_span = emitter.last_span;
            let none_index = emitter.intern_constant(Constant::None);
            emitter.emit_at(
                tail_span,
                opcode::opcode("LOAD_CONST").expect("LOAD_CONST 在表里"),
                none_index as u8,
            );
            emitter.emit_at(
                tail_span,
                opcode::opcode("RETURN_VALUE").expect("RETURN_VALUE 在表里"),
                0,
            );
        }
        emitter.flush_jumps();
        if emitter.unit.constants.is_empty() {
            // 实测（**与补不补尾两条无关**）：函数自己没有任何常量时，参照仍会在常量表里登记一个
            // `None`（`def f(**kw): return kw` ⇒ `['None']`）。原因不明，规则照实写下来。
            emitter.intern_constant(Constant::None);
        }
    }
    Ok(emitter.unit)
}

struct Emitter {
    unit: CompiledUnit,
    kind: ScopeKind,
    /// 编译输入（`BC-14`／`TS-31`）：检查指令**只**在扩展模式 ＋ 深层档位下发射（`BC-25`②）。
    mode: Mode,
    tier: CheckTier,
    /// 当前作用域的 `co_qualname`（`BC-4`）：嵌套 `def` 要用它算下一层的名字。
    qualname: String,
    /// 返回值的边界检查标签下标（`BC-23` 的 `CHECK_BOUNDARY_OUT`；`None` ⇒ 不发）。
    boundary_out: Option<usize>,
    /// **延迟入池**的常量：`(LOAD_CONST 的实参字节偏移, 常量)`。
    ///
    /// 用途：字面量默认值折叠出来的元组，在参照实现里**排在常量表最后**（在模块收尾的 `None`
    /// 之后）——所以要等收尾时再入池，再把下标回填到那条 `LOAD_CONST` 的实参字节上。
    deferred: Vec<(usize, Constant)>,
    /// 最后一条真指令的位置（隐式 return 用它）。
    last_span: Span,
    /// 模块收尾两条指令的位置。实测：`+` 形态跟**右值**走，比较／字面量／名字跟**目标**走
    /// （与 `STORE_NAME` 的形态规则只差比较那一格）。
    epilogue_span: Span,
    /// **折叠出来的常量**：登记时机在收尾之后，先记下"要回填的 `LOAD_CONST` 实参位置"。
    pending: Vec<(usize, Constant)>,
    /// 跳转回填：`(要回填的实参字节位置, 标签号, 该指令占用的码元数)`。
    /// `BC-55`：目标码元 = 当前码元 + 指令占用码元数 + 有符号 oparg ⇒ 回填时反过来算。
    jumps: Vec<(usize, usize, usize)>,
    /// 标签 ⇒ 码元位置。
    labels: Vec<Option<usize>>,
    /// 模块收尾还需不需要补 `LOAD_CONST None; RETURN_VALUE`。
    /// 实测：末尾的 `if/else` 两个分支都 `return` ⇒ **没有**可落到末尾的路径 ⇒ 参照不再补。
    epilogue_needed: bool,
    /// 瞬时标志：正在编译**条件**（`if`／`while` 的）⇒ 比较要带 `bool(...)` 位
    /// （实测：`while a < b` 的 `COMPARE_OP` oparg 是 18 ＝ 2 | 16，而赋值里的比较是 2）。
    in_condition: bool,
    /// 瞬时标志：当前这条语句是**作用域最后一条 `if`** ⇒ 它的每个分支末尾要补一条
    /// `LOAD_CONST None; RETURN_VALUE`（实测；只有模块末尾的 `if` 会这样）。
    if_implicit_return: bool,
}

impl Emitter {
    /// 发射一条指令并记下它的位置（`BC-18`）。
    /// 新开一个标签；返回它的编号。
    fn new_label(&mut self) -> usize {
        self.labels.push(None);
        self.labels.len() - 1
    }

    /// 按**名字**发一条指令（名字一定在表里；内部用）。
    fn emit_named(&mut self, position: Span, name: &str, oparg: u8) {
        self.emit_at(
            position,
            opcode::opcode(name).expect("指令在表里"),
            oparg,
        );
    }

    /// 记下标签落在**当前**码元处。
    fn mark_label(&mut self, label: usize) {
        self.labels[label] = Some(self.unit.code.len() / 2);
    }

    /// 发一条**前向跳转**（目标标签先占位、收尾时回填）。
    fn emit_jump(&mut self, position: Span, opcode: u16, label: usize) {
        self.emit_directed_jump(position, opcode, label, false);
    }

    /// 发一条跳转；`backward` 为真时 oparg 是**往回**的距离
    /// （实测 `JUMP_BACKWARD` 的 oparg ＝ `当前码元 + 占用码元数 − 目标码元`，方向是 opcode 本身定的）。
    fn emit_directed_jump(&mut self, position: Span, opcode: u16, label: usize, backward: bool) {
        let argument_byte = self.unit.code.len() + 1;
        let size = 1 + opcode::inline_cache_entries(opcode) as usize;
        self.emit_at(position, opcode, 0);
        self.jumps.push((argument_byte, label, size | (usize::from(backward) << 16)));
    }

    /// 收尾时把跳转实参回填（`BC-55` 的公式反过来用）。
    fn flush_jumps(&mut self) {
        let jumps = core::mem::take(&mut self.jumps);
        for (argument_byte, label, packed) in jumps {
            let size = packed & 0xFFFF;
            let backward = packed >> 16 != 0;
            let target = self.labels[label].expect("标签必须已经落点");
            let here = argument_byte / 2; // 该指令的 opcode 所在码元
            let argument = if backward {
                (here + size) as i64 - target as i64
            } else {
                target as i64 - (here + size) as i64
            };
            debug_assert!((0..=255).contains(&argument), "本层不支持 EXTENDED_ARG");
            self.unit.code[argument_byte] = argument as u8;
        }
    }

    fn emit_at(&mut self, position: Span, opcode: u16, oparg: u8) {
        self.unit.positions.push(position.tuple());
        self.last_span = position;
        self.unit.code.push(opcode as u8);
        self.unit.code.push(oparg);
        // `BC-35`／`BC-36`：带缓存的指令后必须留等宽**零填充**码元
        for _ in 0..opcode::inline_cache_entries(opcode) {
            self.unit.code.push(0);
            self.unit.code.push(0);
        }
    }

    /// 收尾时把"待定常量"登记进表并回填实参。
    fn flush_pending(&mut self) {
        let pending = core::mem::take(&mut self.pending);
        for (argument_byte, constant) in pending {
            let index = self.intern_constant(constant);
            self.unit.code[argument_byte] = index as u8;
        }
    }

    fn intern_constant(&mut self, constant: Constant) -> usize {
        if let Some(index) = self.unit.constants.iter().position(|item| *item == constant) {
            return index;
        }
        self.unit.constants.push(constant);
        self.unit.constants.len() - 1
    }

    /// 登记一个字面量，规矩照实测：小整数**只在常量表还是空的时候**才登记；大整数与字符串总是登记。
    fn intern_literal(&mut self, constant: Constant) {
        if let Constant::Int(value) = constant {
            if (0..=255).contains(&value) && !self.unit.constants.is_empty() {
                return;
            }
        }
        self.intern_constant(constant);
    }

    fn intern_name(&mut self, name: &str) -> usize {
        if let Some(index) = self.unit.names.iter().position(|item| item == name) {
            return index;
        }
        self.unit.names.push(name.to_owned());
        self.unit.names.len() - 1
    }

    /// 局部槽位（没有就按首次出现顺序追加——形参已经在前面）。
    fn slot_of(&mut self, name: &str) -> usize {
        if let Some(index) = self.unit.varnames.iter().position(|item| item == name) {
            return index;
        }
        self.unit.varnames.push(name.to_owned());
        self.unit.nlocals = self.unit.varnames.len();
        self.unit.varnames.len() - 1
    }

    fn emit_statement(&mut self, statement: &Statement) -> Result<(), CompileError> {
        match statement {
            Statement::Assign {
                target,
                target_span,
                value,
                span,
            } => {
                // 右值最外层是局部时用 `LOAD_FAST`（实测：`x = a` ⇒ `LOAD_FAST 0`，位置是那个名字的）
                let store_span;
                match (self.kind, value) {
                    (ScopeKind::Function, Expression::Name(name, name_span))
                        if self.unit.varnames.iter().any(|item| item == name) =>
                    {
                        let slot = self.slot_of(name);
                        self.emit_at(
                            *name_span,
                            opcode::opcode("LOAD_FAST").expect("LOAD_FAST 在表里"),
                            slot as u8,
                        );
                        store_span = *target_span;
                    }
                    _ => {
                        self.emit_expression(value)?;
                        // **`STORE_NAME` 的位置逐形态实测**（六例吻合）：右值是**非常量**的
                        // 复合表达式（未折叠的 `+`、比较）⇒ 取整段表达式
                        // （`z = w + 2` ⇒ `(1,1,4,9)`、`x = 1 < 2` ⇒ `(1,1,4,9)`）；
                        // 其余（字面量、名字、折叠结果）⇒ 取**目标**
                        // （`x = 1` ⇒ `(0,1)`、`y = x` ⇒ `(7,8)`）
                        let compound = match value {
                            Expression::Add(_, _, _) => fold_constant(value)?.is_none(),
                            Expression::Compare(_, _, _, _) | Expression::Call { .. } => true,
                            _ => false,
                        };
                        store_span = if compound { value.span() } else { *target_span };
                        // 收尾两条的位置逐形态实测：`+`／调用 ⇒ 跟**右值**；比较 ⇒ 跟**目标**；
                        // 字面量／名字／折叠结果 ⇒ 跟**目标**
                        self.epilogue_span = match value {
                            Expression::Add(_, _, _) if compound => value.span(),
                            Expression::Call { .. } => value.span(),
                            _ => *target_span,
                        };
                    }
                }
                let _ = span;
                match self.kind {
                    ScopeKind::Module | ScopeKind::Class => {
                        let index = self.intern_name(target);
                        self.emit_at(
                            store_span,
                            opcode::opcode("STORE_NAME").expect("STORE_NAME 在表里"),
                            index as u8,
                        );
                    }
                    ScopeKind::Function => {
                        let slot = self.slot_of(target);
                        // 实测：`STORE_FAST` 的位置**总是目标**（`x = a` ⇒ `x` 那一格）
                        self.emit_at(
                            *target_span,
                            opcode::opcode("STORE_FAST").expect("STORE_FAST 在表里"),
                            slot as u8,
                        );
                    }
                }
                Ok(())
            }
            Statement::Expression(value, span) => {
                self.emit_expression(value)?;
                // 实测：表达式语句算完 `POP_TOP` 丢掉，位置是整段表达式
                self.emit_at(*span, opcode::opcode("POP_TOP").expect("POP_TOP 在表里"), 0);
                // 收尾两条跟这段表达式走（实测 `f()` 语句 ⇒ 收尾位置 (1,1,0,3)）
                self.epilogue_span = *span;
                Ok(())
            }
            Statement::Raise { value, cause, span } => {
                match (value, cause) {
                    (Some(value), cause) => {
                        self.emit_expression(value)?;
                        let count = if let Some(cause) = cause {
                            self.emit_expression(cause)?;
                            2
                        } else {
                            1
                        };
                        self.emit_named(*span, "RAISE_VARARGS", count);
                    }
                    (None, _) => self.emit_named(*span, "RAISE_VARARGS", 0),
                }
                // `raise` 不落到末尾 ⇒ 不补隐式返回；**收尾两条的位点跟本条语句**
                // （模块收尾读的就是这个字段：每条语句臂都要把它设成自己的跨度）
                self.epilogue_needed = false;
                self.epilogue_span = *span;
                Ok(())
            }
            Statement::Return(value, span) => {
                self.emit_expression(value)?;
                // `BC-23` 的 `CHECK_BOUNDARY_OUT`：**在返回值压栈之后、`RETURN_VALUE` 之前**
                // （它看的是栈顶且**不弹出**）
                if let Some(index) = self.boundary_out {
                    self.emit_at(
                        *span,
                        opcode::opcode("CHECK_BOUNDARY_OUT").expect("专有指令在表里"),
                        index as u8,
                    );
                }
                // **`RETURN_VALUE` 的位置逐形态实测**（参照实现的位置传播细节，四种形态四种值）：
                //   字面量       ⇒ 取**那个字面量**（`return 'a'` ⇒ `(2,2,11,14)`）
                //   未折叠的 `+` ⇒ 取**整个表达式**（`return a + 1` ⇒ `(2,2,11,16)`）
                //   折叠过的 `+` ⇒ 取**整条 return**（`return 200 + 100` ⇒ `(2,2,4,20)`）
                //   裸名字       ⇒ 取**整条 return**（`return a` ⇒ `(2,2,4,12)`）
                let position = match value {
                    Expression::Int(_, _) | Expression::Str(_, _) => value.span(),
                    Expression::Add(_, _, _) if fold_constant(value)?.is_none() => value.span(),
                    _ => *span,
                };
                self.emit_at(
                    position,
                    opcode::opcode("RETURN_VALUE").expect("RETURN_VALUE 在表里"),
                    0,
                );
                Ok(())
            }
            Statement::For {
                span: _,
                target,
                target_span,
                iterable,
                body,
                else_body,
            } => {
                self.emit_expression(iterable)?;
                self.emit_at(
                    iterable.span(),
                    opcode::opcode("GET_ITER").expect("GET_ITER 在表里"),
                    0,
                );
                let loop_label = self.new_label();
                let exhausted = self.new_label();
                self.mark_label(loop_label);
                self.emit_jump(
                    iterable.span(),
                    opcode::opcode("FOR_ITER").expect("FOR_ITER 在表里"),
                    exhausted,
                );
                match self.kind {
                    ScopeKind::Module | ScopeKind::Class => {
                        let index = self.intern_name(target);
                        self.emit_at(
                            *target_span,
                            opcode::opcode("STORE_NAME").expect("STORE_NAME 在表里"),
                            index as u8,
                        );
                    }
                    ScopeKind::Function => {
                        let slot = self.slot_of(target);
                        self.emit_at(
                            *target_span,
                            opcode::opcode("STORE_FAST").expect("STORE_FAST 在表里"),
                            slot as u8,
                        );
                    }
                }
                self.emit_block(body, false)?;
                self.emit_directed_jump(
                    iterable.span(),
                    opcode::opcode("JUMP_BACKWARD").expect("JUMP_BACKWARD 在表里"),
                    loop_label,
                    true,
                );
                self.mark_label(exhausted);
                // 实测：耗尽后 `END_FOR` ＋ `POP_ITER`（`END_FOR` 在 `POP_ITER` 之前，不是反过来）
                self.emit_at(
                    iterable.span(),
                    opcode::opcode("END_FOR").expect("END_FOR 在表里"),
                    0,
                );
                self.emit_at(
                    iterable.span(),
                    opcode::opcode("POP_ITER").expect("POP_ITER 在表里"),
                    0,
                );
                // 实测：`for … else` 的 else 体紧接 `POP_ITER`（正常耗尽才走到这里）
                if !else_body.is_empty() {
                    self.emit_block(else_body, false)?;
                }
                self.epilogue_span = *target_span;
                Ok(())
            }
            Statement::While {
                span: _,
                condition,
                body,
                else_body,
            } => {
                let condition_span = condition.span();
                let start = self.new_label();
                let after = self.new_label();
                self.mark_label(start);
                self.in_condition = true;
                self.emit_expression(condition)?;
                self.in_condition = false;
                // 实测：条件是**比较**时**不再**补 `TO_BOOL`（比较自带的 `bool(...)` 位
                // 已经交出布尔了）；条件不是比较（如裸名字）才补。
                if !matches!(condition, Expression::Compare(_, _, _, _)) {
                    self.emit_at(
                        condition_span,
                        opcode::opcode("TO_BOOL").expect("TO_BOOL 在表里"),
                        0,
                    );
                }
                self.emit_jump(
                    condition_span,
                    opcode::opcode("POP_JUMP_IF_FALSE").expect("POP_JUMP_IF_FALSE 在表里"),
                    after,
                );
                self.emit_at(
                    condition_span,
                    opcode::opcode("NOT_TAKEN").expect("NOT_TAKEN 在表里"),
                    0,
                );
                self.emit_block(body, false)?;
                self.emit_directed_jump(
                    condition_span,
                    opcode::opcode("JUMP_BACKWARD").expect("JUMP_BACKWARD 在表里"),
                    start,
                    true,
                );
                self.mark_label(after);
                // 实测：`while … else` 的 else 体**紧接退出标签**（没有额外跳转）
                if !else_body.is_empty() {
                    self.emit_block(else_body, false)?;
                }
                // 循环之后的收尾跟着循环体最后一条走（实测 `while a: x = 1` ⇒ 收尾位置是条件那一段）
                self.epilogue_span = condition_span;
                Ok(())
            }
            Statement::If {
                span: _,
                condition,
                then_body,
                else_body,
            } => {
                let condition_span = condition.span();
                self.in_condition = true;
                self.emit_expression(condition)?;
                self.in_condition = false;
                // 实测：条件不是比较时补 `TO_BOOL`（3 个缓存槽）⇒ `POP_JUMP_IF_FALSE`（1 个缓存槽）⇒ `NOT_TAKEN`
                if !matches!(condition, Expression::Compare(_, _, _, _)) {
                    self.emit_at(
                        condition_span,
                        opcode::opcode("TO_BOOL").expect("TO_BOOL 在表里"),
                        0,
                    );
                }
                let skip = self.new_label();
                self.emit_jump(
                    condition_span,
                    opcode::opcode("POP_JUMP_IF_FALSE").expect("POP_JUMP_IF_FALSE 在表里"),
                    skip,
                );
                self.emit_at(
                    condition_span,
                    opcode::opcode("NOT_TAKEN").expect("NOT_TAKEN 在表里"),
                    0,
                );
                self.emit_block(then_body, false)?;
                let implicit = self.if_implicit_return;
                if implicit {
                    self.emit_implicit_return();
                }
                if else_body.is_empty() {
                    self.mark_label(skip);
                } else if implicit {
                    // 分支末尾有隐式 `return` ⇒ then 分支**不会**落到 else（实测：这条 `if` 不发
                    // `JUMP_FORWARD`）；两个分支都 return ⇒ 模块末尾也没有可落到的路径
                    // ⇒ 收尾那两条也**不补**（实测）
                    self.mark_label(skip);
                    self.emit_block(else_body, false)?;
                    self.emit_implicit_return();
                    self.epilogue_needed = false;
                } else {
                    let after = self.new_label();
                    self.emit_jump(
                        condition_span,
                        opcode::opcode("JUMP_FORWARD").expect("JUMP_FORWARD 在表里"),
                        after,
                    );
                    self.mark_label(skip);
                    self.emit_block(else_body, false)?;
                    self.mark_label(after);
                }
                Ok(())
            }
            // **属性赋值**（实测）：先压**值**，再压**对象**，然后 `STORE_ATTR <名字下标>`
            Statement::AssignAttr {
                object,
                name,
                value,
                span,
            } => {
                self.emit_expression(value)?;
                self.emit_expression(object)?;
                let index = self.intern_name(name);
                self.emit_named(*span, "STORE_ATTR", index as u8);
                self.epilogue_span = *span;
                Ok(())
            }
            // **类体**（`class C[(B)]: …`）：模块级形态逐条实测——
            // `LOAD_BUILD_CLASS; PUSH_NULL; LOAD_CONST <体 code>; MAKE_FUNCTION;
            //  LOAD_CONST 'C'; [每个基类一条；`LOAD_NAME` 形态见下]; CALL 2+n; STORE_NAME C`
            Statement::Class {
                name,
                span,
                first_line,
                bases,
                body,
            } => {
                // 实测：基类是用 **`LOAD_NAME`** 压栈的（不是 `LOAD_CONST`）
                let nested = compile_class_scope(
                    name,
                    name,
                    self.mode,
                    self.tier,
                    body,
                    *first_line,
                )?;
                let index = self.intern_constant(Constant::Code(Box::new(nested)));
                self.emit_named(*span, "LOAD_BUILD_CLASS", 0);
                self.emit_named(*span, "PUSH_NULL", 0);
                self.emit_named(*span, "LOAD_CONST", index as u8);
                self.emit_named(*span, "MAKE_FUNCTION", 0);
                let name_const = self.intern_constant(Constant::Str(name.clone()));
                self.emit_named(*span, "LOAD_CONST", name_const as u8);
                for base in bases {
                    self.emit_expression(base)?;
                }
                self.emit_named(*span, "CALL", (2 + bases.len()) as u8);
                let store_index = self.intern_name(name);
                self.emit_named(*span, "STORE_NAME", store_index as u8);
                // 收尾两条跟整段（与 `def` 同规则，实测）
                self.epilogue_span = *span;
                Ok(())
            }
            Statement::Def {
                name,
                span,
                first_line,
                parameters,
                kwonly,
                returns,
                varargs,
                varkw,
                body,
            } => {
                // 允许在**模块**与**类体**里定义函数；函数里嵌套 `def` 仍未接线
                if self.kind == ScopeKind::Function {
                    return Err(CompileError::Unsupported("嵌套的函数定义尚未接线".to_owned()));
                }
                // `BC-4` 的 qualname 规则（实测）：模块级 `def f` ⇒ `f`；函数**里**的定义
                // 走 `<locals>` 段（`f.<locals>.g`）；类体（编译器尚未接线）则是 `C.m`。
                // 实测：类体里的 `def m` ⇒ `C.m`；函数里的 `def g` ⇒ `f.<locals>.g`
                let nested_qualname = match self.kind {
                    ScopeKind::Module => name.clone(),
                    ScopeKind::Class => format!("{}.{name}", self.qualname),
                    ScopeKind::Function => format!("{}.<locals>.{name}", self.qualname),
                };
                let nested = compile_scope(
                    name,
                    &nested_qualname,
                    parameters,
                    kwonly,
                    returns.as_ref(),
                    varargs.as_deref(),
                    varkw.as_deref(),
                    self.mode,
                    self.tier,
                    body,
                    ScopeKind::Function,
                    Span::new(*first_line, *first_line, 0, 0),
                )?;
                // **默认值**（实测）：`def f(a, b=x)` ⇒ 逐个求值默认值再 `BUILD_TUPLE n`，
                // 排在 `LOAD_CONST <code>` **之前**；挂载在 `MAKE_FUNCTION` 之后
                // （有注解时次序是 `SET_FUNCTION_ATTRIBUTE 16` 再 `1`）。
                let defaults: Vec<&Expression> = parameters
                    .iter()
                    .filter_map(|parameter| parameter.default.as_ref())
                    .collect();
                if !defaults.is_empty() {
                    // **字面量默认值折叠**（实测：`def f(a, b=2)` ⇒ 一条 `LOAD_CONST (2,)`，
                    // 常量表里那个元组排在**最后**，位点取**体末句**）
                    let literals: Option<Vec<Constant>> =
                        defaults.iter().map(|expression| constant_expression(expression)).collect();
                    match literals {
                        Some(constants) => {
                            // 实测：参照折叠时**先把字面量本身入池**（这些槽没人引用，是折叠的
                            // 痕渍）⇒ 要照做，否则常量表对不上
                            for constant in &constants {
                                self.intern_constant(constant.clone());
                            }
                            let offset = self.unit.code.len() + 1;
                            self.emit_named(*span, "LOAD_CONST", 0);
                            self.deferred.push((offset, Constant::Tuple(constants)));
                        }
                        None => {
                            for expression in &defaults {
                                self.emit_expression(expression)?;
                            }
                            self.emit_named(*span, "BUILD_TUPLE", defaults.len() as u8);
                        }
                    }
                }
                // **仅关键字默认值**（实测）：`LOAD_CONST 'c'; <值>; …; BUILD_MAP n`，
                // 排在位置默认值元组之后、code 之前；**不折叠**（字面量也走 `LOAD_SMALL_INT`）
                let kwdefaults: Vec<&Parameter> = kwonly
                    .iter()
                    .filter(|parameter| parameter.default.is_some())
                    .collect();
                if !kwdefaults.is_empty() {
                    for parameter in &kwdefaults {
                        let key =
                            self.intern_constant(Constant::Str(parameter.name.clone()));
                        self.emit_named(*span, "LOAD_CONST", key as u8);
                        if let Some(default) = parameter.default.as_ref() {
                            self.emit_expression(default)?;
                        }
                    }
                    self.emit_named(*span, "BUILD_MAP", kwdefaults.len() as u8);
                }
                // **PEP 649**：带注解的 `def` 先造 `__annotate__` 单元（实测：它在常量表里
                // 排在函数 code **之前**，随即 `MAKE_FUNCTION` ＋ `SET_FUNCTION_ATTRIBUTE 16`）
                let annotated = parameters
                    .iter()
                    .chain(kwonly.iter())
                    .any(|parameter| parameter.annotation.is_some())
                    || returns.is_some();
                if annotated {
                    let annotate_qualname = if self.qualname == "<module>" {
                        "__annotate__".to_owned()
                    } else {
                        format!("{}.<locals>.__annotate__", self.qualname)
                    };
                    let unit = self.annotate_unit(
                        &annotate_qualname,
                        parameters,
                        kwonly,
                        returns.as_ref(),
                        *span,
                    );
                    let annotate_index = self.intern_constant(Constant::Code(Box::new(unit)));
                    self.emit_named(*span, "LOAD_CONST", annotate_index as u8);
                    self.emit_named(*span, "MAKE_FUNCTION", 0);
                }
                let index = self.intern_constant(Constant::Code(Box::new(nested)));
                // 实测：`def` 的三条指令（＋收尾）位置都是**整个 `def` 语句**
                self.emit_named(*span, "LOAD_CONST", index as u8);
                // 3.14 的 `MAKE_FUNCTION` **没有 oparg**（`dis` 显示 `arg=None`）
                self.emit_named(*span, "MAKE_FUNCTION", 0);
                if annotated {
                    // bit4 `annotate`（`SPEC-bytecode.md` 的属性位表）
                    self.emit_named(*span, "SET_FUNCTION_ATTRIBUTE", 16);
                }
                if !kwdefaults.is_empty() {
                    // bit1 `kwdefaults`；实测的挂载次序是 **16 → 2 → 1**
                    self.emit_named(*span, "SET_FUNCTION_ATTRIBUTE", 2);
                }
                if !defaults.is_empty() {
                    // bit0 `defaults`（同一张位表）
                    self.emit_named(*span, "SET_FUNCTION_ATTRIBUTE", 1);
                }
                let name_index = self.intern_name(name);
                self.emit_named(*span, "STORE_NAME", name_index as u8);
                // 收尾两条跟 `def` 的整段（实测：`def f(): return 1` 的五条位置都是它）
                self.epilogue_span = *span;
                Ok(())
            }
        }
    }

    /// 造一个 **`__annotate__` 单元**（PEP 649 的 3.14 形态，逐条实测）。
    ///
    /// 形态：`format` 参数的守卫（`format > 2` ⇒ `NotImplementedError`）＋ 注解字典
    /// （键＝形参名，最后 `'return'`；值＝注解表达式）＋ `RETURN_VALUE`；
    /// `argcount = 1`、`varnames = ('format',)`、`flags = 0x3`。
    fn annotate_unit(
        &self,
        qualname: &str,
        parameters: &[Parameter],
        kwonly: &[Parameter],
        returns: Option<&Constant>,
        span: Span,
    ) -> CompiledUnit {
        let mut emitter = Emitter {
            mode: self.mode,
            tier: self.tier,
            qualname: qualname.to_owned(),
            boundary_out: None,
            deferred: Vec::new(),
            pending: Vec::new(),
            jumps: Vec::new(),
            labels: Vec::new(),
            if_implicit_return: false,
            in_condition: false,
            epilogue_needed: false,
            epilogue_span: span,
            last_span: span,
            kind: ScopeKind::Function,
            unit: CompiledUnit {
                name: "__annotate__".to_owned(),
                qualname: qualname.to_owned(),
                argcount: 1,
                posonlyargcount: 0,
                kwonlyargcount: 0,
                nlocals: 1,
                flags: 0x3,
                names: Vec::new(),
                varnames: vec!["format".to_owned()],
                cellvars: Vec::new(),
                freevars: Vec::new(),
                constants: Vec::new(),
                code: Vec::new(),
                positions: Vec::new(),
            },
        };
        // 实测：`__annotate__` 的 `RESUME` 取**合成**位点（`(def 行, def 行, 0, 0)`）
        emitter.emit_named(
            Span::new(span.line_start, span.line_start, 0, 0),
            "RESUME",
            0,
        );
        emitter.emit_named(span, "LOAD_FAST_BORROW", 0);
        // 实测：守卫的 `2` **也**在常量表里（下标 0），尽管指令用的是 `LOAD_SMALL_INT`
        emitter.intern_constant(Constant::Int(2));
        emitter.emit_named(span, "LOAD_SMALL_INT", 2);
        // 实测：`COMPARE_OP 132` 就是 `>`（比较下标 << 5 ｜ 提示位）
        emitter.emit_named(span, "COMPARE_OP", 132);
        let end = emitter.new_label();
        emitter.emit_jump(
            span,
            opcode::opcode("POP_JUMP_IF_FALSE").expect("表里有"),
            end,
        );
        emitter.emit_named(span, "NOT_TAKEN", 0);
        emitter.emit_named(span, "LOAD_COMMON_CONSTANT", 1);
        emitter.emit_named(span, "RAISE_VARARGS", 1);
        emitter.mark_label(end);
        let mut count = 0usize;
        // 注解字典按**声明次序**：位置参数 → 仅关键字 → `'return'`（实测）
        for parameter in parameters.iter().chain(kwonly.iter()) {
            let Some(annotation) = parameter.annotation.as_ref() else {
                continue;
            };
            let key = emitter.intern_constant(Constant::Str(parameter.name.clone()));
            emitter.emit_named(span, "LOAD_CONST", key as u8);
            emitter.emit_annotation_expression(annotation, span);
            count += 1;
        }
        if let Some(annotation) = returns {
            let key = emitter.intern_constant(Constant::Str("return".to_owned()));
            emitter.emit_named(span, "LOAD_CONST", key as u8);
            emitter.emit_annotation_expression(annotation, span);
            count += 1;
        }
        emitter.emit_named(span, "BUILD_MAP", count as u8);
        emitter.emit_named(span, "RETURN_VALUE", 0);
        emitter.flush_jumps();
        emitter.unit
    }

    /// 注解**表达式**的发射（实测）：类型名走 `LOAD_GLOBAL`（oparg ＝ `名字下标 << 1`）；
    /// `None` ⇒ `LOAD_CONST None`；`X[...]` ⇒ 先外后内再 `BINARY_OP 26`（`[]`）。
    fn emit_annotation_expression(&mut self, annotation: &Constant, span: Span) {
        match annotation {
            Constant::Type(name) if name == "NoneType" => {
                let index = self.intern_constant(Constant::None);
                self.emit_named(span, "LOAD_CONST", index as u8);
            }
            Constant::Type(name) | Constant::Str(name) => {
                let index = self.intern_name(name);
                self.emit_named(span, "LOAD_GLOBAL", (index << 1) as u8);
            }
            Constant::Tuple(parts) => {
                for part in parts {
                    self.emit_annotation_expression(part, span);
                }
                // 实测：`BINARY_OP 26` 的 argrepr 是 `[]`
                self.emit_named(span, "BINARY_OP", 26);
            }
            _ => {}
        }
    }

    /// 发一条**隐式** `LOAD_CONST None; RETURN_VALUE`（位置取最后一条真指令的）。
    fn emit_implicit_return(&mut self) {
        let index = self.intern_constant(Constant::None);
        let position = self.last_span;
        self.emit_at(
            position,
            opcode::opcode("LOAD_CONST").expect("LOAD_CONST 在表里"),
            index as u8,
        );
        self.emit_at(
            position,
            opcode::opcode("RETURN_VALUE").expect("RETURN_VALUE 在表里"),
            0,
        );
    }

    /// 发一段语句；`implicit_return` 为真时，若最后一条是 `if`，它的**每个分支**末尾
    /// 各补一条 `LOAD_CONST None; RETURN_VALUE`（实测：末尾的 `if` 会这样，非末尾的不会）。
    fn emit_block(
        &mut self,
        statements: &[Statement],
        _implicit_return: bool,
    ) -> Result<(), CompileError> {
        for statement in statements {
            self.emit_statement(statement)?;
        }
        Ok(())
    }

    fn emit_expression(&mut self, expression: &Expression) -> Result<(), CompileError> {
        match expression {
            Expression::Map(pairs, span) => {
                for (key, value) in pairs {
                    self.emit_expression(key)?;
                    self.emit_expression(value)?;
                }
                let count = u8::try_from(pairs.len()).map_err(|_| {
                    CompileError::Unsupported("字典字面量超过 255 对尚未接线".to_owned())
                })?;
                self.emit_named(*span, "BUILD_MAP", count);
                Ok(())
            }
            Expression::List(items, span) => {
                for item in items {
                    self.emit_expression(item)?;
                }
                let count = u8::try_from(items.len()).map_err(|_| {
                    CompileError::Unsupported("列表字面量超过 255 项尚未接线".to_owned())
                })?;
                self.emit_named(*span, "BUILD_LIST", count);
                Ok(())
            }
            Expression::Constant(constant, span) => {
                let index = self.intern_constant(constant.clone());
                self.emit_named(*span, "LOAD_CONST", index as u8);
                Ok(())
            }
            // **属性读**（实测）：`LOAD_FAST_BORROW 0; LOAD_ATTR <名字下标>`；
            // `LOAD_ATTR` 的 oparg 低位是"取方法"标志 ⇒ 纯取值就是 `下标 << 1`
            Expression::Attribute(target, name, _) => {
                self.emit_expression(target)?;
                let index = self.intern_name(name);
                self.emit_named(
                    target.span(),
                    "LOAD_ATTR",
                    (index << 1) as u8,
                );
                Ok(())
            }

            Expression::Int(value, span) => {
                if (0..=255).contains(value) {
                    self.intern_literal(Constant::Int(*value));
                    self.emit_at(
                        *span,
                        opcode::opcode("LOAD_SMALL_INT").expect("LOAD_SMALL_INT 在表里"),
                        *value as u8,
                    );
                } else {
                    let index = self.intern_constant(Constant::Int(*value));
                    self.emit_at(
                        *span,
                        opcode::opcode("LOAD_CONST").expect("LOAD_CONST 在表里"),
                        index as u8,
                    );
                }
                Ok(())
            }
            Expression::Str(text, span) => {
                let index = self.intern_constant(Constant::Str(text.clone()));
                self.emit_at(
                    *span,
                    opcode::opcode("LOAD_CONST").expect("LOAD_CONST 在表里"),
                    index as u8,
                );
                Ok(())
            }
            Expression::Bytes(value, span) => {
                // `bytes` 字面量与字符串同形：一条 `LOAD_CONST`（实例化时常量池里那项建 `BytesObject`）
                let index = self.intern_constant(Constant::Bytes(value.clone()));
                self.emit_at(
                    *span,
                    opcode::opcode("LOAD_CONST").expect("LOAD_CONST 在表里"),
                    index as u8,
                );
                Ok(())
            }
            Expression::Name(name, span) => {
                if self.kind == ScopeKind::Function {
                    if self.unit.varnames.iter().any(|item| item == name) {
                        let slot = self.slot_of(name);
                        self.emit_at(
                            *span,
                            opcode::opcode("LOAD_FAST_BORROW").expect("LOAD_FAST_BORROW 在表里"),
                            slot as u8,
                        );
                        return Ok(());
                    }
                    // **`LOAD_GLOBAL`**（`BC-57`）：函数里读非局部名走它——
                    // oparg 的低位是"压 NULL"标志 ⇒ 纯取值就是 `下标 << 1`（实测）
                    let index = self.intern_name(name);
                    self.emit_at(
                        *span,
                        opcode::opcode("LOAD_GLOBAL").expect("LOAD_GLOBAL 在表里"),
                        (index << 1) as u8,
                    );
                    return Ok(());
                }
                let index = self.intern_name(name);
                self.emit_at(
                    *span,
                    opcode::opcode("LOAD_NAME").expect("LOAD_NAME 在表里"),
                    index as u8,
                );
                Ok(())
            }
            Expression::Call {
                function,
                arguments,
                star_arguments,
                keywords,
                dict_arguments,
                callee_span,
                span,
            } => {
                // 实测形状（无关键字）：`<可调用>; PUSH_NULL; <实参…>; CALL <个数>`
                // 实测形状（带关键字）：`… ; <关键字值…>; LOAD_CONST <名元组>; CALL_KW <位置+关键字>`
                //
                // **例外（实测）**：**函数作用域里对全局名发调用**时，"压 NULL"由 `LOAD_GLOBAL`
                // 的**低位**承担（`LOAD_GLOBAL <下标 << 1 | 1>`），**不再**发单独的 `PUSH_NULL`
                // ——参照的 `def f(): raise ValueError(1)` 就是这样，而 `def f(): return g(1)`
                // 与模块级的 `LOAD_NAME; PUSH_NULL` 形态照旧。
                let global_callee = matches!(self.kind, ScopeKind::Function)
                    && matches!(
                        function.as_ref(),
                        Expression::Name(name, _)
                            if !self.unit.varnames.iter().any(|item| item == name)
                    );
                if global_callee {
                    if let Expression::Name(name, name_span) = function.as_ref() {
                        let index = self.intern_name(name);
                        self.emit_named(*name_span, "LOAD_GLOBAL", ((index << 1) | 1) as u8);
                    }
                } else {
                    self.emit_expression(function)?;
                    self.emit_at(
                        *callee_span,
                        opcode::opcode("PUSH_NULL").expect("PUSH_NULL 在表里"),
                        0,
                    );
                }
                for argument in arguments {
                    self.emit_expression(argument)?;
                }
                let total = arguments.len() + keywords.len() + star_arguments.len();
                if star_arguments.is_empty()
                    && dict_arguments.is_empty()
                    && keywords.is_empty()
                {
                    self.emit_at(
                        *span,
                        opcode::opcode("CALL").expect("CALL 在表里"),
                        u8::try_from(total).map_err(|_| {
                            CompileError::Unsupported("实参超过 255 个尚未接线".to_owned())
                        })?,
                    );
                    return Ok(());
                }
                if !star_arguments.is_empty() || !dict_arguments.is_empty() {
                    // **`CALL_FUNCTION_EX`**（实测四种形状）：
                    //   位置部分：没有 `*` 但有关键字 ⇒ `LOAD_CONST ()`；
                    //     只有一个 `*` 且没有前置位置实参 ⇒ 直接把那个可迭代对象交上去；
                    //     只有一个 `*` 且有前置位置实参 ⇒ `BUILD_LIST n; <* 对象>; LIST_EXTEND 1;
                    //       CALL_INTRINSIC_1 6`（LIST_TO_TUPLE）
                    //   关键字部分：`名字=值` 逐对压栈后 `BUILD_MAP <对数>`（一对都没有就先
                    //     `BUILD_MAP 0`），随后每个 `**` 压栈 ＋ `DICT_MERGE 1`；一个关键字都没有
                    //     就压 `PUSH_NULL`（"没有关键字"那一格）
                    if star_arguments.len() > 1 {
                        return Err(CompileError::Unsupported(
                            "多个 `*` 实参尚未接线".to_owned(),
                        ));
                    }
                    if star_arguments.is_empty() {
                        // 实测：这个空元组常量**收尾之后**才登记（`x = f(**d)` ⇒ `[None, ()]`）
                        // ⇒ 与折叠常量同一条路：先占位、收尾时回填
                        let argument_byte = self.unit.code.len() + 1;
                        self.emit_at(
                            *span,
                            opcode::opcode("LOAD_CONST").expect("LOAD_CONST 在表里"),
                            0,
                        );
                        self.pending
                            .push((argument_byte, Constant::Names(Vec::new())));
                    } else if arguments.is_empty() {
                        self.emit_expression(&star_arguments[0])?;
                    } else {
                        // 前置位置实参**已经压过栈了**（函数入口处统一压的），这里只把它们收进列表
                        self.emit_at(
                            *span,
                            opcode::opcode("BUILD_LIST").expect("BUILD_LIST 在表里"),
                            u8::try_from(arguments.len()).map_err(|_| {
                                CompileError::Unsupported("实参超过 255 个尚未接线".to_owned())
                            })?,
                        );
                        self.emit_expression(&star_arguments[0])?;
                        self.emit_at(
                            *span,
                            opcode::opcode("LIST_EXTEND").expect("LIST_EXTEND 在表里"),
                            1,
                        );
                        self.emit_at(
                            *span,
                            opcode::opcode("CALL_INTRINSIC_1")
                                .expect("CALL_INTRINSIC_1 在表里"),
                            6, // INTRINSIC_LIST_TO_TUPLE
                        );
                    }
                    if keywords.is_empty() && dict_arguments.is_empty() {
                        self.emit_at(
                            *span,
                            opcode::opcode("PUSH_NULL").expect("PUSH_NULL 在表里"),
                            0,
                        );
                    } else {
                        for (name, value) in keywords {
                            let index = self.intern_constant(Constant::Str(name.clone()));
                            self.emit_at(
                                *span,
                                opcode::opcode("LOAD_CONST").expect("LOAD_CONST 在表里"),
                                index as u8,
                            );
                            self.emit_expression(value)?;
                        }
                        self.emit_at(
                            *span,
                            opcode::opcode("BUILD_MAP").expect("BUILD_MAP 在表里"),
                            u8::try_from(keywords.len()).map_err(|_| {
                                CompileError::Unsupported(
                                    "关键字超过 255 个尚未接线".to_owned(),
                                )
                            })?,
                        );
                        for source in dict_arguments {
                            self.emit_expression(source)?;
                            self.emit_at(
                                *span,
                                opcode::opcode("DICT_MERGE").expect("DICT_MERGE 在表里"),
                                1,
                            );
                        }
                    }
                    self.emit_at(
                        *span,
                        opcode::opcode("CALL_FUNCTION_EX")
                            .expect("CALL_FUNCTION_EX 在表里"),
                        0,
                    );
                    return Ok(());
                }
                {
                    for (_, value) in keywords {
                        self.emit_expression(value)?;
                    }
                    let names: Vec<String> =
                        keywords.iter().map(|(name, _)| name.clone()).collect();
                    let _ = &names;
                    let index = self.intern_constant(Constant::Names(names));
                    self.emit_at(
                        *span,
                        opcode::opcode("LOAD_CONST").expect("LOAD_CONST 在表里"),
                        index as u8,
                    );
                    self.emit_at(
                        *span,
                        opcode::opcode("CALL_KW").expect("CALL_KW 在表里"),
                        u8::try_from(total).map_err(|_| {
                            CompileError::Unsupported("实参超过 255 个尚未接线".to_owned())
                        })?,
                    );
                }
                Ok(())
            }
            Expression::Compare(left, operator, right, span) => {
                self.emit_expression(left)?;
                self.emit_expression(right)?;
                // 条件里的比较要多带 `bool(...)` 位（16）——实测 `while a < b` ⇒ 18
                let oparg = if self.in_condition {
                    operator.oparg() | 16
                } else {
                    operator.oparg()
                };
                self.emit_at(
                    *span,
                    opcode::opcode("COMPARE_OP").expect("COMPARE_OP 在表里"),
                    oparg,
                );
                Ok(())
            }
            Expression::Add(left, right, span) => {
                // **常量折叠**（实测三条规则）：只有最左叶子进常量表；结果是小整数走
                // `LOAD_SMALL_INT` 不进表；否则该常量在收尾之后才登记。**折叠出的加载位置是
                // 整段表达式**（实测 `x = 1 + 2` ⇒ `(1,1,4,9)`）。
                if let Some(folded) = fold_constant(expression)? {
                    if let Some(leaf) = leftmost_literal(expression) {
                        self.intern_literal(leaf);
                    }
                    match folded {
                        Constant::Int(value) if (0..=255).contains(&value) => {
                            self.emit_at(
                                *span,
                                opcode::opcode("LOAD_SMALL_INT")
                                    .expect("LOAD_SMALL_INT 在表里"),
                                value as u8,
                            );
                        }
                        other => {
                            let argument_byte = self.unit.code.len() + 1;
                            self.emit_at(
                                *span,
                                opcode::opcode("LOAD_CONST").expect("LOAD_CONST 在表里"),
                                0,
                            );
                            self.pending.push((argument_byte, other));
                        }
                    }
                    return Ok(());
                }
                // 实测：两侧都是**局部**借入加载时打成超指令（位置取**先压的那个**名字）
                let pack = match (self.kind, &**left, &**right) {
                    (ScopeKind::Function, Expression::Name(a, _), Expression::Name(b, _)) => {
                        let slots = &self.unit.varnames;
                        match (
                            slots.iter().position(|item| item == a),
                            slots.iter().position(|item| item == b),
                        ) {
                            (Some(first), Some(second)) => Some((first, second)),
                            _ => None,
                        }
                    }
                    _ => None,
                };
                if let Some((first, second)) = pack {
                    self.emit_at(
                        left.span(),
                        opcode::opcode("LOAD_FAST_BORROW_LOAD_FAST_BORROW")
                            .expect("超指令在表里"),
                        ((first << 4) | second) as u8,
                    );
                } else {
                    self.emit_expression(left)?;
                    self.emit_expression(right)?;
                }
                let plus = crate::opcode::get_nb_ops()
                    .iter()
                    .position(|entry| entry.1 == "+")
                    .expect("nb_ops 里应当有 +") as u8;
                // 实测：`BINARY_OP` 的位置是**整段 `a + b`**
                self.emit_at(
                    *span,
                    opcode::opcode("BINARY_OP").expect("BINARY_OP 在表里"),
                    plus,
                );
                Ok(())
            }
        }
    }
}

// ---- 语法树 ----

#[derive(Debug, Clone, PartialEq, Eq)]
enum Expression {
    Int(i64, Span),
    Str(String, Span),
    /// **`bytes` 字面量**（`P1-12`）。
    Bytes(Vec<u8>, Span),
    Name(String, Span),
    /// **字面量**（`None` 起；`True`／`False` 要等 `Constant::Bool`）。
    /// 发射就是 `LOAD_CONST <常量下标>`（实测：`x = None` ⇒ 常量表 `['None']`）。
    Constant(crate::compile::Constant, Span),
    /// **列表字面量**（`[]`／`[1, 2]`）。实测发射：元素按序先发，再 `BUILD_LIST <个数>`
    /// （`BUILD_LIST` ＝ 46，见 `opcode_metadata.rs`；执行器早就实现了它）。
    List(Vec<Expression>, Span),
    /// **字典字面量**（`{}`／`{1: 2}`）。实测发射：**键先值后**，再 `BUILD_MAP <对数>`。
    Map(Vec<(Expression, Expression)>, Span),
    /// 属性访问 `对象.名字`（`LOAD_ATTR`／`STORE_ATTR` 的 `names` 下标）。
    Attribute(Box<Expression>, String, Span),
    Add(Box<Expression>, Box<Expression>, Span),
    /// 比较（`COMPARE_OP` 的 oparg 逐运算符实测：`下标 << 5 | 提示位`）。
    Compare(Box<Expression>, CompareOperator, Box<Expression>, Span),
    /// 调用：`函数(实参…)`。`callee_span` 是被调用者自己的跨度（`PUSH_NULL` 用它），
    /// `span` 是**整段调用**（`CALL`／`CALL_KW` 用）。
    Call {
        function: Box<Expression>,
        /// 位置实参（在 `*` 之前）。
        arguments: Vec<Expression>,
        /// `*expr`（本层只接线一个）。
        star_arguments: Vec<Expression>,
        /// `名字=值`。
        keywords: Vec<(String, Expression)>,
        /// `**expr`。
        dict_arguments: Vec<Expression>,
        callee_span: Span,
        span: Span,
    },
}

/// 本层接线的比较运算符（`dis` 的 `cmp_op` 下标与实测提示位见 [`CompareOperator::oparg`]）。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum CompareOperator {
    Less,
    LessEqual,
    Equal,
    NotEqual,
    Greater,
    GreaterEqual,
}

impl CompareOperator {
    /// `COMPARE_OP` 的 oparg（**实测**：`<` ⇒ 2、`<=` ⇒ 42、`==` ⇒ 72、`!=` ⇒ 103、
    /// `>` ⇒ 132、`>=` ⇒ 172）。
    fn oparg(self) -> u8 {
        match self {
            CompareOperator::Less => 2,
            CompareOperator::LessEqual => 42,
            CompareOperator::Equal => 72,
            CompareOperator::NotEqual => 103,
            CompareOperator::Greater => 132,
            CompareOperator::GreaterEqual => 172,
        }
    }
}

impl Expression {
    fn span(&self) -> Span {
        match self {
            Expression::Int(_, span)
            | Expression::Str(_, span)
            | Expression::Bytes(_, span)
            | Expression::Name(_, span)
            | Expression::Constant(_, span)
            | Expression::List(_, span)
            | Expression::Map(_, span)
            | Expression::Attribute(_, _, span)
            | Expression::Add(_, _, span)
            | Expression::Compare(_, _, _, span)
            | Expression::Call { span, .. } => *span,
        }
    }
}

/// 一个**形参**：名字 ＋（可选）注解 ＋（可选）默认值。
#[derive(Clone, Debug, PartialEq, Eq)]
struct Parameter {
    /// 形参名。
    name: String,
    /// `BC-*`：是不是**仅位置**形参（`/` 之前的那些）。
    posonly: bool,
    /// 注解（标签常量；`None` ⇒ 没写注解）。
    annotation: Option<Constant>,
    /// 默认值表达式（`None` ⇒ 没有默认值）。
    default: Option<Expression>,
}

/// 模块级／缩进块里的语句。
#[derive(Debug, Clone, PartialEq, Eq)]
enum Statement {
    Assign {
        target: String,
        target_span: Span,
        value: Expression,
        span: Span,
    },
    Return(Expression, Span),
    /// 表达式语句（本层只接线调用：算完 `POP_TOP` 丢掉）。
    Expression(Expression, Span),
    /// `for <目标> in <可迭代>: <体>`。
    For {
        span: Span,
        target: String,
        target_span: Span,
        iterable: Expression,
        body: Vec<Statement>,
        /// `else` 体（空表示没有 `else`）。
        else_body: Vec<Statement>,
    },
    /// `while <条件>: <体>`。
    While {
        span: Span,
        condition: Expression,
        body: Vec<Statement>,
        /// `else` 体（空表示没有 `else`）。
        else_body: Vec<Statement>,
    },
    /// `if <条件>: <体> [else: <体>]`（`else_body` 为空表示没有 else）。
    If {
        span: Span,
        condition: Expression,
        then_body: Vec<Statement>,
        else_body: Vec<Statement>,
    },
    /// `raise [表达式 [from 表达式]]`（`RAISE_VARARGS`：0 裸重抛／1 带值／2 带因）。
    Raise {
        value: Option<Expression>,
        cause: Option<Expression>,
        span: Span,
    },
    /// **属性赋值**：`对象.名字 = 表达式`（`STORE_ATTR`；实测**先值后对象**）。
    /// 单独一个变体而不是把 `Assign` 的目标改成表达式——目标类型是 `String`，
    /// 改它要动解析器/发射器/各处 match，收益一样但风险大。
    AssignAttr {
        /// 被赋属性的对象（`self` 这一层）。
        object: Expression,
        /// 属性名。
        name: String,
        /// 右值。
        value: Expression,
        /// 整条语句的跨度。
        span: Span,
    },
    /// `class <名字> [(<基类…>)]: <体>`
    Class {
        name: String,
        span: Span,
        first_line: u32,
        /// 基类表达式（`class C(B, m.C)` 里那些）。
        bases: Vec<Expression>,
        body: Vec<Statement>,
    },
    Def {
        name: String,
        span: Span,
        first_line: u32,
        /// 形参表（名字 ＋ 注解 ＋ 默认值）。
        parameters: Vec<Parameter>,
        /// **仅关键字**形参（裸 `*` 或 `*args` 之后的那些）。
        kwonly: Vec<Parameter>,
        /// `*args` 的名字（`None` ⇒ 没有）。
        varargs: Option<String>,
        /// `**kw` 的名字（`None` ⇒ 没有）。
        varkw: Option<String>,
        /// 返回注解（标签常量）。
        returns: Option<Constant>,
        body: Vec<Statement>,
    },
}

/// 把一段**全常量**表达式求值（`+` 的常量折叠）；不是全常量给 `None`。
fn fold_constant(expression: &Expression) -> Result<Option<Constant>, CompileError> {
    match expression {
        Expression::Int(value, _) => Ok(Some(Constant::Int(*value))),
        Expression::Str(text, _) => Ok(Some(Constant::Str(text.clone()))),
        Expression::Bytes(value, _) => Ok(Some(Constant::Bytes(value.clone()))),
        Expression::Constant(constant, _) => Ok(Some(constant.clone())),
        // 列表**不是**编译期常量（实测：`x = [1, 2]` 的常量表里没有列表本身）
        Expression::List(_, _) => Ok(None),
        Expression::Map(_, _) => Ok(None),
        Expression::Name(_, _)
        | Expression::Attribute(_, _, _)
        | Expression::Compare(_, _, _, _)
        | Expression::Call { .. } => Ok(None),
        Expression::Add(left, right, _) => {
            let (Some(left_value), Some(right_value)) =
                (fold_constant(left)?, fold_constant(right)?)
            else {
                return Ok(None);
            };
            match (left_value, right_value) {
                (Constant::Int(x), Constant::Int(y)) => {
                    // 折叠结果装不进 `i64`：**不折**（`Ok(None)`）⇒ 交给运行期算。
                    // `TS-45` 的任意精度在**运行期**（`P1-11` 已落地），而常量池里的整数字面量
                    // 仍只有 `Constant::Int(i64)` ⇒ 参照会把 `i64::MAX + 1` 折成大整数常量，
                    // 我们不折（**语义等价、指令流不同**；已登记在 `PLAN` 的 `P1-12` 行附近）。
                    match x.checked_add(y) {
                        Some(sum) => Ok(Some(Constant::Int(sum))),
                        None => Ok(None),
                    }
                }
                (Constant::Str(x), Constant::Str(y)) => Ok(Some(Constant::Str(x + &y))),
                // 实测：`b'ab' + b'cd'` 也在**编译期**折成 `b'abcd'`（与字符串同一条路）
                (Constant::Bytes(x), Constant::Bytes(y)) => {
                    let mut joined = x;
                    joined.extend_from_slice(&y);
                    Ok(Some(Constant::Bytes(joined)))
                }
                _ => Ok(None),
            }
        }
    }
}

/// 全常量表达式的**最左叶子**（实测：折叠时只有它进常量表）。
fn leftmost_literal(expression: &Expression) -> Option<Constant> {
    match expression {
        Expression::Int(value, _) => Some(Constant::Int(*value)),
        Expression::Str(text, _) => Some(Constant::Str(text.clone())),
        Expression::Bytes(value, _) => Some(Constant::Bytes(value.clone())),
        Expression::Constant(constant, _) => Some(constant.clone()),
        Expression::List(_, _) => None,
        Expression::Map(_, _) => None,
        Expression::Name(_, _)
        | Expression::Attribute(_, _, _)
        | Expression::Compare(_, _, _, _)
        | Expression::Call { .. } => None,
        Expression::Add(left, _, _) => leftmost_literal(left),
    }
}

// ---- 词法（**带行列**：`BC-18` 的位置表要它） ----

#[derive(Debug, Clone, PartialEq, Eq)]
enum Lexeme {
    Name(String),
    Int(i64),
    Str(String),
    /// **`bytes` 字面量**（`P1-12`／`b'…'`）：转义在**词法**这一层就解成字节。
    Bytes(Vec<u8>),
    Assign,
    Plus,
    /// `.`（属性访问）
    Dot,
    Colon,
    LeftParen,
    RightParen,
    Comma,
    Newline,
    Indent,
    Dedent,
    Return,
    Def,
    /// `class`
    Class,
    /// `raise`
    Raise,
    If,
    Else,
    While,
    For,
    In,
    Star,
    DoubleStar,
    Arrow,
    LeftBracket,
    LeftBrace,
    RightBracket,
    RightBrace,
    Less,
    LessEqual,
    Greater,
    GreaterEqual,
    EqualEqual,
    NotEqual,
    End,
    /// `/`（形参表里的仅位置分隔符；除法未接线）
    Slash,
}

/// 词法结果：单元 ＋ 与它**一一对应**的跨度。
struct Lexed {
    lexemes: Vec<Lexeme>,
    spans: Vec<Span>,
}

/// 解一个 `bytes` 字面量里的转义（`characters` 从**反斜杠**那一位开始）。
///
/// 回 `(字节, 吃掉几个字符)`。支持集＝参照实测里出现过的那批：`\n \t \r \\ \' \" \a \b \f \v`、
/// `\xNN`（**两位**十六进制）、`\ooo`（一至三位八进制）。其余如实报**未实现**——`\u`／`\U`／
/// `\N{}` 在 bytes 里的口径没实测过，不猜。
fn lex_bytes_escape(characters: &[char], position: usize) -> Result<(u8, usize), CompileError> {
    let simple = |byte: u8| Ok((byte, 2));
    match characters.get(1) {
        Some('n') => simple(b'\n'),
        Some('t') => simple(b'\t'),
        Some('r') => simple(b'\r'),
        Some('\\') => simple(b'\\'),
        Some('\'') => simple(b'\''),
        Some('"') => simple(b'"'),
        Some('a') => simple(0x07),
        Some('b') => simple(0x08),
        Some('f') => simple(0x0c),
        Some('v') => simple(0x0b),
        Some('x') => {
            let digits: String = characters
                .iter()
                .skip(2)
                .take(2)
                .take_while(|character| character.is_ascii_hexdigit())
                .collect();
            if digits.len() != 2 {
                // 实测：`b'\x1'` ⇒ `(value error) invalid \x escape at position 0`
                // （位置是反斜杠相对**字面量内容**起点的下标；`b'a\x1'` ⇒ 1）
                return Err(CompileError::Syntax(format!(
                    "(value error) invalid \\x escape at position {position}"
                )));
            }
            let byte = u8::from_str_radix(&digits, 16)
                .map_err(|_| CompileError::Syntax("invalid \\x escape".to_owned()))?;
            Ok((byte, 4))
        }
        Some(first) if first.is_digit(8) => {
            let digits: String = characters
                .iter()
                .skip(1)
                .take(3)
                .take_while(|character| character.is_digit(8))
                .collect();
            let code = u32::from_str_radix(&digits, 8)
                .map_err(|_| CompileError::Syntax("invalid octal escape".to_owned()))?;
            if code > 0xFF {
                return Err(CompileError::Syntax("octal escape out of range".to_owned()));
            }
            Ok((code as u8, 1 + digits.chars().count()))
        }
        Some(other) => Err(CompileError::Unsupported(format!(
            "bytes 字面量里的转义 \\{other} 尚未接线"
        ))),
        None => Err(CompileError::Syntax(
            "unterminated string literal (detected at line 1)".to_owned(),
        )),
    }
}

fn lex(source: &str) -> Result<Lexed, CompileError> {
    let characters: Vec<char> = source.chars().collect();
    let mut index = 0usize;
    let mut lexemes = Vec::new();
    let mut spans = Vec::new();
    let mut indents: Vec<usize> = vec![0];
    let mut at_line_start = true;
    let mut line: u32 = 1;
    let mut line_start_index = 0usize;
    // 注意：这里**不能**用闭包借 `line_start_index`（后面要改它）⇒ 写成宏式的内联表达式
    macro_rules! column {
        ($index:expr) => {
            ($index - line_start_index) as u32
        };
    }
    while index < characters.len() {
        if at_line_start {
            let mut width = 0usize;
            while matches!(characters.get(index), Some(' ')) {
                width += 1;
                index += 1;
            }
            if matches!(characters.get(index), Some('\n') | None) {
                if let Some('\n') = characters.get(index) {
                    index += 1;
                    line += 1;
                    line_start_index = index;
                }
                continue;
            }
            let current = *indents.last().expect("至少有一个");
            let zero = Span::new(line, line, 0, 0);
            if width > current {
                indents.push(width);
                lexemes.push(Lexeme::Indent);
                spans.push(zero);
            } else if width < current {
                while *indents.last().expect("至少有一个") > width {
                    indents.pop();
                    lexemes.push(Lexeme::Dedent);
                    spans.push(zero);
                }
                if *indents.last().expect("至少有一个") != width {
                    return Err(CompileError::Syntax(format!("缩进对不齐：{width}")));
                }
            }
            at_line_start = false;
        }
        let character = characters[index];
        match character {
            ' ' | '\r' => index += 1,
            '\t' => return Err(CompileError::Unsupported("制表符缩进尚未接线".to_owned())),
            '\n' => {
                lexemes.push(Lexeme::Newline);
                spans.push(Span::new(line, line, column!(index), column!(index)));
                index += 1;
                line += 1;
                line_start_index = index;
                at_line_start = true;
            }
            '<' | '>' | '=' | '!' => {
                // 两字符比较要从**当前**位置看下一个字符
                let next = characters.get(index + 1).copied();
                let (lexeme, width) = match (character, next) {
                    ('<', Some('=')) => (Lexeme::LessEqual, 2),
                    ('>', Some('=')) => (Lexeme::GreaterEqual, 2),
                    ('=', Some('=')) => (Lexeme::EqualEqual, 2),
                    ('!', Some('=')) => (Lexeme::NotEqual, 2),
                    ('<', _) => (Lexeme::Less, 1),
                    ('>', _) => (Lexeme::Greater, 1),
                    ('=', _) => (Lexeme::Assign, 1),
                    _ => return Err(CompileError::Syntax("孤立的 `!`".to_owned())),
                };
                let start = column!(index);
                lexemes.push(lexeme);
                spans.push(Span::new(line, line, start, start + width as u32));
                index += width;
            }
            '/' => {
                // `/`：目前只用于形参表里的"仅位置形参"分隔符（除法仍未接线）
                let start = column!(index);
                lexemes.push(Lexeme::Slash);
                spans.push(Span::new(line, line, start, start + 1));
                index += 1;
            }
            '-' => {
                // `->`（返回注解）；本层**不支持**负数与减法（如实报未接线）
                if characters.get(index + 1) == Some(&'>') {
                    let start = column!(index);
                    lexemes.push(Lexeme::Arrow);
                    spans.push(Span::new(line, line, start, start + 2));
                    index += 2;
                } else {
                    return Err(CompileError::Unsupported(
                        "负号／减法尚未接线（本层只做注解里的 `->`）".to_owned(),
                    ));
                }
            }
            '{' => {
                let start = column!(index);
                lexemes.push(Lexeme::LeftBrace);
                spans.push(Span::new(line, line, start, start + 1));
                index += 1;
            }
            '}' => {
                let start = column!(index);
                lexemes.push(Lexeme::RightBrace);
                spans.push(Span::new(line, line, start, start + 1));
                index += 1;
            }
            '[' => {
                let start = column!(index);
                lexemes.push(Lexeme::LeftBracket);
                spans.push(Span::new(line, line, start, start + 1));
                index += 1;
            }
            ']' => {
                let start = column!(index);
                lexemes.push(Lexeme::RightBracket);
                spans.push(Span::new(line, line, start, start + 1));
                index += 1;
            }
            '*' => {
                let start = column!(index);
                let width: usize = if characters.get(index + 1) == Some(&'*') { 2 } else { 1 };
                lexemes.push(if width == 2 {
                    Lexeme::DoubleStar
                } else {
                    Lexeme::Star
                });
                spans.push(Span::new(line, line, start, start + width as u32));
                index += width;
            }
            '.' | '+' | ':' | '(' | ')' | ',' | ';' => {
                let start = column!(index);
                lexemes.push(match character {
                    '=' => Lexeme::Assign,
                    '.' => Lexeme::Dot,
                    '+' => Lexeme::Plus,
                    ':' => Lexeme::Colon,
                    '(' => Lexeme::LeftParen,
                    ')' => Lexeme::RightParen,
                    ',' => Lexeme::Comma,
                    _ => Lexeme::Newline,
                });
                spans.push(Span::new(line, line, start, start + 1));
                index += 1;
            }
            '#' => {
                while !matches!(characters.get(index), Some('\n') | None) {
                    index += 1;
                }
            }
            // 单双引号**等价**（实测的语料里两种都有；转义仍未接线）
            '\'' | '"' => {
                let quote = characters[index];
                let start = column!(index);
                index += 1;
                let mut text = String::new();
                loop {
                    match characters.get(index) {
                        Some(character) if *character == quote => {
                            index += 1;
                            break;
                        }
                        Some('\\') => {
                            return Err(CompileError::Unsupported(
                                "字符串转义尚未接线".to_owned(),
                            ))
                        }
                        Some(character) => {
                            text.push(*character);
                            index += 1;
                        }
                        None => {
                            return Err(CompileError::Syntax("字符串没有收尾引号".to_owned()))
                        }
                    }
                }
                lexemes.push(Lexeme::Str(text));
                spans.push(Span::new(line, line, start, column!(index)));
            }
            character if character.is_ascii_digit() => {
                let start = column!(index);
                let start_index = index;
                while index < characters.len() && characters[index].is_ascii_digit() {
                    index += 1;
                }
                let text: String = characters[start_index..index].iter().collect();
                let value = text
                    .parse::<i64>()
                    .map_err(|_| CompileError::Unsupported(format!("整数 {text} 超出本层范围")))?;
                lexemes.push(Lexeme::Int(value));
                spans.push(Span::new(line, line, start, column!(index)));
            }
            character if character.is_alphabetic() || character == '_' => {
                // `b'…'`／`B"…"`：**先**看前缀（否则会先被当成名字 `b`）。
                // 转义在这一层就解成**字节**；非 ASCII 字符照参照报 `SyntaxError`。
                if matches!(character, 'b' | 'B')
                    && matches!(characters.get(index + 1), Some('\'') | Some('"'))
                {
                    let start = column!(index);
                    let quote = characters[index + 1];
                    index += 2;
                    // 转义报错里的 `position` 是**相对字面量内容**的下标（实测口径）
                    let content_start = index;
                    let mut value: Vec<u8> = Vec::new();
                    loop {
                        match characters.get(index) {
                            Some(current) if *current == quote => {
                                index += 1;
                                break;
                            }
                            Some('\\') => {
                                let (byte, width) =
                                    lex_bytes_escape(&characters[index..], index - content_start)?;
                                value.push(byte);
                                index += width;
                            }
                            Some(current) if current.is_ascii() => {
                                value.push(*current as u8);
                                index += 1;
                            }
                            Some(_) => {
                                return Err(CompileError::Syntax(
                                    "bytes can only contain ASCII literal characters".to_owned(),
                                ))
                            }
                            None => {
                                return Err(CompileError::Syntax(
                                    "unterminated string literal (detected at line 1)".to_owned(),
                                ))
                            }
                        }
                    }
                    lexemes.push(Lexeme::Bytes(value));
                    spans.push(Span::new(line, line, start, column!(index)));
                    continue;
                }
                let start = column!(index);
                let start_index = index;
                while index < characters.len()
                    && (characters[index].is_alphanumeric() || characters[index] == '_')
                {
                    index += 1;
                }
                let text: String = characters[start_index..index].iter().collect();
                let end = column!(index);
                lexemes.push(match text.as_str() {
                    "return" => Lexeme::Return,
                    "raise" => Lexeme::Raise,
                    "def" => Lexeme::Def,
                    "class" => Lexeme::Class,
                    "if" => Lexeme::If,
                    "while" => Lexeme::While,
                    "for" => Lexeme::For,
                    "in" => Lexeme::In,
                    "else" => Lexeme::Else,
                    _ => Lexeme::Name(text),
                });
                spans.push(Span::new(line, line, start, end));
            }
            other => return Err(CompileError::Syntax(format!("不认识的字符 {other:?}"))),
        }
    }
    let end_span = Span::new(line, line, column!(index), column!(index));
    while indents.len() > 1 {
        indents.pop();
        lexemes.push(Lexeme::Dedent);
        spans.push(end_span);
    }
    lexemes.push(Lexeme::End);
    spans.push(end_span);
    Ok(Lexed { lexemes, spans })
}

// ---- 语法 ----

fn parse_module(lexed: &Lexed) -> Result<Vec<Statement>, CompileError> {
    let mut cursor = 0usize;
    let statements = parse_statements(lexed, &mut cursor, 0, false)?;
    if lexed.lexemes.get(cursor) != Some(&Lexeme::End) {
        return Err(CompileError::Syntax(format!(
            "模块结尾多出了 {:?}",
            lexed.lexemes.get(cursor)
        )));
    }
    Ok(statements)
}

fn parse_statements(
    lexed: &Lexed,
    cursor: &mut usize,
    depth: usize,
    in_function: bool,
) -> Result<Vec<Statement>, CompileError> {
    let tokens = &lexed.lexemes;
    let mut statements = Vec::new();
    loop {
        while matches!(tokens.get(*cursor), Some(Lexeme::Newline)) {
            *cursor += 1;
        }
        match tokens.get(*cursor) {
            Some(Lexeme::End) => break,
            Some(Lexeme::Dedent) => {
                if depth == 0 {
                    return Err(CompileError::Syntax("多余的缩进收尾".to_owned()));
                }
                break;
            }
            // `class <名字> [(<基类…>)]: <体>`
            Some(Lexeme::Class) => {
                let class_span = lexed.spans[*cursor];
                let first_line = class_span.line_start;
                *cursor += 1;
                let name = match tokens.get(*cursor) {
                    Some(Lexeme::Name(name)) => name.clone(),
                    other => {
                        return Err(CompileError::Syntax(format!(
                            "`class` 后面要名字，实际 {other:?}"
                        )))
                    }
                };
                *cursor += 1;
                // 基类（可省略括号；实测基类用 `LOAD_NAME` 压栈）
                let mut bases: Vec<Expression> = Vec::new();
                if tokens.get(*cursor) == Some(&Lexeme::LeftParen) {
                    *cursor += 1;
                    loop {
                        match tokens.get(*cursor) {
                            Some(Lexeme::RightParen) => {
                                *cursor += 1;
                                break;
                            }
                            Some(Lexeme::Comma) => {
                                *cursor += 1;
                            }
                            _ => {
                                let (expression, next) =
                                    parse_expression(lexed, *cursor)?;
                                *cursor = next;
                                bases.push(expression);
                            }
                        }
                    }
                }
                if tokens.get(*cursor) != Some(&Lexeme::Colon) {
                    return Err(CompileError::Syntax("`class` 后面要冒号".to_owned()));
                }
                *cursor += 1;
                if tokens.get(*cursor) != Some(&Lexeme::Newline) {
                    return Err(CompileError::Syntax("`class` 的冒号后面要换行".to_owned()));
                }
                *cursor += 1;
                if tokens.get(*cursor) != Some(&Lexeme::Indent) {
                    return Err(CompileError::Syntax("`class` 的体要缩进".to_owned()));
                }
                *cursor += 1;
                let body = parse_statements(lexed, cursor, depth + 1, in_function)?;
                if tokens.get(*cursor) != Some(&Lexeme::Dedent) {
                    return Err(CompileError::Syntax("`class` 的体没有正常收尾".to_owned()));
                }
                let body_end = statements_last_end(&body).unwrap_or(class_span);
                let span = class_span.to(body_end);
                *cursor += 1;
                statements.push(Statement::Class {
                    name,
                    span,
                    first_line,
                    bases,
                    body,
                });
            }
            Some(Lexeme::Def) => {
                if in_function {
                    return Err(CompileError::Unsupported("嵌套的函数定义尚未接线".to_owned()));
                }
                let def_span = lexed.spans[*cursor];
                let first_line = def_span.line_start;
                *cursor += 1;
                let name = match tokens.get(*cursor) {
                    Some(Lexeme::Name(name)) => name.clone(),
                    other => {
                        return Err(CompileError::Syntax(format!(
                            "`def` 后面要名字，实际 {other:?}"
                        )))
                    }
                };
                *cursor += 1;
                if tokens.get(*cursor) != Some(&Lexeme::LeftParen) {
                    return Err(CompileError::Unsupported(
                        "`def` 只接线了带括号的形参表".to_owned(),
                    ));
                }
                *cursor += 1;
                // 形参表：`名字 [":" 注解] ["=" 默认值]`，逗号分隔
                // `*args`／`**kw` 已支持；裸 `*` 之后的**仅关键字形参**与 `**kw` 后面的形参仍未接
                let mut parameters: Vec<Parameter> = Vec::new();
                let mut kwonly: Vec<Parameter> = Vec::new();
                let mut varargs: Option<String> = None;
                let mut varkw: Option<String> = None;
                // 裸 `*`（或 `*args`）之后就是**仅关键字**形参
                let mut after_star = false;
                let mut expect_parameter = true;
                loop {
                    match tokens.get(*cursor) {
                        Some(Lexeme::RightParen) => {
                            *cursor += 1;
                            break;
                        }
                        // `*args`／`**kw`（`BC-56` 的签名元数据；`varnames` 排在最后两位）
                        Some(Lexeme::Star) => {
                            *cursor += 1;
                            // `*名字` ⇒ `*args`；裸 `*`（后面是逗号或 `)`）⇒ 只是引出仅关键字形参
                            if let Some(Lexeme::Name(name)) = tokens.get(*cursor) {
                                varargs = Some(name.clone());
                                *cursor += 1;
                            }
                            after_star = true;
                            expect_parameter = false;
                        }
                        // `/`：把**它前面**那些位置形参标成"仅位置"
                        Some(Lexeme::Slash) => {
                            *cursor += 1;
                            if after_star || parameters.iter().any(|item| item.posonly) {
                                return Err(CompileError::Syntax(
                                    "`/` 只能出现在位置形参之后、且只能出现一次".to_owned(),
                                ));
                            }
                            for parameter in parameters.iter_mut() {
                                parameter.posonly = true;
                            }
                            expect_parameter = false;
                        }
                        Some(Lexeme::DoubleStar) => {
                            *cursor += 1;
                            match tokens.get(*cursor) {
                                Some(Lexeme::Name(name)) => {
                                    varkw = Some(name.clone());
                                    *cursor += 1;
                                }
                                other => {
                                    return Err(CompileError::Syntax(format!(
                                        "`**` 后面要一个名字，实际 {other:?}"
                                    )))
                                }
                            }
                            expect_parameter = false;
                        }
                        Some(Lexeme::Name(parameter)) if expect_parameter => {
                            let name = parameter.clone();
                            *cursor += 1;
                            let annotation = if tokens.get(*cursor) == Some(&Lexeme::Colon) {
                                let (label, next) = parse_type_at(lexed, *cursor + 1)?;
                                *cursor = next;
                                Some(label)
                            } else {
                                None
                            };
                            // 默认值：`= <表达式>`（`BC-*`：默认值在 **def 那一刻**求值）
                            let default = if tokens.get(*cursor) == Some(&Lexeme::Assign) {
                                let (expression, next) = parse_expression(lexed, *cursor + 1)?;
                                *cursor = next;
                                Some(expression)
                            } else {
                                None
                            };
                            if varkw.is_some() {
                                return Err(CompileError::Syntax(
                                    "`**kw` 之后不能再有形参".to_owned(),
                                ));
                            }
                            let parameter = Parameter {
                                name,
                                posonly: false,
                                annotation,
                                default,
                            };
                            if after_star {
                                kwonly.push(parameter);
                            } else {
                                parameters.push(parameter);
                            }
                            expect_parameter = false;
                        }
                        Some(Lexeme::Comma) if !expect_parameter => {
                            *cursor += 1;
                            expect_parameter = true;
                        }
                        other => {
                            return Err(CompileError::Syntax(format!("形参表里出现 {other:?}")))
                        }
                    }
                }
                // 返回注解：`-> 类型`
                let returns = if tokens.get(*cursor) == Some(&Lexeme::Arrow) {
                    let (label, next) = parse_type_at(lexed, *cursor + 1)?;
                    *cursor = next;
                    Some(label)
                } else {
                    None
                };
                if tokens.get(*cursor) != Some(&Lexeme::Colon) {
                    return Err(CompileError::Syntax("`def` 后面要冒号".to_owned()));
                }
                *cursor += 1;
                if tokens.get(*cursor) != Some(&Lexeme::Newline) {
                    return Err(CompileError::Syntax("`def` 的冒号后面要换行".to_owned()));
                }
                *cursor += 1;
                if tokens.get(*cursor) != Some(&Lexeme::Indent) {
                    return Err(CompileError::Syntax("`def` 的体要缩进".to_owned()));
                }
                *cursor += 1;
                let body = parse_statements(lexed, cursor, depth + 1, true)?;
                if tokens.get(*cursor) != Some(&Lexeme::Dedent) {
                    return Err(CompileError::Syntax("`def` 的体没有正常收尾".to_owned()));
                }
                // `def` 的整段：从 `def` 关键字到**体最后一行的行尾**（实测 `(1, 2, 0, 12)`）
                let body_end = statements_last_end(&body).unwrap_or(def_span);
                let span = def_span.to(body_end);
                *cursor += 1;
                statements.push(Statement::Def {
                    name,
                    span,
                    first_line,
                    parameters,
                    kwonly,
                    returns,
                    varargs,
                    varkw,
                    body,
                });
            }
            Some(Lexeme::For) => {
                let keyword_span = lexed.spans[*cursor];
                *cursor += 1;
                let (target, target_span) = match tokens.get(*cursor) {
                    Some(Lexeme::Name(name)) => (name.clone(), lexed.spans[*cursor]),
                    other => {
                        return Err(CompileError::Syntax(format!(
                            "`for` 后面要一个名字，实际 {other:?}"
                        )))
                    }
                };
                *cursor += 1;
                if tokens.get(*cursor) != Some(&Lexeme::In) {
                    return Err(CompileError::Syntax("`for` 的名字后面要 `in`".to_owned()));
                }
                *cursor += 1;
                let (iterable, next) = parse_expression(lexed, *cursor)?;
                *cursor = next;
                if tokens.get(*cursor) != Some(&Lexeme::Colon) {
                    return Err(CompileError::Syntax("`for` 后面要冒号".to_owned()));
                }
                *cursor += 1;
                if tokens.get(*cursor) != Some(&Lexeme::Newline) {
                    return Err(CompileError::Syntax("`for` 的冒号后面要换行".to_owned()));
                }
                *cursor += 1;
                if tokens.get(*cursor) != Some(&Lexeme::Indent) {
                    return Err(CompileError::Syntax("`for` 的体要缩进".to_owned()));
                }
                *cursor += 1;
                let body = parse_statements(lexed, cursor, depth + 1, in_function)?;
                if tokens.get(*cursor) != Some(&Lexeme::Dedent) {
                    return Err(CompileError::Syntax("`for` 的体没有正常收尾".to_owned()));
                }
                *cursor += 1;
                let mut for_else: Vec<Statement> = Vec::new();
                if tokens.get(*cursor) == Some(&Lexeme::Else) {
                    let (parsed, next) = parse_else_block(lexed, *cursor, depth, in_function)?;
                    for_else = parsed;
                    *cursor = next;
                }
                let body_end = if for_else.is_empty() {
                    statements_last_end(&body)
                } else {
                    statements_last_end(&for_else)
                }
                .unwrap_or(keyword_span);
                statements.push(Statement::For {
                    span: keyword_span.to(body_end),
                    target,
                    target_span,
                    iterable,
                    body,
                    else_body: for_else,
                });
            }
            Some(Lexeme::While) => {
                let keyword_span = lexed.spans[*cursor];
                *cursor += 1;
                let (condition, next) = parse_expression(lexed, *cursor)?;
                *cursor = next;
                if tokens.get(*cursor) != Some(&Lexeme::Colon) {
                    return Err(CompileError::Syntax("`while` 后面要冒号".to_owned()));
                }
                *cursor += 1;
                if tokens.get(*cursor) != Some(&Lexeme::Newline) {
                    return Err(CompileError::Syntax("`while` 的冒号后面要换行".to_owned()));
                }
                *cursor += 1;
                if tokens.get(*cursor) != Some(&Lexeme::Indent) {
                    return Err(CompileError::Syntax("`while` 的体要缩进".to_owned()));
                }
                *cursor += 1;
                let body = parse_statements(lexed, cursor, depth + 1, in_function)?;
                if tokens.get(*cursor) != Some(&Lexeme::Dedent) {
                    return Err(CompileError::Syntax("`while` 的体没有正常收尾".to_owned()));
                }
                *cursor += 1;
                let mut while_else: Vec<Statement> = Vec::new();
                if tokens.get(*cursor) == Some(&Lexeme::Else) {
                    let (parsed, next) = parse_else_block(lexed, *cursor, depth, in_function)?;
                    while_else = parsed;
                    *cursor = next;
                }
                let body_end = if while_else.is_empty() {
                    statements_last_end(&body)
                } else {
                    statements_last_end(&while_else)
                }
                .unwrap_or(keyword_span);
                statements.push(Statement::While {
                    span: keyword_span.to(body_end),
                    condition,
                    body,
                    else_body: while_else,
                });
            }
            Some(Lexeme::If) => {
                let keyword_span = lexed.spans[*cursor];
                *cursor += 1;
                let (condition, next) = parse_expression(lexed, *cursor)?;
                *cursor = next;
                if tokens.get(*cursor) != Some(&Lexeme::Colon) {
                    return Err(CompileError::Syntax("`if` 后面要冒号".to_owned()));
                }
                *cursor += 1;
                if tokens.get(*cursor) != Some(&Lexeme::Newline) {
                    return Err(CompileError::Syntax("`if` 的冒号后面要换行".to_owned()));
                }
                *cursor += 1;
                if tokens.get(*cursor) != Some(&Lexeme::Indent) {
                    return Err(CompileError::Syntax("`if` 的体要缩进".to_owned()));
                }
                *cursor += 1;
                let then_body = parse_statements(lexed, cursor, depth + 1, in_function)?;
                if tokens.get(*cursor) != Some(&Lexeme::Dedent) {
                    return Err(CompileError::Syntax("`if` 的体没有正常收尾".to_owned()));
                }
                *cursor += 1;
                // `else` 可选：`else` `:` NEWLINE INDENT … DEDENT
                let mut else_body: Vec<Statement> = Vec::new();
                if tokens.get(*cursor) == Some(&Lexeme::Else) {
                    *cursor += 1;
                    if tokens.get(*cursor) != Some(&Lexeme::Colon) {
                        return Err(CompileError::Syntax("`else` 后面要冒号".to_owned()));
                    }
                    *cursor += 1;
                    if tokens.get(*cursor) != Some(&Lexeme::Newline) {
                        return Err(CompileError::Syntax("`else` 的冒号后面要换行".to_owned()));
                    }
                    *cursor += 1;
                    if tokens.get(*cursor) != Some(&Lexeme::Indent) {
                        return Err(CompileError::Syntax("`else` 的体要缩进".to_owned()));
                    }
                    *cursor += 1;
                    else_body = parse_statements(lexed, cursor, depth + 1, in_function)?;
                    if tokens.get(*cursor) != Some(&Lexeme::Dedent) {
                        return Err(CompileError::Syntax("`else` 的体没有正常收尾".to_owned()));
                    }
                    *cursor += 1;
                }
                let body_end = if else_body.is_empty() {
                    statements_last_end(&then_body)
                } else {
                    statements_last_end(&else_body)
                }
                .unwrap_or(keyword_span);
                statements.push(Statement::If {
                    span: keyword_span.to(body_end),
                    condition,
                    then_body,
                    else_body,
                });
            }
            Some(Lexeme::Raise) => {
                let keyword_span = lexed.spans[*cursor];
                *cursor += 1;
                let bare = matches!(tokens.get(*cursor), Some(Lexeme::Newline) | None);
                let (value, cause) = if bare {
                    (None, None)
                } else {
                    let (value, next) = parse_expression(lexed, *cursor)?;
                    *cursor = next;
                    let is_from = matches!(tokens.get(*cursor), Some(Lexeme::Name(name)) if name == "from");
                    let cause = if is_from {
                        let (cause, next) = parse_expression(lexed, *cursor + 1)?;
                        *cursor = next;
                        Some(cause)
                    } else {
                        None
                    };
                    (Some(value), cause)
                };
                let end = cause
                    .as_ref()
                    .map(|cause| cause.span())
                    .or_else(|| value.as_ref().map(|value| value.span()))
                    .unwrap_or(keyword_span);
                statements.push(Statement::Raise { value, cause, span: keyword_span.to(end) });
                expect_statement_end(tokens, cursor)?;
            }
            Some(Lexeme::Return) => {
                if !in_function {
                    return Err(CompileError::Syntax(
                        "模块级的 `return`（参照实现也是 SyntaxError）".to_owned(),
                    ));
                }
                let keyword_span = lexed.spans[*cursor];
                *cursor += 1;
                let (value, next) = parse_expression(lexed, *cursor)?;
                *cursor = next;
                // 实测：整条 `return …` 的位置从 `return` 起到表达式末尾
                let span = keyword_span.to(value.span());
                statements.push(Statement::Return(value, span));
                expect_statement_end(tokens, cursor)?;
            }
            Some(Lexeme::Name(target)) => {
                let target = target.clone();
                let target_span = lexed.spans[*cursor];
                // 先看是不是**调用**（表达式语句）：`f()`／`f(1)`
                if matches!(tokens.get(*cursor + 1), Some(Lexeme::LeftParen)) {
                    let (expression, next) = parse_expression(lexed, *cursor)?;
                    *cursor = next;
                    let span = expression.span();
                    statements.push(Statement::Expression(expression, span));
                    expect_statement_end(tokens, cursor)?;
                    continue;
                }
                *cursor += 1;
                // **属性赋值**：`名字 . 名字 [. 名字 …] = 表达式`（`STORE_ATTR`）
                if tokens.get(*cursor) == Some(&Lexeme::Dot) {
                    let mut object = Expression::Name(target, target_span);
                    let mut last_name = String::new();
                    while tokens.get(*cursor) == Some(&Lexeme::Dot) {
                        let attribute = match tokens.get(*cursor + 1) {
                            Some(Lexeme::Name(attribute)) => attribute.clone(),
                            other => {
                                return Err(CompileError::Syntax(format!(
                                    "`.` 后面要名字，实际 {other:?}"
                                )))
                            }
                        };
                        if last_name.is_empty() {
                            last_name = attribute;
                        } else {
                            let span = object.span().to(lexed.spans[*cursor + 1]);
                            object = Expression::Attribute(
                                Box::new(object),
                                last_name.clone(),
                                span,
                            );
                            last_name = attribute;
                        }
                        *cursor += 2;
                    }
                    if tokens.get(*cursor) != Some(&Lexeme::Assign) {
                        return Err(CompileError::Unsupported(
                            "只接线了 `名字 = 表达式`／`对象.名字 = 表达式` 与 `return`".to_owned(),
                        ));
                    }
                    *cursor += 1;
                    let (value, next) = parse_expression(lexed, *cursor)?;
                    *cursor = next;
                    let span = target_span.to(value.span());
                    statements.push(Statement::AssignAttr {
                        object,
                        name: last_name,
                        value,
                        span,
                    });
                    expect_statement_end(tokens, cursor)?;
                    continue;
                }
                if tokens.get(*cursor) != Some(&Lexeme::Assign) {
                    return Err(CompileError::Unsupported(
                        "只接线了 `名字 = 表达式` 与 `return`".to_owned(),
                    ));
                }
                *cursor += 1;
                let (value, next) = parse_expression(lexed, *cursor)?;
                *cursor = next;
                let span = target_span.to(value.span());
                statements.push(Statement::Assign {
                    target,
                    target_span,
                    value,
                    span,
                });
                expect_statement_end(tokens, cursor)?;
            }
            // 字符串字面量单独成句：**文档字符串**那一条（作用域首句才当文档串；
            // 其余位置的常量表达式语句，参照实现也会**丢掉**——实测 `def f(): x = 1; "s"; return x`
            // 的 `co_consts` 里没有那个 `"s"`）
            Some(Lexeme::Str(_)) => {
                let (expression, next) = parse_expression(lexed, *cursor)?;
                *cursor = next;
                let span = expression.span();
                statements.push(Statement::Expression(expression, span));
                expect_statement_end(tokens, cursor)?;
            }
            other => {
                return Err(CompileError::Syntax(format!("不认识的语句开头 {other:?}")));
            }
        }
    }
    Ok(statements)
}

/// 一段语句的最后一条的末尾跨度（`def` 的整段要用它收尾）。
fn statements_last_end(statements: &[Statement]) -> Option<Span> {
    statements.last().map(|statement| match statement {
        Statement::Assign { span, .. }
        | Statement::Return(_, span)
        | Statement::Expression(_, span)
        | Statement::Def { span, .. }
        | Statement::Class { span, .. }
        | Statement::AssignAttr { span, .. }
        | Statement::Raise { span, .. }
        | Statement::If { span, .. }
        | Statement::While { span, .. }
        | Statement::For { span, .. } => *span,
    })
}

/// 解析 `else: <换行> <缩进体>`（`if`／`for`／`while` 共用）；`cursor` 指着 `else`。
fn parse_else_block(
    lexed: &Lexed,
    cursor: usize,
    depth: usize,
    in_function: bool,
) -> Result<(Vec<Statement>, usize), CompileError> {
    let tokens = &lexed.lexemes;
    let mut cursor = cursor + 1;
    if tokens.get(cursor) != Some(&Lexeme::Colon) {
        return Err(CompileError::Syntax("`else` 后面要冒号".to_owned()));
    }
    cursor += 1;
    if tokens.get(cursor) != Some(&Lexeme::Newline) {
        return Err(CompileError::Syntax("`else` 的冒号后面要换行".to_owned()));
    }
    cursor += 1;
    if tokens.get(cursor) != Some(&Lexeme::Indent) {
        return Err(CompileError::Syntax("`else` 的体要缩进".to_owned()));
    }
    cursor += 1;
    let body = parse_statements(lexed, &mut cursor, depth + 1, in_function)?;
    if tokens.get(cursor) != Some(&Lexeme::Dedent) {
        return Err(CompileError::Syntax("`else` 的体没有正常收尾".to_owned()));
    }
    Ok((body, cursor + 1))
}

/// 解析一个**注解类型**（`TS-31` 的边界标签；`BC-24` 的签名条目就是它）。
///
/// - 名字 ⇒ 类型标签（`Constant::Type`）：`int`／`str`／`list` 一类
/// - `Any` ⇒ 执行器认识的 `"Any"` 标签（`TS-28` 的双向相容）
/// - `None` ⇒ `NoneType`（`-> None` 的值就是 `None`；`NoneType` 在内建表里）
/// - `名字[内层]` ⇒ 复合标签 `(外类型, 内标签)`（深层档位按它递归，`TS-30` 的不变性落在外类型上）
fn parse_type_at(lexed: &Lexed, cursor: usize) -> Result<(Constant, usize), CompileError> {
    let name = match lexed.lexemes.get(cursor) {
        Some(Lexeme::Name(name)) => name.clone(),
        other => {
            return Err(CompileError::Syntax(format!(
                "注解里要一个类型名，实际 {other:?}"
            )))
        }
    };
    let mut cursor = cursor + 1;
    let base = match name.as_str() {
        "Any" => Constant::Str("Any".to_owned()),
        "None" => Constant::Type("NoneType".to_owned()),
        _ => Constant::Type(name),
    };
    if lexed.lexemes.get(cursor) == Some(&Lexeme::LeftBracket) {
        let (inner, next) = parse_type_at(lexed, cursor + 1)?;
        cursor = next;
        if lexed.lexemes.get(cursor) != Some(&Lexeme::RightBracket) {
            return Err(CompileError::Syntax("注解的 `[` 没有收尾 `]`".to_owned()));
        }
        return Ok((Constant::Tuple(vec![base, inner]), cursor + 1));
    }
    Ok((base, cursor))
}

fn expect_statement_end(tokens: &[Lexeme], cursor: &mut usize) -> Result<(), CompileError> {
    match tokens.get(*cursor) {
        Some(Lexeme::Newline) | Some(Lexeme::End) | Some(Lexeme::Dedent) => Ok(()),
        other => Err(CompileError::Syntax(format!("语句结尾多出了 {other:?}"))),
    }
}

/// 比较层（在 `+` 之上）：本层只接线**一次**比较，链式（`a < b < c`）如实报未接线。
fn parse_expression(lexed: &Lexed, cursor: usize) -> Result<(Expression, usize), CompileError> {
    let (left, cursor) = parse_sum(lexed, cursor)?;
    let operator = match lexed.lexemes.get(cursor) {
        Some(Lexeme::Less) => Some(CompareOperator::Less),
        Some(Lexeme::LessEqual) => Some(CompareOperator::LessEqual),
        Some(Lexeme::EqualEqual) => Some(CompareOperator::Equal),
        Some(Lexeme::NotEqual) => Some(CompareOperator::NotEqual),
        Some(Lexeme::Greater) => Some(CompareOperator::Greater),
        Some(Lexeme::GreaterEqual) => Some(CompareOperator::GreaterEqual),
        _ => None,
    };
    let Some(operator) = operator else {
        return Ok((left, cursor));
    };
    let (right, cursor) = parse_sum(lexed, cursor + 1)?;
    if matches!(
        lexed.lexemes.get(cursor),
        Some(Lexeme::Less)
            | Some(Lexeme::LessEqual)
            | Some(Lexeme::EqualEqual)
            | Some(Lexeme::NotEqual)
            | Some(Lexeme::Greater)
            | Some(Lexeme::GreaterEqual)
    ) {
        return Err(CompileError::Unsupported(
            "链式比较（`a < b < c`）尚未接线".to_owned(),
        ));
    }
    let span = left.span().to(right.span());
    Ok((
        Expression::Compare(Box::new(left), operator, Box::new(right), span),
        cursor,
    ))
}

fn parse_sum(lexed: &Lexed, cursor: usize) -> Result<(Expression, usize), CompileError> {
    let (mut left, mut cursor) = parse_term(lexed, cursor)?;
    while lexed.lexemes.get(cursor) == Some(&Lexeme::Plus) {
        let (right, next) = parse_term(lexed, cursor + 1)?;
        let span = left.span().to(right.span());
        left = Expression::Add(Box::new(left), Box::new(right), span);
        cursor = next;
    }
    Ok((left, cursor))
}

fn parse_term(lexed: &Lexed, cursor: usize) -> Result<(Expression, usize), CompileError> {
    let span = lexed
        .spans
        .get(cursor)
        .copied()
        .unwrap_or(Span::new(1, 1, 0, 0));
    let (mut term, mut cursor) = match lexed.lexemes.get(cursor) {
        Some(Lexeme::Int(value)) => (Expression::Int(*value, span), cursor + 1),
        Some(Lexeme::Str(text)) => (Expression::Str(text.clone(), span), cursor + 1),
        Some(Lexeme::Bytes(value)) => (Expression::Bytes(value.clone(), span), cursor + 1),
        // **`None` 是常量**（实测：`x = None` ⇒ 常量表 `['None']`、`LOAD_CONST 0`）；
        // `True`／`False` 要等 `Constant::Bool`（下一轮）
        Some(Lexeme::LeftBrace) => {
            let start = lexed.spans[cursor];
            let mut cursor = cursor + 1;
            let mut pairs = Vec::new();
            loop {
                if lexed.lexemes.get(cursor) == Some(&Lexeme::RightBrace) {
                    cursor += 1;
                    break;
                }
                let (key, next) = parse_expression(lexed, cursor)?;
                cursor = next;
                if lexed.lexemes.get(cursor) != Some(&Lexeme::Colon) {
                    return Err(CompileError::Syntax(format!(
                        "字典字面量里键之后要 `:`，实际 {:?}",
                        lexed.lexemes.get(cursor)
                    )));
                }
                cursor += 1;
                let (value, next) = parse_expression(lexed, cursor)?;
                cursor = next;
                pairs.push((key, value));
                match lexed.lexemes.get(cursor) {
                    Some(Lexeme::Comma) => cursor += 1,
                    Some(Lexeme::RightBrace) => {
                        cursor += 1;
                        break;
                    }
                    other => {
                        return Err(CompileError::Syntax(format!(
                            "字典字面量里出现 {other:?}"
                        )))
                    }
                }
            }
            let span = start.to(lexed.spans[cursor - 1]);
            (Expression::Map(pairs, span), cursor)
        }
        Some(Lexeme::LeftBracket) => {
            let start = lexed.spans[cursor];
            // 这个位置的 `cursor` 是**不可变参数**（外层要到 match 之后才 `let (mut term, mut cursor)`）
            // ⇒ 这里遮蔽一个本地可变的
            let mut cursor = cursor + 1;
            let mut items = Vec::new();
            loop {
                if lexed.lexemes.get(cursor) == Some(&Lexeme::RightBracket) {
                    cursor += 1;
                    break;
                }
                let (item, next) = parse_expression(lexed, cursor)?;
                cursor = next;
                items.push(item);
                match lexed.lexemes.get(cursor) {
                    Some(Lexeme::Comma) => cursor += 1,
                    Some(Lexeme::RightBracket) => {
                        cursor += 1;
                        break;
                    }
                    other => {
                        return Err(CompileError::Syntax(format!(
                            "列表字面量里出现 {other:?}"
                        )))
                    }
                }
            }
            // 整段的跨度：实测 `BUILD_LIST` 那条取**整个列表**（`[` 到 `]`），不是只取 `[`
            let span = start.to(lexed.spans[cursor - 1]);
            (Expression::List(items, span), cursor)
        }
        Some(Lexeme::Name(name)) if name == "None" => {
            (Expression::Constant(Constant::None, span), cursor + 1)
        }
        // `True`／`False` 同样是**常量**（实测：`x = True` ⇒ 常量表 `['True', 'None']`）
        Some(Lexeme::Name(name)) if name == "True" => {
            (Expression::Constant(Constant::Bool(true), span), cursor + 1)
        }
        Some(Lexeme::Name(name)) if name == "False" => {
            (Expression::Constant(Constant::Bool(false), span), cursor + 1)
        }
        Some(Lexeme::Name(name)) => (Expression::Name(name.clone(), span), cursor + 1),
        other => return Err(CompileError::Syntax(format!("表达式里出现 {other:?}"))),
    };
    // **后缀属性**：`对象.名字`（可连缀 `a.b.c`）。位置取整段（实测）
    while lexed.lexemes.get(cursor) == Some(&Lexeme::Dot) {
        let name = match lexed.lexemes.get(cursor + 1) {
            Some(Lexeme::Name(name)) => name.clone(),
            other => {
                return Err(CompileError::Syntax(format!(
                    "`.` 后面要名字，实际 {other:?}"
                )))
            }
        };
        let span = term.span().to(lexed.spans[cursor + 1]);
        term = Expression::Attribute(Box::new(term), name, span);
        cursor += 2;
    }
    // 后缀调用：`f` `(` 实参 `)`（本层只接线位置实参）
    while lexed.lexemes.get(cursor) == Some(&Lexeme::LeftParen) {
        let callee_span = term.span();
        cursor += 1;
        let mut arguments = Vec::new();
        let mut star_arguments: Vec<Expression> = Vec::new();
        let mut keywords: Vec<(String, Expression)> = Vec::new();
        let mut dict_arguments: Vec<Expression> = Vec::new();
        loop {
            match lexed.lexemes.get(cursor) {
                Some(Lexeme::RightParen) => {
                    cursor += 1;
                    break;
                }
                _ => {}
            }
            // `*表达式`：本层只接线"位置实参之后、关键字之前"这一个位置
            if lexed.lexemes.get(cursor) == Some(&Lexeme::Star) {
                if !keywords.is_empty() || !dict_arguments.is_empty() {
                    return Err(CompileError::Unsupported(
                        "`*` 出现在关键字实参之后尚未接线".to_owned(),
                    ));
                }
                cursor += 1;
                let (value, next) = parse_expression(lexed, cursor)?;
                star_arguments.push(value);
                cursor = next;
                match lexed.lexemes.get(cursor) {
                    Some(Lexeme::Comma) => cursor += 1,
                    Some(Lexeme::RightParen) => {}
                    other => {
                        return Err(CompileError::Syntax(format!("实参表里出现 {other:?}")));
                    }
                }
                continue;
            }
            // `**表达式`
            if lexed.lexemes.get(cursor) == Some(&Lexeme::DoubleStar) {
                cursor += 1;
                let (value, next) = parse_expression(lexed, cursor)?;
                dict_arguments.push(value);
                cursor = next;
                match lexed.lexemes.get(cursor) {
                    Some(Lexeme::Comma) => cursor += 1,
                    Some(Lexeme::RightParen) => {}
                    other => {
                        return Err(CompileError::Syntax(format!("实参表里出现 {other:?}")));
                    }
                }
                continue;
            }
            // 关键字实参：`名字 = 表达式`
            if let (Some(Lexeme::Name(name)), Some(Lexeme::Assign)) =
                (lexed.lexemes.get(cursor), lexed.lexemes.get(cursor + 1))
            {
                let name = name.clone();
                cursor += 2;
                let (value, next) = parse_expression(lexed, cursor)?;
                keywords.push((name, value));
                cursor = next;
            } else {
                if matches!(
                    lexed.lexemes.get(cursor),
                    Some(Lexeme::Plus) | Some(Lexeme::Less) | Some(Lexeme::Greater)
                ) {
                    return Err(CompileError::Syntax("实参表里出现运算符".to_owned()));
                }
                let (argument, next) = parse_expression(lexed, cursor)?;
                arguments.push(argument);
                cursor = next;
            }
            match lexed.lexemes.get(cursor) {
                Some(Lexeme::Comma) => cursor += 1,
                Some(Lexeme::RightParen) => {}
                other => {
                    return Err(CompileError::Syntax(format!("实参表里出现 {other:?}")));
                }
            }
        }
        let closing = lexed
            .spans
            .get(cursor.saturating_sub(1))
            .copied()
            .unwrap_or(callee_span);
        term = Expression::Call {
            function: Box::new(term),
            arguments,
            star_arguments,
            keywords,
            dict_arguments,
            callee_span,
            span: callee_span.to(closing),
        };
    }
    Ok((term, cursor))
}

// ---- 把编译产物装成真的 `CodeObject`（`P1-10` 与执行器／属性面的接缝） ----

/// 按 [`CompiledUnit`] 造一个 `CodeObject`（**新引用**；嵌套常量递归造）。
///
/// 位置表（`BC-18`）一并带上——`co_positions()`／`co_lines()` 就是从它来的。
pub fn instantiate<'a>(
    instance: &'a crate::Instance,
    unit: &CompiledUnit,
) -> crate::Owned<'a, crate::CodeObject> {
    let code_type = instance
        .type_named("CodeObject")
        .expect("CodeObject 在引导期已登记");
    let consts: Vec<Option<core::ptr::NonNull<crate::Header>>> = unit
        .constants
        .iter()
        .map(|constant| instantiate_constant(instance, constant))
        .collect();
    // `CodeObject::name` 目前是 `&'static str`（`BC-4` 的临时形态）⇒ 这里泄漏一份。
    // 这条路径是"编译产物 → 可执行 code object"的**测试**用途，可接受；正式 loader 接上时
    // 应把 `name` 换成 `String`（或交给实例的内置字符串表）。
    let static_name: &'static str = Box::leak(unit.name.clone().into_boxed_str());
    instance.alloc(crate::CodeObject::new(
        code_type,
        static_name,
        unit.qualname.clone(),
        "<pyawa-test>".to_owned(),
        // `co_firstlineno`（测试路径固定 1）
        1,
        // **`co_stacksize`**：本层给的是**保守上界**（见 `stack_bound` 的说明）
        stack_bound(&unit),
        unit.nlocals,
        unit.argcount,
        unit.posonlyargcount,
        unit.kwonlyargcount,
        unit.flags,
        unit.varnames.clone(),
        unit.names.clone(),
        unit.cellvars.clone(),
        unit.freevars.clone(),
        unit.code.clone(),
        Vec::new(),
        consts,
        unit.positions.clone(),
    ))
}

/// **`co_stacksize` 的保守上界**（`BC-43` 只要求"它是值栈上界、越界必须报错"）。
///
/// 做法：按**发射顺序**线性累加每条指令的净效应（`opcode::stack_effect`）取最大值，再加一份
/// 余量（该单元里最大的 oparg ⇒ 单条指令的最大压栈量近似，另加常数）。
///
/// **不追求与参照的精确值相等**——规格把它列为"元信息"（`SPEC-bytecode.md` §2.4）且只要求
/// 它**是上界**；差异登记在 `tests/conformance/divergences.md` 的 `DIV-8`（归一规则：harness
/// 不比对它，只比对"遵守"，即 `T-BC-14` 的越界报错）。
fn stack_bound(unit: &CompiledUnit) -> usize {
    let mut depth: i64 = 0;
    let mut max_depth: i64 = 0;
    let mut largest_oparg: i64 = 0;
    let mut cursor = 0usize;
    while cursor + 1 < unit.code.len() {
        let opcode = u16::from(unit.code[cursor]);
        let argument = unit.code[cursor + 1];
        largest_oparg = largest_oparg.max(i64::from(argument));
        if let Ok(effect) = opcode::stack_effect(opcode, Some(i64::from(argument)), None) {
            depth += i64::from(effect);
            max_depth = max_depth.max(depth);
            // 分支汇合处线性累加会偏负 ⇒ 夹到 0（保守，不追求精确）
            depth = depth.max(0);
        }
        let width = 2 * (1 + opcode::inline_cache_entries(opcode) as usize);
        cursor += width;
    }
    let margin = largest_oparg + 8;
    (max_depth + margin).clamp(4, 4096) as usize
}

/// 只认**字面量**的表达式 ⇒ 编译期常量（字面量默认值折叠要用）。
///
/// 刻意**不**折叠算术：实测 `def f(a, b=1+2)` 会在常量表里留下参照内部的折叠痕迹
/// （多出一个 `1` 的槽），本层不猜它 ⇒ 只折叠直接写得出来的字面量。
fn constant_expression(expression: &Expression) -> Option<Constant> {
    match expression {
        Expression::Int(value, _) => Some(Constant::Int(*value)),
        Expression::Str(text, _) => Some(Constant::Str(text.clone())),
        Expression::Bytes(value, _) => Some(Constant::Bytes(value.clone())),
        _ => None,
    }
}

/// 把一项编译期常量变成运行期对象；**解析不出来给 `None`**（`Constant::Type` 找不到那个类型名）。
///
/// `None` 会在 `boundary_check` 那里按"签名条目不是标签"**如实报错**——宁可报错，也不静默放行。
fn instantiate_constant(
    instance: &crate::Instance,
    constant: &Constant,
) -> Option<core::ptr::NonNull<crate::Header>> {
    match constant {
        Constant::None => Some(instance.retain(instance.singletons().none())),
        Constant::Int(value) => Some(instance.new_int(*value)),
        // **`True`／`False` 是单例**（`OM-23`）⇒ 给调用方一份新引用
        Constant::Bool(value) => Some(instance.retain(instance.singletons().boolean(*value))),
        Constant::Str(text) => Some(instance.new_str(text)),
        Constant::Bytes(value) => Some(instance.new_bytes(value)),
        Constant::Code(inner) => Some(instantiate(instance, inner).into_raw().cast()),
        Constant::Names(names) => {
            let items: Vec<core::ptr::NonNull<crate::Header>> =
                names.iter().map(|name| instance.new_str(name)).collect();
            Some(instance.new_tuple(items))
        }
        Constant::Type(name) => instance.type_named(name).map(|ty| instance.type_value(ty)),
        Constant::Tuple(parts) => {
            let mut items = Vec::with_capacity(parts.len());
            for part in parts {
                items.push(instantiate_constant(instance, part)?);
            }
            Some(instance.new_tuple(items))
        }
    }
}
