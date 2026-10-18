//! **`PEP 604`** 的 `|`（`NB_OR`）**运行期**语义：`X | None`／`None | X`／union 再 `|`。
//!
//! 参照（`python3` 3.14.4）实测口径：
//! * `int | None` ⇒ `types.UnionType`，`__args__ == (int, NoneType)`（`None` 那一侧是**类**）；
//! * `None | int` 顺序相反；`int | str | None`／`(int | str) | None` ⇒ **扁平**三成员；
//! * union 再 `|` 摊平并按成员 `==` **去重**（`(int | int)` ⇒ `int`）；
//! * **`None | None` 仍是 `TypeError`**（`None` 不是类型；`unsupported operand type(s) for |`）。
//!
//! 这里用**手写字节码**直接钉 `NB_OR`（不经 `Lib/`、不依赖 `print`）；union 的 `__args__`
//! 就是编译期／运行期两侧共用的可观察口径。

mod common;

use core::ptr::NonNull;

use pyawa_core::{ExecError, Header, TupleObject, Value};

use common::{assemble, op, Item, Vm};

/// `BINARY_OP` 的 `NB_OR` 下标（`BC-39`：按 `get_nb_ops()` 的顺序取，不写死）。
fn nb_or() -> u8 {
    pyawa_core::opcode::get_nb_ops()
        .iter()
        .position(|(name, _)| *name == "NB_OR")
        .expect("NB_OR 在 get_nb_ops 表里") as u8
}

/// 类型对象当值用（**新引用**，供常量表接手）。
fn type_value(vm: &Vm, name: &str) -> NonNull<Header> {
    let ty = vm.instance.type_named(name).expect("TS-41：类型应当已登记");
    vm.instance.type_value(ty)
}

/// 类型对象的裸指针（比较用；不变式：类型对象在实例存活期内稳定）。
fn type_header(vm: &Vm, name: &str) -> NonNull<Header> {
    vm.instance
        .type_named(name)
        .expect("TS-41：类型应当已登记")
        .cast::<Header>()
}

/// 跑一段表达式字节码：`instructions` 直接接在 `RESUME` 之后，末尾补 `RETURN_VALUE`。
fn run<'a>(
    vm: &'a Vm,
    instructions: &[(u8, u8)],
    consts: Vec<Option<NonNull<Header>>>,
) -> Result<Value<'a>, ExecError> {
    let mut items = vec![Item::Instr(op("RESUME"), 0)];
    for (opcode, argument) in instructions {
        items.push(Item::Instr(*opcode, *argument));
    }
    items.push(Item::Instr(op("RETURN_VALUE"), 0));
    let code = vm.code(8, 0, assemble(&items), consts);
    vm.run(&code)
}

/// union 成员的名字：**类型对象**给类名（`int`），其余给它的类型名。
fn member_name(vm: &Vm, member: NonNull<Header>) -> String {
    if vm.instance.is_type_object(member) {
        vm.instance.type_name(member.cast::<pyawa_core::TypeObject>())
    } else {
        vm.instance.type_name(vm.instance.type_of(member))
    }
}

/// union 实例的 `__args__` 成员表（**借用**指针；顺序即书写顺序）。
fn union_args(vm: &Vm, union: NonNull<Header>) -> Vec<NonNull<Header>> {
    let args = vm
        .instance
        .attribute_optional_of(union, "__args__")
        .expect("读 `__args__` 不应出错")
        .expect("union 必须有 `__args__`");
    assert_eq!(
        vm.instance.type_of(args),
        vm.instance.type_named("tuple").expect("tuple 已登记"),
        "PEP 604：`__args__` 必须是元组"
    );
    // SAFETY: 类型身份刚确认。
    let parts = unsafe { &*args.as_ptr().cast::<TupleObject>() };
    (0..parts.len()).filter_map(|index| parts.item(index)).collect()
}

#[test]
fn int_or_none_puts_the_none_type_class_in_args() {
    // `int | None` ⇒ `(int, NoneType)`：`None` 那一侧要变成**类** `NoneType`，不是 `None` 实例。
    let vm = Vm::new();
    let consts = vec![Some(type_value(&vm, "int")), Some(vm.instance.new_none())];
    let result = run(
        &vm,
        &[
            (op("LOAD_CONST"), 0),
            (op("LOAD_CONST"), 1),
            (op("BINARY_OP"), nb_or()),
        ],
        consts,
    )
    .expect("`int | None` 必须能算出来（PEP 604）");
    let union = result.as_header(&vm.instance).expect("有返回值");
    let args = union_args(&vm, union);
    assert_eq!(args.len(), 2, "两个成员");
    assert_eq!(args[0], type_header(&vm, "int"), "左侧是 int");
    assert_eq!(args[1], type_header(&vm, "NoneType"), "右侧是 NoneType 类");
    assert_eq!(
        vm.instance.type_name(vm.instance.type_of(union)),
        "UnionType",
        "结果是 types.UnionType"
    );
}

#[test]
fn none_on_the_left_and_chained_or_flatten_in_written_order() {
    // `None | int | str` ⇒ **扁平** `(NoneType, int, str)`（顺序＝书写顺序）。
    let vm = Vm::new();
    let consts = vec![
        Some(vm.instance.new_none()),
        Some(type_value(&vm, "int")),
        Some(type_value(&vm, "str")),
    ];
    let result = run(
        &vm,
        &[
            (op("LOAD_CONST"), 0),
            (op("LOAD_CONST"), 1),
            (op("BINARY_OP"), nb_or()),
            (op("LOAD_CONST"), 2),
            (op("BINARY_OP"), nb_or()),
        ],
        consts,
    )
    .expect("`None | int | str` 必须能算出来");
    let union = result.as_header(&vm.instance).expect("有返回值");
    let args = union_args(&vm, union);
    assert_eq!(args.len(), 3, "三个成员（不能是嵌套 union）：{args:?}");
    assert_eq!(args[0], type_header(&vm, "NoneType"));
    assert_eq!(args[1], type_header(&vm, "int"));
    assert_eq!(args[2], type_header(&vm, "str"));
}

#[test]
fn a_union_on_either_side_flattens_to_one_member_list() {
    // `(int | str) | None` ⇒ `(int, str, NoneType)`；`int | (None | str)` ⇒ `(int, NoneType, str)`。
    let vm = Vm::new();
    let int_str_none = run(
        &vm,
        &[
            (op("LOAD_CONST"), 0),
            (op("LOAD_CONST"), 1),
            (op("BINARY_OP"), nb_or()),
            (op("LOAD_CONST"), 2),
            (op("BINARY_OP"), nb_or()),
        ],
        vec![
            Some(type_value(&vm, "int")),
            Some(type_value(&vm, "str")),
            Some(vm.instance.new_none()),
        ],
    )
    .expect("`(int | str) | None` 必须能算出来");
    let args = union_args(&vm, int_str_none.as_header(&vm.instance).expect("有返回值"));
    assert_eq!(
        args.iter().map(|item| member_name(&vm, *item)).collect::<Vec<_>>(),
        vec!["int", "str", "NoneType"],
        "左侧 union 被摊平且顺序保持"
    );

    let none_str = run(
        &vm,
        &[
            (op("LOAD_CONST"), 0),
            (op("LOAD_CONST"), 1),
            (op("BINARY_OP"), nb_or()),
        ],
        vec![Some(vm.instance.new_none()), Some(type_value(&vm, "str"))],
    )
    .expect("`None | str` 必须能算出来");
    let int_none_str = run(
        &vm,
        &[
            (op("LOAD_CONST"), 0),
            (op("LOAD_CONST"), 1),
            (op("BINARY_OP"), nb_or()),
        ],
        vec![
            Some(type_value(&vm, "int")),
            // 常量表**接手**一份引用 ⇒ 交给它自己的那份（`AS_HEADER` 是借用视图）。
            Some(
                vm.instance
                    .retain(none_str.as_header(&vm.instance).expect("有返回值")),
            ),
        ],
    )
    .expect("`int | (None | str)` 必须能算出来");
    let args = union_args(&vm, int_none_str.as_header(&vm.instance).expect("有返回值"));
    assert_eq!(
        args.iter().map(|item| member_name(&vm, *item)).collect::<Vec<_>>(),
        vec!["int", "NoneType", "str"],
        "右侧 union 被摊平：左侧成员在前"
    );
}

#[test]
fn duplicate_members_collapse_to_the_single_type() {
    // 参照：`int | int` ⇒ `int` 本身（不是 union）；`(int | str) | int` ⇒ 仍两成员。
    let vm = Vm::new();
    let same = run(
        &vm,
        &[
            (op("LOAD_CONST"), 0),
            (op("LOAD_CONST"), 0),
            (op("BINARY_OP"), nb_or()),
        ],
        vec![Some(type_value(&vm, "int"))],
    )
    .expect("`int | int` 必须能算出来");
    let value = same.as_header(&vm.instance).expect("有返回值");
    assert_eq!(value, type_header(&vm, "int"), "`int | int` 去重后就是 int");

    let deduped = run(
        &vm,
        &[
            (op("LOAD_CONST"), 0),
            (op("LOAD_CONST"), 1),
            (op("BINARY_OP"), nb_or()),
            (op("LOAD_CONST"), 0),
            (op("BINARY_OP"), nb_or()),
        ],
        vec![Some(type_value(&vm, "int")), Some(type_value(&vm, "str"))],
    )
    .expect("`(int | str) | int` 必须能算出来");
    let args = union_args(&vm, deduped.as_header(&vm.instance).expect("有返回值"));
    assert_eq!(args.len(), 2, "重复成员必须去掉：{args:?}");
}

#[test]
fn none_or_none_is_still_a_type_error() {
    // `None | None`：两侧都不是类型/union ⇒ 参照 `TypeError: unsupported operand type(s) for |:
    // 'NoneType' and 'NoneType'`（不能悄悄变成一个 union）。
    let vm = Vm::new();
    let result = run(
        &vm,
        &[
            (op("LOAD_CONST"), 0),
            (op("LOAD_CONST"), 1),
            (op("BINARY_OP"), nb_or()),
        ],
        vec![
            Some(vm.instance.new_none()),
            Some(vm.instance.new_none()),
        ],
    );
    assert!(
        matches!(result, Err(ExecError::Raised { .. })),
        "`None | None` 必须抛 TypeError：{result:?}"
    );
    let (type_name, message) = vm.pending_exception().expect("应当有异常");
    assert_eq!(type_name, "TypeError");
    let message = message.expect("消息要带两侧类型");
    assert!(
        message.contains("'NoneType' and 'NoneType'"),
        "参照消息：{message}"
    );
}

#[test]
fn a_bare_type_or_plain_value_is_still_a_type_error() {
    // `int | 5`：只有**类型/union** 才能和类型做 `|` ⇒ 参照 TypeError（不能被 union 分支吞掉）。
    let vm = Vm::new();
    let result = run(
        &vm,
        &[
            (op("LOAD_CONST"), 0),
            (op("LOAD_CONST"), 1),
            (op("BINARY_OP"), nb_or()),
        ],
        vec![Some(type_value(&vm, "int")), Some(vm.instance.new_int(5))],
    );
    assert!(
        matches!(result, Err(ExecError::Raised { .. })),
        "`int | 5` 必须抛 TypeError：{result:?}"
    );
    let (type_name, message) = vm.pending_exception().expect("应当有异常");
    assert_eq!(type_name, "TypeError");
    assert!(
        message.expect("消息").contains("'type' and 'int'"),
        "参照消息"
    );
}
