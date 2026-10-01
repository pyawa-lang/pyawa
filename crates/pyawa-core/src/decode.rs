//! 码元解码（`docs/SPEC-bytecode.md` §8.2／§8.3：**BC-32**…**BC-36**）。
//!
//! - **BC-33**：指令流是**码元序列**，每码元 2 字节（`opcode: u8` ＋ `oparg: u8`）；
//!   无参指令的 oparg **必须**为 0
//! - **BC-34**：`EXTENDED_ARG` 展开按**大端**拼接——每个前缀贡献 8 位高位
//! - **BC-35**：带 cache 的指令，其后**必须**留等宽零填充码元；解码时跳过
//! - **BC-36**：cache 槽**必须**零填充（不用来放自己的优化）——校验时逐槽检查
//! - **BC-32**：**禁止发射** ≥ [`crate::opcode_metadata::MIN_INSTRUMENTED_OPCODE`] 的 instrumented 一族
//! - **BC-54**：`co_exceptiontable` 是 base-64 varint（每条记录 **4 个** varint），解析见
//!   [`parse_exception_table`]
//!
//! 分工：`Decoder::next_instruction` 是执行器的热路径（只做折叠与跳过）；
//! [`validate`] 是发射方（编译器／`.pyac` 载入）的体检，把上面几条一次性查全。

use crate::opcode::{self, inline_cache_entries};
use crate::opcode_metadata::MIN_INSTRUMENTED_OPCODE;

/// **BC-55**：后向跳转**只按名字**判定（`dis._is_backward_jump` 的集合）。
const BACKWARD_JUMPS: [&str; 3] = ["JUMP_BACKWARD", "JUMP_BACKWARD_NO_INTERRUPT", "END_ASYNC_FOR"];

/// 一条**已折叠**的指令：`EXTENDED_ARG` 前缀与 cache 槽都算进它的跨度。
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Instruction {
    /// 起始偏移（**码元**单位，指向第一个 `EXTENDED_ARG` 或指令本身）。
    pub offset: usize,
    /// 非 `EXTENDED_ARG` 的那个码元的操作码。
    pub opcode: u8,
    /// 按大端拼好的 oparg。
    pub oparg: u32,
    /// 本指令占用的码元数（含 `EXTENDED_ARG` 前缀与 cache 槽）。
    pub size: usize,
}

impl Instruction {
    /// **BC-55**：这条指令的跳转目标（**码元**单位）；不是跳转则 `None`。
    ///
    /// `目标码元 = offset + 1 + signed_arg + caches`——前向取 `+arg`、后向取 `−arg`，
    /// 且**必须**计入它**自己的** cache 槽。本实现用 `offset + size`（`size` ＝ 1 ＋ 前缀 ＋
    /// cache）等价地表示同一件事：跳转指令不带 `EXTENDED_ARG` 时两者逐字相同，
    /// 带上时也算"整条指令之后"。
    ///
    /// **禁止**按"下一条指令之后"或"不含自身 cache"的方式解释 oparg。
    pub fn jump_target(&self) -> Option<usize> {
        if !opcode::has_jump(u16::from(self.opcode)) {
            return None;
        }
        let name = opcode::opname(u16::from(self.opcode))?;
        let base = self.offset + self.size;
        let argument = self.oparg as usize;
        if BACKWARD_JUMPS.contains(&name) {
            base.checked_sub(argument)
        } else {
            Some(base + argument)
        }
    }
}

/// 解码／校验的失败形态。
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DecodeError {
    /// 剩下的字节不够一个码元（`BC-33`：每码元 2 字节）。
    TruncatedCodeUnit { offset: usize },
    /// 编号不在指令表里（含空隙编号）。
    UnknownOpcode { offset: usize, opcode: u8 },
    /// `BC-32`：instrumented 一族禁止发射。
    InstrumentedOpcode { offset: usize, opcode: u8 },
    /// `BC-33`：无参指令的 oparg 必须为 0（且不得带 `EXTENDED_ARG` 前缀）。
    UnexpectedArgument { offset: usize, opcode: u8, oparg: u32 },
    /// `BC-35`：带 cache 的指令后面没留够等宽码元。
    MissingCacheSlots { offset: usize, opcode: u8, expected: u32, found: usize },
    /// `BC-36`：cache 槽不是零填充。
    NonZeroCacheSlot { offset: usize },
    /// `BC-34`：`EXTENDED_ARG` 后面没有跟随真正的指令。
    DanglingExtendedArg { offset: usize },
    /// `EXTENDED_ARG` 拼出来的 oparg 超出 `u32`。
    OpargOverflow { offset: usize },
    /// `BC-54`：异常表在记录中间断了。
    TruncatedExceptionTable { offset: usize },
}

/// **`BC-54`**：异常表的一条记录。
///
/// 三个偏移都是**字节**偏移，与 `dis._parse_exception_table` 的 `start`／`end`／`target` 一致
/// ——**表里存的是码元数**（`BC-33`：每码元 2 字节），解析时 ×2。`end` 由记录里的**长度**
/// 加出来。`depth` 是进入处理块时要弹到的栈深，`lasti` 表示"要把最后一条指令压栈"。
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ExceptionEntry {
    /// 保护区间起点（字节）。
    pub start: usize,
    /// 保护区间终点（字节，＝ `start` ＋ 记录里的长度）。
    pub end: usize,
    /// 处理块入口（字节）。
    pub target: usize,
    /// 进入处理块时要弹到的值栈深度。
    pub depth: usize,
    /// 是否把"最后一条指令"压栈（`dis` 的 `lasti`）。
    pub lasti: bool,
}

/// 读一个 `BC-54` 的 varint：每字节贡献 **6 位**，`0x40` 是"还有后续"标志。
fn read_varint(table: &[u8], cursor: &mut usize) -> Result<usize, DecodeError> {
    let mut byte = *table
        .get(*cursor)
        .ok_or(DecodeError::TruncatedExceptionTable { offset: *cursor })?;
    *cursor += 1;
    let mut value = usize::from(byte & 0x3F);
    while byte & 0x40 != 0 {
        byte = *table
            .get(*cursor)
            .ok_or(DecodeError::TruncatedExceptionTable { offset: *cursor })?;
        *cursor += 1;
        value = (value << 6) | usize::from(byte & 0x3F);
    }
    Ok(value)
}

/// **`BC-54`**：解析 `co_exceptiontable`（每条记录 4 个 varint：起点、长度、目标、`depth<<1|lasti`）。
///
/// 与 `dis._parse_exception_table` 的读法一致（码元数 **×2** 换成字节）；测试用参照实现产出的
/// 表逐条对拍。**与 `dis` 的一处差别**：`dis` 碰到半截记录会停手并丢掉它，本层**报错**——
/// 发射方的 bug 不该被悄悄吞掉。
pub fn parse_exception_table(table: &[u8]) -> Result<Vec<ExceptionEntry>, DecodeError> {
    let mut entries = Vec::new();
    let mut cursor = 0usize;
    while cursor < table.len() {
        let start = read_varint(table, &mut cursor)?;
        let length = read_varint(table, &mut cursor)?;
        let target = read_varint(table, &mut cursor)?;
        let depth_and_lasti = read_varint(table, &mut cursor)?;
        // 表里存的是**码元**数，换算成字节（与 dis 一致）
        let (start, length, target) = (start * 2, length * 2, target * 2);
        entries.push(ExceptionEntry {
            start,
            end: start + length,
            target,
            depth: depth_and_lasti >> 1,
            lasti: depth_and_lasti & 1 == 1,
        });
    }
    Ok(entries)
}

/// 码元序列上的游标。构造后反复调用 [`Decoder::next_instruction`] 直到 `Ok(None)`。
pub struct Decoder<'a> {
    code: &'a [u8],
    position: usize,
}

impl<'a> Decoder<'a> {
    /// `code` 是 `co_code` 的字节串（`BC-33`：每码元 2 字节，含 cache 槽）。
    pub fn new(code: &'a [u8]) -> Self {
        Self { code, position: 0 }
    }

    /// 当前游标（**码元**单位）。
    pub fn position(&self) -> usize {
        self.position
    }

    /// 跳到某个**码元**偏移（`BC-55` 算出来的目标）。
    ///
    /// 调用方保证目标落在指令边界上——发射方（编译器）的责任，不是解码器的。
    pub fn set_position(&mut self, offset: usize) {
        self.position = offset;
    }

    fn read_unit(&self, offset: usize) -> Result<(u8, u8), DecodeError> {
        match (self.code.get(offset * 2), self.code.get(offset * 2 + 1)) {
            (Some(opcode), Some(oparg)) => Ok((*opcode, *oparg)),
            _ => Err(DecodeError::TruncatedCodeUnit { offset }),
        }
    }

    /// 取下一条指令；序列耗尽返回 `Ok(None)`。
    pub fn next_instruction(&mut self) -> Result<Option<Instruction>, DecodeError> {
        if self.position * 2 >= self.code.len() {
            return Ok(None);
        }

        let start = self.position;
        let extended_arg = extended_arg_opcode();
        let mut oparg: u32 = 0;
        let mut saw_extended = false;

        loop {
            let (opcode, raw_oparg) = match self.read_unit(self.position) {
                Ok(unit) => unit,
                Err(DecodeError::TruncatedCodeUnit { .. }) if saw_extended => {
                    return Err(DecodeError::DanglingExtendedArg { offset: start });
                }
                Err(error) => return Err(error),
            };
            self.position += 1;

            oparg = oparg
                .checked_mul(256)
                .and_then(|value| value.checked_add(u32::from(raw_oparg)))
                .ok_or(DecodeError::OpargOverflow { offset: start })?;

            if opcode == extended_arg {
                saw_extended = true;
                continue;
            }

            // BC-35：带 cache 的指令，其后留等宽零填充槽（这里只跳过，零填充由 validate 查）
            let cache_units = inline_cache_entries(u16::from(opcode));
            let size = self.position - start + cache_units as usize;
            if (start + size) * 2 > self.code.len() {
                return Err(DecodeError::MissingCacheSlots {
                    offset: start,
                    opcode,
                    expected: cache_units,
                    found: self.code.len() / 2 - self.position,
                });
            }
            self.position += cache_units as usize;

            return Ok(Some(Instruction {
                offset: start,
                opcode,
                oparg,
                size,
            }));
        }
    }
}

/// 编号不在指令表里（含空隙编号）。
fn is_unknown(opcode: u8) -> bool {
    opcode::opname(u16::from(opcode)).is_none()
}

/// `EXTENDED_ARG` 的编号（`BC-30`：从 `opmap` 取，不写死）。
fn extended_arg_opcode() -> u8 {
    opcode::opcode("EXTENDED_ARG").expect("BC-1 要求 opmap 含 EXTENDED_ARG") as u8
}

/// 发射方体检：把 `BC-32`／`BC-33`／`BC-34`／`BC-35`／`BC-36` 一次查全。
pub fn validate(code: &[u8]) -> Result<(), DecodeError> {
    if code.len() % 2 != 0 {
        return Err(DecodeError::TruncatedCodeUnit { offset: code.len() / 2 });
    }

    let mut decoder = Decoder::new(code);
    while let Some(instruction) = decoder.next_instruction()? {
        if is_unknown(instruction.opcode) {
            return Err(DecodeError::UnknownOpcode {
                offset: instruction.offset,
                opcode: instruction.opcode,
            });
        }
        if u16::from(instruction.opcode) >= MIN_INSTRUMENTED_OPCODE {
            return Err(DecodeError::InstrumentedOpcode {
                offset: instruction.offset,
                opcode: instruction.opcode,
            });
        }

        let cache_units = inline_cache_entries(u16::from(instruction.opcode)) as usize;

        // BC-33：无参指令不得带 oparg，也不得带 EXTENDED_ARG 前缀
        if !opcode::has_arg(u16::from(instruction.opcode))
            && (instruction.oparg != 0 || instruction.size != 1 + cache_units)
        {
            return Err(DecodeError::UnexpectedArgument {
                offset: instruction.offset,
                opcode: instruction.opcode,
                oparg: instruction.oparg,
            });
        }

        // BC-36：cache 槽必须是零填充
        for slot in 0..cache_units {
            let unit = instruction.offset + instruction.size - cache_units + slot;
            if code[unit * 2] != 0 || code[unit * 2 + 1] != 0 {
                return Err(DecodeError::NonZeroCacheSlot { offset: unit });
            }
        }
    }
    Ok(())
}
