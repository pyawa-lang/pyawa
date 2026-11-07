//! **`iter` 族** —— 从 `builtin_objects.rs` **原样搬来** ✓（纯移动 ✓、零逻辑改动 ✓）。
//!
//! * 派发表 `iter_method_native` **不搬** ✓（族之间的粘合层 ✓）；
//! * 仍在原处的被引用项：（无） ✓；
//! * `use` 块照抄原文件 ✓（`cargo fix` 随后删多余的 ✓），缺失项由自愈循环补 ✓。
use crate::builtin_objects::BoolObject;
use crate::builtin_objects::BytesObject;
use core::cell::Cell;
use crate::builtin_objects::DequeObject;
use crate::builtin_objects::DictObject;
use crate::executor::ExecError;
use crate::builtin_objects::FloatObject;
use crate::header::Header;
use crate::instance::Instance;
use crate::builtin_objects::IteratorObject;
use crate::builtin_objects::RangeObject;
use crate::builtin_objects::ListObject;
use core::ptr::NonNull;
use crate::builtin_objects::SetObject;
use crate::builtin_objects::TupleObject;
use crate::executor::arithmetic_public;
use crate::executor::builtin_type;
use crate::executor::call_value;
use crate::executor::is_iterator_type;
use crate::executor::iterator_type_for;
use crate::executor::raise_builtin;
use crate::executor::release;
use crate::executor::values_equal;



/// **`iter(x)`**（`OM-11` 的 `iter` 槽位；公开面，`itertools.islice` 一类要用）。
///
/// 规则与 `GET_ITER` **同一处实现**：迭代器（含生成器）**原样**（新引用）；内建可迭代
/// 包一层按下标走的迭代器；其余走 `__iter__`；都没有 ⇒ 照参照**实测**的消息报
/// `TypeError: 'X' object is not iterable`。
/// 调一个**零实参**方法／可调用 ✓（`__iter__` 那一格用 ✓；`this` 是绑定 self ✓）。
fn call_callable_value(
    instance: &Instance,
    function: NonNull<Header>,
    this: Option<NonNull<Header>>,
) -> Result<NonNull<Header>, ExecError> {
    crate::executor::call::call_callable(instance, function, this, Vec::new(), Vec::new(), 0)
}

pub fn iter_value(instance: &Instance, iterable: NonNull<Header>) -> Result<NonNull<Header>, ExecError> {
    // SAFETY: iterable 由调用方保证存活。
    let ty = unsafe { iterable.as_ref() }.ty();
    // 迭代器（含生成器）就是它自己的迭代器（实测 `iter(c) is c`）
    if ty == builtin_type(instance, "generator") || is_iterator_type(instance, ty) {
        // SAFETY: 同上。
        unsafe { instance.incref_object(iterable.as_ptr()) };
        return Ok(iterable);
    }
    // **`deque` 的迭代**（round 86，参照 `collections.deque`）：直接按 `items` 物化成一个 `list`
    // 再交给现成的 `list_iterator` ✓（**如实记**：不是参照的 `_deque_iterator`，是**快照** ✓ ——
    // 迭代中修改 deque 的语义与参照可能不同 ✗；`test_contains`／`in` 这类只读用法完全等价 ✓）。
    if instance
        .type_named("deque")
        .is_some_and(|base| instance.is_subtype(ty, base))
    {
        // SAFETY: iterable 由调用方保证存活、类型刚确认是 deque 族（载荷同构 ✓）。
        let deque = unsafe { &*iterable.as_ptr().cast::<crate::builtin_objects::DequeObject>() };
        let items: Vec<NonNull<Header>> = deque
            .items()
            .into_iter()
            .map(|item| instance.retain(item))
            .collect();
        let snapshot = instance.new_list(items);
        let iterator_type = iterator_type_for(instance, snapshot)?;
        // 迭代器要自己那份 list 引用 ✓（本函数不消耗快照 ✓）。
        // SAFETY: snapshot 是本函数的新引用，存活。
        unsafe { instance.incref_object(snapshot.as_ptr()) };
        let iterator = instance.alloc(IteratorObject::new(
            iterator_type,
            snapshot,
            Cell::new(0),
        ));
        instance.release(snapshot);
        return Ok(iterator.into_raw().cast::<Header>());
    }
    // **子类覆写 `__iter__` ⇒ 不能走内建载荷快路**（round 111）：`LyingTuple.__iter__` 只吐 `1`，
    // 但底层元组是 `(2,)` ⇒ 参照 `list(LyingTuple((2,)))` 给 `[1]` ✓；先前按类型族直造
    // `tuple_iterator` ✗ ⇒ `[2]` ✗（`seq_tests.test_constructors` ✓）。只在"没有自己的 `__iter__`"
    // 时走快路 ✓。
    let overrides_iter = {
        let ty = instance.type_of(iterable);
        instance
            .type_lookup_owner(ty, "__iter__")
            .map(|(owner, _)| {
                ![
                    "list",
                    "tuple",
                    "dict",
                    "set",
                    "frozenset",
                    "str",
                    "bytes",
                    "bytearray",
                    "range",
                ]
                .iter()
                .any(|name| Some(owner) == instance.type_named(name))
            })
            .unwrap_or(false)
    };
    if !overrides_iter {
        if let Ok(iterator_type) = iterator_type_for(instance, iterable) {
        // 迭代器对象要**自己那一份**引用（本函数不消耗入参）
        // SAFETY: iterable 由调用方保证存活。
        unsafe { instance.incref_object(iterable.as_ptr()) };
        let iterator = instance.alloc(IteratorObject::new(
            iterator_type,
            iterable,
            Cell::new(0),
        ));
        return Ok(iterator.into_raw().cast::<Header>());
        }
    }
    // **走属性通道** ✓（第 627 轮修 ✗）：`__iter__` 可能由类型的 **`getattr` 槽**动态给出 ✓（`range` 就是 ✓）
    // —— 先前只走 `attribute_optional` ✗（不查那个槽 ✓）⇒ `list(range(3))`／`for` 一类报
    // `TypeError: 'range' object is not iterable` ✗（本轮 `range_builtin` 对拍实测 ✓）。
    // **特殊方法按 `type(iterable)` 的 MRO 找** ✓（类对象 ⇒ 元类 MRO ✓；第 410 轮，task-21 ✓）
    match crate::executor::attribute::attribute_lookup_special(instance, iterable, "__iter__") {
        Ok(crate::executor::Attribute::Method { function, this }) => {
            let result = call_callable_value(instance, function, Some(this))?;
            ensure_iterator_result(instance, result)
        }
        Ok(crate::executor::Attribute::Value(method))
        | Ok(crate::executor::Attribute::Owned(method)) => {
            let result = call_callable_value(instance, method, Some(iterable))?;
            ensure_iterator_result(instance, result)
        }
        Err(_) => {
            // **旧式序列协议** ✓（第 675 轮）：没有 `__iter__` 但**有 `__getitem__`** 的对象，参照按
            // `0,1,2,…` 依次取、遇 `IndexError` 收尾 ✓（`re._compiler` 的 `_compile` 迭代
            // `SubPattern` 就靠它 ✓）。这里**先物化**成列表再交给现成的 `list_iterator` ✓
            // （对 `re` 的用法等价 ✓；**如实记**：不是惰性 ✗，超大序列会先整体取完 ✓）。
            if let Some(getitem) =
                crate::executor::attribute::attribute_lookup_swallowing(
                    instance,
                    iterable,
                    "__getitem__",
                )
            {
                let mut collected: Vec<NonNull<Header>> = Vec::new();
                let mut index = 0i64;
                loop {
                    let key = instance.new_int(index);
                    let outcome = match getitem {
                        crate::executor::Attribute::Method { function, this } => {
                            crate::executor::call::call_callable(
                                instance, function, Some(this), vec![key], Vec::new(), 0,
                            )
                        }
                        crate::executor::Attribute::Value(method) => {
                            crate::executor::call::call_callable(
                                instance, method, Some(iterable), vec![key], Vec::new(), 0,
                            )
                        }
                        crate::executor::Attribute::Owned(method) => {
                            crate::executor::call::call_callable(
                                instance, method, Some(iterable), vec![key], Vec::new(), 0,
                            )
                        }
                    };
                    match outcome {
                        Ok(item) => {
                            collected.push(item);
                            index += 1;
                        }
                        Err(ExecError::Raised { exception }) => {
                            let raised = instance.type_of(exception);
                            let index_error = instance.type_named("IndexError");
                            if Some(raised) == index_error
                                || index_error.is_some_and(|base| instance.is_subtype(raised, base))
                            {
                                break;
                            }
                            return Err(ExecError::Raised { exception });
                        }
                        Err(other) => return Err(other),
                    }
                }
                let list = instance.new_list(collected);
                return iter_value(instance, list);
            }
            let name = instance.type_name(ty).to_owned();
            if crate::diag::flag("PYAWA_ITER_DEBUG") {
                eprintln!(
                    "[iter] 不可迭代：type={name} site={}\n{}",
                    instance.current_site(),
                    std::backtrace::Backtrace::force_capture()
                );
            }
            Err(raise_builtin(
                instance,
                "TypeError",
                &format!("'{name}' object is not iterable"),
            ))
        }
    }
}

/// **`__iter__` 的返回值必须真是迭代器**（round 105）：参照的 `iter()` 会校验
/// （`TypeError: iter() returned non-iterator of type 'X'` ✓）—— 先前原样交出去 ✗ ⇒ 官方
/// `seq_tests.IterNoNext` 的 `assertRaises(TypeError, list, IterNoNext(s))` 里，
/// 失败发生在后续 `next()` ⇒ 变成**状态 5 未接线** ✗（`test_list.test_constructors` ✓）。
fn ensure_iterator_result(
    instance: &Instance,
    iterator: NonNull<Header>,
) -> Result<NonNull<Header>, ExecError> {
    let ty = unsafe { iterator.as_ref() }.ty();
    let has_next = is_iterator_type(instance, ty)
        || ty == builtin_type(instance, "generator")
        || instance
            .type_lookup(ty, "__next__")
            .is_some();
    if has_next {
        return Ok(iterator);
    }
    let name = instance.type_name(ty);
    instance.release(iterator);
    Err(raise_builtin(
        instance,
        "TypeError",
        &format!("iter() returned non-iterator of type '{name}'"),
    ))
}

/// 判定真假——*临时*只覆盖单例表里的类型（`OM-11` 的 `__bool__` 槽位接线后改走协议）。
pub(crate) fn truthiness(instance: &Instance, raw: NonNull<Header>, _opcode: u8) -> Result<bool, ExecError> {
    // SAFETY: raw 是帧值栈上的存活对象。
    let ty = unsafe { raw.as_ref() }.ty();
    // **类型对象恒为真** ✓（参照：`bool(type)` 走 `type.__bool__` → 真；先前按"实例协议"去取
    // `__bool__` ⇒ 取到**类型字典里的原生实现**、却没有 self ⇒ `Unsupported`（`test_bool.test_types`
    // 遍历 `bool(t)` 时撞上 complex/float 一族 ✓）。
    if instance.is_type_object(raw) {
        return Ok(true);
    }
    let singletons = instance.singletons();
    if ty == singletons.none_type() {
        return Ok(false);
    }
    if ty == singletons.bool_type() {
        // SAFETY: 类型身份已确认（见本模块顶部"临时口径"）。
        return Ok(unsafe { &*raw.as_ptr().cast::<BoolObject>() }.value);
    }
    if ty == singletons.int_type() {
        // SAFETY: 同上。大整数走 `IntValue`（`int_value` 对它给 `None`，会被当成假）
        return Ok(instance.int_of(raw).map(|value| !value.is_zero()).unwrap_or(false));
    }
    // **内建容器的真假**（`OM-11` 的 `__bool__` 槽位接线前，按参照的**内建**规则 ✓）：
    // 空 `str`／`bytes`／`list`／`tuple`／`dict` ⇒ 假；`float` ⇒ `0.0`／`-0.0` 为假（`nan` 为真 ✓）。
    // 第 101 轮实测的触发器：`assert "x"`（上游 `importlib`／`site.py` 里满是这样用 ✓）。
    if instance
        .type_named("str")
        .is_some_and(|base| instance.is_subtype(ty, base))
    {
        // P3-38 S2a-2：真值按"码点/字节是否为空"（两分支一致 ✓；合法 UTF-8 结果不变 ✓）。
        return Ok(!instance.str_is_empty(raw).unwrap_or(false));
    }
    if instance
        .type_named("bytes")
        .is_some_and(|base| instance.is_subtype(ty, base))
    {
        // SAFETY: 同上。
        return Ok(!unsafe { &*raw.as_ptr().cast::<BytesObject>() }.value().is_empty());
    }
    if instance
        .type_named("list")
        .is_some_and(|base| instance.is_subtype(ty, base))
    {
        // SAFETY: 同上。
        return Ok(!unsafe { &*raw.as_ptr().cast::<ListObject>() }.items().is_empty());
    }
    if instance
        .type_named("tuple")
        .is_some_and(|base| instance.is_subtype(ty, base))
    {
        // SAFETY: 同上。
        return Ok(!unsafe { &*raw.as_ptr().cast::<TupleObject>() }.items().is_empty());
    }
    if instance
        .type_named("dict")
        .is_some_and(|base| instance.is_subtype(ty, base))
    {
        // SAFETY: 同上。
        return Ok(unsafe { &*raw.as_ptr().cast::<DictObject>() }.len() != 0);
    }
    if instance
        .type_named("float")
        .is_some_and(|base| instance.is_subtype(ty, base))
    {
        // SAFETY: 同上。
        //  在 IEEE 里为**假** ⇒ 与参照一致（ 为假 ✓）；
        //  为真 ⇒  为真 ✓（与参照一致）。
        return Ok(unsafe { &*raw.as_ptr().cast::<FloatObject>() }.value() != 0.0);
    }
    // **`complex` 的真值**（task-30）：`real != 0.0 || imag != 0.0` ✓（`nan` 为真 ✓，
    // 参照实测 `bool(complex(0,0)) is False` ✓）。放在 `__bool__` 协议之前（内建规则优先 ✓）。
    if let Some((real, imag)) = instance.complex_value(raw) {
        return Ok(real != 0.0 || imag != 0.0);
    }
    // **`deque` 的真值**（真 bug 修 ✗；sys-exit 在 `unittest.mock.patch` 收尾处抓到）：非空为真 ✓
    // —— 与 `len` 同一口径 ✓。先前缺这一条 ⇒ 落到下面的"`__bool__`／`__len__` 协议"都没有 ⇒
    // **默认为真** ✗ ⇒ 空 deque 恒真 ✗ ⇒ `Lib/contextlib.py` 的 `ExitStack.__exit__`
    //（`while self._exit_callbacks: …`）死循环 ✗ ⇒ `patch().stop()` 报
    // `pop from an empty deque` ✓（实测复现：`bool(deque())` 与参照相反 ✓）。
    if instance
        .type_named("deque")
        .is_some_and(|base| instance.is_subtype(ty, base))
    {
        // SAFETY: 类型身份已确认是 deque 族。
        return Ok(unsafe { &*raw.as_ptr().cast::<DequeObject>() }.len() != 0);
    }
    // **`set` 一族**（第 103 轮）：非空为真 ✓（与 `len` 同一口径 ✓）—— 上限榜那一整族 **119** 个模块
    // 的第一句错就是 `if some_set:` ✓（`Lib/enum.py` 里满是这样用 ✓）。认**子类型** ✓。
    if ["set", "frozenset"].iter().any(|name| {
        instance
            .type_named(name)
            .is_some_and(|base| instance.is_subtype(ty, base))
    }) {
        // SAFETY: 类型身份已确认是 set 族。
        return Ok(!unsafe { &*raw.as_ptr().cast::<SetObject>() }.is_empty());
    }
    // **`__bool__`／`__len__` 协议** ✓（第 103 轮）：前五条都是**内建**规则 ✓；自定义类要走协议 ✓
    // —— 先 `__bool__` ✓（用它的返回值判真值 ✓），没有就 `__len__` ✓（非零为真 ✓），
    // 两者都没有 ⇒ **默认为真** ✓（照参照 ✓，不是报错 ✗ —— 先前这里直接 `Unsupported` ✗）。
    if let Some(method) = crate::executor::attribute_optional(instance, raw, "__bool__")? {
        let result = call_value(instance, method, &[], &[])?;
        release(instance, method);
        // **`__bool__` 必须返回 `bool`** ✓（参照实测：`self`／`"Yes"`／`1` 都报 `TypeError`，
        // 文案 `__bool__ should return bool, returned <类型名>`）。先前对返回值**再取一次真值** ✗
        // ⇒ `__bool__` 返回 `self` 时**无限递归** ⇒ Rust 栈溢出 abort ✗（`test_bool` 实测 ✓）。
        let result_type = unsafe { result.as_ref() }.ty();
        if result_type == instance.singletons().bool_type() {
            // SAFETY: 类型身份已确认。
            let truth = unsafe { &*result.as_ptr().cast::<crate::builtin_objects::BoolObject>() }.value;
            release(instance, result);
            return Ok(truth);
        }
        let type_name = instance.type_name(result_type).to_owned();
        release(instance, result);
        return Err(raise_builtin(
            instance,
            "TypeError",
            &format!("__bool__ should return bool, returned {type_name}"),
        ));
    }
    if let Some(method) = crate::executor::attribute_optional(instance, raw, "__len__")? {
        let result = call_value(instance, method, &[], &[])?;
        release(instance, method);
        // **`__len__` 的返回值**：整数（含大整数 ⇒ `OverflowError`）且 `>= 0`（否则 `ValueError`）✓
        // —— 参照实测：`-1` ⇒ `ValueError: __len__() should return >= 0`；`"x"` ⇒
        // `TypeError: 'str' object cannot be interpreted as an integer` ✓。
        let Some(number) = instance.int_of(result) else {
            let type_name = instance.type_name(unsafe { result.as_ref() }.ty()).to_owned();
            release(instance, result);
            return Err(raise_builtin(
                instance,
                "TypeError",
                &format!("'{type_name}' object cannot be interpreted as an integer"),
            ));
        };
        let Some(length) = number.to_i64() else {
            release(instance, result);
            return Err(raise_builtin(
                instance,
                "OverflowError",
                "cannot fit 'int' into an index-sized integer",
            ));
        };
        release(instance, result);
        if length < 0 {
            return Err(raise_builtin(
                instance,
                "ValueError",
                "__len__() should return >= 0",
            ));
        }
        return Ok(length != 0);
    }
    Ok(true)
}

/// 该类型是不是内建 `name`（**含子类** ✓）。
///
/// **task-14 真 bug 修** ✗：先前 `iterable_length`／`iterable_item` 对 tuple／list／set／str／bytes
/// 按**恰好**是那个内建类型判 ✗ ⇒ **子类**（`namedtuple` 的实例 ✓）报"只接线了…的内建迭代器" ✓
/// ⇒ `list(Point(1,2))`／`_asdict()` 全废 ✓。载荷同构 ⇒ 照用 ✓（与 `dict` 那条同一把尺子 ✓）。
fn type_is_subtype_of(
    instance: &Instance,
    ty: NonNull<crate::TypeObject>,
    name: &str,
) -> bool {
    instance
        .type_named(name)
        .is_some_and(|base| instance.is_subtype(ty, base))
}

/// 被迭代对象的元素个数。
pub(crate) fn iterable_length(
    instance: &Instance,
    raw: NonNull<Header>,
    opcode: u8,
) -> Result<usize, ExecError> {
    // SAFETY: raw 是存活对象。
    let ty = unsafe { raw.as_ref() }.ty();
    if type_is_subtype_of(instance, ty, "tuple") {
        // SAFETY: 类型身份已确认。
        return Ok(unsafe { &*raw.as_ptr().cast::<TupleObject>() }.len());
    }
    if type_is_subtype_of(instance, ty, "list") {
        // SAFETY: 同上。
        return Ok(unsafe { &*raw.as_ptr().cast::<ListObject>() }.len());
    }
    if instance.is_subtype(ty, builtin_type(instance, "dict")) {
        // SAFETY: 同上。
        return Ok(unsafe { &*raw.as_ptr().cast::<DictObject>() }.len());
    }
    if type_is_subtype_of(instance, ty, "set") || type_is_subtype_of(instance, ty, "frozenset") {
        // **`frozenset` 与 `set` 同一份载荷**（第 292 轮）：`UNPACK_SEQUENCE` 一族按元素个数
        // 走这条路 ✓ ⇒ 先前只认 `set` ✗ ⇒ `'frozenset' object is not iterable` 之后又撞一条 ✗。
        // SAFETY: 同上。
        return Ok(unsafe { &*raw.as_ptr().cast::<SetObject>() }.len());
    }
    if type_is_subtype_of(instance, ty, "str") {
        // P3-38 S2a-2：`len(str)` 按**码点计数**（代理各算 1 ✓；合法 UTF-8 与旧口径一致 ✓）。
        if let Some(length) = instance.str_code_point_len(raw) {
            return Ok(length);
        }
    }
    if type_is_subtype_of(instance, ty, "bytes") {
        // SAFETY: 同上。
        return Ok(unsafe { &*raw.as_ptr().cast::<BytesObject>() }.value().len());
    }
    if type_is_subtype_of(instance, ty, "bytearray") {
        // **`bytearray` 也要认** ✓（第 645 轮）：载荷是 `BytearrayObject` ✓（`bytes` 那条对不上 ✗）
        // ⇒ `list(bytearray(…))` 先前报"只接线了 tuple／list／dict／set／str／bytes 的内建迭代器" ✗。
        // SAFETY: 类型身份已确认。
        return Ok(unsafe { &*raw.as_ptr().cast::<crate::builtin_objects::BytearrayObject>() }
            .value()
            .len());
    }
    Err(ExecError::Unsupported {
        opcode,
        what: Box::leak(
            format!(
                "'{}'：只接线了 tuple／list／dict／set／str／bytes 的内建迭代器（其余走 __iter__ 协议）",
                instance.type_name(ty)
            )
            .into_boxed_str(),
        ),
    })
}

/// 取被迭代对象的第 `index` 个元素（**新引用**；`str` 会造一个单字符 `str`）。
pub(crate) fn iterable_item(
    instance: &Instance,
    raw: NonNull<Header>,
    index: usize,
    opcode: u8,
) -> Result<NonNull<Header>, ExecError> {
    // SAFETY: raw 是存活对象。
    let ty = unsafe { raw.as_ref() }.ty();

    let owned = |value: NonNull<Header>| {
        // SAFETY: value 由容器持有，存活。
        unsafe { instance.incref_object(value.as_ptr()) };
        value
    };

    if type_is_subtype_of(instance, ty, "tuple") {
        // SAFETY: 类型身份已确认。
        let value = unsafe { &*raw.as_ptr().cast::<TupleObject>() }.item(index);
        return value.map(owned).ok_or(ExecError::Unsupported {
            opcode,
            what: "迭代器游标越界",
        });
    }
    if type_is_subtype_of(instance, ty, "list") {
        // SAFETY: 同上。
        let value = unsafe { &*raw.as_ptr().cast::<ListObject>() }.item(index);
        return value.map(owned).ok_or(ExecError::Unsupported {
            opcode,
            what: "迭代器游标越界",
        });
    }
    if instance.is_subtype(ty, builtin_type(instance, "dict")) {
        // SAFETY: 同上。字典迭代的是**键**（与参照实现一致）
        let value = unsafe { &*raw.as_ptr().cast::<DictObject>() }
            .entry(index)
            .map(|(key, _)| key);
        return value.map(owned).ok_or(ExecError::Unsupported {
            opcode,
            what: "迭代器游标越界",
        });
    }
    if type_is_subtype_of(instance, ty, "set") || type_is_subtype_of(instance, ty, "frozenset") {
        // **`frozenset` 与 `set` 同一份载荷**（第 292 轮）：按游标取元素这条路也要认它 ✓
        //（`_collections_abc` 注册基类时迭代集合 ✓，元类路径打通后当场踩到 ✓）。
        // SAFETY: 同上。
        let value = unsafe { &*raw.as_ptr().cast::<SetObject>() }.item(index);
        return value.map(owned).ok_or(ExecError::Unsupported {
            opcode,
            what: "迭代器游标越界",
        });
    }
    if type_is_subtype_of(instance, ty, "str") {
        // P3-38 S2a-2 批 2：按**码点**取第 `index` 个（孤立代理也各占一格 ✓），单字符对象走
        // `new_str_code_point`（标量⇒UTF-8 表示；代理⇒WTF-8 表示 ✓）。
        let point = instance
            .str_code_point_at(raw, index)
            .ok_or(ExecError::Unsupported {
                opcode,
                what: "迭代器游标越界",
            })?;
        return Ok(instance.new_str_code_point(point));
    }
    // `bytes`：迭代给**整数**（实测 `list(b'ab') == [97, 98]`）
    if type_is_subtype_of(instance, ty, "bytes") {
        // SAFETY: 同上。
        let value = unsafe { &*raw.as_ptr().cast::<BytesObject>() }.value().to_vec();
        let byte = *value.get(index).ok_or(ExecError::Unsupported {
            opcode,
            what: "迭代器游标越界",
        })?;
        return Ok(instance.new_int(i64::from(byte)));
    }
    // **`bytearray`：与 `bytes` 同款，按整数给** ✓（第 647 轮 ✗ 修）：载荷是 `BytearrayObject` ✓
    // —— 先前这条 `iterable_item` 只认 `bytes` ✗ ⇒ `list(bytearray(…))` 报"只接线了 tuple／list／dict／
    // set／str／bytes 的迭代" ✗（**源码里那句本身就是短的** ✗，所以前几轮加的类型名永远显不出来 ✓）。
    if type_is_subtype_of(instance, ty, "bytearray") {
        // SAFETY: 类型身份已确认。
        let value = unsafe { &*raw.as_ptr().cast::<crate::builtin_objects::BytearrayObject>() }
            .value()
            .clone();
        let byte = *value.get(index).ok_or(ExecError::Unsupported {
            opcode,
            what: "迭代器游标越界",
        })?;
        return Ok(instance.new_int(i64::from(byte)));
    }
    Err(ExecError::Unsupported {
        opcode,
        what: Box::leak(
            format!(
                "'{}'：只接线了 tuple／list／dict／set／str／bytes／bytearray 的迭代",
                instance.type_name(ty)
            )
            .into_boxed_str(),
        ),
    })
}

/// **`CONTAINS_OP`**（`in`／`not in`）的判定：`str`／`list`／`tuple`／`dict`／`set`。
///
/// 实测的两条错误消息（**禁止**近似）：
/// - 容器不是那几类 ⇒ `argument of type 'X' is not a container or iterable`
/// - 容器是 `str` 而左操作数不是 `str` ⇒ `'in <string>' requires string as left operand, not X`
///
/// 元素比较走 [`values_equal`]（本层口径：整数／浮点／字符串按值，其余**按身份**）。
/// 所以容器之间的值相等（`[] in [[], []]`、`1 == [1]`）**尚未**接通——那是
/// `OM-11` 的 `richcompare` 槽位那一摊（`lib.rs` 的清单里记着），不是 `in` 自己的事。
pub(crate) fn contains(
    instance: &Instance,
    container: NonNull<Header>,
    item: NonNull<Header>,
    opcode: u8,
) -> Result<bool, ExecError> {
    // SAFETY: container 是帧值栈上的存活对象。
    let container_type = unsafe { container.as_ref() }.ty();
    if container_type == instance.singletons().str_type() {
        // P3-38 S2a-2 批 2：`in` 改**安全口径**——两边都是 str 时按 WTF-8 字节子串判 ✓
        //（自同步编码 ⇒ 等价码点子序列 ✓，见 `Instance::str_contains` 的注释）。
        if let Some(found) = instance.str_contains(container, item) {
            return Ok(found);
        }
        // 左操作数不是 str ⇒ 参照的 TypeError 文案 ✓
        // SAFETY: item 是存活对象。
        let item_type = unsafe { item.as_ref() }.ty();
        // SAFETY: 类型名由注册表持有。
        let name = unsafe { item_type.as_ref() }.name();
        return Err(raise_builtin(
            instance,
            "TypeError",
            &format!("'in <string>' requires string as left operand, not {name}"),
        ));
    }
    // **`range` 的成员判定** ✓（round 56）：参照的 `in range(...)` 是**算术**判定，不是容器迭代
    // —— `1.0 in range(3)` ⇒ True、`'a' in range(3)` ⇒ False、`range(3) in range(3)` ⇒ False ✓。
    // 先前没有这一档 ✗ ⇒ 落进"不是容器"的 TypeError ✗（`test_genexps` 的
    // `... if j in range(n)` ✓）。
    if Some(container_type) == instance.type_named("range") {
        // SAFETY: 类型身份已确认，载荷布局是 `RangeObject`。
        let range = unsafe { &*container.as_ptr().cast::<RangeObject>() };
        // 只认整数（含 `bool`／`int` 子类）与**整值浮点**（参照对 `1.0` 也判 True ✓）。
        let value: Option<i64> = instance.int_value(item).or_else(|| {
            instance.float_value(item).and_then(|number| {
                let min = i64::MIN as f64;
                let max = i64::MAX as f64;
                if number.is_finite() && number.fract() == 0.0 && number >= min && number <= max {
                    Some(number as i64)
                } else {
                    None
                }
            })
        });
        let Some(value) = value else {
            return Ok(false);
        };
        let (start, stop, step) = (range.start(), range.stop(), range.step());
        let contained = if step > 0 {
            value >= start && value < stop && (value - start) % step == 0
        } else {
            value <= start && value > stop && (start - value) % (-step) == 0
        };
        return Ok(contained);
    }
    // `bytes`：**子串**查找（实测 `b'ab' in b'abc'`）；左操作数不是 bytes 时报实测的消息。
    // **整数那一档**（第 284 轮按参照实测补 ✓）：`98 in b"b"` ⇒ `True` ✓、
    // `300 in b"ab"`／`(-1) in b"ab"` ⇒ `ValueError: byte must be in range(0, 256)` ✓、
    // `"a" in b"ab"` ⇒ `TypeError: a bytes-like object is required, not 'str'` ✓
    //（`Lib/` 里 `codecs`／`base64_codec` 一族真的会 `b in bytes` 判字节 ✓）。
    if Some(container_type) == instance.type_named("bytes") {
        let value = instance.bytes_value(container).unwrap_or_default().to_vec();
        // **认 `int` 及其子类** ✓（第 681 轮真 bug 修 ✗）：先前用 `type_of(item) == int` ✗ ⇒
        // `re._constants` 的 `_NamedIntConstant`（**`int` 的子类** ✓）落不进这一档 ✓ ⇒ 明明给的是整数，
        // 却报 `TypeError: a bytes-like object is required, not 'int'` ✗（`_optimize_charset` 里
        // `x in b"…"` 一族当场踩到 ✓；`bool` 也是 `int` 的子类 ✓，参照口径一致 ✓）。
        if instance
            .type_named("int")
            .is_some_and(|int_type| instance.is_subtype(instance.type_of(item), int_type))
        {
            // 超出 `i64` 的整数一定不在 0..256 ✓（参照给的是同一条 `ValueError` ✓）
            let byte = instance.int_value(item).unwrap_or(-1);
            if !(0..256).contains(&byte) {
                return Err(raise_builtin(
                    instance,
                    "ValueError",
                    "byte must be in range(0, 256)",
                ));
            }
            return Ok(value.contains(&(byte as u8)));
        }
        let Some(needle) = instance.bytes_value(item).map(<[u8]>::to_vec) else {
            let name = instance.type_name(instance.type_of(item));
            return Err(raise_builtin(
                instance,
                "TypeError",
                &{
                    if crate::diag::flag("PYAWA_BYTESLIKE_DEBUG") {
                        eprintln!(
                            "[byteslike] 类型={name} 站点={}",
                            instance.current_site()
                        );
                    }
                    format!("a bytes-like object is required, not '{name}'")
                },
            ));
        };
        if needle.is_empty() {
            return Ok(true);
        }
        return Ok(value.windows(needle.len()).any(|window| window == needle.as_slice()));
    }
    if instance.is_subtype(container_type, builtin_type(instance, "list")) {
        // **富比较协议**（round 110）：`in` 对序列必须逐项走 `==`（含用户 `__eq__`／反射／
        // `NotImplemented` 回退 ✓，并按**顺序**短路 ✓，异常原样传播 ✓）—— 先前用只认内建值的
        // `values_equal` ✗ ⇒ `ALWAYS_EQ in [1]` 判 False ✗、`StopCompares` 的异常被吞 ✗
        //（官方 `seq_tests.test_contains_fake`／`test_contains_order` ✓）。比较次序照参照
        // `PyObject_RichCompareBool(item, el, Py_EQ)` ⇒ **查找值在左** ✓。
        // SAFETY: 类型身份已确认。
        let object = unsafe { &*container.as_ptr().cast::<ListObject>() };
        // **长度每轮重读**（round 110 真 bug 修 ✗）：`__eq__` 可能把被查列表改小
        //（`list_tests.test_contains_fake` 的 `EvilCmp`／`test_equal_operator_modifying_operand` ✓）；
        // 参照 `list_contains` 也是每轮 `PyList_GET_SIZE(a)` ✓ —— 先前先取 `0..len` 再逐项下标 ✗
        // ⇒ 缩表后 `item()` 越界 panic ✗。
        let mut index = 0usize;
        while index < object.len() {
            let element = object.item(index).expect("刚查过长度，下标在范围内");
            // **比较期间自己持一份**（round 110 真 bug 修 ✗）：`__eq__` 可能把列表清空／改小
            // ⇒ 借用视图立刻悬垂 ✗（先前直接比较 ⇒ `test_list` 段错误 ✗；参照 `list_contains`
            // 先 `Py_INCREF` 再比 ✓）。
            unsafe { instance.incref_object(element.as_ptr()) };
            let verdict = crate::executor::protocol::equality_public(instance, element, item);
            unsafe { instance.release_object(element.as_ptr()) };
            if verdict? {
                return Ok(true);
            }
            index += 1;
        }
        return Ok(false);
    }
    if instance.is_subtype(container_type, builtin_type(instance, "tuple")) {
        // SAFETY: 同上。
        let object = unsafe { &*container.as_ptr().cast::<TupleObject>() };
        for index in 0..object.len() {
            let element = object.item(index).expect("下标在范围内");
            if crate::executor::protocol::equality_public(instance, item, element)? {
                return Ok(true);
            }
        }
        return Ok(false);
    }
    if instance.is_subtype(container_type, builtin_type(instance, "dict")) {
        // 不可哈希的项 ⇒ 参照的 TypeError ✓（task-46；`[1] in d` ⇒
        // `cannot use 'list' as a dict key (unhashable type: 'list')` ✓）。
        crate::builtin::dict::reject_unhashable_key(instance, item)?;
        // **哈希索引 ＋ `==` 语义**（round 116）：先前逐项"结构相等" ✗ ⇒ 用户键（覆写
        // `__hash__`／`__eq__`）查不到 ✗，且大表 O(n)。`dict_lookup_key` 已含不可哈希线性兜底 ✓。
        return Ok(instance.dict_lookup_key(container, item).is_some());
    }
    // **`set` 有自己的一份**（第 236 轮修 SIGSEGV）：此前这一支和 `dict` 合在一起、把 set 强转成
    // `DictObject` 再遍历 `entries()` ⇒ **类型混淆**、读越界直接崩（推导式能造集合后才被触发）
    // **`frozenset` 与 `set` 同一份载荷** ✓（第 236 轮 ✓）⇒ `in` 也要一并认 ✓
    //（第 283 轮修 ✗：先前只认 `set` ✗ ⇒ `1 in frozenset([1, 2])` 报
    //  `TypeError: argument of type 'frozenset' is not a container or iterable` ✗ ——
    //  `collections` 那一族 **12** 个模块压在它上面 ✓）。
    if instance.is_subtype(container_type, builtin_type(instance, "set"))
        || Some(container_type) == instance.type_named("frozenset")
    {
        // **哈希索引**（round 70）：先前逐元素线性扫 ⇒ `name in seen` 在 8000 项集合上
        // 2.5 s ⇒ O(n²) ✗（`test_collections.test_large_size` 因此超时）；走
        // `set_contains`（惰性哈希索引 ✓，不可哈希退回线性 ✓）。
        return Ok(crate::builtin::set::set_contains(instance, container, item).is_some());
    }
    // **协议回退**（第 514 轮真 bug 修 ✗）：类型自带 `__contains__` ⇒ 调它 ✓
    // （参照口径：`in` **先**走 `__contains__` ✓、再走 `__iter__` ✓）。
    // 实测：`'X' in os.environ`（`os._Environ`，`collections.abc.MutableMapping` 的子类 ✓）先前直接报
    // `argument of type '_Environ' is not a container or iterable` ✗ ⇒ 这一条压着
    // `xml.sax`（5 个 ✓）＋ `xml.dom.pulldom` ✓（`tools/next_work.py` 的队列 ✓）。
    if let Some(found) = instance.type_lookup(container_type, "__contains__") {
        // **`__contains__ = None` 会阻断迭代回退** ✓（参照口径；`test_contains.test_block_fallback`
        // 钉住）：此时报"不是容器"的 TypeError ✓，**不**再按 `__iter__` 回退 ✓。
        if found == instance.singletons().none() {
            // SAFETY: container 是存活对象。
            let name = unsafe { container_type.as_ref() }.name();
            return Err(raise_builtin(
                instance,
                "TypeError",
                &format!("argument of type '{name}' is not a container or iterable"),
            ));
        }
        // 实参按"借用视图"交给 `call_callable`（它自己会 retain ✓）。`bound=Some(container)` 已把
        // `self` 补进实参表 ✗ ⇒ 这里**只**交 `item`（先前多交一个 `container` ⇒ 实测
        // `TypeError: __contains__() takes 2 positional arguments but 3 were given` ✗）。
        instance.retain(item);
        let returned = crate::executor::call::call_callable(
            instance,
            found,
            Some(container),
            vec![item],
            Vec::new(),
            opcode,
        )?;
        let truth = crate::executor::truthiness(instance, returned, opcode)?;
        crate::executor::release(instance, returned);
        return Ok(truth);
    }
    // **迭代回退**（round 86，参照 `in` 的第二条通道）：没有 `__contains__` 的对象按 `__iter__`／
    // 旧式 `__getitem__`／内建可迭代（`deque` 等）逐个比较 ✓（`test_contains` 的 `seq`／`deque` 钉住）。
    // 拿不到迭代器（既不是容器也不能迭代）⇒ 仍报原来那句 TypeError ✓。
    if let Ok(iterator) = iter_value(instance, container) {
        let mut found = false;
        loop {
            match crate::executor::advance_iterator(instance, iterator, opcode) {
                Ok(Some(value)) => {
                    if values_equal(instance, value, item) {
                        crate::executor::release(instance, value);
                        found = true;
                        break;
                    }
                    crate::executor::release(instance, value);
                }
                Ok(None) => break,
                Err(error) => {
                    crate::executor::release(instance, iterator);
                    return Err(error);
                }
            }
        }
        crate::executor::release(instance, iterator);
        return Ok(found);
    }
    // SAFETY: container 是存活对象。
    let _ = opcode;
    let name = unsafe { container_type.as_ref() }.name();
    Err(raise_builtin(
        instance,
        "TypeError",
        &format!("argument of type '{name}' is not a container or iterable"),
    ))
}

/// 把 Rust 的 `1.234568e4` 归一成 C 的 `1.234568e+04` ✓（指数至少两位、带符号 ✓）。
pub(crate) fn normalize_exponent(text: &str, upper: bool) -> String {
    let Some((mantissa, exponent)) = text.split_once(['e', 'E']) else {
        return text.to_owned();
    };
    let exponent: i32 = exponent.parse().unwrap_or(0);
    let marker = if upper { 'E' } else { 'e' };
    format!("{mantissa}{marker}{}{:02}", if exponent < 0 { '-' } else { '+' }, exponent.abs())
}

/// 去掉 `%g` 结果末尾多余的零与孤立的小数点 ✓。
pub(crate) fn strip_trailing_zeros(text: &str) -> String {
    let Some((mantissa, exponent)) = text.split_once(['e', 'E']) else {
        if text.contains('.') {
            return text.trim_end_matches('0').trim_end_matches('.').to_owned();
        }
        return text.to_owned();
    };
    let trimmed = if mantissa.contains('.') {
        mantissa.trim_end_matches('0').trim_end_matches('.')
    } else {
        mantissa
    };
    let exponent: i32 = exponent.parse().unwrap_or(0);
    if exponent == 0 {
        trimmed.to_owned()
    } else {
        format!("{trimmed}e{}{:02}", if exponent < 0 { '-' } else { '+' }, exponent.abs())
    }
}

/// **序列拼接／`+` 的公开入口**（`operator.concat` 与 `operator.add` 共用；将来 `BINARY_OP` 的 `+` 也用它）。
/// 实测：`concat(['a'], ['b'])` 与 `add(['a'], ['b'])` **都是**拼接 ⇒ 两者同一条路。
/// 支持 `str`／`list`／`tuple` 拼接；其余（含整数）落到 [`arithmetic_public`] 的 `+`
/// （整数相加、非可比报实测消息）。
pub fn concat_public(
    instance: &Instance,
    left: NonNull<Header>,
    right: NonNull<Header>,
    opcode: u8,
) -> Result<NonNull<Header>, ExecError> {
    // SAFETY: 两个都是存活对象（调用方保证）。
    let (left_type, right_type) = unsafe { (left.as_ref().ty(), right.as_ref().ty()) };
    // **`str + str`**（P3-38 S2a-2 批 2）：在 **WTF-8 字节层**拼接 ✓（合法 UTF-8 的拼接字节与旧口径
    // 完全一致；含孤立代理的串也能拼 ✓）。结果走 `new_str_wtf8` ⇒ 合法 UTF-8 结果仍存 Utf8 ✓。
    if instance.is_str_object(left) && instance.is_str_object(right) {
        let mut joined = instance
            .str_wtf8_bytes(left)
            .expect("刚判过是 str")
            .to_vec();
        joined.extend_from_slice(instance.str_wtf8_bytes(right).expect("刚判过是 str"));
        return Ok(instance
            .new_str_wtf8(&joined)
            .expect("两个合法 WTF-8 的拼接仍合法"));
    }
    // **`bytearray` 拼接** ✓（第 643 轮）：任一操作数是 `bytearray` ⇒ 结果给 **`bytearray`** ✓（参照口径 ✓）；
    // 另一侧要是 `bytes`／`bytearray` ✓。`re._compiler` 的 `data += chunk` 正需要它 ✓
    //（先前报 `unsupported operand type(s) for +: 'bytearray' and 'bytes'` ✗）。
    let is_byte_like = |object: NonNull<Header>| {
        matches!(
            instance.type_name(instance.type_of(object)).as_str(),
            "bytes" | "bytearray" | "memoryview"
        )
    };
    if (instance.type_name(left_type) == "bytearray"
        || instance.type_name(right_type) == "bytearray")
        && is_byte_like(left)
        && is_byte_like(right)
    {
        let mut joined: Vec<u8> = Vec::new();
        for side in [left, right] {
            let side_type = instance.type_name(instance.type_of(side));
            if side_type == "bytearray" {
                // SAFETY: 类型身份已确认，载荷就是 `BytearrayObject`。
                let data = unsafe { &*side.as_ptr().cast::<crate::builtin_objects::BytearrayObject>() };
                joined.extend(data.value().iter().copied());
            } else if side_type == "memoryview" {
                // **`bytearray += memoryview`**（round 91，性能修 ✗）：`Lib/_pyio.py` 的
                // `BufferedWriter.write` 是 `self._write_buf += memoryview(b)` ✓ —— 先前这一档
                // 落进通用迭代（逐个元素走 Python 协议）⇒ 100 KB 要 0.27 s ✗（`test_bufio`
                // 因此 >200 s、超出调查器 120 s 超时 ✓）。这里按 `tobytes` 同一口径**整段拷贝** ✓。
                if let Some((source, _format, _itemsize, offset, nbytes)) =
                    crate::builtin_objects::memoryview_fields(instance, side)
                {
                    if let Some(bytes) = crate::builtin_objects::memoryview_source_bytes(instance, source) {
                        let end = (offset + nbytes).min(bytes.len());
                        joined.extend_from_slice(&bytes[offset.min(end)..end]);
                    }
                }
            } else {
                // SAFETY: 同上（另一侧是 `bytes`）。
                let data = unsafe { &*side.as_ptr().cast::<crate::builtin_objects::BytesObject>() };
                joined.extend(data.value().iter().copied());
            }
        }
        let ty = instance
            .type_named("bytearray")
            .expect("引导期已登记 bytearray 类型");
        return Ok(instance
            .alloc(crate::builtin_objects::BytearrayObject::new(
                ty,
                std::cell::RefCell::new(joined),
            ))
            .into_raw()
            .cast::<Header>());
    }
    // `bytes + bytes`（`P1-12`；实测 `b'ab' + b'cd' == b'abcd'`）
    // **`bytes` 子类**也收（`bytearray` 已在上一支处理 ✓）：载荷视图统一走 `bytes_like_value` ✓。
    let (left_bytes, right_bytes) = (
        instance.bytes_like_value(left),
        instance.bytes_like_value(right),
    );
    if let (Some(a), Some(b)) = (left_bytes, right_bytes) {
        let mut joined = Vec::with_capacity(a.len() + b.len());
        joined.extend_from_slice(&a);
        joined.extend_from_slice(&b);
        return Ok(instance.new_bytes(&joined));
    }
    // **序列拼接收子类**（round 104）：参照 `list.__add__` 对 **list 子类**同样成立 ✓
    //（官方 `test_xml_dom_minicompat` 的 `NodeList + list`／`list + NodeList` ✓；结果给**普通 list** ✓
    // —— 除非子类自己覆写了 `__add__` ✓，那条在 `binary_override` 已先走 ✓）。
    let is_list_type = |ty: NonNull<crate::TypeObject>| {
        instance
            .type_named("list")
            .is_some_and(|base| instance.is_subtype(ty, base))
    };
    if is_list_type(left_type) && is_list_type(right_type) {
        // SAFETY: 类型身份已确认。
        let (a, b) = unsafe {
            (
                &*left.as_ptr().cast::<ListObject>(),
                &*right.as_ptr().cast::<ListObject>(),
            )
        };
        let mut items: Vec<NonNull<Header>> = Vec::with_capacity(a.len() + b.len());
        for index in 0..a.len() {
            if let Some(item) = a.item(index) {
                // SAFETY: 值由列表持有，存活；新列表要自己那份。
                unsafe { instance.incref_object(item.as_ptr()) };
                items.push(item);
            }
        }
        for index in 0..b.len() {
            if let Some(item) = b.item(index) {
                // SAFETY: 同上。
                unsafe { instance.incref_object(item.as_ptr()) };
                items.push(item);
            }
        }
        return Ok(instance.new_list(items));
    }
    // 同上：**tuple 子类**也按 `tuple.__add__` 拼接 ✓（结果给普通 tuple ✓）。
    let is_tuple_type = |ty: NonNull<crate::TypeObject>| {
        instance
            .type_named("tuple")
            .is_some_and(|base| instance.is_subtype(ty, base))
    };
    if is_tuple_type(left_type) && is_tuple_type(right_type) {
        // SAFETY: 类型身份已确认。
        let (a, b) = unsafe {
            (
                &*left.as_ptr().cast::<TupleObject>(),
                &*right.as_ptr().cast::<TupleObject>(),
            )
        };
        let mut items: Vec<NonNull<Header>> = Vec::with_capacity(a.len() + b.len());
        for index in 0..a.len() {
            if let Some(item) = a.item(index) {
                // SAFETY: 同上。
                unsafe { instance.incref_object(item.as_ptr()) };
                items.push(item);
            }
        }
        for index in 0..b.len() {
            if let Some(item) = b.item(index) {
                // SAFETY: 同上。
                unsafe { instance.incref_object(item.as_ptr()) };
                items.push(item);
            }
        }
        return Ok(instance.new_tuple(items));
    }
    arithmetic_public(instance, left, right, "+", opcode)
}
