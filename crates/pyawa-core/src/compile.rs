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
    /// **常量切片**（实测：界全是常量时参照把 `slice(...)` 放进常量池 ⇒ `LOAD_CONST slice(1, 2, None)`）。
    Slice {
        start: Option<i64>,
        stop: Option<i64>,
        step: Option<i64>,
    },
    /// 嵌套的 code object（本层只有函数体那一种）。
    Code(Box<CompiledUnit>),
    /// **关键字名元组**（`CALL_KW` 之前那条 `LOAD_CONST`；实测紧邻它、名序照源码顺序）。
    Names(Vec<String>),
    /// **折叠出来的 `frozenset`**（实测：`{1, 2, 3}` 这类 **3 个以上**元素且全常量的集合字面量，
    /// 参照发 `BUILD_SET 0; LOAD_CONST frozenset({…}); SET_UPDATE 1`）。
    FrozenSet(Vec<Constant>),
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
    /// **本作用域向外层"索取"的名字**（既不在自己的 `varnames`、也不在本层 cell 里）——
    /// 需求分析的载体：探针编译出的内层单元把它带上来，于是自由名能**逐层上浮** ✓，
    /// 两层闭包（`def a(): x=1; def b(): def c(): return x`）才成立（第 298 轮）。
    /// 纯本层分析用（只有 `analyze_cells` 读写它 ✓）。
    pub demanded: Vec<String>,
    /// `co_consts`。
    pub constants: Vec<Constant>,
    /// 字节码（每码元 2 字节：`opcode` ＋ `oparg`；带缓存的指令后跟等宽零填充）。
    pub code: Vec<u8>,
    /// **`BC-18` 的位置表**：与指令一一对应（起始行／结束行／起始列／结束列；行从 1 起、列从 0 起）。
    /// **`BC-4` 扩**：位置四元组的**每一项都可空**——参照给**合成指令**（`MAKE_CELL`、
     /// 清理块、`PUSH_EXC_INFO` …）的就是 `None`，**禁止**用哨兵数值代替（那是第二个真相）。
    pub positions: Vec<(Option<u32>, Option<u32>, Option<u32>, Option<u32>)>,
    /// **`BC-54` 的异常表**（`co_exceptiontable`）：每条 4 个 6-bit varint（**码元**偏移：
    /// 起点、长度、目标、`depth<<1|lasti`）——`try`／`except` 的派发靠它（`BC-60` ①）。
    pub exceptiontable: Vec<u8>,
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
    // 注意：**空模块与"只有 `pass` 的模块"在参照里都编得过**（实测：`RESUME; LOAD_CONST None;
    // RETURN_VALUE`）⇒ 这里**不能**因为"一条语句都没有"报错（`pass` 不产生指令、也不进语句表）
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
        false,
        &[],
        Span::synthetic(),
    )
}

mod symbols;
use self::symbols::*;

mod emitter;
use self::emitter::*;

mod parser;
use self::parser::*;

mod lexer;
use self::lexer::*;

#[derive(Clone, Copy, PartialEq, Eq)]
enum ScopeKind {
    Module,
    Function,
    /// **类体**（`class C: …`）：作用域与函数不同——无参、无局部槽、flags 0；
    /// 序言要铺 `__module__`／`__qualname__`／`__firstlineno__`，收尾要铺 `__static_attributes__`。
    Class,
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
/// —— **这一支已经接线**（`has_def` 分支就在下面；夹具里 `class C:\n    def m(self): return 1\n`
/// 一类用例全绿）。仍报"嵌套的函数定义尚未接线"的是**函数体内的 `def`**（解析期的 `in_function`
/// 限制），两者不是一回事。
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
        comprehension_locals: Vec::new(),
        pending_cleanups: Vec::new(),
        pending_fused_load: None,
        mode,
        tier,
        qualname: qualname.to_owned(),
        global_names: {
            // **建发射器时就定下本作用域的 `global` 名字**（第 115 轮）：
            // 实测"收集在 A 实例、发存储却在 B 实例"✗（该作用域会被编两趟 ✓）
            // ⇒ 必须在**每个实例**构造处直接扫一遍 ✓。
            let mut names = Vec::new();
            collect_scope_globals(body, &mut names);
            names
        },
            wide_jumps: Vec::new(),
        boundary_out: None,
        deferred: Vec::new(),
        pending: Vec::new(),
        jumps: Vec::new(),
        labels: Vec::new(),
        suppress_chain_tail: false,
        loops: Vec::new(),
        block_end_labels: Vec::new(),
        exception_entries: Vec::new(),
        handler_segments: Vec::new(),
        clause_condition_tail: Span::synthetic(),
        clause_had_else: false,
        boolop_scaffold_span: None,
        if_implicit_return: false,
        in_loop_body: false,
        loop_last_if: false,
        with_return_span: None,
        in_epilogue_body: false,
        defer_return_literal: false,
        with_exit_stack: Vec::new(),
        finally_stack: Vec::new(),
        condition_landings: Vec::new(),
        collect_condition_exits: false,
        pending_condition_copies: Vec::new(),
        pending_chain_copies: Vec::new(),
        chain_takeover: None,
        suppress_chain_takeover: false,
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
            demanded: Vec::new(),
            constants: Vec::new(),
            code: Vec::new(),
            positions: Vec::new(),
            exceptiontable: Vec::new(),
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
        // **`MAKE_CELL` 是合成指令**：参照给的是全 `None`（`BC-4` 扩）
        emitter.emit_named_none("MAKE_CELL", 0);
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
    pre_intern(&mut emitter, body);
    emitter.emit_block(body, false)?;
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
    // **加宽必须在编码异常表之前**（第 121 轮）：插词会移动码元 ⇒ 偏移要一起平移 ✓
    emitter.widen_extended_args();
    emitter.unit.exceptiontable = emitter.encode_exceptiontable();
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
    // `method`：**在 `class` 体里定义**（实测：方法的 `co_flags` 多一位 `0x8000000`）
    method: bool,
    // `freevars`：本作用域的**自由变量**（外层 cell 名，按引用序）——闭包用（第 291 轮）
    freevars: &[String],
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
        comprehension_locals: Vec::new(),
        pending_cleanups: Vec::new(),
        pending_fused_load: None,
        mode,
        tier,
        qualname: qualname.to_owned(),
        global_names: {
            // **建发射器时就定下本作用域的 `global` 名字**（第 115 轮）：
            // 实测"收集在 A 实例、发存储却在 B 实例"✗（该作用域会被编两趟 ✓）
            // ⇒ 必须在**每个实例**构造处直接扫一遍 ✓。
            let mut names = Vec::new();
            collect_scope_globals(body, &mut names);
            names
        },
            wide_jumps: Vec::new(),
        boundary_out: None,
        deferred: Vec::new(),
        pending: Vec::new(),
        jumps: Vec::new(),
        labels: Vec::new(),
        if_implicit_return: false,
        in_loop_body: false,
        loop_last_if: false,
        with_return_span: None,
        in_epilogue_body: false,
        defer_return_literal: false,
        with_exit_stack: Vec::new(),
        finally_stack: Vec::new(),
        condition_landings: Vec::new(),
        collect_condition_exits: false,
        pending_condition_copies: Vec::new(),
        pending_chain_copies: Vec::new(),
        chain_takeover: None,
        suppress_chain_takeover: false,
        suppress_chain_tail: false,
        loops: Vec::new(),
        block_end_labels: Vec::new(),
        exception_entries: Vec::new(),
        handler_segments: Vec::new(),
        clause_condition_tail: Span::synthetic(),
        clause_had_else: false,
        boolop_scaffold_span: None,
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
            demanded: Vec::new(),
            // `varnames` 的顺序（实测／`argbind.rs` 记着）：位置参数 → 仅关键字 → `*args` → `**kw`
            nlocals: parameters.len()
                + kwonly.len()
                + usize::from(varargs.is_some())
                + usize::from(varkw.is_some()),
            flags: if kind == ScopeKind::Function {
                // `0x10` ＝ `CO_NESTED`：**定义在函数里**的函数（含 `lambda`）要置它
                // （实测 `def outer(): return lambda v: v` 的 lambda `co_flags = 19`）；
                // qualname 里带 `.<locals>.` 就说明是嵌套定义
                let nested = u32::from(qualname.contains(".<locals>.")) << 4;
                // **类里定义**的函数（方法）多置 `0x8000000`（实测 `class C: def m` ⇒ `0x8000003`）
                let method_flag = u32::from(method) << 27;
                0x3 | (u32::from(varargs.is_some()) << 2) | (u32::from(varkw.is_some()) << 3)
                    | nested
                    | method_flag
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
            exceptiontable: Vec::new(),
        },
        kind,
    };
    // **闭包（第 291 轮）**：`COPY_FREE_VARS`／`MAKE_CELL` 都排在 `RESUME` **之前**（实测：
    // `def outer(): x = 1; def inner(): return x` ⇒ 外层 `MAKE_CELL x; RESUME; …`、
    // 内层 `COPY_FREE_VARS 1; RESUME; …`）。
    // 局部名要在**发射任何指令之前**收全（`slot_of` 的索引才稳定），闭包分析也放这里。
    // **自由变量表必须早于收局部名**：`slot_of` 靠它把自由名挡在 `varnames` 之外
    // （实测 `nonlocal x; x = 1` 的内层 `varnames=()`、`nlocals=0` ✓；写反了 `x` 会进 varnames ✗）
    emitter.unit.freevars = freevars.to_vec();
    if kind == ScopeKind::Function {
        collect_scope_locals(&mut emitter, statements);
        analyze_cells(&mut emitter, statements, qualname, mode, tier);
    }
    if !freevars.is_empty() {
        // **合成指令没有位点**（实测：参照给 `COPY_FREE_VARS`／`MAKE_CELL` 的是全 `None` ✓，
        // 这是 `BC-4` 扩「合成指令缺失即 `None`、禁止哨兵」的对齐点）
        emitter.emit_none(
            opcode::opcode("COPY_FREE_VARS").expect("COPY_FREE_VARS 在表里"),
            freevars.len() as u8,
        );
    }
    let cellvars = emitter.unit.cellvars.clone();
    for cell in cellvars.iter() {
        let slot = emitter
            .cell_slot(cell)
            .expect("刚拿到的 cellvars 成员") as u8;
        // **`MAKE_CELL` 也没有位点**（实测全 `None` ＝ `BC-4` 扩的对齐点 ✓）
        emitter.emit_none(opcode::opcode("MAKE_CELL").expect("MAKE_CELL 在表里"), slot);
    }
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
    // **源码序预登记**（名字），见 `pre_intern` 的说明
    pre_intern(&mut emitter, body);
    // 作用域体按**统一语句块**发射（块尾标签、死代码、"`try` 之后停止"都在 `emit_block` 里）
    emitter.emit_block(body, false)?;
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
        emitter.flush_pending_cleanups()?;
        emitter.flush_pending();
        emitter.flush_deferred();
    emitter.flush_condition_copies()?;
    emitter.flush_jumps();
    } else if kind == ScopeKind::Module {
        // 不需要收尾（末尾 `if/else` 两分支都 return）
        emitter.flush_pending_cleanups()?;
        emitter.flush_pending();
        emitter.flush_deferred();
        emitter.flush_condition_copies()?;
        emitter.flush_jumps();
    } else {
        emitter.flush_pending_cleanups()?;
        emitter.flush_pending();
        // **函数／类作用域也要冲刷**（第 280 轮修）：此前只在"模块且要收尾"那一支里做 ✗
        // ⇒ 函数里嵌套 `def` 的**默认值元组**会丢（实测参照外层 `co_consts` 末尾有那个元组）。
        emitter.flush_deferred();
        // **函数的隐式返回**：函数体可以"落到末尾"时，参照会补 `LOAD_CONST None; RETURN_VALUE`
        // （`epilogue_needed` 初值为真，遇到 `return` 会置假）。位置取**最后一条真指令**的跨度，
        // 与类体那条规则一致。**之前这里只登记了 `None` 常量、从不发这两条指令** ⇒
        // "没有显式 `return` 的函数"执行时必然 `FellOffEnd`（第 100 轮抓到的根因）。
        // 只有**语句体末尾不是 `return`** 时才补（末尾是 `return` 就没有可落到末尾的路径）；
        // `if/else` 两条分支都 return 的情形已由 `epilogue_needed` 置假覆盖。
        let falls_through = !matches!(body.last(), Some(Statement::Return(_, _)));
        if emitter.epilogue_needed && falls_through {
            // 收尾取 `epilogue_span`（与模块一致：无 `else` 的 `if` 收尾是**条件尾**那一套规则；
            // 实测 `def f(x):\n    if x:\n        return 1\n` 的函数收尾是 `(2,2,7,8)` = 条件的 `x`）
            let tail_span = emitter.epilogue_span;
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
        emitter.flush_condition_copies()?;
        emitter.flush_jumps();
        if emitter.unit.constants.is_empty() {
            // 实测（**与补不补尾两条无关**）：函数自己没有任何常量时，参照仍会在常量表里登记一个
            // `None`（`def f(**kw): return kw` ⇒ `['None']`）。原因不明，规则照实写下来。
            emitter.intern_constant(Constant::None);
        }
    }
    // **加宽必须在编码异常表之前**（第 121 轮）：插词会移动码元 ⇒ 偏移要一起平移 ✓
    emitter.widen_extended_args();
    emitter.unit.exceptiontable = emitter.encode_exceptiontable();
    Ok(emitter.unit)
}


/// **二元运算符**（`BC-39`：`BINARY_OP` 的 oparg 由**符号**从 `get_nb_ops()` 查）。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum BinaryOperator {
    Add,
    Subtract,
    Multiply,
    TrueDivide,
    FloorDivide,
    Remainder,
    Power,
    LeftShift,
    RightShift,
    BitAnd,
    BitXor,
    BitOr,
    /// `@`（矩阵乘；`BC-39` 的 `NB_MATRIX_MULTIPLY` 就在表里，运行期对未支持类型如实报 `TypeError`）。
    MatrixMultiply,
}

impl BinaryOperator {
    /// 源码里的符号（查 `NB_*` 下标用）。
    fn symbol(self) -> &'static str {
        match self {
            BinaryOperator::Add => "+",
            BinaryOperator::Subtract => "-",
            BinaryOperator::Multiply => "*",
            BinaryOperator::TrueDivide => "/",
            BinaryOperator::FloorDivide => "//",
            BinaryOperator::Remainder => "%",
            BinaryOperator::Power => "**",
            BinaryOperator::LeftShift => "<<",
            BinaryOperator::RightShift => ">>",
            BinaryOperator::BitAnd => "&",
            BinaryOperator::BitXor => "^",
            BinaryOperator::BitOr => "|",
            BinaryOperator::MatrixMultiply => "@",
        }
    }
}

/// **一元运算符**（各自对应一条指令）。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum UnaryOperator {
    Positive,
    Negative,
    Invert,
}

/// 常量的真值（只认能一眼判定的；容器／`slice`／`code` 交给运行期按协议判）。
fn truthiness(constant: &Constant) -> Option<bool> {
    Some(match constant {
        Constant::None => false,
        Constant::Bool(value) => *value,
        Constant::Int(value) => *value != 0,
        Constant::Str(text) => !text.is_empty(),
        Constant::Bytes(bytes) => !bytes.is_empty(),
        _ => return None,
    })
}

/// 常量折叠：一元取负（`-i64::MIN` 装不进 `i64` ⇒ 不折）。
fn fold_int_unary_negative(value: i64) -> Result<Option<Constant>, CompileError> {
    let negated = crate::bigint::BigInt::from_i64(value).neg();
    Ok(negated.to_i64().map(Constant::Int))
}

/// 常量折叠：整数二元运算。**装不进 `i64` 就不折**（常量池的整数只有 `Constant::Int(i64)`），
/// 交运行期用任意精度算（`TS-45` 在运行期；参照会把大结果也折成常量 ⇒ 指令流不同，已登记）。
///
/// 三种**故意不折**的：`/`（参照折成 **float** 常量，本层常量池没有浮点）、除数为 0（参照在
/// `compile()` 时就抛 `ZeroDivisionError`，本层交给运行期抛）、负指数的 `**`（参照折成 float）。
fn fold_int_binary(
    operator: BinaryOperator,
    x: i64,
    y: i64,
) -> Result<Option<Constant>, CompileError> {
    use crate::bigint::BigInt;
    let (left, right) = (BigInt::from_i64(x), BigInt::from_i64(y));
    let folded = match operator {
        BinaryOperator::Add => left.add(&right),
        BinaryOperator::Subtract => left.sub(&right),
        BinaryOperator::Multiply => left.mul(&right),
        BinaryOperator::BitAnd => left.bit_and(&right),
        BinaryOperator::BitXor => left.bit_xor(&right),
        BinaryOperator::BitOr => left.bit_or(&right),
        BinaryOperator::FloorDivide | BinaryOperator::Remainder => {
            let Some((quotient, remainder)) = left.divmod_floor(&right) else {
                // 除数为 0：不折（交给运行期报 ZeroDivisionError）
                return Ok(None);
            };
            if operator == BinaryOperator::FloorDivide {
                quotient
            } else {
                remainder
            }
        }
        BinaryOperator::Power => {
            let Ok(exponent) = u32::try_from(y) else {
                // 负指数 ⇒ 参照折成 float；超出 u32 ⇒ 交给运行期
                return Ok(None);
            };
            left.pow_u32(exponent)
        }
        BinaryOperator::LeftShift => {
            let Ok(count) = u64::try_from(y) else {
                return Ok(None);
            };
            match left.shl(count) {
                Some(value) => value,
                None => return Ok(None),
            }
        }
        BinaryOperator::RightShift => {
            let Ok(count) = u64::try_from(y) else {
                return Ok(None);
            };
            left.shr(count)
        }
        // `/` 折成 float：常量池没有浮点 ⇒ 不折（指令流与参照不同，已登记）
        BinaryOperator::TrueDivide => return Ok(None),
        // `@`：本层没有矩阵类型 ⇒ 不折（运行期如实报 `TypeError`）
        BinaryOperator::MatrixMultiply => return Ok(None),
    };
    Ok(folded.to_i64().map(Constant::Int))
}

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
    /// **星号解包**（`*表达式`，只出现在**显示**里 ✓ —— 第 120 轮）。
    ///
    /// 形态（实测）：显示里前面的非星号项先压 ⇒ `BUILD_LIST <前项数>`（位点＝**整个显示** ✓）
    /// ⇒ 每个星号项：表达式 ＋ `LIST_EXTEND 1` ✓；元组末尾再 `CALL_INTRINSIC_1 6` ✓、
    /// 集合走 `SET_UPDATE 1` ✓。
    Starred(Box<Expression>, Span),
    /// **海象**（`(名字 := 表达式)`）：值留在栈上，同时写进目标名 ✓（形态见发射臂 ✓）。
    Walrus {
        target: String,
        target_span: Span,
        value: Box<Expression>,
        span: Span,
    },
    /// **二元运算**（`BINARY_OP`；`BC-39` 的 `NB_*` 下标按符号从 `get_nb_ops()` 取）。
    Binary(BinaryOperator, Box<Expression>, Box<Expression>, Span),
    /// **一元运算**（`UNARY_POSITIVE`／`UNARY_NEGATIVE`／`UNARY_INVERT`）。
    Unary(UnaryOperator, Box<Expression>, Span),
    /// **集合字面量**（`{1, 2}`）：逐元素求值后 `BUILD_SET n`。
    /// 注：参照对"**≥3 个全常量**元素"会折成 `BUILD_SET 0; LOAD_CONST frozenset(…); SET_UPDATE 1`
    /// （`{1, 2}` 两个元素不折）——那一族尚未接线，见 `PLAN`。
    SetLiteral(Vec<Expression>, Span),
    /// **推导式**（3.12+ 是**内联**形态：`LOAD_FAST_AND_CLEAR` 保存外层同名局部 ＋
    /// `BUILD_LIST`／`LIST_APPEND`（集合则是 `BUILD_SET`／`SET_ADD`）＋ 融合指令
    /// ＋ **整段异常表保护**）。
    Comprehension {
        kind: ComprehensionKind,
        /// 列表／集合的**元素**，字典的**键**。
        element: Box<Expression>,
        /// 字典的**值**（列表／集合没有）。
        value: Option<Box<Expression>>,
        generators: Vec<Generator>,
        span: Span,
    },
    /// **f-string**（3.14 实测：逐段求值，`FORMAT_SIMPLE`／`CONVERT_VALUE`／`FORMAT_WITH_SPEC`，
    /// 多于一段再 `BUILD_STRING n`；**纯字面量**的 f-string 直接降成一条 `LOAD_CONST`）。
    FString {
        parts: Vec<FStringPart>,
        span: Span,
    },
    /// **`lambda`**（3.14 实测：嵌套单元 `co_name`／`co_qualname` 都是 `<lambda>`，
    /// 体就是"求值那条表达式再 `RETURN_VALUE`"；`def` 与它共用 `emit_function_object`）。
    Lambda {
        parameters: Vec<Parameter>,
        kwonly: Vec<Parameter>,
        varargs: Option<String>,
        varkw: Option<String>,
        body: Box<Expression>,
        span: Span,
    },
    /// **`not`**（实测：`LOAD …; TO_BOOL; UNARY_NOT`；常量在编译期折成 `bool`）。
    Not(Box<Expression>, Span),
    /// **`and`／`or`**（3.14 的形态：`COPY 1; TO_BOOL; POP_JUMP_IF_*; NOT_TAKEN; POP_TOP`；
    /// `conjunction` 为真表示 `and`。**返回操作数**且**短路**。）
    BoolOp {
        conjunction: bool,
        values: Vec<Expression>,
        span: Span,
    },
    /// **元组字面量**（`(a, b)`／`()`／裸的 `a, b`）。全常量时**折叠成常量**（参照实测：
    /// `x = (1, 2)` 的 `co_consts` 里有那个元组）；否则 `BUILD_TUPLE n`。
    TupleLiteral(Vec<Expression>, Span),
    /// **下标读**（`a[i]`）：3.14 没有单独的取下标指令，实测是 `LOAD a; LOAD i; BINARY_OP NB_SUBSCR`。
    Subscript(Box<Expression>, Box<Expression>, Span),
    /// **切片字面量**（`a[b:c]`／`a[b:c:d]`，至少一段非常量时走这条）：
    /// 实测两段用 **`BINARY_SLICE`**、三段用 `BUILD_SLICE 3` ＋ `BINARY_OP []`；
    /// 缺的界会**显式压 `None`**。全常量界的形态在解析时就折成 `Constant::Slice`（进常量池）。
    SliceLiteral {
        lower: Option<Box<Expression>>,
        upper: Option<Box<Expression>>,
        step: Option<Box<Expression>>,
        span: Span,
    },
    /// 比较（`COMPARE_OP` 的 oparg 逐运算符实测：`下标 << 5 | 提示位`）。
    Compare(Box<Expression>, CompareOperator, Box<Expression>, Span),
    /// **链式比较**（`a < b < c`；≥2 个运算符才有这一支）。实测骨架：
    /// `LOAD 最左; LOAD 次; SWAP 2; COPY 2; COMPARE_OP; COPY 1; TO_BOOL;
    ///  POP_JUMP_IF_FALSE → L; NOT_TAKEN; POP_TOP; LOAD 第三个; COMPARE_OP;`（最后一段不再 SWAP／COPY）
    /// `L: SWAP 2; POP_TOP`（失败路径丢掉左操作数、留下 `False`）。
    ChainedCompare {
        operands: Vec<Expression>,
        operators: Vec<CompareOperator>,
        span: Span,
    },
    /// **三元表达式** `a if b else c`（第 282 轮接线）。
    ///
    /// 参照实测**把余部复制进两个分支**（`y = f(a if b else c)` 里 `CALL`＋存入＋收尾各出现两次；
    /// `return a if b else c` 里 `RETURN_VALUE` 出现两次）——那要**表达式级的续延**模型 ✗。
    /// 本层改用**语义等价**的形态：`<条件>; TO_BOOL; POP_JUMP_IF_FALSE → else; NOT_TAKEN;
    /// <then>; JUMP_FORWARD → end; else: <else>; end:`（多一条 `JUMP_FORWARD`，布局不同 ✓）。
    Conditional {
        condition: Box<Expression>,
        then_value: Box<Expression>,
        else_value: Box<Expression>,
        span: Span,
    },
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
    /// `is`：不是 `COMPARE_OP`——实测走 `IS_OP`，oparg **0**（`BC-58`）。
    Is,
    /// `is not`：`IS_OP` oparg **1**。
    IsNot,
    /// `in`：实测走 `CONTAINS_OP`，oparg **0**。
    In,
    /// `not in`：`CONTAINS_OP` oparg **1**。
    NotIn,
}

impl CompareOperator {
    /// `COMPARE_OP` 的 oparg（**实测**：`<` ⇒ 2、`<=` ⇒ 42、`==` ⇒ 72、`!=` ⇒ 103、
    /// `>` ⇒ 132、`>=` ⇒ 172）。**`is`／`in` 一族走 `IS_OP`／`CONTAINS_OP`** ⇒ 这里给 `None`。
    fn oparg(self) -> Option<u8> {
        Some(match self {
            CompareOperator::Is
            | CompareOperator::IsNot
            | CompareOperator::In
            | CompareOperator::NotIn => return None,
            CompareOperator::Less => 2,
            CompareOperator::LessEqual => 42,
            CompareOperator::Equal => 72,
            CompareOperator::NotEqual => 103,
            CompareOperator::Greater => 132,
            CompareOperator::GreaterEqual => 172,
        })
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
            | Expression::SetLiteral(_, span)
            | Expression::FString { span, .. }
            | Expression::Comprehension { span, .. }
            | Expression::Lambda { span, .. }
            | Expression::Attribute(_, _, span)
            | Expression::Walrus { span, .. }
            | Expression::Starred(_, span)
            | Expression::Binary(_, _, _, span)
            | Expression::Unary(_, _, span)
            | Expression::Not(_, span)
            | Expression::BoolOp { span, .. }
            | Expression::TupleLiteral(_, span)
            | Expression::Subscript(_, _, span)
            | Expression::SliceLiteral { span, .. }
            | Expression::Compare(_, _, _, span)
            | Expression::ChainedCompare { span, .. }
            | Expression::Conditional { span, .. }
            | Expression::Call { span, .. } => *span,
        }
    }
}

/// 一个待外提的推导式清理块（`SWAP 2; POP_TOP; SWAP 2; STORE_FAST <槽>; RERAISE 0`）。
#[derive(Debug, Clone)]
struct PendingCleanup {
    /// 要还原的目标槽（**书写顺序**；冲刷时**逆序**还原）。
    slots: Vec<usize>,
    /// `SWAP` 的层数（＝ 目标数 ＋ 1）。
    depth: u8,
    scaffold: Span,
    region_start: usize,
    region_end: usize,
    label: usize,
}

/// 推导式的容器种类（3.14 实测：只有「建容器／加元素」两条指令不同）。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ComprehensionKind {
    List,
    Set,
    Dict,
}

/// f-string 的一段：字面量，或"表达式 ＋（可选）转换 ＋（可选）格式规格"。
#[derive(Debug, Clone, PartialEq, Eq)]
enum FStringPart {
    /// 字面段（跨度取**字面文字本身**，实测 `f"a{x}b"` 里 `'a'` 是 `(1,1,6,7)`）。
    Literal { text: String, span: Span },
    Formatted {
        expression: Expression,
        /// `!s` ＝ 1、`!r` ＝ 2、`!a` ＝ 3（实测 `!r` ⇒ `CONVERT_VALUE 2`）。
        conversion: Option<u8>,
        /// 格式规格本身又是若干段（`{x:>{w}}` 的 `>{w}`）。
        spec: Option<Vec<FStringPart>>,
        /// **规格那一段**的跨度（含冒号、不含外层 `}`；实测 `f"{x:>{w}}"` 里规格内 `BUILD_STRING`
        /// 取它 `(1,1,8,13)`，而 `FORMAT_WITH_SPEC` 仍取整个 `{…}` `(1,1,6,14)`）。
        spec_span: Option<Span>,
        /// **整个 `{…}`** 的跨度（实测 `FORMAT_SIMPLE` 取它，如 `(1,1,7,10)`）。
        span: Span,
    },
}

/// 推导式的**目标**：一个名字，或一串名字（元组目标 `for k, v in …`）。
#[derive(Debug, Clone, PartialEq, Eq)]
enum ComprehensionTarget {
    Name(String, Span),
    Tuple(Vec<(String, Span)>),
}

/// 推导式的一层生成器：`for <目标> in <可迭代> [if <条件>]*`。
#[derive(Debug, Clone, PartialEq, Eq)]
struct Generator {
    target: ComprehensionTarget,
    iterable: Expression,
    conditions: Vec<Expression>,
}

impl ComprehensionTarget {
    /// 目标里的名字（按书写顺序）。
    fn names(&self) -> Vec<&str> {
        match self {
            ComprehensionTarget::Name(name, _) => vec![name.as_str()],
            ComprehensionTarget::Tuple(items) => items.iter().map(|(name, _)| name.as_str()).collect(),
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
    /// 注解在源码里的跨度（`__annotate__` 单元里 `LOAD_GLOBAL` 取它，实测）。
    annotation_span: Option<Span>,
    /// 默认值表达式（`None` ⇒ 没有默认值）。
    default: Option<Expression>,
}

/// 模块级／缩进块里的语句。
/// `BC-54` 的 6-bit varint（**大端**分组：高 6 位先写，未结束的字节置 `0x40`）。
/// 与 [`crate::decode::parse_exception_table`] 的读法互逆。
fn write_exception_varint(out: &mut Vec<u8>, value: usize) {
    let mut groups = vec![(value & 0x3F) as u8];
    let mut rest = value >> 6;
    while rest > 0 {
        groups.push((rest & 0x3F) as u8);
        rest >>= 6;
    }
    groups.reverse();
    let last = groups.len() - 1;
    for (index, group) in groups.into_iter().enumerate() {
        out.push(if index == last { group } else { group | 0x40 });
    }
}


/// 在一个表达式里**找 lambda**，把每个 lambda 需要从外层拿的名字（体内引用减去自己的形参）
/// 收进 `out`。**只收 lambda 体内的名字** ✗ —— 早先误写成"收整个表达式里的名字"，结果方法形参
/// 都被当成 cell（`class C: def m(self, x): if x: …` ⇒ 多出 `MAKE_CELL` ✗，被夹具当场抓住 ✓）。
fn find_lambda_demands(expression: &Expression, out: &mut Vec<String>) {
    match expression {
        Expression::Lambda {
            parameters,
            kwonly,
            varargs,
            varkw,
            body,
            ..
        } => {
            let mut inner = Vec::new();
            collect_names_in_expression(body, &mut inner);
            for name in &inner {
                let is_parameter = parameters.iter().any(|item| item.name == *name)
                    || kwonly.iter().any(|item| item.name == *name)
                    || varargs.as_deref() == Some(name.as_str())
                    || varkw.as_deref() == Some(name.as_str());
                if !is_parameter && !out.iter().any(|item| item == name) {
                    out.push(name.clone());
                }
            }
        }
        Expression::List(items, _)
        | Expression::SetLiteral(items, _)
        | Expression::TupleLiteral(items, _) => {
            for item in items {
                find_lambda_demands(item, out);
            }
        }
        Expression::Map(items, _) => {
            for (key, value) in items {
                find_lambda_demands(key, out);
                find_lambda_demands(value, out);
            }
        }
        Expression::Attribute(target, _, _)
        | Expression::Not(target, _)
        | Expression::Unary(_, target, _) => find_lambda_demands(target, out),
        Expression::Binary(_, left, right, _)
        | Expression::Compare(left, _, right, _)
        | Expression::Subscript(left, right, _) => {
            find_lambda_demands(left, out);
            find_lambda_demands(right, out);
        }
        Expression::BoolOp { values, .. } | Expression::ChainedCompare { operands: values, .. } => {
            for value in values {
                find_lambda_demands(value, out);
            }
        }
        Expression::Conditional {
            condition,
            then_value,
            else_value,
            ..
        } => {
            find_lambda_demands(condition, out);
            find_lambda_demands(then_value, out);
            find_lambda_demands(else_value, out);
        }
        Expression::FString { parts, .. } => {
            for part in parts {
                if let FStringPart::Formatted { expression, spec, .. } = part {
                    find_lambda_demands(expression, out);
                    if let Some(spec) = spec {
                        for item in spec {
                            if let FStringPart::Formatted { expression, .. } = item {
                                find_lambda_demands(expression, out);
                            }
                        }
                    }
                }
            }
        }
        Expression::Call {
            function,
            arguments,
            star_arguments,
            keywords,
            dict_arguments,
            ..
        } => {
            find_lambda_demands(function, out);
            for argument in arguments.iter().chain(star_arguments) {
                find_lambda_demands(argument, out);
            }
            for (_, value) in keywords {
                find_lambda_demands(value, out);
            }
            for value in dict_arguments {
                find_lambda_demands(value, out);
            }
        }
        Expression::Comprehension {
            element,
            value,
            generators,
            ..
        } => {
            find_lambda_demands(element, out);
            if let Some(value) = value {
                find_lambda_demands(value, out);
            }
            for generator in generators {
                find_lambda_demands(&generator.iterable, out);
                for condition in &generator.conditions {
                    find_lambda_demands(condition, out);
                }
            }
        }
        Expression::SliceLiteral {
            lower,
            upper,
            step,
            ..
        } => {
            for part in [lower, upper, step].into_iter().flatten() {
                find_lambda_demands(part, out);
            }
        }
        _ => {}
    }
}


/// `try` 的一条 `except` 子句。
#[derive(Debug, Clone, PartialEq, Eq)]
struct Handler {
    /// `except <类型>:` 里的类型表达式；裸 `except:` 是 `None`。
    type_: Option<Expression>,
    /// `except … as <名字>:` 里的名字。
    name: Option<String>,
    body: Vec<Statement>,
    span: Span,
}

/// 一层循环的 `break`／`continue` 落点（发射期用）。
#[derive(Clone)]
struct LoopFrame {
    /// `continue` 跳回的地方（`for` 是 `FOR_ITER`、`while` 是条件起点）。
    continue_target: usize,
    /// 是不是 `for`（`break`／`return` 要先 `POP_TOP` 掉迭代器）。
    is_for: bool,
    /// **循环之后的语句**（块结构模型：`break` 的路径把它们就地复制一份）。
    rest: Vec<Statement>,
}

/// 增强赋值的**目标**（三种形态各自一套栈序，实测见 [`Statement::AugAssign`]）。
#[derive(Debug, Clone, PartialEq, Eq)]
enum AugTarget {
    /// 裸名字：`LOAD x; …; STORE x`。
    Name(String, Span),
    /// 属性：`LOAD obj; COPY 1; LOAD_ATTR name; …; SWAP 2; STORE_ATTR name`。
    Attribute {
        object: Expression,
        name: String,
        span: Span,
    },
    /// 下标：`LOAD 容器; LOAD 键; COPY 2; COPY 2; BINARY_OP []; …; SWAP 3; SWAP 2; STORE_SUBSCR`。
    Subscript {
        container: Expression,
        key: Expression,
        target_span: Span,
        span: Span,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum Statement {
    /// **链式赋值**（`a = b = c = x` ✓，目标按**从左到右**存 ✓）。
    ///
    /// 目标复用**表达式**（`Name`／`Attribute`／`Subscript` ✓，与 `del`／`AssignTuple` 同一口径 ✓）。
    AssignChained {
        targets: Vec<Expression>,
        value: Expression,
        span: Span,
    },
    /// **元组解包赋值**（`a, b = x`／`a[0], b = x`／`a, *b, c = x` ✓）。
    ///
    /// 目标复用**表达式**（`Name`／`Attribute`／`Subscript` ✓）＋ 一位星号标记 ✓；
    /// `target_span` 是目标那一段的跨度（`UNPACK_SEQUENCE` 的位点取它 ✓，实测）。
    AssignTuple {
        targets: Vec<(Expression, bool)>,
        value: Expression,
        target_span: Span,
        span: Span,
    },
    /// `del <目标> (, <目标>)*`（3.14 实测形态见发射臂 ✓）。
    ///
    /// 目标复用**表达式**（`Name`／`Attribute`／`Subscript` ✓）：`del a[0]`／`del a.b` 的目标本来
    /// 就是这两个形状 ✓ ⇒ AST 不必另立枚举 ✓。
    Delete {
        targets: Vec<Expression>,
        span: Span,
    },
    Assign {
        target: String,
        target_span: Span,
        value: Expression,
        span: Span,
    },
    Return(Expression, Span),
    /// `assert <测试> [, <消息>]`（3.14 的实测形态见发射臂 ✓）。
    Assert {
        test: Expression,
        message: Option<Expression>,
        span: Span,
    },
    /// `nonlocal a, b`：**不发任何指令**（纯声明 ✓）。作用在分析层：这些名字在本作用域是**自由变量**
    /// （读 `LOAD_DEREF`、写 `STORE_DEREF`），并使**外层**把它记成 cell（第 295 轮）。
    NonLocal(Vec<String>, Span),
    /// `global a, b`（**不发指令** ✓；作用是让这些名字在**任何作用域**都按全局处理 ✓）。
    Global(Vec<String>, Span),
    /// 表达式语句（本层只接线调用：算完 `POP_TOP` 丢掉）。
    Expression(Expression, Span),
    /// `for <目标> in <可迭代>: <体>`。
    For {
        span: Span,
        target: String,
        target_span: Span,
        /// **元组目标**（`for n, line in …` ✓）：空表示单目标（用 `target` ✓）；
        /// 非空时 `target_span` 是**整段目标**（`UNPACK_SEQUENCE` 的位点取它 ✓，实测 `n, line` ✓）。
        tuple_targets: Vec<(String, Span)>,
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
    /// **`pass`**：**不产生指令**（实测），但它的位置要留给收尾（`last_span`）。
    Pass(Span),
    /// **`import <模块> [as <名字>] (, …)*`**
    Import {
        /// `(点分模块名, 可选的 `as` 名字)`。
        items: Vec<(String, Option<String>)>,
        span: Span,
    },
    /// **`from <点*><模块> import <名字> [as <名字>] (, …)*`**（`*` 走 `CALL_INTRINSIC_1 2`）。
    ImportFrom {
        module: String,
        /// 相对导入的点数（`from . import b` ⇒ 1）。
        level: u8,
        names: Vec<(String, Option<String>)>,
        star: bool,
        span: Span,
    },
    /// **`with`**（3.14 的骨架：`LOAD_SPECIAL` 一族；见发射臂的实测注释）。
    /// `items` 是 `(上下文表达式, `as` 目标名)` 的表。
    With {
        items: Vec<(Expression, Option<(String, Span)>)>,
        body: Vec<Statement>,
        span: Span,
    },
    /// **`try`／`except`／`else`／`finally`**（`BC-54` 的异常表 ＋ `PUSH_EXC_INFO` 一族）。
    Try {
        body: Vec<Statement>,
        handlers: Vec<Handler>,
        /// `else:` 体（只有 `body` 正常走完才执行；**不在**受保护区内，实测异常表只盖 `body`）。
        else_body: Vec<Statement>,
        /// `finally:` 体（正常路径就地发一遍；异常路径再发一遍后 `RERAISE`）。
        finally_body: Vec<Statement>,
        span: Span,
    },
    /// **`break`**：跳出最近的循环（`for` 要先 `POP_TOP` 掉迭代器；`else` 体**不执行**）。
    Break(Span),
    /// **`continue`**：回到循环起点（`for` 回 `FOR_ITER`、`while` 回条件）。
    Continue(Span),
    /// **增强赋值**（`x += v`／`a.b += v`／`a[i] += v`）：实测三种目标的栈序各不相同
    /// （名字：`LOAD x; 值; BINARY_OP NB_INPLACE_*; STORE x`；属性：`LOAD obj; COPY 1; LOAD_ATTR;
    /// 值; BINARY_OP; SWAP 2; STORE_ATTR`；下标：`LOAD 容器; LOAD 键; COPY 2; COPY 2; BINARY_OP [];
    /// 值; BINARY_OP; SWAP 3; SWAP 2; STORE_SUBSCR`）。
    AugAssign {
        target: AugTarget,
        operator: AugOperator,
        value: Expression,
        span: Span,
    },
    /// **下标赋值**（`a[i] = v`／`a[i][j] = v`）：实测发射顺序是「值 → 容器 → 键 → `STORE_SUBSCR`」。
    AssignSubscript {
        container: Expression,
        key: Expression,
        value: Expression,
        /// 目标下标本身的跨度（`a[i]` 那一段）——`STORE_SUBSCR` 与收尾取它（实测）。
        target_span: Span,
        span: Span,
    },
    AssignAttr {
        /// 被赋属性的对象（`self` 这一层）。
        object: Expression,
        /// 属性名。
        name: String,
        /// 右值。
        value: Expression,
        /// 整条语句的跨度（AST 层用，如"类体最后一句"的收尾判定）。
        span: Span,
        /// **目标链**那一段的跨度（发射 `STORE_ATTR`／收尾用；实测 `self.v = 5` ⇒ `(3,3,8,14)`）。
        target_span: Span,
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
        /// **装饰器**（`@<表达式>`，**源码序** ✓）：发射时先按序求值、再逆序 `CALL 0` 包上去 ✓
        decorators: Vec<Expression>,
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
        /// 返回注解在源码里的跨度（`__annotate__` 单元的 `LOAD_GLOBAL` 取它，实测）。
        returns_span: Option<Span>,
        body: Vec<Statement>,
    },
}


/// 把一段**全常量**表达式求值（`+` 的常量折叠）；不是全常量给 `None`。
fn fold_constant(expression: &Expression) -> Result<Option<Constant>, CompileError> {
    match expression {
        Expression::Comprehension { .. } => Ok(None),
        Expression::ChainedCompare { .. } => Ok(None),
        Expression::Conditional { .. } => Ok(None),
        Expression::FString { .. } => Ok(None),
        Expression::SetLiteral(_, _) => Ok(None),
        Expression::Lambda { .. } => Ok(None),
        // 海象**不做常量折叠**（它带副作用 ⇒ 折了就丢了写目标 ✓）
        Expression::Walrus { .. } => Ok(None),
        Expression::Starred(_, _) => Ok(None),
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
        Expression::Binary(operator, left, right, _) => {
            let (Some(left_value), Some(right_value)) =
                (fold_constant(left)?, fold_constant(right)?)
            else {
                return Ok(None);
            };
            match (left_value, right_value) {
                (Constant::Int(x), Constant::Int(y)) => {
                    fold_int_binary(*operator, x, y)
                }
                // `+` 的字符串／bytes 拼接（实测：两者都在编译期折）
                (Constant::Str(x), Constant::Str(y)) if *operator == BinaryOperator::Add => {
                    Ok(Some(Constant::Str(x + &y)))
                }
                (Constant::Bytes(x), Constant::Bytes(y)) if *operator == BinaryOperator::Add => {
                    let mut joined = x;
                    joined.extend_from_slice(&y);
                    Ok(Some(Constant::Bytes(joined)))
                }
        _ => Ok(None),
            }
        }
        Expression::TupleLiteral(items, _) => {
            let mut folded = Vec::with_capacity(items.len());
            for item in items {
                let Some(constant) = fold_constant(item)? else {
                    return Ok(None);
                };
                folded.push(constant);
            }
            Ok(Some(Constant::Tuple(folded)))
        }
        // 下标／非常量切片都不是常量（参照也不折）
        Expression::Subscript(_, _, _) | Expression::SliceLiteral { .. } => Ok(None),
        // `and`／`or`：**常量短路**（实测 `1 and 2` ⇒ `2`、`0 and 3` ⇒ `0`；`1 or 2` ⇒ `1`）
        Expression::BoolOp {
            conjunction,
            values,
            ..
        } => {
            let mut last: Option<Constant> = None;
            for value in values {
                let Some(constant) = fold_constant(value)? else {
                    return Ok(None);
                };
                let decided = truthiness(&constant);
                match decided {
                    Some(truth) => {
                        let short_circuit = if *conjunction { !truth } else { truth };
                        if short_circuit {
                            return Ok(Some(constant));
                        }
                    }
                    // 真值判不了的常量（容器／`slice`／`code`）：不折
                    None => return Ok(None),
                }
                last = Some(constant);
            }
            Ok(last)
        }
        // `not`：常量折成 `bool`（实测 `x = not 0` ⇒ `LOAD_CONST True`，`bool` 进常量池）
        Expression::Not(operand, _) => {
            let Some(value) = fold_constant(operand)? else {
                return Ok(None);
            };
            let Some(truth) = truthiness(&value) else {
                return Ok(None);
            };
            Ok(Some(Constant::Bool(!truth)))
        }
        Expression::Unary(operator, operand, _) => {
            let Some(value) = fold_constant(operand)? else {
                return Ok(None);
            };
            match (operator, value) {
                // `+x` 只对整数等价于 x（`+'a'` 在参照里是 TypeError ⇒ 不折）
                (UnaryOperator::Positive, Constant::Int(value)) => Ok(Some(Constant::Int(value))),
                (UnaryOperator::Negative, Constant::Int(value)) => {
                    fold_int_unary_negative(value)
                }
                (UnaryOperator::Invert, Constant::Int(value)) => Ok(Some(Constant::Int(!value))),
                _ => Ok(None),
            }
        }
    }
}

/// 表达式**最左的那个名字读**（推导式里用来判断"紧接着会读哪个局部"）。
fn leftmost_name(expression: &Expression) -> Option<&str> {
    match expression {
        Expression::Name(name, _) => Some(name.as_str()),
        Expression::Binary(_, left, _, _) => leftmost_name(left),
        Expression::Compare(left, _, _, _) => leftmost_name(left),
        Expression::BoolOp { values, .. } => values.first().and_then(leftmost_name),
        Expression::Unary(_, operand, _) | Expression::Not(operand, _) => leftmost_name(operand),
        Expression::Attribute(target, _, _) => leftmost_name(target),
        Expression::Subscript(container, _, _) => leftmost_name(container),
        Expression::TupleLiteral(items, _) => items.first().and_then(leftmost_name),
        _ => None,
    }
}

/// 全常量表达式的**最左叶子**（实测：折叠时只有它进常量表）。
fn leftmost_literal(expression: &Expression) -> Option<Constant> {
    match expression {
        Expression::Comprehension { .. } => None,
        Expression::ChainedCompare { .. } => None,
        Expression::Conditional { .. } => None,
        Expression::FString { .. } => None,
        Expression::SetLiteral(_, _) => None,
        Expression::Lambda { .. } => None,
        Expression::Walrus { .. } => None,
        Expression::Starred(_, _) => None,
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
        // 折叠时"只有最左叶子进常量表"（实测）⇒ 二元递归左操作数、一元递归操作数
        Expression::Binary(_, left, _, _) => leftmost_literal(left),
        Expression::Unary(_, operand, _) | Expression::Not(operand, _) => {
            leftmost_literal(operand)
        }
        Expression::TupleLiteral(items, _) => items.first().and_then(leftmost_literal),
        Expression::Subscript(_, _, _) | Expression::SliceLiteral { .. } => None,
        // `and`／`or`：最左叶子＝第一个操作数的最左叶子（实测 `1 and 2` 会把 `1` 入表）
        Expression::BoolOp { values, .. } => values.first().and_then(leftmost_literal),
    }
}

// ---- 词法（**带行列**：`BC-18` 的位置表要它） ----

/// **增强赋值**的运算符（`+=` 一族）。与 `get_nb_ops()` 里的 `NB_INPLACE_*` 一一对应。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum AugOperator {
    Add,
    Subtract,
    Multiply,
    TrueDivide,
    FloorDivide,
    Remainder,
    Power,
    LeftShift,
    RightShift,
    BitAnd,
    BitXor,
    BitOr,
    MatrixMultiply,
}

impl AugOperator {
    /// 源码里的写法（查 `NB_INPLACE_*` 下标用）。
    fn symbol(self) -> &'static str {
        match self {
            AugOperator::Add => "+=",
            AugOperator::Subtract => "-=",
            AugOperator::Multiply => "*=",
            AugOperator::TrueDivide => "/=",
            AugOperator::FloorDivide => "//=",
            AugOperator::Remainder => "%=",
            AugOperator::Power => "**=",
            AugOperator::LeftShift => "<<=",
            AugOperator::RightShift => ">>=",
            AugOperator::BitAnd => "&=",
            AugOperator::BitXor => "^=",
            AugOperator::BitOr => "|=",
            AugOperator::MatrixMultiply => "@=",
        }
    }
}


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
        // **`BC-54` 的异常表**（`try`／`except` 的派发靠它）——此前这里硬编码空表
        unit.exceptiontable.clone(),
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
        Constant::Slice { start, stop, step } => {
            Some(instance.new_slice(*start, *stop, *step))
        }
        Constant::Code(inner) => Some(instantiate(instance, inner).into_raw().cast()),
        Constant::Names(names) => {
            let items: Vec<core::ptr::NonNull<crate::Header>> =
                names.iter().map(|name| instance.new_str(name)).collect();
            Some(instance.new_tuple(items))
        }
        Constant::FrozenSet(parts) => {
            // 折叠出来的 `frozenset`：这里以**集合对象**落地（`SET_UPDATE` 只按可迭代取元素）
            let mut items = Vec::with_capacity(parts.len());
            for part in parts {
                items.push(instantiate_constant(instance, part)?);
            }
            Some(instance.new_set(items))
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
