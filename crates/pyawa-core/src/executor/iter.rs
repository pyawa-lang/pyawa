//! **`iter` 族** —— 从 `builtin_objects.rs` **原样搬来** ✓（纯移动 ✓、零逻辑改动 ✓）。
//!
//! * 派发表 `iter_method_native` **不搬** ✓（族之间的粘合层 ✓）；
//! * 仍在原处的被引用项：（无） ✓；
//! * `use` 块照抄原文件 ✓（`cargo fix` 随后删多余的 ✓），缺失项由自愈循环补 ✓。
use crate::builtin_objects::BoolObject;
use crate::builtin_objects::BytesObject;
use core::cell::Cell;
use crate::builtin_objects::DictObject;
use crate::executor::ExecError;
use crate::builtin_objects::FloatObject;
use crate::header::Header;
use crate::instance::Instance;
use crate::builtin_objects::IteratorObject;
use crate::builtin_objects::ListObject;
use core::ptr::NonNull;
use crate::builtin_objects::SetObject;
use crate::builtin_objects::StrObject;
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
    // **走属性通道** ✓（第 627 轮修 ✗）：`__iter__` 可能由类型的 **`getattr` 槽**动态给出 ✓（`range` 就是 ✓）
    // —— 先前只走 `attribute_optional` ✗（不查那个槽 ✓）⇒ `list(range(3))`／`for` 一类报
    // `TypeError: 'range' object is not iterable` ✗（本轮 `range_builtin` 对拍实测 ✓）。
    match crate::executor::attribute_lookup(instance, iterable, "__iter__") {
        Ok(crate::executor::Attribute::Method { function, this }) => {
            let result = call_callable_value(instance, function, Some(this));
            result
        }
        Ok(crate::executor::Attribute::Value(method))
        | Ok(crate::executor::Attribute::Owned(method)) => {
            let result = call_callable_value(instance, method, Some(iterable));
            result
        }
        Err(_) => {
            // **旧式序列协议** ✓（第 675 轮）：没有 `__iter__` 但**有 `__getitem__`** 的对象，参照按
            // `0,1,2,…` 依次取、遇 `IndexError` 收尾 ✓（`re._compiler` 的 `_compile` 迭代
            // `SubPattern` 就靠它 ✓）。这里**先物化**成列表再交给现成的 `list_iterator` ✓
            // （对 `re` 的用法等价 ✓；**如实记**：不是惰性 ✗，超大序列会先整体取完 ✓）。
            if let Ok(getitem) = crate::executor::attribute_lookup(instance, iterable, "__getitem__") {
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

/// 判定真假——*临时*只覆盖单例表里的类型（`OM-11` 的 `__bool__` 槽位接线后改走协议）。
pub(crate) fn truthiness(instance: &Instance, raw: NonNull<Header>, opcode: u8) -> Result<bool, ExecError> {
    // SAFETY: raw 是帧值栈上的存活对象。
    let ty = unsafe { raw.as_ref() }.ty();
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
        // SAFETY: 类型身份已确认是 `str`。
        return Ok(!unsafe { &*raw.as_ptr().cast::<StrObject>() }.value().is_empty());
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
        let truth = truthiness(instance, result, opcode)?;
        release(instance, result);
        release(instance, method);
        return Ok(truth);
    }
    if let Some(method) = crate::executor::attribute_optional(instance, raw, "__len__")? {
        let result = call_value(instance, method, &[], &[])?;
        // `__len__` 返回的是**整数** ✓（不是容器 ✓）⇒ 按整数判零 ✓（`length_of` 对它给 `None` ✗，
        // 先前拿 `None` 兜成"真" ⇒ `bool(WithLen(0))` 错成 `True` ✗，探针当场抓到 ✓）。
        let truth = instance
            .int_of(result)
            .map(|value| !value.is_zero())
            .unwrap_or(true);
        release(instance, result);
        release(instance, method);
        return Ok(truth);
    }
    Ok(true)
}

/// 被迭代对象的元素个数。
pub(crate) fn iterable_length(
    instance: &Instance,
    raw: NonNull<Header>,
    opcode: u8,
) -> Result<usize, ExecError> {
    // SAFETY: raw 是存活对象。
    let ty = unsafe { raw.as_ref() }.ty();
    if ty == builtin_type(instance, "tuple") {
        // SAFETY: 类型身份已确认。
        return Ok(unsafe { &*raw.as_ptr().cast::<TupleObject>() }.len());
    }
    if ty == builtin_type(instance, "list") {
        // SAFETY: 同上。
        return Ok(unsafe { &*raw.as_ptr().cast::<ListObject>() }.len());
    }
    if instance.is_subtype(ty, builtin_type(instance, "dict")) {
        // SAFETY: 同上。
        return Ok(unsafe { &*raw.as_ptr().cast::<DictObject>() }.len());
    }
    if ty == builtin_type(instance, "set") || Some(ty) == instance.type_named("frozenset") {
        // **`frozenset` 与 `set` 同一份载荷**（第 292 轮）：`UNPACK_SEQUENCE` 一族按元素个数
        // 走这条路 ✓ ⇒ 先前只认 `set` ✗ ⇒ `'frozenset' object is not iterable` 之后又撞一条 ✗。
        // SAFETY: 同上。
        return Ok(unsafe { &*raw.as_ptr().cast::<SetObject>() }.len());
    }
    if ty == instance.singletons().str_type() {
        // SAFETY: 同上。
        return Ok(unsafe { &*raw.as_ptr().cast::<StrObject>() }.value().chars().count());
    }
    if Some(ty) == instance.type_named("bytes") {
        // SAFETY: 同上。
        return Ok(unsafe { &*raw.as_ptr().cast::<BytesObject>() }.value().len());
    }
    if Some(ty) == instance.type_named("bytearray") {
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

    if ty == builtin_type(instance, "tuple") {
        // SAFETY: 类型身份已确认。
        let value = unsafe { &*raw.as_ptr().cast::<TupleObject>() }.item(index);
        return value.map(owned).ok_or(ExecError::Unsupported {
            opcode,
            what: "迭代器游标越界",
        });
    }
    if ty == builtin_type(instance, "list") {
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
    if ty == builtin_type(instance, "set") || Some(ty) == instance.type_named("frozenset") {
        // **`frozenset` 与 `set` 同一份载荷**（第 292 轮）：按游标取元素这条路也要认它 ✓
        //（`_collections_abc` 注册基类时迭代集合 ✓，元类路径打通后当场踩到 ✓）。
        // SAFETY: 同上。
        let value = unsafe { &*raw.as_ptr().cast::<SetObject>() }.item(index);
        return value.map(owned).ok_or(ExecError::Unsupported {
            opcode,
            what: "迭代器游标越界",
        });
    }
    if ty == instance.singletons().str_type() {
        // SAFETY: 类型身份已确认。
        let text = unsafe { &*raw.as_ptr().cast::<StrObject>() }.value().to_owned();
        let character = text.chars().nth(index).ok_or(ExecError::Unsupported {
            opcode,
            what: "迭代器游标越界",
        })?;
        let object = instance.alloc(StrObject::new(
            instance.singletons().str_type(),
            character.to_string(),
        ));
        return Ok(object.into_raw().cast::<Header>());
    }
    // `bytes`：迭代给**整数**（实测 `list(b'ab') == [97, 98]`）
    if Some(ty) == instance.type_named("bytes") {
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
    if Some(ty) == instance.type_named("bytearray") {
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
        // SAFETY: 类型身份已确认。
        let text = unsafe { &*container.as_ptr().cast::<StrObject>() }.value();
        let needle = instance.text_value(item);
        return match needle {
            Some(needle) => Ok(text.contains(&needle)),
            None => {
                // SAFETY: item 是存活对象。
                let item_type = unsafe { item.as_ref() }.ty();
                // SAFETY: 类型名由注册表持有。
                let name = unsafe { item_type.as_ref() }.name();
                Err(raise_builtin(
                    instance,
                    "TypeError",
                    &format!("'in <string>' requires string as left operand, not {name}"),
                ))
            }
        };
    }
    // `bytes`：**子串**查找（实测 `b'ab' in b'abc'`）；左操作数不是 bytes 时报实测的消息。
    // **整数那一档**（第 284 轮按参照实测补 ✓）：`98 in b"b"` ⇒ `True` ✓、
    // `300 in b"ab"`／`(-1) in b"ab"` ⇒ `ValueError: byte must be in range(0, 256)` ✓、
    // `"a" in b"ab"` ⇒ `TypeError: a bytes-like object is required, not 'str'` ✓
    //（`Lib/` 里 `codecs`／`base64_codec` 一族真的会 `b in bytes` 判字节 ✓）。
    if Some(container_type) == instance.type_named("bytes") {
        let value = instance.bytes_value(container).unwrap_or_default().to_vec();
        if Some(instance.type_of(item)) == instance.type_named("int") {
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
        // SAFETY: 类型身份已确认。
        let object = unsafe { &*container.as_ptr().cast::<ListObject>() };
        for index in 0..object.len() {
            let element = object.item(index).expect("下标在范围内");
            if values_equal(instance, element, item) {
                return Ok(true);
            }
        }
        return Ok(false);
    }
    if instance.is_subtype(container_type, builtin_type(instance, "tuple")) {
        // SAFETY: 同上。
        let object = unsafe { &*container.as_ptr().cast::<TupleObject>() };
        for index in 0..object.len() {
            let element = object.item(index).expect("下标在范围内");
            if values_equal(instance, element, item) {
                return Ok(true);
            }
        }
        return Ok(false);
    }
    if instance.is_subtype(container_type, builtin_type(instance, "dict")) {
        // SAFETY: 同上。
        let object = unsafe { &*container.as_ptr().cast::<DictObject>() };
        for (key, _) in object.entries() {
            if values_equal(instance, key, item) {
                return Ok(true);
            }
        }
        return Ok(false);
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
        // SAFETY: 类型身份已确认。
        let object = unsafe { &*container.as_ptr().cast::<SetObject>() };
        for element in object.items() {
            if values_equal(instance, element, item) {
                return Ok(true);
            }
        }
        return Ok(false);
    }
    // **协议回退**（第 514 轮真 bug 修 ✗）：类型自带 `__contains__` ⇒ 调它 ✓
    // （参照口径：`in` **先**走 `__contains__` ✓、再走 `__iter__` ✓）。
    // 实测：`'X' in os.environ`（`os._Environ`，`collections.abc.MutableMapping` 的子类 ✓）先前直接报
    // `argument of type '_Environ' is not a container or iterable` ✗ ⇒ 这一条压着
    // `xml.sax`（5 个 ✓）＋ `xml.dom.pulldom` ✓（`tools/next_work.py` 的队列 ✓）。
    if let Some(found) = instance.type_lookup(container_type, "__contains__") {
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
    let left_text = instance.text_value(left);
    let right_text = instance.text_value(right);
    if let (Some(a), Some(b)) = (left_text, right_text) {
        return Ok(instance.new_str(&format!("{a}{b}")));
    }
    // **`bytearray` 拼接** ✓（第 643 轮）：任一操作数是 `bytearray` ⇒ 结果给 **`bytearray`** ✓（参照口径 ✓）；
    // 另一侧要是 `bytes`／`bytearray` ✓。`re._compiler` 的 `data += chunk` 正需要它 ✓
    //（先前报 `unsupported operand type(s) for +: 'bytearray' and 'bytes'` ✗）。
    let is_byte_like = |object: NonNull<Header>| {
        matches!(
            instance.type_name(instance.type_of(object)).as_str(),
            "bytes" | "bytearray"
        )
    };
    if (instance.type_name(left_type) == "bytearray"
        || instance.type_name(right_type) == "bytearray")
        && is_byte_like(left)
        && is_byte_like(right)
    {
        let mut joined: Vec<u8> = Vec::new();
        for side in [left, right] {
            if instance.type_name(instance.type_of(side)) == "bytearray" {
                // SAFETY: 类型身份已确认，载荷就是 `BytearrayObject`。
                let data = unsafe { &*side.as_ptr().cast::<crate::builtin_objects::BytearrayObject>() };
                joined.extend(data.value().iter().copied());
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
    let (left_bytes, right_bytes) = (instance.bytes_value(left), instance.bytes_value(right));
    if let (Some(a), Some(b)) = (left_bytes, right_bytes) {
        let mut joined = Vec::with_capacity(a.len() + b.len());
        joined.extend_from_slice(a);
        joined.extend_from_slice(b);
        return Ok(instance.new_bytes(&joined));
    }
    let list_type = instance.type_named("list");
    if Some(left_type) == list_type && Some(right_type) == list_type {
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
    let tuple_type = instance.type_named("tuple");
    if Some(left_type) == tuple_type && Some(right_type) == tuple_type {
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
