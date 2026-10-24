//! `emitter` 的 `expr_literals` 域：字面量与容器显示（含推导式以外的常量形态）。

use super::*;

impl Emitter {
    pub(crate) fn emit_expression_literals(&mut self, expression: &Expression) -> Result<(), CompileError> {
        match expression {
                Expression::Float(bits, span) => {
                    let index = self.intern_constant(Constant::Float(*bits));
                    self.emit_indexed(*span, "LOAD_CONST", index);
                    Ok(())
                }
                // **虚部字面量**（task-29 ✓）：`1j` ⇒ `LOAD_CONST 1j`（常量是 `complex(0,1)` ✓；
                // 实部按参照补 `0.0` ✓）。
                Expression::Imaginary(bits, span) => {
                    let index = self.intern_constant(Constant::Complex {
                        real_bits: 0f64.to_bits(),
                        imag_bits: *bits,
                    });
                    self.emit_indexed(*span, "LOAD_CONST", index);
                    Ok(())
                }
                Expression::Map(items, span) => {
                    // **纯键值对**（没有 `**`）：形状照第 282 轮**一字不动** ✓（夹具守着 ✓）——
                    // 15 对及以下一条 `BUILD_MAP n` ✓、16 对及以上走增量形态
                    //（`BUILD_MAP 0` ＋ 逐对 `MAP_ADD 1` ✓，阈值逐条 `dis` 实测 ✓）。
                    if items.iter().all(|item| matches!(item, MapItem::Pair(_, _))) {
                        if items.len() >= 16 {
                            self.emit_named(*span, "BUILD_MAP", 0);
                            for item in items {
                                let MapItem::Pair(key, value) = item else {
                                    unreachable!("上面刚判过全是键值对")
                                };
                                self.emit_expression(key)?;
                                self.emit_expression(value)?;
                                self.emit_named(*span, "MAP_ADD", 1);
                            }
                            return Ok(());
                        }
                        for item in items {
                            let MapItem::Pair(key, value) = item else {
                                unreachable!("上面刚判过全是键值对")
                            };
                            self.emit_expression(key)?;
                            self.emit_expression(value)?;
                        }
                        self.emit_indexed(*span, "BUILD_MAP", items.len());
                        return Ok(());
                    }
                    // **有 `**` 解包**（第 293 轮）：统一用"累加器"形态 —— `BUILD_MAP 0` 起头 ✓，
                    // 键值对走 `MAP_ADD 1` ✓、解包走 `LOAD <映射>; DICT_UPDATE 1` ✓
                    //（`DICT_UPDATE` 把 TOS 并进 TOS1 再弹掉 TOS ✓ ⇒ 累加器一直在栈上 ✓）。
                    // **如实说** ✗：参照这一档是"每串键值对各发一条 `BUILD_MAP n`"，与这条形状**不逐字节同形** ✓
                    //（语义相同 ✓；夹具里没有 `**` 的用例 ✓ ⇒ 不影响逐字节对拍 ✓）。
                    self.emit_named(*span, "BUILD_MAP", 0);
                    for item in items {
                        match item {
                            MapItem::Pair(key, value) => {
                                self.emit_expression(key)?;
                                self.emit_expression(value)?;
                                self.emit_named(*span, "MAP_ADD", 1);
                            }
                            MapItem::Unpack(value) => {
                                self.emit_expression(value)?;
                                self.emit_named(*span, "DICT_UPDATE", 1);
                            }
                        }
                    }
                    Ok(())
                }
                Expression::TupleLiteral(items, span) => {
                    if items
                        .iter()
                        .any(|item| matches!(item, Expression::Starred(_, _)))
                    {
                        // **星号解包**（第 120 轮，实测形态）：前面的非星号项先压 ⇒
                        // `BUILD_LIST <前项数>`（位点＝**整个显示**）⇒ 每个星号项：表达式 ＋
                        // `LIST_EXTEND 1`（位点同上）；元组末尾再 `CALL_INTRINSIC_1 6`。
                        // **交错形态也接** ✓（第 224 轮）：星号后面还有项 ⇒ 参照发的是 `LIST_APPEND 1` ✓
                        //（实测 `(*a, b)` ⇒ `BUILD_LIST 0; <a>; LIST_EXTEND 1; <b>; LIST_APPEND 1` ✓；
                        //  `[a, *b, c]` ⇒ `BUILD_LIST 1; <b>; LIST_EXTEND 1; <c>; LIST_APPEND 1` ✓）。
                        let leading = items
                            .iter()
                            .take_while(|item| !matches!(item, Expression::Starred(_, _)))
                            .count();
                        // 前导（非星号）项**先按各自位点压栈**（实测  的 
                        //   位点是  ＝  自身，不是整个显示）
                        for item in &items[..leading] {
                            self.emit_expression(item)?;
                        }
                        self.emit_indexed(*span, "BUILD_LIST", leading);
                        for item in &items[leading..] {
                            match item {
                                Expression::Starred(value, _) => {
                                    self.emit_expression(value)?;
                                    self.emit_named(*span, "LIST_EXTEND", 1);
                                }
                                // **星号后面还有项** ✓（第 224 轮）：参照发 `LIST_APPEND 1` ✓
                                //（`i` 是目标列表相对 TOS 的深度 ✓，这里恒为 1 ✓）。
                                _ => {
                                    self.emit_expression(item)?;
                                    self.emit_named(*span, "LIST_APPEND", 1);
                                }
                            }
                        }
                        // 元组末尾把列表转成元组（实测 `CALL_INTRINSIC_1 6`）；**必须在 return 之前** ✓
                        self.emit_named(*span, "CALL_INTRINSIC_1", 6);
                        return Ok(());
                    }

                    // 全常量 ⇒ 折叠成**常量元组**（实测 `x = (1, 2)` 的 `co_consts` 里有它，
                    // 且登记在收尾（`LOAD_CONST None`）**之后** ⇒ 走 `pending` 那条延迟路径）
                    if let Some(folded) = fold_constant(expression)? {
                        if let Some(leaf) = leftmost_literal(expression) {
                            self.intern_literal(leaf);
                        }
                        self.emit_constant(*span, folded);
                        return Ok(());
                    }
                    // **超指令融合**（第 139 轮实测）：前两项都是**裸局部名字**时，参照把它们打成
                    //   `LOAD_FAST_BORROW_LOAD_FAST_BORROW <高4位先压 | 低4位后压>` ✓
                    //   （`yield a, b` ⇒ 一条融合 ＋ `BUILD_TUPLE 2` ✓）。只认 `Expression::Name`
                    //   ⇒ 不误融合（`a.b, c` 的首条是 `LOAD_FAST_BORROW a` ＋ `LOAD_ATTR` ✗ ✓）。
                    let mut start = 0;
                    if items.len() >= 2 {
                        let slot_of = |item: &Expression| match item {
                            Expression::Name(name, _) => self.plain_local_slot(name),
                            _ => None,
                        };
                        // **前提：两个槽号都要装得进 4 位** ✓（第 673 轮真 bug 修 ✗）：与 `emit_two_operands`
                        // 同一条规矩 ✓ —— 融合把两个槽号各塞半个字节 ✓ ⇒ 槽号 ≥16 时 `(first << 4) | second`
                        // **先溢出成 >255、再 `as u8` 截断** ✗ ⇒ 读到的两个槽全错 ✓
                        //（实测 `_parser.py:704` 的 `(min, max, item)`：槽 25 与 9 ⇒ `409 as u8` = **153** = `(9,9)` ✗
                        // ⇒ 元组前两项变成同一个对象 ⇒ 运行期 `int * <内建 len>` ✗✓）。
                        if let (Some(first), Some(second)) = (slot_of(&items[0]), slot_of(&items[1])) {
                            if first > 0x0F || second > 0x0F {
                                // 装不下 ⇒ 走下面的**非融合回退** ✓（逐个 `emit_expression` ✓）
                                for item in &items[..] {
                                    self.emit_expression(item)?;
                                }
                                let count = u8::try_from(items.len()).map_err(|_| {
                                    CompileError::Unsupported("元组字面量超过 255 项尚未接线".to_owned())
                                })?;
                                self.emit_named(*span, "BUILD_TUPLE", count);
                                return Ok(());
                            }
                            self.emit_at(
                                items[0].span(),
                                opcode::opcode("LOAD_FAST_BORROW_LOAD_FAST_BORROW")
                                    .expect("超指令在表里"),
                                ((first << 4) | second) as u8,
                            );
                            start = 2;
                        }
                    }
                    for item in &items[start..] {
                        self.emit_expression(item)?;
                    }
                    let count = u8::try_from(items.len()).map_err(|_| {
                        CompileError::Unsupported("元组字面量超过 255 项尚未接线".to_owned())
                    })?;
                    self.emit_named(*span, "BUILD_TUPLE", count);
                    Ok(())
                }
                // 切片字面量**只能**当下标用（`a[b:c]`）；单独出现是内部错误，别静默发错指令
                // **带上位点**（第 311 轮）：先前只有一句"只能出现在下标里" ⇒ 定不了是哪一行
                // （上限榜上那一族 35 个模块就是被它挡着）⇒ 现在给行／列，便于顺着修。
                Expression::SliceLiteral { span, .. } => Err(CompileError::Unsupported(format!(
                    "切片字面量只能出现在下标里（`a[b:c]`；第 {} 行，列 {}-{}）",
                    span.line_start, span.col_start, span.col_end
                ))),
                Expression::List(items, span) => {
                    if items
                        .iter()
                        .any(|item| matches!(item, Expression::Starred(_, _)))
                    {
                        // **星号解包**（第 120 轮，实测形态）：前面的非星号项先压 ⇒
                        // `BUILD_LIST <前项数>`（位点＝**整个显示**）⇒ 每个星号项：表达式 ＋
                        // `LIST_EXTEND 1`（位点同上）；元组末尾再 `CALL_INTRINSIC_1 6`。
                        // **交错形态也接** ✓（第 224 轮）：星号后面还有项 ⇒ 参照发的是 `LIST_APPEND 1` ✓
                        //（实测 `(*a, b)` ⇒ `BUILD_LIST 0; <a>; LIST_EXTEND 1; <b>; LIST_APPEND 1` ✓；
                        //  `[a, *b, c]` ⇒ `BUILD_LIST 1; <b>; LIST_EXTEND 1; <c>; LIST_APPEND 1` ✓）。
                        let leading = items
                            .iter()
                            .take_while(|item| !matches!(item, Expression::Starred(_, _)))
                            .count();
                        // 前导（非星号）项**先按各自位点压栈**（实测  的 
                        //   位点是  ＝  自身，不是整个显示）
                        for item in &items[..leading] {
                            self.emit_expression(item)?;
                        }
                        self.emit_indexed(*span, "BUILD_LIST", leading);
                        for item in &items[leading..] {
                            match item {
                                Expression::Starred(value, _) => {
                                    self.emit_expression(value)?;
                                    self.emit_named(*span, "LIST_EXTEND", 1);
                                }
                                // **星号后面还有项** ✓（第 224 轮）：参照发 `LIST_APPEND 1` ✓
                                //（`i` 是目标列表相对 TOS 的深度 ✓，这里恒为 1 ✓）。
                                _ => {
                                    self.emit_expression(item)?;
                                    self.emit_named(*span, "LIST_APPEND", 1);
                                }
                            }
                        }
                        return Ok(());
                    }

                    for item in items {
                        self.emit_expression(item)?;
                    }
                    let count = u8::try_from(items.len()).map_err(|_| {
                        CompileError::Unsupported("列表字面量超过 255 项尚未接线".to_owned())
                    })?;
                    self.emit_named(*span, "BUILD_LIST", count);
                    Ok(())
                }
                Expression::Constant(constant, span) => {
                    let index = self.intern_constant(constant.clone());
                    self.emit_indexed(*span, "LOAD_CONST", index);
                    Ok(())
                }
                // **清单推导式**（3.12+ 内联；照实测骨架）：
                //   `可迭代; GET_ITER; LOAD_FAST_AND_CLEAR <目标槽>; SWAP 2; BUILD_LIST 0; SWAP 2;
                //    L1: FOR_ITER → L2; STORE_FAST_LOAD_FAST <槽,槽>; [if 条件…];
                //    <元素>; LIST_APPEND 2; JUMP_BACKWARD → L1; L2: END_FOR; POP_ITER;
                //    SWAP 2; STORE_FAST <槽>`（还原外层同名局部）
                //   **整段受异常表保护**，清理块 `SWAP 2; POP_TOP; SWAP 2; STORE_FAST <槽>; RERAISE 0`
                //   （异常表：起点＝`BUILD_LIST`、到还原前为止，目标＝清理块，`depth` 2、`lasti` 关闭）
                // **生成器表达式**（第 125 轮）：**不内联** —— 编成独立 code object ＋ 生成器协议 ✓。
                //   外层四步（实测）：`LOAD_CONST <code>; MAKE_FUNCTION; <可迭代>; GET_ITER; CALL 0` ✓，
                //   其中 `LOAD_CONST` 与 `CALL` 位点＝**表达式自身** ✓、`GET_ITER` 位点＝可迭代那段 ✓。
                Expression::SetLiteral(items, span) => {
                    if items
                        .iter()
                        .any(|item| matches!(item, Expression::Starred(_, _)))
                    {
                        // **星号解包**（第 120 轮，实测形态）：前面的非星号项先压 ⇒
                        // `BUILD_LIST <前项数>`（位点＝**整个显示**）⇒ 每个星号项：表达式 ＋
                        // `SET_UPDATE 1`（位点同上）；元组末尾再 `CALL_INTRINSIC_1 6`。
                        // **交错形态也接** ✓（第 224 轮）：星号后面还有项 ⇒ 参照发的是 `LIST_APPEND 1` ✓
                        //（实测 `(*a, b)` ⇒ `BUILD_LIST 0; <a>; LIST_EXTEND 1; <b>; LIST_APPEND 1` ✓；
                        //  `[a, *b, c]` ⇒ `BUILD_LIST 1; <b>; LIST_EXTEND 1; <c>; LIST_APPEND 1` ✓）。
                        let leading = items
                            .iter()
                            .take_while(|item| !matches!(item, Expression::Starred(_, _)))
                            .count();
                        for item in &items[..leading] {
                            self.emit_expression(item)?;
                        }
                        self.emit_indexed(*span, "BUILD_SET", leading);
                        for item in &items[leading..] {
                            match item {
                                Expression::Starred(value, _) => {
                                    self.emit_expression(value)?;
                                    self.emit_named(*span, "SET_UPDATE", 1);
                                }
                                // **星号后面还有项** ✓（task-42 修 ✗）：**集合**里参照发的是
                                // `SET_ADD 1` ✓（实测 `{0, *a, 3}` ⇒ `BUILD_SET 1; <a>;
                                // SET_UPDATE 1; <3>; SET_ADD 1` ✓）—— 先前照 list 那套发
                                // `LIST_APPEND` ✗ ⇒ 运行期"容器位置／类型不符" ✗。
                                _ => {
                                    self.emit_expression(item)?;
                                    self.emit_named(*span, "SET_ADD", 1);
                                }
                            }
                        }
                        return Ok(());
                    }

                    // **≥3 个元素且全常量** ⇒ 参照折叠成 `frozenset` 常量（实测 `{1, 2, 3}` ⇒
                    // `BUILD_SET 0; LOAD_CONST frozenset({1, 2, 3}); SET_UPDATE 1`；`{1, 1, 2}` 也折、
                    // 去重后是 `frozenset({1, 2})`）；`{1}`／`{1, 2}`／含非常量 ⇒ 照旧逐元素 `BUILD_SET n`。
                    if items.len() >= 3 {
                        let mut folded: Vec<Constant> = Vec::with_capacity(items.len());
                        let mut all_constant = true;
                        for item in items {
                            match fold_constant(item)? {
                                Some(constant) => folded.push(constant),
                                None => {
                                    all_constant = false;
                                    break;
                                }
                            }
                        }
                        if all_constant {
                            // **最左叶子**照样进常量表（实测常量表是 `(1, None, frozenset(…))`）
                            if let Some(first) = folded.first() {
                                self.intern_literal(first.clone());
                            }
                            // 集合语义：去重（`frozenset` 的元素序不进观测面——渲染时排序）
                            let mut unique: Vec<Constant> = Vec::new();
                            for constant in folded {
                                if !unique.contains(&constant) {
                                    unique.push(constant);
                                }
                            }
                            self.emit_at(
                                *span,
                                opcode::opcode("BUILD_SET").expect("BUILD_SET 在表里"),
                                0,
                            );
                            // 折叠出来的 `frozenset` 与其它折叠常量**同一条路**：延迟到收尾之后入池
                            // （实测 `x = {1, 2, 3}` ⇒ `[1, None, frozenset]`；`x = 200 + 100` ⇒ `[200, None, 300]`）
                            let argument_byte = self.unit.code.len() + 1;
                            self.emit_at(
                                *span,
                                opcode::opcode("LOAD_CONST").expect("LOAD_CONST 在表里"),
                                0,
                            );
                            self.pending
                                .push((argument_byte, Constant::FrozenSet(unique)));
                            self.emit_at(
                                *span,
                                opcode::opcode("SET_UPDATE").expect("SET_UPDATE 在表里"),
                                1,
                            );
                            return Ok(());
                        }
                    }
                    for item in items {
                        self.emit_expression(item)?;
                    }
                    self.emit_indexed(*span, "BUILD_SET", items.len());
                    Ok(())
                }
                // **f-string**（3.14 实测）：逐段求值——字面段 `LOAD_CONST`、插值段"表达式 ＋
                // `CONVERT_VALUE`（有转换时）＋ `FORMAT_SIMPLE`／`FORMAT_WITH_SPEC`"；**多于一段**
                // 再 `BUILD_STRING n`（纯字面量在解析时已降成 `Str`）
                Expression::BigInt(text, span) => {
                    let index = self.intern_constant(Constant::BigInt(text.clone()));
                    self.emit_indexed(*span, "LOAD_CONST", index);
                    Ok(())
                }

                Expression::Int(value, span) => {
                    if (0..=255).contains(value) {
                        // **延迟分支**（实测）：需要收尾机制的块体里的 `return <小整数>` **不入常量表**
                        // （`with a: return 1` ⇒ `co_consts` 只有 `none`；不带 `with` 的 `return 1` ⇒ 入池）
                        if !self.defer_return_literal {
                            self.intern_literal(Constant::Int(*value));
                        }
                        self.emit_at(
                            *span,
                            opcode::opcode("LOAD_SMALL_INT").expect("LOAD_SMALL_INT 在表里"),
                            *value as u8,
                        );
                    } else {
                        let index = self.intern_constant(Constant::Int(*value));
                        self.emit_indexed(*span, "LOAD_CONST", index);
                    }
                    Ok(())
                }
                Expression::Str(text, span) => {
                    let index = self.intern_constant(Constant::Str(text.clone()));
                    self.emit_indexed(*span, "LOAD_CONST", index);
                    Ok(())
                }
                Expression::Bytes(value, span) => {
                    // `bytes` 字面量与字符串同形：一条 `LOAD_CONST`（实例化时常量池里那项建 `BytesObject`）
                    let index = self.intern_constant(Constant::Bytes(value.clone()));
                    self.emit_indexed(*span, "LOAD_CONST", index);
                    Ok(())
                }
            _ => unreachable!("emit_expression_literals 只接收 expr_literals 族的 Expression"),
        }
    }
}
