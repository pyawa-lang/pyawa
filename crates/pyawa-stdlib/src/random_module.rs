//! `_random`（第 722 轮）：`Lib/random.py` 的**原生基类** ✓（`class Random(_random.Random)` ✓）。
//!
//! **如实登记的差异** ✗：参照的 `_random.Random` 是 **MT19937**（624 个字、`getstate()` 是
//! **625** 元组 ✓）；本层用 **xoshiro256\*\***（SplitMix64 摊种 ✓）—— **不追**"种子相同 ⇒ 序列逐字
//! 相同" ✗（那要把 MT19937 原样搬来 ✓，成本高，而 `random.py` 的语义层并不依赖具体序列 ✓）。
//! 对齐的是**语义** ✓：`random()` ∈ `[0, 1)` ✓、`getrandbits(k)` ∈ `[0, 2**k)` ✓、`seed()` 同参可复现 ✓、
//! `getstate()`／`setstate()` 往返一致 ✓；`getstate()` 的**形状**是 **4** 元组（参照 625 ✓）⇒
//! 跨实现的 state 不通用 ✗、`seed(None)` 的熵源是**时钟 ＋ 计数**（参照走 `os.urandom` ✓）。
//!
//! 状态放在**实例字典**里（每个 64 位字存一个整数 ✓）—— 不另开旁表 ✓ ⇒ 地址复用不会串状态 ✓。

use core::ptr::NonNull;

use pyawa_core::bigint::{BigInt, IntValue};
use pyawa_core::{ExecError, Header, Instance, NativeFn};

/// 模块名（`_random`）。
pub const NAME: &str = "_random";

/// 状态在实例字典里的键前缀（4 个字 ✓）。
const STATE_PREFIX: &str = "__pyawa_rng_";

/// `_random.Random` 的方法表 ✓（与参照的公开面一致 ✓）。
const METHODS: &[(&str, NativeFn)] = &[
    ("seed", seed_native as NativeFn),
    ("random", random_native as NativeFn),
    ("getrandbits", getrandbits_native as NativeFn),
    ("getstate", getstate_native as NativeFn),
    ("setstate", setstate_native as NativeFn),
];

/// 建 `_random` 模块的命名空间（**新引用** 的 `dict`）。
pub fn build(instance: &Instance) -> NonNull<Header> {
    let namespace = instance.new_dict();
    if let Some(class) = build_random_class(instance) {
        instance.dict_set(namespace, "Random", class);
    }
    let module_name = instance.new_str(NAME);
    instance.dict_set(namespace, "__name__", module_name);
    let doc = instance.new_str("_random：Lib/random.py 的原生基类（xoshiro256**；见本文件头差异说明）");
    instance.dict_set(namespace, "__doc__", doc);
    namespace
}

/// 建 `Random` 类（照 `_sre` 的 `ensure_class` 手法：`build_class_from_parts` ✓）。
fn build_random_class(instance: &Instance) -> Option<NonNull<Header>> {
    let base = instance.type_named("object")?.cast::<Header>();
    // 契约（`classes.rs`）：`build_class_from_parts` **吃掉**调用方那份 namespace ✓
    // ⇒ 这里刚建的 dict（rc=1）正是"自有的一份" ✓。
    let class_namespace = instance.new_dict();
    for (name, handler) in METHODS {
        let function = make_native(instance, name, *handler);
        instance.dict_set(class_namespace, name, function);
    }
    pyawa_core::build_class_from_parts(
        instance,
        "Random".to_owned(),
        vec![base],
        class_namespace,
        None,
    )
    .ok()
}

fn make_native(instance: &Instance, name: &str, handler: NativeFn) -> NonNull<Header> {
    let ty = instance
        .type_named("builtin_function_or_method")
        .expect("builtin_function_or_method 在引导期已登记");
    let object = instance.alloc(pyawa_core::BuiltinFunctionObject::new(
        ty,
        Box::leak(name.to_owned().into_boxed_str()),
        core::cell::Cell::new(handler),
    ));
    object.into_raw().cast::<Header>()
}

fn bound_object(
    instance: &Instance,
    bound: Option<NonNull<Header>>,
) -> Result<NonNull<Header>, ExecError> {
    bound.ok_or_else(|| instance.raise_builtin_error("TypeError", "descriptor needs an argument"))
}

// ---- 状态：实例字典里的 4 个字 ----------------------------------------------------

fn read_state(instance: &Instance, object: NonNull<Header>) -> Option<[u64; 4]> {
    let dict = pyawa_core::mounted_instance_dict(instance, object)?;
    let mut words = [0u64; 4];
    for (index, slot) in words.iter_mut().enumerate() {
        let key = format!("{STATE_PREFIX}{index}");
        let value = instance.dict_get(dict, &key)?;
        *slot = instance.int_value(value)? as u64;
    }
    Some(words)
}

fn write_state(instance: &Instance, object: NonNull<Header>, words: &[u64; 4]) {
    let Some(dict) = pyawa_core::mounted_instance_dict(instance, object) else {
        return;
    };
    for (index, word) in words.iter().enumerate() {
        let key = format!("{STATE_PREFIX}{index}");
        instance.dict_set(dict, &key, instance.new_int(*word as i64));
    }
}

/// 读状态；**没有就初始化**（`_random.Random()` 直接用时也要能跑 ✓）。
fn load_or_init(instance: &Instance, object: NonNull<Header>) -> [u64; 4] {
    if let Some(words) = read_state(instance, object) {
        return words;
    }
    let words = seed_words(entropy_seed());
    write_state(instance, object, &words);
    words
}

// ---- xoshiro256** ＋ SplitMix64 ---------------------------------------------------

/// SplitMix64：摊出 xoshiro 的 4 个状态字 ✓（确定性 ✓）。
fn splitmix64(state: &mut u64) -> u64 {
    *state = state.wrapping_add(0x9E37_79B9_7F4A_7C15);
    let mut z = *state;
    z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
    z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
    z ^ (z >> 31)
}

pub(crate) fn seed_words(seed: u64) -> [u64; 4] {
    let mut mixer = seed;
    [
        splitmix64(&mut mixer),
        splitmix64(&mut mixer),
        splitmix64(&mut mixer),
        splitmix64(&mut mixer),
    ]
}

/// **xoshiro256\*\*** 的一步 ✓。
pub(crate) fn next_u64(state: &mut [u64; 4]) -> u64 {
    let result = state[1].wrapping_mul(5).rotate_left(7).wrapping_mul(9);
    let t = state[1] << 17;
    state[2] ^= state[0];
    state[3] ^= state[1];
    state[1] ^= state[2];
    state[0] ^= state[3];
    state[2] ^= t;
    state[3] = state[3].rotate_left(45);
    result
}

/// `random()`：**53 位**尾数 ⇒ `[0, 1)` ✓（与参照同口径 ✓）。
fn next_f64(state: &mut [u64; 4]) -> f64 {
    (next_u64(state) >> 11) as f64 * (1.0 / 9_007_199_254_740_992.0)
}

pub(crate) fn entropy_seed() -> u64 {
    use std::sync::atomic::{AtomicU64, Ordering};
    static COUNTER: AtomicU64 = AtomicU64::new(0);
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|duration| duration.as_nanos() as u64)
        .unwrap_or(0);
    nanos ^ COUNTER.fetch_add(0x9E37_79B9_7F4A_7C15, Ordering::Relaxed)
}

/// FNV-1a 64：str／bytes 种子用 ✓（确定性 ✓）。
fn hash_bytes(bytes: &[u8]) -> u64 {
    let mut hash = 0xCBF2_9CE4_8422_2325u64;
    for byte in bytes {
        hash ^= *byte as u64;
        hash = hash.wrapping_mul(0x0000_0100_0000_01B3);
    }
    hash
}

// ---- 方法 -------------------------------------------------------------------------

/// `seed(a=None, version=2)` ✓（`version` 只影响 str／bytes 的折算，本层统一按字节哈希 ✓）。
fn seed_native(
    instance: &Instance,
    bound: Option<NonNull<Header>>,
    args: &[NonNull<Header>],
    _kwargs: &[(NonNull<Header>, NonNull<Header>)],
) -> Result<NonNull<Header>, ExecError> {
    let object = bound_object(instance, bound)?;
    let seed = match args.first().copied() {
        None => entropy_seed(),
        Some(value) if instance.type_of(value) == instance.singletons().none_type() => entropy_seed(),
        Some(value) => seed_value(instance, value)?,
    };
    write_state(instance, object, &seed_words(seed));
    Ok(instance.retain(instance.singletons().none()))
}

fn seed_value(instance: &Instance, value: NonNull<Header>) -> Result<u64, ExecError> {
    if let Some(number) = instance.int_value(value) {
        return Ok(number as u64);
    }
    if let Some(big) = instance.int_of(value) {
        // 大整数：按**十进制文本**哈希 ✓（确定性 ✓）。
        return Ok(hash_bytes(big.to_decimal().as_bytes()));
    }
    if let Some(number) = instance.float_value(value) {
        return Ok(number.to_bits());
    }
    if let Some(bytes) = instance.bytes_value(value) {
        return Ok(hash_bytes(bytes));
    }
    if let Some(text) = instance.text_of(value) {
        return Ok(hash_bytes(text.as_bytes()));
    }
    Err(instance.raise_builtin_error(
        "TypeError",
        "The only supported seed types are:\nNone, int, float, str, bytes, and bytearray.",
    ))
}

/// `random()` → `float` ∈ `[0, 1)` ✓。
fn random_native(
    instance: &Instance,
    bound: Option<NonNull<Header>>,
    _args: &[NonNull<Header>],
    _kwargs: &[(NonNull<Header>, NonNull<Header>)],
) -> Result<NonNull<Header>, ExecError> {
    let object = bound_object(instance, bound)?;
    let mut state = load_or_init(instance, object);
    let value = next_f64(&mut state);
    write_state(instance, object, &state);
    Ok(instance.new_float(value))
}

/// `getrandbits(k)` → `int` ∈ `[0, 2**k)` ✓（任意 `k` ✓，大数走 `BigInt` ✓）。
fn getrandbits_native(
    instance: &Instance,
    bound: Option<NonNull<Header>>,
    args: &[NonNull<Header>],
    _kwargs: &[(NonNull<Header>, NonNull<Header>)],
) -> Result<NonNull<Header>, ExecError> {
    let object = bound_object(instance, bound)?;
    let Some(k) = args.first().and_then(|value| instance.int_value(*value)) else {
        return Err(instance.raise_builtin_error(
            "TypeError",
            "getrandbits() missing required argument 'k' (pos 1)",
        ));
    };
    if k < 0 {
        return Err(instance.raise_builtin_error(
            "ValueError",
            "number of bits must be non-negative",
        ));
    }
    if k == 0 {
        return Ok(instance.new_int(0));
    }
    let mut state = load_or_init(instance, object);
    let result = if k < 64 {
        let word = next_u64(&mut state);
        instance.new_int((word >> (64 - k as u32)) as i64)
    } else {
        let bits = k as usize;
        let words = bits.div_ceil(64);
        let mut generated = Vec::with_capacity(words);
        for _ in 0..words {
            generated.push(next_u64(&mut state));
        }
        big_int_from_words(instance, &generated, bits)?
    };
    write_state(instance, object, &state);
    Ok(result)
}

/// 把**低位在前的 64 位字**拼成整数 ✓（最高字按 `bits` 截断 ✓）。
fn big_int_from_words(
    instance: &Instance,
    words: &[u64],
    bits: usize,
) -> Result<NonNull<Header>, ExecError> {
    let mut value = BigInt::zero();
    for (index, word) in words.iter().enumerate().rev() {
        let mut word = *word;
        if index + 1 == words.len() {
            let used = bits - index * 64;
            if used < 64 {
                word &= (1u64 << used) - 1;
            }
        }
        let shifted = value.shl(64).ok_or_else(|| {
            instance.raise_builtin_error("OverflowError", "getrandbits：位移溢出")
        })?;
        value = shifted.bit_or(&BigInt::from_u64(word));
    }
    Ok(instance.new_int_value(IntValue::from_big(value)))
}

/// `getstate()` → **4 元组** ✓（形状与参照的 625 不同 ✗ —— 见文件头 ✓）。
fn getstate_native(
    instance: &Instance,
    bound: Option<NonNull<Header>>,
    _args: &[NonNull<Header>],
    _kwargs: &[(NonNull<Header>, NonNull<Header>)],
) -> Result<NonNull<Header>, ExecError> {
    let object = bound_object(instance, bound)?;
    let state = load_or_init(instance, object);
    let items: Vec<NonNull<Header>> = state
        .iter()
        .map(|word| instance.new_int(*word as i64))
        .collect();
    Ok(instance.new_tuple(items))
}

/// `setstate(state)`：收**本层 `getstate()` 的 4 元组**（或 `None` ⇒ 重新熵播种 ✓）。
fn setstate_native(
    instance: &Instance,
    bound: Option<NonNull<Header>>,
    args: &[NonNull<Header>],
    _kwargs: &[(NonNull<Header>, NonNull<Header>)],
) -> Result<NonNull<Header>, ExecError> {
    let object = bound_object(instance, bound)?;
    let Some(state_object) = args.first().copied() else {
        return Err(instance.raise_builtin_error(
            "TypeError",
            "setstate() missing required argument 'state' (pos 1)",
        ));
    };
    if instance.type_of(state_object) == instance.singletons().none_type() {
        write_state(instance, object, &seed_words(entropy_seed()));
        return Ok(instance.retain(instance.singletons().none()));
    }
    let Some(items) = instance.tuple_items(state_object) else {
        return Err(instance.raise_builtin_error("TypeError", "state must be a tuple"));
    };
    if items.len() != 4 {
        return Err(instance.raise_builtin_error(
            "ValueError",
            &format!("state with {} items, expected 4", items.len()),
        ));
    }
    let mut words = [0u64; 4];
    for (index, item) in items.iter().enumerate() {
        let Some(word) = instance.int_value(*item) else {
            return Err(instance.raise_builtin_error("TypeError", "state items must be ints"));
        };
        words[index] = word as u64;
    }
    write_state(instance, object, &words);
    Ok(instance.retain(instance.singletons().none()))
}
