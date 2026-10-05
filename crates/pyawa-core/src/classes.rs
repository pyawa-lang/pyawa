//! 类创建：`LOAD_BUILD_CLASS` 压的 `__build_class__`（`OM-14` 的类创建钩子落点）。
//!
//! 实测的发射形状（`class C(Base): …`）：
//!
//! ```text
//! LOAD_BUILD_CLASS; PUSH_NULL; LOAD_CONST <类体 code>; MAKE_FUNCTION;
//! LOAD_CONST 'C'; LOAD_NAME Base; CALL 3; STORE_NAME C
//! ```
//!
//! 也就是说：**类体是一段"局部变量放在映射里"的代码**（`LOAD_NAME`／`STORE_NAME`），
//! `__build_class__` 拿到"类体函数 ＋ 名字 ＋ 基类"，跑完类体后建类型。
//!
//! **尚未接线**：`metaclass=`（非 `type` 的元类）、`__prepare__`、`__set_name__`
//! （要描述符）、`__mro_entries__`（`AB-37` 的宿主类型继承那一侧）。

use core::cell::Cell;
use core::ptr::NonNull;

use crate::builtin_objects::{BuiltinFunctionObject, DictObject, StrObject};
use crate::executor::{raise_builtin, ExecError};
use crate::instance::Instance;
use crate::type_object::TypeObject;
use crate::{Frame, Header, NativeFn};

/// `__build_class__(body, name, *bases, metaclass=None, **kwds)`（原生实现）。
///
/// # Safety
///
/// 契约见 [`NativeFn`]：实参是借用视图、返回值是新引用。
pub unsafe fn build_class_native(
    instance: &Instance,
    _bound: Option<NonNull<Header>>,
    args: &[NonNull<Header>],
    kwargs: &[(NonNull<Header>, NonNull<Header>)],
) -> Result<NonNull<Header>, ExecError> {
    if args.len() < 2 {
        return Err(raise_builtin(
            instance,
            "TypeError",
            "__build_class__: not enough arguments",
        ));
    }
    let body = args[0];
    let name_object = args[1];
    let bases: Vec<NonNull<Header>> = args[2..].to_vec();

    // 名字必须是 str（实测消息：`__build_class__: name is not a string`）
    // SAFETY: 调用方保证实参存活。
    if unsafe { name_object.as_ref() }.ty() != instance.singletons().str_type() {
        return Err(raise_builtin(
            instance,
            "TypeError",
            "__build_class__: name is not a string",
        ));
    }
    // SAFETY: 类型身份已确认。
    let name = unsafe { &*name_object.as_ptr().cast::<StrObject>() }.value().to_owned();

    // **要接的自定义元类** ✓（第 218 轮）：`class X(metaclass=M)` ✓。
    let mut requested_metaclass: Option<NonNull<TypeObject>> = None;
    // **转交给元类的类关键字** ✓（第 292 轮）：除 `metaclass=` 之外**原样**留着 ✓（键对象照用 ✓，
    // `call_value` 会自己 retain ✓）。`Lib/enum.py:1400` 的 `boundary=STRICT` 就是它 ✓。
    let mut forwarded_keywords: Vec<(NonNull<Header>, NonNull<Header>)> = Vec::new();
    // `metaclass=`：默认元类直接放行 ✓；**自定义元类**接住 ✓
    for (key, _value) in kwargs {
        // SAFETY: 键由调用方保证存活。
        let key_type = unsafe { key.as_ref() }.ty();
        if key_type == instance.singletons().str_type() {
            // SAFETY: 同上。
            let text = unsafe { &*key.as_ptr().cast::<StrObject>() }.value();
            if text == "metaclass" {
                // **等于默认元类时直接放行** ✓（第 159 轮）：参照 `class C(metaclass=type)` 与不写**完全等价** ✓。
                //   认两种写法 ✓（`type` 的类型对象 ✓／内建里那个 `type` 名 ✓ —— 本层 `type()` 是 native ✓）；
                //   其余元类仍**如实报未接线** ✗（不静默当默认元类 ✓）。
                let wanted = _value.cast::<Header>();
                let default_type = instance
                    .type_named("type")
                    .map(|ty| ty.cast::<Header>())
                    .is_some_and(|ty| ty == wanted);
                let builtin_type = instance
                    .builtins()
                    .and_then(|builtins| instance.dict_get(builtins, "type"))
                    .is_some_and(|entry| entry == wanted);
                if default_type || builtin_type {
                    continue;
                }
                // **自定义元类** ✓（第 218 轮）：必须是**类型对象** ✓，接住它 ✓；
                // 建完类再把新类的**元类型**设成它 ✓（见本函数末尾 ✓）。
                // **如实说** ✗：参照的做法是调 `M(name, bases, namespace, **kwds)` ✓，
                // 本层**暂时**只做"元类型对"这一步 ✓ —— `M.__new__`／`__init__`／`__prepare__`
                // 尚未被调用 ✗（`ABC` 一族的注册表要等那一步 ✓）。
                if !instance.is_type_object(wanted) {
                    return Err(raise_builtin(instance, "TypeError", "metaclass must be a type"));
                }
                requested_metaclass = Some(wanted.cast::<TypeObject>());
                continue;
            }
        }
        // `metaclass=` 之外的：留着转交 ✓
        forwarded_keywords.push((*key, *_value));
    }

    // **元类从基类推导** ✓（第 292 轮，照参照的 OM-13 口径）：没写 `metaclass=` 时，
    // 取基类里**最派生**的那个元类型 ✓（`class Flag(Enum, boundary=STRICT)` 的元类是
    // `Enum` 的元类 `EnumType` ✓ —— 先前只认显式 `metaclass=` ✗ ⇒ `EnumType.__new__`
    // 从来不被调用 ✗ ⇒ 枚举成员一个也收不上来 ✓）。
    if requested_metaclass.is_none() {
        let mut candidate = instance.metatype();
        for base in &bases {
            // SAFETY: 基类由调用方保证存活。
            let base_type = unsafe { base.as_ref() }.ty();
            if instance.is_subtype(base_type, candidate) {
                candidate = base_type;
            }
        }
        if candidate != instance.metatype() {
            requested_metaclass = Some(candidate);
        }
    }

    // **类命名空间**（第 298 轮补 `__prepare__` 那一格）：默认是一个空 `dict` ✓；
    // 元类**自带** `__prepare__`（不是我们挂在 `type` 上的那个）就照参照**先调它** ✓ ——
    // `M.__prepare__(name, bases, **kwds)` ✓，用它返回的**映射**当命名空间 ✓。
    //
    // 动因：`Lib/enum.py` 的 `EnumType.__prepare__` 返回 `EnumDict`（`dict` 的子类 ✓），
    // `EnumType.__new__` 头一句 `classdict._member_names` 全指望它 ✓ —— 先前只造普通 `dict` ✗
    // ⇒ 报 `AttributeError: 'dict' object has no attribute '_member_names'` ✗（那一族 **42** 个模块 ✓）。
    let default_prepare = instance
        .type_named("type")
        .and_then(|ty| instance.type_lookup(ty, "__prepare__"));
    // **取"真正可调"的那个 `__prepare__`** ✓：`type_lookup` 给的是**未绑定**的描述符 ✗ ——
    // `@classmethod` 直接调会报 `'classmethod' object is not callable` ✗（实测 ✓）⇒
    // 认出来之后走**完整属性协议**（`attribute_optional` ✓）拿绑定好的可调用对象 ✓。
    let mut custom_prepare: Option<(NonNull<TypeObject>, NonNull<Header>)> = None;
    if let Some(metaclass) = requested_metaclass {
        let is_custom = instance
            .type_lookup(metaclass, "__prepare__")
            .is_some_and(|method| Some(method) != default_prepare);
        if is_custom {
            if let Some(method) = crate::executor::attribute_optional(
                instance,
                metaclass.cast::<Header>(),
                "__prepare__",
            )? {
                custom_prepare = Some((metaclass, method));
            }
        }
    }
    let namespace = match custom_prepare {
        Some((metaclass, method)) => {
            // **`@classmethod` 的绑定交给属性那一层**（第 303 轮修 `P3-25`）：`attribute_lookup`
            // 现在自己认得 `classmethod` 并交回**绑好元类**的形态 ✓ ⇒ 这里**不再**手工取
            // `ClassMethodObject::function` ✗、也不再补元类实参 ✗（第 298 轮的绕行已作废 ✓：
            // 那两下一起上就是"4 个实参" ⇒ `__prepare__() takes 3 positional arguments but 4 were given` ✗）。
            let name_value = instance.new_str(&name);
            let mut base_values: Vec<NonNull<Header>> = Vec::with_capacity(bases.len());
            for base in &bases {
                // SAFETY: 基类由调用方保证存活；元组要自己那份引用。
                unsafe { instance.incref_object(base.as_ptr()) };
                base_values.push(*base);
            }
            let bases_value = instance.new_tuple(base_values);
            let _ = metaclass;
            let prepared = crate::executor::call_value(
                instance,
                method,
                &[name_value, bases_value],
                &forwarded_keywords,
            )?;
            // **`__prepare__` 那一支的探针**（第 94 轮，`PYAWA_PREPARE_DEBUG=1` ✓）：上限榜 `-6` 族的
            // 病灶在第 93 轮被收窄到"**这个 `prepared` 映射**"（`Lib/enum.py` 的 `EnumDict` ✓，
            // `dict` 的子类 ✓）⇒ 这里看清三件：**类型名** ✓、**布局对不对**（`instance_size` 与
            // `DictObject` 的 Rust 布局对照 ✓）、以及当 `DictObject` 读时的 `len`／`cap`／`ptr` ✓。
            if std::env::var_os("PYAWA_PREPARE_DEBUG").is_some() {
                let ty = instance.type_of(prepared);
                let name = instance.type_name(ty);
                // SAFETY: ty 由注册表持有。
                let instance_size = unsafe { ty.as_ref() }.instance_size();
                let dict_size = core::mem::size_of::<DictObject>();
                let is_dict = instance
                    .type_named("dict")
                    .map(|dict| instance.is_subtype(ty, dict))
                    .unwrap_or(false);
                // SAFETY: prepared 由调用方保证存活。
                let payload = unsafe { &*prepared.as_ptr().cast::<DictObject>() };
                let entries = payload.entries();
                eprintln!(
                    "[prepare 探针] prepared={prepared:p} 类型={name} 是dict子类={is_dict} instancesize={instance_size} DictObject={dict_size} len={} cap={} ptr={:p}",
                    entries.len(),
                    entries.capacity(),
                    entries.as_ptr()
                );
            }
            // `attribute_optional` 给的是**自己那一份**引用 ⇒ 用完交还 ✓。
            instance.release(method);
            prepared
        }
        None => instance
            .alloc(DictObject::new(
                instance
                    .type_named("dict")
                    .expect("dict 在引导期已登记"),
                core::cell::RefCell::new(Vec::new()),
            ))
            .into_raw()
            .cast::<Header>(),
    };

    // 跑类体：它的局部变量就是这个命名空间（`LOAD_NAME`／`STORE_NAME`）
    crate::executor::run_class_body(instance, body, namespace)?;

    // **元类真正被调用** ✓（第 234 轮）：`metaclass=M` 且 M **自带** `__new__`（不是我们挂的那个
    // `type.__new__` ✓）⇒ 照参照**调它** ✓：`M.__new__(M, name, bases, namespace)` ✓
    //（`Lib/abc.py` 的 `ABCMeta.__new__` 正是这样把自己那一套登记做掉的 ✓ —— `_abc_impl` ✓）。
    // 放在这里而**不**放进核心 ✓：核心若也去调元类 ⇒ **自己调自己** ✗（死循环 ✓）。
    if let Some(metaclass) = requested_metaclass {
        // **自带 `__new__`／`__init__` 才调** ✓：默认那两个（我们挂在 `type` 命名空间里的 ✓）
        // 要跳过 ✗，否则**自己调自己** ⇒ 死循环 ✓。
        let default_new = instance
            .type_named("type")
            .and_then(|ty| instance.type_lookup(ty, "__new__"));
        let default_init = instance
            .type_named("type")
            .and_then(|ty| instance.type_lookup(ty, "__init__"));
        let custom_new = instance
            .type_lookup(metaclass, "__new__")
            .filter(|method| Some(*method) != default_new);
        let custom_init = instance
            .type_lookup(metaclass, "__init__")
            .filter(|method| Some(*method) != default_init);
        if custom_new.is_some() || custom_init.is_some() {
            let name_value = instance.new_str(&name);
            let mut base_values: Vec<NonNull<Header>> = Vec::with_capacity(bases.len());
            for base in &bases {
                // SAFETY: 基类由调用方保证存活；元组要自己那份引用。
                unsafe { instance.incref_object(base.as_ptr()) };
                base_values.push(*base);
            }
            let bases_value = instance.new_tuple(base_values);
            // **`__new__`**：自带的那个才调 ✓（`M.__new__(M, name, bases, namespace, **kwds)` ✓）；
            // 没有自带 ⇒ 走**建类核心**（＝ `type.__new__` 的效果 ✓）。
            // **命名空间的归属**（第 292 轮踩到 ✓）：`build_class_from_parts` 会**吃掉**
            // 调用方那一份（它末尾就 `release_object(namespace)` ✓）；而参照的 `__init__`
            // 还要拿到它 ✓ ⇒ 这里先**自己再留一份** ✓，`__init__` 用完交还 ✓
            //（先前直接用原来那份 ✗ ⇒ "对已释放对象 incref" ⇒ 堆崩 `malloc(): unaligned tcache chunk` ✗）。
            // **引用计数探针**（第 111 轮，`PYAWA_NS_DEBUG=1`）：命名空间在"交给元类前后"各有多少份 ✓
            // —— 毒化档说它在 `EnumType.__new__@540`（`classdict = dict(classdict.items())` 那句重绑）
            // 被放到 0 ✓ ⇒ 重绑之前除帧那份已无人持有 ⇒ **调用方那份早就没了** ✗ ⇒ 就在这段里逐点读 ✓。
            if std::env::var_os("PYAWA_NS_DEBUG").is_some() {
                eprintln!(
                    "[ns 探针] 交元类之前 namespace={namespace:p} rc={}",
                    instance.refcount_of(namespace)
                );
                instance.watch_address(namespace);
            }
            let namespace_for_init = instance.retain(namespace);
            let result = match custom_new {
                Some(new_method) => {
                    let built = crate::executor::call_value(
                        instance,
                        new_method,
                        &[
                            metaclass.cast::<Header>(),
                            name_value,
                            bases_value,
                            namespace,
                        ],
                        &forwarded_keywords,
                    )?;
                    // **这条路不吃命名空间** ⇒ 调用方那一份留着**不动** ✓（与第 234 轮的既有口径一致 ✓：
                    // `M.__new__` 是否"接管"了它由那个元类自己决定 ✓ ⇒ 这里**不还** ✗，免得把
                    // 类字典里那一份打掉 ✓（实测：这里 `release` 会当场把类字典打空 ⇒ 段错误 ✗）。
                    built
                }
                None => build_class_from_parts(
                    instance,
                    name.clone(),
                    bases.clone(),
                    namespace,
                    requested_metaclass,
                )?,
            };
            if std::env::var_os("PYAWA_NS_DEBUG").is_some() {
                eprintln!(
                    "[ns 探针] 元类返回之后 namespace={namespace:p} rc={}",
                    instance.refcount_of(namespace)
                );
            }
            // **`__init__`**：照参照的 `type.__call__` 次序，`__new__` 之后也调它 ✓
            //（`Lib/enum.py` 的 `EnumType` 两半都有 ✓）。
            if let Some(init_method) = custom_init {
                let outcome = crate::executor::call_value(
                    instance,
                    init_method,
                    &[
                        result,
                        name_value,
                        bases_value,
                        namespace,
                    ],
                    &forwarded_keywords,
                )?;
                // `__init__` 的返回值照参照**丢掉** ✓（`type.__call__` 只认 `__new__` 的结果 ✓）
                instance.release(outcome);
            }
            instance.release(namespace_for_init);
            return Ok(result);
        }
    }

    // **建类核心已抽出** ✓（第 234 轮）：`type.__new__` 与"元类真被调用"两条路都要用它 ✓。
    build_class_from_parts(instance, name, bases, namespace, requested_metaclass)
}

/// 造一个"原生可调用对象"形态的 `__build_class__`（给测试与将来的 `builtins` 用）。
#[allow(dead_code)] // 引导期用等价的一份；这里留给测试与将来的 `builtins`
pub fn native_build_class(instance: &Instance) -> NonNull<Header> {
    let object = instance.alloc(BuiltinFunctionObject::new(
        instance
            .type_named("builtin_function_or_method")
            .expect("引导期已登记"),
        "__build_class__",
        Cell::new(build_class_native as NativeFn),
    ));
    object.into_raw().cast::<Header>()
}

/// 帧：把命名空间交给类体（供 [`crate::executor::run_class_body`] 用）。
///
/// **`BC-4`**：把一个类体条目里的**函数**换成"`co_qualname` 已补成 `C.m`"的新函数。
///
/// 不是函数、或名字读不出来时给 `None`（调用方原样搬）。新函数与旧函数共享默认值／
/// `__globals__`（各新增引用，`OM-16`）。
fn requalified_method(
    instance: &Instance,
    class_name: &str,
    key: NonNull<Header>,
    value: NonNull<Header>,
) -> Option<NonNull<Header>> {
    let function_type = instance.type_named("function")?;
    // SAFETY: 调用方保证 key／value 存活。
    if unsafe { value.as_ref() }.ty() != function_type {
        return None;
    }
    // SAFETY: 键是 str（类命名空间的键）。
    let method_name = unsafe { &*key.as_ptr().cast::<crate::StrObject>() }
        .value()
        .to_owned();
    // SAFETY: 上面刚确认是函数对象。
    let function = unsafe { &*value.as_ptr().cast::<crate::FunctionObject>() };
    let code_header = function.code();
    // SAFETY: 函数持有 code 的一份引用，存活。
    let code = unsafe { &*code_header.as_ptr().cast::<crate::CodeObject>() };
    // 已经是限定名（`C.m`）就不必再换
    let expected = format!("{class_name}.{method_name}");
    if code.qualname() == expected {
        return None;
    }
    let new_code = instance.code_with_qualname(code, expected);
    let mut defaults = Vec::with_capacity(function.defaults().len());
    for default in function.defaults() {
        // SAFETY: 默认值由旧函数持有，新函数要自己那份。
        unsafe { instance.incref_object(default.as_ptr()) };
        defaults.push(*default);
    }
    let kwdefaults = function.kwdefaults();
    if let Some(mapping) = kwdefaults {
        // SAFETY: 同上。
        unsafe { instance.incref_object(mapping.as_ptr()) };
    }
    let globals = function.globals();
    if let Some(mapping) = globals {
        // SAFETY: 同上。
        unsafe { instance.incref_object(mapping.as_ptr()) };
    }
    // **换 qualname 不改注解**：把旧函数那份 `__annotate__` 引用接过来（`SET_FUNCTION_ATTRIBUTE`
    // 的 bit4；丢了它，方法的延迟注解就断了）
    let annotate = function.annotate();
    if let Some(callable) = annotate {
        // SAFETY: 该引用由旧函数持有，新函数要自己那份。
        unsafe { instance.incref_object(callable.as_ptr()) };
    }
    // 返回值**带着一份引用**（调用方在 `Some` 分支里**不再** incref，字典 `insert_raw` 接手这份）
    Some(
        instance
            .alloc(crate::FunctionObject::new(
                function_type,
                new_code,
                defaults,
                kwdefaults,
                core::cell::RefCell::new(globals),
                core::cell::RefCell::new(Vec::new()),
                core::cell::RefCell::new(annotate),
            core::cell::RefCell::new(None)))
            .into_raw()
            .cast::<Header>(),
    )
}

/// 帧**自己持有一份**命名空间引用（`Frame::for_code_with_namespace` 接手的是新引用），
/// 所以这里先新增一份——少了它，帧一析构命名空间就没了（调用方那一份不算）。
pub(crate) fn class_body_frame(
    instance: &Instance,
    code: NonNull<Header>,
    namespace: NonNull<Header>,
    globals: Option<NonNull<Header>>,
) -> NonNull<Frame> {
    let frame_type = instance
        .type_named("frame")
        .expect("Frame 在引导期已登记");
    // `Owned::new` 收的是**新引用**（`OM-16`：守卫的 `Drop` 会释放它）⇒ 这里必须自己新增一份。
    // 少了这一步，守卫析构时会释放**函数自己那份** code 引用（症状：随机时刻
    // "对已释放对象 decref"，进而在 glibc 线程退出检查里变成 tcache 堆损坏）。
    // SAFETY: code 由调用方保证存活（它来自类体函数持有的 code object）。
    unsafe { instance.incref_object(code.as_ptr()) };
    let code_reference = crate::Owned::new(code.cast::<crate::CodeObject>(), instance);
    // SAFETY: namespace 由调用方保证存活；这里为帧新增一份引用。
    unsafe { instance.incref_object(namespace.as_ptr()) };
    let frame = instance.alloc(Frame::for_code_with_namespace(
        frame_type,
        &code_reference,
        namespace,
    ));
    // **类体的全局层**（`LOAD_NAME` 的第二层）：取**类体函数**记着的 `__globals__`
    // （`MAKE_FUNCTION` 时捕获），所以类体里能读到模块级名字。
    if let Some(mapping) = globals {
        // 帧接手的是**新引用**（`Frame::clear` 会释放它）
        // SAFETY: mapping 由类体函数持有，存活。
        unsafe { instance.incref_object(mapping.as_ptr()) };
        frame.get().set_globals(mapping);
    }
    frame.into_raw().cast::<Frame>()
}

/// **按"基类 ＋ 命名空间 ＋ 元类"建一个类** ✓（第 234 轮从 `build_class_native` 抽出 ✓，**一处真相** ✓）。
///
/// 两条路共用它 ✓：① `__build_class__`（跑完类体之后 ✓）；② `type.__new__(mcls, name, bases, ns)`
/// （元类真正被调用时 ✓ —— `Lib/abc.py` 的 `ABCMeta.__new__` 里那句 `super().__new__(…)` 就到这儿 ✓）。
/// **参数名与原来那段的局部同名** ✓ ⇒ 抽出时函数体**一字未改** ✓，行为完全一致 ✓。
pub fn build_class_from_parts(
    instance: &Instance,
    name: String,
    bases: Vec<NonNull<Header>>,
    namespace: NonNull<Header>,
    requested_metaclass: Option<NonNull<TypeObject>>,
) -> Result<NonNull<Header>, ExecError> {
    // ---- 基类与布局（`OM-14`／`AB-37`／`AB-58`）----
    //
    // 顺序有意如此：**布局要先定下来**，类型对象才能按正确的 `instance_size` 与槽位建出来。
    let mut base_types: Vec<NonNull<TypeObject>> = Vec::new();
    // **本类最终用的元类型** ✓（第 231 轮）：先给默认 ✓，再按"**最派生**"取 ✓（参照的 OM-13 口径 ✓）。
    // （先前那次"设了元类型就段错误" ✗ 的**真因**是 `is_type_object` 的判据太严 ✓，**不是**传播本身 ✓ →
    //  修好判据后传播可以照装 ✓。）
    let mut effective_metaclass = instance.metatype();
    for base in &bases {
        // SAFETY: base 由调用方保证存活。
        let base_type = unsafe { base.as_ref() }.ty();
        // **放宽到"元类型是 `type` 的子类"** ✓（第 231 轮真 bug 修复 ✗）：先前只认"恰为 `type`" ✗
        // ⇒ 一旦**继承**一个用 `metaclass=ABCMeta` 建的类（`_collections_abc.py` 里比比皆是 ✓）
        // 就报 `bases must be types` ✗。参照允许基类的元类型是 `type` 的**子类** ✓。
        if !instance.is_subtype(base_type, instance.metatype()) {
            // **带上那个基类的类型与 repr** ✓（第 231 轮）：不然只有一句"bases must be types" ✗，定位全靠猜 ✓。
            let what = format!(
                "bases must be types（基类 {} 值 {}）",
                instance.type_name(instance.type_of(*base)),
                instance
                    .object_repr(*base)
                    .unwrap_or_else(|_| "<读不出>".to_owned())
            );
            return Err(raise_builtin(instance, "TypeError", &what));
        }
        // **最派生** ✓：若这个基类的元类型比当前的更派生 ⇒ 换成它 ✓。
        if base_type != effective_metaclass && instance.is_subtype(base_type, effective_metaclass) {
            effective_metaclass = base_type;
        }
        // SAFETY: base_type 由注册表持有。
        // 注意：`base_type` 是**基类自己的类型**（即元类型 `type`）——要查的是**基类本身**
        // SAFETY: base 是存活对象，且上面确认过它是类型对象。
        let info = unsafe { &*base.as_ptr().cast::<TypeObject>() };
        // **`AB-37`**：`PA_TYPE_FINAL` 的类型不可作基类（消息照参照实现）
        if info.is_final() {
            let message = format!("type '{}' is not an acceptable base type", info.name());
            return Err(raise_builtin(instance, "TypeError", &message));
        }
        base_types.push(base.cast::<TypeObject>());
    }
    if base_types.is_empty() {
        base_types.push(instance.type_named("object").expect("object 已登记"));
    }
    // 冗余基类去掉（`class X(Sub, Base)` 合法；`Base` 已被 `Sub` 覆盖）
    let mut pruned: Vec<NonNull<TypeObject>> = Vec::new();
    for (index, candidate) in base_types.iter().enumerate() {
        let redundant = base_types.iter().enumerate().any(|(other, base)| {
            other != index && {
                // SAFETY: 两者都由注册表持有。
                instance.is_subtype(*base, *candidate)
            }
        });
        if !redundant {
            pruned.push(*candidate);
        }
    }
    // **`AB-58`**：定长宿主布局只能有一个（参照实现：`multiple bases have instance lay-out conflict`）
    let mut host_base: Option<NonNull<TypeObject>> = None;
    let mut layout_conflict = false;
    for base in &pruned {
        // SAFETY: base 由注册表持有。
        if unsafe { base.as_ref() }.is_host_layout() {
            if host_base.is_some() {
                layout_conflict = true;
                break;
            }
            host_base = Some(*base);
        }
    }
    if layout_conflict {
        return Err(raise_builtin(
            instance,
            "TypeError",
            "multiple bases have instance lay-out conflict",
        ));
    }

    // 建类型：名字要 `&'static str`（`TypeObject::name` 的临时形态）——
    // 这里把名字**泄漏**成静态串（每建一个类泄漏一次，`TS-43` 的最终形态是 `str` 对象）
    let static_name: &'static str = Box::leak(name.clone().into_boxed_str());
    // **VM 内建里"带布局"的基类**（第 99 轮真 bug 修 ✗）：`class D(dict)` 这种**必须**沿用
    // `DictObject` 的布局与 `dict_new` ✓ —— 否则落到通用 `AttributeObject` 布局 ✗
    // ⇒ 实例根本不是 `DictObject` ✓ ⇒ 按它读"长度／容量／指针"读到的是那块内存里别的东西
    // （ASCII 怪数字 ✓）⇒ 上限榜 `-6`（SIGABRT）族 **116** 个模块 ✓
    //（`Lib/enum.py` 的 `EnumDict(dict)` 正是这个形态 ✓）。
    // 本轮先纳入 **`dict`**（族里最大的那一支 ✓）；`list`／`tuple`／`set` 照同一判据随后补 ✓。
    // 判据：这个基类**自带 `new` 槽**（⇒ 它的载荷由它自己建 ✓），且属于"有布局的内建"一族 ✓。
    // 第 99 轮先纳 `dict` ✓（`Lib/enum.py` 的 `EnumDict` ✓）；第 104 轮把 `list`／`tuple`／`set`／
    // `deque` 照**同一判据**一并纳入 ✓ —— 子类实例化要沿用基类的载荷 ✓。
    let builtin_layout_base = if host_base.is_none() {
        ["dict", "list", "tuple", "set", "frozenset", "deque"]
            .iter()
            .find_map(|name| {
                let family = instance.type_named(name)?;
                pruned.iter().copied().find(|base| {
                    // SAFETY: base 由注册表持有。
                    instance.is_subtype(*base, family)
                        && unsafe { base.as_ref() }.slots().new.is_some()
                })
            })
    } else {
        None
    };
    let ty = match host_base.or(builtin_layout_base) {
        // **`AB-58`／`AB-37`**：宿主类型的 Python 子类**继承同一布局**——载荷按**同一尺寸**
        // 由 VM 分配（宿主无需参与），槽位沿用基类的 `dealloc`／`traverse`／终结器；
        // **没有**默认 `new`：宿主类型实例由宿主经 `pa_newhandle` 建（AB-58）
        Some(base) => {
            // SAFETY: base 由注册表持有。
            let base_info = unsafe { base.as_ref() };
            let slots = base_info.slots().inherit_host_layout_with_new();
            let created = instance.new_type(static_name, base_info.instance_size(), slots);
            // 宿主钩子（`dealloc`／`traverse`）随布局一起继承
            if let (Some(dealloc), Some(traverse)) =
                (base_info.host_dealloc(), base_info.host_traverse())
            {
                // SAFETY: created 由注册表持有。
                unsafe { created.as_ref() }.set_host_hooks(dealloc, traverse);
            }
            // 布局固定 ⇒ 实例字典**另行挂载**（`OM-14`）
            // SAFETY: 同上。
            unsafe { created.as_ref() }.mark_external_instance_dict();
            created
        }
        None => {
            let created = instance.new_type(
                static_name,
                core::mem::size_of::<crate::AttributeObject>(),
                crate::AttributeObject::slots().with_new(crate::builtin_objects::attribute_new),
            );
            // SAFETY: created 由注册表持有。
            unsafe { created.as_ref() }.mark_has_instance_dict();
            created
        }
    };

    if instance.register_bases(ty, pruned).is_none() {
        return Err(raise_builtin(
            instance,
            "TypeError",
            "Cannot create a consistent method resolution order (MRO) for bases",
        ));
    }

    // 把类体的命名空间搬进**类型字典**（`OM-11` 的属性通道就是查它）
    // SAFETY: namespace 是刚造的 dict。
    let mapping = unsafe { &*namespace.as_ptr().cast::<DictObject>() };
    let entries = mapping.entries();
    // 把类命名空间搬进**类型字典**（`OM-11` 的属性通道就是查它）。
    //
    // 键**原样放进**（不另造一份）：`insert_raw` 是转移语义 ⇒ 各新增一份引用；
    // 两个字典共享同一个键对象是正常的（参照实现里键都是 intern 的字符串）。
    //
    // 历史记录：这里曾经观察到**随机堆损坏**，一度被误判为"键共享"引起（当时用"各自持键"绕开）。
    // 真因是 `class_body_frame` 里少了一次 `incref`：`Owned::new` 收的是**新引用**，
    // 我却把类体函数自己那份 code 引用交了出去，守卫析构时多释放一次。修好之后
    // 共享键版本连跑 20 次全绿。
    let type_dict = match unsafe { ty.as_ref() }.dict() {
        Some(mapping) => mapping,
        None => {
            let mapping = instance
                .alloc(DictObject::new(
                    instance.type_named("dict").expect("dict 已登记"),
                    core::cell::RefCell::new(Vec::new()),
                ))
                .into_raw()
                .cast::<Header>();
            // SAFETY: ty 由注册表持有。
            unsafe { ty.as_ref() }.set_dict(Some(mapping));
            mapping
        }
    };
    for (key, value) in entries {
        // **`BC-4`**：类体里的**函数**要把 `co_qualname` 补成 `C.m`——参照实现由**编译器**写死，
        // 编译器**已经有类体**（且会写死 `C.m`）⇒ 这里只在**不匹配**时才补（换一份 code 再包新函数；见
        // `Instance::code_with_qualname` 的说明）。
        let replacement = requalified_method(instance, &name, key, value);
        // SAFETY: 键值由命名空间持有，各新增一份（insert_raw 是转移语义）；
        // 替换出来的新函数**自带一份**，故那时不再 incref。
        unsafe {
            instance.incref_object(key.as_ptr());
            match &replacement {
                Some(_) => {}
                None => instance.incref_object(value.as_ptr()),
            }
        }
        let value = replacement.unwrap_or(value);
        // SAFETY: type_dict 由类型对象持有，存活。
        unsafe { &*type_dict.as_ptr().cast::<DictObject>() }.insert_raw(key, value);
        // **`__set_name__`**（实测 3.14）：对**本类自己命名空间**的每一项（按**插入序**），
        // 若它有 `__set_name__` 就调一次 `(类对象, 属性名)`——继承项不在此循环里，故不会重调；
        // 没有该方法的项跳过；**抛错原样传播**（本函数本来就返回 `Result`）。
        if instance.text_value(key).is_some() {
            match crate::executor::attribute_optional(instance, value, "__set_name__") {
                Ok(Some(setter)) => {
                    let class_value = ty.cast::<Header>();
                    // SAFETY: setter 是刚取到的新引用；key 由命名空间持有。
                    let outcome = crate::executor::call_value(
                        instance,
                        setter,
                        &[class_value, key],
                        &[],
                    );
                    instance.release(setter);
                    match outcome {
                        Ok(result) => instance.release(result),
                        Err(error) => return Err(error),
                    }
                }
                Ok(None) => {}
                Err(error) => return Err(error),
            }
        }
    }
    // SAFETY: namespace 由本函数持有。
    unsafe { instance.release_object(namespace.as_ptr()) };

    // **元类型落在类对象上** ✓（第 218 轮 ＋ 第 231 轮）：
    // 显式 `metaclass=` 优先 ✓；否则取**基类里最派生的那个元类型** ✓（参照的 OM-13 口径 ✓）——
    // 继承 `ABCMeta` 一族建的类时，本类的 `type(X)` 也应当是 `ABCMeta` ✓。
    let chosen = requested_metaclass.unwrap_or(effective_metaclass);
    if chosen != instance.metatype() {
        // SAFETY: ty 是本函数刚造出的类对象（头部在首位 ✓）；chosen 是注册表里的类型 ✓。
        unsafe { ty.cast::<Header>().as_ref() }.set_ty(chosen);
    }

    // `__init_subclass__`（`OM-14` 的类创建钩子）：在**直接基类**上找并调用
    for base in &bases {
        crate::executor::call_dunder_method(
            instance,
            *base,
            "__init_subclass__",
            &[ty.cast::<Header>()],
        )?;
    }

    // SAFETY: ty 由注册表持有；这里新增一份引用交给调用方。
    unsafe { instance.incref_object(ty.cast::<Header>().as_ptr()) };
    Ok(ty.cast::<Header>())
}
