//! **源码 → 字节码**（`P1-10`；契约 `BC-14`…`BC-18`，族级映射见 `SPEC-bytecode.md` §11）。
//!
//! 这一轮只做**最小可测切片**，但它是真编译器：词法 → 语法 → 发射，产物过 `validate`
//! （`BC-36` 的零填充缓存槽也照做），并与参照实现的**逐条指令**对拍
//! （`tools/gen_compile_fixture.py` 导出的夹具）。
//!
//! # 覆盖到的构造（其余如实报未接线）
//!
//! - 语句：`NAME = <表达式>`，用 `;` 或换行分隔（模块级）
//! - 表达式：十进制整数字面量、单引号字符串字面量、名字、`+`（左结合）
//! - 发射细节都是**实测**来的：小整数 `0..=255` 走 `LOAD_SMALL_INT`（`oparg` 就是值），
//!   但**常量表里照样登记**（参照实现如此）；名字表按**发射顺序**登记（值先于目标）；
//!   `None` 在最后登记（`LOAD_CONST <None>` ＋ `RETURN_VALUE` 收尾）
//!
//! # 明确未接线（照实说，不猜）
//!
//! - **常量折叠**：实测 `x = 1 + 2` 折叠成 `LOAD_SMALL_INT 3`，而常量表里留下的是**操作数 1**
//!   （`x = 200 + 100` 更怪：常量表 `[200, None, 300]`——折叠发生在 epilogue 之后）。
//!   这是参照实现的**内部顺序**细节，本层暂不模仿，遇到"两侧都是整数字面量"的 `+` 就报
//!   `Unsupported`（组装产物时也**不**把它放进对拍语料）。
//! - 缩进块（`def`／`if`…）、负数常量、字符串转义、`EXTENDED_ARG`（`oparg > 255`）、
//!   位置表（`BC-18` 的 `co_positions()`）、扩展模式的边界检查指令（`BC-23`…`BC-28`）。

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
}

/// 编译产物（**纯数据**：带着造 `CodeObject` 需要的一切，不带任何实例／路径／时间）。
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
    // "扩展特性清单为空"，所以此刻两种模式的产物相同（`filename` 目前也不进产物：位置表未接线）。
    let _ = (filename, mode);
    let mut emitter = Emitter::default();
    emitter.emit_base(opcode::opcode("RESUME").expect("RESUME 在表里"), 0);
    let statements = parse_statements(source)?;
    if statements.is_empty() {
        return Err(CompileError::Syntax("没有语句".to_owned()));
    }
    for statement in &statements {
        emitter.emit_statement(statement)?;
    }
    // 收尾（实测）：先登记 `None`，再 `LOAD_CONST <None>` ＋ `RETURN_VALUE`
    let none_index = emitter.intern_constant(Constant::None);
    emitter.emit_base(opcode::opcode("LOAD_CONST").expect("LOAD_CONST 在表里"), none_index as u8);
    emitter.emit_base(opcode::opcode("RETURN_VALUE").expect("RETURN_VALUE 在表里"), 0);
    Ok(CompiledUnit {
        name: "<module>".to_owned(),
        argcount: 0,
        posonlyargcount: 0,
        kwonlyargcount: 0,
        nlocals: 0,
        flags: 0,
        names: emitter.names,
        varnames: Vec::new(),
        constants: emitter.constants,
        code: emitter.code,
    })
}

// ---- 语法树 ----

#[derive(Debug, Clone, PartialEq, Eq)]
enum Expression {
    Int(i64),
    Str(String),
    Name(String),
    Add(Box<Expression>, Box<Expression>),
}

// ---- 词法 ----

#[derive(Debug, Clone, PartialEq, Eq)]
enum Token {
    Name(String),
    Int(i64),
    Str(String),
    Assign,
    Plus,
    Semicolon,
    Newline,
    End,
}

fn tokenize(source: &str) -> Result<Vec<Token>, CompileError> {
    let characters: Vec<char> = source.chars().collect();
    let mut index = 0usize;
    let mut tokens = Vec::new();
    while index < characters.len() {
        let character = characters[index];
        match character {
            ' ' | '\t' | '\r' => index += 1,
            '\n' => {
                tokens.push(Token::Newline);
                index += 1;
            }
            ';' => {
                tokens.push(Token::Semicolon);
                index += 1;
            }
            '=' => {
                tokens.push(Token::Assign);
                index += 1;
            }
            '+' => {
                tokens.push(Token::Plus);
                index += 1;
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
                        Some(character) => {
                            if *character == '\\' {
                                // 转义还没接线：如实报，不猜
                                return Err(CompileError::Unsupported(
                                    "字符串转义尚未接线".to_owned(),
                                ));
                            }
                            text.push(*character);
                            index += 1;
                        }
                        None => {
                            return Err(CompileError::Syntax("字符串没有收尾引号".to_owned()))
                        }
                    }
                }
                tokens.push(Token::Str(text));
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
                tokens.push(Token::Int(value));
            }
            character if character.is_alphabetic() || character == '_' => {
                let start = index;
                while index < characters.len()
                    && (characters[index].is_alphanumeric() || characters[index] == '_')
                {
                    index += 1;
                }
                tokens.push(Token::Name(characters[start..index].iter().collect()));
            }
            other => {
                return Err(CompileError::Syntax(format!("不认识的字符 {other:?}")));
            }
        }
    }
    tokens.push(Token::End);
    Ok(tokens)
}

// ---- 语法 ----

fn parse_statements(source: &str) -> Result<Vec<(String, Expression)>, CompileError> {
    let tokens = tokenize(source)?;
    let mut cursor = 0usize;
    let mut statements = Vec::new();
    loop {
        // 跳过分隔符
        while matches!(tokens.get(cursor), Some(Token::Newline) | Some(Token::Semicolon)) {
            cursor += 1;
        }
        if tokens.get(cursor) == Some(&Token::End) {
            break;
        }
        let target = match tokens.get(cursor) {
            Some(Token::Name(name)) => name.clone(),
            other => {
                return Err(CompileError::Syntax(format!(
                    "语句要以名字开头，实际是 {other:?}"
                )))
            }
        };
        cursor += 1;
        if tokens.get(cursor) != Some(&Token::Assign) {
            return Err(CompileError::Unsupported(
                "只接线了 `名字 = 表达式` 这种语句".to_owned(),
            ));
        }
        cursor += 1;
        let (expression, next) = parse_expression(&tokens, cursor)?;
        cursor = next;
        match tokens.get(cursor) {
            Some(Token::Newline) | Some(Token::Semicolon) | Some(Token::End) => {}
            other => {
                return Err(CompileError::Syntax(format!(
                    "语句结尾多出了 {other:?}"
                )))
            }
        }
        statements.push((target, expression));
    }
    Ok(statements)
}

fn parse_expression(tokens: &[Token], cursor: usize) -> Result<(Expression, usize), CompileError> {
    let (mut left, mut cursor) = parse_term(tokens, cursor)?;
    while tokens.get(cursor) == Some(&Token::Plus) {
        let (right, next) = parse_term(tokens, cursor + 1)?;
        left = Expression::Add(Box::new(left), Box::new(right));
        cursor = next;
    }
    Ok((left, cursor))
}

fn parse_term(tokens: &[Token], cursor: usize) -> Result<(Expression, usize), CompileError> {
    match tokens.get(cursor) {
        Some(Token::Int(value)) => Ok((Expression::Int(*value), cursor + 1)),
        Some(Token::Str(text)) => Ok((Expression::Str(text.clone()), cursor + 1)),
        Some(Token::Name(name)) => Ok((Expression::Name(name.clone()), cursor + 1)),
        other => Err(CompileError::Syntax(format!("表达式里出现 {other:?}"))),
    }
}

// ---- 发射 ----

#[derive(Default)]
struct Emitter {
    code: Vec<u8>,
    names: Vec<String>,
    constants: Vec<Constant>,
}

impl Emitter {
    /// 发射一条**不带缓存**的指令（`opcode` ＋ `oparg`，每码元 2 字节）。
    fn emit_base(&mut self, opcode: u16, oparg: u8) {
        self.code.push(opcode as u8);
        self.code.push(oparg);
        // `BC-35`／`BC-36`：带缓存的指令后必须留等宽**零填充**码元
        for _ in 0..opcode::inline_cache_entries(opcode) {
            self.code.push(0);
            self.code.push(0);
        }
    }

    /// 登记一个常量（**去重**；返回下标）。
    fn intern_constant(&mut self, constant: Constant) -> usize {
        if let Some(index) = self.constants.iter().position(|item| *item == constant) {
            return index;
        }
        self.constants.push(constant);
        self.constants.len() - 1
    }

    /// 登记一个名字（**去重**；返回下标）。
    fn intern_name(&mut self, name: &str) -> usize {
        if let Some(index) = self.names.iter().position(|item| item == name) {
            return index;
        }
        self.names.push(name.to_owned());
        self.names.len() - 1
    }

    fn emit_statement(&mut self, statement: &(String, Expression)) -> Result<(), CompileError> {
        self.emit_expression(&statement.1)?;
        // 目标名字在**值之后**登记（实测 `x = y` 的名字表是 `('y', 'x')`）
        let index = self.intern_name(&statement.0);
        self.emit_base(
            opcode::opcode("STORE_NAME").expect("STORE_NAME 在表里"),
            index as u8,
        );
        Ok(())
    }

    fn emit_expression(&mut self, expression: &Expression) -> Result<(), CompileError> {
        match expression {
            Expression::Int(value) => {
                // 实测：小整数走 `LOAD_SMALL_INT`（`oparg` 就是值），但常量表里**照样登记**
                let index = self.intern_constant(Constant::Int(*value));
                if (0..=255).contains(value) {
                    self.emit_base(
                        opcode::opcode("LOAD_SMALL_INT").expect("LOAD_SMALL_INT 在表里"),
                        *value as u8,
                    );
                } else {
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
                let index = self.intern_name(name);
                self.emit_base(
                    opcode::opcode("LOAD_NAME").expect("LOAD_NAME 在表里"),
                    index as u8,
                );
                Ok(())
            }
            Expression::Add(left, right) => {
                if matches!(**left, Expression::Int(_)) && matches!(**right, Expression::Int(_)) {
                    return Err(CompileError::Unsupported(
                        "常量折叠（两侧都是整数字面量）尚未接线：实测参照实现在常量表里留下的顺序\
                         是内部细节，本层不猜"
                            .to_owned(),
                    ));
                }
                self.emit_expression(left)?;
                self.emit_expression(right)?;
                // `BINARY_OP` 的 oparg 是 `nb_ops` 表的下标（实测 `+` 是 0）
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
