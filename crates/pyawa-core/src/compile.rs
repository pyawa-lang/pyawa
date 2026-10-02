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

/// 常量表里的一项。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Constant {
    /// `None`。
    None,
    /// 整数。
    Int(i64),
    /// 字符串。
    Str(String),
    /// 嵌套的 code object（本层只有函数体那一种）。
    Code(Box<CompiledUnit>),
    /// **关键字名元组**（`CALL_KW` 之前那条 `LOAD_CONST`；实测紧邻它、名序照源码顺序）。
    Names(Vec<String>),
}

/// 编译产物（**纯数据**）。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CompiledUnit {
    /// `co_name`。
    pub name: String,
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
pub fn compile(source: &str, filename: &str, mode: Mode) -> Result<CompiledUnit, CompileError> {
    // `BC-14`：模式是显式入参。`BC-15` 要求纯 Python 模式拒绝扩展语法——而 `§13-12` 已决
    // "扩展特性清单为空"，所以此刻两种模式的产物相同（`filename` 也还不进产物）。
    let _ = (filename, mode);
    let lexed = lex(source)?;
    let statements = parse_module(&lexed)?;
    if statements.is_empty() {
        return Err(CompileError::Syntax("没有语句".to_owned()));
    }
    compile_scope(
        "<module>",
        &[],
        &statements,
        ScopeKind::Module,
        Span::synthetic(),
    )
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum ScopeKind {
    Module,
    Function,
}

/// 编译一个作用域（模块或函数）⇒ 一个 [`CompiledUnit`]。
///
/// `first_line` 是这个作用域第一行的行号（模块是 1，函数是 `def` 那一行——实测函数
/// `RESUME` 的位置是 `(def 行, def 行, 0, 0)`）。
fn compile_scope(
    name: &str,
    parameters: &[String],
    statements: &[Statement],
    kind: ScopeKind,
    resume_span: Span,
) -> Result<CompiledUnit, CompileError> {
    let mut emitter = Emitter {
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
            argcount: parameters.len(),
            posonlyargcount: 0,
            kwonlyargcount: 0,
            nlocals: parameters.len(),
            flags: if kind == ScopeKind::Function { 0x3 } else { 0 },
            names: Vec::new(),
            varnames: parameters.to_vec(),
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
    let last_index = statements.len().saturating_sub(1);
    for (index, statement) in statements.iter().enumerate() {
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
        emitter.flush_jumps();
    } else if kind == ScopeKind::Module {
        // 不需要收尾（末尾 `if/else` 两分支都 return）
        emitter.flush_pending();
        emitter.flush_jumps();
    } else {
        emitter.flush_pending();
        emitter.flush_jumps();
        if emitter.unit.constants.is_empty() {
            // 实测：函数自己没有任何常量时，常量表里会登记一个 `None`（原因不明，规则照实写下来）
            emitter.intern_constant(Constant::None);
        }
    }
    Ok(emitter.unit)
}

struct Emitter {
    unit: CompiledUnit,
    kind: ScopeKind,
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
                    ScopeKind::Module => {
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
            Statement::Return(value, span) => {
                self.emit_expression(value)?;
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
                    ScopeKind::Module => {
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
            Statement::Def {
                name,
                span,
                first_line,
                parameters,
                body,
            } => {
                if self.kind != ScopeKind::Module {
                    return Err(CompileError::Unsupported("嵌套的函数定义尚未接线".to_owned()));
                }
                let nested = compile_scope(
                    name,
                    parameters,
                    body,
                    ScopeKind::Function,
                    Span::new(*first_line, *first_line, 0, 0),
                )?;
                let index = self.intern_constant(Constant::Code(Box::new(nested)));
                // 实测：`def` 的三条指令（＋收尾）位置都是**整个 `def` 语句**
                self.emit_at(
                    *span,
                    opcode::opcode("LOAD_CONST").expect("LOAD_CONST 在表里"),
                    index as u8,
                );
                // 3.14 的 `MAKE_FUNCTION` **没有 oparg**（`dis` 显示 `arg=None`）
                self.emit_at(
                    *span,
                    opcode::opcode("MAKE_FUNCTION").expect("MAKE_FUNCTION 在表里"),
                    0,
                );
                let name_index = self.intern_name(name);
                self.emit_at(
                    *span,
                    opcode::opcode("STORE_NAME").expect("STORE_NAME 在表里"),
                    name_index as u8,
                );
                // 收尾两条跟 `def` 的整段（实测：`def f(): return 1` 的五条位置都是它）
                self.epilogue_span = *span;
                Ok(())
            }
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
                    return Err(CompileError::Unsupported(format!(
                        "函数里读非局部名字 `{name}`（`LOAD_GLOBAL` 一族）尚未接线"
                    )));
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
                self.emit_expression(function)?;
                self.emit_at(
                    *callee_span,
                    opcode::opcode("PUSH_NULL").expect("PUSH_NULL 在表里"),
                    0,
                );
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
    Name(String, Span),
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
            | Expression::Name(_, span)
            | Expression::Add(_, _, span)
            | Expression::Compare(_, _, _, span)
            | Expression::Call { span, .. } => *span,
        }
    }
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
    Def {
        name: String,
        span: Span,
        first_line: u32,
        parameters: Vec<String>,
        body: Vec<Statement>,
    },
}

/// 把一段**全常量**表达式求值（`+` 的常量折叠）；不是全常量给 `None`。
fn fold_constant(expression: &Expression) -> Result<Option<Constant>, CompileError> {
    match expression {
        Expression::Int(value, _) => Ok(Some(Constant::Int(*value))),
        Expression::Str(text, _) => Ok(Some(Constant::Str(text.clone()))),
        Expression::Name(_, _)
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
                    let sum = x.checked_add(y).ok_or_else(|| {
                        CompileError::Unsupported(
                            "整数字面量相加溢出（任意精度整数还没接线）".to_owned(),
                        )
                    })?;
                    Ok(Some(Constant::Int(sum)))
                }
                (Constant::Str(x), Constant::Str(y)) => Ok(Some(Constant::Str(x + &y))),
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
        Expression::Name(_, _)
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
    Assign,
    Plus,
    Colon,
    LeftParen,
    RightParen,
    Comma,
    Newline,
    Indent,
    Dedent,
    Return,
    Def,
    If,
    Else,
    While,
    For,
    In,
    Star,
    DoubleStar,
    Less,
    LessEqual,
    Greater,
    GreaterEqual,
    EqualEqual,
    NotEqual,
    End,
}

/// 词法结果：单元 ＋ 与它**一一对应**的跨度。
struct Lexed {
    lexemes: Vec<Lexeme>,
    spans: Vec<Span>,
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
            '+' | ':' | '(' | ')' | ',' | ';' => {
                let start = column!(index);
                lexemes.push(match character {
                    '=' => Lexeme::Assign,
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
            '\'' => {
                let start = column!(index);
                index += 1;
                let mut text = String::new();
                loop {
                    match characters.get(index) {
                        Some('\'') => {
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
                    "def" => Lexeme::Def,
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
                let mut parameters = Vec::new();
                loop {
                    match tokens.get(*cursor) {
                        Some(Lexeme::RightParen) => {
                            *cursor += 1;
                            break;
                        }
                        Some(Lexeme::Name(parameter)) if parameters.is_empty() => {
                            parameters.push(parameter.clone());
                            *cursor += 1;
                        }
                        Some(Lexeme::Comma) => {
                            *cursor += 1;
                            match tokens.get(*cursor) {
                                Some(Lexeme::Name(parameter)) => {
                                    parameters.push(parameter.clone());
                                    *cursor += 1;
                                }
                                other => {
                                    return Err(CompileError::Syntax(format!(
                                        "形参表里出现 {other:?}"
                                    )))
                                }
                            }
                        }
                        other => {
                            return Err(CompileError::Syntax(format!("形参表里出现 {other:?}")))
                        }
                    }
                }
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
        Some(Lexeme::Name(name)) => (Expression::Name(name.clone(), span), cursor + 1),
        other => return Err(CompileError::Syntax(format!("表达式里出现 {other:?}"))),
    };
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
        .map(|constant| match constant {
            Constant::None => Some(instance.retain(instance.singletons().none())),
            Constant::Int(value) => Some(instance.new_int(*value)),
            Constant::Str(text) => Some(instance.new_str(text)),
            Constant::Code(inner) => Some(instantiate(instance, inner).into_raw().cast()),
            Constant::Names(names) => {
                let items: Vec<core::ptr::NonNull<crate::Header>> =
                    names.iter().map(|name| instance.new_str(name)).collect();
                Some(instance.new_tuple(items))
            }
        })
        .collect();
    instance.alloc(crate::CodeObject::new(
        code_type,
        "<module>",
        unit.name.clone(),
        "<pyawa-test>".to_owned(),
        1,
        unit.nlocals.max(1),
        unit.nlocals,
        unit.argcount,
        unit.posonlyargcount,
        unit.kwonlyargcount,
        unit.flags,
        unit.varnames.clone(),
        unit.names.clone(),
        Vec::new(),
        Vec::new(),
        unit.code.clone(),
        Vec::new(),
        consts,
        unit.positions.clone(),
    ))
}
