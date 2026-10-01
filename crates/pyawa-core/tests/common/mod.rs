//! 集成测试共用的小工具：够用的 JSON 解析器（`pyawa-core` 不带 JSON 依赖）。
//!
//! 期望值一律来自 `tests/fixture-*.json`（生成器见 `tools/gen_*_fixture.py`）。

// 共用脚手架：各测试只用到其中一部分，故整体关掉 dead_code
#![allow(dead_code)]

// --------------------------------------------------------------------------- #
// 极简 JSON（够读夹具：对象／数组／字符串／整数／bool／null）
// --------------------------------------------------------------------------- #

#[derive(Debug, Clone, PartialEq)]
pub enum Json {
    Null,
    Bool(bool),
    Num(i64),
    Str(String),
    Arr(Vec<Json>),
    Obj(Vec<(String, Json)>),
}

impl Json {
    pub fn get(&self, key: &str) -> Option<&Json> {
        match self {
            Json::Obj(entries) => entries.iter().find(|(name, _)| name == key).map(|(_, v)| v),
            _ => None,
        }
    }

    pub fn key(&self, key: &str) -> &Json {
        self.get(key).unwrap_or_else(|| panic!("夹具缺字段 {key}"))
    }

    pub fn as_i64(&self) -> i64 {
        match self {
            Json::Num(value) => *value,
            other => panic!("期望整数，得到 {other:?}"),
        }
    }

    pub fn as_bool(&self) -> bool {
        match self {
            Json::Bool(value) => *value,
            other => panic!("期望布尔，得到 {other:?}"),
        }
    }

    pub fn as_str(&self) -> &str {
        match self {
            Json::Str(value) => value,
            other => panic!("期望字符串，得到 {other:?}"),
        }
    }

    pub fn as_arr(&self) -> &[Json] {
        match self {
            Json::Arr(items) => items,
            other => panic!("期望数组，得到 {other:?}"),
        }
    }

    pub fn as_obj(&self) -> &[(String, Json)] {
        match self {
            Json::Obj(entries) => entries,
            other => panic!("期望对象，得到 {other:?}"),
        }
    }
}

struct Parser<'a> {
    bytes: &'a [u8],
    position: usize,
}

impl<'a> Parser<'a> {
    fn new(text: &'a str) -> Self {
        Self { bytes: text.as_bytes(), position: 0 }
    }

    fn parse(mut self) -> Json {
        self.skip_whitespace();
        let value = self.value();
        self.skip_whitespace();
        value
    }

    fn peek(&self) -> u8 {
        *self.bytes.get(self.position).unwrap_or(&0)
    }

    fn skip_whitespace(&mut self) {
        while matches!(self.peek(), b' ' | b'\n' | b'\r' | b'\t') {
            self.position += 1;
        }
    }

    fn value(&mut self) -> Json {
        match self.peek() {
            b'{' => self.object(),
            b'[' => self.array(),
            b'"' => Json::Str(self.string()),
            b't' => {
                self.literal("true");
                Json::Bool(true)
            }
            b'f' => {
                self.literal("false");
                Json::Bool(false)
            }
            b'n' => {
                self.literal("null");
                Json::Null
            }
            _ => self.number(),
        }
    }

    fn literal(&mut self, text: &str) {
        assert!(
            self.bytes[self.position..].starts_with(text.as_bytes()),
            "字面量 {text} 不匹配"
        );
        self.position += text.len();
    }

    fn object(&mut self) -> Json {
        self.position += 1; // '{'
        let mut entries = Vec::new();
        self.skip_whitespace();
        if self.peek() == b'}' {
            self.position += 1;
            return Json::Obj(entries);
        }
        loop {
            self.skip_whitespace();
            let key = self.string();
            self.skip_whitespace();
            assert_eq!(self.peek(), b':');
            self.position += 1;
            self.skip_whitespace();
            entries.push((key, self.value()));
            self.skip_whitespace();
            match self.peek() {
                b',' => self.position += 1,
                b'}' => {
                    self.position += 1;
                    break;
                }
                other => panic!("对象里出现意外字节 {other}"),
            }
        }
        Json::Obj(entries)
    }

    fn array(&mut self) -> Json {
        self.position += 1; // '['
        let mut items = Vec::new();
        self.skip_whitespace();
        if self.peek() == b']' {
            self.position += 1;
            return Json::Arr(items);
        }
        loop {
            self.skip_whitespace();
            items.push(self.value());
            self.skip_whitespace();
            match self.peek() {
                b',' => self.position += 1,
                b']' => {
                    self.position += 1;
                    break;
                }
                other => panic!("数组里出现意外字节 {other}"),
            }
        }
        Json::Arr(items)
    }

    fn string(&mut self) -> String {
        assert_eq!(self.peek(), b'"');
        self.position += 1;
        let mut out = String::new();
        loop {
            match self.peek() {
                b'"' => {
                    self.position += 1;
                    break;
                }
                b'\\' => {
                    self.position += 1;
                    match self.peek() {
                        b'n' => {
                            out.push('\n');
                            self.position += 1;
                        }
                        b't' => {
                            out.push('\t');
                            self.position += 1;
                        }
                        b'"' => {
                            out.push('"');
                            self.position += 1;
                        }
                        b'\\' => {
                            out.push('\\');
                            self.position += 1;
                        }
                        b'/' => {
                            out.push('/');
                            self.position += 1;
                        }
                        b'u' => {
                            let hex = std::str::from_utf8(&self.bytes[self.position + 1..self.position + 5])
                                .expect("转义不是 UTF-8");
                            let code = u32::from_str_radix(hex, 16).expect("转义不是十六进制");
                            out.push(char::from_u32(code).expect("转义不是合法码点"));
                            self.position += 5;
                        }
                        other => panic!("未知转义 \\{other}"),
                    }
                }
                0 => panic!("字符串未闭合"),
                _ => {
                    let rest = std::str::from_utf8(&self.bytes[self.position..]).expect("不是 UTF-8");
                    let character = rest.chars().next().expect("空字符串");
                    out.push(character);
                    self.position += character.len_utf8();
                }
            }
        }
        out
    }

    fn number(&mut self) -> Json {
        let start = self.position;
        if self.peek() == b'-' {
            self.position += 1;
        }
        while self.peek().is_ascii_digit() {
            self.position += 1;
        }
        let text = std::str::from_utf8(&self.bytes[start..self.position]).expect("数字不是 UTF-8");
        Json::Num(text.parse().expect("不是整数"))
    }
}

/// 跑一个帧并把"让出"当成错误（`Vm::run` 用）。
fn common_execute<'a>(
    instance: &'a pyawa_core::Instance,
    frame: &pyawa_core::Owned<'a, Frame>,
) -> Result<Value<'a>, ExecError> {
    match pyawa_core::execute(instance, frame)? {
        pyawa_core::ExecOutcome::Returned(value) => Ok(value),
        pyawa_core::ExecOutcome::Yielded(_) => panic!("顶层程序不该 yield"),
    }
}

/// 跑一个帧并把"让出"当成错误（测试里跑的是普通函数体）。
pub fn execute_value<'a>(
    instance: &'a pyawa_core::Instance,
    frame: &pyawa_core::Owned<'a, Frame>,
) -> Result<Value<'a>, ExecError> {
    match pyawa_core::execute(instance, frame)? {
        pyawa_core::ExecOutcome::Returned(value) => Ok(value),
        pyawa_core::ExecOutcome::Yielded(_) => panic!("测试的这段程序不该 yield"),
    }
}

/// 解析夹具文本（调用方用 `include_str!` 把夹具读进来）。
pub fn parse(text: &str) -> Json {
    Parser::new(text).parse()
}

// --------------------------------------------------------------------------- #
// 执行器测试脚手架（`tests/executor.rs` 与 `tests/containers.rs` 共用）
// --------------------------------------------------------------------------- #

use core::ptr::NonNull;

use pyawa_core::opcode;
use pyawa_core::{
    CodeObject, ExecError, Frame, Header, Instance, IntObject, TypeObject, Value,
};

pub fn op(name: &str) -> u8 {
    opcode::opcode(name).unwrap_or_else(|| panic!("opmap 缺 {name}")) as u8
}

/// `BINARY_OP` 的 oparg 从 `get_nb_ops()` 的顺序取（`BC-39`／`BC-50`：不写死编号）。
pub fn nb(name: &str) -> u8 {
    opcode::get_nb_ops()
        .iter()
        .position(|(candidate, _)| *candidate == name)
        .unwrap_or_else(|| panic!("get_nb_ops 缺 {name}")) as u8
}

/// 按 `BC-35` 发射：每条指令后面留**等宽零填充** cache 槽（宽度从指令表取）。
///
/// 手写码元最容易漏掉这些槽——本文件的第一个版本就是漏了，解码器按规矩报了
/// `MissingCacheSlots`。将来的编译器要做同一件事（`T-BC-12`）。
pub fn emit(instructions: &[(u8, u8)]) -> Vec<u8> {
    let mut bytes = Vec::new();
    for (opcode, oparg) in instructions {
        bytes.extend([*opcode, *oparg]);
        let cache = opcode::inline_cache_entries(u16::from(*opcode));
        for _ in 0..cache {
            bytes.extend([0, 0]);
        }
    }
    bytes
}

/// 带标签的汇编条目：跳转的 oparg 由 [`assemble`] 按 `BC-55` **反解**。
pub enum Item {
    Instr(u8, u8),
    Label(&'static str),
    Jump(u8, &'static str),
}

/// 汇编并返回标签的**字节**偏移（异常表要用字节）。
pub fn assemble_labeled(items: &[Item]) -> (Vec<u8>, Vec<(&'static str, usize)>) {
    let bytes = assemble(items);
    // 再走一遍算标签位置：与 `assemble` 同一套宽度规则
    let mut offsets: Vec<(&'static str, usize)> = Vec::new();
    let mut position = 0usize;
    for item in items {
        match item {
            Item::Label(name) => offsets.push((name, position * 2)),
            Item::Instr(opcode, _) | Item::Jump(opcode, _) => {
                position += 1 + opcode::inline_cache_entries(u16::from(*opcode)) as usize;
            }
        }
    }
    (bytes, offsets)
}

/// 异常表的 varint：每字节 6 位，除最后一字节外都置 `0x40`（与 `BC-54` 的读法互逆）。
pub fn varint(value: usize, out: &mut Vec<u8>) {
    let mut chunks = vec![(value & 0x3F) as u8];
    let mut rest = value >> 6;
    while rest > 0 {
        chunks.push((rest & 0x3F) as u8);
        rest >>= 6;
    }
    chunks.reverse();
    let last = chunks.len() - 1;
    for (index, chunk) in chunks.into_iter().enumerate() {
        out.push(if index == last { chunk } else { chunk | 0x40 });
    }
}

/// 把带标签的条目汇编成码元（按实测宽度补 cache 槽）。
///
/// 跳转的 oparg：目标在前取 `+|差|`、在后取 `−|差|`——与 `BC-55` 的读法互为逆运算，
/// 因此"汇编 → 解码 → `jump_target`"能回到原标签（本文件的循环用例正是这么做的）。
pub fn assemble(items: &[Item]) -> Vec<u8> {
    let mut offsets: Vec<usize> = Vec::new();
    let mut labels: Vec<(&str, usize)> = Vec::new();
    let mut position = 0usize;
    for item in items {
        match item {
            Item::Label(name) => labels.push((name, position)),
            Item::Instr(opcode, _) | Item::Jump(opcode, _) => {
                offsets.push(position);
                position += 1 + opcode::inline_cache_entries(u16::from(*opcode)) as usize;
            }
        }
    }

    let mut bytes = Vec::new();
    let mut index = 0usize;
    for item in items {
        match item {
            Item::Label(_) => continue,
            Item::Instr(opcode, oparg) => {
                bytes.extend([*opcode, *oparg]);
                pad_cache(*opcode, &mut bytes);
            }
            Item::Jump(opcode, label) => {
                let target = labels
                    .iter()
                    .find(|(name, _)| name == label)
                    .unwrap_or_else(|| panic!("没有这个标签：{label}"))
                    .1;
                let caches = opcode::inline_cache_entries(u16::from(*opcode)) as usize;
                let base = offsets[index] + 1 + caches;
                let argument = if target >= base { target - base } else { base - target };
                assert!(argument <= u8::MAX as usize, "oparg 装不进一个字节：{argument}");
                bytes.extend([*opcode, argument as u8]);
                pad_cache(*opcode, &mut bytes);
            }
        }
        index += 1;
    }
    bytes
}

fn pad_cache(opcode_number: u8, bytes: &mut Vec<u8>) {
    for _ in 0..opcode::inline_cache_entries(u16::from(opcode_number)) {
        bytes.extend([0, 0]);
    }
}

pub struct Vm {
    pub instance: Instance,
    pub code_type: NonNull<TypeObject>,
    pub frame_type: NonNull<TypeObject>,
}

impl Vm {
    pub fn new() -> Self {
        let instance = Instance::new();
        let code_type = instance.new_type(
            "CodeObject",
            core::mem::size_of::<CodeObject>(),
            CodeObject::slots(),
        );
        let frame_type = instance.new_type("Frame", core::mem::size_of::<Frame>(), Frame::slots());
        Vm {
            instance,
            code_type,
            frame_type,
        }
    }

    /// 造一个 `int` 常量对象，**把那份新引用交出去**（由常量表接管）。
    pub fn constant(&self, value: i64) -> NonNull<Header> {
        let object = self
            .instance
            .alloc(IntObject::new(self.instance.singletons().int_type(), value));
        object.into_raw().cast::<Header>()
    }

    /// 最近抛出的异常：返回（类型名，消息）。
    ///
    /// 走实例的 `pending_exception`（`BC-60` ②：异常状态按实例存）。
    pub fn pending_exception(&self) -> Option<(String, Option<String>)> {
        let raw = self.instance.pending_exception()?;
        // SAFETY: raw 由实例持有，存活。
        let ty = unsafe { raw.as_ref() }.ty();
        // SAFETY: 同上。
        let type_name = unsafe { ty.as_ref() }.name().to_owned();
        // SAFETY: 同上。
        let message = unsafe { &*raw.as_ptr().cast::<pyawa_core::ExceptionObject>() }
            .message_with(&self.instance);
        Some((type_name, message))
    }

    /// 造一个**带异常表**的 code object（处理块派发要用）。
    #[allow(clippy::too_many_arguments)]
    pub fn try_code(
        &self,
        stacksize: usize,
        nlocals: usize,
        names: Vec<String>,
        bytes: Vec<u8>,
        consts: Vec<Option<NonNull<Header>>>,
        exceptiontable: Vec<u8>,
    ) -> pyawa_core::Owned<'_, CodeObject> {
        self.instance.alloc(CodeObject::new(
            self.code_type,
            "demo",
            "demo".to_owned(),
            "<pyawa-test>".to_owned(),
            0,
            stacksize,
            nlocals,
            0,
            0,
            0,
            0,
            Vec::new(),
            names,
            Vec::new(),
            Vec::new(),
            bytes,
            exceptiontable,
            consts,
        ))
    }

    /// 常量表里的某一项（**借用**）——比较容器元素时用。
    pub fn constant_ref(
        &self,
        code: &pyawa_core::Owned<'_, CodeObject>,
        index: usize,
    ) -> Option<NonNull<Header>> {
        code.get().constant(index)
    }

    pub fn code(
        &self,
        stacksize: usize,
        nlocals: usize,
        bytes: Vec<u8>,
        consts: Vec<Option<NonNull<Header>>>,
    ) -> pyawa_core::Owned<'_, CodeObject> {
        self.function_code(stacksize, nlocals, 0, 0, 0, 0, Vec::new(), bytes, consts)
    }

    /// 造一个**带 `co_names` 与签名**的 code object（`LOAD_ATTR` 一族要用名字表）。
    #[allow(clippy::too_many_arguments)]
    pub fn code_with_names(
        &self,
        stacksize: usize,
        nlocals: usize,
        argcount: usize,
        varnames: Vec<String>,
        names: Vec<String>,
        bytes: Vec<u8>,
        consts: Vec<Option<NonNull<Header>>>,
    ) -> pyawa_core::Owned<'_, CodeObject> {
        self.instance.alloc(CodeObject::new(
            self.code_type,
            "demo",
            "demo".to_owned(),
            "<pyawa-test>".to_owned(),
            0,
            stacksize,
            nlocals,
            argcount,
            0,
            0,
            0,
            varnames,
            names,
            Vec::new(),
            Vec::new(),
            bytes,
            Vec::new(),
            consts,
        ))
    }

    /// 造一个**带签名**的 code object（参数绑定要用 `BC-4` 的那几个字段）。
    #[allow(clippy::too_many_arguments)]
    pub fn function_code(
        &self,
        stacksize: usize,
        nlocals: usize,
        argcount: usize,
        posonlyargcount: usize,
        kwonlyargcount: usize,
        flags: u32,
        varnames: Vec<String>,
        bytes: Vec<u8>,
        consts: Vec<Option<NonNull<Header>>>,
    ) -> pyawa_core::Owned<'_, CodeObject> {
        self.instance.alloc(CodeObject::new(
            self.code_type,
            "demo",
            "demo".to_owned(),
            "<pyawa-test>".to_owned(),
            0,
            stacksize,
            nlocals,
            argcount,
            posonlyargcount,
            kwonlyargcount,
            flags,
            varnames,
            Vec::new(),
            Vec::new(),
            Vec::new(),
            bytes,
            Vec::new(),
            consts,
        ))
    }

    pub fn run(&self, code: &pyawa_core::Owned<'_, CodeObject>) -> Result<Value<'_>, ExecError> {
        let frame = self.instance.alloc(Frame::for_code(self.frame_type, code));
        common_execute(&self.instance, &frame)
    }
}

/// `COMPARE_OP` 的 `<` 在 `opcode.cmp_op` 六元组里的下标（`BC-39`：从表里取）。
pub fn less_than() -> u8 {
    opcode::get_cmp_op()
        .iter()
        .position(|name| *name == "<")
        .expect("cmp_op 里应当有 <") as u8
}
