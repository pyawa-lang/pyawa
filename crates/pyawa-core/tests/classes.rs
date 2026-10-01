//! `class` 语句的落点：`LOAD_BUILD_CLASS` ＋ `__build_class__`（`OM-14` 的类创建钩子）。
//!
//! 实测的发射形状：
//!
//! ```text
//! class Base: pass          → LOAD_BUILD_CLASS; PUSH_NULL; LOAD_CONST <类体>; MAKE_FUNCTION;
//!                             LOAD_CONST 'Base'; CALL 2; STORE_NAME Base
//! class C(Base): x = 1; …   → …; LOAD_CONST 'C'; LOAD_NAME Base; CALL 3; STORE_NAME C
//! ```
//!
//! 类体是一段**局部变量放在映射里**的代码（`LOAD_NAME`／`STORE_NAME`），`__build_class__`
//! 跑完它之后建类型、把命名空间搬进类型字典。
//!
//! **尚未接线**：`metaclass=`、`__prepare__`、`__set_name__`（要描述符）、`__mro_entries__`。

mod common;

use core::cell::RefCell;

use pyawa_core::opcode::get_nb_ops;
use core::ptr::NonNull;

use pyawa_core::{DictObject, Header, StrObject, Value};

use common::{assemble, op, Item, Vm};

fn nb(name: &str) -> u8 {
    get_nb_ops()
        .iter()
        .position(|(candidate, _)| *candidate == name)
        .unwrap_or_else(|| panic!("get_nb_ops 缺 {name}")) as u8
}

/// 造 `class C: x = 1; def m(self): return self.x`，返回一个"模块命名空间帧"跑完的结果。
///
/// 模块级代码的局部变量也是映射（`STORE_NAME`／`LOAD_NAME`），所以顶层帧要走命名空间形态。
fn run_module(
    vm: &Vm,
    bytes: Vec<u8>,
    consts: Vec<Option<NonNull<Header>>>,
    names: Vec<String>,
) -> NonNull<Header> {
    let code = vm.code_with_names(8, 0, 0, Vec::new(), names, bytes, consts);
    let namespace = vm
        .instance
        .alloc(DictObject::new(
            vm.instance.type_named("dict").unwrap(),
            RefCell::new(Vec::new()),
        ))
        .into_raw()
        .cast::<Header>();
    // 帧接手的是**新引用**（局部变量是映射这一形态），故这里先为它新增一份，
    // 测试自己那份要留着还给调用方。
    // SAFETY: namespace 由本测试持有，存活。
    unsafe { vm.instance.incref_object(namespace.as_ptr()) };
    let frame = pyawa_core::Frame::for_code_with_namespace(vm.frame_type, &code, namespace);
    let frame = vm.instance.alloc(frame);
    let outcome = match pyawa_core::execute(&vm.instance, &frame) {
        Ok(outcome) => outcome,
        Err(error) => panic!(
            "跑挂了：{error:?}｜异常：{:?}",
            vm.instance.pending_exception().map(|raw| {
                // SAFETY: raw 由实例持有。
                let ty = unsafe { raw.as_ref() }.ty();
                // SAFETY: 同上。
                let name = unsafe { ty.as_ref() }.name().to_owned();
                // SAFETY: raw 是异常实例。
                let message = unsafe { &*raw.as_ptr().cast::<pyawa_core::ExceptionObject>() }
                    .message_with(&vm.instance);
                (name, message)
            })
        ),
    };
    match outcome {
        pyawa_core::ExecOutcome::Returned(_) => {}
        pyawa_core::ExecOutcome::Yielded(_) => panic!("顶层不该让出"),
    }
    namespace
}

fn namespace_lookup(vm: &Vm, namespace: NonNull<Header>, name: &str) -> NonNull<Header> {
    // SAFETY: namespace 由调用方持有，存活。
    let mapping = unsafe { &*namespace.as_ptr().cast::<DictObject>() };
    let key = vm.instance.new_str(name);
    let position = mapping
        .entries()
        .iter()
        .position(|(existing, _)| {
            // SAFETY: 两边的键都存活。
            let left = unsafe { &*existing.as_ptr().cast::<StrObject>() }.value().to_owned();
            let right = unsafe { &*key.as_ptr().cast::<StrObject>() }.value().to_owned();
            left == right
        })
        .unwrap_or_else(|| panic!("命名空间里没有 {name}"));
    let (_, value) = mapping.entry(position).expect("刚查到的位置");
    value
}

#[test]
fn build_class_creates_a_type_with_the_body_namespace() {
    let vm = Vm::new();

    // 类体：`x = 1; def m(self): return self.x`
    let method_code = vm.code_with_names(
        4,
        1,
        1,
        vec!["self".to_owned()],
        vec!["x".to_owned()],
        assemble(&[
            Item::Instr(op("RESUME"), 0),
            Item::Instr(op("LOAD_FAST"), 0),
            Item::Instr(op("LOAD_ATTR"), 0),
            Item::Instr(op("RETURN_VALUE"), 0),
        ]),
        Vec::new(),
    );
    let method_header = method_code.as_ptr().cast::<Header>();
    // SAFETY: method_code 由本测试持有，常量表要自己那份。
    unsafe { vm.instance.incref_object(method_header.as_ptr()) };

    let body_code = vm.code_with_names(
        8,
        0,
        0,
        Vec::new(),
        vec!["x".to_owned(), "m".to_owned()],
        assemble(&[
            Item::Instr(op("RESUME"), 0),
            Item::Instr(op("LOAD_CONST"), 0),
            Item::Instr(op("STORE_NAME"), 0), // x = 1
            Item::Instr(op("LOAD_CONST"), 1),
            Item::Instr(op("MAKE_FUNCTION"), 0),
            Item::Instr(op("STORE_NAME"), 1), // m = <function>
            Item::Instr(op("LOAD_CONST"), 2),
            Item::Instr(op("RETURN_VALUE"), 0),
        ]),
        vec![
            Some(vm.constant(1)),
            Some(method_header),
            Some(vm.instance.own(vm.instance.singletons().none()).into_raw()),
        ],
    );
    let body_header = body_code.as_ptr().cast::<Header>();
    // SAFETY: body_code 由本测试持有，常量表要自己那份。
    unsafe { vm.instance.incref_object(body_header.as_ptr()) };

    // 模块级：`C = __build_class__(<body>, 'C')`
    let bytes = assemble(&[
        Item::Instr(op("RESUME"), 0),
        Item::Instr(op("LOAD_BUILD_CLASS"), 0),
        Item::Instr(op("PUSH_NULL"), 0),
        Item::Instr(op("LOAD_CONST"), 0),
        Item::Instr(op("MAKE_FUNCTION"), 0),
        Item::Instr(op("LOAD_CONST"), 1),
        Item::Instr(op("CALL"), 2),
        Item::Instr(op("STORE_NAME"), 0),
        Item::Instr(op("LOAD_CONST"), 2),
        Item::Instr(op("RETURN_VALUE"), 0),
    ]);
    let namespace = run_module(
        &vm,
        bytes,
        vec![
            Some(body_header),
            Some(vm.instance.new_str("C")),
            Some(vm.instance.own(vm.instance.singletons().none()).into_raw()),
        ],
        vec!["C".to_owned()],
    );

    let class_header = namespace_lookup(&vm, namespace, "C");
    let class = class_header.cast::<pyawa_core::TypeObject>();
    // SAFETY: class 是存活对象。
    let class_ref = unsafe { &*class.as_ptr() };
    assert_eq!(class_ref.name(), "C");
    assert_eq!(class_ref.bases().len(), 1, "没写基类 ⇒ 继承 object");
    assert!(
        vm.instance.is_subtype(class, vm.instance.type_named("object").unwrap()),
        "任何类都是 object 的子类"
    );
    // 类体里的名字进了类型字典 ⇒ `C.x` 与 `C.m` 都在
    assert!(vm.instance.type_lookup(class, "x").is_some());
    assert!(vm.instance.type_lookup(class, "m").is_some());
}

#[test]
fn a_class_body_local_lookup_uses_the_namespace() {
    // 类体里 `y = x + 1`（`LOAD_NAME x` ⇒ `STORE_NAME y`）走的是命名空间映射
    let vm = Vm::new();
    let body_code = vm.code_with_names(
        8,
        0,
        0,
        Vec::new(),
        vec!["x".to_owned(), "y".to_owned()],
        assemble(&[
            Item::Instr(op("RESUME"), 0),
            Item::Instr(op("LOAD_CONST"), 0),
            Item::Instr(op("STORE_NAME"), 0), // x = 41
            Item::Instr(op("LOAD_NAME"), 0),  // x
            Item::Instr(op("LOAD_CONST"), 1),
            Item::Instr(op("BINARY_OP"), nb("NB_ADD")),
            Item::Instr(op("STORE_NAME"), 1), // y = x + 1
            Item::Instr(op("LOAD_CONST"), 2),
            Item::Instr(op("RETURN_VALUE"), 0),
        ]),
        vec![
            Some(vm.constant(41)),
            Some(vm.constant(1)),
            Some(vm.instance.own(vm.instance.singletons().none()).into_raw()),
        ],
    );
    let body_header = body_code.as_ptr().cast::<Header>();
    // SAFETY: body_code 由本测试持有。
    unsafe { vm.instance.incref_object(body_header.as_ptr()) };

    let bytes = assemble(&[
        Item::Instr(op("RESUME"), 0),
        Item::Instr(op("LOAD_BUILD_CLASS"), 0),
        Item::Instr(op("PUSH_NULL"), 0),
        Item::Instr(op("LOAD_CONST"), 0),
        Item::Instr(op("MAKE_FUNCTION"), 0),
        Item::Instr(op("LOAD_CONST"), 1),
        Item::Instr(op("CALL"), 2),
        Item::Instr(op("STORE_NAME"), 0),
        Item::Instr(op("LOAD_CONST"), 2),
        Item::Instr(op("RETURN_VALUE"), 0),
    ]);
    let namespace = run_module(
        &vm,
        bytes,
        vec![
            Some(body_header),
            Some(vm.instance.new_str("C")),
            Some(vm.instance.own(vm.instance.singletons().none()).into_raw()),
        ],
        vec!["C".to_owned()],
    );
    let class = namespace_lookup(&vm, namespace, "C").cast::<pyawa_core::TypeObject>();
    let y = vm.instance.type_lookup(class, "y").expect("y 应当在类型字典里");
    // SAFETY: y 是整数对象。
    assert_eq!(unsafe { &*y.as_ptr().cast::<pyawa_core::IntObject>() }.value, 42);
}

#[test]
fn instantiation_runs_the_class_init() {
    // 类建好之后 `C(41)` 能跑：`__init__` 从类型字典沿 MRO 找（`OM-14`）
    let vm = Vm::new();
    let init_code = vm.code_with_names(
        8,
        2,
        2,
        vec!["self".to_owned(), "value".to_owned()],
        vec!["value".to_owned()],
        assemble(&[
            Item::Instr(op("RESUME"), 0),
            Item::Instr(op("LOAD_FAST"), 1),
            Item::Instr(op("LOAD_FAST"), 0),
            Item::Instr(op("STORE_ATTR"), 0), // self.value = value
            Item::Instr(op("LOAD_CONST"), 0),
            Item::Instr(op("RETURN_VALUE"), 0),
        ]),
        vec![Some(vm.instance.own(vm.instance.singletons().none()).into_raw())],
    );
    let init_header = init_code.as_ptr().cast::<Header>();
    // SAFETY: init_code 由本测试持有。
    unsafe { vm.instance.incref_object(init_header.as_ptr()) };

    let body_code = vm.code_with_names(
        8,
        0,
        0,
        Vec::new(),
        vec!["__init__".to_owned()],
        assemble(&[
            Item::Instr(op("RESUME"), 0),
            Item::Instr(op("LOAD_CONST"), 0),
            Item::Instr(op("MAKE_FUNCTION"), 0),
            Item::Instr(op("STORE_NAME"), 0),
            Item::Instr(op("LOAD_CONST"), 1),
            Item::Instr(op("RETURN_VALUE"), 0),
        ]),
        vec![
            Some(init_header),
            Some(vm.instance.own(vm.instance.singletons().none()).into_raw()),
        ],
    );
    let body_header = body_code.as_ptr().cast::<Header>();
    // SAFETY: body_code 由本测试持有。
    unsafe { vm.instance.incref_object(body_header.as_ptr()) };

    // 模块级：建类 → 实例化 `C(41)` → 读回 `.value` 存进命名空间（不截断字节，整段重写）
    let bytes = assemble(&[
        Item::Instr(op("RESUME"), 0),
        Item::Instr(op("LOAD_BUILD_CLASS"), 0),
        Item::Instr(op("PUSH_NULL"), 0),
        Item::Instr(op("LOAD_CONST"), 0),
        Item::Instr(op("MAKE_FUNCTION"), 0),
        Item::Instr(op("LOAD_CONST"), 1),
        Item::Instr(op("CALL"), 2),
        Item::Instr(op("STORE_NAME"), 0), // C
        Item::Instr(op("LOAD_NAME"), 0),
        Item::Instr(op("PUSH_NULL"), 0),
        Item::Instr(op("LOAD_CONST"), 2),
        Item::Instr(op("CALL"), 1), // C(41)
        Item::Instr(op("STORE_NAME"), 1), // instance
        Item::Instr(op("LOAD_NAME"), 1),
        // `LOAD_ATTR` 的名字下标是 `oparg >> 1`（BC-57：低位是取方法位）⇒ "value" 是下标 3，oparg ＝ 6
        Item::Instr(op("LOAD_ATTR"), 3 << 1),
        Item::Instr(op("STORE_NAME"), 2), // result
        Item::Instr(op("LOAD_CONST"), 3),
        Item::Instr(op("RETURN_VALUE"), 0),
    ]);
    let namespace = run_module(
        &vm,
        bytes,
        vec![
            Some(body_header),
            Some(vm.instance.new_str("C")),
            Some(vm.constant(41)),
            Some(vm.instance.own(vm.instance.singletons().none()).into_raw()),
        ],
        vec![
            "C".to_owned(),
            "instance".to_owned(),
            "result".to_owned(),
            "value".to_owned(),
        ],
    );
    let result = namespace_lookup(&vm, namespace, "result");
    // SAFETY: result 是整数对象。
    assert_eq!(
        unsafe { &*result.as_ptr().cast::<pyawa_core::IntObject>() }.value,
        41,
        "`__init__` 把 41 存进了实例属性"
    );
    let _ = Value::small_int(0);
}

#[test]
fn minimal_namespace_frame_roundtrip() {
    // 最小复现：命名空间帧里 `LOAD_NAME x` ＋ `STORE_NAME y`，不建类
    let vm = Vm::new();
    let code = vm.code_with_names(
        4,
        0,
        0,
        Vec::new(),
        vec!["x".to_owned(), "y".to_owned()],
        assemble(&[
            Item::Instr(op("RESUME"), 0),
            Item::Instr(op("LOAD_CONST"), 0),
            Item::Instr(op("STORE_NAME"), 0),
            Item::Instr(op("LOAD_NAME"), 0),
            Item::Instr(op("LOAD_CONST"), 2),
            Item::Instr(op("BINARY_OP"), nb("NB_ADD")),
            Item::Instr(op("STORE_NAME"), 1),
            Item::Instr(op("LOAD_CONST"), 1),
            Item::Instr(op("RETURN_VALUE"), 0),
        ]),
        vec![
            Some(vm.constant(5)),
            Some(vm.instance.own(vm.instance.singletons().none()).into_raw()),
            Some(vm.constant(1)),
            Some(vm.instance.own(vm.instance.singletons().none()).into_raw()),
        ],
    );
    let namespace = vm
        .instance
        .alloc(DictObject::new(
            vm.instance.type_named("dict").unwrap(),
            RefCell::new(Vec::new()),
        ))
        .into_raw()
        .cast::<Header>();
    // SAFETY: namespace 由本测试持有。
    unsafe { vm.instance.incref_object(namespace.as_ptr()) };
    let frame = pyawa_core::Frame::for_code_with_namespace(vm.frame_type, &code, namespace);
    let frame = vm.instance.alloc(frame);
    pyawa_core::execute(&vm.instance, &frame).unwrap();
    let y = namespace_lookup(&vm, namespace, "y");
    // SAFETY: y 是整数。
    assert_eq!(unsafe { &*y.as_ptr().cast::<pyawa_core::IntObject>() }.value, 6);
}

// ---- `AB-58`／`AB-37`：宿主类型的 Python 子类 ----

use pyawa_core::{free_fixed_layout, python_level_finalize, HostTraverse, HostVisit, Slots};

unsafe extern "C" fn host_dealloc(_payload: *mut core::ffi::c_void) {}
unsafe extern "C" fn host_traverse(
    _payload: *mut core::ffi::c_void,
    _context: *mut core::ffi::c_void,
    _visit: HostVisit,
) {
}

/// 造一个**定长宿主类型**（`payload` 字节载荷；`final_type` ⇒ `PA_TYPE_FINAL`）。
fn host_type(vm: &Vm, name: &'static str, payload: usize, final_type: bool) -> NonNull<pyawa_core::TypeObject> {
    let ty = vm.instance.new_type(
        name,
        pyawa_core::HEADER_SIZE_BYTES + payload,
        Slots::new(free_fixed_layout)
            .with_traverse(|_ptr, _visit| {})
            .with_finalize(python_level_finalize),
    );
    let info = unsafe { ty.as_ref() };
    info.set_host_hooks(host_dealloc, host_traverse as HostTraverse);
    info.mark_external_instance_dict();
    if final_type {
        info.mark_final();
    }
    ty
}

/// 跑一个模块程序，返回 `Result`（错误要能观察，所以不复用会 panic 的那份）。
fn try_module(
    vm: &Vm,
    bytes: Vec<u8>,
    consts: Vec<Option<NonNull<Header>>>,
    names: Vec<String>,
) -> Result<NonNull<Header>, pyawa_core::ExecError> {
    let code = vm.code_with_names(8, 0, 0, Vec::new(), names, bytes, consts);
    let namespace = vm
        .instance
        .alloc(DictObject::new(
            vm.instance.type_named("dict").unwrap(),
            RefCell::new(Vec::new()),
        ))
        .into_raw()
        .cast::<Header>();
    // SAFETY: namespace 由本测试持有。
    unsafe { vm.instance.incref_object(namespace.as_ptr()) };
    let frame = pyawa_core::Frame::for_code_with_namespace(vm.frame_type, &code, namespace);
    let frame = vm.instance.alloc(frame);
    pyawa_core::execute(&vm.instance, &frame)?;
    Ok(namespace)
}

/// 组一段"`Name = __build_class__(body, 'Name', *bases)`"的模块程序并跑。
fn build_subclass(
    vm: &Vm,
    name: &str,
    bases: Vec<NonNull<Header>>,
) -> Result<NonNull<Header>, pyawa_core::ExecError> {
    let body_code = vm.code_with_names(
        4,
        0,
        0,
        Vec::new(),
        Vec::new(),
        assemble(&[
            Item::Instr(op("RESUME"), 0),
            Item::Instr(op("LOAD_CONST"), 0),
            Item::Instr(op("RETURN_VALUE"), 0),
        ]),
        vec![Some(
            vm.instance.own(vm.instance.singletons().none()).into_raw(),
        )],
    );
    let body_header = body_code.as_ptr().cast::<Header>();
    // SAFETY: body_code 由本测试持有，常量表要自己那份。
    unsafe { vm.instance.incref_object(body_header.as_ptr()) };

    // `LOAD_BUILD_CLASS; PUSH_NULL; LOAD_CONST <body>; MAKE_FUNCTION; LOAD_CONST '<name>';`
    // 然后逐个基类 `LOAD_CONST <base>`，最后 `CALL 3 + len(bases)`
    let mut instructions = vec![
        Item::Instr(op("RESUME"), 0),
        Item::Instr(op("LOAD_BUILD_CLASS"), 0),
        Item::Instr(op("PUSH_NULL"), 0),
        Item::Instr(op("LOAD_CONST"), 0),
        Item::Instr(op("MAKE_FUNCTION"), 0),
        Item::Instr(op("LOAD_CONST"), 1),
    ];
    for index in 0..bases.len() {
        instructions.push(Item::Instr(op("LOAD_CONST"), (2 + index) as u8));
    }
    instructions.push(Item::Instr(
        op("CALL"),
        (2 + bases.len()) as u8,
    ));
    instructions.push(Item::Instr(op("STORE_NAME"), 0));
    instructions.push(Item::Instr(op("LOAD_CONST"), (2 + bases.len()) as u8));
    instructions.push(Item::Instr(op("RETURN_VALUE"), 0));

    let mut consts = vec![
        Some(body_header),
        Some(vm.instance.new_str(name)),
    ];
    for base in bases {
        consts.push(Some(base));
    }
    consts.push(Some(vm.instance.own(vm.instance.singletons().none()).into_raw()));
    try_module(
        vm,
        assemble(&instructions),
        consts,
        vec![name.to_owned()],
    )
}

#[test]
fn a_python_subclass_of_a_host_type_inherits_the_layout() {
    let vm = Vm::new();
    let host = host_type(&vm, "HostWidget", 8, false);
    // 基类以**值**的形式交给 `__build_class__`
    let host_value = vm.instance.type_value(host);
    let namespace = build_subclass(&vm, "Sub", vec![host_value]).expect("建子类");
    let class_header = namespace_lookup(&vm, namespace, "Sub");
    let class = class_header.cast::<pyawa_core::TypeObject>();
    // SAFETY: class 是存活对象。
    let info = unsafe { class.as_ref() };
    assert_eq!(
        info.instance_size(),
        // SAFETY: host 由注册表持有。
        unsafe { host.as_ref() }.instance_size(),
        "AB-58：子类实例的载荷按**同一尺寸**由 VM 分配"
    );
    assert!(info.is_host_layout(), "AB-37：布局与宿主钩子一起继承");
    assert!(
        !info.has_inline_instance_dict(),
        "OM-14：宿主布局固定 ⇒ 实例字典另行挂载"
    );
    // 子类实例按同一布局分配（宿主无需参与）
    let (object, payload) = vm.instance.alloc_host_object(class);
    assert!(payload.is_some(), "载荷 8 字节");
    // SAFETY: object 是本测试持有的新引用。
    unsafe { vm.instance.release_object(object.as_ptr()) };
}

#[test]
fn two_host_bases_have_a_layout_conflict() {
    let vm = Vm::new();
    let first = host_type(&vm, "First", 8, false);
    let second = host_type(&vm, "Second", 8, false);
    let values = vec![
        vm.instance.type_value(first),
        vm.instance.type_value(second),
    ];
    let error = build_subclass(&vm, "Bad", values).expect_err("两个定长基类应当冲突");
    let _ = error;
    let (type_name, message) = vm.pending_exception().expect("应当有异常");
    assert_eq!(type_name, "TypeError");
    assert_eq!(
        message.as_deref(),
        Some("multiple bases have instance lay-out conflict"),
        "实测原话"
    );
}

#[test]
fn a_final_host_type_cannot_be_a_base() {
    let vm = Vm::new();
    let sealed = host_type(&vm, "Sealed", 8, true);
    let value = vm.instance.type_value(sealed);
    build_subclass(&vm, "Nope", vec![value]).expect_err("不可继承的类型不能作基类");
    let (type_name, message) = vm.pending_exception().expect("应当有异常");
    assert_eq!(type_name, "TypeError");
    assert_eq!(
        message.as_deref(),
        Some("type 'Sealed' is not an acceptable base type"),
        "实测原话（AB-37 的 PA_TYPE_FINAL）"
    );
}
