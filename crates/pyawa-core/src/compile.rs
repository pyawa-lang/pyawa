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
//! - **表达式**：十进制整数字面量、单引号字符串字面量、名字、`+`（左结合）
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
}

/// 编译失败。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CompileError {
    /// 语法不认识（消息是本层自己的，**还没**对齐参照实现的 `SyntaxError` 文本）。
    Syntax(String),
    /// 认识但还没接线（消息说明是哪一件）。
    Unsupported(String),
}

/// 编译一段源码（`BC-16`：纯函数——同样的入参给同样的产物）。
pub fn compile(source: &str, filename: &str, mode: Mode) -> Result<CompiledUnit, CompileError> {
    // `BC-14`：模式是显式入参。`BC-15` 要求纯 Python 模式拒绝扩展语法——而 `§13-12` 已决
    // "扩展特性清单为空"，所以此刻两种模式的产物相同（`filename` 也还不进产物：位置表未接线）。
    let _ = (filename, mode);
    let statements = parse_module(source)?;
    if statements.is_empty() {
        return Err(CompileError::Syntax("没有语句".to_owned()));
    }
    compile_scope("<module>", &[], &statements, ScopeKind::Module)
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum ScopeKind {
    Module,
    Function,
}

/// 编译一个作用域（模块或函数）⇒ 一个 [`CompiledUnit`]。
fn compile_scope(
    name: &str,
    parameters: &[String],
    statements: &[Statement],
    kind: ScopeKind,
) -> Result<CompiledUnit, CompileError> {
    let mut emitter = Emitter {
        pending: Vec::new(),
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
        },
        kind,
    };
    emitter.emit_base(opcode::opcode("RESUME").expect("RESUME 在表里"), 0);
    for statement in statements {
        emitter.emit_statement(statement)?;
    }
    // 收尾顺序照实测：
    //   模块：先登记 `None`（`LOAD_CONST <None>` ＋ `RETURN_VALUE`），**然后**才把折叠出来的
    //         常量追加进表尾（`x = 200 + 100` ⇒ `[200, None, 300]`）
    //   函数：先冲刷折叠常量（`return 200 + 100` ⇒ `[200, 300]`），再判"表还空着就登记 None"
    if kind == ScopeKind::Module {
        let none_index = emitter.intern_constant(Constant::None);
        emitter.emit_base(
            opcode::opcode("LOAD_CONST").expect("LOAD_CONST 在表里"),
            none_index as u8,
        );
        emitter.emit_base(opcode::opcode("RETURN_VALUE").expect("RETURN_VALUE 在表里"), 0);
        emitter.flush_pending();
    } else {
        emitter.flush_pending();
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
    /// **折叠出来的常量**：它们的登记时机在**收尾之后**（实测），所以先记下"要回填的
    /// `LOAD_CONST` 实参位置"，等收尾时统一登记并回填。
    pending: Vec<(usize, Constant)>,
}

impl Emitter {
    /// 发射一条指令（`opcode` ＋ `oparg`，每码元 2 字节），并按 `BC-35`／`BC-36` 补零填充缓存槽。
    fn emit_base(&mut self, opcode: u16, oparg: u8) {
        self.unit.code.push(opcode as u8);
        self.unit.code.push(oparg);
        for _ in 0..opcode::inline_cache_entries(opcode) {
            self.unit.code.push(0);
            self.unit.code.push(0);
        }
    }

    /// 收尾时把"待定常量"登记进表并把先前那条 `LOAD_CONST` 的实参回填。
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

    fn intern_name(&mut self, name: &str) -> usize {
        if let Some(index) = self.unit.names.iter().position(|item| item == name) {
            return index;
        }
        self.unit.names.push(name.to_owned());
        self.unit.names.len() - 1
    }

    /// 登记一个**字面量**常量，规矩照实测：
    /// 小整数**只在常量表还是空的时候**才登记（`x = 1` ⇒ `[1, None]`，但 `def f(): return 1`
    /// 之后再 `x = 1` ⇒ `[<code f>, None]`）；大整数与字符串**总是**登记。
    fn intern_literal(&mut self, constant: Constant) -> Option<usize> {
        if let Constant::Int(value) = constant {
            if (0..=255).contains(&value) {
                if !self.unit.constants.is_empty() {
                    return None;
                }
            }
        }
        Some(self.intern_constant(constant))
    }

    /// 局部槽位（没有就按首次出现顺序追加 —— 形参已经在前面）。
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
            Statement::Assign { target, value } => {
                // 右值最外层是局部时用 `LOAD_FAST`（实测：`x = a` ⇒ `LOAD_FAST 0`）
                match (self.kind, value) {
                    (ScopeKind::Function, Expression::Name(name))
                        if self.unit.varnames.iter().any(|item| item == name) =>
                    {
                        let slot = self.slot_of(name);
                        self.emit_base(
                            opcode::opcode("LOAD_FAST").expect("LOAD_FAST 在表里"),
                            slot as u8,
                        );
                    }
                    _ => self.emit_expression(value)?,
                }
                match self.kind {
                    ScopeKind::Module => {
                        let index = self.intern_name(target);
                        self.emit_base(
                            opcode::opcode("STORE_NAME").expect("STORE_NAME 在表里"),
                            index as u8,
                        );
                    }
                    ScopeKind::Function => {
                        let slot = self.slot_of(target);
                        self.emit_base(
                            opcode::opcode("STORE_FAST").expect("STORE_FAST 在表里"),
                            slot as u8,
                        );
                    }
                }
                Ok(())
            }
            Statement::Return(value) => {
                self.emit_expression(value)?;
                self.emit_base(
                    opcode::opcode("RETURN_VALUE").expect("RETURN_VALUE 在表里"),
                    0,
                );
                Ok(())
            }
            Statement::Def {
                name,
                parameters,
                body,
            } => {
                if self.kind != ScopeKind::Module {
                    return Err(CompileError::Unsupported("嵌套的函数定义尚未接线".to_owned()));
                }
                let nested = compile_scope(name, parameters, body, ScopeKind::Function)?;
                let index = self.intern_constant(Constant::Code(Box::new(nested)));
                self.emit_base(
                    opcode::opcode("LOAD_CONST").expect("LOAD_CONST 在表里"),
                    index as u8,
                );
                // 3.14 的 `MAKE_FUNCTION` **没有 oparg**（`dis` 显示 `arg=None`）
                self.emit_base(opcode::opcode("MAKE_FUNCTION").expect("MAKE_FUNCTION 在表里"), 0);
                let name_index = self.intern_name(name);
                self.emit_base(
                    opcode::opcode("STORE_NAME").expect("STORE_NAME 在表里"),
                    name_index as u8,
                );
                Ok(())
            }
        }
    }

    fn emit_expression(&mut self, expression: &Expression) -> Result<(), CompileError> {
        match expression {
            Expression::Int(value) => {
                if (0..=255).contains(value) {
                    self.intern_literal(Constant::Int(*value));
                    self.emit_base(
                        opcode::opcode("LOAD_SMALL_INT").expect("LOAD_SMALL_INT 在表里"),
                        *value as u8,
                    );
                } else {
                    let index = self.intern_constant(Constant::Int(*value));
                    self.emit_base(
                        opcode::opcode("LOAD_CONST").expect("LOAD_CONST 在表里"),
                        index as u8,
                    );
                }
                Ok(())
            }
            Expression::Str(text) => {
                let index = self.intern_constant(Constant::Str(text.clone()));
                self.emit_base(
                    opcode::opcode("LOAD_CONST").expect("LOAD_CONST 在表里"),
                    index as u8,
                );
                Ok(())
            }
            Expression::Name(name) => {
                if self.kind == ScopeKind::Function {
                    if self.unit.varnames.iter().any(|item| item == name) {
                        let slot = self.slot_of(name);
                        self.emit_base(
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
                self.emit_base(
                    opcode::opcode("LOAD_NAME").expect("LOAD_NAME 在表里"),
                    index as u8,
                );
                Ok(())
            }
            Expression::Add(left, right) => {
                // **常量折叠**（实测三条规则）：
                //   ① 只有**最左叶子**字面量进常量表（`1 + 2 + 3` ⇒ 常量表里只有 1）
                //   ② 折叠结果是小整数 ⇒ `LOAD_SMALL_INT`，**不**进常量表
                //   ③ 否则该常量**在收尾之后**才登记（`x = 200 + 100` ⇒ `[200, None, 300]`）
                if let Some(folded) = fold_constant(expression)? {
                    if let Some(leaf) = leftmost_literal(expression) {
                        self.intern_literal(leaf);
                    }
                    match folded {
                        Constant::Int(value) if (0..=255).contains(&value) => {
                            self.emit_base(
                                opcode::opcode("LOAD_SMALL_INT")
                                    .expect("LOAD_SMALL_INT 在表里"),
                                value as u8,
                            );
                        }
                        other => {
                            // 先发射占位，收尾时统一登记并回填实参
                            let argument_byte = self.unit.code.len() + 1;
                            self.emit_base(
                                opcode::opcode("LOAD_CONST").expect("LOAD_CONST 在表里"),
                                0,
                            );
                            self.pending.push((argument_byte, other));
                        }
                    }
                    return Ok(());
                }
                // 实测：两侧都是**局部**借入加载时打成超指令（高 4 位先压、低 4 位后压）
                let pack = match (self.kind, &**left, &**right) {
                    (ScopeKind::Function, Expression::Name(a), Expression::Name(b)) => {
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
                    self.emit_base(
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
                self.emit_base(
                    opcode::opcode("BINARY_OP").expect("BINARY_OP 在表里"),
                    plus,
                );
                Ok(())
            }
        }
    }
}

/// 把一段**全常量**表达式求值（`+` 的常量折叠）；不是全常量给 `None`。
fn fold_constant(expression: &Expression) -> Result<Option<Constant>, CompileError> {
    match expression {
        Expression::Int(value) => Ok(Some(Constant::Int(*value))),
        Expression::Str(text) => Ok(Some(Constant::Str(text.clone()))),
        Expression::Name(_) => Ok(None),
        Expression::Add(left, right) => {
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
        Expression::Int(value) => Some(Constant::Int(*value)),
        Expression::Str(text) => Some(Constant::Str(text.clone())),
        Expression::Name(_) => None,
        Expression::Add(left, _) => leftmost_literal(left),
    }
}

// ---- 语法树 ----

#[derive(Debug, Clone, PartialEq, Eq)]
enum Expression {
    Int(i64),
    Str(String),
    Name(String),
    Add(Box<Expression>, Box<Expression>),
}

/// 模块级／缩进块里的语句（本层只接线这几种）。
#[derive(Debug, Clone, PartialEq, Eq)]
enum Statement {
    Assign { target: String, value: Expression },
    Return(Expression),
    Def {
        name: String,
        parameters: Vec<String>,
        body: Vec<Statement>,
    },
}

// ---- 词法 ----

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
    End,
}

fn lex(source: &str) -> Result<Vec<Lexeme>, CompileError> {
    let characters: Vec<char> = source.chars().collect();
    let mut index = 0usize;
    let mut tokens = Vec::new();
    let mut indents: Vec<usize> = vec![0];
    let mut at_line_start = true;
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
                }
                continue;
            }
            let current = *indents.last().expect("至少有一个");
            if width > current {
                indents.push(width);
                tokens.push(Lexeme::Indent);
            } else if width < current {
                while *indents.last().expect("至少有一个") > width {
                    indents.pop();
                    tokens.push(Lexeme::Dedent);
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
                tokens.push(Lexeme::Newline);
                at_line_start = true;
                index += 1;
            }
            '=' => {
                tokens.push(Lexeme::Assign);
                index += 1;
            }
            '+' => {
                tokens.push(Lexeme::Plus);
                index += 1;
            }
            ':' => {
                tokens.push(Lexeme::Colon);
                index += 1;
            }
            '(' => {
                tokens.push(Lexeme::LeftParen);
                index += 1;
            }
            ')' => {
                tokens.push(Lexeme::RightParen);
                index += 1;
            }
            ',' => {
                tokens.push(Lexeme::Comma);
                index += 1;
            }
            ';' => {
                tokens.push(Lexeme::Newline);
                index += 1;
            }
            '#' => {
                while !matches!(characters.get(index), Some('\n') | None) {
                    index += 1;
                }
            }
            '\'' => {
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
                tokens.push(Lexeme::Str(text));
            }
            character if character.is_ascii_digit() => {
                let start = index;
                while index < characters.len() && characters[index].is_ascii_digit() {
                    index += 1;
                }
                let text: String = characters[start..index].iter().collect();
                let value = text
                    .parse::<i64>()
                    .map_err(|_| CompileError::Unsupported(format!("整数 {text} 超出本层范围")))?;
                tokens.push(Lexeme::Int(value));
            }
            character if character.is_alphabetic() || character == '_' => {
                let start = index;
                while index < characters.len()
                    && (characters[index].is_alphanumeric() || characters[index] == '_')
                {
                    index += 1;
                }
                let text: String = characters[start..index].iter().collect();
                tokens.push(match text.as_str() {
                    "return" => Lexeme::Return,
                    "def" => Lexeme::Def,
                    _ => Lexeme::Name(text),
                });
            }
            other => return Err(CompileError::Syntax(format!("不认识的字符 {other:?}"))),
        }
    }
    while indents.len() > 1 {
        indents.pop();
        tokens.push(Lexeme::Dedent);
    }
    tokens.push(Lexeme::End);
    Ok(tokens)
}

// ---- 语法 ----

fn parse_module(source: &str) -> Result<Vec<Statement>, CompileError> {
    let tokens = lex(source)?;
    let mut cursor = 0usize;
    let statements = parse_statements(&tokens, &mut cursor, 0, false)?;
    if tokens.get(cursor) != Some(&Lexeme::End) {
        return Err(CompileError::Syntax(format!(
            "模块结尾多出了 {:?}",
            tokens.get(cursor)
        )));
    }
    Ok(statements)
}

fn parse_statements(
    tokens: &[Lexeme],
    cursor: &mut usize,
    depth: usize,
    in_function: bool,
) -> Result<Vec<Statement>, CompileError> {
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
                let body = parse_statements(tokens, cursor, depth + 1, true)?;
                if tokens.get(*cursor) != Some(&Lexeme::Dedent) {
                    return Err(CompileError::Syntax("`def` 的体没有正常收尾".to_owned()));
                }
                *cursor += 1;
                statements.push(Statement::Def {
                    name,
                    parameters,
                    body,
                });
            }
            Some(Lexeme::Return) => {
                if !in_function {
                    return Err(CompileError::Syntax(
                        "模块级的 `return`（参照实现也是 SyntaxError）".to_owned(),
                    ));
                }
                *cursor += 1;
                let (value, next) = parse_expression(tokens, *cursor)?;
                *cursor = next;
                statements.push(Statement::Return(value));
                expect_statement_end(tokens, cursor)?;
            }
            Some(Lexeme::Name(target)) => {
                let target = target.clone();
                *cursor += 1;
                if tokens.get(*cursor) != Some(&Lexeme::Assign) {
                    return Err(CompileError::Unsupported(
                        "只接线了 `名字 = 表达式` 与 `return`".to_owned(),
                    ));
                }
                *cursor += 1;
                let (value, next) = parse_expression(tokens, *cursor)?;
                *cursor = next;
                statements.push(Statement::Assign { target, value });
                expect_statement_end(tokens, cursor)?;
            }
            other => {
                return Err(CompileError::Syntax(format!("不认识的语句开头 {other:?}")));
            }
        }
    }
    Ok(statements)
}

fn expect_statement_end(tokens: &[Lexeme], cursor: &mut usize) -> Result<(), CompileError> {
    match tokens.get(*cursor) {
        Some(Lexeme::Newline) | Some(Lexeme::End) | Some(Lexeme::Dedent) => Ok(()),
        other => Err(CompileError::Syntax(format!("语句结尾多出了 {other:?}"))),
    }
}

fn parse_expression(tokens: &[Lexeme], cursor: usize) -> Result<(Expression, usize), CompileError> {
    let (mut left, mut cursor) = parse_term(tokens, cursor)?;
    while tokens.get(cursor) == Some(&Lexeme::Plus) {
        let (right, next) = parse_term(tokens, cursor + 1)?;
        left = Expression::Add(Box::new(left), Box::new(right));
        cursor = next;
    }
    Ok((left, cursor))
}

fn parse_term(tokens: &[Lexeme], cursor: usize) -> Result<(Expression, usize), CompileError> {
    match tokens.get(cursor) {
        Some(Lexeme::Int(value)) => Ok((Expression::Int(*value), cursor + 1)),
        Some(Lexeme::Str(text)) => Ok((Expression::Str(text.clone()), cursor + 1)),
        Some(Lexeme::Name(name)) => Ok((Expression::Name(name.clone()), cursor + 1)),
        other => Err(CompileError::Syntax(format!("表达式里出现 {other:?}"))),
    }
}
