//! 容器与解包族的可测性质（`docs/SPEC-bytecode.md` §10 的"容器与解包"，`BC-49`）。
//!
//! 几处 oparg 约定是**实测**出来的，写在 `executor.rs` 的对应分支上：
//! `PEEK(n)` 是**含指令自身操作数**数的：`LIST_APPEND`／`SET_ADD` 的容器在 `PEEK(oparg)`、
//! `MAP_ADD` 在 `PEEK(oparg)`（值、键各占一层）、`LIST_EXTEND`／`SET_UPDATE` 在 `PEEK(oparg + 1)`。
//! 写实现时别漏掉这一点——第一版就因为在"弹出之后"用同一个下标而差了一层。

mod common;

use pyawa_core::{
    DictObject, Instance, ListObject, SetObject, StrObject, TupleObject, Value,
};

use common::{emit, op, Vm};

fn header_of(value: &Value<'_>, instance: &Instance) -> core::ptr::NonNull<pyawa_core::Header> {
    value
        .as_header(instance)
        .expect("容器与字符串都应当能落到具体对象上")
}

fn assert_type(instance: &Instance, value: &Value<'_>, name: &str) {
    let raw = header_of(value, instance);
    // SAFETY: raw 是存活对象。
    let ty = unsafe { raw.as_ref() }.ty();
    assert_eq!(ty, instance.type_named(name).unwrap(), "类型应当是 {name}");
}

/// # Safety
///
/// 调用方必须先用 [`assert_type`] 确认这就是对应类型（`T` 要与它一致）。
unsafe fn payload<'a, T>(value: &Value<'_>, instance: &'a Instance) -> &'a T {
    let raw = header_of(value, instance);
    // SAFETY: 由调用方保证类型正确。
    unsafe { &*raw.as_ptr().cast::<T>() }
}

#[test]
fn builds_tuple_list_set_and_map() {
    let vm = Vm::new();

    let cases: Vec<(&str, u8)> = vec![
        ("tuple", op("BUILD_TUPLE")),
        ("list", op("BUILD_LIST")),
        ("set", op("BUILD_SET")),
    ];
    for (name, instruction) in cases {
        // 注意：常量表**持有**这些引用（OM-40），所以每个 code object 必须有自己的常量对象
        let consts = vec![Some(vm.constant(1)), Some(vm.constant(2))];
        let code = vm.code(
            4,
            0,
            emit(&[
                (op("LOAD_CONST"), 0),
                (op("LOAD_CONST"), 1),
                (instruction, 2),
                (op("RETURN_VALUE"), 0),
            ]),
            consts,
        );
        let result = vm.run(&code).unwrap();
        assert_type(&vm.instance, &result, name);
        match name {
            "tuple" => {
                // SAFETY: 上面刚确认了类型。
                let object = unsafe { payload::<TupleObject>(&result, &vm.instance) };
                assert_eq!(object.len(), 2);
                assert!(object.item(0).is_some() && object.item(1).is_some());
                assert!(object.item(2).is_none());
            }
            "list" => {
                // SAFETY: 同上。
                let object = unsafe { payload::<ListObject>(&result, &vm.instance) };
                assert_eq!(object.len(), 2);
            }
            _ => {
                // SAFETY: 同上。
                let object = unsafe { payload::<SetObject>(&result, &vm.instance) };
                assert_eq!(object.len(), 2);
            }
        }
    }

    // dict：压栈顺序是 key1 value1 key2 value2
    let keys = vec![
        Some(vm.constant(1)),
        Some(vm.constant(2)),
        Some(vm.constant(10)),
        Some(vm.constant(20)),
    ];
    let code = vm.code(
        8,
        0,
        emit(&[
            (op("LOAD_CONST"), 0),
            (op("LOAD_CONST"), 2),
            (op("LOAD_CONST"), 1),
            (op("LOAD_CONST"), 3),
            (op("BUILD_MAP"), 2),
            (op("RETURN_VALUE"), 0),
        ]),
        keys,
    );
    let result = vm.run(&code).unwrap();
    assert_type(&vm.instance, &result, "dict");
    // SAFETY: 上面刚确认了类型。
    let object = unsafe { payload::<DictObject>(&result, &vm.instance) };
    assert_eq!(object.len(), 2);
    assert_eq!(
        object.entry(0).map(|(key, _)| key),
        code.get().constant(0),
        "先压入的是键"
    );
    assert_eq!(
        object.entry(0).map(|(_, value)| value),
        code.get().constant(2),
        "紧跟键的是它的值"
    );
}

#[test]
fn set_and_dict_deduplicate_by_value() {
    // TS-40 的可观察后果：`True == 1` ⇒ `{1, True}` 只有一个元素、`{1: 'a', True: 'b'}` 只有一个键
    let vm = Vm::new();
    let truth = |vm: &Vm| {
        vm.instance
            .own(vm.instance.singletons().boolean(true))
            .into_raw()
    };

    // 每段程序各有自己的常量对象（常量表持有引用）
    let set_consts = vec![Some(vm.constant(1)), Some(truth(&vm))];
    let code = vm.code(
        4,
        0,
        emit(&[
            (op("LOAD_CONST"), 0),
            (op("LOAD_CONST"), 1),
            (op("BUILD_SET"), 2),
            (op("RETURN_VALUE"), 0),
        ]),
        set_consts,
    );
    let result = vm.run(&code).unwrap();
    // SAFETY: 这条码元只造 set。
    let set = unsafe { payload::<SetObject>(&result, &vm.instance) };
    assert_eq!(set.len(), 1, "1 与 True 是同一个元素");
    assert_eq!(
        set.item(0),
        code.get().constant(0),
        "保留先出现的那个对象（这里是常量表里的 1）"
    );

    let dict_consts = vec![
        Some(vm.constant(1)),
        Some(truth(&vm)),
        Some(vm.constant(2)),
        Some(vm.constant(3)),
    ];
    let code = vm.code(
        8,
        0,
        emit(&[
            (op("LOAD_CONST"), 0),
            (op("LOAD_CONST"), 2),
            (op("LOAD_CONST"), 1),
            (op("LOAD_CONST"), 3),
            (op("BUILD_MAP"), 2),
            (op("RETURN_VALUE"), 0),
        ]),
        dict_consts,
    );
    let result = vm.run(&code).unwrap();
    // SAFETY: 这条码元只造 dict。
    let dict = unsafe { payload::<DictObject>(&result, &vm.instance) };
    assert_eq!(dict.len(), 1, "1 与 True 是同一个键");
    assert_eq!(
        dict.entry(0).map(|(_, value)| value),
        code.get().constant(3),
        "后出现的值覆盖先出现的（键保留先出现的那个）"
    );
}

#[test]
fn build_string_concatenates_and_uses_the_empty_singleton() {
    let vm = Vm::new();
    let str_type = vm.instance.singletons().str_type();
    let left = vm.instance.alloc(StrObject::new(str_type, "你".to_owned()));
    let right = vm.instance.alloc(StrObject::new(str_type, "好".to_owned()));
    let consts = vec![
        Some(left.into_raw().cast::<pyawa_core::Header>()),
        Some(right.into_raw().cast::<pyawa_core::Header>()),
    ];

    let code = vm.code(
        4,
        0,
        emit(&[
            (op("LOAD_CONST"), 0),
            (op("LOAD_CONST"), 1),
            (op("BUILD_STRING"), 2),
            (op("RETURN_VALUE"), 0),
        ]),
        consts,
    );
    let result = vm.run(&code).unwrap();
    assert_type(&vm.instance, &result, "str");
    // SAFETY: 上面刚确认了类型。
    let text = unsafe { payload::<StrObject>(&result, &vm.instance) };
    assert_eq!(text.value(), "你好");

    // 空的 BUILD_STRING 走 `OM-23` 的空串单例
    let code = vm.code(2, 0, emit(&[(op("BUILD_STRING"), 0), (op("RETURN_VALUE"), 0)]), Vec::new());
    let result = vm.run(&code).unwrap();
    assert_eq!(
        header_of(&result, &vm.instance),
        vm.instance.singletons().empty_str(),
        "空串必须是那个单例"
    );
}

#[test]
fn unpack_sequence_pushes_right_to_left() {
    let vm = Vm::new();
    let consts = vec![Some(vm.constant(7)), Some(vm.constant(8))];
    // [7, 8] → a, b = …
    let code = vm.code(
        4,
        2,
        emit(&[
            (op("LOAD_CONST"), 0),
            (op("LOAD_CONST"), 1),
            (op("BUILD_LIST"), 2),
            (op("UNPACK_SEQUENCE"), 2),
            (op("STORE_FAST"), 0),
            (op("STORE_FAST"), 1),
            (op("LOAD_FAST"), 0),
            (op("RETURN_VALUE"), 0),
        ]),
        consts,
    );
    let result = vm.run(&code).unwrap();
    assert!(
        result.is_same(&Value::small_int(7), &vm.instance),
        "最左边的目标拿到第一个元素"
    );
}

#[test]
fn unpack_ex_collects_the_middle_into_a_list() {
    let vm = Vm::new();
    let consts = vec![
        Some(vm.constant(1)),
        Some(vm.constant(2)),
        Some(vm.constant(3)),
        Some(vm.constant(4)),
    ];
    // a, *b, c = [1, 2, 3, 4]：oparg ＝ 1 ｜ 1 << 8（实测 257）
    let code = vm.code(
        8,
        3,
        emit(&[
            (op("LOAD_CONST"), 0),
            (op("LOAD_CONST"), 1),
            (op("LOAD_CONST"), 2),
            (op("LOAD_CONST"), 3),
            (op("BUILD_LIST"), 4),
            // oparg ＝ 前者 1 ｜ 后者 1 << 8 ＝ 257，一个字节装不下 ⇒ 用 EXTENDED_ARG 前缀（BC-34）
            (op("EXTENDED_ARG"), 1),
            (op("UNPACK_EX"), 1),
            (op("STORE_FAST"), 0),
            (op("STORE_FAST"), 1),
            (op("STORE_FAST"), 2),
            (op("LOAD_FAST"), 1),
            (op("RETURN_VALUE"), 0),
        ]),
        consts,
    );
    let result = vm.run(&code).unwrap();
    assert_type(&vm.instance, &result, "list");
    // SAFETY: 上面刚确认了类型。
    let middle = unsafe { payload::<ListObject>(&result, &vm.instance) };
    assert_eq!(middle.len(), 2, "中间那段应当是 [2, 3]");
    assert_eq!(middle.item(0), code.get().constant(1));
    assert_eq!(middle.item(1), code.get().constant(2));
}

#[test]
fn unpack_count_mismatch_is_reported() {
    let vm = Vm::new();
    let code = vm.code(
        4,
        3,
        emit(&[
            (op("LOAD_CONST"), 0),
            (op("BUILD_LIST"), 1),
            (op("UNPACK_SEQUENCE"), 3),
            (op("RETURN_VALUE"), 0),
        ]),
        vec![Some(vm.constant(1))],
    );
    assert!(matches!(
        vm.run(&code),
        Err(pyawa_core::ExecError::Raised { .. })
    ));
    assert_eq!(
        vm.pending_exception(),
        Some((
            "ValueError".to_owned(),
            Some("not enough values to unpack (expected 3, got 1)".to_owned())
        ))
    );
}

#[test]
fn append_helpers_use_the_measured_depths() {
    // list：容器在 PEEK(oparg)，值是 TOS
    let vm = Vm::new();
    let code = vm.code(
        4,
        0,
        emit(&[
            (op("BUILD_LIST"), 0),
            (op("LOAD_CONST"), 0),
            // `list.append(TOS[-oparg], TOS)`：值占一层，容器在 TOS[-2] ⇒ oparg ＝ 2
            (op("LIST_APPEND"), 2),
            (op("RETURN_VALUE"), 0),
        ]),
        vec![Some(vm.constant(5))],
    );
    let result = vm.run(&code).unwrap();
    // SAFETY: 这条码元只造 list。
    let list = unsafe { payload::<ListObject>(&result, &vm.instance) };
    assert_eq!(list.len(), 1);
    assert_eq!(list.item(0), code.get().constant(0));

    // set：同 list 的深度约定
    let code = vm.code(
        4,
        0,
        emit(&[
            (op("BUILD_SET"), 0),
            (op("LOAD_CONST"), 0),
            (op("SET_ADD"), 2),
            (op("RETURN_VALUE"), 0),
        ]),
        vec![Some(vm.constant(5))],
    );
    let result = vm.run(&code).unwrap();
    // SAFETY: 这条码元只造 set。
    let set = unsafe { payload::<SetObject>(&result, &vm.instance) };
    assert_eq!(set.len(), 1);

    // dict：MAP_ADD 的容器在 PEEK(oparg + 1)（实测：值 ＝ TOS、键 ＝ TOS1）
    let code = vm.code(
        6,
        0,
        emit(&[
            (op("BUILD_MAP"), 0),
            (op("LOAD_CONST"), 0),
            (op("LOAD_CONST"), 1),
            (op("MAP_ADD"), 2),
            (op("RETURN_VALUE"), 0),
        ]),
        vec![Some(vm.constant(5)), Some(vm.constant(6))],
    );
    let result = vm.run(&code).unwrap();
    // SAFETY: 这条码元只造 dict。
    let dict = unsafe { payload::<DictObject>(&result, &vm.instance) };
    assert_eq!(dict.len(), 1);
    assert_eq!(dict.entry(0).map(|(key, _)| key), code.get().constant(0));
    assert_eq!(dict.entry(0).map(|(_, value)| value), code.get().constant(1));
}

#[test]
fn list_extend_accepts_tuple_and_str_sources() {
    let vm = Vm::new();
    let str_type = vm.instance.singletons().str_type();
    let text = vm.instance.alloc(StrObject::new(str_type, "ab".to_owned()));
    let consts = vec![
        Some(vm.constant(1)),
        Some(vm.constant(2)),
        Some(text.into_raw().cast::<pyawa_core::Header>()),
    ];

    // [*（1, 2）, *"ab"]：LIST_EXTEND 的容器在 PEEK(oparg)
    let code = vm.code(
        6,
        0,
        emit(&[
            (op("BUILD_LIST"), 0),
            (op("LOAD_CONST"), 0),
            (op("LOAD_CONST"), 1),
            (op("BUILD_TUPLE"), 2),
            (op("LIST_EXTEND"), 1),
            (op("LOAD_CONST"), 2),
            (op("LIST_EXTEND"), 1),
            (op("RETURN_VALUE"), 0),
        ]),
        consts,
    );
    let result = vm.run(&code).unwrap();
    // SAFETY: 这条码元只造 list。
    let list = unsafe { payload::<ListObject>(&result, &vm.instance) };
    assert_eq!(list.len(), 4, "2 个整数 ＋ 2 个字符");
    assert_eq!(list.item(0), code.get().constant(0));
    assert_eq!(list.item(1), code.get().constant(1));
}

#[test]
fn containers_hold_references_and_self_cycles_are_collected() {
    // 容器持有引用（OM-40），且自引用的环必须被回收（BC-45／OM-25）
    let vm = Vm::new();

    let code = vm.code(
        4,
        1,
        emit(&[
            (op("BUILD_LIST"), 0),
            (op("STORE_FAST"), 0),
            (op("LOAD_FAST"), 0),
            (op("LOAD_FAST"), 0),
            (op("LIST_APPEND"), 2), // 列表把自己装进自己；栈上剩下的那一份就是要返回的
            (op("DELETE_FAST"), 0), // **必须**清掉局部槽，否则帧还引用着它（就不是垃圾了）
            (op("RETURN_VALUE"), 0),
        ]),
        Vec::new(),
    );
    let frame = vm
        .instance
        .alloc(pyawa_core::Frame::for_code(vm.frame_type, &code));
    // 基线在 code／frame 造好之后取（它们自己也入回收链：常量表与值栈）
    let base_live = vm.instance.live_objects();
    let base_tracked = vm.instance.tracked_objects();
    let result = pyawa_core::execute(&vm.instance, &frame).unwrap();

    // SAFETY: 这条码元造的是 list。
    let list = unsafe { payload::<ListObject>(&result, &vm.instance) };
    assert_eq!(list.len(), 1);
    assert_eq!(
        list.item(0),
        Some(header_of(&result, &vm.instance)),
        "列表里装的就是它自己"
    );
    assert_eq!(
        vm.instance.tracked_objects(),
        base_tracked + 1,
        "OM-12：容器持有引用 ⇒ 入回收链"
    );

    drop(result);
    assert_eq!(
        vm.instance.live_objects(),
        base_live + 1,
        "环靠计数收不掉（列表自己引用自己）"
    );
    assert_eq!(vm.instance.collect(), 1, "OM-25：自引用的环必须被回收");
    assert_eq!(vm.instance.live_objects(), base_live);
}
