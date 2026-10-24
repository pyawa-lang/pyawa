//! `_hashlib` 模块（round 20，task-44/AP）：**最小诚实面**——md5／sha1／sha256。
//!
//! 本轮只实现这三种算法，**纯 Rust、不引任何新依赖**；其余 OpenSSL 算法块／KDF 一律
//! **如实未实现**（`NotImplementedError`／缺失，不假数据）。
//!
//! ## 形状（照参照 3.14.4 实测）
//! - 三个构造器：`openssl_md5`／`openssl_sha1`／`openssl_sha256`（0/1 个位置实参 `data`，
//!   关键字 `usedforsecurity=` **接受并忽略**——本层没有 FIPS 安全策略）；
//! - `_hashlib.new(name, data=b'', **kwargs)`：名字不区分大小写；不认识的算法报
//!   `ValueError: unsupported hash type <名>`（`hashlib.new` 据此回退并给出同一文案）；
//! - `openssl_md_meth_names`：只列**真有的**三种（参照是 frozenset 且含 OpenSSL 全部算法，
//!   **如实偏差**；本层用 set 以配合 `hashlib.py` 的 `algorithms_available.union(...)`）；
//! - `pbkdf2_hmac`／`scrypt`：名字在、真调用 ⇒ `NotImplementedError`（KDF 未接，不假数据）。
//!
//! ## 哈希对象（`HASH` 类型，三种算法共用——照参照 `type(_hashlib.openssl_md5())` 同型）
//! `.update(data)`／`.digest()`／`.hexdigest()`／`.copy()`／`.name`／`.digest_size`／`.block_size`；
//! `data` 收 `bytes`／`bytearray`（其它 buffer 类型如实 `TypeError: object supporting the buffer API
//! required`；`str` 照参照 `Strings must be encoded before hashing`）。
//!
//! **如实偏差** ✗：① `type(h).__name__` 是 `HASH` 但 `__module__` 不是 `_hashlib`（类型构造面；
//! 不冒充 module 归属）；② `repr(h)` 是本层通用对象形（参照 `<md5 _hashlib.HASH object @ 0x…>`）；
//! ③ 内部状态作为实例属性 `_algorithm`／`_words`／`_buffer`／`_total` 可见（参照里没有这些属性；
//! `dir()`／`getattr` 会看到 —— 如实登记，不做隐藏）；④ 长度计数用 `i64`（> 2^63 字节的输入
//! 不现实，超出部分未定义）。

use core::cell::RefCell;
use core::ptr::NonNull;

use pyawa_core::{AttributeObject, ExecError, Header, Instance, NativeFn};

/// 模块名（`_hashlib`）。
pub const NAME: &str = "_hashlib";

/// 模块 `__doc__`（如实：最小诚实面）。
pub const DOC: &str = "Minimal honest hash interface: pure-Rust md5/sha1/sha256 only. \
Other OpenSSL algorithms and KDFs are not implemented.";

/// 哈希对象的类型名（参照：三种算法**共用** `_hashlib.HASH`）。
const HASH_TYPE: &str = "HASH";

/// "未实现算法"的占位对象类型（只用于 `__get_openssl_constructor` 的**探测调用**返回物；
/// 其方法一律如实 `NotImplementedError`，不给假摘要）。
const UNSUPPORTED_HASH_TYPE: &str = "UNSUPPORTED_HASH";

/// `_blake2` 模块名（上游 `hashlib.py` 对 blake2 只走 builtin 路径 ⇒ 这个名字必须有）。
pub const BLAKE2_NAME: &str = "_blake2";

/// 本轮**没有**实现（如实挂账）的 OpenSSL 算法名 —— `hashlib.py` 的 `__always_supported`
/// 里除 md5／sha1／sha256 之外的名字。它们由本模块提供"探测可过、真调用如实
/// `NotImplementedError`"的构造器，免得 `hashlib.py` 走 `ValueError` 回退路径去
/// `logging.exception`（那会在导入期打噪音，且本层 logging 处理 traceback 还会崩）。
const UNSUPPORTED_ALGORITHMS: &[&str] = &[
    "sha224",
    "sha384",
    "sha512",
    "sha3_224",
    "sha3_256",
    "sha3_384",
    "sha3_512",
    "shake_128",
    "shake_256",
    "blake2b",
    "blake2s",
];

/// 造一个原生可调用对象（**新引用**；与其它模块同一做法）。
fn make_native(instance: &Instance, name: &'static str, handler: NativeFn) -> NonNull<Header> {
    let ty = instance
        .type_named("builtin_function_or_method")
        .expect("builtin_function_or_method 在引导期已登记");
    let object = instance.alloc(pyawa_core::BuiltinFunctionObject::new(
        ty,
        name,
        core::cell::Cell::new(handler),
    ));
    object.into_raw().cast::<Header>()
}

// --------------------------------------------------------------------------- //
// 算法常量（MD5 的 K 表由 sin 生成、SHA-256 的 K 表由素数立方根生成；均照规范）
// --------------------------------------------------------------------------- //

/// MD5 每轮左移位数（4 组各 4 轮）。
const MD5_SHIFTS: [u32; 64] = [
    7, 12, 17, 22, 7, 12, 17, 22, 7, 12, 17, 22, 7, 12, 17, 22,
    5, 9, 14, 20, 5, 9, 14, 20, 5, 9, 14, 20, 5, 9, 14, 20,
    4, 11, 16, 23, 4, 11, 16, 23, 4, 11, 16, 23, 4, 11, 16, 23,
    6, 10, 15, 21, 6, 10, 15, 21, 6, 10, 15, 21, 6, 10, 15, 21,
];

/// MD5 的 K 表（`floor(abs(sin(i+1)) * 2^32)`）。
const MD5_K: [u32; 64] = [
    0xd76aa478, 0xe8c7b756, 0x242070db, 0xc1bdceee, 0xf57c0faf, 0x4787c62a, 0xa8304613, 0xfd469501,
    0x698098d8, 0x8b44f7af, 0xffff5bb1, 0x895cd7be, 0x6b901122, 0xfd987193, 0xa679438e, 0x49b40821,
    0xf61e2562, 0xc040b340, 0x265e5a51, 0xe9b6c7aa, 0xd62f105d, 0x02441453, 0xd8a1e681, 0xe7d3fbc8,
    0x21e1cde6, 0xc33707d6, 0xf4d50d87, 0x455a14ed, 0xa9e3e905, 0xfcefa3f8, 0x676f02d9, 0x8d2a4c8a,
    0xfffa3942, 0x8771f681, 0x6d9d6122, 0xfde5380c, 0xa4beea44, 0x4bdecfa9, 0xf6bb4b60, 0xbebfbc70,
    0x289b7ec6, 0xeaa127fa, 0xd4ef3085, 0x04881d05, 0xd9d4d039, 0xe6db99e5, 0x1fa27cf8, 0xc4ac5665,
    0xf4292244, 0x432aff97, 0xab9423a7, 0xfc93a039, 0x655b59c3, 0x8f0ccc92, 0xffeff47d, 0x85845dd1,
    0x6fa87e4f, 0xfe2ce6e0, 0xa3014314, 0x4e0811a1, 0xf7537e82, 0xbd3af235, 0x2ad7d2bb, 0xeb86d391,
];

/// SHA-256 的 K 表（前 64 个素数的立方根小数部分 × 2^32）。
const SHA256_K: [u32; 64] = [
    0x428a2f98, 0x71374491, 0xb5c0fbcf, 0xe9b5dba5,
    0x3956c25b, 0x59f111f1, 0x923f82a4, 0xab1c5ed5,
    0xd807aa98, 0x12835b01, 0x243185be, 0x550c7dc3,
    0x72be5d74, 0x80deb1fe, 0x9bdc06a7, 0xc19bf174,
    0xe49b69c1, 0xefbe4786, 0x0fc19dc6, 0x240ca1cc,
    0x2de92c6f, 0x4a7484aa, 0x5cb0a9dc, 0x76f988da,
    0x983e5152, 0xa831c66d, 0xb00327c8, 0xbf597fc7,
    0xc6e00bf3, 0xd5a79147, 0x06ca6351, 0x14292967,
    0x27b70a85, 0x2e1b2138, 0x4d2c6dfc, 0x53380d13,
    0x650a7354, 0x766a0abb, 0x81c2c92e, 0x92722c85,
    0xa2bfe8a1, 0xa81a664b, 0xc24b8b70, 0xc76c51a3,
    0xd192e819, 0xd6990624, 0xf40e3585, 0x106aa070,
    0x19a4c116, 0x1e376c08, 0x2748774c, 0x34b0bcb5,
    0x391c0cb3, 0x4ed8aa4a, 0x5b9cca4f, 0x682e6ff3,
    0x748f82ee, 0x78a5636f, 0x84c87814, 0x8cc70208,
    0x90befffa, 0xa4506ceb, 0xbef9a3f7, 0xc67178f2,
];

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Algorithm {
    Md5,
    Sha1,
    Sha256,
}

impl Algorithm {
    /// 名字归一（**不区分大小写**；参照 `hashlib.new('MD5')` 可用）。
    fn from_name(name: &str) -> Option<Self> {
        match name.to_ascii_lowercase().as_str() {
            "md5" => Some(Self::Md5),
            "sha1" => Some(Self::Sha1),
            "sha256" => Some(Self::Sha256),
            _ => None,
        }
    }

    fn name(self) -> &'static str {
        match self {
            Self::Md5 => "md5",
            Self::Sha1 => "sha1",
            Self::Sha256 => "sha256",
        }
    }

    fn digest_size(self) -> usize {
        match self {
            Self::Md5 => 16,
            Self::Sha1 => 20,
            Self::Sha256 => 32,
        }
    }

    /// 分组长度：三种算法都是 64 字节。
    fn block_size(self) -> usize {
        64
    }

    fn initial_words(self) -> Vec<u32> {
        match self {
            Self::Md5 => vec![0x6745_2301, 0xefcd_ab89, 0x98ba_dcfe, 0x1032_5476],
            Self::Sha1 => vec![0x6745_2301, 0xefcd_ab89, 0x98ba_dcfe, 0x1032_5476, 0xc3d2_e1f0],
            Self::Sha256 => vec![
                0x6a09_e667, 0xbb67_ae85, 0x3c6e_f372, 0xa54f_f53a,
                0x510e_527f, 0x9b05_688c, 0x1f83_d9ab, 0x5be0_cd19,
            ],
        }
    }

    /// 摘要字的字节序：MD5 小端、SHA-1／SHA-256 大端。
    fn words_are_little_endian(self) -> bool {
        matches!(self, Self::Md5)
    }
}

/// 流式哈希状态（**全部在 Python 对象里**：`_words`／`_buffer`／`_total`）。
#[derive(Clone, Debug)]
struct HashState {
    algorithm: Algorithm,
    words: Vec<u32>,
    buffer: Vec<u8>,
    total: u64,
}

impl HashState {
    fn new(algorithm: Algorithm) -> Self {
        Self {
            algorithm,
            words: algorithm.initial_words(),
            buffer: Vec::new(),
            total: 0,
        }
    }

    fn update(&mut self, data: &[u8]) {
        self.total = self.total.wrapping_add(data.len() as u64);
        self.buffer.extend_from_slice(data);
        let block_size = self.algorithm.block_size();
        let mut consumed = 0usize;
        while self.buffer.len() - consumed >= block_size {
            let block: Vec<u8> = self.buffer[consumed..consumed + block_size].to_vec();
            self.process_block(&block);
            consumed += block_size;
        }
        if consumed > 0 {
            self.buffer.drain(..consumed);
        }
    }

    /// 处理一个完整分组（调用方保证长度正好是 `block_size`）。
    fn process_block(&mut self, block: &[u8]) {
        match self.algorithm {
            Algorithm::Md5 => {
                let mut schedule = [0u32; 16];
                for (index, word) in block.chunks_exact(4).enumerate() {
                    schedule[index] = u32::from_le_bytes(word.try_into().expect("4 字节"));
                }
                let (mut a, mut b, mut c, mut d) = (
                    self.words[0],
                    self.words[1],
                    self.words[2],
                    self.words[3],
                );
                for round in 0..64 {
                    let (mixed, word_index) = if round < 16 {
                        ((b & c) | ((!b) & d), round)
                    } else if round < 32 {
                        ((d & b) | ((!d) & c), (5 * round + 1) % 16)
                    } else if round < 48 {
                        (b ^ c ^ d, (3 * round + 5) % 16)
                    } else {
                        (c ^ (b | !d), (7 * round) % 16)
                    };
                    let temporary = d;
                    d = c;
                    c = b;
                    let sum = a
                        .wrapping_add(mixed)
                        .wrapping_add(MD5_K[round])
                        .wrapping_add(schedule[word_index]);
                    b = b.wrapping_add(sum.rotate_left(MD5_SHIFTS[round]));
                    a = temporary;
                }
                self.words[0] = self.words[0].wrapping_add(a);
                self.words[1] = self.words[1].wrapping_add(b);
                self.words[2] = self.words[2].wrapping_add(c);
                self.words[3] = self.words[3].wrapping_add(d);
            }
            Algorithm::Sha1 => {
                let mut schedule = [0u32; 80];
                for index in 0..16 {
                    schedule[index] = u32::from_be_bytes(
                        block[index * 4..index * 4 + 4].try_into().expect("4 字节"),
                    );
                }
                for index in 16..80 {
                    schedule[index] = (schedule[index - 3]
                        ^ schedule[index - 8]
                        ^ schedule[index - 14]
                        ^ schedule[index - 16])
                        .rotate_left(1);
                }
                let (mut a, mut b, mut c, mut d, mut e) = (
                    self.words[0],
                    self.words[1],
                    self.words[2],
                    self.words[3],
                    self.words[4],
                );
                for round in 0..80 {
                    let (mixed, constant) = if round < 20 {
                        ((b & c) | ((!b) & d), 0x5a82_7999u32)
                    } else if round < 40 {
                        (b ^ c ^ d, 0x6ed9_eba1)
                    } else if round < 60 {
                        ((b & c) | (b & d) | (c & d), 0x8f1b_bcdc)
                    } else {
                        (b ^ c ^ d, 0xca62_c1d6)
                    };
                    let temporary = a
                        .rotate_left(5)
                        .wrapping_add(mixed)
                        .wrapping_add(e)
                        .wrapping_add(constant)
                        .wrapping_add(schedule[round]);
                    e = d;
                    d = c;
                    c = b.rotate_left(30);
                    b = a;
                    a = temporary;
                }
                self.words[0] = self.words[0].wrapping_add(a);
                self.words[1] = self.words[1].wrapping_add(b);
                self.words[2] = self.words[2].wrapping_add(c);
                self.words[3] = self.words[3].wrapping_add(d);
                self.words[4] = self.words[4].wrapping_add(e);
            }
            Algorithm::Sha256 => {
                let mut schedule = [0u32; 64];
                for index in 0..16 {
                    schedule[index] = u32::from_be_bytes(
                        block[index * 4..index * 4 + 4].try_into().expect("4 字节"),
                    );
                }
                for index in 16..64 {
                    let sigma0 = schedule[index - 15].rotate_right(7)
                        ^ schedule[index - 15].rotate_right(18)
                        ^ (schedule[index - 15] >> 3);
                    let sigma1 = schedule[index - 2].rotate_right(17)
                        ^ schedule[index - 2].rotate_right(19)
                        ^ (schedule[index - 2] >> 10);
                    schedule[index] = sigma1
                        .wrapping_add(schedule[index - 7])
                        .wrapping_add(sigma0)
                        .wrapping_add(schedule[index - 16]);
                }
                let mut working = [
                    self.words[0], self.words[1], self.words[2], self.words[3],
                    self.words[4], self.words[5], self.words[6], self.words[7],
                ];
                for round in 0..64 {
                    let big_sigma1 = working[4].rotate_right(6)
                        ^ working[4].rotate_right(11)
                        ^ working[4].rotate_right(25);
                    let choose = (working[4] & working[5]) ^ ((!working[4]) & working[6]);
                    let temporary1 = working[7]
                        .wrapping_add(big_sigma1)
                        .wrapping_add(choose)
                        .wrapping_add(SHA256_K[round])
                        .wrapping_add(schedule[round]);
                    let big_sigma0 = working[0].rotate_right(2)
                        ^ working[0].rotate_right(13)
                        ^ working[0].rotate_right(22);
                    let majority = (working[0] & working[1])
                        ^ (working[0] & working[2])
                        ^ (working[1] & working[2]);
                    let temporary2 = big_sigma0.wrapping_add(majority);
                    working[7] = working[6];
                    working[6] = working[5];
                    working[5] = working[4];
                    working[4] = working[3].wrapping_add(temporary1);
                    working[3] = working[2];
                    working[2] = working[1];
                    working[1] = working[0];
                    working[0] = temporary1.wrapping_add(temporary2);
                }
                for index in 0..8 {
                    self.words[index] = self.words[index].wrapping_add(working[index]);
                }
            }
        }
    }

    /// 最终摘要（**不改自身**；可重复调用）。
    fn digest(&self) -> Vec<u8> {
        let mut state = self.clone();
        let mut data = state.buffer.clone();
        data.push(0x80);
        while data.len() % state.algorithm.block_size() != state.algorithm.block_size() - 8 {
            data.push(0);
        }
        let bit_length = state.total.wrapping_mul(8);
        if state.algorithm.words_are_little_endian() {
            data.extend_from_slice(&bit_length.to_le_bytes());
        } else {
            data.extend_from_slice(&bit_length.to_be_bytes());
        }
        let block_size = state.algorithm.block_size();
        for chunk in data.chunks_exact(block_size) {
            state.process_block(chunk);
        }
        let mut output = Vec::with_capacity(state.algorithm.digest_size());
        for word in &state.words {
            if state.algorithm.words_are_little_endian() {
                output.extend_from_slice(&word.to_le_bytes());
            } else {
                output.extend_from_slice(&word.to_be_bytes());
            }
        }
        output
    }
}

// --------------------------------------------------------------------------- //
// Python 对象状态读写（对象＝`HASH` 类型的 `AttributeObject`，状态在实例字典里）
// --------------------------------------------------------------------------- //

/// 把 `HashState` 写进对象的实例字典（`dict_set` 是**接管**语义 ⇒ 值都是新引用）。
fn store_state(instance: &Instance, mapping: NonNull<Header>, state: &HashState) {
    instance.dict_set(
        mapping,
        "_algorithm",
        instance.new_str(state.algorithm.name()),
    );
    let words = instance.new_list(
        state
            .words
            .iter()
            .map(|word| instance.new_int(i64::from(*word)))
            .collect(),
    );
    instance.dict_set(mapping, "_words", words);
    instance.dict_set(mapping, "_buffer", instance.new_bytes(&state.buffer));
    instance.dict_set(mapping, "_total", instance.new_int(state.total as i64));
}

/// 从对象的实例字典读回状态；结构不对给 `None`（内部不变量，调用方按异常处理）。
fn read_state(instance: &Instance, object: NonNull<Header>) -> Option<HashState> {
    let mapping = pyawa_core::mounted_instance_dict(instance, object)?;
    let algorithm_object = instance.dict_get(mapping, "_algorithm")?;
    let algorithm = Algorithm::from_name(instance.text_of(algorithm_object)?)?;
    let words_object = instance.dict_get(mapping, "_words")?;
    let words: Vec<u32> = instance
        .list_items(words_object)?
        .iter()
        .map(|item| instance.int_value(*item).unwrap_or(0) as u32)
        .collect();
    let buffer_object = instance.dict_get(mapping, "_buffer")?;
    let buffer = instance.bytes_value(buffer_object)?.to_vec();
    let total_object = instance.dict_get(mapping, "_total")?;
    let total = instance.int_value(total_object)? as u64;
    Some(HashState {
        algorithm,
        words,
        buffer,
        total,
    })
}

/// 造一个哈希对象（**新引用**）。
fn alloc_hash_object(instance: &Instance, state: &HashState) -> NonNull<Header> {
    let hash_type = instance
        .type_named(HASH_TYPE)
        .expect("`HASH` 类型在 `_hashlib` 建模块时已登记");
    let mapping = instance.new_dict();
    store_state(instance, mapping, state);
    instance.dict_set(mapping, "name", instance.new_str(state.algorithm.name()));
    instance.dict_set(
        mapping,
        "digest_size",
        instance.new_int(state.algorithm.digest_size() as i64),
    );
    instance.dict_set(
        mapping,
        "block_size",
        instance.new_int(state.algorithm.block_size() as i64),
    );
    instance
        .alloc(AttributeObject::new(hash_type, RefCell::new(Some(mapping))))
        .into_raw()
        .cast::<Header>()
}

/// 读 `bytes`／`bytearray` 载荷（不是这两种给 `None`；不报错）。
fn bytes_like_data(instance: &Instance, object: NonNull<Header>) -> Option<Vec<u8>> {
    if let Some(bytes) = instance.bytes_value(object) {
        return Some(bytes.to_vec());
    }
    if instance.type_name(instance.type_of(object)) == "bytearray" {
        // 走公开的 `bytes(bytearray)` 构造（stdlib 不能碰载荷布局）。
        let bytes_type = instance.type_named("bytes").expect("bytes 已登记");
        let converted =
            pyawa_core::executor::call::call_value(instance, bytes_type.cast(), &[object], &[]).ok()?;
        let value = instance.bytes_value(converted).map(|slice| slice.to_vec());
        instance.release(converted);
        return value;
    }
    None
}

/// 取哈希实参的字节（照参照的实参口径：`bytes`／`bytearray` 收；`str` 报"先编码"）。
fn hash_data(instance: &Instance, object: NonNull<Header>) -> Result<Vec<u8>, ExecError> {
    if instance.text_of(object).is_some() {
        // 照参照文案（`hashlib.md5("x")`／`update("x")`）。
        return Err(instance.raise_builtin_error("TypeError", "Strings must be encoded before hashing"));
    }
    bytes_like_data(instance, object).ok_or_else(|| {
        instance.raise_builtin_error("TypeError", "object supporting the buffer API required")
    })
}

/// `_hashlib.compare_digest(a, b)`：`Lib/hmac.py` 在 `_hashlib` 可导入时**直接取它**
/// （`from _operator import _compare_digest` 那条 except 分支才用另一处）。
///
/// 语义照参照 3.14.4 实测：两个 `str`（**只收 ASCII**，非 ASCII ⇒
/// `TypeError: comparing strings with non-ASCII characters is not supported`）或两个 buffer
/// （`bytes`／`bytearray`）逐字节比；`str` 与 buffer 混比 ⇒
/// `TypeError: a bytes-like object is required, not 'str'`；其它类型 ⇒ 参照的
/// `unsupported operand types(s) or combination of types: '<左>' and '<右>'`（**原文照抄**）。
/// 本层**不做常数时间保证**（`CX-4` 的范围面；语义值一致，如实登记）。
fn compare_digest_native(
    instance: &Instance,
    _bound: Option<NonNull<Header>>,
    args: &[NonNull<Header>],
    kwargs: &[(NonNull<Header>, NonNull<Header>)],
) -> Result<NonNull<Header>, ExecError> {
    if !kwargs.is_empty() {
        return Err(instance.raise_builtin_error(
            "TypeError",
            "_hashlib.compare_digest() takes no keyword arguments",
        ));
    }
    if args.len() != 2 {
        return Err(instance.raise_builtin_error(
            "TypeError",
            &format!("compare_digest expected 2 arguments, got {}", args.len()),
        ));
    }
    let equal = match (
        instance.text_of(args[0]).map(str::to_owned),
        instance.text_of(args[1]).map(str::to_owned),
    ) {
        (Some(left), Some(right)) => {
            if !left.is_ascii() || !right.is_ascii() {
                return Err(instance.raise_builtin_error(
                    "TypeError",
                    "comparing strings with non-ASCII characters is not supported",
                ));
            }
            left == right
        }
        (Some(_), None) | (None, Some(_)) => {
            return Err(instance.raise_builtin_error(
                "TypeError",
                "a bytes-like object is required, not 'str'",
            ))
        }
        (None, None) => match (
            bytes_like_data(instance, args[0]),
            bytes_like_data(instance, args[1]),
        ) {
            (Some(left), Some(right)) => left == right,
            _ => {
                return Err(instance.raise_builtin_error(
                    "TypeError",
                    &format!(
                        "unsupported operand types(s) or combination of types: '{}' and '{}'",
                        instance.type_name(instance.type_of(args[0])),
                        instance.type_name(instance.type_of(args[1]))
                    ),
                ))
            }
        },
    };
    Ok(instance.new_bool(equal))
}

// --------------------------------------------------------------------------- //
// 构造器族
// --------------------------------------------------------------------------- //

/// 解析构造器实参：0/1 个位置实参 `data`；关键字只认 `usedforsecurity`（**接受并忽略**）。
fn constructor_arguments(
    instance: &Instance,
    function: &str,
    args: &[NonNull<Header>],
    kwargs: &[(NonNull<Header>, NonNull<Header>)],
) -> Result<Vec<u8>, ExecError> {
    for (key, _value) in kwargs {
        let Some(name) = instance.text_of(*key) else { continue };
        if name == "usedforsecurity" {
            continue;
        }
        return Err(instance.raise_builtin_error(
            "TypeError",
            &format!("{function}() got an unexpected keyword argument '{name}'"),
        ));
    }
    if args.len() > 1 {
        return Err(instance.raise_builtin_error(
            "TypeError",
            &format!(
                "{function}() takes at most 1 positional argument ({} given)",
                args.len()
            ),
        ));
    }
    match args.first() {
        Some(data) => hash_data(instance, *data),
        None => Ok(Vec::new()),
    }
}

/// 三个具名构造器的共用实现。
fn construct(
    instance: &Instance,
    function: &str,
    args: &[NonNull<Header>],
    kwargs: &[(NonNull<Header>, NonNull<Header>)],
    algorithm: Algorithm,
) -> Result<NonNull<Header>, ExecError> {
    let data = constructor_arguments(instance, function, args, kwargs)?;
    let mut state = HashState::new(algorithm);
    state.update(&data);
    Ok(alloc_hash_object(instance, &state))
}

fn openssl_md5_native(
    instance: &Instance,
    _bound: Option<NonNull<Header>>,
    args: &[NonNull<Header>],
    kwargs: &[(NonNull<Header>, NonNull<Header>)],
) -> Result<NonNull<Header>, ExecError> {
    construct(instance, "openssl_md5", args, kwargs, Algorithm::Md5)
}

fn openssl_sha1_native(
    instance: &Instance,
    _bound: Option<NonNull<Header>>,
    args: &[NonNull<Header>],
    kwargs: &[(NonNull<Header>, NonNull<Header>)],
) -> Result<NonNull<Header>, ExecError> {
    construct(instance, "openssl_sha1", args, kwargs, Algorithm::Sha1)
}

fn openssl_sha256_native(
    instance: &Instance,
    _bound: Option<NonNull<Header>>,
    args: &[NonNull<Header>],
    kwargs: &[(NonNull<Header>, NonNull<Header>)],
) -> Result<NonNull<Header>, ExecError> {
    construct(instance, "openssl_sha256", args, kwargs, Algorithm::Sha256)
}

/// `_hashlib.new(name, data=b'', **kwargs)`（`hashlib.new` 的底层）。
fn new_native(
    instance: &Instance,
    _bound: Option<NonNull<Header>>,
    args: &[NonNull<Header>],
    kwargs: &[(NonNull<Header>, NonNull<Header>)],
) -> Result<NonNull<Header>, ExecError> {
    let Some(name_object) = args.first().copied() else {
        return Err(instance.raise_builtin_error(
            "TypeError",
            "new() missing required argument 'name' (pos 1)",
        ));
    };
    let Some(name) = instance.text_of(name_object).map(str::to_owned) else {
        return Err(instance.raise_builtin_error("TypeError", "new() argument 'name' must be str"));
    };
    let Some(algorithm) = Algorithm::from_name(&name) else {
        // **已知但本轮未实现** ⇒ 如实 `NotImplementedError`（`hashlib.new` 只捕获 `ValueError`
        // ⇒ 不会回退去编假数据 ✓）。
        if UNSUPPORTED_ALGORITHMS
            .iter()
            .any(|candidate| candidate.eq_ignore_ascii_case(&name))
        {
            return Err(instance.raise_builtin_error(
                "NotImplementedError",
                &format!(
                    "`_hashlib.new('{name}')`：本层只接 md5／sha1／sha256 的纯 Rust 实现（P3-37 挂账，不假数据）"
                ),
            ));
        }
        // 参照 `_hashlib.new` 抛 `UnsupportedDigestmodError`（`ValueError` 的子类）；
        // `hashlib.new` 捕获 `ValueError` 后回退并最终报同一句 `unsupported hash type` ✓。
        return Err(instance.raise_builtin_error(
            "ValueError",
            &format!("unsupported hash type {name}"),
        ));
    };
    let rest = &args[1..];
    let data = constructor_arguments(instance, "new", rest, kwargs)?;
    let mut state = HashState::new(algorithm);
    state.update(&data);
    Ok(alloc_hash_object(instance, &state))
}

/// `_hashlib.hmac_new`／`hmac_digest`：**本层没有 C 实现的 HMAC**（如实不伪装）。
///
/// 上游 `Lib/hmac.py` 的设计是：`_hashlib` 可导入时先试 C 路径，C 抛
/// `UnsupportedDigestmodError`（"这个 digestmod 我这边不支持"）就**回退纯 Python 实现** ✓。
/// 本层正好用这条既有回退：抛该异常 ⇒ `hmac.py` 用**我们真实的** md5／sha1／sha256 对象
/// 走纯 Python HMAC（结果仍是真 HMAC，不假数据），只是没有 C 的速度优势 ✓。
///
/// **如实偏差** ✗：`_hashlib.UnsupportedDigestmodError` 在参照里是 **`ValueError` 的独立子类**；
/// 本层没有"stdlb 侧造异常类"的口径 ⇒ 别名到 `ValueError`（`except` 语义会**过宽**一点：会连带
/// 接住该路径上的普通 `ValueError`，实际这里只有本函数会抛，影响可忽略，如实登记）。
fn unsupported_digestmod_error(instance: &Instance) -> ExecError {
    instance.raise_builtin_error(
        "ValueError",
        "UnsupportedDigestmodError（本层 `_hashlib` 无 C 实现 HMAC，`hmac.py` 走纯 Python 回退 ✓）",
    )
}

fn hmac_new_native(
    instance: &Instance,
    _bound: Option<NonNull<Header>>,
    _args: &[NonNull<Header>],
    _kwargs: &[(NonNull<Header>, NonNull<Header>)],
) -> Result<NonNull<Header>, ExecError> {
    Err(unsupported_digestmod_error(instance))
}

fn hmac_digest_native(
    instance: &Instance,
    _bound: Option<NonNull<Header>>,
    _args: &[NonNull<Header>],
    _kwargs: &[(NonNull<Header>, NonNull<Header>)],
) -> Result<NonNull<Header>, ExecError> {
    Err(unsupported_digestmod_error(instance))
}

/// `_hashlib.pbkdf2_hmac`：KDF 未接（如实 `NotImplementedError`，不假数据）。
fn pbkdf2_hmac_native(
    instance: &Instance,
    _bound: Option<NonNull<Header>>,
    _args: &[NonNull<Header>],
    _kwargs: &[(NonNull<Header>, NonNull<Header>)],
) -> Result<NonNull<Header>, ExecError> {
    Err(instance.raise_builtin_error(
        "NotImplementedError",
        "`_hashlib.pbkdf2_hmac`：本层只接 md5／sha1／sha256 的纯 Rust 实现，KDF 如实未实现（P3-37 挂账，不假数据）",
    ))
}

/// `_hashlib.scrypt`：KDF 未接（如实 `NotImplementedError`）。
fn scrypt_native(
    instance: &Instance,
    _bound: Option<NonNull<Header>>,
    _args: &[NonNull<Header>],
    _kwargs: &[(NonNull<Header>, NonNull<Header>)],
) -> Result<NonNull<Header>, ExecError> {
    Err(instance.raise_builtin_error(
        "NotImplementedError",
        "`_hashlib.scrypt`：本层只接 md5／sha1／sha256 的纯 Rust 实现，KDF 如实未实现（P3-37 挂账，不假数据）",
    ))
}

// --------------------------------------------------------------------------- //
// 哈希对象方法（挂在 `HASH` 类型字典上 ⇒ 实例访问时绑定成方法）
// --------------------------------------------------------------------------- //

/// 方法调用取 `self`：实例调用取 `bound`；类型级调用（`HASH.update(h, …)`）取第一个位置实参。
fn method_self<'a>(
    bound: Option<NonNull<Header>>,
    args: &'a [NonNull<Header>],
) -> (Option<NonNull<Header>>, &'a [NonNull<Header>]) {
    match bound {
        Some(this) => (Some(this), args),
        None => match args.split_first() {
            Some((this, rest)) => (Some(*this), rest),
            None => (None, args),
        },
    }
}

fn update_native(
    instance: &Instance,
    bound: Option<NonNull<Header>>,
    args: &[NonNull<Header>],
    _kwargs: &[(NonNull<Header>, NonNull<Header>)],
) -> Result<NonNull<Header>, ExecError> {
    let (this, rest) = method_self(bound, args);
    let Some(this) = this else {
        return Err(instance.raise_builtin_error(
            "TypeError",
            "HASH.update() takes exactly one argument (0 given)",
        ));
    };
    if rest.len() != 1 {
        return Err(instance.raise_builtin_error(
            "TypeError",
            &format!("HASH.update() takes exactly one argument ({} given)", rest.len()),
        ));
    }
    let data = hash_data(instance, rest[0])?;
    let Some(mut state) = read_state(instance, this) else {
        return Err(instance.raise_builtin_error("TypeError", "HASH.update(): 对象状态读不出来"));
    };
    state.update(&data);
    if let Some(mapping) = pyawa_core::mounted_instance_dict(instance, this) {
        store_state(instance, mapping, &state);
    }
    Ok(instance.retain(instance.singletons().none()))
}

/// `digest()` 的共用实现。
fn digest_native(
    instance: &Instance,
    bound: Option<NonNull<Header>>,
    args: &[NonNull<Header>],
    _kwargs: &[(NonNull<Header>, NonNull<Header>)],
) -> Result<NonNull<Header>, ExecError> {
    let (this, rest) = method_self(bound, args);
    let Some(this) = this else {
        return Err(instance.raise_builtin_error(
            "TypeError",
            "HASH.digest() takes no arguments (0 given)",
        ));
    };
    if !rest.is_empty() {
        return Err(instance.raise_builtin_error(
            "TypeError",
            &format!("HASH.digest() takes no arguments ({} given)", rest.len()),
        ));
    }
    let Some(state) = read_state(instance, this) else {
        return Err(instance.raise_builtin_error("TypeError", "HASH.digest(): 对象状态读不出来"));
    };
    let digest = state.digest();
    Ok(instance.new_bytes(&digest))
}

fn hexdigest_native(
    instance: &Instance,
    bound: Option<NonNull<Header>>,
    args: &[NonNull<Header>],
    _kwargs: &[(NonNull<Header>, NonNull<Header>)],
) -> Result<NonNull<Header>, ExecError> {
    let (this, rest) = method_self(bound, args);
    let Some(this) = this else {
        return Err(instance.raise_builtin_error(
            "TypeError",
            "HASH.hexdigest() takes no arguments (0 given)",
        ));
    };
    if !rest.is_empty() {
        return Err(instance.raise_builtin_error(
            "TypeError",
            &format!("HASH.hexdigest() takes no arguments ({} given)", rest.len()),
        ));
    }
    let Some(state) = read_state(instance, this) else {
        return Err(instance.raise_builtin_error(
            "TypeError",
            "HASH.hexdigest(): 对象状态读不出来",
        ));
    };
    let digest = state.digest();
    let mut text = String::with_capacity(digest.len() * 2);
    for byte in digest {
        text.push_str(&format!("{byte:02x}"));
    }
    Ok(instance.new_str(&text))
}

fn copy_native(
    instance: &Instance,
    bound: Option<NonNull<Header>>,
    args: &[NonNull<Header>],
    _kwargs: &[(NonNull<Header>, NonNull<Header>)],
) -> Result<NonNull<Header>, ExecError> {
    let (this, rest) = method_self(bound, args);
    let Some(this) = this else {
        return Err(instance.raise_builtin_error(
            "TypeError",
            "HASH.copy() takes no arguments (0 given)",
        ));
    };
    if !rest.is_empty() {
        return Err(instance.raise_builtin_error(
            "TypeError",
            &format!("HASH.copy() takes no arguments ({} given)", rest.len()),
        ));
    }
    let Some(state) = read_state(instance, this) else {
        return Err(instance.raise_builtin_error("TypeError", "HASH.copy(): 对象状态读不出来"));
    };
    Ok(alloc_hash_object(instance, &state))
}

// --------------------------------------------------------------------------- //
// 未实现算法的"诚实构造器"（`hashlib.py` 导入期会逐个探测）
// --------------------------------------------------------------------------- //
//
// `Lib/hashlib.py` 的导入期对 `__always_supported` 的 14 个算法逐个 `__get_hash(name)`：
// 缺 `_hashlib.openssl_<name>` ⇒ 回退 `__get_builtin_constructor` ⇒ 也缺 ⇒ `ValueError` ⇒
// 外层 `logging.exception(...)`。本层 logging 的 traceback 处理会崩（`NoneType.tb_frame`）⇒
// **必须让这些名字都存在**。做法：给出**诚实构造器**——探测调用（只有 `usedforsecurity=`
// 关键字）返回一个"未实现占位对象"（其方法一律 `NotImplementedError`，不给假摘要）；任何
// 真实调用（带 data 或裸调用）⇒ 直接 `NotImplementedError`。
//
// **如实偏差** ✗：参照的 `hashlib.sha512(usedforsecurity=False)` 会返回可用的 sha512 对象；
// 本层返回的是占位对象，真正使用（digest/hexdigest/update）时如实报未实现。

fn alloc_unsupported_hash(instance: &Instance, algorithm: &str) -> NonNull<Header> {
    let ty = instance
        .type_named(UNSUPPORTED_HASH_TYPE)
        .expect("`UNSUPPORTED_HASH` 类型在 `_hashlib` 建模块时已登记");
    let mapping = instance.new_dict();
    instance.dict_set(mapping, "name", instance.new_str(algorithm));
    instance
        .alloc(AttributeObject::new(ty, RefCell::new(Some(mapping))))
        .into_raw()
        .cast::<Header>()
}

/// 未实现算法的探测/调用共用实现。
fn unsupported_openssl(
    instance: &Instance,
    args: &[NonNull<Header>],
    kwargs: &[(NonNull<Header>, NonNull<Header>)],
    function: &str,
    algorithm: &str,
) -> Result<NonNull<Header>, ExecError> {
    // `__get_openssl_constructor` 的可用性探测：`f(usedforsecurity=False)`（无 data、仅该关键字）
    // ⇒ 返回占位对象（调用方随即丢弃）；除此之外一律如实未实现。
    let probe = args.is_empty()
        && kwargs.len() == 1
        && instance.text_of(kwargs[0].0).as_deref() == Some("usedforsecurity");
    if probe {
        return Ok(alloc_unsupported_hash(instance, algorithm));
    }
    Err(instance.raise_builtin_error(
        "NotImplementedError",
        &format!(
            "`_hashlib.{function}`：本层只接 md5／sha1／sha256 的纯 Rust 实现（{algorithm} 未接，P3-37 挂账，不假数据）"
        ),
    ))
}

/// 生成一个"未实现算法"的 openssl 构造器原生。
macro_rules! unsupported_openssl_native {
    ($function:ident => $name:literal, $algorithm:literal $(,)?) => {
        fn $function(
            instance: &Instance,
            _bound: Option<NonNull<Header>>,
            args: &[NonNull<Header>],
            kwargs: &[(NonNull<Header>, NonNull<Header>)],
        ) -> Result<NonNull<Header>, ExecError> {
            unsupported_openssl(instance, args, kwargs, $name, $algorithm)
        }
    };
}

unsupported_openssl_native!(openssl_sha224_native => "openssl_sha224", "sha224");
unsupported_openssl_native!(openssl_sha384_native => "openssl_sha384", "sha384");
unsupported_openssl_native!(openssl_sha512_native => "openssl_sha512", "sha512");
unsupported_openssl_native!(openssl_sha3_224_native => "openssl_sha3_224", "sha3_224");
unsupported_openssl_native!(openssl_sha3_256_native => "openssl_sha3_256", "sha3_256");
unsupported_openssl_native!(openssl_sha3_384_native => "openssl_sha3_384", "sha3_384");
unsupported_openssl_native!(openssl_sha3_512_native => "openssl_sha3_512", "sha3_512");
unsupported_openssl_native!(openssl_shake_128_native => "openssl_shake_128", "shake_128");
unsupported_openssl_native!(openssl_shake_256_native => "openssl_shake_256", "shake_256");

/// 未实现占位对象的**方法**：真调用一律如实 `NotImplementedError`。
macro_rules! unsupported_hash_method {
    ($function:ident => $name:literal $(,)?) => {
        fn $function(
            instance: &Instance,
            _bound: Option<NonNull<Header>>,
            _args: &[NonNull<Header>],
            _kwargs: &[(NonNull<Header>, NonNull<Header>)],
        ) -> Result<NonNull<Header>, ExecError> {
            Err(instance.raise_builtin_error(
                "NotImplementedError",
                concat!(
                    "`_hashlib` 的 ",
                    $name,
                    " 方法：该算法的真实实现未接（P3-37 挂账，不假数据）"
                ),
            ))
        }
    };
}

unsupported_hash_method!(unsupported_update_native => "update");
unsupported_hash_method!(unsupported_digest_native => "digest");
unsupported_hash_method!(unsupported_hexdigest_native => "hexdigest");
unsupported_hash_method!(unsupported_copy_native => "copy");

/// 建 `_blake2` 模块（上游 `hashlib.py` 对 blake2 只走 builtin 路径 ⇒ 名字必须存在；
/// 真调用如实 `NotImplementedError`）。
pub fn build_blake2(instance: &Instance) -> NonNull<Header> {
    let namespace = instance.new_dict();
    for (name, handler) in [
        ("blake2b", blake2b_native as NativeFn),
        ("blake2s", blake2s_native as NativeFn),
    ] {
        let function = make_native(instance, name, handler);
        instance.dict_set(namespace, name, function);
    }
    let module_name = instance.new_str(BLAKE2_NAME);
    instance.dict_set(namespace, "__name__", module_name);
    let doc = instance.new_str("Minimal honest placeholder: blake2b/blake2s are not implemented in this layer.");
    instance.dict_set(namespace, "__doc__", doc);
    namespace
}

fn blake2b_native(
    instance: &Instance,
    _bound: Option<NonNull<Header>>,
    _args: &[NonNull<Header>],
    _kwargs: &[(NonNull<Header>, NonNull<Header>)],
) -> Result<NonNull<Header>, ExecError> {
    Err(instance.raise_builtin_error(
        "NotImplementedError",
        "`_blake2.blake2b`：本层只接 md5／sha1／sha256 的纯 Rust 实现（blake2b 未接，P3-37 挂账，不假数据）",
    ))
}

fn blake2s_native(
    instance: &Instance,
    _bound: Option<NonNull<Header>>,
    _args: &[NonNull<Header>],
    _kwargs: &[(NonNull<Header>, NonNull<Header>)],
) -> Result<NonNull<Header>, ExecError> {
    Err(instance.raise_builtin_error(
        "NotImplementedError",
        "`_blake2.blake2s`：本层只接 md5／sha1／sha256 的纯 Rust 实现（blake2s 未接，P3-37 挂账，不假数据）",
    ))
}

// --------------------------------------------------------------------------- //
// `Lib/hashlib.py` 的导入口：`set.union`
// --------------------------------------------------------------------------- //
//
// 上游 `hashlib.py` 在**导入期**做 `algorithms_available.union(_hashlib.openssl_md_meth_names)`，
// 而本层 `set` 的方法面目前只有 `add`／`discard`／`pop`／`update`／`copy`／`issuperset`（没有
// `union`）⇒ 直接 `AttributeError: 'set' object has no attribute 'union'` ⇒ `import hashlib` 失败。
// 这里给 `set` 类型**补一个诚实的 `union`**（真取并集、真去重；不是假接口）——写域在 stdlib，
// 不动 core；语义照参照（返回新 set，可接多个可迭代实参）。
// **如实范围** ✗：实参只收 list／tuple／set／frozenset（走 core 的 `collect_iterable`），其余可迭代
// 对象（生成器／range 等）如实 `TypeError: '<类型>' object is not iterable`（迭代协议面随后补）。

fn set_union_native(
    instance: &Instance,
    bound: Option<NonNull<Header>>,
    args: &[NonNull<Header>],
    _kwargs: &[(NonNull<Header>, NonNull<Header>)],
) -> Result<NonNull<Header>, ExecError> {
    let (this, rest) = method_self(bound, args);
    let Some(this) = this else {
        return Err(instance.raise_builtin_error("TypeError", "union() 缺少 self 实参"));
    };
    // 去重口径＝引擎统一的"值相等"（与 `set.add` 同一处真相）。
    let mut accumulated: Vec<NonNull<Header>> = Vec::new();
    let mut push_unique = |item: NonNull<Header>| -> bool {
        if accumulated
            .iter()
            .any(|existing| pyawa_core::executor::values_equal_public(instance, *existing, item))
        {
            return false;
        }
        accumulated.push(item);
        true
    };
    for item in instance.set_items(this).unwrap_or_default() {
        if push_unique(item) {
            // `new_set` 接管**新引用**；`set_items` 给的是借用 ⇒ 自己留一份。
            instance.retain(item);
        }
    }
    for argument in rest {
        let items = match instance.collect_iterable(*argument) {
            Ok(items) => items,
            Err(ExecError::Unsupported { .. }) => {
                return Err(instance.raise_builtin_error(
                    "TypeError",
                    &format!(
                        "'{}' object is not iterable",
                        instance.type_name(instance.type_of(*argument))
                    ),
                ))
            }
            Err(error) => return Err(error),
        };
        for item in items {
            if push_unique(item) {
                instance.retain(item);
            }
        }
    }
    Ok(instance.new_set(accumulated))
}

// --------------------------------------------------------------------------- //
// 模块构建
// --------------------------------------------------------------------------- //

/// 建 `_hashlib` 模块的命名空间（**新引用** 的 `dict`）。
pub fn build(instance: &Instance) -> NonNull<Header> {
    let namespace = instance.new_dict();
    // `HASH` 类型：三种算法共用（照参照 `type(_hashlib.openssl_md5())` 同型）。
    let hash_type = instance
        .type_named(HASH_TYPE)
        .unwrap_or_else(|| instance.new_attribute_type(HASH_TYPE));
    for (name, handler) in [
        ("update", update_native as NativeFn),
        ("digest", digest_native as NativeFn),
        ("hexdigest", hexdigest_native as NativeFn),
        ("copy", copy_native as NativeFn),
    ] {
        let function = make_native(instance, name, handler);
        instance.set_type_attribute(hash_type, name, function);
    }
    for (name, handler) in [
        ("openssl_md5", openssl_md5_native as NativeFn),
        ("openssl_sha1", openssl_sha1_native as NativeFn),
        ("openssl_sha256", openssl_sha256_native as NativeFn),
        // 未实现算法的"诚实构造器"（`hashlib.py` 导入期要逐个探测，见上方说明）。
        ("openssl_sha224", openssl_sha224_native as NativeFn),
        ("openssl_sha384", openssl_sha384_native as NativeFn),
        ("openssl_sha512", openssl_sha512_native as NativeFn),
        ("openssl_sha3_224", openssl_sha3_224_native as NativeFn),
        ("openssl_sha3_256", openssl_sha3_256_native as NativeFn),
        ("openssl_sha3_384", openssl_sha3_384_native as NativeFn),
        ("openssl_sha3_512", openssl_sha3_512_native as NativeFn),
        ("openssl_shake_128", openssl_shake_128_native as NativeFn),
        ("openssl_shake_256", openssl_shake_256_native as NativeFn),
        // `Lib/hmac.py` 在 `_hashlib` 可导入时直接取这个（本层必须先有它 ✓）。
        ("compare_digest", compare_digest_native as NativeFn),
        // C 实现 HMAC：本层没有 ⇒ 抛"不支持"让 `hmac.py` 走纯 Python 回退（见上方说明）。
        ("hmac_new", hmac_new_native as NativeFn),
        ("hmac_digest", hmac_digest_native as NativeFn),
        ("new", new_native as NativeFn),
        ("pbkdf2_hmac", pbkdf2_hmac_native as NativeFn),
        ("scrypt", scrypt_native as NativeFn),
    ] {
        let function = make_native(instance, name, handler);
        instance.dict_set(namespace, name, function);
    }
    // 未实现算法的占位对象类型（探测调用返回它；方法一律如实 NotImplementedError）。
    let unsupported_type = instance
        .type_named(UNSUPPORTED_HASH_TYPE)
        .unwrap_or_else(|| instance.new_attribute_type(UNSUPPORTED_HASH_TYPE));
    for (name, handler) in [
        ("update", unsupported_update_native as NativeFn),
        ("digest", unsupported_digest_native as NativeFn),
        ("hexdigest", unsupported_hexdigest_native as NativeFn),
        ("copy", unsupported_copy_native as NativeFn),
    ] {
        let function = make_native(instance, name, handler);
        instance.set_type_attribute(unsupported_type, name, function);
    }
    // 只列**真有的**算法名（`hashlib.py` 用它并进 `algorithms_available`）。
    let names: Vec<NonNull<Header>> = ["md5", "sha1", "sha256"]
        .iter()
        .map(|name| instance.new_str(name))
        .collect();
    let names = instance.new_set(names);
    instance.dict_set(namespace, "openssl_md_meth_names", names);
    // **`set.union`**：见上方「导入口」的说明（`Lib/hashlib.py` 导入期就要它）。
    if let Some(set_type) = instance.type_named("set") {
        let union = make_native(instance, "union", set_union_native as NativeFn);
        instance.set_type_attribute(set_type, "union", union);
    }
    let module_name = instance.new_str(NAME);
    instance.dict_set(namespace, "__name__", module_name);
    let doc = instance.new_str(DOC);
    instance.dict_set(namespace, "__doc__", doc);
    // `hmac.py` 的 `except _hashopenssl.UnsupportedDigestmodError`：别名到 `ValueError`
    // （参照是独立子类；本层没有 stdlib 侧造异常类的口径，见上方如实说明）。
    if let Some(value_error) = instance.type_named("ValueError") {
        instance.dict_set(namespace, "UnsupportedDigestmodError", value_error.cast());
    }
    namespace
}
