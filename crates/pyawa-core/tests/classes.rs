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

use pyawa_core::{BuiltinFunctionObject, DictObject, Header, StrObject, Value};

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

// --------------------------------------------------------------------------- #
// task-6：零参 `super()` 的**定义类**解析（快路＝全局名字；慢路＝按帧 code 在 MRO 反查）
// --------------------------------------------------------------------------- #

/// 跑一段**模块源码**（编译 → 实例化 → 在带 `__name__` 的命名空间里执行），失败原样交回。
///
/// 零参 `super()` 的验证必须走**编译器真的产出的字节码**（方法的 `co_qualname` 补写、类体里
/// `__new__` 的 staticmethod 包装都在类创建钩子里）——手写字节码够不着那条链。
fn try_run_source(
    vm: &Vm,
    source: &str,
) -> Result<NonNull<Header>, pyawa_core::ExecError> {
    use pyawa_core::compile::{compile, instantiate, CheckTier, Mode};
    let unit = compile(source, "<t>", Mode::PurePython, CheckTier::Shallow, 0).expect("编得过");
    let code = instantiate(&vm.instance, &unit);
    let namespace = vm.instance.new_dict();
    let module_name = vm.instance.new_str("__main__");
    vm.instance.dict_set(namespace, "__name__", module_name);
    // core 的测试**不装 stdlib 的 `builtins`**（那些名字在 stdlib 注册）⇒ 给模块命名空间里挂测试要用的
    // 最小几个：`super`（同一个 native，`pyawa_stdlib::builtins_module` 一处真相）＋ `type`（类对象，
    // 只当元类的基类用，不被调用）。
    let super_function = vm.instance.alloc(BuiltinFunctionObject::new(
        vm
            .instance
            .type_named("builtin_function_or_method")
            .expect("builtin_function_or_method 在引导期已登记"),
        "super",
        core::cell::Cell::new(pyawa_core::super_new as pyawa_core::NativeFn),
    ));
    vm.instance
        .dict_set(namespace, "super", super_function.as_ptr().cast::<Header>());
    let type_object = vm.instance.type_named("type").expect("type 在引导期已登记");
    let type_header = type_object.cast::<Header>();
    vm.instance.dict_set(namespace, "type", type_header);
    // `NotImplemented` 单例（富比较协议的反向派发要它；stdlib 里才挂名字）。
    let not_implemented = vm.instance.not_implemented();
    vm.instance.dict_set(namespace, "NotImplemented", not_implemented);
    // `type.__new__` 也归 stdlib 的 builtins 安装（`crates/pyawa-stdlib/src/builtins_module.rs`），
    // core 测试里没有 ⇒ 给 `type` 命名空间挂**同一个** native（`pyawa_core::type_new_native`）。
    // 元类里 `super().__new__(…)` 的查找就要它。
    let type_namespace = vm
        .instance
        .type_namespace(type_header)
        .expect("type 的命名空间应当拿得到");
    if vm.instance.dict_get(type_namespace, "__new__").is_none() {
        let new_method = vm.instance.alloc(BuiltinFunctionObject::new(
            vm
                .instance
                .type_named("builtin_function_or_method")
                .expect("builtin_function_or_method 在引导期已登记"),
            "__new__",
            core::cell::Cell::new(pyawa_core::type_new_native as pyawa_core::NativeFn),
        ));
        vm.instance
            .dict_set(type_namespace, "__new__", new_method.as_ptr().cast::<Header>());
    }
    // 同理 `object.__new__`（`super().__new__(cls)` 的落点就是它）。
    let object_type = vm.instance.type_named("object").expect("object 在引导期已登记");
    let object_namespace = vm
        .instance
        .type_namespace(object_type.cast::<Header>())
        .expect("object 的命名空间应当拿得到");
    if vm.instance.dict_get(object_namespace, "__new__").is_none() {
        let new_method = vm.instance.alloc(BuiltinFunctionObject::new(
            vm
                .instance
                .type_named("builtin_function_or_method")
                .expect("builtin_function_or_method 在引导期已登记"),
            "__new__",
            core::cell::Cell::new(pyawa_core::object_new_native as pyawa_core::NativeFn),
        ));
        vm.instance
            .dict_set(object_namespace, "__new__", new_method.as_ptr().cast::<Header>());
    }
    // SAFETY: namespace 由本函数持有；`for_code_with_namespace` 接手一份引用（`OM-16`）。
    unsafe { vm.instance.incref_object(namespace.as_ptr()) };
    let frame = pyawa_core::Frame::for_code_with_namespace(vm.frame_type, &code, namespace);
    let frame = vm.instance.alloc(frame);
    pyawa_core::execute(&vm.instance, &frame).map(|_| namespace)
}

/// 同 [`try_run_source`]，失败直接 panic 并把异常类型／消息打出来（测试断言主路用）。
fn run_source(vm: &Vm, source: &str) -> NonNull<Header> {
    match try_run_source(vm, source) {
        Ok(namespace) => namespace,
        Err(error) => panic!(
            "源码跑挂了：{error:?}｜异常：{:?}",
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
    }
}

/// 命名空间里某项的字符串值。
fn source_text(vm: &Vm, namespace: NonNull<Header>, name: &str) -> String {
    let value = namespace_lookup(vm, namespace, name);
    vm.instance
        .text_value(value)
        .unwrap_or_else(|| panic!("{name} 应当是 str"))
}

#[test]
fn nested_tail_if_condition_exits_land_after_the_body() {
    // task-63（预存 emitter 回填 bug）：方法末尾的 `if A and B and C:` 里再套 `for`／`if` 时，
    // 外层前两个假出口曾被内层 `if` 的收尾逻辑**抢占**回填到体内 ⇒ 第二次调用（外层条件为假）
    // 直接跳进 `lines.append` 段 ⇒ `UnboundLocalError: lines`。判据：**第二次调用不炸且结果不变** ✓。
    let vm = Vm::new();
    let namespace = run_source(
        &vm,
        r#"
class F:
    def __init__(self):
        self._lines = None
        self.lineno = 1
        self.end_lineno = 2

    def s(self):
        if (
            self._lines is None
            and self.lineno is not None
            and self.end_lineno is not None
        ):
            lines = []
            for lineno in (self.lineno, self.end_lineno):
                line = lineno
                if not line:
                    line = -1
                lines.append(line)
            self._lines = lines

f = F()
f.s()
first_ok = f._lines
f.s()
second_ok = f._lines
ok = first_ok == second_ok and first_ok[0] == 1 and first_ok[1] == 2
"#,
    );
    let ok = namespace_lookup(&vm, namespace, "ok");
    assert_eq!(
        vm.instance.int_value(ok),
        Some(1),
        "第二次调用：外层条件为假 ⇒ 假出口必须落到**函数收尾**（不能跳进体内）；两次结果都应是 [1, 2]"
    );
}

#[test]
fn zero_arg_super_reaches_through_two_levels_and_a_nested_class() {
    // 两层继承：`Leaf`／`Inner` 的函数里定义的类 → 模块级 `Mid` → 模块级 `Root`；
    // 以及**类体里嵌套的类**（`Outer.Inner`，`co_qualname` 是 `Outer.Inner.__init__`，
    // 按"取最后一段类名再查全局"必然失败 ⇒ 慢路）。
    let vm = Vm::new();
    let namespace = run_source(
        &vm,
        r#"
class Root:
    def __init__(self):
        self.trace = ["Root"]

class Mid(Root):
    def __init__(self):
        super().__init__()
        self.trace.append("Mid")

def make():
    class Leaf(Mid):
        def __init__(self):
            super().__init__()
            self.trace.append("Leaf")
    return Leaf()

class Outer:
    class Inner(Mid):
        def __init__(self):
            super().__init__()
            self.trace.append("Inner")

leaf_trace = ",".join(make().trace)
inner_trace = ",".join(Outer.Inner().trace)
"#,
    );
    assert_eq!(
        source_text(&vm, namespace, "leaf_trace"),
        "Root,Mid,Leaf",
        "函数里定义的 Leaf：MRO 上每一层都要落到对的类"
    );
    assert_eq!(
        source_text(&vm, namespace, "inner_trace"),
        "Root,Mid,Inner",
        "类体里嵌套的类：`Outer.Inner` 也要能反查到定义类"
    );
}

#[test]
fn zero_arg_super_works_when_every_class_is_defined_inside_the_function() {
    // 三层**全在函数里**：每一层的 `super()` 都走慢路，且都要跳过"正确的这一层"
    let vm = Vm::new();
    let namespace = run_source(
        &vm,
        r#"
def make():
    class A:
        def __init__(self):
            self.trace = ["A"]
    class B(A):
        def __init__(self):
            super().__init__()
            self.trace.append("B")
    class C(B):
        def __init__(self):
            super().__init__()
            self.trace.append("C")
    return C()

result = "".join(make().trace)
"#,
    );
    assert_eq!(
        source_text(&vm, namespace, "result"),
        "ABC",
        "三层都定义在函数里：super() 必须逐层向上"
    );
}

#[test]
fn zero_arg_super_in_new_of_a_function_local_metaclass() {
    // `__new__` 在类体里被 `type.__new__` 自动包成 **staticmethod** ⇒ 慢路必须剥壳再比 code；
    // 这条同时验证"定义类是元类自己"的情形（参照输出 `Meta`）。
    let vm = Vm::new();
    let namespace = run_source(
        &vm,
        r#"
def make_meta():
    class Meta(type):
        def __new__(mcls, name, bases, namespace):
            cls = super().__new__(mcls, name, bases, namespace)
            cls.made_by = "Meta"
            return cls
    return Meta

M = make_meta()

class C(metaclass=M):
    pass

result = C.made_by
"#,
    );
    assert_eq!(
        source_text(&vm, namespace, "result"),
        "Meta",
        "函数里定义的元类：Meta.__new__ 的零参 super() 要找到 type.__new__"
    );
    let _ = namespace_lookup(&vm, namespace, "C");
}

#[test]
fn zero_arg_super_new_in_a_class_defined_inside_a_function() {
    // `__new__` 在类体里被包成 **staticmethod**，且 `super()` 的 `__self__` 是**类对象** ⇒
    // 两半都要对：慢路剥壳找到定义类 `Local`；`super_lookup` 用**该类自己的 MRO** 找到 `object.__new__`
    //（task-6 前一律查元类 MRO ⇒ 报 `type.__new__ 至少要 3 个实参`；模块级类也一样，见任务记录）。
    let vm = Vm::new();
    let namespace = run_source(
        &vm,
        r#"
class Base:
    def __init__(self, x):
        self.x = x

def make():
    class Local(Base):
        def __new__(cls, *args):
            return super().__new__(cls)
        def __init__(self, x):
            self.x = x + 1
    return Local(7)

result = make().x
"#,
    );
    let result = namespace_lookup(&vm, namespace, "result");
    assert_eq!(
        vm.instance.int_value(result),
        Some(8),
        "函数里定义的类：`__new__` 里的零参 `super().__new__(cls)` 要接到 `object.__new__`"
    );
}

#[test]
fn zero_arg_super_keeps_the_module_level_global_fast_path() {
    // 快路不回归：模块级类仍走"全局按名字查"；顺带覆盖"类对象存到**另一个**全局名"的慢路。
    let vm = Vm::new();
    let namespace = run_source(
        &vm,
        r#"
class A:
    def __init__(self, x):
        self.x = x

class B(A):
    def __init__(self, x):
        super().__init__(x * 2)

module_level = B(3).x

def make():
    class Base:
        def hi(self):
            return "hi"
    class Local(Base):
        def hi(self):
            return super().hi() + "!"
    return Local

K = make()
renamed = K().hi()
"#,
    );
    let module_level = namespace_lookup(&vm, namespace, "module_level");
    assert_eq!(
        vm.instance.int_value(module_level),
        Some(6),
        "模块级类的老路径（全局快路）不能回归"
    );
    assert_eq!(
        source_text(&vm, namespace, "renamed"),
        "hi!",
        "定义类被赋给别的全局名（K）时，慢路仍按 MRO 反查到 Local"
    );
}

#[test]
fn zero_arg_super_keeps_the_original_error_when_no_defining_class_is_found() {
    // **不吞错**：故意拿一个不属于定义类层次的对象当 `self` ⇒ 慢路在它的 MRO 上找不到
    // "持有当前 code 的类" ⇒ 保留原来的 `RuntimeError`（文案照旧）。
    // （参照这里给 `TypeError: super(type, obj): obj must be an instance or subtype of type`；
    //  本层慢路按"保留原报错"处理 —— 如实差异，别改成静默。）
    let vm = Vm::new();
    let outcome = try_run_source(
        &vm,
        r#"
class Base:
    pass

def make():
    class Local(Base):
        def m(self):
            return super().nope
    return Local

class Other:
    pass

make().m(Other())
"#,
    );
    assert!(outcome.is_err(), "拿无关对象当 self 时应当报错（不能静默）");
    let (type_name, message) = vm
        .pending_exception()
        .expect("报错后实例上应当挂着 pending exception");
    assert_eq!(type_name, "RuntimeError", "保留原来的异常类型");
    assert!(
        message
            .unwrap_or_default()
            .starts_with("super(): 全局里找不到定义类 Local"),
        "慢路找不到定义类时应当保留原文案"
    );
}

// --------------------------------------------------------------------------- #
// task-12：用户类富比较协议（`__lt__`／`__le__`／`__gt__`／`__ge__`）
// --------------------------------------------------------------------------- #

#[test]
fn rich_comparison_dispatches_to_user_methods() {
    // `<`／`<=`／`>`／`>=` 都要落到用户类的对应 dunder（含真／假两侧）
    let vm = Vm::new();
    let namespace = run_source(
        &vm,
        r#"
class K:
    def __init__(self, v):
        self.v = v
    def __lt__(self, other):
        return self.v < other.v
    def __le__(self, other):
        return self.v <= other.v
    def __gt__(self, other):
        return self.v > other.v
    def __ge__(self, other):
        return self.v >= other.v

hits = 0
if K(1) < K(2):
    hits += 1
if K(1) <= K(1):
    hits += 1
if K(2) > K(1):
    hits += 1
if K(2) >= K(2):
    hits += 1

misses = 0
if K(2) < K(1):
    misses += 1
if K(1) <= K(0):
    misses += 1
if K(1) > K(2):
    misses += 1
if K(0) >= K(1):
    misses += 1
"#,
    );
    let hits = namespace_lookup(&vm, namespace, "hits");
    assert_eq!(vm.instance.int_value(hits), Some(4), "四个正向比较都该为真");
    let misses = namespace_lookup(&vm, namespace, "misses");
    assert_eq!(vm.instance.int_value(misses), Some(0), "四个反向比较都该为假");
}

#[test]
fn rich_comparison_falls_back_to_the_reflected_method() {
    // `Left.__lt__` 返 `NotImplemented` ⇒ 照参照走 `Right.__gt__` ✓（反向派发）
    let vm = Vm::new();
    let namespace = run_source(
        &vm,
        r#"
class Left:
    def __lt__(self, other):
        return NotImplemented

class Right:
    def __gt__(self, other):
        return 1

via_reflected = 0
if Left() < Right():
    via_reflected = 1
"#,
    );
    let via_reflected = namespace_lookup(&vm, namespace, "via_reflected");
    assert_eq!(
        vm.instance.int_value(via_reflected),
        Some(1),
        "左操作数 NotImplemented ⇒ 必须走右操作数的 __gt__"
    );
}

#[test]
fn rich_comparison_without_any_method_keeps_the_reference_type_error() {
    // **不吞错**：两边都没有可比方法 ⇒ 原来的 `TypeError` 文案（与参照一致）
    let vm = Vm::new();
    let outcome = try_run_source(
        &vm,
        r#"
class Nothing:
    pass

Nothing() < Nothing()
"#,
    );
    assert!(outcome.is_err(), "不可比的用户类必须报错");
    let (type_name, message) = vm
        .pending_exception()
        .expect("报错后实例上应当挂着 pending exception");
    assert_eq!(type_name, "TypeError");
    assert_eq!(
        message.unwrap_or_default(),
        "'<' not supported between instances of 'Nothing' and 'Nothing'"
    );
}

#[test]
fn list_sort_uses_user_rich_comparison() {
    // task-12 的最小复现走的是 `sorted`／`list.sort` 那条（`Instance::order_of`）⇒ 现在也要派发协议
    let vm = Vm::new();
    let namespace = run_source(
        &vm,
        r#"
def make():
    class K:
        def __init__(self, v):
            self.v = v
        def __lt__(self, other):
            return self.v < other.v
    return K

K = make()
items = [K(3), K(1), K(2)]
items.sort()
result = items[0].v
"#,
    );
    let result = namespace_lookup(&vm, namespace, "result");
    assert_eq!(
        vm.instance.int_value(result),
        Some(1),
        "`list.sort()` 要按用户类的 __lt__ 排"
    );
}

#[test]
fn builtin_comparisons_keep_their_fast_paths() {
    // 内建快速路径不回归（`int`／`str`／`tuple`／`set`；用户类协议只在判不出时进）
    let vm = Vm::new();
    let namespace = run_source(
        &vm,
        r#"
hits = 0
if 1 < 2:
    hits += 1
if "a" < "b":
    hits += 1
if (1, 2) < (1, 3):
    hits += 1
if {1} < {1, 2}:
    hits += 1
if 1.5 > 1:
    hits += 1
"#,
    );
    let hits = namespace_lookup(&vm, namespace, "hits");
    assert_eq!(vm.instance.int_value(hits), Some(5), "内建比较快路径不能回归");
}

// --------------------------------------------------------------------------- #
// task-12 连带：函数里定义的类，**方法**引用外层函数局部（闭包要穿过类体转发）
// --------------------------------------------------------------------------- #

#[test]
fn a_method_of_a_function_local_class_captures_the_outer_local() {
    // 关键形态：**类体自己**不读 `base`、只有**方法体**读 ⇒ 编译器也要把 `base` 变成外层
    // cell、类体带 freevars、方法再接上闭包（先前只按"类体自己的 co_names"收 ⇒ 方法退化成
    // 全局读 ⇒ 运行期 `NameError`）。真 `functools.cmp_to_key` 的 `K.__lt__` 引用 `mycmp` 同型。
    let vm = Vm::new();
    let namespace = run_source(
        &vm,
        r#"
def make():
    base = 10
    class K:
        def __init__(self, v):
            self.v = v
        def __lt__(self, other):
            return base < other
        def plain(self):
            return base
    return K

K = make()
k = K(1)
plain_value = k.plain()
lt_value = 1 if k.__lt__(5) else 0
"#,
    );
    let plain_value = namespace_lookup(&vm, namespace, "plain_value");
    assert_eq!(
        vm.instance.int_value(plain_value),
        Some(10),
        "方法里的 `base` 必须闭包捕获外层局部（不是全局读）"
    );
    let lt_value = namespace_lookup(&vm, namespace, "lt_value");
    assert_eq!(
        vm.instance.int_value(lt_value),
        Some(0),
        "`__lt__` 里也要能读到闭包变量（10 < 5 ⇒ False）"
    );
}

// --------------------------------------------------------------------------- #
// task-17：`if/else` 非尾分支里最后一条是 `with` ⇒ 不能发作用域收尾（提前 return None）
// --------------------------------------------------------------------------- #

#[test]
fn a_tail_with_inside_an_if_else_branch_does_not_return_early() {
    // 分支体**不是尾块**时（`if` 后面还有 `return`），尾位 `with` 的正常出口只能跳到块尾，
    // 不能发 `LOAD_CONST None; RETURN_VALUE`（先前会把外层真正的 return 变成死代码）。
    let vm = Vm::new();
    let namespace = run_source(
        &vm,
        r#"
class Lock:
    def __enter__(self):
        return self
    def __exit__(self, *args):
        return False

LOCK = Lock()

def else_branch():
    if False:
        pass
    else:
        with LOCK:
            pass
    return 7

def then_branch():
    if True:
        with LOCK:
            pass
    else:
        pass
    return 11

def else_with_then_stmt():
    if False:
        pass
    else:
        with LOCK:
            pass
        z = 5
    return z

def mid_function(flag):
    if flag:
        a = 1
    else:
        with LOCK:
            a = 2
    b = 3
    return (a, b)

else_value = else_branch()
then_value = then_branch()
stmt_value = else_with_then_stmt()
mid_a, mid_b = mid_function(False)
"#,
    );
    let else_value = namespace_lookup(&vm, namespace, "else_value");
    assert_eq!(
        vm.instance.int_value(else_value),
        Some(7),
        "else 分支里的尾位 with 之后必须还能继续执行外层的 return"
    );
    let then_value = namespace_lookup(&vm, namespace, "then_value");
    assert_eq!(
        vm.instance.int_value(then_value),
        Some(11),
        "then 分支里的尾位 with 也不能提前 return"
    );
    let stmt_value = namespace_lookup(&vm, namespace, "stmt_value");
    assert_eq!(
        vm.instance.int_value(stmt_value),
        Some(5),
        "else 分支里 with 之后的语句要照常执行"
    );
    let mid_a = namespace_lookup(&vm, namespace, "mid_a");
    let mid_b = namespace_lookup(&vm, namespace, "mid_b");
    assert_eq!(vm.instance.int_value(mid_a), Some(2), "else 分支的执行结果");
    assert_eq!(vm.instance.int_value(mid_b), Some(3), "if/else 之后的语句要照常执行");
}

// --------------------------------------------------------------------------- #
// task-25：P3-35 语法缺口批 A —— **执行期**回归（行内套件／for-star／链式元组目标／
// with 属性目标）
// --------------------------------------------------------------------------- #

#[test]
fn task25_syntax_gaps_execute_with_reference_semantics() {
    let vm = Vm::new();
    let namespace = run_source(
        &vm,
        r#"
class Cm:
    def __enter__(self):
        return 7
    def __exit__(self, *args):
        return False

class Holder:
    pass

def inline(flag):
    try: a = 1
    except ValueError: a = 2
    else: a = 3
    finally: a = 4
    if flag: b = 10
    else: b = 20
    for i in [1]:
        pass
    else: b = b + 1
    return (a, b)

def star(items):
    first, *rest = items
    total = 0
    for name, *args in items:
        count = 0
        for extra in args:
            count = count + 1
        total = total + count
    return (first, rest, total)

def chained(a, b):
    x = _, y = a, b
    return (x, y)

def with_target(cm, holder):
    with cm as holder.value:
        pass
    return holder.value

inline_a, inline_b = inline(False)
star_first, star_rest, star_total = star([(1, 2, 3), (4, 5)])
chain_x, chain_y = chained(8, 9)
with_value = with_target(Cm(), Holder())
"#,
    );
    let inline_a = namespace_lookup(&vm, namespace, "inline_a");
    let inline_b = namespace_lookup(&vm, namespace, "inline_b");
    assert_eq!(vm.instance.int_value(inline_a), Some(4), "try/except/else/finally 行内体次序");
    assert_eq!(vm.instance.int_value(inline_b), Some(21), "if/else 行内体 + for 的 else 行内体");
    let star_total = namespace_lookup(&vm, namespace, "star_total");
    assert_eq!(vm.instance.int_value(star_total), Some(3), "`for name, *args in …` 的星号收余项");
    let star_rest_name = namespace_lookup(&vm, namespace, "star_rest");
    assert!(
        vm.instance.list_items(star_rest_name).is_some_and(|items| items.len() == 1),
        "`first, *rest = items` 的 rest 应当收到 1 项"
    );
    let chain_x = namespace_lookup(&vm, namespace, "chain_x");
    assert!(
        vm.instance.tuple_items(chain_x).is_some_and(|items| items.len() == 2),
        "链式赋值的目标可以是元组（`x = _, y = a, b` ⇒ x 收整个元组）"
    );
    let chain_y = namespace_lookup(&vm, namespace, "chain_y");
    assert_eq!(vm.instance.int_value(chain_y), Some(9), "链式元组目标里 y 的绑定");
    let with_value = namespace_lookup(&vm, namespace, "with_value");
    assert_eq!(vm.instance.int_value(with_value), Some(7), "`with cm as holder.value` 的属性目标");
}

// --------------------------------------------------------------------------- #
// task-32：**连续** `try`／`with` 块（≥16）⇒ 余部重放指数爆炸（StackUnderflow／挂死）
// --------------------------------------------------------------------------- #

#[test]
fn consecutive_try_blocks_do_not_overflow_or_hang() {
    // 修复前 N=16 ⇒ `帧操作失败：StackUnderflow`、N=17 ⇒ 挂死（每个 try 的处理块把"后面所有
    // try"再复制一份 ⇒ 2^N 代码量）。修复后：处理块正常完成改为**跳回余部副本起点**（与参照
    // `two_tries` 的 `POP_EXCEPT; JUMP_BACKWARD_NO_INTERRUPT` 同形）⇒ 代码量线性。
    let mut source = String::from("v = 0\n");
    for index in 0..64 {
        source.push_str(&format!(
            "try:\n    v = {index}\nexcept BaseException:\n    pass\n"
        ));
    }
    source.push_str("module_result = v\n");
    // 函数里同样来一遍（作用域收尾那条路也要走 ✓）
    source.push_str("def f():\n    v = 0\n");
    for index in 0..64 {
        source.push_str(&format!(
            "    try:\n        v = {index}\n    except BaseException:\n        pass\n"
        ));
    }
    source.push_str("    return v\nfunction_result = f()\n");
    let vm = Vm::new();
    let namespace = run_source(&vm, &source);
    let module_result = namespace_lookup(&vm, namespace, "module_result");
    assert_eq!(
        vm.instance.int_value(module_result),
        Some(63),
        "64 个连续 try/except：模块级"
    );
    let function_result = namespace_lookup(&vm, namespace, "function_result");
    assert_eq!(
        vm.instance.int_value(function_result),
        Some(63),
        "64 个连续 try/except：函数内"
    );
}

#[test]
fn consecutive_with_blocks_do_not_overflow_or_hang() {
    // 同一机制也压 `with`：清理块（`index == 0`）先前照旧重放余部 ⇒ 连续 with 同样指数爆炸。
    let mut source = String::from(
        "class CM:\n    def __enter__(self):\n        return 1\n    def __exit__(self, *args):\n        return False\ncm = CM()\nv = 0\n",
    );
    for index in 0..64 {
        source.push_str(&format!("with cm:\n    v = v + {index}\n"));
    }
    source.push_str("module_result = v\n");
    source.push_str("def f():\n    v = 0\n");
    for index in 0..64 {
        source.push_str(&format!("    with cm:\n        v = v + {index}\n"));
    }
    source.push_str("    return v\nfunction_result = f()\n");
    let vm = Vm::new();
    let namespace = run_source(&vm, &source);
    let module_result = namespace_lookup(&vm, namespace, "module_result");
    assert_eq!(
        vm.instance.int_value(module_result),
        Some(2016),
        "64 个连续 with：模块级（0+…+63 = 2016）"
    );
    let function_result = namespace_lookup(&vm, namespace, "function_result");
    assert_eq!(
        vm.instance.int_value(function_result),
        Some(2016),
        "64 个连续 with：函数内"
    );
}
