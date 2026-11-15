//! **内部集合的快速哈希**（round 248 性能，A2-3）：`live` 活表、类型索引、字典/集合的查找索引等。
//!
//! 为什么需要：这些集合的键是**对象地址**（`usize`）或**对象哈希**（`u64`），不是对抗输入 ⇒
//! 不需要 SipHash 的 DoS 抗性；而 `std::collections` 默认用 `RandomState`（SipHash-1-3）⇒
//! 每次分配／释放／查字典都要算一遍慢哈希 ✗（perf：`hash_one::<&usize>`＋sip 合计 ≈24%）。
//!
//! 口径：**只换哈希函数，不换语义** —— 桶内冲突仍由 `Eq` 消解 ✓；集合/映射的**迭代顺序**
//! 本来就是"未定义"（`live` 只在计数与整批释放里迭代，顺序无观察面 ✓）。**不**用于面向 Python
//! 的值哈希（那是 `values::hash_text`／`hash_bytes` 的 CPython 口径，另案 ✓）。

use core::hash::{BuildHasherDefault, Hasher};

/// FxHash 风格的乘法哈希（非密码学；用于内部地址／哈希键 ✓）。
pub struct FastHasher {
    hash: u64,
}

impl FastHasher {
    #[inline]
    fn add(&mut self, word: u64) {
        self.hash = (self.hash.rotate_left(5) ^ word).wrapping_mul(0x517c_c1b7_2722_0a95);
    }
}

impl Default for FastHasher {
    #[inline]
    fn default() -> Self {
        // 非零种子（空输入也能散开 ✓）。
        Self {
            hash: 0x9e37_79b9_7f4a_7c15,
        }
    }
}

impl Hasher for FastHasher {
    #[inline]
    fn finish(&self) -> u64 {
        self.hash
    }

    #[inline]
    fn write(&mut self, bytes: &[u8]) {
        let mut chunks = bytes.chunks_exact(8);
        for chunk in &mut chunks {
            let mut word = [0u8; 8];
            word.copy_from_slice(chunk);
            self.add(u64::from_le_bytes(word));
        }
        let rest = chunks.remainder();
        if !rest.is_empty() {
            let mut word = [0u8; 8];
            word[..rest.len()].copy_from_slice(rest);
            self.add(u64::from_le_bytes(word));
        }
    }

    #[inline]
    fn write_u8(&mut self, value: u8) {
        self.add(value as u64);
    }

    #[inline]
    fn write_u32(&mut self, value: u32) {
        self.add(value as u64);
    }

    #[inline]
    fn write_u64(&mut self, value: u64) {
        self.add(value);
    }

    #[inline]
    fn write_usize(&mut self, value: usize) {
        self.add(value as u64);
    }
}

/// `BuildHasher` 实现（`HashSet::default()` 可用 ✓）。
pub type FastBuildHasher = BuildHasherDefault<FastHasher>;
/// 内部用的快速 `HashSet`。
pub type FastSet<T> = std::collections::HashSet<T, FastBuildHasher>;
/// 内部用的快速 `HashMap`。
pub type FastMap<K, V> = std::collections::HashMap<K, V, FastBuildHasher>;
