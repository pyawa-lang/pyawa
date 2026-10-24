//! 本文件是 `builtin_objects` 的 `type_runtime` 域：类型／帧／内建迭代器构造器（`type.__new__`／`super`／`sys._getframe`／`zip`／`map` 一族）。
//!
//! 从单文件 `builtin_objects.rs` **原样搬来**（纯移动，逐行一致）；父模块 `pub use` 再导出
//! ⇒ `crate::builtin_objects::…` 的旧路径不变。外部的 `use` 清单在 `super::prelude`。

use super::prelude::*;
use super::*;



/// **`type.__new__(mcls, name, bases, namespace)`** ✓（第 234 轮）：参照的类创建**那一处真相** ✓。
///
/// 两条路都到这儿 ✓：元类里写的 `super().__new__(mcls, …)` ✓ 与显式的 `type.__new__(…)` ✓。
/// **取后三个实参**当 `(name, bases, namespace)` ✓ ⇒ 两种调用形状**都合** ✓。
pub fn type_new_native(
    instance: &Instance,
    _bound: Option<NonNull<Header>>,
    args: &[NonNull<Header>],
    kwargs: &[(NonNull<Header>, NonNull<Header>)],
) -> Result<NonNull<Header>, crate::ExecError> {
    if args.len() < 3 {
        // **带上实参个数与首参** ✓（第 189 轮）：先前只有一句"至少要 3 个" ✗ ⇒ 定位全靠猜 ✓。
        // **带上"谁在调"** ✓（第 190 轮）：用当前帧的 `co_qualname` 一锤定音 ✓（第 230 轮的 API ✓）。
        let caller = instance
            .current_frame()
            .and_then(|frame| {
                // SAFETY: 帧由执行器守卫持有，存活。
                let frame = unsafe { &*frame.as_ptr().cast::<crate::Frame>() };
                let code = frame.code()?;
                // SAFETY: code 由帧持有，存活。
                let code = unsafe { &*code.as_ptr().cast::<crate::CodeObject>() };
                Some(format!(
                    "{}（{} 第 {} 行起）",
                    code.qualname(),
                    code.filename(),
                    code.firstlineno()
                ))
            })
            .unwrap_or_else(|| "<没有当前帧>".to_owned());
        let what = format!(
            "type.__new__ 至少要 3 个实参，实际 {} 个（调用者 {}，首参 {}）",
            args.len(),
            caller,
            args.first()
                .map(|value| instance
                    .object_repr(*value)
                    .unwrap_or_else(|_| "<读不出>".to_owned()))
                .unwrap_or_else(|| "<无>".to_owned())
        );
        return Err(instance.raise_builtin_error("TypeError", &what));
    }
    let namespace = args[args.len() - 1];
    let bases_value = args[args.len() - 2];
    let name_value = args[args.len() - 3];
    let Some(name) = instance.text_of(name_value) else {
        return Err(instance.raise_builtin_error("TypeError", "type.__new__ 的名字要是 str"));
    };
    if Some(instance.type_of(bases_value)) != instance.type_named("tuple") {
        return Err(instance.raise_builtin_error("TypeError", "type.__new__ 的基类要是 tuple"));
    }
    // SAFETY: 类型身份刚确认。
    let bases: Vec<NonNull<Header>> =
        unsafe { &*bases_value.as_ptr().cast::<crate::TupleObject>() }.items().to_vec();
    // **命名空间只要是 `dict` 的**子类**就收** ✓（第 507 轮真 bug 修 ✗）：先前按**类型身份**比较 ✗
    // ⇒ `Lib/enum.py` 把 `_EnumDict`（`dict` 子类 ✓）**本身**交给 `super().__new__` ✓ ⇒ 我们当场报
    // `type.__new__ 的命名空间要是 dict` ✗，参照给 `D-ok` ✓。参照的口径是"映射"而非"恰好是 dict" ✓。
    let namespace_is_dict = instance
        .type_named("dict")
        .is_some_and(|dict| instance.is_subtype(instance.type_of(namespace), dict));
    if !namespace_is_dict {
        return Err(instance.raise_builtin_error("TypeError", "type.__new__ 的命名空间要是 dict"));
    }
    // `mcls` 那一位：是个**类型对象**就用它当元类 ✓（`super().__new__(mcls, …)` 正是这样 ✓）。
    let requested = args
        .first()
        .filter(|value| instance.is_type_object(**value))
        .map(|value| value.cast::<crate::TypeObject>());
    // **类关键字转交 `__init_subclass__`** ✓（task-24 尾件 ✓；`metaclass` 由被调方滤掉 ✓）。
    crate::classes::build_class_from_parts_with_keywords(
        instance,
        name.to_owned(),
        bases,
        namespace,
        requested,
        kwargs,
    )
}

/// **零参 `super()` 慢路的"定义类"反查**（task-6 ✓）：在 `this` 的 MRO 上找**类字典持有当前 `code`** 的那个类。
///
/// 参照语义：定义类 ＝ **方法所在的那个类对象**（CPython 编译期把 `__class__` cell 塞进方法的闭包）。
/// 本层**不改编译器字节码**（`compile` 的逐字节对拍不能动 ✓）⇒ 运行期反查：
/// 方法的 code object 就在类字典的那个函数里 ⇒ 沿 MRO **由近到远**找第一个"类字典里有一项是同一 code 的函数"。
///
/// - `this` 是**类对象**（`__new__`／`__init_subclass__` 这种 staticmethod／classmethod 形态 ⇒ 第一个实参是类）
///   ⇒ 先查**这个类自己**的 MRO ✓（`__new__` 就是这条）；再兜底查它的**元类** MRO ✓（元类里定义的 `__new__`）。
/// - `this` 是实例 ⇒ 查 `type(this)` 的 MRO ✓（`type(this)` 必是定义类的子类 ✓）。
/// - 找不到 ⇒ `None`（调用方保留原报错 ✓）。
fn find_defining_class(
    instance: &Instance,
    code: NonNull<Header>,
    this: NonNull<Header>,
) -> Option<NonNull<Header>> {
    if instance.is_type_object(this) {
        // SAFETY: `is_type_object` 刚确认 ⇒ 载荷是 `TypeObject`，由注册表持有。
        let self_type = this.cast::<crate::TypeObject>();
        if let Some(found) = scan_mro_for_code(instance, code, self_type) {
            return Some(found);
        }
        // 元类里定义的 `__new__`／classmethod：定义类是 `type(this)` 的 MRO 上的那个类。
        return scan_mro_for_code(instance, code, instance.type_of(this));
    }
    scan_mro_for_code(instance, code, instance.type_of(this))
}

/// 沿 `root` 的 MRO **由近到远**找"类字典里持有 `code` 那个函数"的类。
fn scan_mro_for_code(
    instance: &Instance,
    code: NonNull<Header>,
    root: NonNull<crate::TypeObject>,
) -> Option<NonNull<Header>> {
    // SAFETY: root 由注册表持有；MRO 里的类型同样存活。
    for entry in unsafe { root.as_ref() }.mro() {
        // SAFETY: 同上。
        let Some(mapping) = (unsafe { entry.as_ref() }).dict() else {
            continue;
        };
        let Some(entries) = instance.dict_entries(mapping) else {
            continue;
        };
        if entries
            .into_iter()
            .any(|(_key, value)| class_dict_value_has_code(instance, value, code))
        {
            return Some(entry.cast::<Header>());
        }
    }
    None
}

/// 类字典里的这一项是不是"定义在 `code` 上的函数"。
///
/// 除了裸 `function`，还要剥开类体里会自动出现的包装：`__new__` 被 `type.__new__` 包成
/// **`staticmethod`** ✓、`@classmethod`／`@staticmethod`／`@property` 装饰的也是包装对象 ✓。
/// **只认精确的内建包装类型** ✗（Python 子类实例走通用 `AttributeObject` 布局、载荷里没有被包的对象
/// ⇒ 强转会读到垃圾／NULL；判据同 `builtin_wrapper_layout_ok` 的注释 ✓）。
fn class_dict_value_has_code(
    instance: &Instance,
    value: NonNull<Header>,
    code: NonNull<Header>,
) -> bool {
    let Some(function_type) = instance.type_named("function") else {
        return false;
    };
    let value_type = instance.type_of(value);
    if value_type == function_type {
        return function_has_code(instance, value, function_type, code);
    }
    if Some(value_type) == instance.type_named("staticmethod") {
        // SAFETY: 类型身份刚确认 ⇒ 载荷是 `StaticMethodObject`。
        let inner = unsafe { &*value.as_ptr().cast::<StaticMethodObject>() }.function();
        return function_has_code(instance, inner, function_type, code);
    }
    if Some(value_type) == instance.type_named("classmethod") {
        // SAFETY: 类型身份刚确认 ⇒ 载荷是 `ClassMethodObject`。
        let inner = unsafe { &*value.as_ptr().cast::<ClassMethodObject>() }.function();
        return function_has_code(instance, inner, function_type, code);
    }
    if Some(value_type) == instance.type_named("property") {
        // SAFETY: 类型身份刚确认 ⇒ 载荷是 `PropertyObject`（`fset`／`fdel` 可能是 `None` 单例，下面按类型滤）。
        let property = unsafe { &*value.as_ptr().cast::<PropertyObject>() };
        return [property.fget(), property.fset(), property.fdel()]
            .into_iter()
            .any(|inner| function_has_code(instance, inner, function_type, code));
    }
    false
}

/// `candidate` 是函数对象且它的 code 就是 `code`。
fn function_has_code(
    instance: &Instance,
    candidate: NonNull<Header>,
    function_type: NonNull<crate::TypeObject>,
    code: NonNull<Header>,
) -> bool {
    instance.type_of(candidate) == function_type
        // SAFETY: 类型身份刚确认 ⇒ 载荷是 `FunctionObject`。
        && unsafe { &*candidate.as_ptr().cast::<FunctionObject>() }.code() == code
}

/// **`super()`**（**零参**）✓（第 233 轮）：从**当前帧**取 `self` ✓，从 `co_qualname` 取**定义该方法的类名** ✓
///（`A.hi` ⇒ `A` ✓，在**当前全局**里查 ✓）。
///
/// **快路 ⇒ 慢路**（task-6 ✓）：模块级类走的仍是"全局按名字查"的**快路** ✓（`super()` 很热 ⇒ 不能给快路加扫描）；
/// 查不到时走**慢路**：用**当前帧的 code** 在 `type(self)` 的 MRO 里反查"类字典持有这个函数"的那个类
/// （＝参照里 `__class__` cell 给出的**定义类** ✓）—— 函数里／嵌套定义的类不在全局里，只能这样定位 ✓。
/// 慢路也查不到 ⇒ **保留原来的报错** ✓（不吞错 ✓）。
///
/// **如实说** ✗：只接**零参**形式 ✓；慢路按"函数 code 相同"认定义类 ⇒ 把同一个函数**别名**给另一个类
/// （`B.f = A.f`）时认不出编译期那个 `__class__` cell（取 MRO 上**最近**的匹配 ✓，如实差异）。
pub fn super_new(
    instance: &Instance,
    _bound: Option<NonNull<Header>>,
    _args: &[NonNull<Header>],
    _kwargs: &[(NonNull<Header>, NonNull<Header>)],
) -> Result<NonNull<Header>, crate::ExecError> {
    let frame_header = instance
        .current_frame()
        .ok_or_else(|| instance.raise_builtin_error("RuntimeError", "super(): 没有当前帧"))?;
    // SAFETY: 帧由执行器守卫持有，存活。
    let frame = unsafe { &*frame_header.as_ptr().cast::<crate::Frame>() };
    let code_header = frame
        .code()
        .ok_or_else(|| instance.raise_builtin_error("RuntimeError", "super(): 帧没有 code"))?;
    // SAFETY: code 由帧持有，存活。
    let code = unsafe { &*code_header.as_ptr().cast::<crate::CodeObject>() };
    let qualname = code.qualname().to_owned();
    let Some((prefix, _)) = qualname.rsplit_once('.') else {
        return Err(instance.raise_builtin_error("RuntimeError", "super(): 当前不在类方法里"));
    };
    let class_name = prefix.rsplit('.').next().unwrap_or(prefix).to_owned();
    let globals = instance
        .current_globals()
        .ok_or_else(|| instance.raise_builtin_error("RuntimeError", "super(): 没有当前全局"))?;
    // **快路**：全局里按名字查（模块级类的老路径 ✓，`super()` 很热 ⇒ 这一支的顺序与开销保持原样：
    // 先查全局、再取第一个实参；慢路的 MRO 扫描与 `frame.local(0)` 都不进快路）。
    let class_value = if let Some(found) = instance.dict_get(globals, &class_name) {
        found
    } else {
        // **慢路**：函数里／嵌套定义的类不在全局里 ⇒ 按"类字典持有当前 code"反查定义类。
        let this = frame.local(0).ok().flatten();
        match this.and_then(|value| find_defining_class(instance, code_header, value)) {
            Some(found) => found,
            // 慢路也找不到 ⇒ **保留原来的报错**（不吞错 ✓）。
            None => {
                return Err(instance.raise_builtin_error(
                    "RuntimeError",
                    &format!("super(): 全局里找不到定义类 {class_name}"),
                ));
            }
        }
    };
    let this = frame.local(0).ok().flatten().ok_or_else(|| {
        instance.raise_builtin_error("RuntimeError", "super(): 当前帧没有第一个实参")
    })?;
    let super_type = instance.type_named("super").ok_or(crate::ExecError::Unsupported {
        opcode: 0,
        what: "super 类型未登记",
    })?;
    let object = instance
        .alloc(crate::builtin_objects::AttributeObject::new(
            super_type,
            core::cell::RefCell::new(Some(instance.new_dict())),
        ))
        .into_raw()
        .cast::<Header>();
    // **直接写"内联属性字典"** ✗（第 233 轮实测）：`set_attribute_value` 写的是**挂载**字典 ✓，
    // 而 `AttributeObject::attributes()` 读的是**内联**字典 ✓ ⇒ 两头对不上 ⇒ `super_lookup` 读不到 ✓。
    // SAFETY: object 是本函数刚造的存活对象，载荷就是 `AttributeObject` ✓。
    let attrs = unsafe { &*object.as_ptr().cast::<crate::builtin_objects::AttributeObject>() };
    if let Some(dict) = attrs.attributes() {
        instance.dict_set(dict, "__thisclass__", class_value);
        instance.dict_set(dict, "__self__", this);
    }
    Ok(object)
}


/// **调用栈**（第 617 轮 ✓）：`CurrentFrameGuard` 装帧/卸帧时各维护一次 ✓ ⇒ `sys._getframemodulename(depth)`
/// 能回到**调用者**那一帧 ✓。为什么需要它 ✗：`Lib/collections/__init__.py:519-527` 的 `namedtuple` 先试
/// `_sys._getframemodulename(1)` ✓、失败才走 `_getframe(1).f_globals` ✓ —— 而我们的帧**没有** `f_globals` ✗。
pub(crate) static FRAME_STACK: std::sync::Mutex<Vec<usize>> = std::sync::Mutex::new(Vec::new());

pub(crate) fn push_frame(frame: NonNull<Header>) {
    if let Ok(mut stack) = FRAME_STACK.lock() {
        stack.push(frame.as_ptr() as usize);
    }
}

pub(crate) fn pop_frame() {
    if let Ok(mut stack) = FRAME_STACK.lock() {
        stack.pop();
    }
}

/// 取"当前正在跑的帧"下面第 `depth` 层的那一帧 ✓（`depth == 0` ⇒ 自己 ✓）。
pub(crate) fn frame_at_depth(depth: usize) -> Option<NonNull<Header>> {
    let stack = FRAME_STACK.lock().ok()?;
    let index = stack.len().checked_sub(1 + depth)?;
    NonNull::new(stack[index] as *mut Header)
}

/// **`sys._getframemodulename([depth])`** ✓（第 617 轮）：给**那一帧**的模块名 ✓（拿不到 ⇒ `None` ✓，
/// 参照的 `namedtuple` 正是 `_sys._getframemodulename(1) or '__main__'` ✓）。
pub fn getframemodulename_native(
    instance: &Instance,
    _bound: Option<NonNull<Header>>,
    args: &[NonNull<Header>],
    _kwargs: &[(NonNull<Header>, NonNull<Header>)],
) -> Result<NonNull<Header>, crate::ExecError> {
    let depth = match args.first() {
        Some(value) => instance.index_value(*value)?.unwrap_or(0),
        None => 0,
    };
    let depth = usize::try_from(depth).unwrap_or(usize::MAX);
    let Some(frame) = frame_at_depth(depth) else {
        return Ok(instance.retain(instance.singletons().none()));
    };
    // SAFETY: frame 在调用栈里，存活。
    let frame_ref = unsafe { &*frame.as_ptr().cast::<crate::frame::Frame>() };
    let namespace = frame_ref
        .globals()
        .or_else(|| frame_ref.namespace());
    let Some(namespace) = namespace else {
        return Ok(instance.retain(instance.singletons().none()));
    };
    match instance.dict_get(namespace, "__name__") {
        Some(name) => Ok(instance.retain(name)),
        None => Ok(instance.retain(instance.singletons().none())),
    }
}

/// **`sys._getframe([depth])`** ✓（第 230 轮）：给**当前帧对象** ✓
///（`_collections_abc.py:89` 的 `sys._getframe().f_locals` 要它 ✓）。
///
/// **如实说** ✗：只接 `depth` ＝ 0 ✓（`f_back` 链随后补 ✓）；`depth` 非整数如实报错 ✓。
pub fn getframe_native(
    instance: &Instance,
    _bound: Option<NonNull<Header>>,
    args: &[NonNull<Header>],
    _kwargs: &[(NonNull<Header>, NonNull<Header>)],
) -> Result<NonNull<Header>, crate::ExecError> {
    if let Some(depth) = args.first() {
        match instance.index_value(*depth)? {
            Some(0) => {}
            Some(_) => {
                return Err(crate::ExecError::Unsupported {
                    opcode: 0,
                    what: "sys._getframe 目前只接 depth＝0（f_back 链随后补）",
                })
            }
            None => {
                return Err(instance
                    .raise_builtin_error("TypeError", "sys._getframe 的 depth 要整数"))
            }
        }
    }
    let Some(frame) = instance.current_frame() else {
        return Err(instance.raise_builtin_error("ValueError", "sys._getframe: 没有当前帧"));
    };
    // SAFETY: 帧由执行器守卫持有，这里新增一份引用交给调用方 ✓。
    unsafe { instance.incref_object(frame.as_ptr()) };
    Ok(frame)
}

/// **`dir([object])`**（第 406 轮 ✓）：`pickle.py:190` 的
/// `__all__.extend(x for x in dir() if x.isupper() and not x.startswith('_'))` 是**导入期**调用 ✓
/// ⇒ 六个模块（logging.config／logging.handlers／pickle／pickletools／shelve／tracemalloc）卡它 ✓。
///
/// 面（本轮最小可用 ✓，照参照实测）：
/// - **无参**：当前帧的局部名（模块／类帧＝它的命名空间 ✓、函数帧＝已绑定的 fast locals ✓）；
/// - **类型对象**：它自己 `__mro__` 上各类型字典的键 ✓（参照：`dir(int)` **不含**元类的
///   `mro` 这类名字 ✓ —— 实测 `'mro' in dir(int)` 为 `False` ✓）；
/// - **实例**：类型的 `__mro__` 字典键 ＋ 实例属性字典键 ✓；
/// - **模块**：模块命名空间键（＋ module 类型的面 ✓）。
/// **如实范围** ✗：**不调用** `__dir__` 自定义协议 ✗（参照会调用 ✓；随后补）。
pub fn dir_new(
    instance: &Instance,
    _bound: Option<NonNull<Header>>,
    args: &[NonNull<Header>],
    kwargs: &[(NonNull<Header>, NonNull<Header>)],
) -> Result<NonNull<Header>, crate::ExecError> {
    if !kwargs.is_empty() {
        return Err(instance.raise_builtin_error("TypeError", "dir() takes no keyword arguments"));
    }
    if args.len() > 1 {
        return Err(instance.raise_builtin_error(
            "TypeError",
            &format!("dir expected at most 1 argument, got {}", args.len()),
        ));
    }
    let mut names: std::collections::BTreeSet<String> = std::collections::BTreeSet::new();
    if args.is_empty() {
        if let Some(frame) = instance.current_frame() {
            // SAFETY: 当前帧由执行器守卫持有、存活。
            let frame = unsafe { &*frame.as_ptr().cast::<crate::Frame>() };
            let locals = frame.locals_snapshot(instance);
            collect_mapping_keys(instance, locals, &mut names);
            instance.release(locals);
        }
    } else {
        let object = args[0];
        if instance.is_type_object(object) {
            collect_type_mro_names(instance, object.cast(), &mut names);
        } else {
            let ty = instance.type_of(object);
            collect_type_mro_names(instance, ty.cast(), &mut names);
            if let Some(mapping) = crate::mounted_instance_dict(instance, object) {
                collect_mapping_keys(instance, mapping, &mut names);
            }
        }
    }
    let items: Vec<NonNull<Header>> = names
        .into_iter()
        .map(|name| instance.new_str(&name))
        .collect();
    Ok(instance.new_list(items))
}

/// 把一个 `dict` 的**字符串键**收进集合（非字符串键跳过 ✓ —— Python 的 `dir` 只回字符串 ✓）。
pub(crate) fn collect_mapping_keys(
    instance: &Instance,
    mapping: NonNull<Header>,
    out: &mut std::collections::BTreeSet<String>,
) {
    // SAFETY: mapping 由调用方保证是本实例存活的 dict。
    let dict = unsafe { &*mapping.as_ptr().cast::<DictObject>() };
    for (key, _) in dict.entries() {
        if let Some(text) = instance.text_of(key) {
            out.insert(text.to_owned());
        }
    }
}

/// 一个**类型对象**（或"某个实例的类型"）的 `__mro__` 上所有字典键 ✓。
pub(crate) fn collect_type_mro_names(
    instance: &Instance,
    ty: NonNull<crate::TypeObject>,
    out: &mut std::collections::BTreeSet<String>,
) {
    // SAFETY: ty 由注册表持有、存活。
    for base in unsafe { &*ty.as_ptr() }.mro() {
        // SAFETY: 同上。
        if let Some(mapping) = unsafe { &*base.as_ptr() }.dict() {
            collect_mapping_keys(instance, mapping, out);
        }
    }
}

/// **`zip(*iterables)`** ✓（第 229 轮）：**惰性** ✓、**取最短** ✓
///（`_collections_abc.py:81` 要 `type(iter(zip()))` ✓；`os.py:563` 要 `zip(dirs[::-1], entries[::-1])` ✓）。
pub fn zip_new(
    instance: &Instance,
    _bound: Option<NonNull<Header>>,
    args: &[NonNull<Header>],
    _kwargs: &[(NonNull<Header>, NonNull<Header>)],
) -> Result<NonNull<Header>, crate::ExecError> {
    // 每个实参先 `iter()` ✓（走执行器**同一处** ✓）。
    let mut items: Vec<NonNull<Header>> = Vec::with_capacity(args.len());
    for argument in args {
        items.push(instance.iter_object(*argument)?);
    }
    let list = instance.new_list(items);
    let iterator = instance.new_zip_iterator(list);
    // `new_zip_iterator` 自己**又 incref 了一份** ✓ ⇒ 这里还掉我们这份 ✓。
    // SAFETY: list 由本函数持有。
    unsafe { instance.release_object(list.as_ptr()) };
    Ok(iterator)
}

/// **`map(function, iterable, ...)`** ✓（第 338 轮）：**急求值** —— 返回一个 **`list`** ✓。
///
/// **如实登记的偏差** ✗：参照返回**惰性**的 `map` 对象 ✓（`type(...)` 是 `map` ✓、可以套无限可迭代
/// 对象 ✓）；本层返回**列表** ✓。为什么这样落：真正的惰性 `map` 需要**新迭代器类型**，而那个改动
/// （把 `map`／`filter` 认成迭代器 ✓）会在**套件上下文里抖出一条潜伏 UAF** ✗（第 335／337 轮已把
/// 触发点夹到"这两个类型被 `is_iterator_type` 认成迭代器"这一处 ✓，但根因还欠 ✓）⇒ **先按急求值**
/// 让上限榜上那 **74** 个模块过这一关 ✓，惰性面随后补 ✓。
/// **不静默** ✗：偏差写在这里、写进台账、写进语料注释 ✓（值与迭代行为都与参照一致 ✓，
/// 只有"类型名"与"惰性"两点不同 ✓）。
pub fn map_new(
    instance: &Instance,
    _bound: Option<NonNull<Header>>,
    args: &[NonNull<Header>],
    _kwargs: &[(NonNull<Header>, NonNull<Header>)],
) -> Result<NonNull<Header>, crate::ExecError> {
    if args.len() < 2 {
        return Err(instance.raise_builtin_error(
            "TypeError",
            "map() must have at least two arguments.",
        ));
    }
    let function = args[0];
    // 每个可迭代实参先 `iter()` ✓（走执行器**同一处** ✓）
    let mut iterators: Vec<NonNull<Header>> = Vec::with_capacity(args.len() - 1);
    for argument in &args[1..] {
        iterators.push(instance.iter_object(*argument)?);
    }
    let mut items: Vec<NonNull<Header>> = Vec::new();
    loop {
        let mut row: Vec<NonNull<Header>> = Vec::with_capacity(iterators.len());
        let mut exhausted = false;
        for inner in &iterators {
            match instance.advance_iterator(*inner)? {
                Some(item) => row.push(item),
                None => {
                    exhausted = true;
                    break;
                }
            }
        }
        if exhausted {
            for item in row {
                // SAFETY: 刚取出来的新引用 ⇒ 交还实例 ✓
                unsafe { instance.release_object(item.as_ptr()) };
            }
            break;
        }
        // **实参表的所有权交给 `call_callable`** ✓（它的契约就是接手 ✓）
        let value = crate::executor::call::call_callable(instance, function, None, row, Vec::new(), 0)?;
        items.push(value);
    }
    Ok(instance.new_list(items))
}

/// **`filter(predicate, iterable)`** ✓（第 338 轮）：**急求值** —— 返回一个 **`list`** ✓
/// （偏差同 [`map_new`] ✓：参照是惰性的 `filter` 对象 ✗）。
pub fn filter_new(
    instance: &Instance,
    _bound: Option<NonNull<Header>>,
    args: &[NonNull<Header>],
    _kwargs: &[(NonNull<Header>, NonNull<Header>)],
) -> Result<NonNull<Header>, crate::ExecError> {
    if args.len() != 2 {
        return Err(instance.raise_builtin_error(
            "TypeError",
            &format!("filter expected 2 arguments, got {}", args.len()),
        ));
    }
    let predicate = args[0];
    let none = instance.singletons().none();
    let mut items: Vec<NonNull<Header>> = Vec::new();
    // **与 `map` 走同一处**（`iter_object` ＋ `advance_iterator` ✓）：先前用 `collect_iterable` ✗
    // ⇒ `filter(lambda x: x > 1, range(5))` 会报出一条**张冠李戴**的消息
    // （`bytes(<可迭代>)：只接线了 list／tuple` ✗ —— 明明与 `bytes` 无关 ✓），实测抓到 ✓。
    let inner = instance.iter_object(args[1])?;
    let values: Vec<NonNull<Header>> = {
        let mut collected = Vec::new();
        while let Some(value) = instance.advance_iterator(inner)? {
            collected.push(value);
        }
        // 迭代器本身那份引用用完就还 ✓
        // SAFETY: inner 由本函数持有 ⇒ 交还实例 ✓。
        unsafe { instance.release_object(inner.as_ptr()) };
        collected
    };
    for value in values {
        let keep = if predicate == none {
            instance.truth_of(value)
        } else {
            let mut call_args: Vec<NonNull<Header>> = Vec::with_capacity(1);
            // SAFETY: value 由本函数持有 ⇒ 新增一份交给调用 ✓。
            unsafe { instance.incref_object(value.as_ptr()) };
            call_args.push(value);
            let verdict = crate::executor::call::call_callable(instance, predicate, None, call_args, Vec::new(), 0)?;
            let truth = instance.truth_of(verdict);
            // SAFETY: verdict 由本次调用返回 ⇒ 交还实例 ✓。
            unsafe { instance.release_object(verdict.as_ptr()) };
            truth
        };
        if keep {
            items.push(value);
        } else {
            // SAFETY: value 由本函数持有 ⇒ 交还实例 ✓。
            unsafe { instance.release_object(value.as_ptr()) };
        }
    }
    Ok(instance.new_list(items))
}

/// **`enumerate(iterable, start=0)`** ✓（第 347 轮）：**急求值** —— 返回 `(下标, 元素)` 的 **`list`** ✓。
///
/// **如实登记的偏差** ✗：参照返回**惰性**的 `enumerate` 对象 ✓（`type(...)` 是 `enumerate` ✓）；
/// 本层返回列表 ✓ —— 与 `map`／`filter` 同一口径与同一理由 ✓（真惰性要**新迭代器类型** ✓，
/// 而"把它认成迭代器"那一步会在套件上下文里抖出一条潜伏 UAF ✗，第 335／337 轮把触发点夹到过 ✓）。
/// 上限榜上 `NameError: name 'enumerate' is not defined` × **76** 个模块 ✓。
pub fn enumerate_new(
    instance: &Instance,
    _bound: Option<NonNull<Header>>,
    args: &[NonNull<Header>],
    _kwargs: &[(NonNull<Header>, NonNull<Header>)],
) -> Result<NonNull<Header>, crate::ExecError> {
    let Some(iterable) = args.first() else {
        return Err(instance.raise_builtin_error(
            "TypeError",
            "enumerate() missing required argument 'iterable' (pos 1)",
        ));
    };
    let start = match args.get(1) {
        Some(value) => instance
            .int_of(*value)
            .and_then(|value| value.to_i64())
            .ok_or_else(|| {
                instance.raise_builtin_error("TypeError", "'something else' object cannot be interpreted as an integer")
            })?,
        None => 0,
    };
    // **与 `map`／`filter` 走同一处**（`iter_object` ＋ `advance_iterator` ✓）：`collect_iterable` 对
    // **非 list／tuple 的可迭代对象**会报一条张冠李戴的消息 ✗（`bytes(<可迭代>)：只接线了 list／tuple` ✓
    // —— 与 `bytes` 毫无关系 ✓，第 338 轮做 `filter` 时撞过同一处 ✓），而 `advance_iterator` 接的
    // 是**迭代协议** ✓ ⇒ 字符串／生成器／`range` 都能摊 ✓。返回的每项是**新引用** ✓ ⇒ 直接交给元组 ✓。
    let inner = instance.iter_object(*iterable)?;
    let mut items: Vec<NonNull<Header>> = Vec::new();
    let mut index = start;
    while let Some(value) = instance.advance_iterator(inner)? {
        let counter = instance.new_int(index);
        let pair = instance.new_tuple(vec![counter, value]);
        items.push(pair);
        index += 1;
    }
    // SAFETY: inner 由本函数持有 ⇒ 用完交还实例 ✓。
    unsafe { instance.release_object(inner.as_ptr()) };
    Ok(instance.new_list(items))
}

/// **`reversed(<list>)`** ✓（第 227 轮）：给一个 **`list_reverseiterator`** ✓
///（`Lib/_collections_abc.py:75` 要 `type(iter(reversed([])))` ✓）。
///
/// **如实说** ✗：目前只接线 **`list`** ✓（`tuple`／`str`／`range` 一族随后补 ✓ —— 参照给的是**别的**类型名 ✓）。
pub fn reversed_new(
    instance: &Instance,
    _bound: Option<NonNull<Header>>,
    args: &[NonNull<Header>],
    _kwargs: &[(NonNull<Header>, NonNull<Header>)],
) -> Result<NonNull<Header>, crate::ExecError> {
    let Some(target) = args.first().copied() else {
        return Err(instance.raise_builtin_error("TypeError", "reversed expected 1 argument, got 0"));
    };
    // ① **`__reversed__` 优先** ✓（参照口径 ✓：`list`／`dict` 一族自己有它就先用 ✓）。
    if let Some(method) = crate::executor::attribute::attribute_optional(instance, target, "__reversed__")? {
        let result = crate::executor::call::call_value(instance, method, &[], &[]);
        // SAFETY: method 是新引用。
        unsafe { instance.release_object(method.as_ptr()) };
        return result;
    }
    // ③ **`dict` 的键反向** ✓（参照的口径：`dict.__reversed__` ✓）：先取键列表，再**惰性**反向 ✓
    //（`subscript_read(dict, 0)` 是按键查值 ✗，不能走序列协议 ✓）。
    if instance.type_name(instance.type_of(target)) == "dict" {
        // SAFETY: 类型身份刚确认。
        let entries = unsafe { &*target.as_ptr().cast::<DictObject>() }.entries();
        // `new_list` **吃掉**每项引用 ⇒ 各 retain 一份 ✓。
        let items: Vec<NonNull<Header>> = entries
            .iter()
            .map(|(key, _)| instance.retain(*key))
            .collect();
        let list = instance.new_list(items);
        let ty = instance.type_named("reversed").ok_or(crate::ExecError::Unsupported {
            opcode: 0,
            what: "reversed 类型未登记",
        })?;
        // SAFETY: list 是本函数刚造的（新引用 ✓）⇒ 交给迭代器持有 ✓（不必再 incref ✓）。
        let length = unsafe { &*list.as_ptr().cast::<ListObject>() }.items().len();
        let iterator = instance.alloc(IteratorObject::new(ty, list, core::cell::Cell::new(length)));
        return Ok(iterator.into_raw().cast::<Header>());
    }
    // ② **序列协议** ✓（`__len__` ＋ `__getitem__` ⇒ 通用惰性反向迭代器 ✓：tuple／str／range／bytes／自定义 ✓）。
    // 长度走 **`len()` 的同一处实现** ✓（`length_with_protocol` ✓：内建槽与 `__len__` 都认 ✓）。
    let builtin_sequence = matches!(
        instance.type_name(instance.type_of(target)).as_str(),
        "tuple" | "list" | "str" | "range" | "bytes" | "bytearray"
    );
    let has_getitem = crate::executor::attribute::attribute_optional(instance, target, "__getitem__")?.is_some();
    let length = instance.length_with_protocol(target).ok().flatten();
    let Some(length) = length.filter(|_| builtin_sequence || has_getitem) else {
        let name = instance.type_name(instance.type_of(target));
        return Err(instance.raise_builtin_error(
            "TypeError",
            &format!("'{name}' object is not reversible"),
        ));
    };
    let ty = instance.type_named("reversed").ok_or(crate::ExecError::Unsupported {
        opcode: 0,
        what: "reversed 类型未登记",
    })?;
    // SAFETY: 迭代器对象要**自己那一份**引用。
    unsafe { instance.incref_object(target.as_ptr()) };
    let iterator = instance.alloc(IteratorObject::new(ty, target, core::cell::Cell::new(length)));
    Ok(iterator.into_raw().cast::<Header>())
}
