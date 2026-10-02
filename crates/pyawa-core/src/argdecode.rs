//! 逐指令 oparg 解码（`docs/SPEC-bytecode.md` §8.2：**BC-57**／**BC-58**／**BC-59**）。
//!
//! 口径以 `dis` 为 **oracle**：`argval` 与 `argrepr` 都要**逐字**一致
//! （`T-BC-19`／`T-BC-20`／`T-BC-21`）。因此本模块的输出是**字符串**，规则照 `dis`：
//!
//! - 无操作数的指令：**以 `_opcode.has_arg` 为唯一权威**（`BC-58`）——不是"编号 < HAVE_ARGUMENT"
//!   的数字比较：`WITH_EXCEPT_START` 编号 ≥ 43 却没有 oparg（实测）
//! - 有操作数但不解释：`argval = 十进制`、`argrepr = ""`
//! - 名类：`BC-57` 只有 `LOAD_GLOBAL`／`LOAD_ATTR`／`LOAD_SUPER_ATTR` 移位，其余**不移位**
//! - 常量类：`argval = argrepr = repr(常量)`
//! - 局部类：`argval = repr(名字)`、`argrepr = 名字`；打包类**高 4 位是第一个**
//! - 跳转类：`argval = 目标字节偏移`、`argrepr = "to L" + 序号`（序号按目标**升序**编）
//! - `COMPARE_OP`：`cmp = cmp_op[oparg >> 5]`；bit 4 ＝ `bool(...)` 标志
//!
//! **没覆盖的形态一律 `Err`**——`BC-59` 禁止"文档没写就跳过"，所以语料一扩，这里就会红。

use core::ptr::NonNull;

use crate::builtin_objects::{BoolObject, FloatObject, IntObject, StrObject, TupleObject};
use crate::code::CodeObject;
use crate::decode::{Decoder, Instruction};
use crate::header::Header;
use crate::instance::Instance;
use crate::opcode;

/// 解码失败：**没覆盖**的形态（`BC-59`：不许静默跳过）。
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ArgDecodeError {
    /// 这条指令的 oparg 形态还没接线。
    Unsupported { offset: usize, opname: String },
    /// 常量没法按 `dis` 的 `repr` 口径渲染（例如 code object——要 `co_filename` 等字段）。
    UnrenderableConstant { offset: usize, opname: String },
    /// 码元本身解码失败。
    Decode(crate::decode::DecodeError),
}

/// 一条指令的解码结果（与 `dis` 同口径：偏移是**字节**）。
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DecodedInstruction {
    /// 字节偏移（`dis` 的 `Instruction.offset`）。
    pub offset: usize,
    /// 指令名（从 `opmap` 取，`BC-50`）。
    pub opname: &'static str,
    /// 与 `dis` 的 `argval` 同口径的字符串。
    pub argval: String,
    /// 与 `dis` 的 `argrepr` 同口径的字符串。
    pub argrepr: String,
}

/// 按 `dis` 的口径渲染一个常量的 `repr`（只覆盖语料里出现的形态）。
fn render_constant(instance: &Instance, raw: NonNull<Header>) -> Option<String> {
    // SAFETY: raw 是存活对象。
    let ty = unsafe { raw.as_ref() }.ty();
    let singletons = instance.singletons();
    if ty == singletons.none_type() {
        return Some("None".to_owned());
    }
    if ty == singletons.bool_type() {
        // SAFETY: 类型身份已确认。
        let value = unsafe { &*raw.as_ptr().cast::<BoolObject>() }.value;
        return Some(if value { "True" } else { "False" }.to_owned());
    }
    if ty == singletons.int_type() {
        // SAFETY: 同上。
        return Some(unsafe { &*raw.as_ptr().cast::<IntObject>() }.value.to_decimal());
    }
    if Some(ty) == instance.type_named("float") {
        // SAFETY: 同上。用 `{:?}` 才能得到参照实现的 `1.0` 而不是 `1`
        return Some(format!("{:?}", unsafe { &*raw.as_ptr().cast::<FloatObject>() }.value()));
    }
    if ty == singletons.str_type() {
        // SAFETY: 同上。
        let text = unsafe { &*raw.as_ptr().cast::<StrObject>() }.value();
        let mut rendered = String::from("'");
        for character in text.chars() {
            match character {
                '\\' => rendered.push_str("\\\\"),
                '\'' => rendered.push_str("\\'"),
                '\n' => rendered.push_str("\\n"),
                '\t' => rendered.push_str("\\t"),
                other => rendered.push(other),
            }
        }
        rendered.push('\'');
        return Some(rendered);
    }
    if Some(ty) == instance.type_named("tuple") {
        // SAFETY: 同上。
        let items = unsafe { &*raw.as_ptr().cast::<TupleObject>() };
        let mut rendered = String::from("(");
        for index in 0..items.len() {
            if index > 0 {
                rendered.push_str(", ");
            }
            rendered.push_str(&render_constant(instance, items.item(index)?)?);
        }
        if items.len() == 1 {
            rendered.push(',');
        }
        rendered.push(')');
        return Some(rendered);
    }
    None
}

/// 名字表里的一项（`co_names`）。
fn name_at(code: &CodeObject, index: usize) -> Result<String, ()> {
    code.name_at(index)
        .map(|name| format!("'{name}'"))
        .ok_or(())
}

/// 局部名表里的一项（`co_varnames`）。
fn varname_at(code: &CodeObject, index: usize) -> Result<String, ()> {
    code.varname(index)
        .map(str::to_owned)
        .ok_or(())
}

/// 解码整段码元。`labels` 按目标升序自动编号（与 `dis` 一致）。
pub fn decode_all(
    instance: &Instance,
    code: &CodeObject,
) -> Result<Vec<DecodedInstruction>, ArgDecodeError> {
    let mut instructions: Vec<Instruction> = Vec::new();
    let mut decoder = Decoder::new(code.code());
    while let Some(instruction) = decoder.next_instruction().map_err(ArgDecodeError::Decode)? {
        instructions.push(instruction);
    }

    // 跳转目标的标签：按**目标升序**编号 L1…Ln（实测 `dis` 如此）
    let mut targets: Vec<usize> = instructions
        .iter()
        .filter_map(|instruction| instruction.jump_target())
        .collect();
    targets.sort_unstable();
    targets.dedup();
    let label_of = |target: usize| -> String {
        let index = targets
            .iter()
            .position(|candidate| *candidate == target)
            .expect("目标一定在表里");
        format!("L{}", index + 1)
    };

    let mut decoded = Vec::with_capacity(instructions.len());
    for instruction in instructions {
        let opname = opcode::opname(u16::from(instruction.opcode)).unwrap_or("<unknown>");
        let oparg = instruction.oparg as usize;
        // `dis` 把 `EXTENDED_ARG` 列成**单独一条**（解码器为执行把它们折叠了）——
        // 这里按 `dis` 的列法补出前缀，真实指令的偏移也要往后挪。
        let caches = opcode::inline_cache_entries(u16::from(instruction.opcode)) as usize;
        let prefixes = instruction
            .size
            .saturating_sub(1 + caches);
        for step in 0..prefixes {
            let unit = instruction.offset + step;
            let raw = code.code().get(unit * 2 + 1).copied().unwrap_or(0);
            decoded.push(DecodedInstruction {
                offset: unit * 2,
                opname: "EXTENDED_ARG",
                argval: raw.to_string(),
                argrepr: String::new(),
            });
        }
        let bytes = (instruction.offset + prefixes) * 2;

        let unsupported = || ArgDecodeError::Unsupported {
            offset: bytes,
            opname: opname.to_owned(),
        };

        let (argval, argrepr) = match opname {
            "LOAD_CONST" => {
                let constant = code.constant(oparg).ok_or_else(unsupported)?;
                let rendered =
                    render_constant(instance, constant).ok_or(ArgDecodeError::UnrenderableConstant {
                        offset: bytes,
                        opname: opname.to_owned(),
                    })?;
                (rendered.clone(), rendered)
            }
            // BC-57：只有这三条移位
            "LOAD_GLOBAL" => {
                let name = name_at(code, oparg >> 1).map_err(|_| unsupported())?;
                let suffix = if oparg & 1 != 0 { " + NULL" } else { "" };
                (name.clone(), format!("{}{suffix}", &name[1..name.len() - 1]))
            }
            "LOAD_ATTR" => {
                let name = name_at(code, oparg >> 1).map_err(|_| unsupported())?;
                let suffix = if oparg & 1 != 0 { " + NULL|self" } else { "" };
                (name.clone(), format!("{}{suffix}", &name[1..name.len() - 1]))
            }
            "LOAD_SUPER_ATTR" => {
                let name = name_at(code, oparg >> 2).map_err(|_| unsupported())?;
                let mut suffix = String::new();
                if oparg & 1 != 0 {
                    suffix.push_str(" + NULL|self");
                }
                if oparg & 2 != 0 {
                    suffix.push_str(" + super(C, self)");
                }
                (name.clone(), format!("{}{suffix}", &name[1..name.len() - 1]))
            }
            // BC-57：其余名类**不移位**
            "STORE_ATTR" | "DELETE_ATTR" | "LOAD_NAME" | "STORE_NAME" | "DELETE_NAME"
            | "STORE_GLOBAL" | "DELETE_GLOBAL" | "IMPORT_NAME" | "IMPORT_FROM" => {
                let name = name_at(code, oparg).map_err(|_| unsupported())?;
                (name.clone(), name[1..name.len() - 1].to_owned())
            }
            // `CALL_INTRINSIC_1`／`_2`：oparg 是**各自**那张 intrinsic 名字表的下标
            // （`BC-38`：表由生成器导出；两元的名字表是分开的，别共用一张）
            "CALL_INTRINSIC_1" => {
                let name = crate::opcode::get_intrinsic1_descs()
                    .get(oparg as usize)
                    .ok_or_else(unsupported)?;
                (format!("{}", oparg), (*name).to_owned())
            }
            "CALL_INTRINSIC_2" => {
                let name = crate::opcode::get_intrinsic2_descs()
                    .get(oparg as usize)
                    .ok_or_else(unsupported)?;
                (format!("{}", oparg), (*name).to_owned())
            }
            // `LOAD_SPECIAL`：oparg 是 `_opcode.get_special_method_names()` 的下标
            // （`BC-38`：表由生成器导出，**禁止**手写；`dis` 的 argrepr 就是那个名字）
            "LOAD_SPECIAL" => {
                let name = crate::opcode::get_special_method_names()
                    .get(oparg as usize)
                    .ok_or_else(unsupported)?;
                (format!("{}", oparg), (*name).to_owned())
            }
            // 局部类（含 3.14 的借用形态）
            "LOAD_FAST" | "LOAD_FAST_CHECK" | "LOAD_FAST_BORROW" | "STORE_FAST"
            | "STORE_FAST_MAYBE_NULL" | "DELETE_FAST" => {
                let name = varname_at(code, oparg).map_err(|_| unsupported())?;
                (format!("'{name}'"), name)
            }
            // BC-58：两个槽位打包，高 4 位是第一个
            "LOAD_FAST_LOAD_FAST" | "LOAD_FAST_BORROW_LOAD_FAST_BORROW"
            | "STORE_FAST_LOAD_FAST" | "STORE_FAST_STORE_FAST" => {
                let first = varname_at(code, oparg >> 4).map_err(|_| unsupported())?;
                let second = varname_at(code, oparg & 15).map_err(|_| unsupported())?;
                (
                    format!("('{first}', '{second}')"),
                    format!("{first}, {second}"),
                )
            }
            // 跳转（BC-55 的算法给出的目标）
            _ if opcode::has_jump(u16::from(instruction.opcode)) => {
                let target = instruction.jump_target().ok_or_else(unsupported)?;
                // `dis` 对 `END_ASYNC_FOR` 用 **"from"** 而不是 "to"（实测 `dis.py` 里的
                // `preposition = "from" if deop == END_ASYNC_FOR else "to"`；注释还说它
                // "not really a jump, but it has a target"）——`async for` 的对拍把它抓了出来。
                let preposition = if opcode::opname(u16::from(instruction.opcode))
                    == Some("END_ASYNC_FOR")
                {
                    "from"
                } else {
                    "to"
                };
                (
                    format!("{}", target * 2),
                    format!("{preposition} {}", label_of(target)),
                )
            }
            "COMPARE_OP" => {
                let comparator = opcode::get_cmp_op().get(oparg >> 5).ok_or_else(unsupported)?;
                let repr = if oparg & 16 != 0 {
                    format!("bool({comparator})")
                } else {
                    (*comparator).to_owned()
                };
                (format!("'{comparator}'"), repr)
            }
            "IS_OP" => (
                oparg.to_string(),
                if oparg == 0 { "is" } else { "is not" }.to_owned(),
            ),
            "CONTAINS_OP" => (
                oparg.to_string(),
                if oparg == 0 { "in" } else { "not in" }.to_owned(),
            ),
            "BINARY_OP" => {
                let symbol = opcode::get_nb_ops()
                    .get(oparg)
                    .map(|(_, symbol)| *symbol)
                    .ok_or_else(unsupported)?;
                (oparg.to_string(), symbol.to_owned())
            }
            // 其余：编号 < HAVE_ARGUMENT 的没有操作数，有操作数但不解释的按十进制
            _ => {
                if !opcode::has_arg(u16::from(instruction.opcode)) {
                    ("None".to_owned(), String::new())
                } else {
                    (instruction.oparg.to_string(), String::new())
                }
            }
        };

        decoded.push(DecodedInstruction {
            offset: bytes,
            opname,
            argval,
            argrepr,
        });
    }
    Ok(decoded)
}
