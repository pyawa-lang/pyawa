//! 对象头 `flags` 位分配（`docs/SPEC-object-model.md` **OM-7**）。
//!
//! 位的语义**只在这里定义一处**：`Header`、类型对象与回收器都从这里取常量。

/// **OM-7** bit 0：计数不增减。M1 只预留并保持 0（**OM-24**）。
pub const IMMORTAL: u32 = 1 << 0;
/// **OM-7** bit 1：参与循环回收。**OM-12** 要求可成环的类型必须置位。
pub const GC_TRACKED: u32 = 1 << 1;
/// **OM-7** bit 2：存在弱引用（回收顺序见 **OM-27**）。
pub const HAS_WEAKREFS: u32 = 1 << 2;
/// **OM-7** bit 3：正在执行 `__del__`，用于防重入（**OM-20** ①）。
pub const FINALIZING: u32 = 1 << 3;
/// **OM-7** bit 4–7：无 GIL 所需，**禁止**占用。
///
/// 写成**字面量**而不是 `0b1111 << 4`：`CX-7` 要求位常量"取值必须可求值"
/// （字面量／`1 << N`／已有常量的或），`check.py` 的 `T-CX-9` 会逐条求值。
pub const RESERVED_MASK: u32 = 0b1111_0000;
/// **OM-7** bit 8–31：类型相关，由类型对象自行定义。
///
/// 同样写作字面量（原为 `!0b1111_1111`，不在 `CX-7` 允许的三种形态内）。
pub const TYPE_MASK: u32 = 0xffff_ff00;
/// 本层已定义的位（bit 0–3），其余位留给预留区与类型。
pub const CORE_MASK: u32 = IMMORTAL | GC_TRACKED | HAS_WEAKREFS | FINALIZING;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bits_match_the_spec_table() {
        assert_eq!(IMMORTAL, 0b0000_0001);
        assert_eq!(GC_TRACKED, 0b0000_0010);
        assert_eq!(HAS_WEAKREFS, 0b0000_0100);
        assert_eq!(FINALIZING, 0b0000_1000);
        assert_eq!(RESERVED_MASK, 0b1111_0000);
        assert_eq!(CORE_MASK, 0b0000_1111);
    }

    #[test]
    fn bit_regions_do_not_overlap() {
        assert_eq!(CORE_MASK & RESERVED_MASK, 0);
        assert_eq!(CORE_MASK & TYPE_MASK, 0);
        assert_eq!(RESERVED_MASK & TYPE_MASK, 0);
        assert_eq!(CORE_MASK | RESERVED_MASK | TYPE_MASK, u32::MAX);
    }
}
