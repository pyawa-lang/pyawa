//! `marshal` 模块（**`CM-27`**：自有二进制格式 ＋ 自己的版本号，`loads(dumps(x))` 往返一致）。
//!
//! 义务边界（照 `CM-27` 的原文）：
//!
//! - **必须存在且自洽**：`dump`／`dumps`／`load`／`loads`／`version` 齐备，往返一致
//! - **不追**与参照实现的**字节兼容**（参照的 marshal 格式按版本走、属实现定义行为）
//!   ⇒ 字节差异按 `MS-19` 登记（本模块的格式是**我们自己的**）
//! - `.pyac` 走**自己的**容器（`IM-18`），**不依赖** marshal
//!
//! **本格式**（版本 [`FORMAT_VERSION`]，每项：1 字节 tag ＋ 载荷；整数与长度一律小端）：
//!
//! ```text
//! 0x00 None       0x01 True      0x02 False     0x03 i64（8 字节）
//! 0x04 大整数（u32 长度 ＋ 十进制 ASCII）
//! 0x05 float（8 字节 IEEE-754 位模式）   0x06 str（u32 长度 ＋ UTF-8）
//! 0x07 bytes（u32 长度 ＋ 原始字节）
//! 0x08 tuple／0x09 list／0x0b set（u32 个数 ＋ 逐项）
//! 0x0a dict（u32 对数 ＋ 逐对「键、值」）
//! 0x0c 引用（u32 下标）：指向前一个**可变容器**（list／dict）——循环引用靠它
//! ```
//!
//! **与参照的已知差异**（登记在 `PLAN-milestones.md` 的 `P1-12` 行；`CM-27` 允许）：
//!
//! - 版本号是我们自己的（参照 3.14 实测是 `5`）
//! - 循环引用只在 **list／dict** 上支持；穿过 **tuple／set** 的环报错（我们的消息）
//! - 嵌套深度上限 [`MAX_DEPTH`]（我们的消息）：解码深嵌套输入不能把宿主栈打爆
//! - `dump`／`load` 要**文件对象**（fs 域／M3+ 还没落地）⇒ 如实报未实现

use core::cell::Cell;
use core::ptr::NonNull;

use pyawa_core::{BuiltinFunctionObject, ExecError, Header, Instance};

/// 模块名。
pub const NAME: &str = "marshal";

/// 模块的 `__doc__`。
pub const DOC: &str = "This module contains functions that can read and write Python values in a binary format.";

/// **Pyawa 自己的 marshal 格式版本**（`CM-27`：自有格式、自己的版本号）。
pub const FORMAT_VERSION: u8 = 1;

/// 嵌套深度上限（**我们自己的**实现上限；超限报错而不是把宿主栈打爆）。
pub const MAX_DEPTH: usize = 200;

const TAG_NONE: u8 = 0x00;
const TAG_TRUE: u8 = 0x01;
const TAG_FALSE: u8 = 0x02;
const TAG_INT: u8 = 0x03;
const TAG_BIG_INT: u8 = 0x04;
const TAG_FLOAT: u8 = 0x05;
const TAG_STR: u8 = 0x06;
const TAG_BYTES: u8 = 0x07;
const TAG_TUPLE: u8 = 0x08;
const TAG_LIST: u8 = 0x09;
const TAG_DICT: u8 = 0x0a;
const TAG_SET: u8 = 0x0b;
const TAG_REF: u8 = 0x0c;

/// 建 `marshal` 模块的命名空间（**新引用** 的 `dict`）。
pub fn build(instance: &Instance) -> NonNull<Header> {
    let namespace = instance.new_dict();
    let version = instance.new_int(i64::from(FORMAT_VERSION));
    instance.dict_set(namespace, "version", version);
    for (name, handler) in [
        ("dumps", dumps_native as pyawa_core::NativeFn),
        ("loads", loads_native as pyawa_core::NativeFn),
        ("dump", dump_native as pyawa_core::NativeFn),
        ("load", load_native as pyawa_core::NativeFn),
    ] {
        let function = make_native(instance, name, handler);
        instance.dict_set(namespace, name, function);
    }
    let module_name = instance.new_str(NAME);
    instance.dict_set(namespace, "__name__", module_name);
    let doc = instance.new_str(DOC);
    instance.dict_set(namespace, "__doc__", doc);
    namespace
}

fn make_native(instance: &Instance, name: &str, handler: pyawa_core::NativeFn) -> NonNull<Header> {
    let ty = instance
        .type_named("builtin_function_or_method")
        .expect("builtin_function_or_method 在引导期已登记");
    let object = instance.alloc(BuiltinFunctionObject::new(
        ty,
        Box::leak(name.to_owned().into_boxed_str()),
        Cell::new(handler),
    ));
    object.into_raw().cast::<Header>()
}

// --------------------------------------------------------------------------- #
// 编码
// --------------------------------------------------------------------------- #

/// 编码期状态：**当前路径**上的可变容器（地址 → 引用下标）＋ 下一个下标。
struct Encoder {
    containers: Vec<(usize, usize)>,
    next_index: usize,
}

impl Encoder {
    fn new() -> Self {
        Self { containers: Vec::new(), next_index: 0 }
    }

    fn index_of(&self, address: usize) -> Option<usize> {
        self.containers
            .iter()
            .find(|(candidate, _)| *candidate == address)
            .map(|(_, index)| *index)
    }
}

fn push_u32(out: &mut Vec<u8>, value: usize) {
    out.extend_from_slice(&(value as u32).to_le_bytes());
}

/// `marshal.dumps(x)` 的主体：把对象写成我们自己的格式。
fn encode(
    instance: &Instance,
    object: NonNull<Header>,
    out: &mut Vec<u8>,
    state: &mut Encoder,
    depth: usize,
) -> Result<(), ExecError> {
    if depth > MAX_DEPTH {
        return Err(instance.raise_builtin_error("ValueError", "marshal data too deeply nested"));
    }
    let ty = instance.type_of(object);
    if Some(ty) == instance.type_named("NoneType") {
        out.push(TAG_NONE);
        return Ok(());
    }
    if let Some(value) = instance.bool_value(object) {
        out.push(if value { TAG_TRUE } else { TAG_FALSE });
        return Ok(());
    }
    if let Some(value) = instance.int_of(object) {
        let wide = value.to_bigint();
        if let Some(small) = wide.to_i64() {
            out.push(TAG_INT);
            out.extend_from_slice(&small.to_le_bytes());
        } else {
            out.push(TAG_BIG_INT);
            let text = wide.to_decimal();
            push_u32(out, text.len());
            out.extend_from_slice(text.as_bytes());
        }
        return Ok(());
    }
    if let Some(number) = instance.float_value(object) {
        out.push(TAG_FLOAT);
        out.extend_from_slice(&number.to_bits().to_le_bytes());
        return Ok(());
    }
    if let Some(text) = instance.text_value(object) {
        out.push(TAG_STR);
        push_u32(out, text.len());
        out.extend_from_slice(text.as_bytes());
        return Ok(());
    }
    if let Some(bytes) = instance.bytes_value(object) {
        out.push(TAG_BYTES);
        push_u32(out, bytes.len());
        out.extend_from_slice(bytes);
        return Ok(());
    }

    let address = object.as_ptr() as usize;
    let mutable = Some(ty) == instance.type_named("list") || Some(ty) == instance.type_named("dict");
    if mutable {
        if let Some(index) = state.index_of(address) {
            // 已经在**当前路径**上 ⇒ 一个循环引用
            out.push(TAG_REF);
            push_u32(out, index);
            return Ok(());
        }
    } else if state.index_of(address).is_some() {
        // 穿过 tuple／set 的环：我们的格式表示不了 ⇒ 如实报错（登记为差异）
        return Err(instance.raise_builtin_error(
            "ValueError",
            "circular reference through an immutable container is not supported by pyawa's marshal format",
        ));
    }

    if let Some(items) = instance.tuple_items(object) {
        out.push(TAG_TUPLE);
        push_u32(out, items.len());
        for item in items {
            encode(instance, item, out, state, depth + 1)?;
        }
        return Ok(());
    }
    if let Some(items) = instance.list_items(object) {
        let index = state.next_index;
        state.next_index += 1;
        state.containers.push((address, index));
        out.push(TAG_LIST);
        push_u32(out, items.len());
        for item in items {
            encode(instance, item, out, state, depth + 1)?;
        }
        state.containers.pop();
        return Ok(());
    }
    if let Some(entries) = instance.dict_entries(object) {
        let index = state.next_index;
        state.next_index += 1;
        state.containers.push((address, index));
        out.push(TAG_DICT);
        push_u32(out, entries.len());
        for (key, value) in entries {
            encode(instance, key, out, state, depth + 1)?;
            encode(instance, value, out, state, depth + 1)?;
        }
        state.containers.pop();
        return Ok(());
    }
    if let Some(items) = instance.set_items(object) {
        out.push(TAG_SET);
        push_u32(out, items.len());
        for item in items {
            encode(instance, item, out, state, depth + 1)?;
        }
        return Ok(());
    }
    Err(instance.raise_builtin_error(
        "ValueError",
        &format!(
            "cannot marshal '{}' objects",
            instance.type_name(instance.type_of(object))
        ),
    ))
}

// --------------------------------------------------------------------------- #
// 解码
// --------------------------------------------------------------------------- #

/// 解码期状态：已建好的**可变容器**（按创建顺序，供 `TAG_REF` 指回来）。
struct Decoder<'a> {
    bytes: &'a [u8],
    at: usize,
    built: Vec<NonNull<Header>>,
}

impl<'a> Decoder<'a> {
    fn new(bytes: &'a [u8]) -> Self {
        Self { bytes, at: 0, built: Vec::new() }
    }

    fn short(&self, instance: &Instance) -> ExecError {
        instance.raise_builtin_error("EOFError", "marshal data too short")
    }

    fn take(&mut self, count: usize, instance: &Instance) -> Result<&'a [u8], ExecError> {
        if self.at + count > self.bytes.len() {
            return Err(self.short(instance));
        }
        let slice = &self.bytes[self.at..self.at + count];
        self.at += count;
        Ok(slice)
    }

    fn u8(&mut self, instance: &Instance) -> Result<u8, ExecError> {
        Ok(self.take(1, instance)?[0])
    }

    fn u32(&mut self, instance: &Instance) -> Result<usize, ExecError> {
        let bytes = self.take(4, instance)?;
        Ok(u32::from_le_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]) as usize)
    }

    fn i64(&mut self, instance: &Instance) -> Result<i64, ExecError> {
        let bytes = self.take(8, instance)?;
        let mut array = [0u8; 8];
        array.copy_from_slice(bytes);
        Ok(i64::from_le_bytes(array))
    }

    fn f64(&mut self, instance: &Instance) -> Result<f64, ExecError> {
        let bytes = self.take(8, instance)?;
        let mut array = [0u8; 8];
        array.copy_from_slice(bytes);
        Ok(f64::from_bits(u64::from_le_bytes(array)))
    }

    /// 长度字段：先按字节数读，再校验"读得出来"（避免拿一个天文数字去 `Vec::with_capacity`）。
    fn length(&mut self, instance: &Instance, remaining_hint: usize) -> Result<usize, ExecError> {
        let length = self.u32(instance)?;
        if length > self.bytes.len().max(remaining_hint) {
            return Err(self.short(instance));
        }
        Ok(length)
    }

    fn decode(&mut self, instance: &Instance, depth: usize) -> Result<NonNull<Header>, ExecError> {
        if depth > MAX_DEPTH {
            return Err(instance.raise_builtin_error(
                "ValueError",
                "marshal data too deeply nested",
            ));
        }
        let tag = self.u8(instance)?;
        match tag {
            TAG_NONE => Ok(instance.retain(instance.singletons().none())),
            TAG_TRUE => Ok(instance.retain(instance.singletons().boolean(true))),
            TAG_FALSE => Ok(instance.retain(instance.singletons().boolean(false))),
            TAG_INT => {
                let value = self.i64(instance)?;
                Ok(instance.new_int(value))
            }
            TAG_BIG_INT => {
                let length = self.length(instance, 0)?;
                let text = String::from_utf8(self.take(length, instance)?.to_vec())
                    .map_err(|_| instance.raise_builtin_error("ValueError", "bad marshal data"))?;
                let value = pyawa_core::bigint::BigInt::from_decimal(&text).ok_or_else(|| {
                    instance.raise_builtin_error("ValueError", "bad marshal data")
                })?;
                Ok(instance.new_int_value(pyawa_core::bigint::IntValue::from_big(value)))
            }
            TAG_FLOAT => {
                let number = self.f64(instance)?;
                Ok(instance.new_float(number))
            }
            TAG_STR => {
                let length = self.length(instance, 0)?;
                let text = String::from_utf8(self.take(length, instance)?.to_vec())
                    .map_err(|_| instance.raise_builtin_error("ValueError", "bad marshal data"))?;
                Ok(instance.new_str(&text))
            }
            TAG_BYTES => {
                let length = self.length(instance, 0)?;
                let value = self.take(length, instance)?.to_vec();
                Ok(instance.new_bytes(&value))
            }
            TAG_TUPLE => {
                let count = self.length(instance, 0)?;
                let mut items = Vec::with_capacity(count.min(1024));
                for _ in 0..count {
                    items.push(self.decode(instance, depth + 1)?);
                }
                Ok(instance.new_tuple(items))
            }
            TAG_LIST => {
                let count = self.length(instance, 0)?;
                let list = instance.new_list(Vec::new());
                // **先登记再填**：自引用要能指回这个正在填的列表
                self.built.push(list);
                for _ in 0..count {
                    let item = self.decode(instance, depth + 1)?;
                    instance.list_append(list, item);
                }
                Ok(list)
            }
            TAG_DICT => {
                let count = self.length(instance, 0)?;
                let dict = instance.new_dict();
                self.built.push(dict);
                for _ in 0..count {
                    let key = self.decode(instance, depth + 1)?;
                    let value = self.decode(instance, depth + 1)?;
                    instance.dict_insert_raw(dict, key, value);
                }
                Ok(dict)
            }
            TAG_SET => {
                let count = self.length(instance, 0)?;
                let set = instance.new_set(Vec::new());
                for _ in 0..count {
                    let item = self.decode(instance, depth + 1)?;
                    instance.set_insert_raw(set, item);
                }
                Ok(set)
            }
            TAG_REF => {
                let index = self.u32(instance)?;
                match self.built.get(index) {
                    Some(object) => Ok(instance.retain(*object)),
                    None => Err(instance.raise_builtin_error(
                        "ValueError",
                        "bad marshal data (bad reference)",
                    )),
                }
            }
            _ => Err(instance.raise_builtin_error(
                "ValueError",
                "bad marshal data (unknown type code)",
            )),
        }
    }
}

// --------------------------------------------------------------------------- #
// 三个入口
// --------------------------------------------------------------------------- #

/// `marshal.dumps(value)`：编成 `bytes`（**新引用**）。
fn dumps_native(
    instance: &Instance,
    _bound: Option<NonNull<Header>>,
    args: &[NonNull<Header>],
    _kwargs: &[(NonNull<Header>, NonNull<Header>)],
) -> Result<NonNull<Header>, ExecError> {
    let [value] = args else {
        return Err(instance.raise_builtin_error(
            "TypeError",
            "dumps() takes exactly one argument",
        ));
    };
    let mut out = Vec::new();
    out.push(FORMAT_VERSION);
    let mut state = Encoder::new();
    encode(instance, *value, &mut out, &mut state, 0)?;
    Ok(instance.new_bytes(&out))
}

/// `marshal.loads(bytes)`：解回来（**新引用**）。
fn loads_native(
    instance: &Instance,
    _bound: Option<NonNull<Header>>,
    args: &[NonNull<Header>],
    _kwargs: &[(NonNull<Header>, NonNull<Header>)],
) -> Result<NonNull<Header>, ExecError> {
    let [data] = args else {
        return Err(instance.raise_builtin_error(
            "TypeError",
            "loads() takes exactly one argument",
        ));
    };
    let Some(bytes) = instance.bytes_value(*data) else {
        let name = instance.type_name(instance.type_of(*data));
        return Err(instance.raise_builtin_error(
            "TypeError",
            &format!("a bytes-like object is required, not '{name}'"),
        ));
    };
    let bytes = bytes.to_vec();
    let mut decoder = Decoder::new(&bytes);
    let version = decoder.u8(instance).map_err(|_| instance.raise_builtin_error(
        "EOFError",
        "EOF read where object expected",
    ))?;
    if version != FORMAT_VERSION {
        return Err(instance.raise_builtin_error(
            "ValueError",
            &format!("bad marshal data (unknown format version {version})"),
        ));
    }
    if decoder.at >= bytes.len() {
        return Err(instance.raise_builtin_error("EOFError", "EOF read where object expected"));
    }
    let value = decoder.decode(instance, 0)?;
    if decoder.at != bytes.len() {
        // 尾部有多余字节：如实报错（不做"取前一段"这种猜测）
        return Err(instance.raise_builtin_error("ValueError", "bad marshal data (trailing bytes)"));
    }
    Ok(value)
}

/// `marshal.dump(value, file)`：要**文件对象**（fs 域／M3+），如实报未实现。
fn dump_native(
    _instance: &Instance,
    _bound: Option<NonNull<Header>>,
    _args: &[NonNull<Header>],
    _kwargs: &[(NonNull<Header>, NonNull<Header>)],
) -> Result<NonNull<Header>, ExecError> {
    Err(ExecError::Unsupported {
        opcode: 0,
        what: "marshal.dump 要文件对象（fs 域／M3+ 还没落地）；先用 dumps／loads",
    })
}

/// `marshal.load(file)`：同上。
fn load_native(
    instance: &Instance,
    _bound: Option<NonNull<Header>>,
    _args: &[NonNull<Header>],
    _kwargs: &[(NonNull<Header>, NonNull<Header>)],
) -> Result<NonNull<Header>, ExecError> {
    let _ = instance;
    Err(ExecError::Unsupported {
        opcode: 0,
        what: "marshal.load 要文件对象（fs 域／M3+ 还没落地）；先用 dumps／loads",
    })
}
