//! `sys` 模块（**不依赖能力域**的部分）的契约测试（`SPEC-c-modules.md` §5.2.3）。
//!
//! 三类断言：
//! ① **身份**（`CX-13`）：`implementation.name` 报 `pyawa`，且与参照**必然不同**；
//! ② **语言级别**：`version_info`／`hexversion` 与参照**一致**（库用它做特性检测）；
//! ③ **与实现无关的常量**：`maxunicode`／`maxsize`／`byteorder` 与参照一致。

use core::ptr::NonNull;

use pyawa_core::{Header, Instance, IntObject, StrObject, TupleObject};
use pyawa_stdlib::sys_module;

#[path = "fixtures/sys.rs"]
mod fixture;

use fixture::{
    REFERENCE_BYTEORDER, REFERENCE_CACHE_TAG, REFERENCE_HEXVERSION,
    REFERENCE_IMPLEMENTATION_NAME, REFERENCE_MAXSIZE, REFERENCE_MAXUNICODE, REFERENCE_VERSION,
    REFERENCE_VERSION_INFO,
    REFERENCE_GET_WITH_ARGS_MESSAGE, REFERENCE_INT_MAX_STR_DIGITS, REFERENCE_SETTING_ZERO_SUCCEEDS,
    REFERENCE_SET_BELOW_MESSAGE, REFERENCE_SET_HUGE_MESSAGE, REFERENCE_SET_NEGATIVE_MESSAGE,
    REFERENCE_SET_NOT_INTEGER_MESSAGE, REFERENCE_SET_NO_ARGS_MESSAGE,
    REFERENCE_SET_TWO_ARGS_MESSAGE, REFERENCE_STR_DIGITS_THRESHOLD, REFERENCE_ZERO_MEANS_UNLIMITED,
};

fn int_of(object: NonNull<Header>) -> i64 {
    // SAFETY: 调用方保证是整数对象。
    unsafe { &*object.as_ptr().cast::<IntObject>() }.value.to_i64().expect("平台常量是小整数")
}

fn text_of(object: NonNull<Header>) -> String {
    // SAFETY: 调用方保证是字符串对象。
    unsafe { &*object.as_ptr().cast::<StrObject>() }.value().to_owned()
}

fn attribute(instance: &Instance, namespace: NonNull<Header>, name: &str) -> NonNull<Header> {
    instance
        .dict_get(namespace, name)
        .unwrap_or_else(|| panic!("§5.2.3：`sys.{name}` 必须存在"))
}

/// 读 `sys.implementation` 的字段：它们挂在**类型字典**上（`OM-10` 的沿 MRO 查），
/// 所以这里走核心的 `type_lookup`，而不是把实例当字典读（那会读到垃圾）。
fn implementation_field(
    instance: &Instance,
    implementation: NonNull<Header>,
    name: &str,
) -> NonNull<Header> {
    let ty = instance.type_of(implementation);
    instance
        .type_lookup(ty, name)
        .unwrap_or_else(|| panic!("§5.2.3：`sys.implementation.{name}` 必须存在"))
}

#[test]
fn the_identity_is_pyawa_not_cpython() {
    // `CX-13`：`implementation.name` **必须**报 `pyawa`——谎报 `cpython` 会让库去加载
    // 不存在的 C 扩展，而不是走它自带的多 Python 回退路径
    let instance = Instance::new();
    let namespace = sys_module::build(&instance);
    let implementation = attribute(&instance, namespace, "implementation");
    let name = implementation_field(&instance, implementation, "name");
    assert_eq!(text_of(name), sys_module::IMPLEMENTATION_NAME);
    assert_eq!(text_of(name), "pyawa");
    assert_ne!(
        text_of(name),
        REFERENCE_IMPLEMENTATION_NAME,
        "实现观测面：身份必须与参照不同（DESIGN §9）"
    );

    // `cache_tag` 用自己的值：`pyawa-<指令集版本>`，**禁止**冒用 `cpython-3xx`
    let tag = implementation_field(&instance, implementation, "cache_tag");
    let tag = text_of(tag);
    assert!(tag.starts_with("pyawa-"), "cache_tag ＝ {tag}");
    assert_ne!(tag, REFERENCE_CACHE_TAG, "不得冒用参照的 cache_tag");
    assert!(
        tag.ends_with(&pyawa_core::opcode_metadata::INSTRUCTION_SET_VERSION.to_string()),
        "cache_tag 带上指令集版本（BC-29）：{tag}"
    );

    // `implementation.version` 是**本实现自己的**版本（不是语言版本）
    let version = implementation_field(&instance, implementation, "version");
    // SAFETY: 上面刚放进的是五元组。
    let tuple = unsafe { &*version.as_ptr().cast::<TupleObject>() };
    assert_eq!(tuple.len(), 5, "版本元组是五元");
    assert_eq!(int_of(tuple.item(0).expect("有元素")), 0, "工作区版本 0.0.0");
}

#[test]
fn the_language_level_matches_the_reference() {
    // 语言级别报的是**所实现的语言**（对拍参照 3.14.4），不是实现自己的版本号：
    // 库用 `sys.version_info >= (3, 11)` 一类做特性检测，报错了会走错分支
    let instance = Instance::new();
    let namespace = sys_module::build(&instance);
    assert_eq!(sys_module::LANGUAGE_VERSION, REFERENCE_VERSION_INFO);
    let version_info = attribute(&instance, namespace, "version_info");
    // SAFETY: 建命名空间时放进去的是五元组。
    let tuple = unsafe { &*version_info.as_ptr().cast::<TupleObject>() };
    assert_eq!(tuple.len(), 5);
    assert_eq!(int_of(tuple.item(0).expect("有元素")), i64::from(REFERENCE_VERSION_INFO.0));
    assert_eq!(int_of(tuple.item(1).expect("有元素")), i64::from(REFERENCE_VERSION_INFO.1));
    assert_eq!(int_of(tuple.item(2).expect("有元素")), i64::from(REFERENCE_VERSION_INFO.2));
    assert_eq!(
        text_of(tuple.item(3).expect("有元素")),
        REFERENCE_VERSION_INFO.3
    );
    assert_eq!(int_of(tuple.item(4).expect("有元素")), i64::from(REFERENCE_VERSION_INFO.4));

    // `hexversion` 与参照**逐位相同**（同一种编码）
    let hexversion = attribute(&instance, namespace, "hexversion");
    assert_eq!(int_of(hexversion), REFERENCE_HEXVERSION);

    // 构建串必须含 `pyawa`，且与参照的构建串不同（不得伪装）
    let version = attribute(&instance, namespace, "version");
    let text = text_of(version);
    assert!(text.contains("pyawa"), "构建串必须含 pyawa：{text}");
    assert_ne!(text, REFERENCE_VERSION, "不得伪装成参照的构建串");
}

#[test]
fn implementation_free_constants_match_the_reference() {
    let instance = Instance::new();
    let namespace = sys_module::build(&instance);
    assert_eq!(int_of(attribute(&instance, namespace, "maxunicode")), REFERENCE_MAXUNICODE);
    assert_eq!(int_of(attribute(&instance, namespace, "maxsize")), REFERENCE_MAXSIZE);
    assert_eq!(
        text_of(attribute(&instance, namespace, "byteorder")),
        REFERENCE_BYTEORDER,
        "字节序与宿主同源（不硬编码 little）"
    );
}

#[test]
fn the_capability_free_containers_have_the_documented_shape() {
    // `argv` 默认 `[""]`；`path` 由 `site.py` 建（本层先给空表）；`modules` 是空表
    let instance = Instance::new();
    let namespace = sys_module::build(&instance);
    let argv = attribute(&instance, namespace, "argv");
    // SAFETY: 建命名空间时放进去的是列表。
    let argv = unsafe { &*argv.as_ptr().cast::<pyawa_core::ListObject>() };
    assert_eq!(argv.len(), 1, "嵌入式默认只有一个空串");
    let only = argv.item(0).expect("有元素");
    assert_eq!(text_of(only), "");

    let path = attribute(&instance, namespace, "path");
    // SAFETY: 同上。
    let path = unsafe { &*path.as_ptr().cast::<pyawa_core::ListObject>() };
    assert!(path.is_empty(), "`site.py` 还没接 ⇒ 空表（IM-24）");

    let modules = attribute(&instance, namespace, "modules");
    // SAFETY: 同上（空字典）。
    let modules = unsafe { &*modules.as_ptr().cast::<pyawa_core::DictObject>() };
    assert!(modules.entries().is_empty(), "import 未接 ⇒ 空表");

    assert_eq!(text_of(attribute(&instance, namespace, "__name__")), "sys");
}

#[test]
fn getrefcount_returns_the_real_count_plus_one() {
    // `OM-22`：`sys.getrefcount(obj)` 返回**真实计数加一**（借用参数的那一份）。
    // `§13-5` 决定不实现 immortal ⇒ 具体数字是本实现的真实计数，**不进**严格对照（`MS-18`）。
    let instance = Instance::new();
    let namespace = sys_module::build(&instance);
    let function = attribute(&instance, namespace, "getrefcount");
    // SAFETY: 上面刚放进的是原生可调用对象。
    let handler = unsafe {
        (*function.as_ptr().cast::<pyawa_core::BuiltinFunctionObject>()).function()
    };
    let value = instance.new_int(7);
    let before = instance.refcount_of(value);
    // SAFETY: 实参是本测试持有的引用（handler 只借用）。
    let result = unsafe { handler(&instance, None, &[value], &[]) }.expect("一个实参应当成功");
    assert_eq!(
        int_of(result),
        i64::from(before) + 1,
        "OM-22：返回计数加一"
    );

    // 消息逐条实测：0 个实参、带关键字（关键字先判）
    // SAFETY: 同上。
    let error = unsafe { handler(&instance, None, &[], &[]) }.expect_err("0 个实参要报错");
    assert_eq!(
        message_of(&instance, error),
        "sys.getrefcount() takes exactly one argument (0 given)"
    );
    let keyword = instance.new_str("x");
    // SAFETY: 同上（关键字对只借用）。
    let error = unsafe { handler(&instance, None, &[], &[(keyword, value)]) }
        .expect_err("关键字要报错");
    assert_eq!(
        message_of(&instance, error),
        "sys.getrefcount() takes no keyword arguments"
    );
}

/// 取异常消息（`ExecError::Raised` 的 payload）。
fn message_of(instance: &Instance, error: pyawa_core::ExecError) -> String {
    match error {
        pyawa_core::ExecError::Raised { exception } => {
            // SAFETY: exception 是存活对象。
            unsafe { &*exception.as_ptr().cast::<pyawa_core::ExceptionObject>() }
                .message_with(instance)
                .unwrap_or_default()
        }
        other => panic!("应当是脚本异常，实际 {other:?}"),
    }
}

/// 取 `sys` 里那个原生函数的**处理函数**（与本文件 `getrefcount` 那条同款）。
fn native(instance: &Instance, namespace: NonNull<Header>, name: &str) -> pyawa_core::NativeFn {
    let function = attribute(instance, namespace, name);
    // SAFETY: `sys` 里放进去的都是原生可调用对象。
    unsafe { (*function.as_ptr().cast::<pyawa_core::BuiltinFunctionObject>()).function() }
}

/// 脚本异常的 `"类名: 消息"`（夹具里的消息就是这个形状）。
fn error_text(instance: &Instance, error: pyawa_core::ExecError) -> String {
    match error {
        pyawa_core::ExecError::Raised { exception } => {
            // SAFETY: exception 是存活对象。
            let ty = unsafe { exception.as_ref() }.ty();
            let name = unsafe { ty.as_ref() }.name().to_owned();
            // SAFETY: 同上。
            let message = unsafe { &*exception.as_ptr().cast::<pyawa_core::ExceptionObject>() }
                .message_with(instance)
                .unwrap_or_default();
            format!("{name}: {message}")
        }
        other => panic!("应当是脚本异常，实际 {other:?}"),
    }
}

#[test]
fn the_integer_string_limit_entry_points_match_the_probe() {
    // `TS-45` ①：`sys.get_int_max_str_digits`／`set_int_max_str_digits`
    // 期望值来自 `tools/gen_sys_fixture.py`（参照实测；转换本身的消息在整型夹具那边）。
    let instance = Instance::new();
    let namespace = sys_module::build(&instance);
    let get_limit = native(&instance, namespace, "get_int_max_str_digits");
    let set_limit = native(&instance, namespace, "set_int_max_str_digits");

    // 默认值
    // SAFETY: 实参与返回都按原生函数契约给。
    let default = unsafe { get_limit(&instance, None, &[], &[]) }.expect("get_ 应当成功");
    assert_eq!(int_of(default), REFERENCE_INT_MAX_STR_DIGITS);

    // 设 1000 ⇒ 读回 1000；`set_` 返回 `None`
    let thousand = instance.new_int(1000);
    // SAFETY: 同上。
    let returned = unsafe { set_limit(&instance, None, &[thousand], &[]) }.expect("set_ 应当成功");
    assert_eq!(returned, instance.singletons().none(), "`set_` 返回 `None`");
    // SAFETY: 同上。
    let read = unsafe { get_limit(&instance, None, &[], &[]) }.expect("get_ 应当成功");
    assert_eq!(int_of(read), 1000);

    // `0` ＝ 不限
    let zero = instance.new_int(0);
    assert!(REFERENCE_SETTING_ZERO_SUCCEEDS, "参照实测 `set_(0)` 成功");
    // SAFETY: 同上。
    unsafe { set_limit(&instance, None, &[zero], &[]) }.expect("`set_(0)` 应当成功");
    assert!(REFERENCE_ZERO_MEANS_UNLIMITED, "参照实测 `0` 即不限");
    // SAFETY: 同上。
    let read = unsafe { get_limit(&instance, None, &[], &[]) }.expect("get_ 应当成功");
    assert_eq!(int_of(read), 0);

    // 五种非法形态 ＋ 参数个数：消息与参照**逐字**一致
    let below = instance.new_int(REFERENCE_STR_DIGITS_THRESHOLD - 1);
    let negative = instance.new_int(-1);
    let not_integer = instance.new_str("x");
    let huge = instance.new_int(1 << 40);
    let two_a = instance.new_int(1000);
    let two_b = instance.new_int(2000);
    let one = instance.new_int(1);
    for (args, expected) in [
        (vec![below], REFERENCE_SET_BELOW_MESSAGE),
        (vec![negative], REFERENCE_SET_NEGATIVE_MESSAGE),
        (vec![not_integer], REFERENCE_SET_NOT_INTEGER_MESSAGE),
        (vec![huge], REFERENCE_SET_HUGE_MESSAGE),
        (Vec::new(), REFERENCE_SET_NO_ARGS_MESSAGE),
        (vec![two_a, two_b], REFERENCE_SET_TWO_ARGS_MESSAGE),
    ] {
        // SAFETY: 同上。
        let error = unsafe { set_limit(&instance, None, &args, &[]) }.expect_err("应当报错");
        assert_eq!(
            error_text(&instance, error),
            expected.expect("夹具里必须有这条消息")
        );
    }
    // SAFETY: 同上。
    let error = unsafe { get_limit(&instance, None, &[one], &[]) }.expect_err("`get_` 不收实参");
    assert_eq!(
        error_text(&instance, error),
        REFERENCE_GET_WITH_ARGS_MESSAGE.expect("夹具里必须有这条消息")
    );
}
