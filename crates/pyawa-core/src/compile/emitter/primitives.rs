//! `emitter` 的 `primitives` 域：发射原语（指令／常量／名字／槽位／标签）——所有域共用的底层。

use super::*;

impl Emitter {
    /// 发射一条指令并记下它的位置（`BC-18`）。
    /// 新开一个标签；返回它的编号。
    /// 发一个**条件跳转**（落在新标签上；调用方拿标签去 `mark_label`）。
    pub(crate) fn emit_condition_jump(
        &mut self,
        condition: &Expression,
        jump_if_true: bool,
    ) -> Result<usize, CompileError> {
        let target = self.new_label();
        self.emit_condition_jump_to(condition, jump_if_true, target)?;
        Ok(target)
    }

    #[track_caller]
    pub(crate) fn new_label(&mut self) -> usize {
        self.labels.push(None);
        self.label_origins.push(core::panic::Location::caller());
        self.labels.len() - 1
    }

    /// 按**名字**发一条指令（名字一定在表里；内部用）。
    pub(crate) fn emit_named(&mut self, position: Span, name: &str, oparg: u8) {
        self.emit_at(
            position,
            opcode::opcode(name).expect("指令在表里"),
            oparg,
        );
    }

    /// **存一个名字的统一入口**（第 114 轮）：`global` 声明的 ⇒ `STORE_GLOBAL` ✓；
    /// 函数局部 ⇒ `STORE_FAST` ✓；其余 ⇒ `STORE_NAME` ✓（实测：`global a, b` 后 `a = 1`
    /// 在模块层与函数层都走 `STORE_GLOBAL` ✓）。
    pub(crate) fn emit_store_name(&mut self, span: Span, name: &str) {
        if self.global_names.iter().any(|item| item == name) {
            let index = self.intern_name(name);
            self.emit_indexed(span, "STORE_GLOBAL", index);
        } else if let Some(slot) = self.deref_slot(name) {
            // **cell／自由变量要走 `STORE_DEREF`**（第 343 轮真 bug 修 ✗）：先前这一支**漏了**
            // ✗ ⇒ 名字是 cell（被内层函数闭包捕获 ✓）时落到最后的 `STORE_NAME` ✗ ⇒ 在**函数**帧里
            // 撞执行器的"`STORE_NAME` 需要命名空间帧（模块／类体）" ✗。
            // 实测原形 ✓：`Lib/collections/__init__.py` 的 `namedtuple` 里
            //   `_dict, _tuple, _len, _map, _zip = dict, tuple, len, map, zip`（第 437 行 ✓）——
            // 这五个名字都被它**内层那几个方法**捕获 ✓ ⇒ 是 cell ✓ ⇒ 解包赋值直接中止 ✗
            // （上限榜上 78 个模块压在它上面 ✓）。把名字换掉（不再是 cell ✓）就正常 ✓，实测过 ✓。
            self.emit_indexed(span, "STORE_DEREF", slot);
        } else if self.kind == ScopeKind::Function
            && self.unit.varnames.iter().any(|item| item == name)
        {
            let slot = self.slot_of(name);
            self.emit_indexed(span, "STORE_FAST", slot);
        } else {
            let index = self.intern_name(name);
            self.emit_indexed(span, "STORE_NAME", index);
        }
    }

    /// **删一个名字的统一入口**（第 202 轮）：口径与 [`Self::emit_store_name`] **对称** ✓ ——
    /// `global` 声明的 ⇒ `DELETE_GLOBAL` ✓；函数局部 ⇒ `DELETE_FAST <槽>` ✓；其余 ⇒ `DELETE_NAME` ✓。
    ///
    /// **为什么必需** ✗：`except … as 名字` 的**两处**（绑定那次 ✓ 与收尾那次 ✓）先前**写死**了
    /// `STORE_NAME`／`DELETE_NAME` ✗ ⇒ 一旦出现在**函数**里 ⇒ 撞执行器的
    /// "`STORE_NAME` 需要命名空间帧（模块／类体）" ✗（实测：函数内 `try/except … as` 直接中止 ✓）。
    pub(crate) fn emit_delete_name(&mut self, span: Span, name: &str) {
        if self.global_names.iter().any(|item| item == name) {
            let index = self.intern_name(name);
            self.emit_indexed(span, "DELETE_GLOBAL", index);
        } else if self.kind == ScopeKind::Function
            && self.unit.varnames.iter().any(|item| item == name)
        {
            let slot = self.slot_of(name);
            self.emit_indexed(span, "DELETE_FAST", slot);
        } else {
            let index = self.intern_name(name);
            self.emit_indexed(span, "DELETE_NAME", index);
        }
    }

    /// 发一条**行号有、列全空**的指令（第 124 轮）：生成器的 `RETURN_GENERATOR`／`POP_TOP`
    /// 实测位点是 `(def 行, def 行, None, None)` ✓，`emit_named` 表达不了"列空" ⇒ 单列一条 ✓。
    pub(crate) fn emit_line_only(&mut self, line: u32, name: &str, oparg: u8) {
        self.unit
            .positions
            .push((Some(line), Some(line), None, None));
        let opcode = opcode::opcode(name).expect("指令在表里");
        self.unit.code.push(opcode as u8);
        self.unit.code.push(oparg);
        for _ in 0..opcode::inline_cache_entries(opcode) {
            self.unit.code.push(0);
            self.unit.code.push(0);
        }
    }

    /// **带下标的指令**（第 122 轮）：下标 > 255 时先发一条 `EXTENDED_ARG <高位>` ✓，
    /// **位点与随后那条完全相同**（实测 `STORE_NAME 301`／`LOAD_CONST 300` 都是这样 ✓）。
    ///
    /// 只用于"实参就是下标本身"的指令 ✓（`LOAD_GLOBAL`／`LOAD_ATTR` 那种带标志位的另行处理 ✗）。
    pub(crate) fn emit_indexed(&mut self, span: Span, name: &str, index: usize) {
        let opcode = opcode::opcode(name).expect("指令在表里");
        if index > 255 {
            self.emit_at(
                span,
                opcode::opcode("EXTENDED_ARG").expect("EXTENDED_ARG 在表里"),
                (index >> 8) as u8,
            );
        }
        self.emit_at(span, opcode, (index & 0xFF) as u8);
    }

    /// **带标志位／移位的名字下标**（`LOAD_ATTR`／`LOAD_GLOBAL` 的 `(下标 << 1) | 标志` ✓）：
    /// `oparg > 255` 时按高位到低位先发 `EXTENDED_ARG` ✓ —— 与 [`Self::emit_indexed`] 同口径 ✓。
    ///
    /// **第 725 轮真 bug 修** ✗：先前这几处一律 `as u8` **截断** ⇒ 名字表超过 255 项后
    /// `LOAD_ATTR`／`LOAD_GLOBAL` 会取到**别的名字** ✗（实测 `Lib/zipfile/__init__.py` 有 557 个
    /// 不同名字 ⇒ `class _ZipWriteFile(io.BufferedIOBase)` 报
    /// `'module' object has no attribute 'os'`／`stat`／`shutil` ✗ —— 名字跟着探针位置变 ✓）。
    pub(crate) fn emit_shifted(&mut self, span: Span, name: &str, oparg: usize) {
        let opcode = opcode::opcode(name).expect("指令在表里");
        let extended = opcode::opcode("EXTENDED_ARG").expect("EXTENDED_ARG 在表里");
        let mut high_bytes: Vec<u8> = Vec::new();
        let mut high = oparg >> 8;
        while high > 0 {
            high_bytes.push((high & 0xFF) as u8);
            high >>= 8;
        }
        for byte in high_bytes.iter().rev() {
            self.emit_at(span, extended, *byte);
        }
        self.emit_at(span, opcode, (oparg & 0xFF) as u8);
    }

    /// 同 [`Self::emit_indexed`]，但记**无位点**（合成指令那条路 ✓）。
    pub(crate) fn emit_indexed_none(&mut self, name: &str, index: usize) {
        let opcode = opcode::opcode(name).expect("指令在表里");
        if index > 255 {
            self.emit_none(
                opcode::opcode("EXTENDED_ARG").expect("EXTENDED_ARG 在表里"),
                (index >> 8) as u8,
            );
        }
        self.emit_none(opcode, (index & 0xFF) as u8);
    }

    /// 同 `emit_named`，但记**无位点**（`BC-4` 扩：参照给合成指令的是全 `None`）。
    pub(crate) fn emit_named_none(&mut self, name: &str, oparg: u8) {
        self.emit_none(opcode::opcode(name).expect("指令在表里"), oparg);
    }

    /// 记下标签落在**当前**码元处。
    pub(crate) fn mark_label(&mut self, label: usize) {
        self.labels[label] = Some(self.unit.code.len() / 2);
    }

    /// 发一条**前向跳转**（目标标签先占位、收尾时回填）。
    pub(crate) fn emit_jump(&mut self, position: Span, opcode: u16, label: usize) {
        if crate::diag::flag("PYAWA_JUMP_DEBUG") {
            eprintln!("[jump] emit_jump 目标标签={label} code_len={}", self.unit.code.len());
        }
        self.emit_directed_jump(position, opcode, label, false);
    }

    /// 发一条跳转；`backward` 为真时 oparg 是**往回**的距离
    /// （实测 `JUMP_BACKWARD` 的 oparg ＝ `当前码元 + 占用码元数 − 目标码元`，方向是 opcode 本身定的）。
    pub(crate) fn emit_directed_jump(&mut self, position: Span, opcode: u16, label: usize, backward: bool) {
        if crate::diag::flag("PYAWA_JUMP_DEBUG") {
            eprintln!("[jump] emit_directed_jump 目标标签={label} 向后={backward} code_len={}", self.unit.code.len());
        }
        let argument_byte = self.unit.code.len() + 1;
        let size = 1 + opcode::inline_cache_entries(opcode) as usize;
        self.emit_at(position, opcode, 0);
        self.jumps.push((argument_byte, label, size | (usize::from(backward) << 16)));
    }

    /// 发射一条指令并记位点：`position = None` ⇒ 四元组**全 `None`**（`BC-4` 扩的合成指令）。
    pub(crate) fn emit_core(&mut self, position: Option<Span>, opcode: u16, oparg: u8) {
        self.unit.positions.push(match position {
            Some(span) => (
                Some(span.line_start),
                Some(span.line_end),
                Some(span.col_start),
                Some(span.col_end),
            ),
            None => (None, None, None, None),
        });
        if let Some(span) = position {
            self.last_span = span;
        }
        self.unit.code.push(opcode as u8);
        self.unit.code.push(oparg);
        // `BC-35`／`BC-36`：带缓存的指令后必须留等宽**零填充**码元
        for _ in 0..opcode::inline_cache_entries(opcode) {
            self.unit.code.push(0);
            self.unit.code.push(0);
        }
    }

    /// 发射一条**带位点**的指令（绝大多数情况）。
    pub(crate) fn emit_at(&mut self, position: Span, opcode: u16, oparg: u8) {
        // **op87 的"码元 / arg / 位点"** ✓（第 661 轮门控 `PYAWA_EMIT87_DEBUG=1`）：与运行期的
        // **指令偏移**同一把坐标 ✓ ⇒ 两边一对就能钉死"这条融合是谁发的" ✓（前几轮在发射端猜了四处都没中 ✗）。
        if crate::diag::flag("PYAWA_EMIT87_DEBUG") && opcode == 87 {
            if opcode == 87 && self.unit.code.len() / 2 == 2561 {
                eprintln!("[emit87-bt] 命中 2561：\n{}", std::backtrace::Backtrace::force_capture());
            }
            eprintln!(
                "[emit87] name={} 码元={} arg={} 高={} 低={} 位点={:?}",
                self.unit.name,
                self.unit.code.len() / 2,
                oparg,
                oparg >> 4,
                oparg & 0x0F,
                position
            );
            if self.unit.name == "_parse" {
                eprintln!("{}", std::backtrace::Backtrace::force_capture());
            }
        }
        self.emit_core(Some(position), opcode, oparg);
    }

    /// 发射一条**没有位点**的合成指令（`BC-4` 扩：四元组全 `None`）。
    pub(crate) fn emit_none(&mut self, opcode: u16, oparg: u8) {
        self.emit_core(None, opcode, oparg);
    }

    /// **`LOAD_CONST` 一个常量**（task-20 第 2 轮真 bug 修 ✗）：已入池就**直发** ✓；没入池时，为了
    /// **常量表顺序照参照**而走 [`Self::pending`] 延迟回填 —— 但**下标一旦可能 ≥ 256** 就必须**当场入池** ✗
    /// （`flush_pending` 只能写 `index as u8` ✗ ⇒ **高字节丢** ✓：实测 `Lib/inspect.py:1616` 的
    /// `('frame',) + Traceback._fields` 取到的是**别的常量** ✓ —— 常量表 260 项、`('frame',)` 真下标 301
    /// ⇒ 低字节 45 ⇒ 读到 `(45,)` ✗）。
    pub(crate) fn emit_constant(&mut self, span: Span, constant: Constant) {
        if let Some(index) = self.unit.constants.iter().position(|item| *item == constant) {
            self.emit_indexed(span, "LOAD_CONST", index);
            return;
        }
        // 延迟入池后，这条的下标上界＝现有项数 ＋ 待回填项数 ＋ 自己 ✓ —— 但**后面还会入池更多常量** ✗
        // （下标在 `flush_pending` 才定 ✓）⇒ 只留 **64 项余量**：上界 > 191 就**当场入池** ✓
        // （小模块仍走延迟 ⇒ 常量表顺序照参照 ✓、夹具逐字节不动 ✓；大模块（`inspect` 常量 260 ✓）
        // 当场入池 ⇒ 下标不会溢出 ✓）。
        const DEFER_LIMIT: usize = 191;
        let bound = self.unit.constants.len() + self.pending.len() + 1;
        if bound > DEFER_LIMIT {
            let index = self.intern_constant(constant);
            self.emit_indexed(span, "LOAD_CONST", index);
            return;
        }
        let argument_byte = self.unit.code.len() + 1;
        self.emit_at(
            span,
            opcode::opcode("LOAD_CONST").expect("LOAD_CONST 在表里"),
            0,
        );
        self.pending.push((argument_byte, constant));
    }

    pub(crate) fn intern_constant(&mut self, constant: Constant) -> usize {
        if let Some(index) = self.unit.constants.iter().position(|item| *item == constant) {
            return index;
        }
        self.unit.constants.push(constant);
        self.unit.constants.len() - 1
    }

    /// 登记一个字面量，规矩照实测：小整数**只在常量表还是空的时候**才登记；大整数与字符串总是登记。
    pub(crate) fn intern_literal(&mut self, constant: Constant) {
        if let Constant::Int(value) = constant {
            if (0..=255).contains(&value) && !self.unit.constants.is_empty() {
                return;
            }
        }
        self.intern_constant(constant);
    }

    pub(crate) fn intern_name(&mut self, name: &str) -> usize {
        if let Some(index) = self.unit.names.iter().position(|item| item == name) {
            return index;
        }
        self.unit.names.push(name.to_owned());
        self.unit.names.len() - 1
    }

    /// 局部槽位（没有就按首次出现顺序追加——形参已经在前面）。
    /// **本作用域里这个名是不是 cell／free**（第 292 轮）：是 ⇒ 返回 `LOAD_DEREF`／`STORE_DEREF`
    /// 的 localsplus 索引（cell 排在 `varnames` 之后；free 就在自己的自由变量表里）。
    pub(crate) fn deref_slot(&self, name: &str) -> Option<usize> {
        if self.unit.cellvars.iter().any(|item| item == name) {
            return Some(self.cell_slot(name).expect("刚查过在 cellvars 里"));
        }
        self.unit.freevars.iter().position(|item| item == name).map(|free| {
            // localsplus 布局：`varnames` ＋ `cellvars` ＋ `freevars`
            // **freevars 从"追加后的 cell"之后起** ✓（不是 `cellvars.len()` ✗ —— 形参 cell 不占位 ✓）。
            self.unit.varnames.len() + self.appended_cells() + free
        })
    }

    /// **cell 在 localsplus 里的槽**（实测两种）：**形参** cell 用它自己的 `varnames` 槽
    /// （`def outer(x): …` ⇒ `MAKE_CELL 0`、元组元素 `LOAD_FAST_BORROW 0`，同时 `varnames=('x','inner')`）；
    /// **局部** cell 排在 `varnames` **之后**（`def outer(): x = 1 …` ⇒ `varnames=('inner',)`、
    /// `MAKE_CELL 1`）。
    /// **`localsplus` 里"追加"的 cell 个数** ✓（第 210 轮真 bug 修复 ✗）：`cellvars` 中**已经是形参**
    /// 的那些**复用** `varnames` 槽 ✓、**不占**追加位 ⇒ 只有**非形参**的 cell 才追加 ✓
    /// （与 [`crate::CodeObject::localsplus_kinds`] **同一条规矩** ✓ —— 一处真相 ✓）。
    /// 先前这里直接用 `cellvars` 的**下标**当偏移 ✗ ⇒ 一个形参 cell 混在里面就**整体多算一格** ✗
    /// （实测 `Lib/types.py` 的 `coroutine`：`cellvars=[func(形参), co_flags, _collections_abc]` ⇒
    /// 第三个 cell 发成槽 6、而 localsplus 只有 6 ✗ ⇒ 差一 ✓）。
    pub(crate) fn appended_cells(&self) -> usize {
        self.unit
            .cellvars
            .iter()
            .filter(|name| !self.unit.varnames.contains(name))
            .count()
    }

    /// `cellvars` 里**第 `cell` 个**之前的"追加"cell 个数 ✓。
    pub(crate) fn appended_cells_before(&self, cell: usize) -> usize {
        self.unit
            .cellvars
            .iter()
            .take(cell)
            .filter(|name| !self.unit.varnames.contains(name))
            .count()
    }

    /// **能不能走"裸局部变量"快路**（`LOAD_FAST_BORROW`／`LOAD_FAST_BORROW_LOAD_FAST_BORROW`／
    /// `STORE_FAST_LOAD_FAST`／`STORE_FAST_STORE_FAST` 这些按 **localsplus 槽号**直接读写的超指令）：
    /// **cell（含形参 cell）与自由变量的槽里放的是 cell 对象** ✗ ⇒ 只能走 `LOAD_DEREF`／
    /// `STORE_DEREF`（第 406 轮真 bug 修 ✗：`def __init__(self, flags): self.flags = flags`
    /// 而 `flags` 被生成器表达式捕获成 cell 时，先前把**cell 对象本身**当值传给 `STORE_ATTR`
    /// ⇒ 运行期 `TypeError: <cell object …> must be a signal dict` ✓）。
    #[must_use]
    pub(crate) fn plain_local_slot(&self, name: &str) -> Option<usize> {
        if self.deref_slot(name).is_some() {
            return None;
        }
        self.unit.varnames.iter().position(|item| item == name)
    }

    pub(crate) fn cell_slot(&self, name: &str) -> Option<usize> {
        let cell = self.unit.cellvars.iter().position(|item| item == name)?;
        if let Some(local) = self.unit.varnames.iter().position(|item| item == name) {
            return Some(local); // 形参（`varnames` 前缀）——它的实参槽就是 cell 槽
        }
        Some(self.unit.varnames.len() + self.appended_cells_before(cell))
    }

    /// `varnames` 里属于**参数**的个数（前缀：仅位置 + 位置或关键字 + 关键字）。
    /// **形参槽数**（`varnames` 开头那一段 ✓）——**含 `*args`／`**kwargs`** ✓（task-22 第 3 轮真 bug 修 ✗）：
    /// 先前只算 位置 ＋ 仅关键字 ✗ ⇒ `def f(a, *args, **kwargs)` 里被**内层闭包捕获**的 `args`／`kwargs`
    /// 会被当成"局部 cell"从 `varnames` **剔掉** ✗ ⇒ 运行期 `bind_arguments` 按槽位绑定 ⇒ **越界**
    /// （实测 `import concurrent.futures.thread` 报**空消息**的 Rust panic ✓；最小复现
    /// `def f(a, *args, **kwargs): g = lambda: (args, kwargs)` ✓）。参照里形参 cell **留在
    /// `co_varnames`** 里 ✓（实测 `def f(a, *args, **kwargs)` ⇒ `varnames=('a','args','kwargs','g')`、
    /// `cellvars=('args','kwargs')` ✓）。
    pub(crate) fn parameter_count(&self) -> usize {
        (self.unit.posonlyargcount
            + self.unit.argcount
            + self.unit.kwonlyargcount
            + usize::from(self.unit.flags & 0x04 != 0)
            + usize::from(self.unit.flags & 0x08 != 0)) as usize
    }

    pub(crate) fn slot_of(&mut self, name: &str) -> usize {
        // **cell／自由变量优先**：它们都不进 `varnames`（`nonlocal` 声明的名字因此不会被
        // `collect_locals` 当成局部 ✓）
        if let Some(slot) = self.deref_slot(name) {
            return slot;
        }
        if let Some(index) = self.unit.varnames.iter().position(|item| item == name) {
            return index;
        }
        // **cell 名不进 `varnames`**（它在 localsplus 里排在 varnames 之后）⇒ 返回 cell 索引，
        // 免得 `collect_locals` 二次调用时把它又加回 `varnames` ✗（索引就乱了）
        if let Some(cell) = self.unit.cellvars.iter().position(|item| item == name) {
            // **与 `cell_slot` 同一条公式** ✓（第 261 轮：这里先前用 `cellvars` 的**下标** ✗ ⇒
            // 有**形参 cell**时会整体多算一格 ✗ ⇒ 与 `cell_slot` 的口径不一致 ✓）。
            return self.unit.varnames.len() + self.appended_cells_before(cell);
        }
        self.unit.varnames.push(name.to_owned());
        self.unit.nlocals = self.unit.varnames.len();
        self.unit.varnames.len() - 1
    }

    /// 本作用域里这个名字是不是**局部／cell／自由变量**（外壳的"外层闭包"判据 ✓）。
    pub(crate) fn local_or_cell_or_free(&self, name: &str) -> bool {
        self.unit.varnames.iter().any(|item| item == name)
            || self.unit.cellvars.iter().any(|item| item == name)
            || self.unit.freevars.iter().any(|item| item == name)
    }

    /// 压两个操作数：两边都是**本函数的局部**时打成超指令（位置取先压的那个名字，实测）。
    pub(crate) fn emit_two_operands(
        &mut self,
        left: &Expression,
        right: &Expression,
    ) -> Result<(), CompileError> {
        // **待抵消的融合读**：`STORE_FAST_LOAD_FAST` 已经把左操作数压回来了 ⇒ 不能再打成
        // "两个局部名的超指令"（否则会**多压一份**，第 235 轮实测：函数作用域的推导式因此在
        // `LIST_APPEND` 时栈上多一个值、取到迭代器而报错）——只发右操作数
        if let Expression::Name(a, _) = left {
            if let Some(slot) = self.pending_fused_load {
                if self.unit.varnames.get(slot).is_some_and(|item| item == a) {
                    self.pending_fused_load = None;
                    return self.emit_expression(right);
                }
            }
        }
        let pack = match (self.kind, left, right) {
            (ScopeKind::Function, Expression::Name(a, _), Expression::Name(b, _)) => {
                match (self.plain_local_slot(a), self.plain_local_slot(b)) {
                    // **前提：两个槽号都要装得进 4 位** ✓（第 662 轮真 bug 修 ✗）：融合形式把两个槽号
                    // 各塞进半个字节（`oparg >> 4` / `oparg & 0x0F` ✓）⇒ 槽号 ≥16 时装不下 ✗ ⇒ 低半格
                    // **按位截断**，于是读到**完全不同的槽** ✓（实测 `_parser.py:879` 的 `subpattern[i]`
                    // 是槽 5 与 **46** ✓ ⇒ `(5 << 4) | 46` 被截成 `(5 << 4) | 14` = 126 ✗ ⇒ 运行期去读
                    // **槽 14**（名叫 `set` ✓、此刻还没绑定 ✓）⇒ `局部槽 14 未绑定` ✗）。
                    // 装不下就返回 `None` ✓，交给下面的**非融合回退**发两条独立加载 ✓（参照同样只在 <16 时融合 ✓
                    // —— 与 `AssignAttr` 那处同一条规矩 ✓）。
                    (Some(first), Some(second)) if first <= 0x0F && second <= 0x0F => {
                        Some((first, second))
                    }
                    (Some(_), Some(_)) => None,
                    _ => None,
                }
            }
            _ => None,
        };
        match pack {
            Some((first, second)) => {
                self.emit_at(
                    left.span(),
                    opcode::opcode("LOAD_FAST_BORROW_LOAD_FAST_BORROW")
                        .expect("超指令在表里"),
                    ((first << 4) | second) as u8,
                );
                Ok(())
            }
            None => {
                self.emit_expression(left)?;
                self.emit_expression(right)
            }
        }
    }

    /// 发射一个**可缺省**的表达式：缺省时压 `LOAD_CONST None`（切片缺界的实测形态）。
    pub(crate) fn emit_optional(
        &mut self,
        owner: &Expression,
        part: &Option<Box<Expression>>,
    ) -> Result<(), CompileError> {
        match part {
            Some(expression) => self.emit_expression(expression),
            None => {
                let index = self.intern_constant(Constant::None);
                self.emit_indexed(owner.span(), "LOAD_CONST", index);
                Ok(())
            }
        }
    }

}
