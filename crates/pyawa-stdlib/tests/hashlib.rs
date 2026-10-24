//! `_hashlib` 最小面（round 20，task-44）的契约测试：**纯 Rust md5／sha1／sha256**。
//!
//! 期望值来自本机 `python3` 3.14.4 实测（标准测试向量）；测试只经公开的
//! `Instance`／`_hashlib` 模块面走，不碰对象内部布局。

use core::ptr::NonNull;

use pyawa_core::{BuiltinFunctionObject, ExecError, Header, Instance};
use pyawa_stdlib::_hashlib_module;

fn native(instance: &Instance, namespace: NonNull<Header>, name: &str) -> pyawa_core::NativeFn {
    let function = instance
        .dict_get(namespace, name)
        .unwrap_or_else(|| panic!("`_hashlib.{name}` 必须存在"));
    // SAFETY: 模块里放进去的都是原生可调用对象。
    unsafe { (*function.as_ptr().cast::<BuiltinFunctionObject>()).function() }
}

fn call(
    instance: &Instance,
    handler: pyawa_core::NativeFn,
    args: &[NonNull<Header>],
    kwargs: &[(NonNull<Header>, NonNull<Header>)],
) -> Result<NonNull<Header>, ExecError> {
    // SAFETY: 实参由本测试持有（handler 只借用）。
    unsafe { handler(instance, None, args, kwargs) }
}

/// 调哈希对象的方法（属性通道拿到的是**绑定方法**对象 ⇒ 必须走 `call_value`）。
fn call_method(
    instance: &Instance,
    object: NonNull<Header>,
    name: &str,
    args: &[NonNull<Header>],
) -> Result<NonNull<Header>, ExecError> {
    let method = instance
        .attribute_optional_of(object, name)
        .expect("方法属性读失败")
        .unwrap_or_else(|| panic!("哈希对象必须有 {name}"));
    // SAFETY: 方法对象由属性通道给出的新引用，存活。
    let result = pyawa_core::executor::call::call_value(instance, method, args, &[]);
    instance.release(method);
    result
}

fn hexdigest_of(instance: &Instance, object: NonNull<Header>) -> String {
    let result = call_method(instance, object, "hexdigest", &[]).expect("hexdigest 不应失败");
    instance.text_of(result).expect("hexdigest 必须给 str").to_owned()
}

fn hash_hex(instance: &Instance, namespace: NonNull<Header>, constructor: &str, data: &[u8]) -> String {
    let constructor = native(instance, namespace, constructor);
    let argument = instance.new_bytes(data);
    let object = call(instance, constructor, &[argument], &[]).expect("构造器不应失败");
    hexdigest_of(instance, object)
}

#[test]
fn standard_vectors_match_the_reference() {
    let instance = Instance::new();
    let namespace = _hashlib_module::build(&instance);
    let empty: &[u8] = b"";
    assert_eq!(
        hash_hex(&instance, namespace, "openssl_md5", empty),
        "d41d8cd98f00b204e9800998ecf8427e"
    );
    assert_eq!(
        hash_hex(&instance, namespace, "openssl_md5", b"abc"),
        "900150983cd24fb0d6963f7d28e17f72"
    );
    assert_eq!(
        hash_hex(&instance, namespace, "openssl_sha1", b"abc"),
        "a9993e364706816aba3e25717850c26c9cd0d89d"
    );
    assert_eq!(
        hash_hex(&instance, namespace, "openssl_sha256", b"abc"),
        "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"
    );
    // 跨分组（> 64 字节）向量：`Nobody inspects the spammish repetition`。
    let sample = b"Nobody inspects the spammish repetition";
    assert_eq!(
        hash_hex(&instance, namespace, "openssl_md5", sample),
        "bb649c83dd1ea5c9d9dec9a18df0ffe9"
    );
    assert_eq!(
        hash_hex(&instance, namespace, "openssl_sha1", sample),
        "531b07a0f5b66477a21742d2827176264f4bbfe2"
    );
    assert_eq!(
        hash_hex(&instance, namespace, "openssl_sha256", sample),
        "031edd7d41651593c5fe5c006fa5752b37fddff7bc4e843aa6af0c950f4b9406"
    );
}

#[test]
fn update_copy_and_incremental_updates() {
    let instance = Instance::new();
    let namespace = _hashlib_module::build(&instance);
    let constructor = native(&instance, namespace, "openssl_md5");
    let object = call(&instance, constructor, &[], &[]).expect("空构造");
    for piece in [b"Nobody inspects".as_slice(), b" the spammish repetition".as_slice()] {
        let data = instance.new_bytes(piece);
        call_method(&instance, object, "update", &[data]).expect("update 不应失败");
    }
    assert_eq!(hexdigest_of(&instance, object), "bb649c83dd1ea5c9d9dec9a18df0ffe9");
    // `digest` 可重复调用，不改状态。
    assert_eq!(hexdigest_of(&instance, object), "bb649c83dd1ea5c9d9dec9a18df0ffe9");
    // copy：先 copy、再 update 原对象，互不影响。
    let clone = call_method(&instance, object, "copy", &[]).expect("copy 不应失败");
    assert_eq!(hexdigest_of(&instance, clone), "bb649c83dd1ea5c9d9dec9a18df0ffe9");
}

#[test]
fn compare_digest_matches_the_reference() {
    let instance = Instance::new();
    let namespace = _hashlib_module::build(&instance);
    let compare_digest = native(&instance, namespace, "compare_digest");
    // 两个 bytes：等/不等/长度不等。
    for (left, right, expected) in [
        (b"abc".as_slice(), b"abc".as_slice(), true),
        (b"abc".as_slice(), b"abd".as_slice(), false),
        (b"abc".as_slice(), b"ab".as_slice(), false),
    ] {
        let left = instance.new_bytes(left);
        let right = instance.new_bytes(right);
        let result = call(&instance, compare_digest, &[left, right], &[]).expect("bytes 比较");
        assert_eq!(instance.bool_value(result), Some(expected));
    }
    // 两个 ASCII str：值比较。
    let left = instance.new_str("token");
    let right = instance.new_str("token");
    let result = call(&instance, compare_digest, &[left, right], &[]).expect("str 比较");
    assert_eq!(instance.bool_value(result), Some(true));
    // 非 ASCII str ⇒ 参照文案。
    let left = instance.new_str("é");
    let right = instance.new_str("é");
    let error = call(&instance, compare_digest, &[left, right], &[]).expect_err("非 ASCII 必须报错");
    match error {
        ExecError::Raised { exception } => {
            // SAFETY: exception 是存活对象。
            let ty = unsafe { exception.as_ref() }.ty();
            // SAFETY: ty 由注册表持有。
            assert_eq!(unsafe { ty.as_ref() }.name(), "TypeError");
        }
        other => panic!("期望 TypeError，实际 {other:?}"),
    }
    // 实参个数：`compare_digest expected 2 arguments, got N`。
    let only = instance.new_bytes(b"a");
    let error = call(&instance, compare_digest, &[only], &[]).expect_err("缺实参必须报错");
    match error {
        ExecError::Raised { exception } => {
            // SAFETY: exception 是存活对象。
            let ty = unsafe { exception.as_ref() }.ty();
            // SAFETY: ty 由注册表持有。
            assert_eq!(unsafe { ty.as_ref() }.name(), "TypeError");
        }
        other => panic!("期望 TypeError，实际 {other:?}"),
    }
}

#[test]
fn attributes_and_new_and_honest_boundaries() {
    let instance = Instance::new();
    let namespace = _hashlib_module::build(&instance);
    let constructor = native(&instance, namespace, "openssl_sha256");
    let object = call(&instance, constructor, &[], &[]).expect("空构造");
    for (attribute, expected) in [("name", "sha256"), ("digest_size", "32"), ("block_size", "64")] {
        let value = instance
            .attribute_optional_of(object, attribute)
            .expect("属性读失败")
            .unwrap_or_else(|| panic!("哈希对象必须有 {attribute}"));
        let actual = if let Some(text) = instance.text_of(value) {
            text.to_owned()
        } else {
            instance.int_value(value).expect("整数属性").to_string()
        };
        assert_eq!(actual, expected, "{attribute}");
    }
    // `_hashlib.new`：大小写不敏感；未知算法 ValueError。
    let new = native(&instance, namespace, "new");
    let name = instance.new_str("SHA256");
    let data = instance.new_bytes(b"abc");
    let object = call(&instance, new, &[name, data], &[]).expect("new 不应失败");
    assert_eq!(
        hexdigest_of(&instance, object),
        "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"
    );
    // 已知但**本轮未实现**的算法：如实 NotImplementedError（不冒充、不假数据）。
    let unimplemented = instance.new_str("sha512");
    let error = call(&instance, new, &[unimplemented], &[]).expect_err("未实现算法必须如实报错");
    match error {
        ExecError::Raised { exception } => {
            // SAFETY: exception 是存活对象。
            let ty = unsafe { exception.as_ref() }.ty();
            // SAFETY: ty 由注册表持有。
            assert_eq!(unsafe { ty.as_ref() }.name(), "NotImplementedError");
        }
        other => panic!("期望 NotImplementedError，实际 {other:?}"),
    }
    // 完全未知的算法：ValueError（`hashlib.new` 据此回退并给出同一句 `unsupported hash type`）。
    let unknown = instance.new_str("definitely-not-a-hash");
    let error = call(&instance, new, &[unknown], &[]).expect_err("未知算法必须报 ValueError");
    match error {
        ExecError::Raised { exception } => {
            // SAFETY: exception 是存活对象。
            let ty = unsafe { exception.as_ref() }.ty();
            // SAFETY: ty 由注册表持有。
            assert_eq!(unsafe { ty.as_ref() }.name(), "ValueError");
        }
        other => panic!("期望 ValueError，实际 {other:?}"),
    }
    // KDF：如实 NotImplementedError。
    let pbkdf2 = native(&instance, namespace, "pbkdf2_hmac");
    let error = call(&instance, pbkdf2, &[], &[]).expect_err("KDF 必须如实报未实现");
    match error {
        ExecError::Raised { exception } => {
            // SAFETY: exception 是存活对象。
            let ty = unsafe { exception.as_ref() }.ty();
            // SAFETY: ty 由注册表持有。
            assert_eq!(unsafe { ty.as_ref() }.name(), "NotImplementedError");
        }
        other => panic!("期望 NotImplementedError，实际 {other:?}"),
    }
}
