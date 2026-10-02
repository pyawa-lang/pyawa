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
    assert_eq!(unsafe { &*y.as_ptr().cast::<pyawa_core::IntObject>() }.value.to_i64().expect("测试里是小整数"), 42);
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
        unsafe { &*result.as_ptr().cast::<pyawa_core::IntObject>() }.value.to_i64().expect("测试里是小整数"),
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
    assert_eq!(unsafe { &*y.as_ptr().cast::<pyawa_core::IntObject>() }.value.to_i64().expect("测试里是小整数"), 6);
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

// ---- `LOAD_GLOBAL` 与 `__globals__`（调用与返回族的收尾）----

/// 模块：`g = 41`；`def f(): return g`；`r = f()`。
fn module_with_a_global(vm: &Vm) -> NonNull<Header> {
    // 函数体：`return g`（`LOAD_GLOBAL` 带 NULL 位？不带——不是调用）
    let function_code = vm.code_with_names(
        4,
        0,
        0,
        Vec::new(),
        vec!["g".to_owned()],
        assemble(&[
            Item::Instr(op("RESUME"), 0),
            Item::Instr(op("LOAD_GLOBAL"), 0), // 名字下标 0 ＝ `oparg >> 1`
            Item::Instr(op("RETURN_VALUE"), 0),
        ]),
        Vec::new(),
    );
    let function_header = function_code.as_ptr().cast::<Header>();
    // SAFETY: function_code 由本测试持有，常量表要自己那份。
    unsafe { vm.instance.incref_object(function_header.as_ptr()) };

    let bytes = assemble(&[
        Item::Instr(op("RESUME"), 0),
        Item::Instr(op("LOAD_CONST"), 0),
        Item::Instr(op("STORE_NAME"), 0), // g = 41
        Item::Instr(op("LOAD_CONST"), 1),
        Item::Instr(op("MAKE_FUNCTION"), 0),
        Item::Instr(op("STORE_NAME"), 1), // f = <function>
        Item::Instr(op("LOAD_NAME"), 1),
        Item::Instr(op("PUSH_NULL"), 0),
        Item::Instr(op("CALL"), 0),
        Item::Instr(op("STORE_NAME"), 2), // r = f()
        Item::Instr(op("LOAD_CONST"), 2),
        Item::Instr(op("RETURN_VALUE"), 0),
    ]);
    run_module(
        &vm,
        bytes,
        vec![
            Some(vm.constant(41)),
            Some(function_header),
            Some(vm.instance.own(vm.instance.singletons().none()).into_raw()),
        ],
        vec!["g".to_owned(), "f".to_owned(), "r".to_owned()],
    )
}

#[test]
fn a_function_reads_the_module_globals_it_was_defined_in() {
    let vm = Vm::new();
    let namespace = module_with_a_global(&vm);
    let result = namespace_lookup(&vm, namespace, "r");
    // SAFETY: r 是整数。
    assert_eq!(
        unsafe { &*result.as_ptr().cast::<pyawa_core::IntObject>() }.value.to_i64().expect("测试里是小整数"),
        41,
        "函数体里的 `LOAD_GLOBAL g` 要看到模块级那个 41（MAKE_FUNCTION 捕获的 __globals__）"
    );
}

#[test]
fn an_unknown_global_is_a_name_error() {
    let vm = Vm::new();
    // 函数体读一个不存在的全局
    let function_code = vm.code_with_names(
        4,
        0,
        0,
        Vec::new(),
        vec!["missing".to_owned()],
        assemble(&[
            Item::Instr(op("RESUME"), 0),
            Item::Instr(op("LOAD_GLOBAL"), 0),
            Item::Instr(op("RETURN_VALUE"), 0),
        ]),
        Vec::new(),
    );
    let function_header = function_code.as_ptr().cast::<Header>();
    // SAFETY: function_code 由本测试持有。
    unsafe { vm.instance.incref_object(function_header.as_ptr()) };
    let bytes = assemble(&[
        Item::Instr(op("RESUME"), 0),
        Item::Instr(op("LOAD_CONST"), 0),
        Item::Instr(op("MAKE_FUNCTION"), 0),
        Item::Instr(op("STORE_NAME"), 0),
        Item::Instr(op("LOAD_NAME"), 0),
        Item::Instr(op("PUSH_NULL"), 0),
        Item::Instr(op("CALL"), 0),
        Item::Instr(op("RETURN_VALUE"), 0),
    ]);
    let code = vm.code_with_names(
        8,
        0,
        0,
        Vec::new(),
        vec!["f".to_owned()],
        bytes,
        vec![
            Some(function_header),
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
    // SAFETY: namespace 由本测试持有，帧要自己那份。
    unsafe { vm.instance.incref_object(namespace.as_ptr()) };
    let frame = pyawa_core::Frame::for_code_with_namespace(vm.frame_type, &code, namespace);
    let frame = vm.instance.alloc(frame);
    let _ = pyawa_core::execute(&vm.instance, &frame);
    let (type_name, message) = vm.pending_exception().expect("应当有异常");
    assert_eq!(type_name, "NameError");
    assert_eq!(message.as_deref(), Some("name 'missing' is not defined"));
}

#[test]
fn a_class_body_can_read_the_module_globals() {
    // 类体里 `x = g`（`g` 在模块层）——`LOAD_NAME` 的第二层要能找到它
    let vm = Vm::new();
    let body_code = vm.code_with_names(
        4,
        0,
        0,
        Vec::new(),
        vec!["g".to_owned(), "x".to_owned()],
        assemble(&[
            Item::Instr(op("RESUME"), 0),
            Item::Instr(op("LOAD_NAME"), 0), // g
            Item::Instr(op("STORE_NAME"), 1), // x = g
            Item::Instr(op("LOAD_CONST"), 0),
            Item::Instr(op("RETURN_VALUE"), 0),
        ]),
        vec![Some(vm.instance.own(vm.instance.singletons().none()).into_raw())],
    );
    let body_header = body_code.as_ptr().cast::<Header>();
    // SAFETY: body_code 由本测试持有，常量表要自己那份。
    unsafe { vm.instance.incref_object(body_header.as_ptr()) };

    let bytes = assemble(&[
        Item::Instr(op("RESUME"), 0),
        Item::Instr(op("LOAD_CONST"), 0),
        Item::Instr(op("STORE_NAME"), 0), // g = 7
        Item::Instr(op("LOAD_BUILD_CLASS"), 0),
        Item::Instr(op("PUSH_NULL"), 0),
        Item::Instr(op("LOAD_CONST"), 1),
        Item::Instr(op("MAKE_FUNCTION"), 0),
        Item::Instr(op("LOAD_CONST"), 2), // "C"
        Item::Instr(op("CALL"), 2),
        Item::Instr(op("STORE_NAME"), 1), // C
        Item::Instr(op("LOAD_CONST"), 3),
        Item::Instr(op("RETURN_VALUE"), 0),
    ]);
    let namespace = run_module(
        &vm,
        bytes,
        vec![
            Some(vm.constant(7)),
            Some(body_header),
            Some(vm.instance.new_str("C")),
            Some(vm.instance.own(vm.instance.singletons().none()).into_raw()),
        ],
        vec!["g".to_owned(), "C".to_owned()],
    );
    let class_header = namespace_lookup(&vm, namespace, "C");
    let class = class_header.cast::<pyawa_core::TypeObject>();
    // SAFETY: class 由注册表持有。
    let info = unsafe { class.as_ref() };
    let mapping = info.dict().expect("类有类型字典");
    // SAFETY: 字典由类型对象持有。
    let dict = unsafe { &*mapping.as_ptr().cast::<DictObject>() };
    let position = dict
        .entries()
        .iter()
        .position(|(key, _)| {
            // SAFETY: key 由字典持有。
            unsafe { &*key.as_ptr().cast::<pyawa_core::StrObject>() }.value() == "x"
        })
        .expect("类体里 x 应当在类型字典里");
    let (_, value) = dict.entry(position).expect("刚查到的位置");
    // SAFETY: 值是整数。
    assert_eq!(
        unsafe { &*value.as_ptr().cast::<pyawa_core::IntObject>() }.value.to_i64().expect("测试里是小整数"),
        7,
        "类体里的 `LOAD_NAME g` 要看到模块层的 7"
    );
}

#[test]
fn host_subclass_instance_carries_an_attribute_dict() {
    // `OM-14` 的验收合起来走一遍：**宿主类型**（定长载荷、布局固定）的 **Python 子类**实例
    // 也要能带属性字典——字典另行挂载，且**不碰**宿主那 8 字节载荷。
    let vm = Vm::new();
    let host = host_type(&vm, "HostWidget2", 8, false);
    let host_value = vm.instance.type_value(host);
    let namespace = build_subclass(&vm, "Sub2", vec![host_value]).expect("建子类");
    let class = namespace_lookup(&vm, namespace, "Sub2").cast::<pyawa_core::TypeObject>();
    let (object, payload) = vm.instance.alloc_host_object(class);
    let payload = payload.expect("AB-58：8 字节载荷");
    // 载荷先写可辨认的哨兵值，跑完再看它有没有被动过
    // SAFETY: payload 指向本实例的载荷，本测试独占。
    unsafe {
        for offset in 0..8 {
            payload.as_ptr().add(offset).write(0xAB);
        }
    }
    // SAFETY: object 是本测试持有的新引用；常量表再持一份。
    unsafe { vm.instance.incref_object(object.as_ptr()) };

    let bytes = assemble(&[
        Item::Instr(op("RESUME"), 0),
        Item::Instr(op("LOAD_CONST"), 1),
        Item::Instr(op("LOAD_CONST"), 0),
        Item::Instr(op("STORE_ATTR"), 0), // 名字下标 = oparg（STORE_ATTR 不移位，BC-57）
        Item::Instr(op("LOAD_CONST"), 0),
        Item::Instr(op("LOAD_ATTR"), 0),
        Item::Instr(op("RETURN_VALUE"), 0),
    ]);
    let code = vm.code_with_names(
        8,
        0,
        0,
        Vec::new(),
        vec!["answer".to_owned()],
        bytes,
        vec![Some(object), Some(vm.constant(42))],
    );
    let result = vm.run(&code).expect("属性写入与读回都应当成功");
    assert!(
        result.is_same(&Value::small_int(42), &vm.instance),
        "宿主子类实例 `obj.answer` 应当读回 42"
    );

    // 字典挂在头部那一格（另行挂载），载荷 8 字节一个都没动
    let mapping = unsafe { object.as_ref() }
        .instance_dict()
        .expect("OM-14：字典应当已挂上");
    // SAFETY: mapping 由该对象持有。
    let dict = unsafe { &*mapping.as_ptr().cast::<DictObject>() };
    assert_eq!(dict.entries().len(), 1, "字典里应当是 answer=42");
    for offset in 0..8 {
        // SAFETY: 同上，载荷在本实例内且仍然存活。
        let byte = unsafe { payload.as_ptr().add(offset).read() };
        assert_eq!(byte, 0xAB, "宿主载荷第 {offset} 字节不该被字典动过");
    }
    // SAFETY: 常量表那份引用由本测试归还。
    unsafe { vm.instance.release_object(object.as_ptr()) };
}

#[test]
fn class_creation_fills_in_the_method_qualname() {
    // **`BC-4`**：参照实现里方法的 `co_qualname`（`C.m`）由**编译器**写死；本层编译器还没有
    // 类体 ⇒ 由**类创建钩子**在建类时补写。这里造一个"类体里定义函数"的类，验补写生效。
    let vm = Vm::new();
    let none = vm.instance.singletons().none();
    // 方法本身：code 的 qualname 先只有 `m`（编译器在类体里还没接线）
    let method_code = vm.instance.alloc(pyawa_core::CodeObject::new(
        vm.code_type,
        "m",
        "m".to_owned(),
        "<t>".to_owned(),
        1,
        4,
        1,
        1,
        0,
        0,
        0,
        vec!["self".to_owned()],
        Vec::new(),
        Vec::new(),
        Vec::new(),
        assemble(&[
            Item::Instr(op("RESUME"), 0),
            Item::Instr(op("LOAD_CONST"), 0),
            Item::Instr(op("RETURN_VALUE"), 0),
        ]),
        Vec::new(),
        // 常量表**接手**一份引用 ⇒ 单例要先 `own`（`OM-16`）
        vec![Some(vm.instance.own(none).into_raw())],
        Vec::new(),
    ));
    // 类体：`m = <函数>`（`MAKE_FUNCTION` ＋ `STORE_NAME`）
    let body = vm.code_with_names(
        8,
        0,
        0,
        Vec::new(),
        vec!["m".to_owned()],
        assemble(&[
            Item::Instr(op("RESUME"), 0),
            Item::Instr(op("LOAD_CONST"), 0),
            Item::Instr(op("MAKE_FUNCTION"), 0),
            Item::Instr(op("STORE_NAME"), 0),
            Item::Instr(op("LOAD_CONST"), 1),
            Item::Instr(op("RETURN_VALUE"), 0),
        ]),
        vec![
            Some(method_code.as_ptr().cast::<Header>()),
            Some(vm.instance.own(none).into_raw()),
        ],
    );
    // SAFETY: body 与 method_code 都由本测试持有，常量表要各自那份。
    unsafe {
        vm.instance.incref_object(body.as_ptr().cast::<Header>().as_ptr());
        vm.instance.incref_object(method_code.as_ptr().cast::<Header>().as_ptr());
    }
    let program = assemble(&[
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
    let namespace = try_module(
        &vm,
        program,
        vec![
            Some(body.as_ptr().cast::<Header>()),
            Some(vm.instance.new_str("Widget")),
            Some(vm.instance.own(none).into_raw()),
        ],
        vec!["Widget".to_owned()],
    )
    .expect("建类应当成功");
    // 类型字典里的 `m`：code 的 qualname 必须已被补成 `Widget.m`
    let class = namespace_lookup(&vm, namespace, "Widget").cast::<pyawa_core::TypeObject>();
    let method = vm
        .instance
        .type_lookup(class, "m")
        .expect("类字典里应当有 m");
    // SAFETY: 类体里放进去的是函数对象。
    let function = unsafe { &*method.as_ptr().cast::<pyawa_core::FunctionObject>() };
    let method_code = function.code();
    // SAFETY: 函数持有 code 的一份引用。
    let method_code = unsafe { &*method_code.as_ptr().cast::<pyawa_core::CodeObject>() };
    assert_eq!(
        method_code.qualname(),
        "Widget.m",
        "BC-4：类创建钩子必须把方法的 co_qualname 补成 C.m"
    );
}
