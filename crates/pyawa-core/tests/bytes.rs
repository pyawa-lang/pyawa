//! **`bytes` 类型面**（`P1-12`／`TS-42` 的"M2 之后、M3 之前"档）· 参照对拍。
//!
//! 期望值全部来自 `tests/fixture-bytes-3.14.json`（`tools/gen_bytes_fixture.py` 在参照实现上
//! **现场实测**导出）——`repr` 的引号／转义规则、构造的每条消息、索引、比较、长度都在里面。
//!
//! 第一刀接的是：载荷 ＋ 类型对象 ＋ 构造（空／整数计数／`bytes`／整数可迭代／`str`+UTF-8）
//! ＋ `repr`／`str` ＋ `len` ＋ 索引（整数）＋ 迭代 ＋ 等值比较。
//! **仍未接线**：切片（要 `slice` 类型，M3+）、`bytes` 的哈希（`hash()` 本身还没接线——
//! 实测口径记在夹具里：`hash(bytes)` 与同内容 ASCII `str` **相同**）、方法面（`hex`／`decode`
//! 一族，按 oracle 逐批）。

mod common;

use core::ptr::NonNull;

use pyawa_core::executor::{advance, compare_public, subscript_read, values_equal_public};
use pyawa_core::Header;

use common::Vm;

fn fixture() -> common::Json {
    common::parse(include_str!("fixture-bytes-3.14.json"))
}

fn from_hex(text: &str) -> Vec<u8> {
    if text == "-" {
        return Vec::new();
    }
    (0..text.len())
        .step_by(2)
        .map(|index| u8::from_str_radix(&text[index..index + 2], 16).expect("夹具里的十六进制"))
        .collect()
}

fn to_hex(value: &[u8]) -> String {
    value.iter().map(|byte| format!("{byte:02x}")).collect()
}

/// 造一个 `bytes` 对象（走 `bytes(...)` 的**类型调用**，即真实构造路径）。
fn bytes_object(vm: &Vm, args: &[Option<NonNull<Header>>]) -> Result<NonNull<Header>, String> {
    let callable = vm
        .instance
        .type_value(vm.instance.type_named("bytes").expect("bytes 在注册表里"));
    let owned: Vec<NonNull<Header>> = args.iter().map(|arg| arg.expect("实参都在")).collect();
    match pyawa_core::call_value(&vm.instance, callable, &owned, &[]) {
        Ok(value) => Ok(value),
        Err(error) => Err(error_text(&vm.instance, error)),
    }
}

fn error_text(instance: &pyawa_core::Instance, error: pyawa_core::ExecError) -> String {
    match error {
        pyawa_core::ExecError::Raised { exception } => {
            // SAFETY: 异常对象由实例保活。
            let ty = unsafe { exception.as_ref() }.ty();
            let name = unsafe { ty.as_ref() }.name().to_owned();
            // SAFETY: 同上。
            let message = unsafe { &*exception.as_ptr().cast::<pyawa_core::ExceptionObject>() }
                .message_with(instance)
                .unwrap_or_default();
            format!("{name}: {message}")
        }
        other => panic!("应当是脚本异常，得到 {other:?}"),
    }
}

fn repr_of(vm: &Vm, object: NonNull<Header>) -> String {
    vm.instance.object_repr(object).expect("bytes 的 repr")
}

fn payload(vm: &Vm, object: NonNull<Header>) -> Vec<u8> {
    vm.instance
        .bytes_value(object)
        .expect("应当是 bytes")
        .to_vec()
}

#[test]
fn repr_and_str_match_the_reference() {
    let vm = Vm::new();
    for row in fixture().key("repr").as_arr() {
        let object = bytes_from_hex(&vm, row.key("hex").as_str());
        assert_eq!(repr_of(&vm, object), row.key("repr").as_str(), "repr 与参照不一致");
        assert_eq!(
            vm.instance.object_str(object).expect("str"),
            row.key("str").as_str(),
            "str 与参照不一致"
        );
        assert_eq!(
            vm.instance.length_of(object),
            Some(payload(&vm, object).len()),
            "len 与载荷不一致"
        );
    }
}

/// 用给定字节造 `bytes` 对象（`bytes(<整数可迭代>)` 那条真路径）。
fn bytes_from_hex(vm: &Vm, hex: &str) -> NonNull<Header> {
    let numbers: Vec<NonNull<Header>> = from_hex(hex)
        .into_iter()
        .map(|byte| vm.instance.new_int(i64::from(byte)))
        .collect();
    let list = vm.instance.new_list(numbers);
    bytes_object(vm, &[Some(list)]).expect("bytes(<整数列表>) 应当成功")
}

fn int_object(vm: &Vm, value: i64) -> NonNull<Header> {
    vm.instance.new_int(value)
}

#[test]
fn length_matches_the_reference() {
    let vm = Vm::new();
    for row in fixture().key("length").as_arr() {
        let object = bytes_from_hex(&vm, row.key("hex").as_str());
        assert_eq!(
            vm.instance.length_of(object),
            Some(row.key("len").as_i64() as usize),
            "len 与参照不一致"
        );
    }
}

#[test]
fn construction_matches_the_reference() {
    let vm = Vm::new();
    let fixture = fixture();
    for row in fixture.key("construct").as_arr() {
        let call = row.key("call").as_str();
        let object = match call {
            "bytes()" => bytes_object(&vm, &[]).expect("bytes()"),
            "bytes(0)" => bytes_object(&vm, &[Some(int_object(&vm, 0))]).expect("bytes(0)"),
            "bytes(3)" => bytes_object(&vm, &[Some(int_object(&vm, 3))]).expect("bytes(3)"),
            "bytes(256)" => bytes_object(&vm, &[Some(int_object(&vm, 256))]).expect("bytes(256)"),
            "bytes(b'ab')" => {
                let inner = bytes_from_hex(&vm, "6162");
                bytes_object(&vm, &[Some(inner)]).expect("bytes(bytes)")
            }
            "bytes([0, 1, 255])" => {
                let numbers: Vec<NonNull<Header>> =
                    [0i64, 1, 255].into_iter().map(|byte| int_object(&vm, byte)).collect();
                let list = vm.instance.new_list(numbers);
                bytes_object(&vm, &[Some(list)]).expect("bytes(list)")
            }
            "bytes('abc', 'utf-8')" => {
                let text = vm.instance.new_str("abc");
                let encoding = vm.instance.new_str("utf-8");
                bytes_object(&vm, &[Some(text), Some(encoding)]).expect("bytes(str, utf-8)")
            }
            "bytes('é', 'utf-8')" => {
                let text = vm.instance.new_str("é");
                let encoding = vm.instance.new_str("utf-8");
                bytes_object(&vm, &[Some(text), Some(encoding)]).expect("bytes(str, utf-8)")
            }
            "bytes((1, 2, 3))" => {
                let numbers: Vec<NonNull<Header>> =
                    [1i64, 2, 3].into_iter().map(|byte| int_object(&vm, byte)).collect();
                let tuple = vm.instance.new_tuple(numbers);
                bytes_object(&vm, &[Some(tuple)]).expect("bytes(tuple)")
            }
            other => panic!("夹具里有没见过的构造写法：{other}"),
        };
        assert_eq!(to_hex(&payload(&vm, object)), row.key("hex").as_str(), "{call}");
        // SAFETY: object 由本测试持有，归还这一份。
        unsafe { vm.instance.release_object(object.as_ptr()) };
    }
}

#[test]
fn construction_failures_match_the_reference() {
    let vm = Vm::new();
    for row in fixture().key("construct_errors").as_arr() {
        let call = row.key("call").as_str();
        let outcome = match call {
            "bytes(-1)" => bytes_object(&vm, &[Some(int_object(&vm, -1))]),
            "bytes([256])" => {
                let list = vm.instance.new_list(vec![int_object(&vm, 256)]);
                bytes_object(&vm, &[Some(list)])
            }
            "bytes(['a'])" => {
                let list = vm.instance.new_list(vec![vm.instance.new_str("a")]);
                bytes_object(&vm, &[Some(list)])
            }
            "bytes('abc')" => {
                let text = vm.instance.new_str("abc");
                bytes_object(&vm, &[Some(text)])
            }
            "bytes(1.5)" => {
                let number = vm.instance.alloc(pyawa_core::FloatObject::new(
                    vm.instance.type_named("float").expect("float"),
                    1.5,
                ));
                bytes_object(&vm, &[Some(number.into_raw().cast::<Header>())])
            }
            "bytes('abc', 'nope')" => {
                let text = vm.instance.new_str("abc");
                let encoding = vm.instance.new_str("nope");
                bytes_object(&vm, &[Some(text), Some(encoding)])
            }
            other => panic!("夹具里有没见过的构造写法：{other}"),
        };
        let observed = outcome.expect_err("这条应当失败");
        assert_eq!(observed, row.key("error").as_str(), "{call} 的报错与参照不一致");
    }
}

#[test]
fn indexing_matches_the_reference() {
    let vm = Vm::new();
    let fixture = fixture();
    for row in fixture.key("index").as_arr() {
        let container = bytes_from_hex(&vm, "616263");
        let key = int_object(&vm, index_of(row.key("expr").as_str()));
        let result = subscript_read(&vm.instance, container, key).expect("整数下标应当成功");
        assert_eq!(
            vm.instance.int_value(result),
            Some(row.key("int").as_i64()),
            "{} 与参照不一致",
            row.key("expr").as_str()
        );
    }
    // 越界：实测消息是 `IndexError: index out of range`
    for row in fixture.key("index_errors").as_arr() {
        let container = bytes_from_hex(&vm, "616263");
        let key = int_object(&vm, index_of(row.key("expr").as_str()));
        let error = subscript_read(&vm.instance, container, key).expect_err("越界应当报错");
        assert_eq!(
            error_text(&vm.instance, error),
            row.key("error").as_str(),
            "{} 的报错",
            row.key("expr").as_str()
        );
    }
}

/// 从 `b'abc'[0]` 这样的表达式里取那个下标数字。
fn index_of(expr: &str) -> i64 {
    let start = expr.find('[').expect("表达式里有 [") + 1;
    let end = expr.find(']').expect("表达式里有 ]");
    expr[start..end].parse().expect("下标是整数")
}

#[test]
fn comparison_matches_the_reference() {
    let vm = Vm::new();
    for row in fixture().key("compare").as_arr() {
        let left = bytes_from_hex(&vm, row.key("a").as_str());
        let right = bytes_from_hex(&vm, row.key("b").as_str());
        for (symbol, field) in [("<", "lt"), ("<=", "le"), (">", "gt"), (">=", "ge")] {
            let observed = compare_public(&vm.instance, left, right, symbol, 0)
                .unwrap_or_else(|error| panic!("{symbol} 应当可比：{error:?}"));
            assert_eq!(observed, row.key(field).as_bool(), "{symbol} 与参照不一致");
        }
        let equal = values_equal_public(&vm.instance, left, right);
        assert_eq!(equal, row.key("eq").as_bool(), "== 与参照不一致");
        assert_eq!(!equal, row.key("ne").as_bool(), "!= 与参照不一致");
    }
}

#[test]
fn iteration_yields_integers_like_the_reference() {
    let vm = Vm::new();
    for row in fixture().key("iteration").as_arr() {
        let object = bytes_from_hex(&vm, row.key("hex").as_str());
        // `bytes_iterator` 的名字照探测表；这里走**执行器的公开入口**：先按类型造迭代器
        let iterator = vm
            .instance
            .alloc(pyawa_core::IteratorObject::new(
                vm.instance
                    .type_named("bytes_iterator")
                    .expect("bytes_iterator 在注册表里"),
                object,
                core::cell::Cell::new(0),
            ))
            .into_raw()
            .cast::<Header>();
        let expected: Vec<i64> = row.key("items").as_arr().iter().map(|item| item.as_i64()).collect();
        let mut observed = Vec::new();
        while let Some(item) = advance(&vm.instance, iterator).expect("迭代应当成功") {
            observed.push(vm.instance.int_value(item).expect("迭代给整数"));
        }
        assert_eq!(observed, expected, "迭代结果与参照不一致");
        assert!(row.key("contains_97").as_bool() || !observed.contains(&97));
    }
}

#[test]
fn the_hash_rule_is_recorded_for_the_hash_builtin() {
    // `hash()` 本身还没接线；`TS-45` 那边同样把核心与调用点分开。这里只钉住**实测规则**：
    // 同内容的 ASCII `str` 与 `bytes` 哈希相同（将来接线时别写成两套）。
    let fixture = fixture();
    assert!(
        fixture.key("hash_equals_ascii_str").as_bool(),
        "参照实测：hash(bytes) == hash(同内容 ASCII str)"
    );
    assert!(fixture.key("hash").as_arr().len() >= 4);
}

// --------------------------------------------------------------------------- #
// 字面量（`P1-12`：编译器 `b'…'` → 常量池 → `BytesObject`）
// --------------------------------------------------------------------------- #

/// 编译并执行一段脚本，成功时取命名空间里那个全局。
fn run_source(vm: &Vm, source: &str, name: &str) -> Result<NonNull<Header>, String> {
    let unit = pyawa_core::compile::compile(
        source,
        "<t>",
        pyawa_core::compile::Mode::PurePython,
        pyawa_core::compile::CheckTier::Shallow,
        0,
    )
    // 编译错误取**载荷文本**（`CompileError` 没有 `Display`；`{:?}` 会把反斜杠转义掉，
    // 与夹具里的实测文本对不上）
    .map_err(|error| match error {
        pyawa_core::compile::CompileError::Syntax(text)
        | pyawa_core::compile::CompileError::Unsupported(text) => text,
    })?;
    let code = pyawa_core::compile::instantiate(&vm.instance, &unit);
    let namespace = vm.instance.new_dict();
    let module_name = vm.instance.new_str("__main__");
    vm.instance.dict_set(namespace, "__name__", module_name);
    // SAFETY: namespace 由本函数持有，帧接手一份引用。
    unsafe { vm.instance.incref_object(namespace.as_ptr()) };
    let frame = pyawa_core::Frame::for_code_with_namespace(vm.frame_type, &code, namespace);
    let frame = vm.instance.alloc(frame);
    match pyawa_core::execute(&vm.instance, &frame) {
        Ok(_) => Ok(vm
            .instance
            .dict_get(namespace, name)
            .unwrap_or_else(|| panic!("命名空间里应当有 {name}"))),
        Err(pyawa_core::ExecError::Raised { exception }) => Err(error_text(
            &vm.instance,
            pyawa_core::ExecError::Raised { exception },
        )),
        Err(other) => Err(format!("{other:?}")),
    }
}

#[test]
fn literals_compile_and_run_like_the_reference() {
    let vm = Vm::new();
    for row in fixture().key("literal").as_arr() {
        let source = format!("x = {}", row.key("source").as_str());
        let value = run_source(&vm, &source, "x")
            .unwrap_or_else(|error| panic!("{source} 应当跑得通：{error}"));
        assert_eq!(
            to_hex(&payload(&vm, value)),
            row.key("hex").as_str(),
            "{source} 的值与参照不一致"
        );
        assert_eq!(repr_of(&vm, value), row.key("repr").as_str(), "{source} 的 repr");
    }
}

#[test]
fn literal_errors_are_reported_like_the_reference() {
    // 参照那条消息还带 `(<string>, line 1)`；本层的编译错误包装不同 ⇒ 比**核心句**。
    for row in fixture().key("literal_errors").as_arr() {
        let source = format!("x = {}", row.key("source").as_str());
        let error = run_source(&Vm::new(), &source, "x").expect_err("这条应当编不过");
        // 参照那条是 `SyntaxError: <核心句> (<string>, line 1)`；本层的编译错误是
        // `Syntax(<核心句>)`（`CompileError` 的 Display 由 CLI 层再包装）⇒ 比**核心句**
        let expected = row.key("error").as_str();
        let expected = expected.strip_prefix("SyntaxError: ").unwrap_or(expected);
        let expected = expected.split(" (<string>").next().expect("夹具里有消息");
        assert!(
            error.contains(expected),
            "{source} 的报错要含 {expected:?}，实际：{error}"
        );
    }
}

#[test]
fn concatenation_matches_the_reference() {
    // 实测 `b'ab' + b'cd' == b'abcd'`（`concat_public` 里那条 `bytes` 分支）
    let vm = Vm::new();
    let left = bytes_from_hex(&vm, "6162");
    let right = bytes_from_hex(&vm, "6364");
    let joined = pyawa_core::executor::concat_public(&vm.instance, left, right, 0)
        .expect("bytes 相加应当成功");
    assert_eq!(to_hex(&payload(&vm, joined)), "61626364");
    // 端到端：`x = b'ab' + b'cd'` 走编译器 → 常量折叠 → 执行
    let value = run_source(&vm, "x = b'ab' + b'cd'", "x").expect("应当跑得通");
    assert_eq!(to_hex(&payload(&vm, value)), "61626364");
}

// --------------------------------------------------------------------------- #
// 方法面（`P1-12`；按 oracle 逐批，结果一律用 `repr` 对拍）
// --------------------------------------------------------------------------- #

/// 按夹具里的 `kind` 造一个实参。
fn method_argument(vm: &Vm, spec: &common::Json) -> Option<NonNull<Header>> {
    match spec.key("kind").as_str() {
        "bytes" => Some(bytes_from_hex(vm, spec.key("value").as_str())),
        "str" => Some(vm.instance.new_str(spec.key("value").as_str())),
        "int" => Some(int_object(vm, spec.key("value").as_i64())),
        "int_list" => {
            let items: Vec<NonNull<Header>> = spec
                .key("value")
                .as_arr()
                .iter()
                .map(|item| int_object(vm, item.as_i64()))
                .collect();
            Some(vm.instance.new_list(items))
        }
        "bytes_list" => {
            let items: Vec<NonNull<Header>> = spec
                .key("value")
                .as_arr()
                .iter()
                .map(|item| bytes_from_hex(vm, item.as_str()))
                .collect();
            Some(vm.instance.new_list(items))
        }
        other => panic!("夹具里有没见过的实参形态：{other}"),
    }
}

#[test]
fn methods_match_the_reference() {
    let vm = Vm::new();
    let mut checked = 0;
    for row in fixture().key("methods").as_arr() {
        let receiver = bytes_from_hex(&vm, row.key("receiver").as_str());
        let name = row.key("method").as_str();
        let method = pyawa_core::executor::attribute_read(&vm.instance, receiver, name)
            .unwrap_or_else(|error| panic!("{name} 应当取得到：{error:?}"));
        let args: Vec<NonNull<Header>> = row
            .key("args")
            .as_arr()
            .iter()
            .map(|spec| method_argument(&vm, spec).expect("实参都得造得出来"))
            .collect();
        let outcome = pyawa_core::call_value(&vm.instance, method, &args, &[]);
        let label = format!("b'…'.{name}()");
        match (outcome, row.get("repr"), row.get("error")) {
            (Ok(result), Some(common::Json::Str(expected)), _) => {
                assert_eq!(&repr_of(&vm, result), expected, "{label} 与参照不一致")
            }
            (Err(error), _, Some(common::Json::Str(expected))) => {
                assert_eq!(&error_text(&vm.instance, error), expected, "{label} 的报错")
            }
            (other, _, _) => panic!("{label} 与夹具对不上：{other:?}"),
        }
        // SAFETY: method 是新引用，本测试持有。
        unsafe { vm.instance.release_object(method.as_ptr()) };
        checked += 1;
    }
    assert!(checked >= 16, "方法夹具条目太少（{checked}）");
}
