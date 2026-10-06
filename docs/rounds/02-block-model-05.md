> 本卷是 `docs/ROUNDS.md` 分卷台账之一（**非规范**、只增不改；卷目见该索引）✓

#### 第 137 轮：`subscript` 域**未过** ✗（工具改进的补丁也没落上 ✗）—— 如实记 ✓

**① 读到的真因** ✓（`SPLIT_KEEP=1` 保留现场 ✓）：
```
error[E0425]: cannot find type `ExecError` in this scope   --> executor/subscript.rs:29
error[E0425]: cannot find type `ListObject` ...            --> executor/subscript.rs:46
error[E0425]: cannot find type `TupleObject` ...           --> executor/subscript.rs:60
```
⇒ 新文件缺类型 import ✓，而 **`ExecError` 恰恰定义在 `executor.rs` 自己里** ✓ ⇒ 我原先的自愈
**只会去 `builtin_objects` 找** ✗ ⇒ 补不上 ✓。

**② 打算做的改进** ✓（下一轮接着做 ✓）：自愈改成"**全 crate 找定义处**" ✓ ——
在 crates/pyawa-core/src/ 下的全部 .rs 里按 `^pub (unsafe )?(struct|enum|type|trait|fn|const|static|mod) <名>`
或 `^pub use … <名>` 找到它 ✓ ⇒ 由文件路径推出模块路径 ✓ ⇒ 生成 `use crate::<模块>::<名>;` ✓
（`ExecError` 就会得到 `use crate::executor::ExecError;` ✓）；
插入位置＝新文件**最后一行 `use` 之后** ✓。

**③ 为什么这一轮没落上** ✗：补丁的匹配串**漏了脚本里另一行**（上一轮加的
"`names |= set(re.findall(r"cannot find macro …`" ✓）⇒ `assert t.count(old) == 1` 直接失败 ✗
⇒ **脚本没被改** ✓；随后命令按**原样**又跑了一次 ✓ ⇒ 依旧编译不过 ✓ ⇒ **事务式守卫自动还原** ✓
⇒ 树干净 ✓、`executor.rs` 仍 **9239** 行 ✓、`builtin_objects.rs` 仍 **4165** 行 ✓。
**教训** ✓：改脚本前**先把要匹配的整段读出来** ✓（这一步我已吃过三次 ✗：转义、SyntaxError、漏行 ✓）。

**④ 验收** ✓（本轮无行为改动 ✓）：**0 警告** ✓、树**干净** ✓、逐字节 **4/4** ✓、`check.py` 12/12 ✓；
目标第 ⑥ 条当前状态 ✓：`builtin_objects.rs` **4165**（已达标 ✓）、`executor.rs` **9239**（**未达标** ✗）。

#### 第 136 轮：拆分器**推广到 `executor.rs`** ✓（工具成了 ✓）—— 第一个域 `subscript` 试拆未过 ✗、**自动还原** ✓

**① 工具推广（本轮实做 ✓）**：把 `builtin_objects.rs` 专用脚本参数化成 `target/split_domain.py` ✓：
- `SPLIT_SRC`（源文件 ✓）／`SPLIT_DIR`（目标目录 ✓）／`SPLIT_DECL`（加 `mod X;` 的文件 ✓）／
  `SPLIT_OLD`（旧模块前缀 ✓）／`SPLIT_NEW`（新模块路径 ✓）；
- `use` 锚点改成**源文件里第一个 `use` 行** ✓（`executor.rs` 没有 `use crate::header::Header;` ✗）；
- `out.parent.mkdir(parents=True, exist_ok=True)` ✓（**目录不存在**是这一轮头两次失败的真因 ✗ ——
  `FileNotFoundError` 发生在写盘、**守卫没兜住** ✗ ⇒ 顺带给"写盘＋编译"整段加了兜底 ✓）；
- 这样 `foo.rs` ＋ `foo/bar.rs` 的形态可用 ✓ ⇒ **不必**把 `executor.rs` 改名成 `executor/mod.rs` ✗
  （`docs/` 的路径引用因此不受影响 ✓）。

**② 第一个域试拆** ✗：`subscript`（`subscript_read`／`subscript_write`／`subscript_del` 一族 ✓）
脚本跑通（生成的 `executor/subscript.rs` ✓、`executor.rs` 9239 → 8821 ✓），**但编译未过** ✗
（落点 `crates/pyawa-core/src/executor/subscript.rs:83` ✓）⇒ **事务式守卫自动还原** ✓
⇒ `executor.rs` 回到 **9239** 行 ✓、工作树**干净** ✓ ⇒ **本轮不改代码** ✓（目标第 ⑦ 条 ✓）。

**③ 下一轮**（就一件事 ✓）：把 `subscript.rs:83` 那处错误**完整读出来** ✓（用 `SPLIT_KEEP=1` ✓），
按错因决定：① 自愈补 import ✓；② 若是"域边界没切对"（比如 `subscript` 与调用机制共享私有状态 ✓）
⇒ 换一个**更内聚**的域（`arithmetic`／`import`／`attribute` 之一 ✓）或把共享私有项一起搬 ✓。
**判据**仍是全闸门 ✓（0 警告／逐字节 4/4／对拍两模式／`check.py`／夹具／语料下限／`selftest`／`t_ab_1` ✓）。

**④ 验收** ✓（本轮无行为改动 ✓）：**0 警告** ✓、工作树**干净** ✓、逐字节 **4/4** ✓（未被触碰 ✓）、
`builtin_objects.rs` 仍 **4165** 行 ✓、`executor.rs` 仍 **9239** 行 ✓。

#### 第 135 轮：`thread` 族收掉 ✓（`builtin_objects.rs` 4420 → **4165**）✓ —— 第三处工具缺口**有了处置法** ✓

**① 处置法** ✓（本轮实测有效 ✓，写进流程 ✓）：
```
SPLIT_KEEP=1 python3 target/split_family.py thread     # 先保留现场（脚本自检失败时**别**还原 ✓）
手工把 lib.rs 的 `pub use builtin_objects::{a, b};` 改成逐条 `pub use builtin::thread::<名>;` ✓
cargo build → 0 错 → cargo fix → 0 警告 → 逐字节 4/4 ✓
```
**为什么上次没成** ✗：脚本自检失败时**先**还原了 `builtin.rs` 里的 `mod thread;` ✓，我的补丁才落下去 ✓
⇒ 变成"指向不存在的模块"（`E0432: unresolved import builtin::thread` ✗）⇒ **顺序**问题 ✓ ——
所以要用 `SPLIT_KEEP=1` ✓（这也是为什么**先读真因、再动手** ✓）。

**② 结果** ✓（纯移动 ✓、零逻辑改动 ✓）：
```
新增  crates/pyawa-core/src/builtin/thread.rs  （270 行 ✓）
builtin_objects.rs   4420 → **4165** 行 ✓
```
十四族累计 ✓：… ／ `thread` ⇒ **8990 → 4165** ✓（约 **54%** 已搬出 ✓；`builtin_objects.rs`
已从"近万行"降到 **4165** ✓，**不再**是巨型文件 ✓）。扫描面同一步加上 `thread.rs` ✓
（`gc_field_coverage` 3/3 绿 ✓）。

**③ 验收** ✓：**0 警告** ✓、逐字节 **4/4** ✓、`code_layout` ✓、`cargo test --workspace`（唯一红仍是那条
既有间歇缺陷 ✓）、对拍普通与 `DANGLING` 同既有口径 ✓、`check.py` 12/12 ✓、夹具 **490** ✓、
语料下限 **182** ✓、`selftest` 22 ✓、`t_ab_1` ✓。

**④ 下一轮** ✓：**把拆分器推广到 `executor.rs`**（9239 行 ✓ —— 目标第 ⑥ 条只剩它 ✓）：
`foo.rs` ＋ `foo/bar.rs` 合法 ✓ ⇒ 在 `executor.rs` 里加 `mod <域>;` ✓、源文件加
`use crate::executor::<域>::*;` ✓；脚本参数化"源文件 ＋ 声明所在文件 ＋ 目标目录" ✓
（`SPLIT_SRC`／`SPLIT_DECL`／`SPLIT_DIR` ✓）。**第一个域＝`subscript`**（6 个函数，边界最清楚 ✓），
**成不成由闸门判** ✓（编译不过自动还原 ✓）。

#### 第 134 轮：`thread` 族的**真因拿到** ✓（工具的第三处缺口）—— 本轮**不改代码**，如实记 ✓

**① 真因** ✓（保留现场读出来的 ✓）：
```
error[E0603]: function import `thread_allocate_lock_native` is private
  --> crates/pyawa-core/src/lib.rs:26:27
26 | pub use builtin_objects::{thread_allocate_lock_native, thread_get_ident_native};
```
⇒ `lib.rs` 的再导出是**花括号形式**（`pub use builtin_objects::{a, b};` ✗），而我的脚本只认
`builtin_objects::名字` 这一种写法 ✗ ⇒ **路径没跟着搬** ✓ ⇒ 搬走之后旧路径落空／可见性对不上 ✓。
**这已经是工具的第三处缺口** ✓（前两处：不认 `#[...]` 与空行 ✗、把 `impl` 里的方法当顶层项 ✗）。

**② 本轮**不改代码** ✓（如实 ✓）**：修这一处要么把"花括号再导出"也重写成两条独立 `pub use`
（并确保搬走项仍是 `pub` ✓），要么把整个再导出改成经新模块 ✓ —— 都需要**重新跑一遍闸门** ✓；
本轮剩余预算不够**稳妥**做完 ✓ ⇒ **不留半成品** ✓（这也是目标第 ⑦ 条 ✓）。
当前树**干净** ✓、**0 警告** ✓、`builtin_objects.rs` 仍 **4420** 行 ✓、`executor.rs` 仍 **9239** 行 ✓。

**③ 下一轮的两件事（都钉死了 ✓）**：
1. **修工具第三处缺口** ✓：处理 `pub use <旧模块>::{a, b};` 形式 ✓（改成逐条指向新模块 ✓，
   并核对被搬项的可见性仍是 `pub` ✓ —— 上一轮那条 `E0603` 说明有个环节把它当成了私有 ✓）；
   然后按序把这个 **`thread`（2 个函数）** 收掉 ✓（小，但这是工具最后一次被"家族外再导出"绊住 ✓）；
2. **把拆分器推广到 `executor.rs`** ✓：Rust 允许 `foo.rs` ＋ `foo/bar.rs` ✓ ⇒ **不必**把
   `executor.rs` 改名成 `executor/mod.rs` ✗（那样会打断 `docs/` 里的路径引用 ✓）；做法是在
   `executor.rs` 里加 `mod <域>;` ✓、源文件里加 `use crate::executor::<域>::*;` ✓，
   脚本只需参数化"源文件 ＋ 声明所在文件 ＋ 目标目录" ✓（`SPLIT_SRC`／`SPLIT_DECL`／`SPLIT_DIR` ✓）。
   第一个域建议取**边界最清楚**的 `subscript`（`subscript_read`／`subscript_write`／`subscript_del`
   一族 ✓，6 个函数 ✓），成不成**由闸门判** ✓（编译不过就自动还原 ✓）。

**④ 验收** ✓（本轮只有诊断与台账 ✓）：**0 警告** ✓、工作树**干净** ✓、逐字节 **4/4**（未被触碰 ✓）、
`check.py` 12/12 ✓；十三族累计不变 ✓：**8990 → 4420** ✓。

#### 第 133 轮：又拆三族 ✓（`int` 6 ／ `function` 7 ／ `float` 6）—— `builtin_objects.rs` 5101 → **4420** ✓；`thread` 仍卡 ✗

**① 本轮结果** ✓（纯移动 ✓、零逻辑改动 ✓）：
```
新增  crates/pyawa-core/src/builtin/int.rs       （6 个 ✓ 259 行 ✓）
新增  crates/pyawa-core/src/builtin/function.rs  （7 个 ✓ 316 行 ✓）
新增  crates/pyawa-core/src/builtin/float.rs     （6 个 ✓ 184 行 ✓）
builtin_objects.rs   5101 → **4420** 行 ✓
```
十三族累计 ✓：`str` 45 ／ `bytes` 32 ／ `dict` 18 ／ `deque` 19 ／ `list` 14 ／ `object` 9 ／ `context` 16 ／
`generator` 9 ／ `set` 10 ／ `property` 9 ／ `int` 6 ／ `function` 7 ／ `float` 6 ⇒ **8990 → 4420** ✓
（约 **51%** 已搬出 ✓）。被引用项（`bound_int`／`bytes_getattr`／`digit_limit_message`／`parse_decimal` ✓）
放宽 `pub(crate)` ✓；自愈循环补了 `Decimal`／`IntObject`／`FunctionObject`／`FloatObject` 等 ✓；
扫描面同一步加上三个文件 ✓（`gc_field_coverage` 3/3 绿 ✓）。

**② `thread` 族仍没过** ✗（**事务式守卫自动还原** ✓）：新文件第 8 行就编译错 ✗
（自愈循环补的是 `crate::builtin_objects::X` ✓ —— 第 8 行是 `use` 区 ✓ ⇒ 说明缺的东西**不在
`builtin_objects`**（多半在别的模块 ✓，或在 `pyawa-capabilities` ✓）⇒ 自愈补不上 ✓）。
**下一轮**：把 `thread` 的完整错误读出来 ✓，按"缺什么就从**哪个模块** import"扩展自愈 ✓
（这一步也给后面 `executor.rs`/`instance.rs` 的大拆分铺路 ✓）。

**③ 验收** ✓：**0 警告** ✓、逐字节 **4/4** ✓、`code_layout` ✓、`cargo test --workspace`（唯一红仍是那条
既有间歇缺陷 ✓）、对拍普通与 `DANGLING` 同既有口径 ✓、`check.py` 12/12 ✓、夹具 **490** ✓、
语料下限 **182** ✓、`selftest` 22 ✓、`t_ab_1` ✓。

**④ 下一轮** ✓：① 修 `thread` ✓；② 把 `builtin_objects.rs` 剩下的 **4420 行**按域继续拆 ✓
（消息/格式化/异常/`is` 一族/属性槽等 ✓ —— 这些不是"类型族"，要按**函数名前缀 + 用途**成组 ✓）；
③ 之后进 `executor.rs`（`call`/`subscript`/`arithmetic`/`import`/`attribute` ✓）⇒ 之后 `instance.rs` 与
把 `PYAWA_*` 诊断收进 `diag.rs` ✓。

#### 第 132 轮：`set`／`property` 两族修好搬成 ✓（10 + 9 个）—— 真因是**工具的两处**（误搬 `impl` 方法 ✗、私有项没放宽 ✗）

**① 上一轮那两族为什么没过** ✓（本轮把编译错误完整读出来 ✓）：
- `set` ✗：`error: 'self' parameter is only allowed in associated functions` ⇒ 我的模式带了
  `^[ \t]*` ✓ ⇒ 把 **`impl` 块里的方法**（`pub fn set_kind(&self, …)` ✓）也当"顶层函数"搬走了 ✗
  ⇒ 修法：**只搬行首无空白的顶层项** ✓（去掉 `[ \t]*` ✓）；
- `property` ✗：`error[E0603]: enum 'PropertySlot' is private` ⇒ 自愈循环想 import ✓ 但那个 `enum`
  在原文件里是**私有**的 ✗ ⇒ 修法：自愈时若缺的名字在原文件里是**私有定义** ⇒ **先放宽成 `pub(crate)`** ✓
  再 import ✓（与 `refs` 同一处理 ✓）。

**② 结果** ✓（纯移动 ✓、零逻辑改动 ✓）：
```
新增  crates/pyawa-core/src/builtin/set.rs       （10 个函数 ✓ 272 行 ✓）
新增  crates/pyawa-core/src/builtin/property.rs  （9 个函数 ✓ 223 行 ✓）
builtin_objects.rs   5545 → **5101** 行 ✓
```
十族累计 ✓：`str` 45 ／ `bytes` 32 ／ `dict` 18 ／ `deque` 19 ／ `list` 14 ／ `object` 9 ／ `context` 16 ／
`generator` 9 ／ `set` 10 ／ `property` 9 ⇒ **8990 → 5101** ✓（约 **57%** 已搬出 ✓）。
被引用项（`bound_set`／`container_contains_native`／`tuple_clear`／`tuple_traverse`／`bound_property`／
`dict_getattr` ✓）放宽 `pub(crate)` ✓；自愈循环补了两族的 `py_object!` 生成类型与 `PropertySlot` ✓；
扫描面同一步加上两个文件 ✓（`gc_field_coverage` 3/3 绿 ✓）。

**③ 验收** ✓：**0 警告** ✓、逐字节 **4/4** ✓、`code_layout` ✓、`cargo test --workspace`（唯一红仍是那条
既有间歇缺陷 ✓）、对拍普通与 `DANGLING` 同既有口径 ✓、`check.py` 12/12 ✓、`selftest` 22 ✓、`t_ab_1` ✓。

**④ 下一轮** ✓：`int`(3)／`thread`(2)／`function`(2)／`float`(2)／更大的族（如 `bytes`（已搬）之外剩下的
`str_*` 之类 ✓）⇒ 再之后 `executor.rs`（`call`/`subscript`/`arithmetic`/`import`/`attribute` ✓）⇒
之后 `instance.rs` 与把 `PYAWA_*` 诊断收进 `diag.rs` ✓。

#### 第 131 轮：拆出 `generator` 族 ✓（9 个 / 301 行）✓；`set`／`property` 两族**编译未过、自动还原** ✗（下轮细看 ✓）

**① 本轮结果** ✓（纯移动 ✓、零逻辑改动 ✓）：
```
新增  crates/pyawa-core/src/builtin/generator.rs  （9 个函数 ✓ 301 行 ✓）
builtin_objects.rs   5842 → **5545** 行 ✓
```
八族累计 ✓：`str` 45 ／ `bytes` 32 ／ `dict` 18 ／ `deque` 19 ／ `list` 14 ／ `object` 9 ／ `context` 16 ／
`generator` 9 ⇒ **8990 → 5545** ✓（约 **62%** 已搬出 ✓）。被引用项
`async_generator_anext_native`／`async_generator_asend_native`／`exception_instance`／`resume_with_sent`／
`stop_iteration`／`thrown_exception` 放宽 `pub(crate)` ✓；自愈循环补 5 个 `py_object!` 生成类型 ✓；
扫描面同一步加上 `generator.rs` ✓（`gc_field_coverage` 3/3 绿 ✓）。

**② 同一轮的另两族没有过** ✗（**事务式守卫按纪律自动还原** ✓，树始终干净 ✓）：
`set` 与 `property` 的新文件**编译不过** ✗（脚本尾部打出的是 `set.rs:196`／`property.rs:15` 的编译错 ✓）
⇒ 两族**未搬** ✓，`builtin_objects.rs` 里原样留着 ✓。**下一轮**要把这两族的编译错误**完整读出来** ✓
（自愈循环只会补 `crate::builtin_objects::X` ✓ —— 若缺的东西在**别的模块**（如 `crate::instance::…` ✓）
它就补不上 ✓，这大概就是这两族卡住的原因 ✓），再决定是"补 import"还是"换个切法" ✓。

**③ 验收** ✓：**0 警告** ✓、逐字节 **4/4** ✓、`code_layout` ✓、`cargo test --workspace`（唯一红仍是那条
既有间歇缺陷 ✓）、对拍普通与 `DANGLING` 均按既有口径 ✓、`check.py` 12/12 ✓、夹具 **490** ✓、
语料下限 **182** ✓、`selftest` 22 ✓、`t_ab_1` ✓。

**④ 下一轮** ✓：先修 `set`／`property` 两族 ✓ ⇒ 再 `int`(3)／`thread`(2)／`function`(2)／`float`(2) ✓
⇒ 之后 `executor.rs`（`call`/`subscript`/`arithmetic`/`import`/`attribute` ✓）⇒ 之后 `instance.rs` 与
`diag.rs` ✓。

#### 第 130 轮：`context` 族**修好工具后搬成** ✓（16 个 / 369 行）✓ —— 上一轮回退的真因是**工具没带 `#[...]` 属性** ✗

**① 真因** ✓（承第 129 轮的回退 ✓）：那 4 条 `never used` **不是**引用缺口 ✓，而是
`#[allow(dead_code)]` **和函数分了家** ✗ —— 那 4 个实现是**故意留作"下一轮接线"**的 ✓
（第 332 轮把 `Context` 的方法面改成如实报 `NotImplementedError` ✓，实现体留着 ✓），
属性与文档注释之间还**隔着一个空行** ✗ ⇒ 我的脚本往上只吃 `///` **不吃 `#[...]`／空行** ✗
⇒ **属性留在原文件、函数搬走** ✓ ⇒ 4 条"无人引用" ✓。

**② 工具这次的修法** ✓（两处 ✓）：往上吃紧邻的 **`#[...]` 属性** ✓，并且**允许中间夹空行** ✓
（`s.startswith("///") or s.startswith("#[") or s == ""` ✓）。修完重跑 ⇒ `context.rs` 里
`#[allow(dead_code)]` **4 处** ✓ 全都跟着搬走了 ✓ ⇒ **0 警告** ✓。
**没有**用"新增 `#[allow(dead_code)]`／放松闸门"的办法压绿 ✓（那是第 ⑦ 条不允许的 ✓）。

**③ 结果** ✓（纯移动 ✓、零逻辑改动 ✓）：
```
新增  crates/pyawa-core/src/builtin/context.rs  （16 个函数 ✓ 369 行 ✓）
builtin_objects.rs   6168 → **5842** 行 ✓
```
七族累计 ✓：`str` 45 ／ `bytes` 32 ／ `dict` 18 ／ `deque` 19 ／ `list` 14 ／ `object` 9 ／ `context` 16
⇒ **8990 → 5842** ✓（约 **65%** 已搬出 ✓）。派发表 `context_getattr` 不搬 ✓；
被引用项 `copy_context_value` 放宽 `pub(crate)` ✓；自愈循环补了 6 个 `py_object!` 生成类型 ✓；
扫描面同一步加上 `context.rs` ✓（`gc_field_coverage` 3/3 绿 ✓）。

**④ 验收** ✓：**0 警告** ✓、逐字节 **4/4** ✓、`code_layout` ✓、`cargo test --workspace` 无失败 ✓、
对拍普通与 `DANGLING` 均 **181/182**（新差异 1 ＝既有间歇缺陷 ✓）、`check.py` 12/12 ✓、
`selftest` 22 ✓、`t_ab_1` ✓。

**⑤ 下一轮** ✓：`set`(4) → `property`(4) → `generator`(4) → `int`(3) → `thread`(2) → `function`(2) →
`float`(2) ⇒ 之后 `executor.rs`（`call`/`subscript`/`arithmetic`/`import`/`attribute` ✓）⇒ 之后 `instance.rs`
与把 `PYAWA_*` 诊断收进 `diag.rs` ✓。**教训入册** ✓：**拆族时"文档注释之外的紧邻属性"必须一起走** ✓。

#### 第 129 轮：`context` 族**试拆后回退** ✓（如实记 ✓）—— 4 条「无人引用」警告暴露了它的**引用关系还没弄清** ✗

**① 试拆的结果** ✗：脚本对 `context` 族一次通过（搬走 **16** 个、`context.rs` 341 行、
`builtin_objects.rs` 6168 → **5842** ✓、逐字节 **4/4** ✓、`gc_field_coverage` 3/3 ✓），
**但 `cargo check --workspace --all-targets` 报出 4 条警告** ✗：
```
function `context_get_native` is never used
function `context_contains_native` is never used
function `context_copy_native` is never used
function `context_run_native` is never used
```
而 **0 警告是硬闸门** ✗ ⇒ 按纪律**回退**（目标第 ⑦ 条 ✓）。

**② 已查明的一半** ✓：`builtin_objects.rs` 里 `use crate::builtin::context::*;` **在** ✓（glob 生效 ✓），
但全仓 grep **找不到**这 4 个函数在 `context.rs` 之外的**任何引用** ✗ ⇒ 也就是说：
搬运**没有破坏引用**（否则会编译错 ✗），而是这 4 个函数被搬出去、**放宽成 `pub(crate)`** 之后
**暴露出"本来无人调用"** ✗（此前它们是同文件里的**私有**函数 ⇒ 编译器只在**同文件**里判"未使用" ✓，
所以从来没报过 ✗ —— 这是 Rust 的 `dead_code` 判定范围 ✓，不是本次搬运的语义变化 ✓）。

**③ 下一轮的处置（两条路，选一条 ✓）**：
1. **先查清"谁本该调用它们"** ✓：`contextvars` 的方法面（`Context` 的 `get`／`contains`／`copy`／`run` ✓）
   在参照里是**方法** ✓ ⇒ 大概率是**派发表还没接这 4 个** ✗（也就是一条**真实的缺口** ✓，不只是搬家问题 ✓）
   ⇒ 那就**先接上**（属于功能补线 ✓，另开一轮 ✓），再搬 ✓；
2. 若确认是**规格上就不该有**（多余的实现 ✓）⇒ 按"一处真相"**删掉** ✓，再搬 ✓。
**注意** ✗：**不许**用 `#[allow(dead_code)]` 把闸门压绿 ✓（那是把判据放松 ✓，目标第 ⑦ 条不允许 ✓）。

**④ 本轮状态** ✓：`context` 族**未搬**（`builtin_objects.rs` 仍 **6168** 行 ✓）；
回退后 **0 警告** ✓、逐字节 **4/4** ✓、工作树**干净** ✓；`gc_field_coverage` 的扫描面也随回退复原 ✓。
**六族累计不变** ✓：8990 → 6168 ✓。

#### 第 128 轮：拆出第六族 ✓ —— `builtin/object.rs`（9 个函数 / 198 行）✓

**① 结果** ✓（纯移动 ✓、零逻辑改动 ✓）：
```
新增  crates/pyawa-core/src/builtin/object.rs  （9 个 object_* 函数 ✓ 198 行）
builtin_objects.rs   6355 → **6168** 行 ✓
```
六族累计 ✓：`str` 45 ／ `bytes` 32 ／ `dict` 18 ／ `deque` 19 ／ `list` 14 ／ `object` 9
⇒ **8990 → 6168** ✓（约 **69%** 已搬出 ✓）。

**② 这一轮的一处**特别核对** ✓**：`object_*` 全是 `pub fn` ✓（`object_init_native` 等还被
`lib.rs` 以 `pub use` 再导出 ✓）⇒ 搬运时脚本把它们的新路径一并改到 `builtin::object::` ✓，
**可见性没被降级**（不是 `pub(crate)` ✓）⇒ `lib.rs` 的再导出语义不变 ✓（编译即验 ✓）。

**③ 检查清单照做 ✓**：拆完同一步把 `include_str!("../src/builtin/object.rs")` 加进
`gc_field_coverage`（**源码扫描型**闸门 ✓）的扫描面 ✓ ⇒ 当场复测 **3/3 绿** ✓。

**④ 验收** ✓：**0 警告** ✓、逐字节 **4/4** ✓、`code_layout` ✓、`cargo test --workspace` **无失败** ✓、
对拍普通与 `DANGLING` **均 181/182**（新差异 1 ＝既有间歇缺陷 ✓）、`check.py` 12/12 ✓、夹具 **490** ✓、
语料下限 **182** ✓、`selftest` 22 ✓、`t_ab_1` ✓；`stability` 唯一红仍落在**对拍**那目标 ✓
（既有间歇缺陷 ✓，目标里已写明"允许同格" ✓）。

**⑤ 下一轮** ✓：`context`(8) → `int`/`float`/`set`/`property`/`generator`/`function`/`thread`/`message`…
⇒ 之后 `executor.rs`（`call`/`subscript`/`arithmetic`/`import`/`attribute` ✓）⇒ 之后 `instance.rs` 与 `diag.rs` ✓。

#### 第 127 轮：拆出第五族 ✓ —— `builtin/list.rs`（14 个函数 / 335 行）✓；扫描面同一步跟上 ✓

**① 结果** ✓（纯移动 ✓、零逻辑改动 ✓）：
```
新增  crates/pyawa-core/src/builtin/list.rs  （14 个 list_* 函数 ✓ 335 行）
builtin_objects.rs   6675 → **6355** 行 ✓
```
五族累计 ✓：`str` 45 ／ `bytes` 32 ／ `dict` 18 ／ `deque` 19 ／ `list` 14 ⇒ **8990 → 6355** ✓（约 **71%** 已搬出 ✓）。

**② 上一轮的教训**这一轮**同一步照做** ✓：拆完立刻把 `include_str!("../src/builtin/list.rs")` 加进
`gc_field_coverage`（`CX-12`，**源码扫描型** ✓）的扫描面 ✓ ⇒ 该闸门当场复测 **3/3 绿** ✓，
不再出现"拆完才发现闸门红" ✗（这条已写进下一轮的检查清单 ✓）。

**③ 验收** ✓（本轮比上一轮更干净 ✓）：**0 警告** ✓、逐字节 **4/4** ✓、`code_layout` ✓、
`cargo test --workspace` **无失败** ✓（这一轮没撞上那条抖动 ✓）、`gc_field_coverage` 3/3 ✓、
对拍普通与 `DANGLING` **均 181/182**（新差异 1 ＝既有间歇缺陷 ✓）、`check.py` 12/12 ✓、
夹具 **490** ✓、语料下限 **182** ✓、`selftest` 22 ✓、`t_ab_1` ✓、`stability`（落点若红只会是那条既有缺陷 ✓）。

**④ 下一轮** ✓（一条命令一族 ✓）：`object`(9) → `context`(8) → `int`/`float`/`set`/`property`/`generator`…
⇒ 之后 `executor.rs`（`call`/`subscript`/`arithmetic`/`import`/`attribute` ✓）⇒ 之后 `instance.rs` 与 `diag.rs` ✓。

#### 第 126 轮：拆出第四族 ✓ —— `builtin/deque.rs`（19 个函数 / 425 行）✓；脚本补上 `unsafe fn` 与**宏**两类 ✓

**① 结果** ✓（纯移动 ✓、零逻辑改动 ✓）：
```
新增  crates/pyawa-core/src/builtin/deque.rs  （19 个 deque_* 函数 ✓ 425 行）
builtin_objects.rs   7084 → **6675** 行 ✓
```
四族累计 ✓：`str` 45 ／ `bytes` 32 ／ `dict` 18 ／ `deque` 19 ⇒ **8990 → 6675** ✓。

**② 脚本这一轮补了两类本事** ✓（`target/split_family.py` ✓）：
- **`unsafe fn`** ✓：`deque` 族全是 `pub unsafe fn` / `unsafe fn` ✗ ⇒ 先前的模式认不出来 ✓
  （第一次跑直接报"没找到 deque_* 函数" ✓）；
- **`macro_rules!` 宏** ✓：`deque_self!` 这种宏被搬走的代码用到 ✗ ⇒ 脚本现在会
  ① 在原文件的宏定义后补 `pub(crate) use <宏>;` ✓ ② 在新文件 import 它 ✓；
- **自愈循环** ✓ 本轮又补了 `BuiltinFunctionObject`／`DequeObject`／`MethodObject`／`NativeFn` ✓
  （与上一轮的 `DictObject` 一族同一类 ✓ —— `py_object!` 生成的类型 ✓）。
- 每次补丁都先跑 `ast.parse` **语法自检** ✓（上一轮"用 python 改 python"把脚本写成 SyntaxError 的教训 ✓），
  且事务式守卫仍在 ✓（失败自动还原 ✓）。

**③ 验收** ✓：**0 警告** ✓、逐字节 **4/4** ✓、`code_layout` ✓、`check.py` 12/12 ✓、夹具 **490** ✓、
语料下限 **182** ✓、`selftest` 22 ✓、`t_ab_1` ✓、`stability` ✓；
对拍 ✓：普通 **181/182**（新差异 1 ✓）与 `PYAWA_DANGLING=1` **181/182**（新差异 1 ✓）
—— 都是那条既有间歇缺陷（`class_keywords`／`method_defaults` ✓），与纯移动无关 ✓。

**③′ 收尾时补记（如实 ✓）**：`stability` 这一趟**红** ✗ —— 落点是 `-p pyawa-core --test gc_field_coverage` ✓：
那条闸门（`CX-12`）是**源码扫描型** ✓，它只 `include_str!("../src/builtin_objects.rs")` **一个文件** ✗
⇒ `deque` 族搬走后 `deque_traverse` 不在扫描面上 ✗ ⇒ "找不到" ✓。
**处置** ✓：把扫描面按"**判据不变、覆盖扩大**"扩到 `builtin/` 各文件 ✓（`concat!` 逐个 `include_str!` ✓，
并写明"**每新增一族都要加进来**" ✓）⇒ 复测 `gc_field_coverage` **3 项全绿** ✓。
**改正一句 ✗**（原写"`stability` 绿"是不准确的 ✓）：复跑 `stability` 这一趟**仍红** ✗，但落点换成了
**对拍那一目标** ✓（`-p pyawa-abi --test conformance` ✓）＝本会话那条**既有间歇缺陷**
（`class_keywords`／`method_defaults` ✓，单跑两模式均 181/182 ✓）⇒ 与本次拆分**无关** ✓。
**教训入册** ✓：**拆分每一个族，都要同步看一遍"源码扫描型闸门"的扫描面** ✓（下一轮的检查清单里加这一条 ✓）。

**④ 下一轮** ✓（一条命令一族 ✓）：`list`(9) → `object`(9) → `context`(8) → `int`/`float`/`set`/`property`/
`generator`… ⇒ 之后 `executor.rs` ⇒ 之后 `instance.rs` 与 `diag.rs` ✓。

#### 第 125 轮：拆出第三族 ✓ —— `builtin/dict.rs`（18 个函数 / 439 行）✓；脚本加**自愈补 import** ✓

**① 结果** ✓（纯移动 ✓、零逻辑改动 ✓）：
```
新增  crates/pyawa-core/src/builtin/dict.rs   （18 个 dict_* 函数 ✓ 439 行）
builtin_objects.rs   7510 → **7084** 行 ✓
```
（`str` 45 → `bytes` 32 → `dict` 18 ✓ ⇒ 8990 → 7084 ✓）

**② 脚本这一轮又长了一条本事** ✓（`target/split_family.py` ✓，不进仓库 ✓）：
- **自愈循环** ✓：编译失败时**照编译器报的缺失名字**自动补
  `use crate::builtin_objects::{…};` ✓，最多 6 轮 ✓ —— 本轮自动补了
  **`DictObject`／`ListObject`／`SetObject`／`TupleObject`** ✓
  （它们是 **`py_object!` 生成**、定义在原文件里的**类型** ✗ ⇒ 我原先只算了 `fn/const/static` ✗）；
- 并把新文件的 `use` 块改成**照抄原文件** ✓（`cargo fix` 随后删多余的 ✓）
  —— 上一版我手写的最小 `use` 块不够 ✗（`DictObject` 找不到 ✓），连续两次被事务式守卫拦下 ✓。
- 另有小修 ✓：补丁里的换行转义多写一层 ✗ ⇒ 照抄的 `use` 块成了空 ✓；以及上一轮的补丁把脚本写成
  SyntaxError ✗（**未执行** ⇒ 树干净 ✓）⇒ 这一轮**整份重写**（不再"用 python 改 python" ✓）。

**③ 验收** ✓：**0 警告** ✓、逐字节 **4/4** ✓、`code_layout` ✓、`check.py` 12/12 ✓、夹具 **490** ✓、
语料下限 **182** ✓、`stability`／`selftest` 22／`t_ab_1` ✓。
对拍 ✓：普通 **181/182**（新差异 1 ✓）；`PYAWA_DANGLING=1` **首跑 2 → 复跑 1** ✓，
报告里的差异用例是 **`class_keywords`** 与 **`method_defaults`** ✓（**两条既有**间歇缺陷 ✓，
本会话开头那条 `RefCell already borrowed` 就是后者 ✓）⇒ 两趟在 2/1 之间摆动，与**纯移动**无关 ✓
（记下来 ✓：这个模式**每轮都会出现**，所以验收口径按"复跑回到 1"判 ✓）。

**④ 下一轮** ✓（一条命令一族 ✓）：`deque`(13) → `list`(9) → `object`(9) → `context`(8) → `int`/`float`/
`set`/`property`/`generator`… ⇒ 之后 `executor.rs` ⇒ 之后 `instance.rs` 与 `diag.rs` ✓。

#### 第 124 轮：拆出第二族 ✓ —— `builtin/bytes.rs`（32 个函数 / 587 行）✓；脚本改成**按行区间 + 事务式** ✓

**① 这一轮的结果** ✓（纯移动 ✓、零逻辑改动 ✓）：
```
新增  crates/pyawa-core/src/builtin/bytes.rs   （32 个 bytes_* 函数 ✓ 587 行）
builtin_objects.rs   8085 → **7510** 行 ✓
```
派发表 `bytes_method_native` **不搬** ✓；被引用项 `starts_ends_with` 放宽 `pub(crate)` 后 import ✓；
外部引用（`instance.rs`）按「**实际搬走的名字集合**」精确改 ✓；`cargo fix` 清多余 `use` ✓。

**② 工具这一轮被修对了** ✓（`target/split_family.py` ✓，不进仓库 ✓）：
- **按行区间**搬运/剔除 ✓（上一版按**字节**拼接 ✗ ⇒ 把 `builtin_objects.rs` 切出
  `Ok(vec![text.to_ows,gsast(es&mndd,t.t(es(&out))` 这种**错位垃圾** ✗ ⇒ 编译不过 ✓）；
- **事务式** ✓：先备份 → 落盘 → `cargo build -p pyawa-core` → **失败自动还原** ✓
  （实测自动还原了 **2 次** ✓ ⇒ 树一直是干净的 ✓，这正是上一轮定的纪律 ✓）；
- 还加了**行区间两两不交**的断言 ✓（防重叠删除 ✓）。
⇒ 从"试错三轮"变成"**一条命令一族**" ✓。

**③ 验收** ✓：**0 警告** ✓、逐字节 **4/4** ✓、`code_layout` ✓、
对拍**普通与 `DANGLING` 两趟都 181/182**（新差异 **1** ＝ 既有间歇缺陷 `class_keywords` ✓；
`DANGLING` 第一次跑到过 **2** ✗ ⇒ 连跑两趟都回到 **1** ✓ ⇒ 是那条缺陷的抖动 ✓，与移动无关 ✓）、
`check.py` 12/12 ✓、夹具 **490** ✓、语料下限 **182** ✓、`stability`／`selftest` 22／`t_ab_1` ✓。

**④ 下一轮** ✓（一条命令一族 ✓）：`dict`(16) → `deque`(13) → `list`(9) → `object`(9) → `context`(8) →
`int`/`float`/`set`/`property`/`generator`… ⇒ 之后 `executor.rs`（`call`/`subscript`/`arithmetic`/`import`/
`attribute` ✓，`mod.rs` 只留指令循环 ✓）⇒ 之后 `instance.rs` 与把 `PYAWA_*` 诊断收进 `diag.rs` ✓。

#### 第 123 轮：**拆文件开工** ✓ —— `str` 族（45 个原生 / 842 行）搬进 `builtin/str.rs` ✓；外加 `lib.rs` 头部 2133 行注释进文档 ✓

**① 千行头部注释 → 文档** ✓（用户点名 ✓）：`crates/pyawa-core/src/lib.rs` 共 2253 行，**2133 行是 `//!`** ✓
（95% ✓）⇒ 原样搬进 `docs/CORE-MODULES.md` ✓，`lib.rs` 只留**短头 + 指向该文档** ✓
⇒ **2253 → 81 行** ✓（`bd43788` 文档 / `92bd332` 代码 ✓，纯注释、零逻辑 ✓）。

**② `builtin_objects.rs` 拆出第一族** ✓（纯移动 ✓、零逻辑改动 ✓）：
```
新增  crates/pyawa-core/src/builtin.rs        （mod 声明 ✓）
新增  crates/pyawa-core/src/builtin/str.rs    （45 个 str_* 原生 + 头说明 ✓ 915 行）
builtin_objects.rs  8990 → **8133** 行 ✓
```
取舍与踩到的坑（都记下来 ✓）：
- **派发表不搬** ✓：`str_method_native` 同时管 `bytes` 的名字 ✓ ⇒ 属"族之间的粘合层" ✓ 留在原处 ✓
  （第一次脚本把它一起搬了 ✗ ⇒ 报 70 个错 ✓ ⇒ 发现并回搬 ✓）；
- **可见性** ✓：搬走的函数要 `pub(crate)` ✓，否则 `use crate::builtin::str::*;` 的 glob 看不到它们 ✗
  （第一次漏了 ⇒ 44 个"cannot find value" ✓）；
- **外部引用** ✓：`instance.rs` 5 处 + `lib.rs` 1 处原来写 `crate::builtin_objects::str_*` ✓ ⇒ 要指到
  `builtin::str::` ✓（第一次我用**全文替换** ✗ ⇒ 把**没搬**的 `str_new`／`str_repr` 也改了 ✗ ⇒
  按"**实际搬走的名字集合**"精确修才干净 ✓）；
- **多余 `use`** ✓：`cargo fix --lib -p pyawa-core` 清掉 6 处 ✓（0 警告 ✓）。

**③ 验收** ✓：**0 警告** ✓、逐字节 **4/4** ✓、`code_layout` ✓、对拍 **181/182**（与移动前**同一格**的既有
间歇缺陷 ✓ ⇒ **行为零改动** ✓）、`check.py` 12/12 ✓、夹具 **490** ✓、语料下限 **182** ✓、`selftest` 22 ✓、
`t_ab_1` ✓。**未动** `docs/` 里对 `builtin_objects.rs` 的引用（该文件仍在 ✓ ⇒ 路径不断 ✓）。

**④ 下一轮** ✓（同一套流程，一轮一族 ✓）：`bytes`(25) → `dict`(16) → `deque`(13) → `list`(9) → `object`(9)
→ `context`(8) → …；每轮**纯移动 + 全闸门 + 单独提交** ✓。之后再拆 `executor.rs`
（`call.rs`／`subscript.rs`／`arithmetic.rs`／`import.rs` ✓），并在 `call.rs` 那一刀**顺带**做
「接收者只算一次」的契约收敛 ✓（与重构分开提交 ✓）。

#### 第 122 轮：隔离档下的**不对称** ✓ —— 有的命名空间**只涨不跌**（多留 ✗）、另一个却**被提前放到 0** ✗ ⇒ "同一批对象被按两种角色计数" ✓

**① 按第 121 轮的实验跑** ✓（隔离档 ＋ 释放轨迹 ✓，地址稳定 ✓）：
```
[ns 探针] namespace=0x…19a0 rc=2
[watch] incref → rc=3 现场=<module>@212
[watch] incref → rc=4 现场=<module>@212
[watch] incref → rc=5 现场=EnumType.__new__@39
[watch] incref → rc=6 现场=EnumType.__new__@40
[watch] decref → rc=5 现场=EnumType.__new__@40
…
[隔离区] incref 撞上**已释放对象** 0x…2fe0（原类型 dict，72 字节；释放于 EnumType.__new__@540）✗
```
⇒ **不对称** ✓：被盯的那个命名空间**只涨不跌**（一路 2→6 ✓，多留 ✗）；而**死掉的是另一个**类的命名空间 ✓
（`0x…2fe0` ✓，同样是 `dict`／72 字节 ✓）。

**② 这条不对称很说明问题** ✓：同一批对象（各类的命名空间 ✓）里，
**有的被多留**（泄漏 ✗）、**有的被提前放到 0**（少留 ✗）⇒ 不是简单的"少加一份"或"多加一份" ✓，
而是**同一批对象被按两种角色分别计数** ✓ —— 与第 118 轮那条读法（"**同一个对象被塞进两种角色**：
接收者 ＋ 属性字典" ✓、第 104 轮"实例化把实例**也放进 `args[0]`**、`bound` 另有其一" ✓）**正好互相印证** ✓。
⇒ 也就是说：**修法应当从"角色只算一次"入手** ✓，而不是在某处补一次 `retain` ✗
（补 `retain` 只会把"少留"挪到"多留" ✓，把崩溃换成泄漏 ✗ —— 这正是我前几轮没敢乱补的原因 ✓，如实记 ✓）。

**③ 下一轮（方向已收敛到一处 ✓）**：核"**同一个对象同时作为 `bound_self` 与 `args[0]`**"这条口径 ✓ ——
即 `call_callable`／native 那一支对**接收者**的计数 ✓（第 116 轮已读到 native 支"释放整个 `args` ✓、
`bound_self` 是借用 ✓"）；把**接收者只算一次**做成一处真相 ✓（要么调用方放进 `args` 时不再单独给 `bound` ✓、
要么 native 支跳过 `args[0]` 里的接收者 ✓）。**判据** ✓：`PYAWA_WATCH_DICT=1` 跑 `import enum`，
`attribute_clear` 不再出现在 `dict→0` 的回溯里 ✓；`PYAWA_QUARANTINE=1` 不再报"incref 撞上已释放对象" ✓；
普通档与逐字节闸门不回归 ✓。

**④ 闸门与数字** ✓（本轮无行为改动 ✓）：`cargo check --workspace --all-targets` **0 警告** ✓、
`cargo test --workspace` ✓、`check.py` 12/12 ✓、`CX-8` ✓、夹具守卫 ✓、语料下限 182 ✓、逐字节 **4/4** ✓、
对拍普通趟 ✓（`共 182 ⇒ 通过 181 · 已知 0 · 新差异 1` ＝ 那条既有间歇缺陷 ✓）；上限 **162** ✓、
判据① **27.4%**（172 ÷ 628 ✓）。

#### 第 121 轮：`load_module` 的命名空间账**读上去是平的** ✓ ⇒ 多放的那次在别处 ✗（如实记 ✓）；下一轮用"隔离档 ＋ 释放轨迹过滤地址"来定

**① 逐行读 `load_module`** ✓（`executor.rs:2478` 一带 ✓）：
```rust
let namespace = instance.new_dict();                       // 计数 1（我方那份 ✓）
let module = instance.alloc(AttributeObject::new(       // **模块对象接手它** ⇒ 1 份 ✓
    module_type, RefCell::new(Some(namespace)), ...));
…
unsafe { instance.incref_object(namespace.as_ptr()) };     // 给帧**新增**一份 ✓
let frame = instance.alloc(Frame::for_code_with_namespace(frame_type, &code, namespace));
```
⇒ 摆下来：**模块对象 1 份 ＋ 帧 1 份 ＝ 2** ✓，`new_dict()` 那一份**已交给模块** ✓ ⇒ **这一处是平的** ✓
（如实 ✓：我第 120 轮那句"缺一份"的口气**收回来** ✓ —— 账面上不缺 ✓）。

**② 那么"多放"在哪** ✓：既然模块与帧各自持一份 ✓、而实证里 `EnumDict` 是被
`attribute_clear`（清某个**内联属性字典**的对象 ✓）放掉的 ✓ ⇒ 只能说明**有一个"属性字典＝命名空间"的对象
被提前释放** ✓ —— 即**模块对象本身**（或另一个同类对象 ✓）**在帧还在用的时候就被放到 0** 了 ✗。
⇒ 病灶从"命名空间少一份"改成"**那个持有者被提前释放**" ✓（方向更准 ✓）。

**③ 下一轮（实验已定 ✓，且要挑"能真崩"的那一档 ✓）**：用**隔离档**（`PYAWA_QUARANTINE=1` ✓，它能让
这条缺陷**确定性**复现 ✓、地址也稳定 ✓）**同时**开 `PYAWA_FREE_DEBUG=1` ✓ ⇒ 把**释放轨迹**按
**命名空间地址**过滤 ✓ ⇒ 就能看到"**哪个持有者在什么时候把它放了**" ✓（第 112 轮做过一次、那次没崩所以
没有命中 ✓ —— 这次挑能崩的档 ✓）。**判据** ✓：改完后 `PYAWA_WATCH_DICT=1` 跑 `import enum`，
`attribute_clear` 不再出现在 `dict→0` 的回溯里 ✓；`PYAWA_QUARANTINE=1` 不再报"incref 撞上已释放对象" ✓；
普通档与逐字节闸门不回归 ✓。

**④ 闸门与数字** ✓（本轮无行为改动 ✓）：`cargo check --workspace --all-targets` **0 警告** ✓、
`cargo test --workspace` ✓、`check.py` 12/12 ✓、`CX-8` ✓、夹具守卫 ✓、语料下限 182 ✓、逐字节 **4/4** ✓、
对拍普通趟 ✓（`共 182 ⇒ 通过 181 · 已知 0 · 新差异 1` ＝ 那条既有间歇缺陷 ✓）；上限 **162** ✓、
判据① **27.4%**（172 ÷ 628 ✓）。

#### 第 120 轮：🎯🎯 **最后一格找到了** ✓ —— 模块对象的属性字典**就是**命名空间（`executor.rs:2493` ✓）⇒ 模块一被释放，命名空间就跟着没 ✗

**① 三处探针（本轮落地 ✓）**：`header.rs`（外部实例字典 ✓）、`type_object.rs`（类型字典 ✓）、
`builtin_objects.rs`（内联属性字典 ✓）各加一发 `PYAWA_SETDICT_DEBUG=1` 的探针 ✓ ⇒ 跑 `import enum` ✓，
把 `PYAWA_WATCH_DICT=1` 那条 `dict→0`（现场 `EnumDict.__init__@10` ✓）的**地址**拿去对号 ✓：
```
第一个死的 dict=0x612cf51bc900
[setdict] …… 共 44 条 ✓，但**这个地址一条都没出现** ✗
```
⇒ 说明：它**不是**通过三处 setter 登记进去的 ✓ ⇒ 只剩**构造函数**那条路 ✓
（`AttributeObject::new(ty, RefCell::new(Some(x)))` ✓，不走 `set_attributes` ✗）。

**② 顺着构造函数一查，就看到了** ✓：
```rust
// executor.rs:2490 附近（建模块对象 ✓）
let module = instance.alloc(crate::builtin_objects::AttributeObject::new(
    module_type,
    core::cell::RefCell::new(Some(namespace)),      // ← **模块的属性字典就是命名空间** ✓
)).into_raw().cast::<Header>();
```
⇒ 这**语义上是对的** ✓（模块的 `__dict__` 本来就是它的命名空间 ✓）—— 但它意味着：
**模块对象一旦被释放** ✓，`AttributeObject` 的 `attribute_clear` 就会**把命名空间放掉** ✗
⇒ 而**同一个命名空间**此时还被别处用着 ✓（帧的 `globals` ✓、类创建 ✓）⇒ 少一份 ⇒ 之后一用就撞上已释放 ✓✓
—— 与第 117／118 轮那条回溯（`attribute_clear` 放掉 `EnumDict` ✓、帧里有 `execute`／`call_callable` ✓）
**完全吻合** ✓。

**③ 下一轮（就是修复本身 ✓，判据明确 ✓）**：查 `load_module`（`executor.rs:2480` 一带 ✓）这个**模块对象**与
**命名空间**的所有权 ✓ —— 模块对象被谁在什么时候放掉 ✓、它的 `attributes` 是不是**只有它自己**那一份 ✓；
按参照口径，模块的命名空间至少要**被帧的 `globals` 与模块对象各持一份** ✓ ⇒ 缺的那一份补上 ✓
（或在模块对象释放时**不**把命名空间当成"它自己唯一那份" ✓）。
**判据** ✓：改完后 `PYAWA_WATCH_DICT=1` 跑 `import enum`，`attribute_clear` 不再出现在 `dict→0` 的回溯里 ✓；
`PYAWA_QUARANTINE=1` 不再报"incref 撞上已释放对象" ✓；普通档与逐字节闸门不回归 ✓。

**④ 闸门与数字** ✓：`cargo check --workspace --all-targets` **0 警告** ✓、`cargo test --workspace` ✓、
`check.py` 12/12 ✓、`CX-8` ✓、夹具守卫 ✓、语料下限 182 ✓、逐字节 **4/4** ✓、对拍普通趟 ✓
（`共 182 ⇒ 通过 181 · 已知 0 · 新差异 1` ＝ 那条既有间歇缺陷 ✓）；上限 **162** ✓、
判据① **27.4%**（172 ÷ 628 ✓）。

#### 第 119 轮：把"谁按属性字典登记"的**三处入口定位清楚** ✓（本轮**无行为改动** ✓，如实记 ✓）

**① 本轮做了什么** ✓：按第 118 轮的清单去查"谁把 `EnumDict` 设成某个对象的属性字典" ✓ ——
三处入口都**找到并读准了** ✓（含确切签名与位置 ✓）：
| 入口 | 位置 | 语义 |
|---|---|---|
| `Header::store_instance_dict(mapping)` | `header.rs:74` | **外部**实例字典（固定布局的实例／子类 ✓） |
| `AttributeObject::set_attributes(Option<mapping>)` | `builtin_objects.rs` | **内联**属性字典（`HAS_INLINE_INSTANCE_DICT` 那一支 ✓） |
| `TypeObject::set_dict(Option<mapping>)` | `type_object.rs:328` | **类型字典**（`type.__dict__` ✓） |

**② 但本轮的探针没加成** ✗（如实 ✓）：第一次改 `builtin_objects.rs` 时**函数签名与我的匹配式不符** ✗
⇒ 脚本在断言处停下 ✓ ⇒ **没有写入任何文件** ✓（`crates/` 已确认干净 ✓）⇒ 本轮**不改代码** ✓，
只把"下一轮要改哪三处、确切签名是什么"钉下来 ✓。

**③ 下一轮（最后一格，动作已确定 ✓）**：在这三处各加一发受控探针 ✓
（`PYAWA_SETDICT_DEBUG=1` ✓：打印**传进来的映射地址 ＋ 当前 Python 现场** ✓），
配合 `PYAWA_WATCH_DICT=1` 那条 `dict→0` 的地址 ✓ **同一趟**对号 ✓ ⇒
**谁把命名空间按"属性字典"登记出去**就会现形 ✓（这是整条链的最后一格 ✓）。
**判据** ✓：改完后 `PYAWA_WATCH_DICT=1` 跑 `import enum`，`attribute_clear` 不再出现在 `dict→0` 的回溯里 ✓，
且 `PYAWA_QUARANTINE=1` 不再报"incref 撞上已释放对象" ✓、普通档不回归 ✓。

**④ 闸门与数字** ✓（本轮无行为改动 ✓）：`cargo check --workspace --all-targets` **0 警告** ✓、
`cargo test --workspace` ✓、`check.py` 12/12 ✓、`CX-8` ✓、夹具守卫 ✓、语料下限 182 ✓、逐字节 **4/4** ✓、
对拍普通趟 ✓（`共 182 ⇒ 通过 181 · 已知 0 · 新差异 1` ＝ 那条既有间歇缺陷 ✓）；上限 **162** ✓、
判据① **27.4%**（172 ÷ 628 ✓）。

#### 第 118 轮：🎯 回溯给到**完整的 8 帧** ✓ —— 解释器释放"某个内联属性字典对象"时，那个对象的属性字典**就是 `EnumDict`** ✗

**① 完整回溯** ✓（把 `dict→0` 那条的 16 行都取出来 ✓）：
```
0: Instance::release_object
1: pyawa_core::builtin_objects::attribute_clear      ← 清"内联属性字典"⇒ 把 EnumDict 放了 ✗
2: Instance::release_one                            ← 那个对象本身掉到 0 ⇒ 触发 clear
3: Instance::release_object
4: pyawa_core::executor::release                    ← **解释器在释放一个值** ✓
5: executor::execute::{closure#1}
6: executor::execute
7: executor::call_callable
```
⇒ 读法 ✓：**解释器释放了某个"内联属性字典"的对象** ✓，而**那个对象的属性字典就是 `EnumDict` 实例** ✗
⇒ 清它的那一刻把命名空间当"自己那一份"放掉 ✓ ⇒ 少了一份 ✓ 之后别人再用就撞上已释放对象 ✓✓。

**② 顺手排掉一条** ✓：`mounted_instance_dict`（惰性建实例字典那处 ✓）**建的是新 `dict`** ✓
（`DictObject::new(builtin_dict, RefCell::new(Vec::new()))` ✓）⇒ **不是**它把命名空间塞进去的 ✗
（如实记 ✓，省得下一轮白查 ✓）。

**③ 下一轮（最后一格 ✓）**：查**谁把 `EnumDict` 设成了某个对象的属性字典** ✓ —— 候选就三处 ✓：
1. `set_attributes(Some(x))` 一族 ✓（内联字典那一支 ✓）；
2. `store_instance_dict(x)` ✓（外部那一支 ✓）；
3. `TypeObject::set_dict(x)` ✓（类型字典那一支 ✓，且第 116 轮那两条 `EnumType.__new__@{1042,1080}`
   正是 `classdict` 与 `enum_class.__dict__` ✓）。
**做法** ✓：在这三处各加一发受控探针 ✓（"若传进来的是 `dict` 且大小 72 ⇒ 报现场" ✓）⇒ 一轮就能看到
**是谁把命名空间按"属性字典"登记出去的** ✓。
**判据** ✓：改完后 `PYAWA_WATCH_DICT=1` 跑 `import enum`，`attribute_clear` 不再出现在 `dict→0` 的回溯里 ✓，
且 `PYAWA_QUARANTINE=1` 不再报"incref 撞上已释放对象" ✓、普通档不回归 ✓。

**④ 闸门与数字** ✓（本轮无行为改动 ✓）：`cargo check --workspace --all-targets` **0 警告** ✓、
`cargo test --workspace` ✓、`check.py` 12/12 ✓、`CX-8` ✓、夹具守卫 ✓、语料下限 182 ✓、逐字节 **4/4** ✓、
对拍普通趟 ✓（`共 182 ⇒ 通过 181 · 已知 0 · 新差异 1` ＝ 那条既有间歇缺陷 ✓）；上限 **162** ✓、
判据① **27.4%**（172 ÷ 628 ✓）。

#### 第 117 轮：🎯 **拿到最后一帧** ✓ —— 放掉 `EnumDict` 实例的是 **`attribute_clear`**（"清某个对象的属性字典"）✓

**① 让 `dict→0` 探针带 Rust 回溯** ✓（一行的事 ✓）⇒ 那条 `EnumDict.__init__@10` 的**释放方**现形 ✓：
```
0: Instance::release_object
1: pyawa_core::builtin_objects::attribute_clear     ← **清属性字典**那一步 ✓
2: Instance::release_one
3: Instance::release_object
```
⇒ 说明：**某个对象的"属性／实例字典"就是那个 `EnumDict` 实例** ✗ ⇒ 清它的那一刻，我们的代码把
**命名空间当成"自己那一份属性字典"放了** ✗ ⇒ 这正是"多放一份" ✓✓。

**② 与前面几轮合起来（链条闭合到最后一格 ✓）**：
- 死的是 **`EnumDict` 实例本身** ✓（第 116 轮：`EnumDict.__init__@10` ＝ 字节 20 ＝ `LOAD_SUPER_ATTR __init__` ✓）；
- 时机是它**自己的 `super().__init__()`** ✓；
- 现在是 **`attribute_clear`** 放的 ✓ ⇒ 即"**谁把命名空间错当成某个对象的属性字典**" ✓。
⇒ 而第 104 轮那条正相关（"实例化会把实例**也放进 `args[0]`**、`bound` 另有其一" ✓）在这条上**正好呼应**：
`super()`／实例化那条路上，**同一个对象被塞进两种角色**（接收者 ＋ 属性字典）✓ ⇒ 收尾时两份都被放 ✗。

**③ 下一轮（最后一格 ✓，判据明确 ✓）**：查**谁把命名空间设成了某个对象的属性/实例字典** ✓ ——
`TypeObject::set_dict`（类型字典 ✓）与 `take_instance_dict`／实例字典那两处 ✓
（`classes.rs` 里 `set_dict(Some(mapping))` ✓、`instance.rs` 里两处 ✓）⇒ 逐处确认"**谁持有、谁释放**" ✓。
**判据** ✓：改完后 `PYAWA_WATCH_DICT=1` 跑 `import enum` 时 **`attribute_clear` 不再出现在 `dict→0` 的回溯里** ✓，
且 `PYAWA_QUARANTINE=1` 不再报"incref 撞上已释放对象" ✓、普通档不回归 ✓。

**④ 闸门与数字** ✓（本轮只有受控探针 ✓）：`cargo check --workspace --all-targets` **0 警告** ✓、
`cargo test --workspace` ✓、`check.py` 12/12 ✓、`CX-8` ✓、夹具守卫 ✓、语料下限 182 ✓、逐字节 **4/4** ✓、
对拍普通趟 ✓（`共 182 ⇒ 通过 181 · 已知 0 · 新差异 1` ＝ 那条既有间歇缺陷 ✓）；上限 **162** ✓、
判据① **27.4%**（172 ÷ 628 ✓）。

#### 第 116 轮：🎯 **对到源码 + 找到嫌疑很大的一处** ✓ —— 死掉的是 **`EnumDict` 实例本身**，死在它自己 `super().__init__()` 那一刻 ✓；而 native 调用路径**释放 `args`、`bound_self` 却是借用** ✓

**① 按 `offset × 2` 对源码** ✓（第 115 轮定的方法 ✓）：
```
EnumDict.__init__ 字节 20  → 落在 18 **LOAD_SUPER_ATTR __init__**（就是 `super().__init__()` 那一句 ✓）
EnumType.__new__  字节 1042 → 落在 1036 CALL（`classdict = dict(classdict.items())` 那次 ✓）
EnumType.__new__  字节 1080 → 落在 1070 **LOAD_ATTR __dict__**（`enum_class.__dict__.update(...)` ✓）
```
⇒ 第 115 轮清单里那条 `EnumDict.__init__@10`（＝字节 20 ✓）掉的 `dict` **就是 `EnumDict` 实例本身** ✓
（`EnumDict` 是 `dict` 子类 ✓ ⇒ 类型正是 `dict` ✓）⇒ **它在自己 `super().__init__()` 那一刻被放到 0** ✗。

**② 顺着查调用机制，读到一处**契约不对等** ✓**（`executor` 里 native 那一支 ✓）：
```rust
let result = unsafe { slot(callable.as_ptr(), bound_self, &args, &kwargs, instance) };
for argument in args { release(instance, argument); }     // ← **释放整个 args**（按"持有"算 ✓）
```
而 `bound_self` 那一侧写明是**借用**（"调用方持有那份引用" ✓）✓
⇒ 也就是说：**`args` 被当成"调用方移交的所有权"，`bound_self` 被当成借用** ✓。
⇒ 那么**只要哪个调用方把"借用项"塞进 `args`** ✗，native 这一支就会**多放一份** ✓ ⇒ 对象被提前释放 ✓。
而第 104 轮刚查清的一条**正相关** ✓：本层**实例化会把实例也放进 `args[0]`**（`bound` 另有其一 ✓）
⇒ `super().__init__()` 走到 `dict.__init__`（第 104 轮新加的真实现 ✓）时，**接收者极可能同时在两处** ✓
⇒ 正是这一格 ✓（与"死的正是 `EnumDict` 实例、且死在它自己那次 `super()` 调用上"**完全吻合** ✓）。

**③ 下一轮（就一处，判据明确 ✓）**：核 `LOAD_SUPER_ATTR`／`CALL` 那条**调 native** 的路 ✓ ——
`args` 里那个接收者**是持有还是借用** ✓（若是借用 ⇒ 要么这里 incref ✓、要么 native 那一支不要释放它 ✓）。
**判据** ✓：改完后 `PYAWA_WATCH_DICT=1` 跑 `import enum` 时，**`EnumDict.__init__@10` 那条不再出现** ✓，
且 `PYAWA_QUARANTINE=1` 不再报"incref 撞上已释放对象" ✓（普通档同时不回归 ✓）。

**④ 闸门与数字** ✓（本轮无行为改动 ✓）：`cargo check --workspace --all-targets` **0 警告** ✓、
`cargo test --workspace` ✓、`check.py` 12/12 ✓、`CX-8` ✓、夹具守卫 ✓、语料下限 182 ✓、逐字节 **4/4** ✓、
对拍普通趟 ✓（`共 182 ⇒ 通过 181 · 已知 0 · 新差异 1` ＝ 那条既有间歇缺陷 ✓）；上限 **162** ✓、
判据① **27.4%**（172 ÷ 628 ✓）。

#### 第 115 轮：🎯 拿到**「dict 掉到 0」的现场清单** ✓ —— 全在 enum 的类创建路上 ✓（`EnumDict.__init__@10`、`EnumType.__new__@521`／`@540` ×2、`<module>@212`）

**① 按判据盯** ✓（照第 114 轮的纠正 ✓）：`PYAWA_WATCH_DICT=1` 时，**任何** `dict` 的引用计数
**掉到 0** 都报现场 ✓（不再盯一个地址 ✗）⇒ 一跑就把"死掉的命名空间"全列出来 ✓：
```
[dict→0] 0x…1c10 掉到 0：现场=EnumDict.__init__@10
[dict→0] 0x…1180 掉到 0：现场=<module>@212
[dict→0] 0x…3000 掉到 0：现场=EnumType.__new__@521
[dict→0] 0x…0790 掉到 0：现场=EnumType.__new__@540
[dict→0] 0x…32d0 掉到 0：现场=EnumType.__new__@540
[dict→0] 0x…30e0 掉到 0：现场=<module>@212
（共 8 条 ✓；这趟进程**没崩** ✗ —— 最后一个错误是普通的"下标赋值只接线了 list／dict" ✓
 ⇒ 再次印证第 113／114 轮那条"**对扰动敏感**" ✓）
```
⇒ **全部落在 enum 的类创建路上** ✓（`EnumDict.__init__` 建 `_member_names = {}` ✓、`EnumType.__new__` 的
`@521`／`@540` ✓、模块级 `@212` ✓）—— 与第 82 轮那个凶手栈（`build_class_native`／`run_class_body` ✓）
**同一片区** ✓。

**② 顺手发现的一处**必须记下**的方法问题** ✓：想把 `@521`／`@540` 对到**源码**时，用 CPython 的 `dis`
**对不上** ✗（`@540` 恰好撞上一条真指令 ✓ 是巧合 ✗，`@521`／`@10` 落在指令中间 ✗）
⇒ 说明**本层 `current_site()` 的偏移单位与 CPython 的字节偏移不是一回事** ✗
（`dispatch_raise` 里用的是 `instruction.offset * 2` ✓ ⇒ 本层偏好像是以**码元**计 ✓ hmm: 那 `@540`
按码元算 ＝ 字节 1080 ✓ —— 而字节 1080 落在 `LOAD_ATTR __dict__`（1070）与 `CALL`（1090）之间 ✓，
正是 `enum_class.__dict__.update(...)` 那一段 ✓✓！**与第 292 轮那条注记（"这里 `release` 会当场把类字典
打空 ⇒ 段错误" ✗）**完全对上** ✓）。
⇒ 所以下一轮对源码时，要按 **`offset × 2`** 换算 ✓（这是本轮**真正拿到**的定位线索 ✓）。

**③ 下一轮（两件，都具体 ✓）**：
1. 按 **`offset × 2`** 把 `EnumDict.__init__@10`（＝字节 20 ✓）与 `EnumType.__new__@521`（＝字节 1042 ✓）
   对到源码语句 ✓；
2. 重点看 `@540`（＝字节 1080 ✓，`enum_class.__dict__.update(...)` 那一段 ✓）——**类字典**在那里掉到 0 ✗，
   而第 292 轮已经在**另一条路**上见过同一症状（当时补了 `retain` ✓）⇒ 很可能是**同一类**的所有权缺口 ✓
   在**这条**路上还没补 ✓。

**④ 闸门与数字** ✓（本轮只有受控探针 ✓、不动行为 ✓）：`cargo check --workspace --all-targets` **0 警告** ✓、
`cargo test --workspace` ✓、`check.py` 12/12 ✓、`CX-8` ✓、夹具守卫 ✓、语料下限 182 ✓、逐字节 **4/4** ✓、
对拍普通趟 ✓（`共 182 ⇒ 通过 181 · 已知 0 · 新差异 1` ＝ 那条既有间歇缺陷 ✓）；上限 **162** ✓、
判据① **27.4%**（172 ÷ 628 ✓）。

#### 第 114 轮：🎯 **归因纠正 + 性质确认** ✓ —— 死在 `@540` 的是**另一个类**的命名空间 ✓（bug **按类重复发生** ✓）

**① 把观测与复现放在同一档** ✓（第 113 轮定的下一步 ✓）：`PYAWA_QUARANTINE=1` ＋ `PYAWA_NS_DEBUG=1`
**同一趟**跑 `import enum` ✓，于是地址稳定 ✓、计数轨迹可信 ✓：
```
[watch] … 被盯的命名空间 0x…85b0：rc 5→4→5→6→5→4→5→6→5→4→**3**（到 3 就停 ✓，**始终没归零** ✗）
[隔离区] incref 撞上**已释放对象** 0x…2ea100（原类型 dict，72 字节；释放于 EnumType.__new__@540）✗
```
⇒ **两个地址不是同一个** ✓ ⇒ 死在 `@540` 的是**另一个 `dict`**（同样 72 字节 ✓）⇒
`Lib/enum.py` 里 `EnumType.__new__` 要建**很多**类 ✓（`Enum`／`ReprEnum`／各 `IntEnum` 一族 ✓）
⇒ **这个是"按类重复发生"的** ✓ —— 我第 111 轮盯的是**第一个**命名空间 ✓，而**死在前面的是别的那个** ✓。

**② 归因纠正（第 112 轮那条否定在这里落定 ✓）**：第 107～111 轮把"释放现场"看成"**那个**命名空间" ✗
是不对的 ✓ —— 现场（`@540` ＝ `enum.py:511` 的 `classdict = dict(classdict.items())` ✓）**是对的** ✓，
但**对象是另一个类的命名空间** ✓。⇒ **结论仍然是"某个类的命名空间在 `@540` 被放到 0"** ✓，
只是**不是我盯的那个** ✓。

**③ 下一轮（方法已改对 ✓）**：把盯的**范围**从"一个地址"改成**"某一类对象"** ✓ ——
`类型=dict 且 72 字节`（或干脆"所有 `dict`" ✓）**每次 decref 都报现场与计数** ✓，
于是**第一个被打到 0 的那个**就会自己现形 ✓（带**现场** ✓ ⇒ 直接看那一句 ✓）。
（`watch` 那一格已经从"一个地址"扩成"一个判据"即可 ✓，改动很小 ✓。）

**④ 闸门与数字** ✓（本轮无行为改动 ✓）：`cargo check --workspace --all-targets` **0 警告** ✓、
`cargo test --workspace` ✓、`check.py` 12/12 ✓、`CX-8` ✓、夹具守卫 ✓、语料下限 182 ✓、逐字节 **4/4** ✓、
对拍普通趟 ✓（`共 182 ⇒ 通过 181 · 已知 0 · 新差异 1` ＝ 那条既有间歇缺陷 ✓）；上限 **162** ✓、
判据① **27.4%**（172 ÷ 628 ✓）。

#### 第 113 轮：把"计数与现场"做成**盯地址**的探针 ✓ —— 结果：**这个崩溃对扰动极敏感**（探针一开就躲掉 ✓）

**① 换观测方式** ✓（第 112 轮的教训：**别再靠地址归因** ✓）：给 `Instance` 加一格 `watch` ✓
（`PYAWA_NS_DEBUG=1` 时由建类那处**把命名空间地址记进去** ✓），此后对这**一个地址**的
**每次 incref／decref 都报"现场 ＋ 计数"** ✓ —— 读的是**计数与现场** ✓，不是"释放于何处" ✓。

**② 实测（本轮最有价值的一条 ✓）**：
```
[ns 探针] 交元类之前 namespace=0x…d1f0 rc=2
[watch] incref 0x…d1f0 → rc=6 现场=EnumType.__new__@212
[watch] decref 0x…d1f0 → rc=5 现场=EnumType.__new__@212
[watch] decref 0x…d1f0 → rc=4 现场=EnumType.__new__@224
[watch] incref 0x…d1f0 → rc=5 现场=EnumType.__new__@278
[watch] incref 0x…d1f0 → rc=6 现场=EnumType.__new__@279
[watch] decref 0x…d1f0 → rc=5 现场=EnumType.__new__@279
[watch] decref 0x…d1f0 → rc=4 现场=EnumType.__new__@290
[watch] decref 0x…d1f0 → rc=3 现场=EnumType.__new__@298
（到 rc=3 就停 ✓；进程**没有崩** ✓，最后是一个**普通的** Python 错误：
 "指令 38 尚未接线：下标赋值只接线了 list／dict" ✓）
```
⇒ **探针一开，崩溃就躲掉了** ✓ —— 与第 104／105 轮那条"**只在套件里闪**"（`class_keywords` ＋信号 11 ✓）
**同一性质** ✓：它是**时序／堆布局敏感**的缺陷 ✓（`eprintln!` 这点扰动就足以改变结果 ✓）。

**③ 这一轮的价值与边界（如实 ✓）**：
- **价值** ✓：①"盯地址"这套探针**可用且可靠** ✓（读计数与现场 ✓，不依赖地址归因 ✓）；
  ②把这条族**性质**定了：**不是必然的双重释放** ✗，而是**条件触发的**内存缺陷 ✓
  （前面几轮那几次"确定性"是在**毒化档**下 ✓ —— 毒化本身就改变了布局 ✓）。
- **边界** ✗：**还没修好** ✓ ⇒ 不能声称内存缺陷已定位到某一行 ✓；本轮**无行为改动** ✓（只有受控探针 ✓）。

**④ 下一轮（换"不扰动"的观测 ✓）**：把观测**挪到进程外／事后** ✓ —— 用 `PYAWA_QUARANTINE=1`
（它**本来就**会改布局 ✓，是"能稳定复现"的那一档 ✓）＋ `unlink`／`quarantine_put` 里的**现场** ✓，
或者干脆按"**在毒化档下稳定复现**"来做修复与验证 ✓（判据：毒化档不再报"incref 撞上已释放对象" ✓，
且普通档不回归 ✓）。这样"观测"与"复现条件"就不互相打架 ✓。

**⑤ 闸门与数字** ✓：`cargo check --workspace --all-targets` **0 警告** ✓、`cargo test --workspace` ✓、
`check.py` 12/12 ✓、`CX-8` ✓、夹具守卫 ✓、语料下限 182 ✓、逐字节 **4/4** ✓、对拍普通／`DANGLING` ✓；
上限 **162** ✓、判据① **27.4%**（172 ÷ 628 ✓）。

#### 第 112 轮：⚠️ 一个**否定** ✓ —— 那条 `@540` 的归因可能**又是地址复用**（该地址在非隔离路上从未被释放 ✓）

**① 释放探针（`PYAWA_FREE_DEBUG=1` ✓，加在 `unlink` 里 ✓）**：每次真正摘除对象都报
**地址 ＋ 类型 ＋ Python 现场 ＋ Rust 回溯** ✓。跑 `import enum` 并与命名空间地址**对号** ✓：
```
[ns 探针] 交元类之前 namespace=0x61d8d5b97060 rc=2      ← 探针给出的地址 ✓
[free 探针] …… 共 1328 次 ✓（进程随后 abort ✓）
但：**该地址在 1328 条 free 记录里一次都没出现** ✗
```
⇒ 在**非隔离**这条路上，那个命名空间**从来没有被释放过** ✗ ⇒ 第 107～111 轮把"释放现场"归到
`EnumType.__new__@540` 的那条推断 **站不住** ✗（很可能又是**地址复用**：隔离档虽然**扣住**对象 ✓，
但 `quarantine_check` 之后**仍会放出**它们 ✓ ⇒ 地址会被后来者复用 ✓ ⇒ "释放于某处"就可能指到
**另一个**对象 ✓ —— 与第 92 轮那次假阳性**同一类** ✗）。

**② 这一轮的价值** ✓：把"**不要再用地址归因**"这条教训**再钉一次** ✓（第 92 轮已钉过一次 ✓），
并明确下一步该怎么做 ✓：**在隔离档的登记点（`quarantine_put`）加探针** ✓ —— 那里记的是
"**这次是哪个现场把它记成已释放**" ✓，而隔离档里对象**不被真释放** ✓ ⇒ 地址**稳定** ✓
⇒ 归因才可信 ✓（这才是"哪一次释放把它记成零"的正确观测点 ✓）。

**③ 下一轮（观测点已选定 ✓）**：
1. 在 `quarantine_put` 加探针 ✓（地址 ＋ 类型 ＋ 现场 ＋ 回溯 ✓），跑 `import enum` ✓：
   看那个命名空间**是否真的被记过一次** ✓；
2. 若记过 ⇒ 再回到 `unlink` 那一路看**计数为什么到 0** ✓（`rc=2` 进去、内部归零 ✗ —— 这一条**是**
   可靠的 ✓，因为它是**计数读数**不是地址归因 ✓）；
3. 若没记过 ⇒ 那"已释放"的判断本身就是错的 ✓ ⇒ 病灶另找 ✓（但 `rc=2 → 0` 这条仍然要解释 ✓）。

**④ 闸门与数字** ✓（本轮只有受控探针 ✓、不动行为 ✓）：`cargo check --workspace --all-targets` **0 警告** ✓、
`cargo test --workspace` ✓、`check.py` 12/12 ✓、`CX-8` ✓、夹具守卫 ✓、语料下限 182 ✓、逐字节 **4/4** ✓、
对拍普通趟 ✓（`共 182 ⇒ 通过 181 · 已知 0 · 新差异 1` ＝ 那条既有间歇缺陷 ✓）；上限 **162** ✓、
判据① **27.4%**（172 ÷ 628 ✓）。

#### 第 111 轮：命名空间**交进元类时 `rc=2`** ✓，却在**调用内部**被放到 0 ✗ ⇒ **双重释放** ✓

**① 探针（照第 92 轮那招 ✓）**：在 `build_class_native` 里对命名空间读**引用计数** ✓（`PYAWA_NS_DEBUG=1` ✓）：
```
[ns 探针] 交元类之前 namespace=0x…5b0 rc=2        ← 有**两份**（我方那份 ＋ 调用机制给帧的那份 ✓）
（"元类返回之后"那一行**没有打印** ✗ ⇒ 进程在**调用内部**就崩了 ✓）
```
⇒ 也就是说：进去时 **2** ✓，出来前就归零 ✗ ⇒ **在 `EnumType.__new__` 内部被释放了两次** ✓
（一次是 `classdict` 参数被重绑时放掉帧那份 ✓ —— 那是**正当**的一次 ✓；另一次 ✗ 就是病灶 ✓）。

**② 与已有证据链合起来 ✓**（这条链现在很完整 ✓）：
- 毒化档：释放现场 `EnumType.__new__@540` ✓ ＝ `enum.py:511` 的 `classdict = dict(classdict.items())` ✓；
- Rust 栈：撞上的那次 incref 在 `executor::push` ✓，帧里还看得见 `classes::build_class_native` ✓；
- 本条：进去时 **2**、内部归零 ✗ ⇒ **两次释放** ✓。
⇒ 结论：**我们的 VM 在"参数被重绑"这件事上放多了** ✓（正当那一次是帧自己的那份 ✓；
多出来的那一次多半是**同一次重绑被记了两遍** ✗，或**参数与局部各放一次** ✗）。

**③ 下一轮（方法已备好 ✓）**：在 `unlink`／释放路径上，对**这个地址**逐次打印"第几次释放 ＋ Rust 回溯" ✓
（`PYAWA_QUARANTINE=1` 已经会记录释放现场 ✓，再加"计数 ＋ 调用栈"就能看出**哪两次** ✓）：
- 若两次都来自**同一条指令**（`STORE_FAST` 的重绑 ✓）⇒ 是**重绑实现**放了两次 ✓；
- 若一次来自重绑、一次来自**调用收尾** ✓ ⇒ 是**实参表的收尾**多放一份 ✓。
两者修法不同 ✓，一次读数就能分开 ✓。

**④ 闸门与数字** ✓（本轮只有受控探针 ✓、不动行为 ✓）：`cargo check --workspace --all-targets` **0 警告** ✓、
`cargo test --workspace` ✓、`check.py` 12/12 ✓、`CX-8` ✓、夹具守卫 ✓、语料下限 182 ✓、逐字节 **4/4** ✓、
对拍普通趟 ✓（`共 182 ⇒ 通过 181 · 已知 0 · 新差异 1` ＝ 那条既有间歇缺陷 ✓）；上限 **162** ✓、
判据① **27.4%**（172 ÷ 628 ✓）。

#### 第 110 轮：调用机制**是平的** ✓ ⇒ 少的那一份不在 `call_value` 里 ✗；下一轮改用"引用计数前后对照" ✓

**① 先把调用约定查清** ✓（不猜 ✓）：
- `bind_arguments` 按**值**收 `Vec` ✓（所有权转给帧 ✓），注释写明返回值是"**新引用**" ✓；
- `call_value` ✓ **对每个实参都 incref** ✓（`for argument in args { incref_object(...) }` ✓，
  kwargs 的键值也各一份 ✓）⇒ 被调帧因此**自己持有** ✓，调用方那份**不受影响** ✓。
⇒ 所以 `M.__new__(M, name, bases, namespace)` 这条路上，**命名空间不会因为被调方重绑参数就被打掉** ✗
—— **这条路是平的** ✓（如实 ✓：上一轮我把"调用方那份被打掉"当成结论 ✗，这里**否掉** ✓）。

**② 那诊断说明什么** ✓：毒化档报"释放于 `EnumType.__new__@540`" ✓（＝ `classdict = dict(classdict.items())` ✓
那句重绑 ✓）⇒ 重绑时**帧自己那份**被放掉 ✓ ⇒ 若那一刻它的计数**降到 0** ✗，就说明**在重绑之前**，
除了帧那一份**已经没有别人持有它**了 ✗ ⇒ **调用方那份早就没了** ✗（在调用**之前**或**之中**被放掉 ✓）。
⇒ 目标从"调用机制"挪到"**`build_class_native` 里这个名字空间引用在调用前后的去向**" ✓。

**③ 下一轮（方法照第 92 轮那招 ✓）**：在 `build_class_native` 里给命名空间加一发**引用计数探针** ✓
（调用前读一次 ✓、`M.__new__` 返回后再读一次 ✓、交给 `build_class_from_parts` 前再读一次 ✓ ✓）：
- 若**调用前就已低于应有值** ✗ ⇒ 病灶在更早（类体跑完到调用之间 ✓）；
- 若**调用后少了** ✗ ⇒ 病灶在被调方收尾或我们的收尾 ✓。
两次读数就能分开 ✓（与第 92 轮"造好字典 rc=1"同一手法 ✓，那一次正是靠它发现了假阳性 ✓）。

**④ 闸门与数字** ✓（本轮无行为改动 ✓）：`cargo check --workspace --all-targets` **0 警告** ✓、
`cargo test --workspace` ✓、`check.py` 12/12 ✓、`CX-8` ✓、夹具守卫 ✓、语料下限 182 ✓、逐字节 **4/4** ✓、
对拍普通趟 ✓（`共 182 ⇒ 通过 181 · 已知 0 · 新差异 1` ＝ 那条既有间歇缺陷 ✓）；上限 **162** ✓、
判据① **27.4%**（172 ÷ 628 ✓）。

#### 第 109 轮：🎯 **`@540` 对到了具体指令** ✓ —— `STORE_FAST classdict`（**重绑参数**）⇒ 对象在那一刻降到零 ✗

**① 怎么对的** ✓（换了个更快的办法 ✓）：不再去修布局工具的抽取 ✗，改用 **CPython 的 `dis`** ✓
看 `Lib/enum.py` 的 `EnumType.__new__`（共 **640** 条指令 ✓）在**同一偏移**上是哪条 ✓：
```
 502 LOAD_FAST_BORROW  classdict
 504 LOAD_ATTR          items + NULL|self
 524 CALL
 532 CALL
 540 STORE_FAST        classdict        ← **就是它**（我们报的释放现场 ✓）
 542 LOAD_FAST_BORROW  _gnv
 550 LOAD_FAST_BORROW_LOAD_FAST_BORROW  _gnv, classdict
 554 STORE_SUBSCR                       ← 紧跟着又往 classdict 里写
```
⇒ `EnumType.__new__` 里那两步是"**先重绑参数 `classdict`** ✓，紧接着再往（新的）`classdict` 里塞
`_generate_next_value_`" ✓ —— 而我们的诊断说：**对象在 `STORE_FAST` 那一刻被释放** ✓
⇒ 就是**重绑参数**把旧的命名空间字典降到了零 ✓。

**② 由此得到的因果链（与栈完全吻合 ✓）**：
```
EnumType.__new__(metacls, cls, bases, classdict, ...)   ← 我们（build_class_native）把命名空间交进去
    … classdict = <新字典>（STORE_FAST @540）⇒ 旧的降到零 ✗
    … 随后 VM 再往旧的上面写／取值 ⇒ push 时 incref 撞上已释放对象 ✗
```
栈里出现 `classes::build_class_native` ✓、崩在 `executor::push` ✓ —— 与"**调用方本该还握着那份命名空间，
却没有**"完全吻合 ✓（第 104 轮刚查清的另一条正相关 ✓：实例化会把实例**也放进 `args[0]`**、
`bound` 另有其一 ✓ ⇒ 实参表的所有权本来就容易**少一份** ✓）。

**②′ 再确认一步（本轮顺手 ✓）**：`Lib/enum.py:511` 正是 **`classdict = dict(classdict.items())`** ✓
—— 与 `@540` 那条 `STORE_FAST classdict` **完全对上** ✓。再看我们这侧 `classes.rs` 里把命名空间交给元类那段 ✓：
它**已经**记着同一类问题（第 292 轮 ✓）："`build_class_from_parts` 会吃掉调用方那一份 ✓ … 这里先**自己再留一份** ✓，
`__init__` 用完交还 ✓（先前直接用原来那份 ✗ ⇒ '对已释放对象 incref' ⇒ 堆崩 `malloc(): unaligned tcache chunk` ✗）" ✓
—— 也就是说**同一类**的事**为 `__init__` 那条路补过** ✓，而
**`M.__new__(M, name, bases, namespace)` 这条路没有额外留一份** ✓（注释写"这条路不吃命名空间 ⇒ 调用方那一份留着不动 ✓"）。
⇒ 与"**被调方重绑参数（`enum.py:511`）就把调用方那份打掉**"完全吻合 ✓ ⇒ 这就是要动的那一处 ✓。

**③ 下一轮（就一处 ✓，且**两种改法**要按调用约定选 ✓）**：查 `build_class_native` 里把**命名空间**交给元类
（`M.__new__(M, name, bases, namespace)` ✓）那一步的**所有权** ✓ —— 参照的口径是"调用时实参表**持有**一份" ✓
（被调方可以放心重绑 ✓）；我们这边要么**少加了一份** ✓、要么**调用收尾多放了一份** ✓。
**判据** ✓：改完后 `PYAWA_QUARANTINE=1` 跑 `import enum` **不再报**"incref 撞上已释放对象" ✓，
且 `import enum` 继续往下走 ✓。

**④ 闸门与数字** ✓（本轮无行为改动 ✓）：`cargo check --workspace --all-targets` **0 警告** ✓、
`cargo test --workspace` ✓、`check.py` 12/12 ✓、`CX-8` ✓、夹具守卫 ✓、语料下限 182 ✓、逐字节 **4/4** ✓、
对拍普通趟 ✓（`共 182 ⇒ 通过 181 · 已知 0 · 新差异 1` ＝ 那条既有间歇缺陷 ✓）；上限 **162** ✓、
判据① **27.4%**（172 ÷ 628 ✓）。

#### 第 108 轮：把那条内存缺陷**再钉一层** ✓ —— 撞上已释放对象的那次 incref 是 `executor::push` ✓（在 `EnumType.__new__` 里）

**① 让毒化档的命中带出**我们自己的 Rust 栈** ✓**（`RUST_BACKTRACE=1` ＋ `PYAWA_QUARANTINE=1` ✓）：
```
[隔离区] incref 撞上**已释放对象** 0x…（原类型 dict，72 字节；释放于 EnumType.__new__@540）✗
  2: <Header>::incref
  3: Instance::incref_object
  4: Instance::own
  5: pyawa_core::executor::push                     ← 往**值栈**压一个值
  6: executor::execute::{closure#1}
  8: executor::call_callable
  9: executor::call_value
 10: classes::build_class_native                    ← 类创建
 14: executor::load_module
```
⇒ 结论 ✓：**VM 把一个"已经被释放的 dict"压上了值栈** ✓，而它的释放现场是**同一个函数** `EnumType.__new__@540` ✓
⇒ 也就是说：`@540` 那一步**多减了一份**（函数里的局部／栈槽还有人持有它 ✗）。

**② 顺手排掉一个方向** ✓（如实 ✓）：3.14 的 **`LOAD_FAST_BORROW`** 我们和 `LOAD_FAST` **走同一条路** ✓
（`push` ⇒ `own` ⇒ incref ✓，即"栈上持有" ✓）—— 这只会**多留一份**（泄漏 ✗），方向是"**多**" ✓，
与观察到的"**提前释放**"**相反** ✗ ⇒ 所以病灶**不是**它 ✓（但这条口径差异记下来 ✓，将来量内存时要看 ✓）。

**③ 下一轮（方法已定 ✓）**：把 **byte 540** 对到**具体那条语句** ✓ —— 用布局工具 dump
`EnumType.__new__` 的码元 ✓（上一轮我的匹配式写歪了 ✗，没取到那几行 ✓，本轮已把方法单独抽成
`target/enew.py` ✓ 备用 ✓），然后拿那条指令对照本层"**谁在释放**"的实现 ✓：
- 若是 **`STORE_*`／`POP_TOP`／`CALL` 收尾**在同一对象上放了两遍 ✗ ⇒ 就是它 ✓；
- 若是**某个原生方法**（如 `dict.setdefault`／`dict.pop` 一类 ✓）多放了一份 ✓ ⇒ 也一目了然 ✓
  （栈里第 5 帧是 `push` ✓，说明崩在**取值／压栈**那一步 ✓，释放点在它之前 ✓）。

**④ 闸门与数字** ✓（本轮无行为改动 ✓）：`cargo check --workspace --all-targets` **0 警告** ✓、
`cargo test --workspace` ✓、`check.py` 12/12 ✓、`CX-8` ✓、夹具守卫 ✓、语料下限 182 ✓、逐字节 **4/4** ✓、
对拍普通趟 ✓（`共 182 ⇒ 通过 181 · 已知 0 · 新差异 1` ＝ 那条既有间歇缺陷 ✓）；上限 **162** ✓、
判据① **27.4%**（172 ÷ 628 ✓）。

#### 第 107 轮：🎯 **底层内存缺陷报出了名字与现场** ✓ —— `EnumType.__new__@540` 处**多放了一份 `dict`** ✓

**① 追法（承上一轮 ✓）**：族里第一句错又回到 `-6` ✓ ⇒ 先看崩溃消息 ✓：
```
tcache_thread_shutdown(): unaligned tcache chunk detected      ← glibc 在**线程退出**时才发现的堆破坏 ✓
```
⇒ glibc 的检查档（`MALLOC_CHECK_=3` ＋ `MALLOC_PERTURB_` ✓）**没能在更早处**抓住 ✗ ⇒ 换**本层的毒化档** ✓
（`PYAWA_QUARANTINE=1` ✓：释放后把载荷毒化成 `0xDE` ✓，并在 **incref／decref** 撞上已释放对象时**报名字** ✓）：
```
[隔离区] incref 撞上**已释放对象** 0x…（原类型 dict，72 字节；释放于 EnumType.__new__@540）
         ⇒ 提前释放／多放一份 ✗；当前帧：EnumType.__new__
```
**两次连跑都给同一处** ✓（确定性 ✓）⇒ 结论确凿 ✓：**`Lib/enum.py` 的 `EnumType.__new__` 那条路上
多放了一份 `dict` 引用** ✓（对象类型 `dict`、72 字节 ✓；释放现场 `@540` ✓），随后有人 incref 就撞上 ⇒
堆被写坏 ⇒ 线程退出时 glibc 报 `unaligned tcache chunk` ✓。

**② 这一条为什么关键** ✓：它是这十几轮所有"崩溃／间歇／`RefCell already borrowed`"的**共同底层** ✓
（第 70 轮起登记的那条 ✓），而这次是**第一次**拿到"**哪个函数、哪条指令、什么类型、多大**" ✓
⇒ 下一轮就能照现场查那一句的所有权 ✓（`EnumType.__new__` 里 `@540` 附近 ✓）。

**③ 下一轮的抓手（已具名 ✓）**：
1. 看 `Lib/enum.py` 的 `EnumType.__new__` 在 `@540` 附近对那个 `dict`（**多半是 `classdict`／命名空间** ✓）
   做了什么 ✓；再沿**我们的调用约定**查谁多放了一份 ✓ —— 第 104 轮刚查清的一条正相关 ✓：
   **实例化会把实例也放进 `args[0]`（`bound` 另有其一）** ✓ ⇒ `__new__`／`__init__` 的实参表里
   **同一对象出现两次** ✓，谁在收尾时把两份都放了 ✗ 就会多放一份 ✓；
2. 修好之后复测上限与判据① ✓（上一轮上限已 161 → **162** ✓）。

**④ 闸门与数字** ✓（本轮无行为改动 ✓，只有诊断与台账 ✓）：`cargo check --workspace --all-targets`
**0 警告** ✓、`cargo test --workspace` ✓、`check.py` 12/12 ✓、`CX-8` ✓、夹具守卫 ✓、语料下限 182 ✓、
逐字节 **4/4** ✓、对拍普通趟 ✓（`共 182 ⇒ 通过 181 · 已知 0 · 新差异 1` ＝ 那条既有间歇缺陷 ✓）；
上限 **162** ✓、判据① **27.4%**（172 ÷ 628 ✓）。

#### 第 106 轮：🎯 **`super().<方法>()` 的绑定修好了** ✓ —— 族里 119 个模块的第一句错消失 ✓，`import enum` 明显前进 ✓

**① 根因（上一轮的现场 + 本轮读码 ✓）**：`super_lookup` 在 MRO 上找到方法后**只在类型恰好是
`function` 时**才绑到 `__self__` ✓：
```rust
if instance.type_of(found) == builtin_type(instance, "function") { return Ok(Some(Attribute::Method{..})) }
return Ok(Some(Attribute::Value(found)));        // ← 原生方法走这条 ⇒ **一个 self 都没有** ✗
```
而**原生方法**（`builtin_function_or_method` ✓，例如 `dict.__init__` ✓）是**需要接收者**的 ✓
⇒ `super().__init__()` 落到原生那侧报 `descriptor needs an argument` ✓（现场实测 `EnumDict.__init__@21` ✓）。

**② 修法** ✓（一处 ✓）：两族都绑 ✓ —— `function` **或** `builtin_function_or_method` ⇒ 交
`Attribute::Method { function, this }` ✓（与实例方法同一条路 ✓）。

**③ 实测** ✓：
```
两行复现（class D(dict): super().__init__()）  → 0 / 1 ✓（与参照逐字同 ✓）
内建子类探针（dict/list/set 子类）              → 逐字同 ✓
import enum                                   → **过了这一格** ✓，改撞「下标赋值只接线了 list／dict」✗
```
⇒ 族里 **119** 个模块的**第一句错**（`descriptor needs an argument`）消失 ✓。
**上限也涨了** ✓：**161 → 162** ✓（这条链上**第三次**上涨 ✓）。
**如实补一句** ✗：随后测上限时，那一族的**第一句错又变回 `子进程退出码 -6`**（信号 6 ✓）——
因为绑定修好后 `EnumDict` **跑得更远** ✓，撞上了后一层的**内存缺陷** ✓（原来那个错只是**挡在前面** ✓）。
⇒ 这一修是**真进步**（上限 +1 ✓），但**没有**消掉那条族 ✓；族里现在第一句错的形态回到了崩溃 ✓。

**④ 下一轮的抓手（据实测更正后 ✓）**：那两条"下标"报文是**直接构造的 `ExecError::Unsupported`** ✗
（不经 `raise_builtin_error` ✓ ⇒ 我这一轮的现场探针没响 ✓，如实记 ✓）⇒ 下一轮把现场印在
`Unsupported` 的构造点 ✓（顺带看看是不是"子类覆盖了 `__setitem__`"那条路 ✓ —— `EnumDict` 正是覆盖了 ✓）。

**⑤ 闸门与数字** ✓：`cargo check --workspace --all-targets` **0 警告** ✓、`cargo test --workspace` ✓、
`check.py` 12/12 ✓、`CX-8` ✓、夹具守卫 ✓、语料下限 182 ✓、逐字节 **4/4** ✓、对拍普通趟 ✓
（`共 182 ⇒ 通过 181 · 已知 0 · 新差异 1` ＝ 那条既有间歇缺陷 ✓）；
上限 **161** ✓、判据① **27.4%**（172 ÷ 628 ✓，下一轮复测 ✓）。

#### 第 105 轮：把两条**残留**分别钉住 ✓ —— ① `descriptor needs an argument` 的现场 = `super().__init__()` ✓；② 闸门闪的是 `class_keywords` vs `method_defaults` ✓

**① 优先排"唯一还在闪的闸门"** ✓（第 104 轮列的下一轮第一项 ✓）：
- **独立跑 10 次全过** ✓（`target/.../class_keywords.py` ✓ 退出码全 0 ✓）⇒ 这条要**套件上下文**才闪 ✓
  （与"内存布局／压力"有关 ✓，和前面几轮那些"只在套件里出现"的现象同一类 ✓）；
- **毒化档（`PYAWA_QUARANTINE=1`）连跑三次** ✓：**确定性**地变成**另一个用例** ✗ ——
  `method_defaults` ＋ `RefCell already borrowed` ✓（**就是本会话开头那条老缺陷** ✓，自第 347 轮起登记 ✓）。
⇒ 两条残留**分清了** ✓：**普通**跑：`class_keywords` ＋ **信号 11**（间歇 ✓）；
  **毒化**跑：`method_defaults` ＋ `RefCell already borrowed`（确定 ✓）。都是既有缺陷 ✓，非本地改动引进 ✓。

**② 更值钱的那件（`descriptor needs an argument` ✓，族里 119 个模块的第一句错 ✓）**：
这条报文有 **8 处以上**出处 ✗ ⇒ 给它临时加上**现场**（`current_site()` ✓）一次读出 ✓：
```
TypeError: descriptor needs an argument（现场：EnumDict.__init__@21）
```
⇒ 就是 `EnumDict.__init__` 里那句 **`super().__init__()`** 的 `CALL` ✓（码元：`0 RESUME／1 LOAD_GLOBAL／
6 CALL／10 LOAD_ATTR／20 PUSH_NULL／**21 CALL**` ✓）。⇒ 结论 ✓：**`super().<方法>()` 没有把 `self`
绑给原生方法** ✗ —— 我第 104 轮新加的 `dict.__init__` 需要 `self`（`bound` 或 `args[0]` ✓），
而 `super()` 这条路**一个都没给** ✗ ⇒ 于是报"需要实参" ✓。
（探针已撤 ✓，现场写进台账 ✓。）

**③ 下一轮（两件，都定位到了 ✓）**：
1. **`super()` 的绑定** ✓：`super_lookup`／调用那一侧要把 `__self__` 作为**绑定接收者**交出去 ✓
   （与普通方法调用同一条路 ✓）；这一处修好，`EnumDict`／`Enum` 就能继续往下走 ✓，
   而那 **119** 个模块的第一句错就消失了 ✓；
2. 之后再看上限与判据① 是否跟着动 ✓。

**④ 闸门与数字** ✓（以本轮未改动行为为准 ✓）：`cargo check --workspace --all-targets` **0 警告** ✓、
`cargo test --workspace` ✓、`check.py` 12/12 ✓、`CX-8` ✓、夹具守卫 ✓、语料下限 182 ✓、逐字节 **4/4** ✓；
对拍：`计数：共 182 ⇒ 通过 181 · 已知差异 0 · 新差异 1` ✓（差异就是上面那条 ✓）；
上限 **161** ✓、判据① **27.4%**（172 ÷ 628 ✓）。

#### 第 104 轮：🎉 **内建子类真的能用了** ✓（`dict`／`list`／`set` 子类全通 ✓）—— 上限 **160 → 161** ✓

**① 这一轮把"子类"这条线一次做到底** ✓（前十轮攒下的同一形状 ✓）：
- **布局基类**扩到 `list`／`tuple`／`set`／`frozenset`／`deque` ✓（判据不变 ✓：**自带 `new` 槽** ✓
  ⇒ 载荷由它自己建 ✓）；第 99 轮只纳了 `dict` ✓；
- **`instance` 侧四处**（`tuple`／`list`／`dict`／`set` 的 entries 助手 ✓）从**精确类型**改**子类型** ✓
  —— 其中 `dict_entries` 就是 `TypeError: not a dict`（族里 119 个模块的第一句错 ✓）的来源 ✓；
- **`length_of`** 六个容器分支全部改子类型 ✓（先前只有 `dict` 那一支 ✓）；
- **下标**（读／写／删）12 处 `container_type == builtin_type(...)` 全部改子类型 ✓；
- **`dict.__init__` 真实现** ✓（源可以是**映射**或**成对的可迭代** ✓）：第 99 轮让 `dict.__new__` 只建空映射 ✓，
  而 `dict` **没有自己的 `__init__`** ✗（继承 `object` 的空操作 ✗）⇒ `D([("a", 1)])` 会**静默**给空字典 ✗
  —— 探针当场抓到 ✓（**静默错比报错更糟** ✓，所以当场补实现而不是放过 ✓）；
  取 `self` 按**本层约定**（实例化会把实例**也放进 `args[0]`** ✓，`bound` 另有其一 ✓）：两条都认 ✓。

**② 实测** ✓（8 行探针**两侧逐字同** ✓）：`dict` 子类 `keys()`／`values()`／`len`／
**`D([("a", 1)])`** ✓；`list` 子类 `len`／下标 ✓；`set` 子类 `len` ✓。

**③ 上限** ✓：**160 → 161** ✓（这是那一族链上**第二次**上涨 ✓）；族仍然 **119**
（现在第一句错是 `TypeError: descriptor needs an argument` ✓ —— 又一个**普通**错误 ✓，比"内存崩溃"远得多 ✓）。

**④ 闸门** ✓：`cargo check --workspace --all-targets` **0 警告** ✓、`cargo test --workspace` ✓、
`check.py` 12/12 ✓、`CX-8` ✓、夹具守卫 ✓、语料下限 182 ✓、逐字节 **4/4** ✓。
**如实记一条** ✓：本轮第一次跑对拍时**两趟都红** ✗，落点是 `class_keywords` ＋**信号 11** ✓
—— 正是第 347 轮起那条**既有、间歇**缺陷 ✓；**连跑三次全绿** ✓ ⇒ 与本轮改动无关 ✓
（但这条已经很值得**专门修一次** ✓ —— 它是唯一还在闪的闸门 ✓）。

**④′ 收尾复验（如实补记 ✓）**：`tests/ci/stability.py` 这一趟**红** ✗ —— 它连跑三遍整套（含对拍 ✓），
落点仍是 `class_keywords` ＋**信号 11** ✓（对拍单跑时 `计数：共 182 ⇒ 通过 181 · 已知差异 0 ·
新差异 1` ✓，差异用例就是它 ✓）。⇒ **这条既有缺陷最近出现得更勤** ✓（几轮前是"偶尔" ✓），
它现在是**唯一还在闪的闸门** ✓ ⇒ 下一轮**优先**排它 ✓（第 347 轮起登记到现在 ✓，值得专门一轮 ✓）。

**⑤ 下一轮** ✓（两件 ✓）：① 查 `TypeError: descriptor needs an argument` 是哪个 native ✓
（多半是某个描述符式方法没认"`args[0]` 里也带着实例"这条约定 ✓ —— 与 `dict_init` 同一形状 ✓）；
② 顺带把那条**间歇闪烁**的闸门（`class_keywords` ＋信号 11 ✓）排一处 ✓。

#### 第 103 轮：真假判定补齐 ✓（子类型 ＋ `set` 一族 ＋ `__bool__`／`__len__` 协议 ✓）—— 族前进到 `not a dict` ✓

**① 动手** ✓（`truthiness` 一处 ✓）：
- 六处内建判定（`str`／`bytes`／`list`／`tuple`／`dict`／`float` ✓）从**精确类型**改成**子类型** ✓
  （与第 99／101 轮同一形状 ✓）；
- 新增 **`set`／`frozenset`**：非空为真 ✓（族里第一句错就是 `if some_set:` ✓）；
- **协议兜底** ✓：先 `__bool__`（用它的返回值判真值 ✓）、再 `__len__`（非零为真 ✓）、
  两者都没有 ⇒ **默认为真** ✓（照参照 ✓）—— 先前这里**直接报 `Unsupported`** ✗。

**② 探针当场抓到一处错 ✗（如实 ✓）**：`bool(WithLen(0))` 我们错成 `True` ✓ —— 因为我在 `__len__` 那支
拿 `length_of` 去读它的**返回值** ✗，而返回值是**整数**、`length_of` 对它给 `None` ⇒ 兜成"真" ✓
⇒ 改成**按整数判零** ✓（`int_of(..).is_zero()` ✓）。改完 11 行探针**两侧逐字同** ✓
（`set()`／`{1}`／`frozenset` ✓、`WithLen(0)` ⇒ `False` ✓、`WithBool()` ⇒ `False` ✓、裸对象 ⇒ `True` ✓）。

**③ 上限榜：族再前进一格 ✓**：
```
上一格：119  真假判定只接线了 …（set 一族与 __bool__ 协议未接线）
这一格：119  TypeError: not a dict          ← **普通类型错误** ✓（不再是"未接线" ✓）
上限：  160 ✓（上一轮 159 → 160 ✓）
```

**④ 闸门** ✓：`cargo check --workspace --all-targets` **0 警告** ✓、`cargo test --workspace` ✓、
`check.py` 12/12 ✓、`CX-8` ✓、夹具守卫 ✓、语料下限 182 ✓、逐字节 **4/4** ✓、
对拍**普通与 `DANGLING` 两趟全绿** ✓；上限 **160** ✓、判据① 上测 **27.4%**（172 ÷ 628 ✓，下一轮复测 ✓）。

**⑤ 下一轮（一小步 ✓）**：查 **`TypeError: not a dict`** 从哪来 ✓ —— 多半是某处把"映射"按**精确类型**
校验 ✓（与前十轮同一形状 ✓：`dict(...)`／`dict.update`／`__prepare__` 那条 ✓），改成**子类型**即可 ✓。

#### 第 102 轮：🎯 集合四运算符接上 ✓ —— **上限第一次动了**（159 → **160** ✓），族前进到"`set` 的真假判定" ✓

**① 动手** ✓：`&`／`|`／`-`／`^` 先前一律落到"一处真相"的 `unsupported_operand` ✗
⇒ 在 `arithmetic_public` 里**先认集合**（子类型判定 ✓，`set`／`frozenset` 都算 ✓），走新的
`set_operation` ✓：认元素用**值相等**（与 `in` 同一口径 ✓）、结果给**新 `set`** ✓、
每个元素先 `incref` 一份交给 `new_set` ✓（`OM-16` ✓）。
（落地时踩了一个编译口 ✓：`values_equal` 是**自由函数**不是 `Instance` 的方法 ✓ —— 当场改对 ✓。）

**② 实测** ✓（6 行探针**两侧逐字同** ✓）：`a & b` → `[2, 3]` ✓、`a | b` → `[1, 2, 3, 4]` ✓、
`a - b` → `[1]` ✓、`a ^ b` → `[1, 4]` ✓、`a & {7}` → `[]` ✓、`len(a | b)` → `4` ✓。

**③ 上限榜：族再前进一格 ✓，而且**上限动了** ✓**：
```
上一格：119  下标只接线了 tuple／list／dict／str／bytes
这一格：119  真假判定只接线了 …（`set` 一族与 `__bool__` 协议未接线）✓
上限：  159 → **160** ✓   ← 这一条链上**第一次**看到"能 import 的数"增加 ✓
```
⇒ 那 **119** 个模块（`argparse`／`asyncio`／`textwrap`／`_markupbase` … ✓）现在只差
**`set` 的真假判定** ✓（`if some_set:` ✓）—— 一步小活 ✓。

**④ 闸门** ✓：`cargo test --workspace` ✓、0 警告 ✓、`check.py` 12/12 ✓、`CX-8` ✓、夹具守卫 ✓、
语料下限 182 ✓、逐字节 **4/4** ✓、对拍普通趟 ✓（`DANGLING` 那趟红 ✗ ＝ 那条既有、间歇缺陷 ✓）；
判据① 上一测 **27.4%**（172 ÷ 628 ✓，上限一变 ⇒ **下一轮复测** ✓）。

**⑤ 收尾时的一个**闸门细节** ✓（如实记 ✓）**：这一轮的代码落地后，`cargo check --workspace --all-targets`
报了 **1 条**警告 ✗（`set_operation` 里的闭包多了个 `mut` ✓）—— 而"**0 警告**"是本项目的硬闸门 ✓
⇒ 当场去掉 ✓、复验 **0** ✓，并把集合运算与逐字节对拍重跑一遍确认没动到别的 ✓。
（教训：**提交前那条 `cargo check` 要真看数** ✓ —— 本轮差点让一条警告跟着提交进去 ✓。）

**⑤ 下一轮（一小步 ✓）**：把**真假判定**补上 `set` 一族 ✓（`if some_set:` ✓ —— 按"非空为真" ✓，
与 `len` 同一口径 ✓）；顺带把 `__bool__` 协议那条通道接上 ✓（`Lib/` 里到处是自定义 `__bool__` ✓）。
接完立刻复测上限与判据① ✓。

#### 第 101 轮：下标／`in`／迭代的 dict 判定改成**子类型** ✓ —— 两行复现**与参照完全一致** ✓，族前进到 `set & set` ✓

**① 动手** ✓：`executor` 里"是不是 dict"的判定共 **7 处**用的是**精确类型**比较 ✗
（`container_type == builtin_type(instance, "dict")` ✓）—— 下标**读**／**写**／**删**三处 ✓ ＋
`in`／迭代／`is_true` 那几处 ✓ ⇒ 一律改成 `instance.is_subtype(container_type, builtin_type(instance, "dict"))` ✓
（与 `length_of` 第 99 轮那处**同一形状** ✓）。

**② 实测（两行复现**完全对齐** ✓）**：
```
class D(dict): … → len(d) = 0 ✓、d["k"] = 1 之后再 len(d) = 1 ✓    参照：0 / 1 ✓（逐字同 ✓）
```
`import enum` 又前进一格 ✓：现在只差 `TypeError: unsupported operand type(s) for &: 'set' and 'set'` ✓。

**③ 上限榜：一族连跳三格 ✓**（都是普通缺口，不再是崩溃 ✓）：
```
-6（SIGABRT，116）  →  setdefault（119）  →  下标派发（121）  →  set & set（119）✓
```
⇒ 那 **119** 个模块（`argparse`／`asyncio`／`textwrap`／`_markupbase` … ✓）已经从"**内存崩溃**"一路
走到"**少一个集合运算符**" ✓ —— 这是整条链最接近"能 import"的时刻 ✓。

**④ 闸门** ✓：`cargo test --workspace` ✓、0 警告 ✓、`check.py` 12/12 ✓、`CX-8` ✓、夹具守卫 ✓、
语料下限 182 ✓、逐字节 **4/4** ✓、对拍**普通与 `DANGLING` 两趟全绿** ✓、`stability` ✓、`selftest` ✓、
`t_ab_1` ✓；上限 **159** ✓、判据① **27.4%**（172 ÷ 628 ✓）。

**⑤ 下一轮（一小步 ✓）**：把 **`set` 的运算符**接上 ✓（`&`／`|`／`-`／`^` ✓，含"方法与运算符两条通道" ✓）
—— 族里第一句错就是它 ✓；接完再量上限与判据① ✓（若那一族整体过关，上限与判据① 应当**第一次真正跳动** ✓）。

#### 第 100 轮：方法面也随布局继承 ✓ —— 上限榜的族**连跳两格**：`setdefault`（119）→ 下标派发（121）✓

**① 补 `dict` 方法面** ✓：查了一圈 —— `setdefault` **本来就在** `dict` 的方法表里 ✓
（`"setdefault" => dict_setdefault_native` ✓）。所以 `EnumDict` 报"没有 `setdefault`" ✗ 不是缺实现 ✓，
而是**子类看不到基类的方法面** ✗ —— `dict` 那些方法挂在 **`getattr` 槽**上 ✓（不在类型字典里 ✓），
而 `inherit_host_layout` 只继承 `dealloc`／`finalize`／`traverse`／`clear` ✗
⇒ 顺手把 **`getattr`／`setattr` 也一起继承** ✓（`inherit_host_layout_with_new` 里两行 ✓）。

**② 效果（一族连跳两格 ✓）**：
```
上限榜前：119  AttributeError: 'EnumDict' object has no attribute 'setdefault'
上限榜后：121  指令 44 尚未接线：下标只接线了 tuple／list／dict／str／bytes      ← 又是"精确类型判定"那一形状 ✓
```
⇒ `setdefault` 那一格**过了** ✓，现在整族卡在**下标派发**上 ✓ —— 与 `length_of` 完全同一个病 ✓
（`if Some(ty) == self.type_named("dict")` ✗ ⇒ 子类落空 ✓）。**病灶从"内存崩溃"→"缺方法"→"派发不认子类"，
一路降级成最普通的一类缺口** ✓。

**③ 闸门** ✓：`cargo test --workspace` ✓、0 警告 ✓、`check.py` 12/12 ✓、`CX-8` ✓、夹具守卫 ✓、
语料下限 182 ✓、逐字节 **4/4** ✓、对拍普通趟 ✓（`DANGLING` 那趟红 ✗ ＝ 那条既有、间歇缺陷 ✓）；
上限 **159** ✓、判据① **27.4%**（172 ÷ 628 ✓）。

**④ 下一轮（一步，形状已经很清楚 ✓）**：把**下标读写**（`subscript_read`／`subscript_write` 那条派发 ✓）
从"精确类型"改成**子类型判定** ✓ —— 与 `length_of` 那一处的改法**同一形状** ✓（`dict` 先改 ✓，
`list`／`tuple`／`str`／`bytes` 随后照同一判据 ✓）。改完再量一次上限与判据① ✓。

#### 第 99 轮：🎉🎉 **`-6`（SIGABRT）整族消失** ✓ —— 「VM 内建带布局的基类」补上了 ✓（`dict` 子类现在真的是 dict ✓）

**① 按第 98 轮的定位动手** ✓（那一格就是"布局继承只看 `host_base`" ✓）：
- **布局基类** ✓：在 `classes.rs` 里，`host_base` 为空时**再看 VM 内建**（本轮先纳 `dict` ✓）——
  判据：该基类是 `dict` 的子类型**且自带 `new` 槽** ✓；命中就走**同一支** ✓
  （`new_type(static_name, base_info.instance_size(), slots)` ✓）；
- **`new` 槽** ✓：重新加回 `Slots::inherit_host_layout_with_new()` ✓（第 97 轮那个变体 ✓）并在该支使用 ✓
  ⇒ 实例化会走 **`dict_new`** ✓ ⇒ 载荷（那格 `RefCell<Vec<…>>`）**建起来了** ✓；
- **`dict_new` 忽略实参** ✓（照参照：`dict.__new__` 只建空映射 ✓，实参归 `__init__` 管 ✓ ——
  `EnumDict.__init__(self, cls_name=None)` 正是靠这个 ✓）；先前"有实参就报未接线" ✗；
- **派发** ✓：`length_of` 的 `dict` 支改成**子类型判定** ✓。

**② 实测（三条硬证据 ✓）**：
```
两行复现  class D(dict): …            → len(d) 现在给 **0** ✓（参照 0 ✓；先前 TypeError ✗）
import enum                          → **不再崩** ✓，变成普通错误 AttributeError: 'EnumDict' object
                                        has no attribute 'setdefault' ✓
上限榜                                → `-6`（SIGABRT）**整族消失** ✓
```
⇒ 上限榜最大的族从 **116** 个"内存崩溃" ✓ 变成 **119** 个**普通缺失方法**
（`AttributeError: 'EnumDict' object has no attribute 'setdefault'` ✓）—— **病灶从"内存被写坏"
降级成"少一个方法"** ✓✓ 这正是这十几轮追下来的那一格 ✓。

**③ 闸门** ✓：`cargo test --workspace` ✓、0 警告 ✓、`check.py` 12/12 ✓、`CX-8` ✓、夹具守卫 ✓、
语料下限 182 ✓、`pyawa-core --test compile` 逐字节 **4/4** ✓、对拍普通趟 ✓
（`PYAWA_DANGLING=1` 这趟红 ✗ ＝ 那条既有、间歇缺陷 ✓）；上限 **159** ✓、判据① **27.4%**（172 ÷ 628 ✓）。

**④ 下一轮（三步，都很直 ✓）**：
1. **`dict` 方法面** ✓：补 `setdefault`（本轮 119 个模块就等它 ✓）——顺带把 `dict` 缺的方法**一次对齐** ✓；
2. **下标读写按子类型判** ✓（现在 `d["k"] = 1` 仍报"只接线了 list／dict" ✗ —— 与 `length_of` 同一形状 ✓）；
3. 之后再量一次上限与判据① ✓。

#### 第 98 轮：🎯 **缺口精确定位** ✓ —— 布局继承只看 **`host_base`**（`AB-58` 宿主类型）✗，**VM 内建**（`dict`／`list`…）没被算进去 ✓

**① 顺实例化路径查** ✓：`executor` 里"调类型"那一支读的是**类自己的 `new` 槽** ✓
（`let new_slot = class.slots().new` ✓；为空就报 `cannot create '<类名>' instances` ✓）——
而我们那条两行复现报的是 `object of type 'D' has no len()` ✓ **不是**"cannot create" ✓
⇒ 说明 `D()` **确实建出了实例** ✓ ⇒ 类创建时给 Python 类装了**通用 `new`** ✓。

**② 装在哪一支（本轮的关键 ✓）**：`classes.rs` 建类型那段，判据是 **`host_base`** ✓ ——
```
let ty = match host_base {
    Some(base) => { ... new_type(static_name, base_info.instance_size(), slots) ... }   // AB-58：宿主类型
    None       => { ... AttributeObject::slots().with_new(attribute_new) ... }          // 通用
```
注释写得很明白 ✓：「宿主类型的 Python 子类**继承同一布局**…**没有**默认 `new`：宿主类型实例由宿主经
`pa_newhandle` 建（`AB-58`）」✓ —— 也就是说：**只有"宿主注册的类型"** 才算"有布局的基类" ✗，
而 **`dict`／`list`／`tuple`／`set` 这些 VM 内建**（`AB-58` 之外 ✓）**不在此列** ✗
⇒ `class D(dict)` 落到 `None` 那一支 ✓ ⇒ 实例是**通用 `AttributeObject` 布局** ✓
（挂外部实例字典 ✓、`new` ＝ `attribute_new` ✓）⇒ **它根本不是 `DictObject`** ✓✓
⇒ 于是"按 `DictObject` 读"读到的是那块内存里的字符串数据 ✓（ASCII 怪数字 ✓）、
`len` 也不认 ✓ —— **第 95～97 轮那条链的最后一格就是这里** ✓。

**③ 下一轮施工图（一处、且已看清 ✓）**：把"**布局基类**"的选择从"只看 `host_base`"扩到
"**也看带布局的 VM 内建**" ✓ ——
① 选出这样一个基类（判据：它的 `instance_size` 是**它自己的载荷**、且／或有 `new` 槽 ✓，如 `dict` ✓）；
② 走同一支：`new_type(static_name, base_info.instance_size(), slots)` ✓，槽位用
**`inherit_host_layout_with_new()`**（第 97 轮那个变体 ✓，把 `new` 一起继承 ✓ ⇒ 实例化就会走
**`dict_new`** ✓，载荷随之建好 ✓）；
③ 再把 `length_of`／下标读写／方法面改成**子类型判定**（第 96／97 轮的改动 ✓，这次与布局一起落地 ✓）。
**验收用第 95 轮那两行复现** ✓（参照 `0`／`1` ✓）＋ `import enum` ✓。

**④ 闸门与数字** ✓：`cargo test --workspace` ✓、0 警告 ✓、`check.py` 12/12 ✓、`CX-8` ✓、夹具守卫 ✓、
语料下限 182 ✓、逐字节 **4/4** ✓、对拍普通／`DANGLING` ✓；判据① **27.4%**（172 ÷ 628 ✓）、
上限 **159** ✓、族：`-6` **116** ✓、`eval` 76 ✓、`annotationlib` 28 ✓、`_struct` 19 ✓。

#### 第 97 轮：`new` 槽**已随布局继承** ✓（`dict` 确实有 `dict_new` ✓），但实例化**仍不走它** ✗ ⇒ 下一格是**实例化路径** ✓；半修**撤回** ✗

**① 按第 96 轮施工图动手** ✓（"布局初始化 ＋ 派发"一起做 ✓）：
- **布局/实例化** ✓：`Slots::inherit_host_layout` 只继承 `dealloc`／`finalize`／`traverse`／`clear` ✓，
  **把 `new` 置空** ✗ ⇒ 加了 `inherit_host_layout_with_new()` ✓（`slots.new = self.new` ✓），
  类创建那一支改用它 ✓ —— 查证：`dict` 的类型对象**确实**接了 `dict_new` ✓（`instance.rs` 里
  `.with_new(crate::builtin_objects::dict_new)` ✓），所以继承**应当**生效 ✓；
- **派发** ✓：`length_of` 的 `dict` 支改成**子类型判定** ✓（`is_subtype(ty, dict)` ✓）。

**② 结果** ✓（如实 ✓）：两行复现**仍然崩** ✗（`memory allocation of 1748935431469216 bytes failed` ✓）、
`import enum` 也照旧崩 ✗ ⇒ **继承 `new` 没有让它走上 `dict_new`** ✗ ⇒ 说明**实例化路径不看这个槽** ✓
（多半走的是"通用分配 ＋ 宿主 `pa_newhandle`"那条 ✓ —— 代码里那句注释正是这么写的 ✗：
"**没有**默认 `new`：宿主类型实例由宿主经 `pa_newhandle` 建（`AB-58`）" ✓）。

**③ 按纪律撤回** ✓：这一轮的两处改动**合起来**仍是**半修** ✗（把**干净的** `TypeError` 变成 **abort** ✓，
观测上更糟 ✓）⇒ `crates/` 回到原样 ✓（复现照旧 `TypeError` ✓）。

**④ 下一格（已定位到具体一条路 ✓）**：**实例化路径**（`type.__call__` → 建实例那一步 ✓）——
现在的判据是"宿主类型由 `pa_newhandle` 建" ✗，可**Python 层的子类**（`class D(dict)` ✓）没有宿主 ✓
⇒ 得让它在**实例化时按布局基类建载荷** ✓（两条可行路：① 实例化时查**继承来的 `new` 槽**并调它 ✓；
② 或按布局基类在 `alloc` 前先把载荷构造出来 ✓）。
**探针** ✓（下一轮第一步 ✓）：在实例化那一步打印"这条实例化走的是哪一支、`new` 槽读到没有" ✓ ——
一次就能确认它到底看没看 `new` ✓。

**⑤ 闸门与数字** ✓：`cargo test --workspace` ✓、0 警告 ✓、`check.py` 12/12 ✓、`CX-8` ✓、夹具守卫 ✓、
语料下限 182 ✓、逐字节 **4/4** ✓、对拍普通／`DANGLING` ✓；判据① **27.4%**（172 ÷ 628 ✓）、
上限 **159** ✓、族：`-6` **116** ✓、`eval` 76 ✓、`annotationlib` 28 ✓、`_struct` 19 ✓。

#### 第 96 轮：病灶**再往下一层** ✓ —— `dict` 子类的实例**载荷从未初始化**（基类 `tp_new` 没跑）⇒ 比 `length_of` 更根 ✓；半修**撤回** ✗

**① 按第 95 轮的方向动手** ✓（让 `dict` 子类"就是 dict"）：先查**布局**——类创建时子类实例尺寸
已经是按基类继承的 ✓（`classes.rs`：`new_type(…, base_info.instance_size(), …)` ✓），所以缺的是**派发** ✓：
`Instance::length_of` 里全是**精确类型**比较 ✗（`Some(ty) == self.type_named("dict")` ✓）⇒ 子类必然落空 ✓。

**② 改了 `dict` 那一支试** ✓（改成 `is_subtype` 判定 ✓）⇒ `TypeError: object of type 'D' has no len()` **没了** ✓
**但**紧接着撞上**荒唐分配** ✗（`memory allocation of 1672393011983152 bytes failed` ✓）⇒ 也就是：
`len` 现在**认得**子类了 ✓，可"按 `DictObject` 去读"读到的仍是垃圾 ✓ ⇒ 说明**那个实例的 dict 载荷本来就没被建起来** ✗。

**③ 顺着查到了更根的一层** ✓：`dict` 的实例是 `dict_new` 里造出来的 ✓ ——
```rust
instance.alloc(DictObject::new(class, core::cell::RefCell::new(Vec::new())))
```
⇒ **那格 `RefCell<Vec<…>>` 是 `dict_new` 建的** ✓；而 `class D(dict)` 的实例走的是**通用实例化** ✗
⇒ **基类的 `tp_new` 根本没跑** ✗ ⇒ 载荷是**未初始化内存** ✓ ⇒ 于是"长度／容量／指针"读出来就是
那块内存里的字符串数据 ✓（ASCII 怪数字 ✓）。**这才是 `EnumDict(dict)` 那条链的真根** ✓
（`Lib/enum.py` 里 `EnumDict.__init__` 的 `super().__init__()` 走到的是**空操作**的 `object_init_native` ✓，
也不会去建载荷 ✓）。

**④ 试改的那一下**撤回**了 ✗**（如实 ✓）：只把 `length_of` 改成子类型判定是**半修** ✗ ——
它把"**干净的** `TypeError`"变成了"**abort**" ✓ ⇒ 观测上更糟 ✓ ⇒ 按纪律撤 ✓
（`crates/` 回到原样 ✓，复现照旧报 `TypeError` ✓）。**修法必须"布局初始化 ＋ 派发"一起做** ✓，
不能只做一半 ✓。

**⑤ 下一轮施工图（就差这一处 ✓）**：让**内建类型的子类**在实例化时跑**基类的 `tp_new`** ✓ ——
① `type.__call__`／实例创建路径上，按"布局基类"（`dict`／`list`／`tuple`／`set`…）去调它自己的
`new`（或在 `alloc` 时按基类初始化载荷 ✓）；② 再把 `length_of`／下标读写／方法面改成**子类型判定** ✓
（第 95 轮那两行复现会同时变绿 ✓）。做对之后 `EnumDict` 才真的能用 ✓，`Lib/enum.py` 才进得去 ✓，
而那 **116** 个模块的 `-6` 族就压在它上面 ✓。

**⑥ 闸门与数字** ✓：`cargo test --workspace` ✓、0 警告 ✓、`check.py` 12/12 ✓、`CX-8` ✓、夹具守卫 ✓、
语料下限 182 ✓、逐字节 **4/4** ✓、对拍普通／`DANGLING` ✓；判据① **27.4%**（172 ÷ 628 ✓）、
上限 **159** ✓、族：`-6` **116** ✓、`eval` 76 ✓、`annotationlib` 28 ✓、`_struct` 19 ✓。

#### 第 95 轮：🎯🎯 **病灶定住了：`dict` 的子类没有被当成 dict** ✓（两行最小复现 ✓）—— 那 116 个模块的族就在这里

**① 按计划二分 `Lib/enum.py` 的模块级语句** ✓（第 81 轮那套"前缀导入" ✓，落在语句边界上 ✓）：
```
1-1105  ok                                        ← 到这里都正常 ✓
1-1329  unsafe precondition: ptr::copy_nonoverlapping …   ✗
```
⇒ 崩溃由 **1106～1329 行**引入 ✓ ＝ `class Enum(metaclass=EnumType):` ✓。再往类体内分 ✓：
```
1-1145  UB ✗        ← **只到"类头 ＋ docstring"就崩** ✓（1-1105 还是 ok ✓）
```
⇒ **崩在"创建 `Enum` 这个类"这一步** ✓，与类体内容无关 ✓。再探 `__prepare__` ✓（`PYAWA_PREPARE_DEBUG=1` ✓）：
**探针没打印** ✓ ⇒ 崩在 **`EnumType.__prepare__` 内部** ✓ —— 而它第一件事就是 `EnumDict(cls)` ✓
（`EnumDict` 是 `dict` 的**子类** ✓）⇒ 焦点落到"`dict` 子类的实例"上 ✓。

**② 两行最小复现（本轮最硬的产出 ✓）**：
```python
class D(dict):
    def __init__(self):
        super().__init__()
d = D()
print(len(d))          # 我们：TypeError: object of type 'D' has no len() ✗   参照：0 ✓
d["k"] = 1             # 我们：指令 38 尚未接线：下标赋值只接线了 list／dict ✗   参照：正常 ✓
```
⇒ **`dict` 的子类实例没有被当成 dict** ✓（`len` 不认识 ✓、下标赋值不认 ✓）。
**这就解释了一切** ✓：`EnumDict(dict)` 的对象在类创建路径上被**按 `DictObject` 去用** ✓，
而它的实例布局／派发并不是 dict ✗ ⇒ 读到的"长度／容量／指针"其实是那块内存里的**字符串数据** ✓
（正是前几轮那些 ASCII 怪数字 `__cod__`／`name` ✓）、数值随运行而变 ✓、有时撞 UB 有时撞荒唐分配 ✓。

**③ 下一轮（范围明确 ✓）**：让 **`dict` 的子类**成为"真的 dict" ✓ ——
① 子类实例的**布局/尺寸**要沿用 `DictObject` ✓（`alloc_type_raw` 里按基类定 `instance_size` ✓）；
② **派发**要认基类：`len`／下标读写／`dict` 的方法面（`__contains__`／`get`／`keys` …）在子类实例上都要走 dict 那一套 ✓
（现在只认"类型就是 `dict`" ✗）。这一处做对，`EnumDict` 才可能工作 ✓，`Lib/enum.py` 才可能进得去 ✓，
而那 **116** 个模块的 `-6` 族就是压在它上面的 ✓。

**④ 闸门与数字** ✓：`cargo test --workspace` ✓、0 警告 ✓、`check.py` 12/12 ✓、`CX-8` ✓、夹具守卫 ✓、
语料下限 182 ✓、逐字节 **4/4** ✓、对拍普通／`DANGLING` ✓；判据① **27.4%**（172 ÷ 628 ✓）、
上限 **159** ✓、族：`-6` **116** ✓、`eval` 76 ✓、`annotationlib` 28 ✓、`_struct` 19 ✓。

#### 第 94 轮：又一处**撤回** ✓ —— 崩在 `__prepare__` **之前**（`import enum` 也是同一张脸）⇒ 病灶在**模块级代码**更早处 ✓

**① 按第 93 轮的收窄去探 `__prepare__`** ✓（`PYAWA_PREPARE_DEBUG=1` ✓：打印 `prepared` 的**类型名**、
是不是 `dict` 子类、`instance_size` 与 `DictObject` 的 Rust 布局对照、以及当 `DictObject` 读时的
`len`／`cap`／`ptr` ✓）—— 跑那两条会崩的（`import re` ✓、`import enum` ✓）：**探针一次都没响** ✗，
两条都在**荒唐分配**上 abort ✓ ⇒ 结论：**崩发生在走到 `__prepare__` 之前** ✓
⇒ 第 93 轮那条"病灶是 `__prepare__` 返回的那个映射" **不成立，撤回** ✗
（与第 91 轮 `super_new` 那条一样，都是**被假阳性带偏**的推论 ✓ —— 这次是靠"探针不响"当场否掉的 ✓）。

**② 现在**站得住**的（逐条都独立 ✓）**：
- 第 82 轮 `RUST_BACKTRACE` 真回溯 ✓：`lookup_in_mapping` ← `execute` ← `run_class_body`
  ← `build_class_native` ✓（**类体**路径 ✓）；
- 第 83 轮直读字段 ✓：**某个**类体命名空间字典在类体跑之前就已经是烂内存 ✓
  （`len=137163532649041`、`ptr=0x195` ✓）；
- 第 93 轮对照 ✓：**普通** `dict` 这一支"局部值 → 上堆"完全正常 ✓（`len=0 cap=0 ptr=0x8` ✓）
  ⇒ 烂内存**不是**新造普通 `dict` 造出来的 ✓；
- 本轮 ✓：`import enum` 与 `import re` 都**在元类路径之前**就崩 ✓（**模块级代码**里 ✓）。
⇒ 合起来：**崩点在"模块级代码"里某一步**，且它与"类体命名空间被写坏"是**同一条链上的先后**（先崩／先坏）。
⇒ 病灶的**位置**从"类创建"往**前**退到了**模块级语句** ✓（这是个方向性的更正 ✓）。

**③ 下一轮（方法已验证过 ✓）**：**按模块级语句二分** `Lib/enum.py` ✓ ——
用第 81 轮那套"前缀导入"（把前 N 条顶层语句当观测源跑 ✓，落在语句边界上 ✓）：
一段段缩小到**第一条会崩的语句** ✓，再对那一条做最小复现 ✓（第 81 轮用同一手法把 `_constants.py`
缩到 224 行、并对 `re` 链定位成功 ✓）⇒ 届时会拿到**具体那一句 Python** ✓。

**④ 闸门与数字** ✓：`cargo test --workspace` ✓、0 警告 ✓、`check.py` 12/12 ✓、`CX-8` ✓、夹具守卫 ✓、
语料下限 182 ✓、逐字节 **4/4** ✓、对拍普通／`DANGLING` ✓、`stability` ✓、`selftest` ✓、`t_ab_1` ✓；
判据① **27.4%**（172 ÷ 628 ✓）、上限 **159** ✓、族：`-6` **116** ✓、`eval` 76 ✓、`annotationlib` 28 ✓、
`_struct` 19 ✓。

#### 第 93 轮：两个**决定性否定** ✓ —— GC 不是原因 ✓、新造的命名空间字典**上堆后是好的** ✓ ⇒ 病灶在**另一支**（`__prepare__` 那条）✓

**① 否定一：GC 不是"刚造好就是垃圾"的原因** ✓：给 `alloc` 里那次回收加开关（`PYAWA_NO_GC=1` ✓）——
关掉之后**照样崩** ✗（只是换回另一张脸：`memory allocation of …` 与 UB 交替 ✓）⇒ **否掉** ✓。
（这个开关**留着** ✓：下次不必再试 ✓；默认关 ＝ 行为不变 ✓。）

**② 否定二（更要紧 ✓）**：在"新造命名空间"那一支加**对照探针**（`PYAWA_DICT_DEBUG=1` ✓）——
先读**局部值**（还没上堆 ✓）、再读 `alloc` 之后的**同一个对象** ✓：
```
[字典对照] 局部值: len=0 cap=0
[字典对照] 上堆后 0x…16cca0: len=0 cap=0 ptr=0x8      ← 完全正常 ✓
```
⇒ `alloc` **没有**弄坏它 ✓，「刚造好就是垃圾」**不是**普通 `dict` 这一支 ✓。
⇒ 那第 83 轮看到的那块烂内存，只可能是**另一支**：`__prepare__` 返回的 **`EnumDict`** ✓
（`Lib/enum.py` 的 `EnumDict` 是 `dict` 的**子类** ✓，走的是 `Some((metaclass, method))` 那一支 ✓）
—— 与第 91 轮回溯落在 **`super_new`**（正是 `EnumDict.__init__` 里那句 `super()` 调的 ✓）**同路** ✓✓。
⇒ **病灶范围收窄到：元类 `__prepare__` 路径上的那个"映射"对象** ✓。

**③ 下一轮（就一处 ✓）**：在 `__prepare__` 那一支探针 ✓ —— `prepared = call_value(...)` 之后
打印**它的类型名** ✓、`type_of(prepared)` 是不是 `dict` 的子类 ✓、以及把它当 `DictObject` 读时的
`len`／`cap`／`ptr` ✓ ⇒ 判定：是"`EnumDict` 实例被当成 `DictObject` 读 **布局不对**" ✓，
还是"这个对象本身已经烂了" ✓。两者修法完全不同 ✓（前者是**子类实例布局／内联字典**那一摊 ✓，
后者才是内存问题 ✓）—— 一步就能分开 ✓。

**④ 闸门与数字** ✓：`cargo test --workspace` ✓、0 警告 ✓、`check.py` 12/12 ✓、`CX-8` ✓、夹具守卫 ✓、
语料下限 182 ✓、逐字节 **4/4** ✓、对拍普通／`DANGLING` ✓；判据① **27.4%**（172 ÷ 628 ✓）、
上限 **159** ✓、族：`-6` **116** ✓、`eval` 76 ✓、`annotationlib` 28 ✓、`_struct` 19 ✓。

#### 第 92 轮：⚠️ **撤回并改正** ✓ —— 第 88～91 轮那条"僵尸写"是**假阳性** ✗（登记表按地址记、地址会被复用 ✓）

**① 怎么发现的** ✓：本轮在 `super_new` 里加**引用计数探针** ✓（造字典 → 读 `rc` → 造 `super` 对象 → 再读 ✓）：
```
[super 探针] 造好字典 0x…1cb610：rc=1（此刻应是 1 ✓）
[super 探针] 造好 super 对象 0x…148030 之后：字典 0x…1cb610 rc=1
[僵尸写]      正在写一个**已释放**的对象 0x…1cb610（str；释放于 EnumDict.__init__@6）✗
```
⇒ **僵尸写指认的地址，正是我刚造好、`rc` 明明还是 1 的那个字典** ✓✓ ⇒ **自相矛盾** ✓ ⇒ 说明是**探测错了** ✗，
不是程序错了 ✓。

**② 错在哪（根因 ✓）**：`freed_sites` 登记表是**按地址**记的 ✓，而**地址会被复用** ✓ ——
只要那块地址**以前**被释放过、现在又分给了新对象 ✓ ⇒ 写入时一查就命中 ⇒ **误报** ✓
（第 89 轮我把 `insert` 改成 `or_insert`（只记第一次）✗ ⇒ 反而**加剧**了这一点 ✓：老条目永远不会被顶掉 ✓）。

**③ 改正** ✓（本轮落地 ✓）：**分配时把该地址从登记表里清掉** ✓ ⇒ 命中就只剩一种含义：
"**释放之后没再分配过**" ✓ ⇒ 可靠 ✓；同时释放登记改回"最近一次" ✓。改完重跑 ✓：
**僵尸写不再响** ✓ ⇒ 第 88～91 轮那一条链（"写入目标是已释放的 `str`" → "`dict_set` 收回非 dict"
→ "**凶手是 `super_new`**" ✓）**全部撤回** ✗ —— **`super_new` 那一段是平的** ✓（rc 全程 1 ✓）。

**④ 仍然站得住的（与登记表无关 ✓）**：第 83 轮那条**独立证据** ✓ —— 类体命名空间字典**刚造好就是垃圾**
（`len=137163532649041`、`ptr=0x195` ✓，直接读对象字段 ✓，不依赖任何登记 ✓）；
第 82 轮 `RUST_BACKTRACE` 的**真回溯** ✓（`lookup_in_mapping` ← `execute` ← `run_class_body`
← `build_class_native` ✓）；第 84 轮"`alloc` 拿到该地址时它不在活表里" ✓（⇒ 不是重复发放 ✓）。
⇒ 病灶仍是"**类体命名空间字典在造出来的那一刻就已经是一块烂内存**" ✓，但**持有的那个"僵尸写"理由是错的** ✗。

**⑤ 教训与下一轮** ✓：**按地址记的登记表，必须"分配时清"** ✓ —— 否则命中不等于"释放后没被复用" ✓；
这次是**引用计数探针**（一条独立通道 ✓）把假阳性照出来的 ✓ ⇒ 下一轮换个**与登记表无关**的办法：
给那块地址记一条**分配／释放历史**（(地址, 类型, 现场) 环形缓冲 ✓，按**时间序**读 ✓），
看"类体命名空间字典这块地址"在它被造出来之前**是谁的** ✓；或直接在**造命名空间那一刻**校验
`DictObject` 的字段是否等于"新的空字典"（`len==0` ✓），不成立就**当场报错** ✓（比事后追更硬 ✓）。

**⑥ 闸门与数字** ✓：`cargo test --workspace` ✓、0 警告 ✓、`check.py` 12/12 ✓、`CX-8` ✓、夹具守卫 ✓、
语料下限 182 ✓、逐字节 **4/4** ✓、对拍普通 ✓；判据① **27.4%**（172 ÷ 628 ✓）、上限 **159** ✓、
族：`-6` **116** ✓、`eval` 76 ✓、`annotationlib` 28 ✓、`_struct` 19 ✓。

#### 第 91 轮：🎯 **凶手是 `super_new`** ✓ —— `super()` 的实现往**已释放**对象里写字典（Rust 回溯确证 ✓）

**① 先看码元** ✓：把 `EnumDict.__init__` 摊开 ✓ ⇒ "指令 6" 落在 **`super().__init__()`** 那一段的 `CALL` 上 ✓
（`0 RESUME／1 LOAD_GLOBAL／6 CALL／10 LOAD_ATTR／20 PUSH_NULL／21 CALL／25 POP_TOP／26 BUILD_MAP…` ✓）
⇒ 现场就是那句 **`super()`** ✓。

**② 再让探测**自己交出 Rust 回溯** ✓**（把 `Backtrace::force_capture()` 加进僵尸那条 panic ✓；
**试过**先查"映射类型对不对" ✗ —— **没用** ✓：那块内存的表头自己已经烂了 ✓，`type_of` 读不出真类型 ✓，如实撤掉 ✓）：
```
0: Instance::zombie_probe
1: Instance::dict_set
2: pyawa_core::builtin_objects::super_new      ← **凶手**
3: executor::call_callable
…
12: classes::build_class_native
```
⇒ **`super_new` 里那两行 `dict_set`（写 `__thisclass__`／`__self__`）打在了一个已释放的对象上** ✓✓。

**③ 病灶形状（确凿到一段代码 ✓）**：`super_new` 给 `super` 对象造"内联属性字典"：
```rust
let object = instance.alloc(AttributeObject::new(
    super_type,
    RefCell::new(Some(instance.new_dict())),   // ← 这份字典的唯一引用
)).into_raw().cast::<Header>();
let attrs = unsafe { &*object.as_ptr().cast::<AttributeObject>() };
if let Some(dict) = attrs.attributes() {
    instance.dict_set(dict, "__thisclass__", class_value);   // ← 写的时候它已经死了 ✗
    instance.dict_set(dict, "__self__", this);
}
```
⇒ `new_dict()` 返回的是**裸指针**（引用归调用方 ✓）、交给 `AttributeObject` ✓ ⇒ 但**下一次 `dict_set` 时它已经被放掉** ✓
⇒ 所有权链上**某处多放了一次** ✓（候选：`AttributeObject::new` 的落点、`alloc` 那一刻的 **GC 回收** ✓、
或 `attributes()` 拿到的引用与 `clear` 的那份不是同一份 ✓）。**已核过**：`attribute_traverse` **会**访问这份字典 ✓、
`alloc` 是"先 `adopt`（含 `link_gc`）后 `collect`" ✓ ⇒ 这两条把"GC 误收"的嫌疑**压小**了 ✓，
剩下最像的是"这份引用在交给对象之前／之后被放了一次" ✓。

**④ 下一轮（就差这一步 ✓）**：在 `super_new` 那段里加一发**引用计数探针** ✓（造字典时记 `refcount` ✓、
`dict_set` 之前再读一次 ✓ ⇒ 少了就说明谁放的 ✓），或直接对着这条链逐行走一遍所有权 ✓ ——
**这是 `-6` 族 116 个模块的唯一根** ✓（`EnumDict.__init__` 那句 `super()` 是所有 Enum 类创建都要走的 ✓）。

**⑤ 落地** ✓（`feat(diag)`）：僵尸写那条 panic **连 Rust 回溯一起打** ✓（默认关、零开销 ✓）——
它是本轮真正拿到名字的那一下 ✓（与"标签创建处"同一条路子 ✓）。

**⑥ 闸门与数字** ✓：`cargo test --workspace` ✓、0 警告 ✓、`check.py` 12/12 ✓、`CX-8` ✓、夹具守卫 ✓、
语料下限 182 ✓、`stability` ✓、`selftest` ✓、`t_ab_1` ✓、逐字节 **4/4** ✓、对拍普通／`DANGLING` ✓；
判据① **27.4%**（172 ÷ 628 ✓）、上限 **159** ✓、族：`-6` **116** ✓、`eval` 76 ✓、`annotationlib` 28 ✓、
`_struct` 19 ✓。

#### 第 90 轮：🎯 **写入方 = 释放方 = `EnumDict.__init__@6`** ✓（同一条语句既放掉它、又往它里面写）—— 最后一格钉死 ✓

**① 把写入方的现场也打进报错** ✓（同一套工具 ✓）：
```
[僵尸写] 正在写一个**已释放**的对象 0x72e30c1168b0（str；释放于 EnumDict.__init__@6）✗；
         写入方：EnumDict.__init__@6
```
⇒ **写入方与释放方是同一个现场** ✓ ⇒ `Lib/enum.py` 的 `EnumDict.__init__` 里**第 6 条指令**那一步：
**先放掉**那个对象、**紧接着又通过（已失效的）指针往里写** ✓；而那个对象的类型是 **`str`** ✓
⇒ 也就是**把一个 `str` 当成映射／命名空间在写** ✓（第 89 轮"类型混淆"的结论，这里给到了**具体那一句** ✓）。

**② 这条链现在完整了** ✓（几轮攒起来的 ✓）：
`Lib/re/__init__.py` 触发 ✓ → `build_class_native`／`run_class_body`（第 82 轮凶手栈 ✓）
→ 类体命名空间字典**刚造好就是垃圾**（第 83 轮 ✓）→ `alloc` 拿到的地址**不在活表里**（第 84 轮 ✓，
排除"重复发放" ✓）→ **僵尸写**（第 88 轮抓到 ✓）→ 写入目标是**已释放的 `str`**（第 89 轮 ✓）
→ **`EnumDict.__init__@6` 既释放又写**（本轮 ✓）。
⇒ **下一站只需一处** ✓：看 `Lib/enum.py` 的 `EnumDict.__init__` 第 6 条对应哪句 ✓，
再核我们**给 `EnumType.__prepare__`／`EnumDict` 传的 `self`（或那个映射参数）为什么是 `str`** ✓ ——
`__prepare__` 那条**元类**路径的实参／槽位就是最后要核的地方 ✓（第 86 轮已排除
`build_class_from_parts` 那一侧 ✓）。

**③ 落地** ✓（`feat(diag)`）：探测报错加上"写入方现场" ✓（默认关、零开销 ✓）。

**④ 闸门与数字** ✓：`cargo test --workspace` ✓、0 警告 ✓、`check.py` 12/12 ✓、`CX-8` ✓、夹具守卫 ✓、
语料下限 182 ✓、`stability` ✓、`selftest` ✓、`t_ab_1` ✓、逐字节 **4/4** ✓、对拍普通／`DANGLING` ✓；
判据① **27.4%**（172 ÷ 628 ✓）、上限 **159** ✓、族：`-6` **116** ✓、`eval` 76 ✓、`annotationlib` 28 ✓、
`_struct` 19 ✓。

#### 第 89 轮：僵尸写**再收一格** ✓ —— 不是"旧的 dict 被复用"，而是**拿 `str` 当 dict 写** ✓（类型混淆）

**① 先把探测做对** ✓：`freed_sites` 先前会被**后一次释放覆盖** ✗（地址复用 ⇒ 只剩"最后占着它的对象"
的现场 ✓，实测就是那个 `str` ✗）⇒ 改成 **`or_insert`：只记"第一次"释放** ✓ ⇒ 拿到的就是**最早那次**释放的
现场 ✓。

**② 结果** ✓（重跑那条会崩的 `import re` ✓）：
```
[僵尸写] 正在写一个**已释放**的对象 0x75ced01168b0（str；释放于 EnumDict.__init__@6）✗
```
⇒ 该地址**第一次**被释放时的类型就是 **`str`** ✓ ⇒ 说明：`dict_set` 收到的那个指针**从来就不是 dict** ✗
—— **拿一个 `str`（且已释放）当 dict 去写** ✓ ⇒ **类型混淆** ✓（不是"旧 dict 被复用"那一支 ✗）。
⇒ 与前面所有症状合得上 ✓：ASCII 残留是"那块内存里本来就是字符串数据" ✓；"新造的字典刚造好就是垃圾" ✓
＝ 它落在被写坏的堆上 ✓；UB 是读那个"名字段" ✓。**释放现场 `EnumDict.__init__@6`（`Lib/enum.py`）** ✓
⇒ 与"类体／元类那一侧"（第 82 轮凶手栈 ✓）一致 ✓。

**③ 下一站（给最后一轮）** ✓：查**谁把 `str` 当命名空间／映射递进 `dict_set`** ✓ ——
`dict_set` 的调用方（`STORE_NAME` 一条线 ✓、类创建／`__prepare__` 一条线 ✓）里，谁手里那个"映射"
其实是 `str` ✓。线索现成：`current_site()` 在**释放**时给的是 `EnumDict.__init__@6` ✓
⇒ 在**写入**处再打一发 `current_site()` ✓（同一套工具 ✓）就能看到**写入方**是哪条 Python 语句 ✓。

**④ 落地** ✓：`freed_sites` 改 `or_insert`（只记第一次 ✓）已随本轮留下 ✓ —— 它是"地址复用后仍能指认
最早释放者"的那一步 ✓，与探测本体同属工具 ✓（默认关、零开销 ✓）。

**⑤ 闸门与数字** ✓：`cargo test --workspace` ✓、0 警告 ✓、`check.py` 12/12 ✓、`CX-8` ✓、夹具守卫 ✓、
语料下限 182 ✓、`stability` ✓、`selftest` ✓、`t_ab_1` ✓、逐字节 **4/4** ✓、对拍普通／`DANGLING` ✓；
判据① **27.4%**（172 ÷ 628 ✓）、上限 **159** ✓、族：`-6` **116** ✓、`eval` 76 ✓、`annotationlib` 28 ✓、
`_struct` 19 ✓。

#### 第 88 轮：🎯🎯 **僵尸写抓到现行了** ✓ —— 写入目标是一个**已释放的 `str`**（释放于 `EnumDict.__init__@6`）✓

**① 按上一轮的施工图把检查放到**写入点** ✓**（那里手里有 `&Instance` ✓，不用改接口 ✓）：
新增 `Instance::freed_sites` ＋ `zombie_trace`（`PYAWA_ZOMBIE_TRACE=1` ✓，**关着零开销** ✓）——
`unlink` 时记 `地址 → (类型名, 释放现场)` ✓，`dict_set`／`dict_set_int` **每次写之前查一眼** ✓：
要写的对象**已经在释放登记里** ⇒ 就是**旧主人还在写已释放的对象** ✓ ⇒ 当场 panic 报出**是谁释放的** ✓。

**② 一跑就中 ✓**（跑那条会崩的 `import re`，带 `_sre` 桩 ✓）：
```
[僵尸写] 正在写一个**已释放**的对象 0x705cfc0b33f0（str；释放于 EnumDict.__init__@6）✗
```
⇒ **写入目标是一个"曾经是 `str` 的地址"** ✓ —— 而那正是上限榜 `-6` 族所有怪症状的源头 ✓：
- **ASCII 残留**（`__code__`／`name` ✓）就是"这块内存曾被字符串占着" ✓；
- 新造的字典"刚造好就是垃圾" ✓ ＝ 它落在了一块**已被释放、地址被复用**的内存上 ✓；
- `ptr::copy_nonoverlapping` 的 UB ✓ ＝ 读那个"名字段" ✓。

**③ 结论（确凿 ✓）**：我们的代码**手里握着一个已经释放的 dict 指针** ✓（它的地址后来被一个 `str` 占过、
那个 `str` 也已释放 ✓）⇒ 还在往里写 ✓ ⇒ 这就是**僵尸写** ✓；释放现场给到了
**`EnumDict.__init__@6`** ✓（`Lib/enum.py` ✓）⇒ 与"类体／元类那一侧"（第 82 轮凶手栈 ✓）**对上了** ✓。

**④ 落地** ✓（`feat(diag)`）：这个探测**留在树里** ✓ —— 默认关、零额外开销 ✓，却能在**一轮里**把
"谁提前释放、谁还在写"钉死 ✓（与第 359 轮的"标签创建处"同一条路子 ✓，是本轮真正拿到名字的工具 ✓）。

**⑤ 下一轮施工图（最后一层 ✓）**：顺 `EnumDict.__init__`（`Lib/enum.py` 的 `EnumType.__prepare__`
返回的那个 `EnumDict` ✓）与**类创建**这一条线 ✓，找**谁把那个 dict 的引用放多了** ✓
（`__prepare__` 的返回值所有权 ✓、类体命名空间 ✓、`type_dict` ✓ —— 第 86 轮核过
`build_class_from_parts` 是平的 ✓，那嫌疑就落在 `__prepare__` 那条**元类**路径上 ✓）。

**⑥ 闸门与数字** ✓：`cargo test --workspace` ✓、0 警告 ✓、`check.py` 12/12 ✓、`CX-8` ✓、夹具守卫 ✓、
语料下限 182 ✓、`stability` ✓、`selftest` ✓、`t_ab_1` ✓、逐字节 **4/4** ✓、对拍普通／`DANGLING` ✓；
判据① **27.4%**（172 ÷ 628 ✓）、上限 **159** ✓、族：`-6` **116** ✓、`eval` 76 ✓、`annotationlib` 28 ✓、
`_struct` 19 ✓。

#### 第 87 轮：隔离档那条"释放现场"报告**在这一格不响** ✓（如实 ✓）；改法需要**把 `&Instance` 接到写入点** ✓

**① 实测** ✓：把 `PYAWA_QUARANTINE=1`（毒化 ＋ 释放现场登记 ✓）＋ `RUST_BACKTRACE=1` 一起上，
跑那条会崩的 `import re`（带 `_sre` 桩 ✓）：既**没有**「隔离区 …载荷在**释放之后**被写过 ✗」那条报告 ✗、
也没有释放现场 ✗ —— 只有那条 `ptr::copy_nonoverlapping` 的 UB ⇒ **abort** ✓。
⇒ 也就是说：**现成那套毒化检查抓不到这一格** ✗（它是在 `alloc`／`unlink` 时查**载荷有没有被写过** ✓，
而这里的顺序是"释放 → 内存被别人复用 → **新对象刚造好就已是垃圾**" ✓ ⇒ 检查跑在那一步时已经晚了／
窗口不对 ✓）。

**② 因此改法要变** ✓（下一轮施工图，已定形 ✓）：把"**释放后仍被写**"的检查放到**真正写入的那一刻** ✓，
也就是容器的写入点（`DictObject::insert_raw` ✓／列表 `append` 类 ✓）—— 但这些方法**手里没有
`&Instance`** ✗（`DictObject` 的方法只拿 `&self` ✓）⇒ 要么给写入点**加 `&Instance` 参数** ✓、
要么把检查挪到**紧邻写入的调用方** ✓（`executor`／`stdlib` 里那一层，手里有 `instance` ✓）。
两条路都要动**接口** ✓ ⇒ 属"设计一步" ✓（不是随手一行 ✗）—— 在剩余轮次里按此办 ✓。

**③ 本轮未落地代码改动** ✓（如实 ✓）：crates/ 一字未动 ✓ —— 只把"**哪条路走不通**"钉死了 ✓
（省掉下一轮再试一遍 ✗）。

**④ 闸门与数字** ✓：`cargo test --workspace` ✓、0 警告 ✓、`check.py` 12/12 ✓、`CX-8` ✓、夹具守卫 ✓、
语料下限 182 ✓、`stability` ✓、`selftest` ✓、`t_ab_1` ✓、逐字节 **4/4** ✓、对拍普通趟 ✓；
`PYAWA_DANGLING=1` 这趟红 ✗（那条既有、间歇缺陷 ✓，本轮无代码改动 ✓）。
判据① **27.4%**（172 ÷ 628 ✓）、上限 **159** ✓、族：`-6` **116** ✓、`eval` 76 ✓、`annotationlib` 28 ✓、
`_struct` 19 ✓。

#### 第 86 轮：僵尸写的持有人**再窄一步** ✓ —— `build_class_from_parts` 那侧是**平**的 ✓；落在"释放后仍被写入的容器" ✓

**① 继续按施工图走** ✓：把 `build_class_from_parts` 里命名空间的所有权逐条核了一遍 ✓ ——
取／建 `type_dict` ✓、把命名空间里的条目**逐条 incref** 后 `insert_raw` ✓、末尾
`release_object(namespace)` 放掉**调用方那一份** ✓ ⇒ 这一侧是**平**的 ✓（没有明显多放 ✓）。

**② 与第 83／84 轮的两条实测合起来看** ✓，结论更窄了 ✓：
- 第 83 轮：类体命名空间字典**刚造好就是**另一个字典风格的残留头部 ✓（`len`／`cap`／`ptr` ✓）；
- 第 84 轮：`alloc` 拿到它时，地址**不在活表里**（不是"重复发放" ✓）⇒ 那块内存**确实已被释放** ✓
  ⇒ 但**随后又被写进了别的东西** ✓ ⇒ **僵尸写** ✓；
- 而那两处残留是 **`__code__`／`name` 这样的条目名** ✓ ⇒ 更像"**某个 dict/list 在释放之后仍被
  `insert`／`append`**" ✓（容器类对象 ✓），而不是"某个 str 被复用" ✗。

**③ 下一轮施工图（定形 ✓）**：把"释放后仍被写"这一格**抓现行** ✓ —— 现成机制**正好对得上** ✓：
`PYAWA_QUARANTINE=1` 会把释放后的载荷**毒化成 `0xDE`** ✓，且 `quarantine_check` 已经能报
「载荷在释放之后被写过 ✗ ⇒ use-after-free」✓（`instance.rs` 里那条 ✓，带**释放现场** ✓）。
把它从"alloc／free 时才查" ✓ 扩到**容器的写入点**也查 ✓（`DictObject::insert_raw` ✓／列表 `append` 类 ✓）
⇒ 僵尸写一发生就**当场报出"是哪次释放、哪个 Python 现场"** ✓ —— 一轮就能拿到名字 ✓。

**④ 闸门与数字** ✓：`cargo test --workspace` ✓、0 警告 ✓、`check.py` 12/12 ✓、`CX-8` ✓、夹具守卫 ✓、
语料下限 182 ✓、`stability` ✓、`selftest` ✓、`t_ab_1` ✓、逐字节 **4/4** ✓、对拍普通／`DANGLING` ✓；
判据① **27.4%**（172 ÷ 628 ✓）、上限 **159** ✓、族：`-6` **116** ✓、`eval` 76 ✓、`annotationlib` 28 ✓、
`_struct` 19 ✓。

#### 第 85 轮：把那处**乘积容量**收成有界 ✓（`capacity overflow` 不再从这里来 ✓）；僵尸写**仍在** ✗

**① 顺着上一轮的抓手** ✓（隔离档下症状是 `capacity overflow` ✓）：全 `crates/` 里**只有一处**是
"**乘积**算容量"的形状 ✓ —— `bytes.replace(b"", new)` 那一格：
```rust
Vec::with_capacity(value.len() * (new.len() + 1) + new.len())
```
⇒ 只要**长度字段被污染**（僵尸写 ✓，第 83／84 轮 ✓），乘积必爆 ⇒ `capacity overflow` panic ✓
（`-6` 族在隔离档下的那张脸 ✓）。

**② 落地** ✓（`fix(core)`）：改成**检查算术 ＋ 上界**（`checked_mul`／`checked_add`／`<= 1 TiB` ✓），
算不出合理容量就**不预留** ✓ —— **正确性不受影响** ✓（`push` 自己会增长 ✓）。
6 行探针（空 `old` 的插入语义 ✓、普通替换 ✓、`str` 与 `bytes` 两侧 ✓）与参照**逐字同** ✓。

**③ 如实说清它的分量** ✓：这条是**健壮性**修复 ✓ —— 它把那处 `capacity overflow` 掐掉 ✓，
但**没有**修僵尸写 ✗（根因仍在 ✓）。实测：隔离档下重跑 `import re`，「`capacity overflow`」那一张脸
**不再出现** ✓，改回撞 `ptr::copy_nonoverlapping` 的 UB ✓ ⇒ **两张脸里少了一张** ✓、根还在 ✓。

**④ 下一轮施工图（不变 ✓）**：第 84 轮那两件 —— ① 让 panic 位置现形（`pa_exec_string` 的"内部 panic
回填"在 `capacity overflow` 那一格**没生效** ✓，报文是空的 ✗ ⇒ 查明为何）；② 顺 `run_class_body`／
`build_class_native` 一侧找**僵尸写**的持有人 ✓（第 82 轮凶手栈 ＋ 第 83／84 轮结论 ✓，范围已经很窄 ✓）。

**⑤ 闸门与数字** ✓：`cargo test --workspace` ✓、0 警告 ✓、`check.py` 12/12 ✓、`CX-8` ✓、夹具守卫 ✓、
语料下限 182 ✓、`stability` ✓、`selftest` ✓、`t_ab_1` ✓、逐字节 **4/4** ✓、对拍普通／`DANGLING` **两趟全绿** ✓；
判据① **27.4%**（172 ÷ 628 ✓）、上限 **159** ✓、族：`-6` **116** ✓、`eval` 76 ✓、`annotationlib` 28 ✓、`_struct` 19 ✓。

#### 第 84 轮：病灶**再收一格** ✓ —— 不是"重复发放" ✗，而是**僵尸写**（旧主人还在写已释放的内存）✓；隔离档下症状变成 `capacity overflow` ✓

**① 按上一轮的施工图做了"存活自查"** ✓（`alloc` 侧：地址**还在活表里**却又要发出去 ⇒ 具名报出占着它的对象 ✓；
`dealloc` 侧同步记 `地址 → (类型名, 分配现场)` ✓、**关着时零额外开销** ✓）—— **没有响** ✗
⇒ 排除了"重复发放"这一支 ✓：地址在分配时**确实已经释放** ✓ ⇒ 那就是**旧主人还在往那块内存里写** ✓
（**僵尸写** ✓）⇒ 新对象刚造好就被覆盖 ⇒ 与"造好的字典读出另一个字典风格的残留头部" ✓ 完全相符 ✓。
（这轮追踪代码**未落地** ✓ —— 没响的工具不留 ✓，如实撤掉 ✓。）

**② 换隔离档跑，症状**换了脸** ✓（这是个新抓手 ✓）：
```
exit=1 ｜ exception_type=capacity overflow ｜ exception_message=
```
⇒ 打开毒化（`PYAWA_QUARANTINE=1` ✓）后，那块内存里的值变成 `0xDE…` ✓ ⇒ 于是**不再**是分配器的
"memory allocation of … failed" ✗、而是某处 **`Vec::with_capacity` 的 `capacity overflow` panic** ✓
⇒ **确证**：那个被污染的"长度"会被拿去算**容量** ✓（与第 81 轮列的 `Vec::with_capacity` 候选点对上 ✓）。
**可惜报文是空的** ✗（`capacity overflow` 只留下类型、没有位置 ✓）⇒ 下一轮把它变成**具名**的 ✓。

**③ 下一轮施工图** ✓（两件都小 ✓）：
1. **让 panic 的位置现形** ✓：`pa_exec_string` 那条"内部 panic 回填"（第 329 轮 ✓）在这里**没生效** ✗
   （报文空 ✓）⇒ 查明为什么（多半是被内层 `boundary` 先吞了 ✓），把 `capacity overflow` 变成
   "`内部 panic：crates/…:行`" ✓ —— 一轮就能看到**是哪个 `with_capacity`** ✓；
2. 再顺着那个点找**僵尸写**的来源 ✓（谁在对象释放后还留着引用并写它 ✓）—— 第 82 轮的凶手栈
   （`run_class_body`／`build_class_native` ✓）＋ 本轮的"僵尸写"结论合起来，范围已经很窄 ✓。

**④ 闸门与数字** ✓：`cargo test --workspace` ✓、0 警告 ✓、`check.py` 12/12 ✓、`CX-8` ✓、夹具守卫 ✓、
语料下限 182 ✓、`stability` ✓、`selftest` ✓、`t_ab_1` ✓、逐字节 4/4 ✓、对拍普通／`DANGLING` ✓；
`heap_and_concurrency` 3/4 ✗（那条既有、间歇缺陷 ✓）。判据① **27.4%**（172 ÷ 628 ✓）、上限 **159** ✓、
族：`-6` **116** ✓、`eval` 76 ✓、`annotationlib` 28 ✓、`_struct` 19 ✓。

#### 第 83 轮：🎯 病根**再钉一层** ✓ —— 类体命名空间字典**一造出来就是烂的** ✓（`alloc` 把仍在用的内存发了出去）

**① 从上一轮的凶手栈往下走** ✓（`lookup_in_mapping` ← `run_class_body` ← `build_class_native` ✓）：
加三处**临时诊断** ✓（崩掉时 harness 会把 stderr 记进"事故"字段 ✓ ⇒ 看得见 ✓）：
- 进类体时打印命名空间指针 ✓、`class_body_frame` 里再打一次 ✓、崩点（`entries()` 被读时 ✓）
  打印**字典自己是谁 ＋ 它自诉的 `len`／`cap`／`ptr`** ✓、以及**刚造好待用时**的状态 ✓。
**输出一锤定音** ✓：
```
[字典诊断] 建好待用:      dict=0x…101b90 len=137163532649041 cap=137130053448112 ptr=0x195   ← 已经是垃圾 ✗
[字典诊断] 类体 namespace=0x…101b90 globals=Some(0x…228fa0)
[字典诊断] class_body_frame namespace=0x…101b90
[字典诊断] entries 读取:   dict=0x…101b90 len=137163532649041 cap=137130053448112 ptr=0x195
```
⇒ 那个字典**在"刚造好待用"这一刻就已经烂了** ✗ —— 而它就是 `Lib/re/__init__.py` 里某个
**普通类**的命名空间 ✓（该文件**没有元类** ✓ ⇒ 走的是"新造一个空 `dict`"那一支 ✓）。

**② 结论（确凿 ✓）**：新造的 `DictObject`（`DictObject::new(ty, RefCell::new(Vec::new()))` ✓，
本应是 `len=0` ✓）读出来却是**另一个字典风格的残留头部**（`len`／`cap`／`ptr` 三件 ✓）
⇒ **`alloc` 把一块"仍在被使用"的内存交给了新对象** ✓ ⇒ 别处存在**提前释放／重复释放** ✓
（与第 82 轮那条"`DictObject` 释放后使用"是**同一件事的两面** ✓）。
两条旁证同向 ✓：那三个"荒唐长度"的十六进制里含 `__cod__`／`name`（内存被字符串复用 ✓）、
两次运行值不同 ✓。

**③ 临时诊断已撤** ✓（三处打印 ＋ 一个只读借用访问器 ✓），`crates/` 只留第 82 轮那 13 行**注记** ✓
（纯注释 ✓、不动行为 ✓）。**本轮未落地行为改动** ✓（如实 ✓）—— 病灶已钉到"`alloc` 发出在用内存" ✓，
但**是哪一处提前释放**还没定位 ✗。

**④ 下一轮施工图（最后一层 ✓，且现成工具在手 ✓）**：给 `alloc`／`dealloc` 加一发**存活集自查** ✓ ——
`Instance` 里**本来就有** `live: RefCell<HashSet<usize>>`（`PYAWA_QUARANTINE` 用的那套 ✓）✓
⇒ `alloc` 一拿到块就查"这个地址是否仍在存活集里" ✓，是 ⇒ **立刻具名报出"重复发放"** ✓
（带对象类型名 ✓），`dealloc` 处同理查"释放不在存活集里的地址" ✓ ⇒ **一轮就能定位是谁提前释放的** ✓。

**⑤ 闸门与数字** ✓：`cargo test --workspace` ✓、0 警告 ✓、`check.py` 12/12 ✓、`CX-8` ✓、夹具守卫 ✓、
语料下限 182 ✓、`stability` ✓、`selftest` ✓、`t_ab_1` ✓、逐字节 4/4 ✓、对拍普通趟与
`PYAWA_DANGLING=1` ✓；判据① **27.4%**（172 ÷ 628 ✓）、上限 **159** ✓、族：`-6` **116** ✓、
`eval` 76 ✓、`annotationlib` 28 ✓、`_struct` 19 ✓。

#### 第 82 轮：🎯 `-6` 族的**病根抓到了** ✓ —— `RUST_BACKTRACE=1` 直接把凶手栈打了出来 ✓（释放后使用）

**① 一招破局** ✓：UB 检查那种 panic 是**非展开**的 ✓ ⇒ `catch_unwind` 抓不到 ✗、`PYAWA_QUARANTINE`／
`PYAWA_DANGLING` 也一样只会 abort ✗。但**它仍然是 panic** ✓ ⇒ `RUST_BACKTRACE=1` 有效 ✓：
```
4: core::ptr::copy_nonoverlapping::<(NonNull<Header>, NonNull<Header>)>
6: <[…;(NonNull<Header>, NonNull<Header>)]>::to_vec
9: <pyawa_core::builtin_objects::DictObject>::entries          ← 我们
10: pyawa_core::executor::lookup_in_mapping                    ← 我们
11: pyawa_core::executor::execute::{closure#1}
13: pyawa_core::executor::run_class_body                       ← 我们
14: pyawa_core::classes::build_class_native                    ← 我们
```

**② 病根（确凿 ✓）**：`DictObject::entries` 只是 `self.entries.borrow().clone()` ✓ —— 唯一能让 std 的
`copy_nonoverlapping` 违反前置条件的解释是：**这个 `DictObject` 已经被释放**、它那块内存被别的东西复用 ✓。
而"复用者"是**字符串** ✓：上一轮那些"荒唐长度"的十六进制里含 `__cod__`／`name` ✓
⇒ **释放后使用** ✓ 完全自洽 ✓（也解释了为什么两次运行值不同 ✓）。

**③ 诊断试过、如实撤掉** ✓：在 `entries()` 里查 `Vec` 自诉的 `len`／`capacity`／指针是否自洽 ✓ ——
**不响** ✗（对象整体悬垂时读到的字段可能"自洽" ✓），而且 `entries()` 在**查找热路径**上 ✓
⇒ 白付开销 ✗ ⇒ 撤掉守卫 ✓、改成**注记**（13 行、纯注释 ✓，把上面那条凶手栈与结论钉在代码里 ✓）。

**④ 下一轮施工图（已窄到一侧 ✓）**：从 `run_class_body`／`build_class_native` 这一侧核
**类体命名空间字典的所有权** ✓ —— 类体帧的 `namespace`（`class_body_frame` 里 incref ✓）、
`build_class_native` 里那个命名空间字典的建立与释放 ✓、以及 `lookup_in_mapping` 走的
`globals`／`builtins`／`namespace` 三层各自的引用 ✓，找**谁提前释放了它** ✓。
（`Lib/re/__init__.py` 顶层就能触发 ✓、`_sre` 缺失也仍触发 ✓ ⇒ 与导入失败路径无关 ✓。）

**⑤ 闸门与数字** ✓：`cargo test --workspace` ✓、0 警告 ✓、`check.py` 12/12 ✓、`CX-8` ✓、夹具守卫 ✓、
语料下限 182 ✓、`stability` ✓、`selftest` ✓、`t_ab_1` ✓、逐字节 **4/4** ✓；对拍普通趟 ✓、
`PYAWA_DANGLING=1` 这趟红 ✗ —— 差异用例是 **`class_keywords`** ✓，正是第 347 轮起那条**既有、间歇**缺陷 ✓
（本轮改动**纯注释 13 行** ✓、不动行为 ⇒ 非本轮引进 ✓）。判据① **27.4%**（172 ÷ 628 ✓）、上限 **159** ✓、
族：`-6` **116** ✓、`eval` 76 ✓、`annotationlib` 28 ✓、`_struct` 19 ✓。

#### 第 81 轮：那两张脸的**案发地缩到 `re/__init__.py`** ✓；顺带查清 `re` 现在**根本进不去**（缺 `_sre`）✓

**① 荒唐长度**是 ASCII 文本** ✓ **（把四个实测值摊成十六进制 ✓）**：
```
429520055404197360 = 0x5f5f636f645f5f0…   ⇒ 字节 5f 5f 63 6f 64 5f 5f = "__cod__"（像 "__code__" ✓）
2081334018578144   = 0x764f656d616e0…    ⇒ 含 6e 61 6d 65 = "name" ✓
```
⇒ 长度字段是**从字符串载荷里读出来的** ✓（类型混淆／错位 ✓）；而且**两次运行值不同** ✓
（`508263012064` → `541444545584` ✓）⇒ 不是某个活对象的稳定字段 ✓，更像读到了**错位／已释放**的内存 ✓。

**② 我们自己的代码里**没有** `copy_nonoverlapping` ✓**（`crates/` 三个 crate 全查 ✓）⇒ 那条 UB 是
**std 内部**的复制路径（`Vec`／`String` 的 `extend_from_slice`／`push_str`／`insert` 之类 ✓）收到了
**坏切片** ✓ ⇒ 病灶在我们交给 std 的**指针 ＋ 长度**上 ✓（不是某个手写的 `unsafe` 拷贝 ✓）。

**③ 案发地缩到 `Lib/re/__init__.py` 的**自身**顶层路径** ✓（本轮最有用的推进 ✓）：
- `re` 是 **Python 层实现** ✓（`Lib/re/{__init__,_compiler,_parser,_constants}.py` ✓）；
- 逐个导入定位 ✓：`re._constants` → UB ✓、`re._parser`／`re._compiler`／`re` → 荒唐分配 ✓；
- 先排掉了"失败路径"这个假设 ✓：造一个最小包（`pkg/__init__.py` 里 `from . import mod` ✓、
  `mod.py` 里 `from _sre import …` ✓ 缺依赖 ✓）⇒ **干净** `ModuleNotFoundError` ✓，不崩 ✓；
- 但给 `re` 放一个 `_sre` **桩**之后再 `import re` ✓ ⇒ **仍然 UB** ✓
  ⇒ 说明它**不是**"缺 `_sre` 的失败路径" ✓，而是 `re/__init__.py` **自己的顶层代码**就能触发 ✓。

**④ 一个必须如实说的现状** ✓：`_sre` 在 `crates/` 里**没有实现** ✓（`grep "_sre"` 无 ✓）⇒
`from _sre import MAXREPEAT, MAXGROUPS` 必然失败 ✓ ⇒ **`re` 这条链现在根本走不完** ✓
⇒ 上限榜里凡是依赖 `re` 的模块（`argparse`／`asyncio`／`textwrap`／`_markupbase` … ✓）
**既缺 `_sre`、又撞内存缺陷** ✓ —— 两件都得做 ✓，不能只修一件 ✓。

**⑤ 本轮未落地代码改动** ✓（如实 ✓）：病灶已缩到"`re/__init__.py` 顶层 ＋ 交给 std 的坏切片" ✓，
但**改哪一处**还没定位到具体行 ✗（我们代码里没有 `copy_nonoverlapping` ⇒ 要顺着"谁交出了坏切片"查 ✓）
—— 在剩余上下文里硬塞不安全 ✓ ⇒ 记进下一轮的施工图 ✓：给按 Python 数据算容量的分配点加一发
**具名守卫** ✓（照第 359 轮"标签创建处"那招 ✓：荒唐值 ⇒ 报出**是哪个点**、值多少 ✓），
把 abort 变成**具名错误** ✓，下一轮就能一次定位 ✓。

**⑥ 闸门与数字** ✓：`cargo test --workspace` ✓、0 警告 ✓、`check.py` 12/12 ✓、`CX-8` ✓、夹具守卫 ✓、
语料下限 **182** ✓、`stability` ✓、`selftest` ✓、`t_ab_1` ✓、逐字节 4/4 ✓、对拍普通／`DANGLING` ✓；
`PYAWA_QUARANTINE=1` 与 `heap_and_concurrency` 是那条既有、间歇缺陷 ✓。
判据① **27.4%**（172 ÷ 628 ✓）；上限 **159** ✓；族：`-6` **116** ✓ ← 最大 ✓、`eval` 76 ✓、
`annotationlib` 28 ✓、`_struct` 19 ✓。

#### 第 361 轮：🎉 布尔条件的**落点**修好了 ✓ —— 编译期 panic 整类消失 ✓（逐字节 4/4 绿 ✓、语义语料通过 ✓）

**① 这一轮把上一轮"只差一格 oparg"补完了** ✓。上一轮撤回时留下的两个抓手（1 行复现 ✓、16 行证伪语料 ✓）
本轮都用上了 ✓，关键是**别猜、去读接线** ✓：给 `emit_test_bare` 的布尔分支加一发探针 ✓
（`PYAWA_BOOLOP_DEBUG=1` ✓）后一眼看清参照与我们的**口径差**：

- 我们把"非末操作数为假"一律送去 `target` ✗ —— 那是"条件出口" ✓，可 `(a and b) or c` 那种情形里，
  `and` 为假**不**意味着整个条件为假 ✓（后面还有 `or c` ✓）⇒ **外层还要继续** ✓；
- **照参照码元分开两条口径** ✓（`if a and b or c` 的参照布局：`a` 假 ⇒ `POP_JUMP_IF_FALSE` 落到
  **下一个操作数 `c`** 那一格 ✓；`b` 真 ⇒ `POP_JUMP_IF_TRUE` 落到**体** ✓）：
  * **`and`** ✓：非末操作数为假 ⇒ 跳"and 的假出口" —— `jump_if_true == false` ⇒ 条件出口 `target` ✓；
    `jump_if_true == true` ⇒ **本子式之后**（调用方给了 `cleanup` 就用它 ✓，否则当场开一个并在本分支
    末尾落点 ✓ —— 参照的 `L1` 正是这一格 ✓）；
  * **`or`** ✓：非末操作数为真 ⇒ 跳"本子式之后"（`fresh` ✓）；为假 ⇒ 继续**下一个操作数** ✓
    （各给一个落点 ✓，这样内层若是 `and`，它的"为假"就有处可去 ✓）；
  * 调用方传进来的 `cleanup` **交给末操作数** ✓ ⇒ 第 359 轮那条"外层标签没人落点"的 panic 消失 ✓。

**② 验证** ✓（四件齐 ✓）：1 行复现 ✓、**16 行证伪语料** ✓（第 359 轮正是它把那一版当场证伪的 ✓，
这版通过 ✓）、`pyawa-core --test compile` **逐字节 4/4 绿** ✓（含上一轮唯一失败的那格
`if a and b or c` ✓）、`code_layout` ✓；新语料 `boolop_condition_landing.py`（32 行 ✓）两侧逐字同 ✓，
对拍 **181 → 182** ✓，普通与 `PYAWA_DANGLING=1` 两趟**全绿** ✓。

**③ 影响（实测 ✓）**：编译期那类 panic **整类消失** ✓ —— 先前崩在这里的
`textwrap`／`argparse`／`asyncio`／`_markupbase`／`_pylong` ✓ 现在都**编得过**了 ✓，
改而撞上**运行期内存缺陷** ✓（第 358 轮摊开的两张脸 ✓）：`memory allocation of <荒唐大数> bytes failed` ✓
与 `unsafe precondition(s) violated: ptr::copy_nonoverlapping …` ✓ ⇒ 上限榜 `-6` 族 **96 → 116** ✓
（**变多** ✓ —— 因为更多模块**走得更远**才崩 ✓）。**上限仍 159** ✓、判据① 仍 **27.4%**（172 ÷ 628）✗
—— 这一修**没有**变成 import 数 ✓，但把"编译不过"这一整类清掉了 ✓。

**④ 闸门实况** ✓：`cargo test --workspace` ✓、0 警告 ✓、`check.py` 12/12 ✓、`CX-8` ✓、夹具守卫 ✓、
语料下限 **182**/112 ✓、`stability` ✓、`selftest` ✓、`t_ab_1` ✓、逐字节 4/4 ✓、`code_layout` ✓；
`PYAWA_QUARANTINE=1` 与 `heap_and_concurrency` 是那条既有、间歇缺陷 ✓（每轮如实记 ✓）。

**⑤ 下一站** ✓（据实 ✓）：`-6` 族的**两个内存缺陷** ✓ ——
1. `ptr::copy_nonoverlapping` 的 unsafe 前置条件被违反 ✓（`re`／`argparse`／`asyncio` ✓）；
2. "长度字段被读成垃圾"⇒ 荒唐分配 ✓（`textwrap`／`_markupbase`／`_pylong` ✓，数值形如 ASCII `_` ✓
   ⇒ 像是把**字符串数据**当**长度字段**读 ✓ ⇒ 类型混淆／读到已释放对象 ✓）。
这两处都**不再是编译期** ✓、都能用"具名诊断 + 最小复现"的老路子推进 ✓。

#### 第 360 轮：`and`／`or` 两条短路口径**按参照补齐**（语义已对 ✓）；逐字节还差**一格 oparg** ✗ ⇒ 按纪律撤回 ✓

**① 这一轮的进展（比前两轮都近 ✓）**：照参照码元重写 `emit_test_bare` 的布尔运算分支 ✓ ——
把"合取（`and`）"与"析取（`or`）"**分开** ✓：
- **`and`** ✓：任一非末操作数为**假** ⇒ 整个 `and` 为假 ⇒ 跳"这个 and 的假出口" ✓ ——
  `jump_if_true == false`（`if` 的口径）⇒ 直接跳条件自己的出口 `target` ✓（参照实测
  `drop_whitespace ⇒ POP_JUMP_IF_FALSE L3` ✓）；`jump_if_true == true` ⇒ 跳**外层给的落点** ✓；
- **`or`** ✓：非末操作数为**真** ⇒ 跳"本子式之后" ✓；为**假** ⇒ 继续**下一个操作数** ✓ ——
  每个非末操作数各给一个"下一个操作数"的落点 ✓（这样内层若是 `and` ✓，它的"为假"就有处可去 ✓）；
- 调用方传进来的 `cleanup` **交给末操作数** ✓ ⇒ 第 359 轮那条"外层标签没人落点"的**编译期 panic 消失** ✓。

**② 结果（如实 ✓）**：1 行复现（`target/lb1.py` ✓）与**第 359 轮那条 16 行证伪语料** ✓
（`or`／`and`／嵌套／四种实参 ✓）**两侧全部逐字同** ✓ ⇒ **语义这次是对的** ✓（第 359 轮那版当场
被它证伪 ✗，这版通过 ✓）；**但** `pyawa-core --test compile` 逐字节对拍还差**一格** ✗：
```
用例 "if a and b or c:\n    x = 1\n"
  我们： (12, POP_JUMP_IF_FALSE, Some(17))     ← 极性与参照**一致**了 ✓
  参照： (12, POP_JUMP_IF_FALSE, Some(9))     ← 只差落点的**远近**（一个 oparg 的差 ✓）
```
⇒ 也就是说：**极性已经对齐 ✓、只差"那一跳落在哪一格"** ✓ —— 病灶从"整族编译期崩溃"缩到
**一个 oparg** ✓（这就是下一轮 5 分钟的活 ✓）。

**③ 按纪律撤回** ✓：闸门红则撤 ✓ ⇒ `emitter.rs` 原样撤回 ✓（逐字节对拍 **4/4 绿** ✓、
1 行复现照旧 panic ✓ —— 未修好的不装作修好 ✓）。**第 359 轮落地的诊断**（标签创建处 ✓）**保留** ✓
—— 正是它把这三轮的路照亮了 ✓。

**④ 闸门实况** ✓：`cargo test --workspace` ✓、0 警告 ✓、`check.py` 12/12 ✓、`CX-8` ✓、夹具守卫 ✓、
语料下限 181 ✓、`stability` ✓、`selftest` ✓、`t_ab_1` ✓、逐字节 4/4 ✓、`code_layout` ✓；
`PYAWA_QUARANTINE=1` 与 `heap_and_concurrency` 是那条既有、间歇缺陷 ✓。

**⑤ 数字** ✓：判据① **27.4%**（172 ÷ 628 ✓）；上限 **159** ✓；
族：`-6` **96** ✓ ← 最大（其根因已缩到**一个 oparg** ✓）、`eval` 76 ✓、`annotationlib` 28 ✓、
`_struct` 19 ✓；语料 **181** ✓。

---

#### 目标 80 轮的**总账**（据实 ✓，`AGENTS.md`「完成度如实」）

**判据①**：**27.2% → 27.4%**（171 → 172 ÷ 628 ✓，阈值 **67%** ✗）—— **没有达到** ✗。
**判据②**（对拍无新差异 ✓）：**成立** ✓ —— 语料从 **157 → 181** ✓，普通与 `PYAWA_DANGLING=1` 两趟
**全绿** ✓、0 已知差异 ✓；`PYAWA_QUARANTINE=1` 那一趟自第 347 轮起**一直是同一条既有、间歇缺陷** ✓
（每轮如实记 ✓，非本地改动引进 ✓）。

**这 80 轮里真正修好、并逐字节／逐字证实的** ✓：
1. `P3-20`（第 353 轮 ✓）：嵌套 `try` 的**处理块路径**按块深度分流 ✓ —— 压在 **101** 个模块上的老根 ✓，
   修后上限榜那一族**整族消失** ✓；
2. `FellOffEnd`（第 357 轮 ✓）：`emit_block` 进出**嵌套块**时保存／还原"作用域收尾"标志 ✓ ——
   修后 **104** 个模块的族**整族消失** ✓（上限**158 → 159** ✓）；
3. `str`／数值方法一批 ✓（`rfind`／`index`／`rindex`／`rpartition`／`isprintable`／`istitle`／
   `translate`／`int.bit_count`／`float.is_integer`／`float.as_integer_ratio` ✓，全部两侧逐字同 ✓）；
4. 对拍 harness 的临时文件**按年龄清** ✓（并行安全 ✓）、`FellOffEnd`／`-6` 的**具名诊断** ✓
   （代码对象名 ✓、标签创建处 ✓）—— 后两件是"照亮问题"的投入 ✓，它们直接决定了后面几轮的走向 ✓。

**没做到的** ✗（同样是总账的一部分 ✓）：`P3-12` 的 `.pyac` 容器与陈旧判定等**尚未接线** ✓；
`eval`／`exec`（+76 ✓）因那条**重入借用**仍未落地 ✓；`-6` 族（96 ✓）根因已缩到**一个 oparg**
（外加两处内存缺陷：`ptr::copy_nonoverlapping` 的 UB ✓、长度字段被读成垃圾的荒唐分配 ✓）但**未修** ✓；
`annotationlib` 的 PEP 649（28 ✓）、`_struct`（19 ✓）等族仍原样 ✓。
⇒ **一句话**：这 80 轮把**两堵最大的墙拆了**、把**第三堵墙缩到一个 oparg**，但判据① **仍差 39.6 个
百分点** ✓ —— **M3 判据① 未成立** ✓，不声称完成 ✓。

#### 第 359 轮：`-6` 族的**编译器崩溃**查到"最后一格" ✓（1 行复现 + 标签创建处）；候选修法**语义不对** ✗，撤回 ✓

**① 让那条内部断言**自己说出病灶** ✓（落地 ✓ `feat(diag)`）**：报错是
`跳转目标标签 40 从未落点（跳转指令在码元 657，位点 …）` ✗ —— 光看它只能干猜 ✓。本轮给**每个标签
记下创建处** ✓（`new_label` 加 `#[track_caller]` ＋ `label_origins` ✓，纯诊断 ✓、不动发射 ✓），
于是一行复现直接报出答案 ✓：
```
跳转目标标签 6 从未落点（标签创建处：crates/pyawa-core/src/compile/emitter.rs:4247:30；…）
```

**② 根因（确凿 ✓）**：`emit_test_bare` 遇到**布尔运算**时 ✗
（`emitter.rs` 的 `BoolOp` 分支 ✓）：
```rust
let fresh = self.new_label();
for inner in &values[..len-1] { self.emit_test_bare(inner, !conj, fresh, None)?; }
return self.emit_test_bare(last, jump_if_true, target, Some(fresh));   // ← 传进来的 cleanup 丢了 ✗
```
⇒ 当**某个操作数本身又是布尔运算**时，**外层传进来的 `cleanup` 标签没人落点** ✗ ⇒ 收尾断言 ⇒
**编译期 panic** ✓。1 行复现（`target/lb1.py` ✓，`textwrap.py:313` 的同形压缩 ✓）：
```python
def f(a, b, c, d):
    if a or (not d or a and b == 1) and b <= c:
        print("hit")
    print("after")
```
（单个操作符都不崩 ✓，两层叠起来才露 ✓ —— 与上一轮的形状二分一致 ✓。）

**③ 候选修法试过、**语义证伪** ✓（如实 ✓）**：在 `BoolOp` 分支末尾把传进来的 `cleanup` 落点
（与叶子那处同一位置 ✓）⇒ 编译**不再崩** ✓，1 行复现也与参照逐字同 ✓ —— **但**
新语料 `boolop_condition_landing.py`（16 行、含 `or`／`and`／嵌套／多种实参 ✓）**当场证伪** ✗：
参照 `miss` 而我们 `hit` ✓ ⇒ **落点该在"外层合取式判定完成"的位置** ✓，不是"整段条件测完"的位置 ✗
⇒ 按纪律**撤回** ✓（语料也撤 ✓，不留红用例 ✓），根因注释留在代码里 ✓。

**④ 这一修（若做对）值多少：上限榜上 `-6` 从 **96 → 113** ✓**（临时打上修法后测的 ✓）——
⇒ 说明这些模块**确实是在这一处崩的** ✓，修对之后它们会**走得更远**（撞下一堵墙 ✓）。
**上限仍是 159** ✓（都还没到"能 import"那一步 ✓）。

**⑤ 闸门实况** ✓：`cargo test --workspace` ✓、0 警告 ✓、`check.py` 12/12 ✓、`CX-8` ✓、夹具守卫 ✓、
语料下限 181 ✓、`stability` ✓、`selftest` ✓、`t_ab_1` ✓、`pyawa-core --test compile` **逐字节 4/4 绿** ✓、
`code_layout` ✓ —— `PYAWA_QUARANTINE=1` 与 `heap_and_concurrency` 仍是那条既有、间歇缺陷 ✓。

**⑥ 数字** ✓：判据① **27.4%**（172 ÷ 628 ✓）；上限 **159** ✓；
族：`-6` **96** ✓ ← 最大（其根因本轮已定 ✓）、`eval` 76 ✓、`annotationlib`（`t""`）28 ✓、`_struct` 19 ✓；
语料 **181** ✓（不动 ✓）。

**⑦ 下一轮的施工图** ✓（只剩一轮 ✓，据实列在这里 ✓）：
1. `BoolOp` 分支的 `cleanup` **落点位置**要按"外层合取／析取的判定点"定 ✓ —— 已有
   1 行复现 ＋ 16 行证伪语料 ＋ 标签创建处三元组 ✓，照着参照码元**逐条对齐**即可 ✓；
2. 之后是两处**内存缺陷** ✓：`ptr::copy_nonoverlapping` 的 unsafe 前置条件被违反 ✓（`re`／`argparse`／
   `asyncio` ✓）、以及"长度字段被读成垃圾"导致的**荒唐分配** ✓（`textwrap`／`_markupbase` ✓，
   数值形如 ASCII `_` ⇒ 像是把字符串数据当长度读 ✓）。

#### 第 358 轮：那堵"最后一堵墙"（`-6` × 96）**摊开成两张脸** ✓ —— 一张**内存 UB** ✓、一张**编译器崩溃（1 行复现）** ✓

**① 拿到真凭实据** ✓：第 357 轮之后，上限榜最大的族是 `子进程退出码 -6`（**SIGABRT** × **96** ✓）。
先前只知道"崩" ✗ —— 本轮**直接跑**那台子进程并**读它的 stderr** ✓（工具那条路只印第一行 ✗），
于是读到真正的死因 ✓：
```
memory allocation of 508263012064 bytes failed
```
⇒ 一次 **508 GB** 的分配 ✗ ⇒ 某个"长度"字段被读成了垃圾 ✓（`508263012064 = 0x765F…` ✓ —— 形如一串
ASCII `_`（`0x5F` ✓）⇒ 像是把**字符串数据**当成了**长度字段**读 ✓ ⇒ 类型混淆／读到已释放对象 ✓ ✓）。

**② 逐模块二分** ✓（把 argparse 的依赖一个个单独 import ✓）：`os`／`io`／`collections`／`itertools`／
`operator` ✓ 正常；`warnings`／`gettext`／`functools`／`copy` ✓ 是**正常的 Python 层失败** ✓（有观测块 ✓）；
**`re`** ✗ 与 **`textwrap`**／`argparse` ✗ 是**真崩** ✓ ⇒ 两张脸 ✓：
- **`re`** ✗：`unsafe precondition(s) violated: ptr::copy_nonoverlapping requires that both pointer
  arguments are aligned and non-null …` ✓ ⇒ **内存 UB** ✓（指针越界／错位 ✓）；
- **`textwrap`** ✗：**我们自己的断言** ✓
  `跳转目标标签 40 从未落点（跳转指令在码元 657，位点 = 第 313 行 25-31 列）` ✓
  ⇒ **编译器崩溃** ✗ ⇒ 编译期就 abort ⇒ 整族 `-6` 的一大半来源 ✓ ✓。

**③ 把编译崩溃压到 1 行复现** ✓（本轮最硬的产出 ✓）：
```python
def f(a, b, c, d):
    if a or (not d or a and b == 1) and b <= c:
        print("hit")
    print("after")
```
（`textwrap.py` 第 313 行那条跨行 `or/and` 条件的**同形压缩** ✓。）形状二分结果 ✓（全在
`PYAWA_LAND_DEBUG=1` 与逐例对照下得到 ✓）：
- `a or b` ✓、`(a or b) and c` ✓、`a or (b and c) and d` ✓、`a or (not d or a) and b <= c` ✓ 都**正常**；
- `a and b == 1 and not d` ✓ 单独也**正常**；
- **两者嵌套**（`a or (not d or a and b == 1) and b <= c` ✓）才崩 ✗ ⇒ **状态交互** ✓，
  不是某一单个操作符 ✓。
**定位到登记点** ✓：条件出口的"落点"只在
`to_target && self.collect_condition_exits` 时才登记 ✓（`emitter.rs` 的 `condition_landings.push` ✓），
而探针显示崩的那一刻 **`condition_landings` 是空的** ✓ ⇒ 那个跳转的去处**根本没登记** ✗
⇒ 病灶就在"该登记却没登记"的这一格 ✓，下一轮从这里下手 ✓（**已有 1 行复现 + 登记点 + 空表证据** ✓）。

**④ 未落地任何代码改动** ✓（如实 ✓）：本轮试了 `list.sort`（发现**整个方法都缺** ✗ —— 与 `sorted`
不同 ✓，`sorted` 带 `key`／`reverse` 都已能用 ✓）⇒ 它是**列表类型的方法**，得进 `pyawa-core` 实现 ✓，
在剩余的轮次里要连"比较器 ＋ `key`／`reverse` ＋ 逐字节闸门"一起做 ✗ ⇒ 本轮**不塞** ✓，
记进下一轮的候选 ✓。**`crates/` 与语料一字未动** ✓，闸门全绿 ✓。

**⑤ 闸门实况** ✓：`cargo test --workspace` ✓、0 警告 ✓、`check.py` 12/12 ✓、`CX-8` ✓、夹具守卫 ✓、
语料下限 181 ✓、`stability` ✓、`selftest` ✓、`t_ab_1` ✓、普通／`PYAWA_DANGLING=1` 对拍 ✓ ——
`PYAWA_QUARANTINE=1` 与 `heap_and_concurrency` 仍是那条既有、间歇缺陷 ✓（每轮如实记 ✓）。

**⑥ 数字** ✓：判据① **27.4%**（172 ÷ 628 ✓）；上限 **159** ✓；
族：`-6` **96** ✓ ← 最大 ✓（其中至少一大半是**编译器崩溃** ✓）、`eval` 76 ✓、`annotationlib`（`t""`）28 ✓、
`_struct` 19 ✓；语料 **181** ✓ 不动 ✓。

#### 第 357 轮：🎉 **`FellOffEnd` 那一族（104 个模块）修好了** ✓ —— 一处**保存／还原**就够 ✓（逐字节对拍 4/4 绿 ✓）

**① 施工图变成修法** ✓（承接第 355 轮撤回时留下的三条逐字节纪律 ✓）：试了四处之后（第 355 轮台账逐条记着 ✓），
真正对的那一处是**最朴素的一处** ✓：把 `emit_block` 里那个"作用域还要不要收尾"的标志
（`epilogue_needed` ✓）**进出嵌套块时保存／还原** ✓ ——
```rust
let saved_epilogue_needed = self.epilogue_needed;
let nested_block = self.block_depth > 1;   // 进块时
…
if nested_block { self.epilogue_needed = saved_epilogue_needed; }   // 出块时
```
理由一句话 ✓：那个标志说的是"**作用域**末尾要不要补收尾" ✓，而 `raise`／`return` 两条语句臂把它置假 ✗
—— 那是**对"本块"说的** ✓；**嵌套块**（`if` 的体／`for` 的体／`try` 的套体 … ✓）里的终止**替不了
作用域的账** ✗。先前不还原 ⇒ 末尾语句的**内部**终止过 ⇒ 标志留在假 ⇒ **漏发收尾** ⇒ 掉底 ✓。

**② 为什么这次不用试第四遍** ✓：前三处（改判据 `!block_terminates` ✗、尾位 `if` 置假 ✗、
循环臂置真 ✗）之所以红 ✗，正是因为它们在**各条路**上打补丁 ✗，而**参照的账是一本** ✓
—— "作用域收尾"只该由**作用域**记 ✓。把账本修好，四条逐字节用例与四个最小复现**同时**对上 ✓ ✓。

**③ 验证** ✓：`pyawa-core --test compile` 的逐字节对拍 **4/4 绿** ✓、`code_layout` ✓、
四个复现（`if` 体 raise ✓／两层 `for` ✓／类里 classmethod 同形 ✓／第 353 轮那条 `P3-20` ✓）
**全部与参照逐字同** ✓；新语料 `implicit_return_tail.py` **30 行**（含 `else`／类体／双层嵌套变体 ✓）
两侧逐字同 ✓；对拍 **180 → 181** ✓，**普通／`PYAWA_DANGLING=1` 两趟全绿** ✓。

**④ 上限榜** ✓：`码元跑完却没有 RETURN_VALUE`（**104** ✓）**整族消失** ✓；上限 **158 → 159** ✓。
现在撞的是 **`子进程退出码 -6`（SIGABRT ✓）× 96** ✗ —— **就是那条一直挂在闸门上的既有缺陷** ✓
（`RefCell already borrowed` 一族 ✓），它同时挡着 `eval`（76 ✓）⇒ **它是最后一堵墙** ✓。

**⑤ 数字（如实 ✓）**：判据① 仍 **27.4%**（172 ÷ 628 ✓，连测两次）—— 这一修把 104 个模块**送过了掉底** ✓，
但它们立刻撞上 `-6` ✓ ⇒ 还差那堵墙才能变成 import 数 ✓。**本轮确实修好了一件事** ✓
（逐字节对拍可证 ✓），但判据没跳 ✓ —— 两句都如实说 ✓。
**合起来看** ✓：`P3-20`（第 353 轮 ✓，+101 越过老墙 ✓）与这一轮（+104 越过掉底 ✓）**都不是**判据跳变 ✓，
它们是**把最后那堵墙露出来**的两步 ✓。

