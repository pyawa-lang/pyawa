//! **任意精度整数**（`TS-45`／`P1-11`）的**纯算术核心**。
//!
//! 与对象模型**解耦**：这里的入参出参都是 Rust 值，验收靠 `tests/bigint.rs` 的参照夹具
//! （`tests/fixture-int-3.14.json`，由 `tools/gen_int_fixture.py` 在参照实现上**现场实测**导出）。
//! 接线（`int` 载荷、`BINARY_OP`、比较、`hash`、`repr`／`str` 的 4300 位上限、与 `float` 互转）
//! 随后逐笔做——本模块只提供**算法与不变量**。
//!
//! 口径（都对着夹具钉住，不是猜的）：
//!
//! - **`//` 是 floor、`%` 取除数的符号**（实测 `-7 // 2 == -4`、`7 % -2 == -1`）。
//!   Rust 的 `div_euclid`／`rem_euclid` 在**负除数**上与参照不一致 ⇒ 本模块不借它们。
//! - **`hash`**：`sign × (|x| mod (2^61-1))`，结果 `-1` 改判 `-2`（实测 13 条，含
//!   `hash(2**100) == 549755813888` 与 `i64::MAX`／`i64::MIN`）。
//! - `to_decimal` **不设位数上限**：上限（默认 4300）是调用点的策略（`TS-45` ①），
//!   调用方拿到字符串再按 `len()` 判——本模块提供数字，不替策略做决定。
//! - `to_f64` 溢出给 `±inf`（**不 panic**）：参照在 `float(huge)` 上抛 `OverflowError:
//!   int too large to convert to float`，映射是调用点的事。
//!
//! 内部表示：`limbs` 是 **2^32 进制的小端**，已规范化（无前导零；零 ⇒ 空表且 `negative == false`）。

use core::cmp::Ordering;

/// `hash` 用的模数：`2^61 - 1`（参照实现的 `_PyHASH_MODULUS`）。
const HASH_MODULUS: u128 = (1u128 << 61) - 1;

/// 已规范化的任意精度整数。
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct BigInt {
    negative: bool,
    limbs: Vec<u32>,
}

impl BigInt {
    /// 零。
    pub fn zero() -> Self {
        Self { negative: false, limbs: Vec::new() }
    }

    /// 从 `i64` 造（`i64::MIN` 也走这条，不经过 `abs`）。
    pub fn from_i64(value: i64) -> Self {
        if value == 0 {
            return Self::zero();
        }
        let negative = value < 0;
        let magnitude = value.unsigned_abs();
        Self { negative, limbs: vec![magnitude as u32, (magnitude >> 32) as u32] }.normalized()
    }

    /// 装得下就给 `Some`（`int_value` 的快路径用）。
    pub fn to_i64(&self) -> Option<i64> {
        let magnitude = self.to_u64()?;
        if self.negative {
            if magnitude == (i64::MAX as u64) + 1 {
                Some(i64::MIN)
            } else {
                i64::try_from(magnitude).ok().map(|value| -value)
            }
        } else {
            i64::try_from(magnitude).ok()
        }
    }

    /// 装得下 `u64` 就给 `Some`（内部与转换用）。
    pub fn to_u64(&self) -> Option<u64> {
        match self.limbs.len() {
            0 => Some(0),
            1 => Some(u64::from(self.limbs[0])),
            2 => Some(u64::from(self.limbs[0]) | (u64::from(self.limbs[1]) << 32)),
            _ => None,
        }
    }

    /// 是不是零。
    pub fn is_zero(&self) -> bool {
        self.limbs.is_empty()
    }

    /// 符号：`-1`／`0`／`1`。
    pub fn signum(&self) -> i32 {
        if self.limbs.is_empty() {
            0
        } else if self.negative {
            -1
        } else {
            1
        }
    }

    /// 相反数。
    pub fn neg(&self) -> Self {
        if self.is_zero() {
            return Self::zero();
        }
        Self { negative: !self.negative, limbs: self.limbs.clone() }
    }

    /// 绝对值。
    pub fn abs(&self) -> Self {
        Self { negative: false, limbs: self.limbs.clone() }
    }

    /// 加法。
    pub fn add(&self, other: &Self) -> Self {
        if self.negative == other.negative {
            return Self {
                negative: self.negative,
                limbs: add_magnitude(&self.limbs, &other.limbs),
            }
            .normalized();
        }
        // 异号 ⇒ 大减小，符号跟大的那个
        match cmp_magnitude(&self.limbs, &other.limbs) {
            Ordering::Equal => Self::zero(),
            Ordering::Greater => Self {
                negative: self.negative,
                limbs: sub_magnitude(&self.limbs, &other.limbs),
            }
            .normalized(),
            Ordering::Less => Self {
                negative: other.negative,
                limbs: sub_magnitude(&other.limbs, &self.limbs),
            }
            .normalized(),
        }
    }

    /// 减法。
    pub fn sub(&self, other: &Self) -> Self {
        self.add(&other.neg())
    }

    /// 乘法（schoolbook）。
    pub fn mul(&self, other: &Self) -> Self {
        if self.is_zero() || other.is_zero() {
            return Self::zero();
        }
        let mut limbs = vec![0u32; self.limbs.len() + other.limbs.len()];
        for (left_index, left) in self.limbs.iter().enumerate() {
            let mut carry: u64 = 0;
            for (right_index, right) in other.limbs.iter().enumerate() {
                let position = left_index + right_index;
                let current = u64::from(limbs[position])
                    + u64::from(*left) * u64::from(*right)
                    + carry;
                limbs[position] = current as u32;
                carry = current >> 32;
            }
            let mut position = left_index + other.limbs.len();
            while carry != 0 {
                let current = u64::from(limbs[position]) + carry;
                limbs[position] = current as u32;
                carry = current >> 32;
                position += 1;
            }
        }
        Self { negative: self.negative != other.negative, limbs }.normalized()
    }

    /// **floor 除法与取模**（Python 的 `//`／`%`）：除数或余数为零时给 `None`（除零）。
    pub fn divmod_floor(&self, other: &Self) -> Option<(Self, Self)> {
        if other.is_zero() {
            return None;
        }
        let (mut quotient, mut remainder) = divmod_trunc(self, other);
        // trunc 的余数跟**被除数**同号；参照的余数跟**除数**同号 ⇒ 余数非零且符号不同就借一位
        if !remainder.is_zero() && remainder.negative != other.negative {
            quotient = quotient.sub(&Self::from_i64(1));
            remainder = remainder.add(other);
        }
        Some((quotient, remainder))
    }

    /// 幂（指数非负）。负指数在参照里给 `float`（`2 ** -1 == 0.5`）⇒ 这里**不做**，
    /// 由调用点按"与 `float` 互转"那条路处理。
    pub fn pow_u32(&self, exponent: u32) -> Self {
        let mut result = Self::from_i64(1);
        let mut base = self.clone();
        let mut power = exponent;
        while power > 0 {
            if power & 1 == 1 {
                result = result.mul(&base);
            }
            power >>= 1;
            if power > 0 {
                base = base.mul(&base);
            }
        }
        result
    }

    /// 比较（含符号）。
    pub fn cmp(&self, other: &Self) -> Ordering {
        match (self.signum(), other.signum()) {
            (left, right) if left != right => left.cmp(&right),
            (0, 0) => Ordering::Equal,
            (1, 1) => cmp_magnitude(&self.limbs, &other.limbs),
            _ => cmp_magnitude(&other.limbs, &self.limbs),
        }
    }

    /// **`hash` 与参照一致**（`TS-45` ②）：`sign × (|x| mod (2^61-1))`，`-1` 改判 `-2`。
    pub fn hash(&self) -> i64 {
        if self.is_zero() {
            return 0;
        }
        let mut residue: u128 = 0;
        for limb in self.limbs.iter().rev() {
            residue = ((residue << 32) | u128::from(*limb)) % HASH_MODULUS;
        }
        let mut value = if self.negative { -(residue as i128) } else { residue as i128 };
        if value == -1 {
            value = -2;
        }
        value as i64
    }

    /// 十进制串（**不带**位数上限；上限是调用点的策略）。
    pub fn to_decimal(&self) -> String {
        if self.is_zero() {
            return "0".to_owned();
        }
        const BASE: u32 = 1_000_000_000;
        let mut chunks = Vec::new();
        let mut remaining = self.abs();
        while !remaining.is_zero() {
            let (next, chunk) = remaining.divmod_small(BASE);
            chunks.push(chunk);
            remaining = next;
        }
        let mut text = String::new();
        if self.negative {
            text.push('-');
        }
        text.push_str(&chunks.pop().expect("至少一块").to_string());
        for chunk in chunks.iter().rev() {
            text.push_str(&format!("{chunk:09}"));
        }
        text
    }

    /// 从十进制串解析（可选 `+`／`-`，其余必须是 ASCII 数字；不认下划线与空白）。
    ///
    /// 位数上限（`TS-45` ①）**不在这里**：调用点先看串长再决定要不要报 `ValueError`。
    pub fn from_decimal(text: &str) -> Option<Self> {
        let mut characters = text.chars().peekable();
        let mut negative = false;
        match characters.peek() {
            Some('+') => {
                characters.next();
            }
            Some('-') => {
                negative = true;
                characters.next();
            }
            _ => {}
        }
        let mut limbs: Vec<u32> = Vec::new();
        let mut count = 0usize;
        for character in characters {
            if !character.is_ascii_digit() {
                return None;
            }
            let digit = character as u32 - '0' as u32;
            mul_small_into(&mut limbs, 10);
            add_small_into(&mut limbs, digit);
            count += 1;
        }
        if count == 0 {
            return None;
        }
        Some(Self { negative, limbs }.normalized())
    }

    /// 转 `f64`：**正确舍入**（取前 54 位，按"最近偶数"进位）；溢出给 `±inf`。
    pub fn to_f64(&self) -> f64 {
        if self.limbs.is_empty() {
            return 0.0;
        }
        let bits = self.bit_length();
        let magnitude = if bits <= 53 {
            self.to_u64().expect("53 位装得下") as f64
        } else {
            let shift = bits - 54;
            let top = self.top_bits_u64(shift); // 54 位
            let sticky = self.any_bit_below(shift);
            let mut mantissa = top >> 1; // 53 位
            let round_bit = top & 1;
            let mut exponent = shift + 1;
            if round_bit == 1 && (sticky || (mantissa & 1) == 1) {
                mantissa += 1;
                if mantissa == (1u64 << 53) {
                    mantissa >>= 1;
                    exponent += 1;
                }
            }
            (mantissa as f64) * 2f64.powi(i32::try_from(exponent).unwrap_or(i32::MAX))
        };
        if self.negative { -magnitude } else { magnitude }
    }

    // ---- 内部：位与规范化 ----

    /// 去掉前导零并修正零的符号。
    fn normalized(mut self) -> Self {
        while matches!(self.limbs.last(), Some(0)) {
            self.limbs.pop();
        }
        if self.limbs.is_empty() {
            self.negative = false;
        }
        self
    }

    fn bit_length(&self) -> u64 {
        match self.limbs.last() {
            None => 0,
            Some(top) => (self.limbs.len() as u64 - 1) * 32 + u64::from(32 - top.leading_zeros()),
        }
    }

    fn bit(&self, index: u64) -> bool {
        let limb = (index / 32) as usize;
        let offset = (index % 32) as u32;
        self.limbs.get(limb).is_some_and(|value| (value >> offset) & 1 == 1)
    }

    /// 从 `shift` 位起往上的**连续位**（调用方保证不超过 64 位）。
    fn top_bits_u64(&self, shift: u64) -> u64 {
        let mut out = 0u64;
        for index in (shift..self.bit_length()).rev() {
            out = (out << 1) | u64::from(self.bit(index));
        }
        out
    }

    fn any_bit_below(&self, shift: u64) -> bool {
        (0..shift).any(|index| self.bit(index))
    }

    /// 除以一个小除数（十进制转换用）：O(limbs)。
    fn divmod_small(&self, divisor: u32) -> (Self, u32) {
        let mut quotient = vec![0u32; self.limbs.len()];
        let mut remainder: u64 = 0;
        for index in (0..self.limbs.len()).rev() {
            let current = (remainder << 32) | u64::from(self.limbs[index]);
            quotient[index] = (current / u64::from(divisor)) as u32;
            remainder = current % u64::from(divisor);
        }
        (
            Self { negative: self.negative, limbs: quotient }.normalized(),
            remainder as u32,
        )
    }
}

// --------------------------------------------------------------------------- #
// 量级运算（都按小端 `u32` 表，规范化由调用方负责）
// --------------------------------------------------------------------------- #

fn cmp_magnitude(left: &[u32], right: &[u32]) -> Ordering {
    let left = trim(left);
    let right = trim(right);
    if left.len() != right.len() {
        return left.len().cmp(&right.len());
    }
    for index in (0..left.len()).rev() {
        if left[index] != right[index] {
            return left[index].cmp(&right[index]);
        }
    }
    Ordering::Equal
}

fn trim(limbs: &[u32]) -> &[u32] {
    let mut end = limbs.len();
    while end > 0 && limbs[end - 1] == 0 {
        end -= 1;
    }
    &limbs[..end]
}

fn add_magnitude(left: &[u32], right: &[u32]) -> Vec<u32> {
    let length = left.len().max(right.len());
    let mut out = Vec::with_capacity(length + 1);
    let mut carry: u64 = 0;
    for index in 0..length {
        let sum = u64::from(*left.get(index).unwrap_or(&0))
            + u64::from(*right.get(index).unwrap_or(&0))
            + carry;
        out.push(sum as u32);
        carry = sum >> 32;
    }
    if carry != 0 {
        out.push(carry as u32);
    }
    out
}

/// `left - right`；调用方保证 `left >= right`。
fn sub_magnitude(left: &[u32], right: &[u32]) -> Vec<u32> {
    let mut out = Vec::with_capacity(left.len());
    let mut borrow: i64 = 0;
    for index in 0..left.len() {
        let difference = i64::from(left[index]) - i64::from(*right.get(index).unwrap_or(&0)) - borrow;
        if difference < 0 {
            out.push((difference + (1i64 << 32)) as u32);
            borrow = 1;
        } else {
            out.push(difference as u32);
            borrow = 0;
        }
    }
    out
}

fn shl1_magnitude(limbs: &mut Vec<u32>) {
    let mut carry = 0u32;
    for limb in limbs.iter_mut() {
        let next = *limb >> 31;
        *limb = (*limb << 1) | carry;
        carry = next;
    }
    if carry != 0 {
        limbs.push(carry);
    }
}

/// 截断除法（商向零取整、余数跟被除数同号）＋ `floor` 修正交给调用方。
fn divmod_trunc(dividend: &BigInt, divisor: &BigInt) -> (BigInt, BigInt) {
    let numerator = &dividend.limbs;
    let denominator = trim(&divisor.limbs);
    debug_assert!(!denominator.is_empty(), "除零在调用点已拦");
    let negative = dividend.negative != divisor.negative;
    if cmp_magnitude(numerator, denominator) == Ordering::Less {
        return (BigInt::zero(), BigInt { negative: dividend.negative, limbs: numerator.to_vec() }.normalized());
    }
    // 二进制长除法：逐位"左移一位、够减就减"。位数上限 ≈ 1.4 万（4300 位十进制）⇒ 够用。
    let bits = (numerator.len() as u64) * 32;
    let mut quotient = vec![0u32; numerator.len()];
    let mut remainder: Vec<u32> = Vec::new();
    for index in (0..bits).rev() {
        shl1_magnitude(&mut remainder);
        let limb = (index / 32) as usize;
        let offset = (index % 32) as u32;
        if (numerator[limb] >> offset) & 1 == 1 {
            if remainder.is_empty() {
                remainder.push(1);
            } else {
                remainder[0] |= 1;
            }
        }
        if cmp_magnitude(&remainder, denominator) != Ordering::Less {
            remainder = sub_magnitude(&remainder, denominator);
            quotient[limb] |= 1 << offset;
        }
    }
    (
        BigInt { negative, limbs: quotient }.normalized(),
        BigInt { negative: dividend.negative, limbs: remainder }.normalized(),
    )
}

fn mul_small_into(limbs: &mut Vec<u32>, factor: u32) {
    let mut carry: u64 = 0;
    for limb in limbs.iter_mut() {
        let product = u64::from(*limb) * u64::from(factor) + carry;
        *limb = product as u32;
        carry = product >> 32;
    }
    if carry != 0 {
        limbs.push(carry as u32);
    }
}

fn add_small_into(limbs: &mut Vec<u32>, addend: u32) {
    let mut carry = u64::from(addend);
    let mut index = 0;
    while carry != 0 {
        if index == limbs.len() {
            limbs.push(carry as u32);
            return;
        }
        let sum = u64::from(limbs[index]) + carry;
        limbs[index] = sum as u32;
        carry = sum >> 32;
        index += 1;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn normalization_and_round_trip() {
        assert_eq!(BigInt::from_i64(0).to_decimal(), "0");
        assert_eq!(BigInt::from_i64(-1).to_decimal(), "-1");
        for value in [i64::MIN, i64::MAX, -1, 0, 1, 256, 257, -5] {
            let big = BigInt::from_i64(value);
            assert_eq!(big.to_i64(), Some(value), "{value}");
            assert_eq!(BigInt::from_decimal(&big.to_decimal()).as_ref(), Some(&big), "{value}");
        }
        assert_eq!(BigInt::from_decimal(""), None);
        assert_eq!(BigInt::from_decimal("+"), None);
        assert_eq!(BigInt::from_decimal("12a"), None);
        assert!(BigInt::from_decimal("-0").expect("可解析").is_zero());
    }

    #[test]
    fn floor_division_and_modulo_follow_the_divisor_sign() {
        let cases = [
            (-7i64, 2i64, -4i64, 1i64),
            (7, -2, -4, -1),
            (-7, -2, 3, -1),
            (7, 2, 3, 1),
        ];
        for (a, b, quotient, remainder) in cases {
            let (q, r) = BigInt::from_i64(a)
                .divmod_floor(&BigInt::from_i64(b))
                .expect("非零除数");
            assert_eq!(q.to_i64(), Some(quotient), "{a} // {b}");
            assert_eq!(r.to_i64(), Some(remainder), "{a} % {b}");
        }
        assert!(BigInt::from_i64(1).divmod_floor(&BigInt::zero()).is_none());
    }
}
