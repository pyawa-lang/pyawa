//! **`T-BC-18`**：参数绑定的错误消息与参照实现**逐字一致**（`BC-56`）。
//!
//! 期望值来自 `tests/fixture-argbind-3.14.json`（`tools/gen_argbind_fixture.py` 从参照导出，
//! **禁止手写近似文本**）。两边各用各的方式到达同一情形：参照跑真 Python 函数，我们手搭
//! `co_argcount`／`co_posonlyargcount`／`co_kwonlyargcount`／`co_flags`／`co_varnames`／默认值
//! ——签名元数据本身也照参照实测（`co_varnames` 的顺序是"位置参数 → 仅关键字 → *args → **kwargs"）。

mod common;

use std::ptr::NonNull;

use core::cell::RefCell;

use pyawa_core::{ExecError, FunctionObject, Header};

use common::{op, Vm};

/// 一个函数的签名（与夹具里那条 `def` 一一对应）。
struct Signature {
    argcount: u8,
    posonlyargcount: u8,
    kwonlyargcount: u8,
    varargs: bool,
    varkw: bool,
    varnames: &'static [&'static str],
    /// 位置参数默认值的个数（决定"takes from A to B"里那个 A）。
    defaults: usize,
}

/// 照签名造一个函数对象（**新引用**）。
fn build(vm: &Vm, signature: &Signature) -> NonNull<Header> {
    let flags = u32::from(signature.varargs) * 4 + u32::from(signature.varkw) * 8;
    let code = vm.function_code(
        4,
        signature.varnames.len(),
        signature.argcount as usize,
        signature.posonlyargcount as usize,
        signature.kwonlyargcount as usize,
        flags,
        signature.varnames.iter().map(|name| (*name).to_owned()).collect(),
        common::emit(&[
            // `RESUME` 的实参是**标志**（不是行号），这里给 0
            (op("RESUME"), 0),
            (op("LOAD_CONST"), 0),
            (op("RETURN_VALUE"), 0),
        ]),
        vec![Some(vm.instance.own(vm.instance.singletons().none()).into_raw())],
    );
    let code_header = code.as_ptr().cast::<Header>();
    // SAFETY: code 由本测试持有，函数对象要自己那份引用。
    unsafe { vm.instance.incref_object(code_header.as_ptr()) };
    let defaults: Vec<NonNull<Header>> = (0..signature.defaults)
        .map(|index| vm.instance.new_int(index as i64 + 1))
        .collect();
    let function = vm.instance.alloc(FunctionObject::new(
        vm.instance.type_named("function").expect("function 已登记"),
        code_header,
        defaults,
        None,
    
    RefCell::new(None),
            core::cell::RefCell::new(None)));
    function.into_raw().cast::<Header>()
}

/// 按参数名造关键字实参对。
fn kwargs(vm: &Vm, pairs: &[(&str, i64)]) -> Vec<(NonNull<Header>, NonNull<Header>)> {
    pairs
        .iter()
        .map(|(name, value)| {
            (
                vm.instance.new_str(name).cast::<Header>(),
                vm.instance.new_int(*value).cast::<Header>(),
            )
        })
        .collect()
}

fn ints(vm: &Vm, values: &[i64]) -> Vec<NonNull<Header>> {
    values
        .iter()
        .map(|value| vm.instance.new_int(*value).cast::<Header>())
        .collect()
}

/// 调用并把"成功／消息"写成与夹具同形的值。
fn call_once(
    vm: &Vm,
    function: NonNull<Header>,
    args: Vec<NonNull<Header>>,
    keyword_arguments: Vec<(NonNull<Header>, NonNull<Header>)>,
) -> Option<String> {
    match pyawa_core::call_value(&vm.instance, function, &args, &keyword_arguments) {
        Ok(_) => {
            for argument in args {
                // SAFETY: 本测试持有这些引用。
                unsafe { vm.instance.release_object(argument.as_ptr()) };
            }
            for (key, value) in keyword_arguments {
                // SAFETY: 同上。
                unsafe {
                    vm.instance.release_object(key.as_ptr());
                    vm.instance.release_object(value.as_ptr());
                }
            }
            None
        }
        Err(ExecError::Raised { exception: _ }) => {
            let (_, message) = vm.pending_exception().expect("应当有异常");
            for argument in args {
                // SAFETY: 同上。
                unsafe { vm.instance.release_object(argument.as_ptr()) };
            }
            for (key, value) in keyword_arguments {
                // SAFETY: 同上。
                unsafe {
                    vm.instance.release_object(key.as_ptr());
                    vm.instance.release_object(value.as_ptr());
                }
            }
            Some(message.unwrap_or_default())
        }
        Err(other) => panic!("报了非脚本异常：{other:?}"),
    }
}

/// 夹具里某个用例的期望消息（`None` ＝ 不该报错）。
fn expected(case: &str) -> Option<String> {
    let fixture = common::parse(include_str!("fixture-argbind-3.14.json"));
    let entry = fixture.key("cases").key(case).key("message");
    match entry {
        common::Json::Null => None,
        common::Json::Str(text) => Some(text.clone()),
        other => panic!("夹具里 {case} 的 message 形状不对：{other:?}"),
    }
}

/// 跑一个用例并与夹具比对。
fn check(
    vm: &Vm,
    case: &str,
    signature: Signature,
    args: Vec<NonNull<Header>>,
    keyword_arguments: Vec<(NonNull<Header>, NonNull<Header>)>,
) {
    let function = build(vm, &signature);
    let observed = call_once(vm, function, args, keyword_arguments);
    assert_eq!(observed, expected(case), "用例 {case} 的消息与参照不一致");
    // SAFETY: function 由本测试持有。
    unsafe { vm.instance.release_object(function.as_ptr()) };
}

const TWO_POSITIONAL: Signature = Signature {
    argcount: 2,
    posonlyargcount: 0,
    kwonlyargcount: 0,
    varargs: false,
    varkw: false,
    varnames: &["a", "b"],
    defaults: 0,
};

#[test]
fn bc56_four_kinds_match_the_reference() {
    // `BC-56` 的四类：多余位置实参／缺必填／同一参数重复给／未知关键字
    let vm = Vm::new();
    check(&vm, "missing_two", TWO_POSITIONAL, ints(&vm, &[]), Vec::new());
    check(&vm, "missing_one", TWO_POSITIONAL, ints(&vm, &[1]), Vec::new());
    check(&vm, "too_many", TWO_POSITIONAL, ints(&vm, &[1, 2, 3]), Vec::new());
    check(
        &vm,
        "duplicate",
        TWO_POSITIONAL,
        ints(&vm, &[1]),
        kwargs(&vm, &[("a", 2)]),
    );
    check(
        &vm,
        "unexpected_keyword",
        TWO_POSITIONAL,
        ints(&vm, &[1, 2]),
        kwargs(&vm, &[("x", 3)]),
    );
}

#[test]
fn defaults_kwonly_and_posonly_match_the_reference() {
    let vm = Vm::new();
    // `(a, b=1, *, c=2)`：带默认值时消息用"from 1 to 2"
    check(
        &vm,
        "too_many_with_default",
        Signature {
            argcount: 2,
            posonlyargcount: 0,
            kwonlyargcount: 1,
            varargs: false,
            varkw: false,
            varnames: &["a", "b", "c"],
            defaults: 1,
        },
        ints(&vm, &[1, 2, 3]),
        Vec::new(),
    );
    // `(a, b, *args, c, **kwargs)`：缺仅关键字参数
    check(
        &vm,
        "missing_keyword_only",
        Signature {
            argcount: 2,
            posonlyargcount: 0,
            kwonlyargcount: 1,
            varargs: true,
            varkw: true,
            varnames: &["a", "b", "c", "args", "kwargs"],
            defaults: 0,
        },
        ints(&vm, &[1, 2]),
        Vec::new(),
    );
    // `(a, /, b, *, c)`：仅位置参数被当关键字传
    check(
        &vm,
        "posonly_as_keyword",
        Signature {
            argcount: 2,
            posonlyargcount: 1,
            kwonlyargcount: 1,
            varargs: false,
            varkw: false,
            varnames: &["a", "b", "c"],
            defaults: 0,
        },
        ints(&vm, &[]),
        kwargs(&vm, &[("a", 1), ("b", 2), ("c", 3)]),
    );
    // `(**kwargs)`：一个位置实参都不收
    check(
        &vm,
        "none_positional",
        Signature {
            argcount: 0,
            posonlyargcount: 0,
            kwonlyargcount: 0,
            varargs: false,
            varkw: true,
            varnames: &["kwargs"],
            defaults: 0,
        },
        ints(&vm, &[1]),
        Vec::new(),
    );
}

#[test]
fn star_arguments_absorb_what_they_should() {
    let vm = Vm::new();
    // `(*args, **kwargs)`：什么都收得住 ⇒ 不该报错
    check(
        &vm,
        "varargs_ok",
        Signature {
            argcount: 0,
            posonlyargcount: 0,
            kwonlyargcount: 0,
            varargs: true,
            varkw: true,
            varnames: &["args", "kwargs"],
            defaults: 0,
        },
        ints(&vm, &[1]),
        kwargs(&vm, &[("x", 2)]),
    );
    // `(a, b, *args, c, **kwargs)`：位置与关键字都放得下 ⇒ 不该报错
    check(
        &vm,
        "full_ok",
        Signature {
            argcount: 2,
            posonlyargcount: 0,
            kwonlyargcount: 1,
            varargs: true,
            varkw: true,
            varnames: &["a", "b", "c", "args", "kwargs"],
            defaults: 0,
        },
        ints(&vm, &[1, 2, 3, 4]),
        kwargs(&vm, &[("c", 5), ("d", 6)]),
    );
}

#[test]
fn a_successful_call_returns_the_body_value() {
    // 对照：能绑上的调用要真的跑起来（体里 `return None`）
    let vm = Vm::new();
    let function = build(&vm, &TWO_POSITIONAL);
    let args = ints(&vm, &[1, 2]);
    let result = pyawa_core::call_value(&vm.instance, function, &args, &[]).expect("应当调通");
    assert_eq!(
        vm.instance.type_of(result),
        vm.instance.singletons().none_type(),
        "体里 `return None`（`Value::small_int` 那条断言是笔误）"
    );
    // SAFETY: 本测试持有这些引用。
    unsafe {
        for argument in args {
            vm.instance.release_object(argument.as_ptr());
        }
        vm.instance.release_object(result.as_ptr());
        vm.instance.release_object(function.as_ptr());
    }
}
