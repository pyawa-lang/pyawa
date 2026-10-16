> 本卷是 `docs/ROUNDS.md` 分卷台账之一（**非规范**、只增不改；卷目见该索引）✓

> **收口归档说明**（2026-10-11 ✓）：本卷由 `NEXT.md` 收口回单页时**逐字搬出** ✓（原文照录 ✓、一字不改 ✓）。
> 覆盖轮次区间 **第 579–720 轮** ✓；共 **81 个小节**（80 个「## 本轮」✓ ＋ 1 个「## 里程碑（684）」✓）、
> **1210 行** ✓ —— 与源文件 `NEXT.md` 第 14–1223 行的**小节数／行数逐项一致** ✓（搬前先自查 ✓、确认保全后才从单页删 ✓）。
> **未搬入本卷** ✗：`NEXT.md` 里过期的「## 现在」与旧「## 下一条命令」两块 —— 它们是被收口重写**取代**的状态块 ✗，
> 不是逐轮复盘 ✓；现行版本只在新单页 `NEXT.md` ✓。

## 本轮（579）：`_sre` 底层**已落地** ✓

- `crates/pyawa-stdlib/src/_sre_module.rs`：常量（`MAGIC`／`CODESIZE`／`MAXREPEAT`／`MAXGROUPS` ✓）
  ＋ 四个 `cased/tolower` ✓ ＋ **`compile_raw(pattern, flags) -> id`** ✓（`regex` crate 直编**源串** ✓，
  忽略 `_compiler` 给的 SRE 字节码 ✓）＋ **`match_raw(id, string, kind) -> "s,e;g1s,g1e;…" | None`** ✓；
- 依赖边：`regex = "1"` **只在 `pyawa-stdlib`** ✓（核心 crate 里那笔误加的依赖边**连理由注释一起挪走** ✓ —— 一处真相 ✓）；
- **验收** ✓：六例与参照**逐例一致**（`diff` 为空 ✓）；`tools/quickcheck.sh` ✓；`tools/slowcheck.sh` 十项全绿 ✓。

## 本轮（580）：`_sre` 的**字符偏移**真 bug 修 ✓

`match_raw` 原先直接把 `regex` crate 的**字节**偏移当结果 ✓，而参照 `re` 的 `span()`／`start()`／`end()`
一律是**字符**偏移 ✓ ⇒ 非 ASCII 整片错位（`\w+` 对 `"αβγ δ"`：旧 `0,6` ✗ ⇒ 新 `0,3` ✓）。
修法：`char_offset(text, byte)`（`text.get(..byte)` 的前缀字符数 ✓，理论上边界必在字符处 ✓）。
护栏：`crates/pyawa-runtime/tests/_sre_shapes.rs`（11 例 ✓，参照值由本机 `python3` 3.14 逐例实测 ✓：
6 条 ASCII ＋ 5 条非 ASCII／大小写不敏感 ✓）。

## 本轮（581）：目标第 2 条的判据**已实证** ✓ ＋ `id` 内建落地

**判据**（`NEXT.md` 与目标里都写着 ✓）：`PYAWA_QUARANTINE=1` 下 `meta_path_shapes` **稳定 20/20 绿** ✓
**且** builtins 命名空间 **+1** 不再触发 panic ✓ ⇒ 本轮**两项都实测通过** ✓：
```
QUARANTINE=1 × 20 次 ⇒ 20/20 绿 ✓        不带 QUARANTINE × 20 次 ⇒ 20/20 绿 ✓
（"命名空间 +1" 就是用本轮落的 `id` 当场验的 ✓ —— 它在第 571 轮正是"加一个名字就随机红"的那一笔 ✓）
```
⇒ 第 578 轮那两处修（`__del__` 特殊查找 ✓、incref 断言窗口 ✓）**经判据确认** ✓，不再是"重压 0/36"的旁证 ✓。

**本轮的 +1** ✓：`id(object)`（`builtins_module.rs` ✓，照 `repr_native` 形状 ✓、参数校验走 `need_args` ✓、
名字清单 `IMPLEMENTED` 同步 ✓）。与参照逐条一致 ✓（`id(a)==id(a)` True ✓、`id(a)!=id(b)` True ✓、`type(id(1)) is int` ✓）。
**如实范围** ✗：我们直接给**地址** ✓ ⇒ 对象释放后地址可能复用 ✗（参照只保证"生命周期内唯一" ✓，差别仅在复用后可能与旧值相同 ✗）。

## 本轮（582）：`_sre.compile` ＋ `Pattern`／`Match` 已能跑 ✓（Rust 侧 ✓）

```
_sre.compile(pattern, flags, code, groups, groupindex, indexgroup) -> re.Pattern 实例
  ├ match/search/fullmatch(string) -> re.Match 实例 | None      ✓ 已落
  └ re.Match: span/start/end(group=0) -> (int,int)/int/int      ✓ 已落（未匹配 ⇒ None ✓）
```
**关键实现事实** ✓：本 crate 是 `#![forbid(unsafe_code)]` ✗ ⇒ **不自己解 `TypeObject` 指针** ✓，
建类走 core 的**安全**入口 `build_class_from_parts` ✓（与 `class` 语句**同一条路** ✓，一处真相 ✓）；
数据放**按对象地址索引**的静态表 ✓ ⇒ **不新增载荷类型** ✗（`complex` 的教训 ✓）。
**验收** ✓：与参照**逐条一致** ✓（`re.compile("(a)(b)?").search("xaby")` ⇒ `span (1,3)` ✓、`span(1) (1,2)` ✓、
`span(2) (2,3)` ✓、`match None` ✓、`fullmatch` ✓）⇒ 已钉进 `_sre_shapes.rs` ✓（第 2 条用例 ✓）。

**未落面（下一条命令 ✓，增量落、每步验 ✓）**：
模板未知转义报错 ✗、表项回收 ✗、迭代器 `__next__` 属性面 ✗（core 侧 ✓）。
`re/__init__.py:315` 的 `Pattern = type(_compiler.compile('', 0))` ✓ —— 现在**已经有了** ✓。

## 本轮（583）：`Match.group`／`groups` ✓（命名组、未匹配组都对）

```
m.group() / m.group(0) / m.group(i) / m.group("名字") / m.group(a, b…) -> str | None | tuple
m.groups(default=None)                                                   -> tuple
```
**实现要点** ✓：`groupindex`（名字⇒组号 ✓）来自 `compile` 的第 5 个实参 ✓（`re/_compiler.py` 的 `p.state.groupdict` ✓）；
切片按**字符**下标 ✓（与 `span()` 同口径 ✓）。**验收** ✓：与参照**逐条一致** ✓（`m.group()`/`group(1)`/`group(2)`/
`group("w")`/`groups()`/未匹配 `group(2) ⇒ None`/`groups() ⇒ ('a', None)`/`group(1, 2) ⇒ tuple` ✓），
已钉进 `_sre_shapes.rs` 第 3 条用例 ✓。

## 本轮（584）：`Pattern.findall` ✓ ＋ `Match.groupdict` ✓

```
Pattern.findall(string)  ->  无组：串列表 ✓ ／ 一组：串列表 ✓ ／ 多组：元组列表 ✓（未匹配的组写**空串** ✓）
Match.groupdict(default=None) -> {名字: 文本} ✓（未匹配 ⇒ default ✓）
```
**验收** ✓：与参照**逐条一致** ✓（`['ab','ac','ad']` ✓、`['b','c']` ✓、`[('a','b'),('a','')]` ✓、
`{'x': 'a'}` ✓、`groupdict("-")` ✓）；已钉进 `_sre_shapes.rs` 第 4 条用例 ✓。

## 本轮（585）：`pos`／`endpos` ✓（窗口内匹配、下标仍相对整串）

```
Pattern.match/search/fullmatch(string, pos=0, endpos=len) ✓   Pattern.findall(string, pos, endpos) ✓
```
**做法** ✓：在 `[pos, endpos)` 这段**字符窗口**上扫 ✓，再把跨度**平移**回整串下标 ✓；
切片一律用**原文** ✗ 不能用窗口 ✓ —— 本轮就踩了这条（`findall("banana", 2)` 先给 `['a','']` ✗、
参照 `['a','a']` ✓，改用原文切片后一致 ✓）。**验收** ✓：8 条与参照**逐条一致** ✓，已钉进 `_sre_shapes.rs` 第 5 条用例 ✓。

## 本轮（586）：`Pattern.split` ✓

```
Pattern.split(string, maxsplit=0) -> list   无组 ⇒ 直接切 ✓；有组 ⇒ 组文本**插进**结果 ✓（未匹配 ⇒ None ✓）
```
**验收** ✓：7 条与参照**逐条一致** ✓（含 `maxsplit` ✓、交替组 `(,)|(;)` ⇒ `['a', ',', None, 'b', None, ';', 'c']` ✓、
**空模式** `""` 在每位切一刀 ⇒ `['', 'a', 'b', 'c', '']` ✓），已钉进 `_sre_shapes.rs` 第 6 条用例 ✓。

## 本轮（587）：`Pattern.sub`／`subn` ✓（字符串替换模板）

```
Pattern.sub(repl, string, count=0) -> str ✓        Pattern.subn(repl, string, count=0) -> (str, int) ✓
模板：\1…\99 ✓ ／ \g<名字> ✓ ／ \g<0> ✓ ／ \\ ✓ ／ \n \t \r ✓（未识别转义按参照原样保留 ✓）
```
**验收** ✓：9 条与参照**逐条一致** ✓（含 `\g<名字>` ✓、`\g<0>!` ＋ count=1 ✓、未匹配组展开成**空串** ✓、
`subn` 元组 ✓、`\\`／`\t` ✓）；护栏第 7 条用例的**预期值由实测输出生成** ✓（避免手写转义出错 ✓）。
**未接线（如实 ✓）**：**可调用替换**（`repl` 是函数 ⇒ 现在报 `TypeError` ✓）；模板里的**未知转义**
在 CPython 3.12+ 是**报错** ✗ 而我们原样保留 ✓ —— 与 `_sre.template` 一起补 ✓。

## 本轮（588）：`sub`／`subn` 接**可调用替换** ✓

```
Pattern.sub(函数, string, count=0) ✓   替换函数收到 re.Match ✓；返回非 str ⇒ TypeError ✓（与参照一致 ✓）
```
**做法** ✓：`repl` 不是 `str` 时当可调用对象 ✓ —— 造 `re.Match` 后走 core 的**公共**入口
`pyawa_core::executor::call::call_value` ✓（stdlib 里 `codecs_module`／`operator_module` 已在用 ✓，
不是新通道 ✓），随后 `instance.release(matched)`／`release(result)` 结清两处自有引用 ✓。
**验收** ✓：4 条与参照**逐条一致** ✓（`bAnAnA` ✓、`<x>` 模板与 `count=2` ✓、`m.group(1) or "?"` ✓、
返回非 `str` ⇒ `TypeError` ✓），已钉进 `_sre_shapes.rs` 第 8 条用例 ✓（预期值仍由实测生成 ✓）。

## 本轮（589）：`Pattern.finditer` ✓（真迭代器）

```
Pattern.finditer(string, pos=0, endpos=len) -> iterator ✓
```
**做法** ✓：造一批 `re.Match` ✓ 再用 core 的**公共**入口 `pyawa_core::executor::iter::iter_value` 包成迭代器 ✓
（`itertools` 也在用同一入口 ✓ ⇒ 不是新通道 ✓）；窗口与 `findall` 同款 ✓（窗口只用于扫 ✓、跨度平移回整串 ✓、
`MatchData.text` 存**原文** ✓）。
**验收** ✓：3 条与参照**逐条一致** ✓（跨度列表 ✓、`pos` 窗口 ✓、`(a)(n)?` 的组文本 `['an','an','a']` ✓），
已钉进 `_sre_shapes.rs` 第 9 条用例 ✓。**如实差异** ✗：参照 `hasattr(it, "__next__")` 为 `True` ✓ 而我们是 `False` ✗
—— 这是**迭代器类型**缺 `__next__` **属性面** ✓（`next(it)` 已可用 ✓），属 **core 侧** ✓ 不在 `_sre` ✓
⇒ 下一条命令把它补上 ✓（会影响所有迭代器 ✓，值得单独一笔 ✓）。

## 本轮（590）：`Match.expand` ✓

```
Match.expand(template) -> str ✓（与 `sub` 的模板**同一处实现** ✓ 一处真相 ✓）
```
**验收** ✓：3 条与参照**逐条一致** ✓（`[a][a][b]` ✓、无转义原文 ✓、`\g<0>!` ⇒ `ab!` ✓），
已钉进 `_sre_shapes.rs` 第 10 条用例 ✓（预期值由实测输出生成 ✓）。

## 本轮（591）：`Pattern.pattern`／`flags`／`groups`／`groupindex` ✓

`re/__init__.py` 会读 `Pattern.pattern`／`.flags` ✓ ⇒ 这四项挂进**实例字典** ✓（实例本来就有字典槽 ✓，
走普通属性通道 ✓ 不开新通道 ✓）。**验收** ✓：5 条与参照**逐条一致** ✓（`pattern` ✓、`flags=32` ✓
—— 参照 `re.compile` 默认带 `re.UNICODE` ✓ 故要显式喂 32 才是同口径 ✓、`groups` ✓、`groupindex` ✓、
匹配仍正常 ✓），已钉进 `_sre_shapes.rs` 第 11 条用例 ✓。

**本轮查实的一条事实** ✓（改变了做法 ✓）：`_sre.error` 在参照里**不存在** ✗（`re.error` ＝
`re._compiler.PatternError` ✓ 是 **Python 类** ✓）⇒ **native 层造不出它** ✗ ⇒ "模板／模式的错误类型"
这一条**不能**在 `_sre` 里对齐 ✓ —— 要么等 `re` 起来后由 Python 层包一层 ✓，要么如实登记为已知偏离 ✓。

## 本轮（592）：`Match.string`／`re`／`pos`／`endpos` ✓

同一机制 ✓（挂实例字典 ✓）：`string` ✓、`re`（＝**同一个模式对象** ✓ ⇒ `m.re is p` 为真 ✓）、
`pos`／`endpos`（反映传入的窗口 ✓）。**验收** ✓：3 条与参照**逐条一致** ✓，已钉进 `_sre_shapes.rs` 第 12 条用例 ✓
（顺带删掉已无调用者的 `new_instance` ✓ 保持 0 警告 ✓）。

**查实的下一步事实** ✓（改变了做法 ✓）：**`_sre.template` 在 `re.sub` 的必经路径上** ✓
（3.13+：`re/__init__.py:375 _compile_template` ⇒ `_sre.template(pattern, _parser.parse_template(repl, pattern))` ✓
返回一个**可调用对象** ✓ 再交给 `Pattern.sub` ✓）⇒ 要做它就得**走列表/元组/字符串的解析结构** ✓
（`_parser.parse_template` 的产物 ✗）⇒ 比前面几笔重 ✓，且要先确认那个结构的形状 ✓ ⇒ 单独一笔做 ✓。

## 本轮（593）：**core 侧 `__call__`** ✓（面很宽的真缺口）

```
实例只要 MRO 里有 __call__ 就可调用 ✓（`add(5)` ✓、`map(add, [1,2,3])` ✓、默认参数 ✓、`callable(add)` ✓）
```
**缺口的实情** ✓：core 里**一处都没有** `__call__` 的调用分派 ✗ ⇒ 任何带 `__call__` 的用户类被调用
都报 `TypeError: 'X' object is not callable` ✗；`_sre.template` 要返回的**模板可调用对象**也压在这上面 ✓。
**修法** ✓（两处，同一口径 ✓）：① `call_callable` 在"不是那几种可调用类型"时先走**属性通道**找 `__call__` ✓
（不另开分派 ✗）；② `Instance::is_callable`（`callable()` 的口径 ✓）同步对齐 ✓ ——
否则出现"`callable(x)` 为 `False` 但 `x()` 能调"的**口径分叉** ✗（第一版就是 `15 False` ✗，已修 ✓）。
**验收** ✓：4 条与参照**逐条一致** ✓，钉进**新护栏** `crates/pyawa-runtime/tests/callable_shapes.rs` ✓。

## 本轮（594）：`_sre.template` ✓（`re.sub` 那道闸的最后一块）

```
_sre.template(pattern, parsed) -> 可调用对象 ✓（`__call__(match) -> str` ✓）—— 第 593 轮的 `__call__` 刚接通 ✓
解析结构（本机实测 ✓）：`['[', 1, ']']` —— 字面量与组号交替的 list ✓（转义已展开 ✓、名字已换号 ✓）
```
**验收** ✓：4 条与参照**等价口径**逐条一致 ✓（`[a]`／未匹配组 ⇒ `[a]` ✓／`\g<0>!` ⇒ `ab!` ✓／`plain` ✓），
已钉进 `_sre_shapes.rs` 第 13 条用例 ✓。**形状差异（如实 ✓）**：参照的 `_sre.template` 返回
**`SRE_Template` 对象** ✓ 且它**自己不可调用** ✗（由 C 层 `Pattern.sub` 特认 ✓）；我们的 `Pattern.sub`
收**任意可调用对象** ✓ ⇒ 返回带 `__call__` 的对象 ✓ ⇒ 故对照脚本用 `Match.expand(同一模板)` 作**等价口径** ✓。

## 本轮（595）：`Match.lastindex`／`lastgroup` ✓

```
m.lastindex -> 最后一个**匹配上的**捕获组号 ✓（都没匹配 ⇒ None ✓）
m.lastgroup -> 该组的名字 ✓（无名 ⇒ None ✓，从 compile 的 groupindex 推 ✓）
```
**验收** ✓：5 条与参照**逐条一致** ✓（`2` ✓、未匹配 ⇒ `1` ✓、`y` ✓、`None` ✓、交替组 ⇒ `2 y` ✓），
已钉进 `_sre_shapes.rs` 第 14 条用例 ✓。**又踩一次同类脚本坑** ✓：`lastgroup` 只能从 `groupindex` 推 ✓
⇒ 护栏脚本要显式传它 ✓（第一版漏传 ⇒ 报 `None` vs `y` ✗，是脚本的错 ✓ 不是实现的错 ✓，如实记 ✓）。

## 本轮（596）：`Match` 的 `str`／`repr` ✓ ＋ **查实一个面很宽的 `print` bug** ✗

```
str(m) == repr(m) == "<re.Match object; span=(1, 3), match='ab'>" ✓（参照形状 ✓）
```
**做法** ✓：`__repr__` 与 `__str__` 挂**同一个原生** ✓（一处真相 ✓；我们的回落不走 `object.__str__ → __repr__` ✗
⇒ 两个名字都要挂 ✓）。**验收** ✓：2 条与参照**逐条一致** ✓，钉进 `_sre_shapes.rs` 第 15 条用例 ✓。

**新查实的 core bug** ✗（面很宽 ✓，已单独记 ✓ 不混进本笔 ✓）：
```
用户类：str(a) ⇒ "A-str" ✓（对 ✓）   而   print(a) ⇒ "<A object at 0x…>" ✗（参照是 "A-str" ✓）
```
⇒ `print` 的渲染通道与 `str()` **分叉** ✓（`print_native` 里明明写的是走 `object_str_native` ✓ 与 `str(x)` 同一处 ✓
—— 说明**实际生效的不是那条** ✗ 或中间还有一层 ✓）⇒ 下一笔的第一件事就是把它钉住 ✓。
**它也是本笔护栏只钉两行的原因** ✓（`print(m)` 那条仍红 ✗，不许当通过 ✓）。

## 本轮（597）：**`print` 与 `str()` 的口径分叉已修** ✓（面很宽的真 bug）

**病灶** ✓（一行读出来的 ✓）：`Instance::object_str_native`（`instance/accessors.rs:139` ✓）只认类型的
**`str` 槽** ✗ 并回落默认 `repr` ✗ —— **不查类字典的 `__str__`** ✓；而 `object_repr` 查 `__repr__` ✓
⇒ 凡覆写 `__str__` 的类：`str(a)` 对 ✓、`print(a)` 错 ✗（`<A object at 0x…>` ✗，参照 `A-str` ✓）。
**修法** ✓：`object_str_native` 先走**属性通道**找 `__str__` ✓（与 `object_repr` 对 `__repr__` 同一口径 ✓，
一处真相 ✓）。**验收** ✓：`print(a)`／`str(a)`／`print("prefix:", a)` 三条都是 `A-str` ✓（与参照一致 ✓）；
`print(m)`（`re.Match`）也跟着对了 ✓（第 596 轮那笔 `Match.__str__` 现在真的生效 ✓）。
钉进**新护栏** `crates/pyawa-runtime/tests/text_shapes.rs` ✓。
**未验的相邻口径** ✗（如实 ✓）：只有 `__repr__` 没有 `__str__` 时，`str(x)` 的回落是否等于 `__repr__` ✓ 还没测 ✓。

## 本轮（598）：`str → __repr__` 回落 ✓（接上轮那条"未验的相邻口径" ✓）

**修法** ✓：`object_str_native` 在"没有 `__str__`、也没有 `str` 槽"时改为调 **`object_repr`** ✓
（它自带"属性通道的 `__repr__` ⇒ 槽 ⇒ 默认形式"这条链 ✓ 一处真相 ✓）；先前直接调 `object_repr_native` ✗
⇒ **只有 `__repr__` 的类**的 `print` 会打默认形式 ✗。
**验收** ✓：3 条与参照**逐条一致** ✓（只有 `__repr__` ⇒ `print(b)`／`str(b)`／`repr(b)` 都是 `B-repr` ✓；
两个都有 ⇒ `C-str C-str C-repr` ✓），钉进 `text_shapes.rs` 第 2 条用例 ✓。
**顺带查实一个不相干的缺口** ✗（如实 ✓，没混进本笔 ✓）：默认 repr 少了**模块限定** ✓ ——
我们 `<D object at 0x…>` ✗ vs 参照 `<__main__.D object at 0x…>` ✓。

## 本轮（599）：默认 repr 的**模块限定** ✓

**修法** ✓：`object_repr_native` 的默认形式由 `<类名 object at 0x…>` 改成 **`<模块.类名 object at 0x…>`** ✓
（`__module__` 缺失或为 `builtins` 时只用类名 ✓ —— 内建类型**不带**前缀 ✓）。这正是该处注释里写的
"模块／qualname 随类创建钩子接线后补" ✗ 的**模块那一半** ✓（qualname 那半仍缺 ✗，如实 ✓）。
**验收** ✓：3 条与参照**逐条一致** ✓（`__main__.D` ✓、`object()` 不带前缀 ✓、`__module__` 改成 `mymod` ⇒ `mymod.E` ✓），
钉进 `text_shapes.rs` 第 3 条用例 ✓；`cargo test --workspace` 无失败 ✓。

## 本轮（601，新目标第 1 轮）：**假货 enum 顶包** ＋ `int(x, base)` 接线 ✓

**目标已改设并重新武装** ✓（同一个 goal，上限 600 → **720** ✓）：
> 以假货 `Lib/enum.py` 顶包，把 `import re` 跑通并落地 ✓（判据＝`import re` ✓ ＋
> `re.match/search/sub/split/findall` 与参照逐例一致 ✓）⇒ 再走整包搬迁 ⇒ 报判据① 前后分子 ✓。

**本笔落了两件事** ✓：
1. **假货 `Lib/enum.py`** ✓（用户 2026-10-07 拍的口径 ✓）：文件头写死**三步换回上游**的流程 ＋ 四条不许违反的纪律 ✓
   （只提供上游同名面／宁少不偏／不写闭包／唯一落点 ✓）＋ 明确的**偏离清单** ✓。它绕开了我们 VM 的**四道墙** ✓
   （元类**动态**建类 ✗、`object.__init_subclass__` 缺失 ✗、装饰器对**可调用实例**不传被装饰对象 ✗、
   **闭包捕获不可靠** ✗），成员做成**普通 int** ✓ ⇒ 连"造 `int` 子类实例"那道也绕掉了 ✓；
   而 `re` 只用 `isinstance(flags, RegexFlag)` 两处 ✓ ⇒ 行为**等价** ✓（成员是 int ⇒ 那两行永不执行 ✓）。
   已验：**被导入**的模块**在** `sys.modules` 里 ✓ ⇒ `global_enum` 对 `re` 能落地 ✓。
2. **`int(x, base)`** ✓（`builtin/int.rs`）：口径照参照 15 例实测 ✓（含 `base == 0` 的前缀判定 ✓、
   `"010"` 报错 ✓、2／8／16 允许对应前缀 ✓、下划线只允许数字之间 ✓、两条异常消息原文 ✓）。
   护栏：`tests/int_shapes.rs` ✓、`tests/enum_shapes.rs` ✓（后者**只钉 `re` 依赖的面** ✓，
   并在注释里写明"换回真货时同笔改 `isinstance(F.A, F)` 那一格" ✓）。

**下一道墙（已见形）** ✗：`import re` 现在报 `TypeError: 'str' object cannot be interpreted as an integer`
——某处在 `import` 期把 **str 当 base** 传给 `int` ✓ ⇒ 下一轮用最小复现钉它（先看 `Lib/re/_casefix.py`／`_constants.py`
里带 `int(` 两参的位点 ✓），接线后继续往顶 ✓。`Lib/re/` 的 5 个源文件已在工作区 ✓（**未提交** ✗：
它还没过同步的两道必检 ✓）。

## 本轮（605）：**实例化"覆写优先、槽兜底"** 落地 ✓（③ 撤 ✗）

**落** ✓（`executor/call.rs`，闸门绿 ✓）：参照的 `type.__call__` 只做 `cls.__new__(cls, *args)` ✓，槽只是
C 层默认实现 ✓ ⇒ 把"找 Python 级 `__new__` 覆写（并排除 `type.__new__`／`object.__new__`）"提到槽调用**之前** ✓，
有覆写就不调槽 ✓。效果 ✓：`class N(int)` 覆写 `__new__(cls, value, name)` 时槽**不再先吃全部实参** ✓
（先前 `int_new` 的 `base` 收到 `"three"` ✗ —— `Lib/re/_constants.py:70` 同型 ✓）。
另加 ✓：`super_lookup` 里**只在 `super` 这条路上**、对"有 `new` 槽但字典里没有 `__new__`"的 MRO 条目，
回一个绑到**该条目**的共享桥接 ✓ —— **不进类型字典** ✗（第 604 轮就是"进字典"把元类建类打红 ✗：
`__new__() takes 4 positional arguments but 5 were given` ✓）。

**撤** ✗：`super_lookup` 那处"`this` 是类 ⇒ 用 `this.__mro__`"（照参照 `super(C, cls)` ✓）**单独叠上也红** ✗
（`meta_path_shapes` 三条；报错原文本轮**没读到** ✗）⇒ 已 `git checkout` 撤回 ✓，只留上面两处 ✓。

**下一手** ✓：先把 ③ 那条红**读出原文**（`PYAWA` 探针或直接跑该用例 ✓），弄清 `meta_path_shapes` 里
`super(...)` 的 `__thisclass__`／`__self__`／MRO 实际形状 ✓，再决定怎么让 `super().__new__` 找到 **`int` 的槽** ✓
（现在是 `object.__new__` ✓ —— 因为 ③ 没上时 `super(N, cls)` 走的是 `type(cls)` 的 MRO ✗）。

## 本轮（606）：撤掉打红的桥接代码，只留"覆写优先" ✓（并把上一笔的不自洽修掉 ✓）

**实测结论** ✓：`new` 槽 ⇒ `__new__` 的桥接——**进不进类型字典都一样**——只要被 `super_lookup` 返回，
`meta_path_shapes` 就**三条全红** ✗（`TypeError: __new__() takes 4 positional arguments but 5 were given` ✓）。
⇒ 桥接**整块删除** ✓（70 行 ✓），只留 `call.rs` 的"**覆写优先、槽兜底**" ✓（真修 ✓：`class N(int)` 覆写
`__new__(cls, value, name)` 时槽不再先吃全部实参 ✓）。
**修掉的不自洽** ✗：上一笔（`fa2c248`）里 `super_lookup` 的钩子被 `git checkout` 抹掉 ✗ ⇒ 桥接成死代码 ✓、
`cargo check` 报 5 条 dead_code 警告 ✗（十项闸门第 1 项要 0 警告 ✓）⇒ 本笔删掉桥接后 **0 警告** ✓ 且
`quickcheck` 绿 ✓、`slowcheck` **十项全绿**（314 s ✓）。

**改变计划的事实** ✓（下一手据此 ✓）：那 5 个实参**不是**从 `call.rs` 覆写路径来的 ✓（`PYAWA_NEW_DEBUG` 探针
在那条路**一条都没打** ✓）⇒ 下一手**先**在**类创建路径**（`classes.rs` 调元类 `__new__` 的几处）抓"谁多传一格" ✓，
弄清**之后**再谈桥接 ✓；在此之前**不再**碰 `super_lookup` ✗。

## 本轮（607）：`new` 槽桥接**落地**（第一次带着钩子提交 ✓）＋ 钩子判定太窄的事实 ✓

**打红/打绿的对照（都实测 ✓）**：
```
钩子（super 路上回桥接）单独           ⇒ meta_path_shapes **3 过** ✓（复现推进到 `type.__new__` ✗）
钩子 ＋ ③（this 是类 ⇒ 用自身 MRO）    ⇒ **红** ✗（`meta_path_shapes` 失败；且钩子**一条都没打** ✗）
```
⇒ 本笔**只落钩子** ✓（＋ `call.rs` 的第 605 轮"覆写优先" ✓），**③ 撤掉** ✗。

**钩子判定太窄的事实** ✓（下一手就改这行 ✓）：钩子条件写的是
`instance.type_lookup(entry, "__new__").is_none()` ✗ —— 但 `int` 的 `__new__` 会**一路扫到我们装的
`object.__new__`** ✓ ⇒ 永远不为 `None` ✗ ⇒ **钩子对 `int` 永不触发** ✓，于是 `super().__new__` 落回
`object.__new__`（2 实参 ✗）✓。**改法** ✓：条件换成"**该条目自己有 `new` 槽** ✓ 且（查不到 ✓ 或查到的就是
`object` 那层的默认 `__new__` ✓）" ⇒ 用**该条目**的槽 ✓；同时**排除 `object`／`type` 两个条目** ✗
（对它们做桥接会波及元类建类 ✓，正是 ③ 那种红的来源 ✓）。

**另记** ✓：`③`（`this` 是类 ⇒ 用 `this.__mro__`）单独也会红 ✗ ⇒ 不走全局改 MRO 的路 ✗；
要的话改成"**只在主 MRO 落回 `object` 层默认 `__new__` 时**再看 `this.__mro__`"的**兜底** ✓。

## 本轮（608）：钩子判定修好并落地 ✓；**下一道墙换成"`int` 子类实例没有 `__dict__`"** ✓

**改了并落地的** ✓（`executor.rs::super_lookup` 的钩子条件）：判据从
`type_lookup(entry,"__new__").is_none()` ✗（太窄 ✓ —— `int` 的查询会一路扫到我们装的 `object.__new__` ✓）
换成"**该条目自己有 `new` 槽** ✓ 且（查不到 ✓ 或查到的就是 `object` 层默认 ✓）" ✓，
并**排除 `object`／`type`** ✗（对它们桥接会波及元类建类 ✓）。**闸门绿** ✓（0 警告 ✓、quickcheck ✓、slowcheck 十项 ✓）。

**二分结果** ✓（决定性的 ✓）：把"`__new__`＋self 是类 ⇒ 走自身 MRO"那一处叠上去 ⇒ **红** ✗；
只留钩子条件 ⇒ **绿** ✓ ⇒ 打红的是那处窄 MRO ✗（已撤 ✓）。

**那处窄 MRO 一跑，反而把复现推到了全新一层** ✓（这是本轮最有价值的收获 ✓）：`super().__new__`
**已正确落到 `int` 的槽** ✓、造出了 `int` 实例 ✓，随后卡在
```
AttributeError: 'int' object has no attribute 'name' and no __dict__ for setting new attributes
```
⇒ **下一道墙＝"Python 定义的 `int` 子类，其实例要能挂属性（有 `__dict__`）"** ✓（参照里子类实例天然有 dict ✓，
`re/_constants.py:70` 的 `_NamedIntConstant.__new__` 正是 `self.name = name` ✓）。

**下一手** ✓：在 core 里让"Python 定义的、基类型是定长内置类型（`int` 等）的子类"实例支持**实例字典**
（`header` 那条字典通道 ✓，`mounted_instance_dict`／`store_instance_dict` 已有 ✓ 不必新造 ✓）✓；
判据＝最小复现 `class N(int)` ＋ `super().__new__` ＋ `self.name = …` 与参照一致 ✓，
且 `meta_path_shapes` 不红 ✓；然后再把"窄 MRO"那处按**只影响 `__new__`＋类是 self**的形状重做一遍 ✓（这次要带二分 ✓）。

## 本轮（609）：Python 子类补上"实例字典位" ✓；**下一道墙＝内建 `new` 槽忽略目标类** ✓

**落** ✓（`classes.rs::build_class_from_parts`）：对"**非内联字典布局**的新建类"补 `HAS_INSTANCE_DICT` ✓，
字典走**头部那一格** ✓（`mounted_instance_dict` → `store_instance_dict` ✓ 已有 ✓ 不新造通道 ✓）。
**关键坑（本轮实测 ✓）**：一开始用 `mark_has_instance_dict()` ✗ —— 它**连 `GENERIC_ALLOCATION` 一起置上** ✗
（`type_object.rs:351` ✓）⇒ 实例化那边按"通用分配 ⇒ 带实参就报错" ✗ ⇒ `N(3)` 当场 `N() takes no arguments` ✓；
换成 `OM-14` 的 `mark_external_instance_dict()`（**只置字典位** ✓）后 ✓ 恢复 ✓。

**下一道墙（探针实测钉住 ✓）**：`class N(int)` 的 `N(3)` 造出来的**实例类型是 `int` 而不是 `N`** ✗
（日志：`建类 name=N 内联字典=false 已有字典位=true` ✓ 但报错写的是 `'int' object …` ✓）
⇒ 属性写到"无字典的 `int`"上 ✗ ⇒ 报 `AttributeError … and no __dict__ …` ✓。
**下一手** ✓：让**内建类型的 `new` 槽尊重目标类** ✓（`int_new` 目前忽略 `_class` ✗ ⇒ 一律造 `int` ✓）：
当 `_class` 是 `int` 的**子类**（布局相同 ✓）时，用**该子类**作为实例的类型 ✓；判据＝
`type(N(3)) is N` ✓、`n.tag = 7` 能挂 ✓、与参照逐例一致 ✓，且 `meta_path_shapes` 不红 ✓。
（`re/_constants.py:70` 的 `_NamedIntConstant` 正是靠这个 ✓。）

## 本轮（610）：内建 `int` 的 `new` 槽**尊重目标类** ✓ —— 子类实例终于"是子类"了 ✓

**落** ✓：`int_new` 造实例时用**调用方给的类** ✓（`make_int(_class, …)` ✓；`_class == int` 时原样走旧路 ✓
不动既有行为 ✓）；`int_of` 放宽到**认 `int` 的子类** ✓（否则子类实例一造出来就"不再是整数" ✗）。
**验收** ✓（与参照逐例一致 ✓）：`type(N(3)) is N` ⇒ **True** ✓、`n.tag = 7` **挂得上** ✓、
`n + 1` ⇒ 4 ✓、`isinstance(n, int)` ⇒ True ✓、`print(n)` ⇒ 3 ✓；闸门 0 警告 ✓、`quickcheck` ✓、`slowcheck` 十项 ✓。

**同一型还差** ✗（下一手照抄即可 ✓）：`class S(str)` 仍报 `'str' object has no attribute 'tag' …` ✓
⇒ `str_new`（以及 `bytes`／`float`／`tuple` 等）**同样忽略目标类** ✗ ⇒ 用同一个 `make_*` 模式逐个补齐 ✓。

## 本轮（611）：`__new__` 的**兜底**落地 ✓ —— `_NamedIntConstant` 通了，`import re` 又推进一大截 ✓

**落** ✓（`super_lookup`）：当 `__new__` 在主路上落到 **`type.__new__`／`object.__new__` 这两个默认**之一 ✓、
且 `self` 是**类** ✓ 时，去**它自己的 MRO**找一个"**内建**条目（`has_generic_allocation()==false` ✓、
且不是 `object`／`type` ✓、自己有 `new` 槽 ✓）"⇒ 用**它的槽** ✓。
**为什么只认内建条目** ✓：Python 定义的类（如 `ABCMeta` ✓）**不**在这里桥接 ✗ —— 第 607/608 轮正是那条路
把 `meta_path_shapes` 打红 ✓；本轮加上这道闸后 **护栏绿** ✓。

**验收** ✓：`class N(int)` ＋ `super(N, cls).__new__(cls, value)` ＋ `self.name = …` ⇒ 与参照**逐字一致** ✓
（`5 five True` ✓）；`meta_path_shapes` 绿 ✓；0 警告 ✓、`quickcheck` ✓、`slowcheck` 十项 ✓。
`import re` 也从"`type.__new__` 参数不够" ✗ 推进到 **`NameError: name 'LITERAL' is not defined`** ✓。

**下一道墙（新，已见形 ✓）**：`Lib/re/_constants.py` 的
`def _makecodes(*names): … globals().update({item.name: item for item in items})` ✓
＋ `re/_compiler.py` 的 `from ._constants import *` ✓ ⇒ 我们这边 `LITERAL` 等名字没进到模块里 ✗
⇒ 下一手先**最小复现**：`globals()` 是不是**活命名空间** ✓（更新能不能被后续 `from X import *` 看到 ✓）、
以及 `import *` 会不会取"后来动态加的全局" ✓；据此接线 ✓（判据＝`from _constants import *` 后 `LITERAL` 在 ✓）。

## 本轮（612）：`str` 子类也"尊重目标类" ✓＋`NameError: LITERAL` 的**排查链** ✓

**落** ✓（`builtin_objects.rs`）：加 `make_str(class, …)`（与 `make_int` 同型 ✓），并把
"本来就是 `str` ⇒ 原样给回"那条**限定为仅当目标类就是 `str`** ✓ —— 先前它对 `class S(str)` 也生效 ✗
⇒ `S("ab")` 返回的是普通 `str` ✗（`type(s) is S` 为假 ✓）。**验收** ✓：`dictsub.py` 与参照**逐行一致** ✓
（`挂属性 ok: 7` ✓、`str 子类 ok: 8` ✓）、`type(s) is S` ⇒ True ✓；闸门 0 警告 ✓、`quickcheck` ✓、`slowcheck` 十项 ✓。

**`NameError: name 'LITERAL' is not defined` 的排查链** ✓（本轮实测，四条**都否掉**了错的那半 ✓）：
```
globals().update({…}) 在"被导入模块的函数里" ⇒ **活命名空间** ✓（后续能看见 ✓）
from X import * 取**动态加的全局**              ⇒ 取得到 ✓
字典推导式：模块级 ✓／函数内 ✓／函数内用参数 ✓ ⇒ 都正常 ✓
把 _constants.py 原样拷成探针 ⇒ 单独 import 就挂 ✗（`LITERAL` 未定义 ✓）；前 70 行 ok ✓
⇒ 缩小到 `_makecodes(OPCODES…)` 那一段的**某种组合** ✓（还差最后一刀 ✓）
```
**下一手** ✓：用 `target/recon/gc.py` 那个二分脚本继续缩（loop 形态 ok ✓、推导式形态 ✗ —— 但把推导式拆开后各自都 ok ✗
⇒ 差别在**组合**：`*names` ＋ `enumerate` ＋ 属性键 ＋ `globals().update` 四者同现 ✓）⇒ 一刀见底后接线 ✓。

## 本轮（614）：字典推导式发射的**真 bug 修** ✓ —— `LITERAL` 那道墙过了 ✓

**根因** ✓（`compile/emitter.rs` 字典推导式）：融合快路用 **`leftmost_name(element)`** 判"键就是这个裸名字" ✗ ——
而 `leftmost_name`（`compile.rs:1960` ✓）**会钻进 `Attribute`／`Subscript`** ✓ ⇒ 对 `{item.name: item for item in items}`
它给出 **`item`** ✗（真键是 `item.name` ✓）⇒ 键发错／融合记账错 ⇒ 栈不平 ✓（`帧操作失败：StackUnderflow` ✓）。
**修法（已过逐字节夹具 ✓）**：**只对"键"要求裸名字** ✓（`matches!(element, Expression::Name(..))` ✓）；
**值那半照参照保留** ✓ —— 参照对 `{k: k + 1 …}` 融合的正是**值的 leftmost 名字** ✓（夹具
`y = {k: k + 1 for k in s if k}` 要求 `LOAD_FAST_BORROW_LOAD_FAST_BORROW` ✓ ⇒ 我第一版把值也闸掉 ✗ ⇒ 夹具红 ✓，
已改回 ✓）。
**验收** ✓：`pyawa-core --test compile` **4 过** ✓（逐字节 ✓）；`import re` 从
`NameError: name 'LITERAL' is not defined` ✗ 推进到 **`ModuleNotFoundError: No module named 'functools'`** ✓
（说明 `enum`／`_constants`／`_compiler` 全过 ✓）；0 警告 ✓、`quickcheck` ✓、`slowcheck` 十项 ✓。

**如实** ✗：另写的 `bisect4.py` 变体 B（`Obj` 与推导式同模块 ✓）**仍** `StackUnderflow` ✓ ⇒ 同族还有一形状没修 ✓
（我试着在通用路径前清 `pending_fused_load` ✗ 无效 ✓，已撤回 ✓；记着 ✓，不假装全好 ✓）。

**下一道墙** ✓：`functools`（`re/__init__.py:127` ✓）—— CPython 里是**纯 Python** ✓ ⇒ 下一手按整包两道必检
把它同步进 `Lib/` ✓（先读它在 3.14 的 import 面 ✓，看是否连带 `types`／`collections.abc` ✓）。

## 本轮（616）：**最小 `eval` 接通** ✓ —— `functools` 的墙移到 `sys._getframe(depth>0)` ✓

**探针一击命中** ✓（`PYAWA_EVAL_DEBUG` 临时代理 ✓，用完即撤 ✓）：`functools` 那条 `eval` 的真实形状是
```
eval("lambda _cls, hits, misses, maxsize, currsize: _tuple_new(_cls, (hits, misses, maxsize, currsize))", ns)
```
⇒ **2 个实参** ✓（源码 ＋ 命名空间 ✓）⇒ 只要"编译表达式并在给定命名空间求值"就够 ✓。
**落地** ✓：`Instance::eval_source(source, namespace)`（core ✓：把源码包成 `__pyawa_eval_result__ = (… )` ✓
再 `compile`＋`instantiate`＋`Frame::for_code_with_namespace`＋`execute` ✓ 取回结果 ✓）＋ stdlib 注册 `eval` ✓；
**如实范围** ✗：只支持 `eval(源码, 命名空间)` ✓（拿不到调用者帧 ⇒ 无命名空间的形态如实报 `Unsupported` ✓）。
**验收** ✓：`eval("1 + 2", ns)` ⇒ **3** ✓、`eval("lambda x: x * 3", ns)(4)` ⇒ **12** ✓（与参照逐条一致 ✓）；
0 警告 ✓、`quickcheck` ✓、`slowcheck` 十项 ✓。

**下一道墙** ✓：`sys._getframe(depth>0)`（报"目前只接 depth＝0" ✓）—— `functools` 的 `_CacheInfo`／`cached_property`
要用 `f_back` 链 ✓（`DESIGN`／`NEXT` 里早就列着这块 ✓）⇒ 下一手接 `f_back`（帧链 ✓）：
判据＝`sys._getframe(1)` 拿到调用者帧 ✓、`f_back` 链可走 ✓、与参照逐例一致 ✓，且闸门不红 ✓。

## 本轮（617）：**调用栈 ＋ `sys._getframemodulename`** ✓ —— `_getframe` 那道墙过了 ✓

**落地** ✓：`CurrentFrameGuard` 装帧/卸帧时压/弹一条**调用栈**（core ✓）；`sys._getframemodulename([depth])` ✓
（取那一帧的模块名 ✓，拿不到给 `None` ✓）＋ 从 core 导出 ＋ 注册进 `sys` ✓。
**为什么走这条** ✓：`Lib/collections/__init__.py:519-527` 的 `namedtuple` **先试** `_sys._getframemodulename(1)` ✓、
失败才走 `_getframe(1).f_globals` ✓ —— 而我们的帧只有 `f_locals` ✗、**没有 `f_globals`** ✓ ⇒ 接前者最短 ✓。
**验收** ✓：`functools` 从"`sys._getframe` 只接 depth＝0" ✗ 推进到 **`update_wrapper() missing 1 required
positional argument: 'wrapper'`** ✓；`collections.namedtuple` 也从同一处推进到
**`object.__new__() takes exactly one argument … 实际给了 1 个`** ✓；0 警告 ✓、`quickcheck` ✓、`slowcheck` 十项 ✓。

**两道新墙** ✓（下一手逐个来 ✓）：
1. **`object.__new__(cls)`（1 个实参）被拒** ✗（消息自相矛盾 ⇒ 像是把"绑定进来的类"也数成实参了 ✓）——
   `namedtuple` 建类时要用 ✓；
2. **`functools.update_wrapper()` 的 `wrapper` 实参没绑上** ✗（`functools.py` 里那句是关键字调用 ✓
   ⇒ 疑与我们**原生函数的 kwargs 绑定**有关 ✓）。

## 本轮（625）：**两个 core 真 bug 修掉 ⇒ `functools` 通了** ✓✓（`re` 只剩一道墙 ✓）

**修一：装饰器调可调用实例时的那一格实参** ✓（`executor/call.rs` 的 `__call__` 派发 ✓）：
装饰器应用编译成 `[装饰器, 被装饰对象] CALL 0` ✓（参照 `dis` 实测 ✓）⇒ 被装饰对象落在 `self_or_null` 槽 ✓、
对"非绑定可调用"就是**第一个位置实参** ✓；先前这条路上**整格丢掉** ✗ ⇒ 探针实测 `partial.__call__` 进来时
`args＝()` ✗（`functools.wraps` 返回的 `partial` 正是这么被套到 `reduce` 上的 ✓）。
**修二：被闭包捕获的名字当"被调用者"时误发 `LOAD_GLOBAL`** ✗（`compile/emitter.rs` 的 `global_callee` ✓）：
判据只查了 `varnames`（本层局部 ✓），没排 cell／free ✓ ⇒ `def inner(): return x(1)`（`x` 来自外层 ✓）报
`NameError: name 'x' is not defined` ✓（`functools` 的 `py_reduce(*args, **kwargs)` ✓ 同型 ✓）。
**验收** ✓：`functools ok: 6` ✓（`reduce` 正确 ✓）；装饰器与闭包两组最小复现与参照**逐例一致** ✓；
0 警告 ✓、`quickcheck` ✓、`slowcheck` 十项 ✓。

**`re` 现状** ✓：已冲过 `enum`／`_constants`／`_compiler`／`functools` ✓，只剩
**`TypeError: object of type 'SubPattern' has no len()`** ✗（`Lib/re/_parser.py:111` 的 `SubPattern` ✓）。

## 本轮（626）：**`len()` 与切片的实例协议**接上 ✓ —— `re` 只剩"**`range` 不是 `range`**"这一道 ✓

**落一：`len()` 的实例协议** ✓（`instance/containers.rs::length_with_protocol` ＋ stdlib `len` ✓）：内建那几种走
`length_of` ✓，其余**只在类型上**查 `__len__` ✓ 并调它 ✓；非整数 ⇒ `TypeError: '<类型>' object cannot be
interpreted as an integer` ✓、负数 ⇒ `ValueError: __len__() should return >= 0` ✓（与参照逐例一致 ✓）。
**落二：切片的 `__getitem__` 协议** ✓（`executor/subscript.rs` ✓）：内建四种之外，把 **slice 对象**交给
**类型上**的 `__getitem__` ✓（`c[1:2]` ⇒ `('键', slice(1, 2, None))` ✓，与参照一致 ✓）。
**改写** ✓：切片接不上的报错**带上类型名** ✓（定位不用再绕 ✓）。
**`range` 这一道（本轮新事实 ✓✓）**：
```
range(5) 在我们这边 ⇒ type(r).__name__ == 'range_iterator' ✗（参照 'range' ✓）
⇒ re 里"切一个 range"就报 切片…这里是 'range_iterator' ✗
```
根在 `builtin_objects.rs:1068-1070` 的**"改型"** ✓：`range(...)` 直接改型成迭代器 ✗ ⇒ 缺一个**真正的 `range` 对象** ✓。
**下一手** ✓：`range` 做成真对象（载荷 `start`／`stop`／`step` ✓；`__len__`／`__getitem__`（负下标＋步长 ✓）／迭代 ✓／
`repr` ✓）⇒ 判据：`range(5)[1:3]` ⇒ `range(1, 3)` ✓、`len(range(0, 10, 3))` ⇒ 4 ✓、`range(5)[::-1]` 与参照一致 ✓。

## 本轮（627）：**`range` 做成真对象** ✓✓（`re` 那道墙过了）＋ **迭代走协议** ✓

**落一：`range` 是真对象** ✓：载荷 `RangeObject{start, stop, step, long_range}` ✓；`range_new` **不再**
把 `islice(count(...))` 改型成迭代器 ✗（旧偏差 ✓）⇒ 给真 `range` ✓，负步长也放行 ✓（先前的
`NotImplementedError` 一并去掉 ✓）；槽挂**构造**／`repr`（`range(0, 5)`／`range(0, 10, 3)` ✓）／方法面
`__len__`／`__getitem__`（整数给整数 ✓、**切片给新 range** ✓、负下标 ✓、负步长 ✓）／`__iter__`
（**此刻才**把 `count`＋`islice` 改成 `range_iterator`／`longrange_iterator` ✓）。
**验收**（与参照**逐行一致** ✓）：`type`／`repr`／`list`／`len`／`r[1:3]`＝`range(1, 3)` ✓／`r[-1]`＝4 ✓／
`range(0, 10, 3)` 长 4 ✓／`range(5)[::-1]`＝`range(4, -1, -1)` ✓／`range(0)` 空 ✓／`for` 迭代 ✓。

**落二：迭代与长度统一走"属性通道／协议"** ✓（这是本轮真 bug 的共性 ✗）：`__iter__`／`__len__` 可能由类型的
**`getattr` 槽**动态给出 ✓（`range` 就是 ✓）⇒ `iter_value` 改走 `attribute_lookup` ✓；`iterable_items`
（`sum`／`sorted`／`tuple` 一类 stdlib 助手都经它 ✓）**先走 `iter()` 协议** ✓ 再逐个 `advance` ✓；
整数下标那条路补上 `__getitem__` 协议回退 ✓。
**验收** ✓：conformance 对拍 `range_builtin` 从"新差异"回到**通过** ✓（`slowcheck` 十项全绿 ✓、
0 警告 ✓）—— 这一条是被对拍真正抓出来的回归 ✗ ⇒ 也是被对拍确认修好的 ✓。

**`re` 新墙** ✓：**`TypeError: 'NULL' object is not iterable`** ✓（我们 VM 的 `NULL` 哨兵漏进了某处
"可迭代"取用 ✓ —— 下一手挂 `PYAWA_ITER_DEBUG` 打站点＋回溯 ✓，或从 `re` 的导入序列二分 ✓）。

## 本轮（628）：`NULL` 来源的**门控探针** ✓ —— 一击定到 `SubPattern.getwidth` ✓

**落** ✓（`executor.rs` 的 `RETURN_VALUE` ✓，门控 `PYAWA_NULL_TRACE=1` ✓）：谁把 VM 的 **NULL 哨兵**当值返回 ✓
就打出**代码名 ＋ 站点 ＋ 回溯** ✓（比"解包时才发现 NULL"✗ 近一大步 ✓）。
**实测输出** ✓（`PYAWA_NULL_TRACE=1 import re` ✓）：
```
[null-return] code=getwidth site=SubPattern.getwidth@634      ← 出现两次（递归 ✓）
```
⇒ **`Lib/re/_parser.py:178` 的 `SubPattern.getwidth`** 在返回时给的是 NULL ✗（而不是它末尾那个
`self.width = min(lo, MAXWIDTH), min(hi, MAXWIDTH)` ✓ 的元组 ✓）⇒ `_compile_info` 的解包当场炸 ✓。

**本次已否掉的（都实测 ✓）**：隐式返回 ✓、`for` 后隐式返回 ✓、空 `for` ✓、`for…else` ＋ `break` ✓、
`for op, av in data` 解包 ✓、内层 `for av in av[1]` 复用同名变量 ✓（我的等价复现**都对** ✓）——
⇒ 关键差别在于 `getwidth` 里判的是 **`op is BRANCH` 这类身份比较** ✓，而 `op` 是**假货 enum 造的
`_NamedIntConstant`（int 子类）成员** ✓ ⇒ 下一手就用**真 `_constants` 里的成员**做复现 ✓（而不用字符串 ✓），
并给 `Lib/re/_parser.py` 装 MARK 探针把 NULL 那条返回路径夹出来 ✓。

## 本轮（629）：**长跳落点的真根因被数据证实** ✓✓ —— 修法只差"按加宽后坐标重算实参"这一步 ✓

**已落** ✓（`PYAWA_JUMP_DEBUG=1` ✓，`compile/emitter.rs`）：`flush_jumps` 现把"**指令码元 → 目标码元 → 相对实参
→ size**"打出来 ✓；`widen_extended_args` 现把"**哪些跳转需要加宽**（码元, 实参 ✓）＋**插了几个词**"打出来 ✓。
**实测（`target/recon/size10.py` ✓）**：
```
[jump] 需要加宽的跳转 4 条：[(6, 350), (36, 319), (68, 287), (356, 352)]
[jump] 加宽插入 4 个词（码元总数 363 ⇒ 367）
⇒ 帧操作失败：StackUnderflow
```
参照同一函数：`EXTENDED_ARG 1` ＋ `FOR_ITER 367`（总码元 368 ✓ 与我们一致 ✓）。

**根因（已证实 ✓）**：`widen_extended_args` 插 `EXTENDED_ARG` 时**搬了 `code`／`positions`／异常表平移** ✓，
却**没有修正已经回填好的"相对跳转实参"** ✗（也没移 `labels` ✓）⇒ 每条插在"跳转指令与它的目标之间"的加宽
都让**落点短 1 格** ✓ ⇒ 循环体越长、被加宽的跳转越多 ✓ ⇒ 从 `StackUnderflow`（10／14 臂 ✗）
到**直接跳进循环体**（18 臂 ⇒ `局部槽 6 未绑定` ✗）—— 三条实测**完全吻合** ✓。
这也解释了 `Lib/re` 的 NULL ✗：`SubPattern.getwidth` 的空迭代循环跳进了体里 ✓（`MARK` 实测 ✓）。

**修复方案（下一手第一步，已具足全部输入 ✓）**：在 `widen_extended_args` 的**重建循环**里，对每条跳转
按"加宽后的坐标"重算实参 ✓ —— 旧基准 `base=word+size`、旧目标 `target=base±arg` ✓；
新基准 `= word+shift[word]+size+own_prefix` ✓（自己那份前缀要算进 `size` ✓）；
新目标 `= target + shift[target]` ✓；实参 `= ±(新目标−新基准)` ✓；**前缀的高位字节用新值** ✓、
随后那条指令的实参低字节用新值 ✓。若重算后仍 > 255 ⇒ 再跑一遍（最多两轮 ✓）。
判据：`target/recon/size10.py`（及 14／18 臂 ✓）与参照逐例一致 ✓＋`import re` 往前 ✓＋十项闸门绿 ✓。

## 本轮（630）：629 那条根因**改出来了** ✓（规模族全对 ✓、`re` 又推进一层 ✓），但撞上"第二轮加宽" ✗ 已回退 ✓

**改了并实测有效的部分** ✓（`emitter.rs`）：`flush_jumps` 记下每条跳转的 `(自身码元, size, 旧目标码元, 是否向后,
实参字节下标)` ✓；`widen_extended_args` 插前缀时**同步平移 `labels`** ✓；重建之后新增
`refill_jump_args` ✓ 按**新坐标**重算每条实参（新基准 `= new_here + own_prefix + size` ✓、
新目标 `= 旧目标 + shift[旧目标]` ✓、前缀高位与实参低字节都写新值 ✓）。
**实测 ✓**：`target/recon/size.py` 的 **10／14／18 臂全部正确** ✓（先前 10／14 `StackUnderflow` ✗、18 跳进体内 ✗）；
`import re` 从 `'NULL' object is not iterable` ✗ 推进到 **`TypeError: '<' not supported between instances of
'int' and 'int'`** ✓（说明 `SubPattern.getwidth`／`_compile_info` 那一段**真的过去了** ✓）。

**为什么回退** ✗：`cargo test -p pyawa-core --test lib_compile`（整棵 `Lib/` 编译 ✓）撞上我那句**越界断言**
`跳转实参 256 超过 1 字节但没排进加宽清单` ✗ —— 即**第二轮加宽**是必需的 ✓；而我最后那次"迭代到不动点＋
识别已有前缀"的补丁**第三条锚点没匹配** ✓（脚本在写文件前就断言失败 ✓ ⇒ 改动没落地 ✓）⇒ 树回退到
`c8445e8` ✓、十项闸门绿 ✓。

**下一手（就差这一处，方向已验证 ✓）**：把加宽做成**迭代到不动点** ✓：
① 外壳循环 `widen_once()` 直到 `wide_jumps` 为空（上限 4 轮 ✓）；
② `refill_jump_args` 里"重算后 >255"的**不再断言** ✓，而是**记进下一轮** `wide_jumps` ✓；
③ 重建时识别"**上一词就是自己的 `EXTENDED_ARG`**"（本层只把它当前缀用 ✓）⇒ **原地改高位字节** ✓、
不重复插前缀 ✓（否则 oparg 会被两条前缀叠歪 ✗）。
判据：`lib_compile` 绿 ✓、`size.py` 10／14／18 与参照一致 ✓、`import re` 往前 ✓、十项闸门绿 ✓。

## 本轮（631）：**长跳落点真 bug 修好** ✓✓（`lib_compile` 绿 ✓）—— `re` 站上新边界 ✓

**落** ✓（`compile/emitter.rs`）：① `flush_jumps` 记下每条跳转的 `(自身词位, size, 目标词位, 方向, opcode 实参字节下标)` ✓；
② `widen_extended_args` 插 `EXTENDED_ARG` 时**同步平移 `labels`** ✓，并把平移表 `shift` 留给回填 ✓；
③ 重建之后新增 **`refill_jump_args`** ✓ 按**新坐标**重算每条实参（自己有没有前缀**从 code 里认** ✓：
`new_here` 那一词是不是 `EXTENDED_ARG` ✓ —— 不依赖本轮计划 ✓，所以上一轮插的前缀不会被覆盖 ✓）；
④ 加宽做成**迭代到不动点** ✓（外壳循环 `widen_once` ✓ 上限 4 轮 ✓）：重算后仍 >255 就**记进下一轮** ✓
（先前是断言 ✗ —— `Lib/` 里正好有一条 256 ✓）。
**验收** ✓：`pyawa-core --test lib_compile`（整棵 `Lib/` 编译 ✓）**通过** ✓；`size.py` 的 10／14／18 臂
（先前 10／14 `StackUnderflow` ✗、18 跳进循环体 ✗）**全部正确** ✓；0 警告 ✓、`quickcheck` ✓、`slowcheck` **十项全绿**（331 s ✓）。

**`re` 现状** ✓：`enum` ✓ → `_constants` ✓ → `_compiler` ✓ → `functools` ✓ → `_getframe` ✓ → `range` ✓ →
`getwidth`／`_compile_info` ✓ ⇒ 新边界一句：
**`TypeError: '<' not supported between instances of 'int' and 'int'`** ✓（疑为"**int 子类 vs int** 的比较"
✗ —— `_NamedIntConstant` 一族的 `__lt__` 走的还是我们的 int 快路 ✓ ⇒ 下一手先最小复现
`class N(int): pass` ＋ `N(1) < 2`／`2 < N(1)`／`N(1) == 1` ✓）。

## 本轮（632）：`min`／`max`／`sorted` 比不了大整数 ✗ 修好 ✓ —— `re` 又推进一层 ✓

**最小复现** ✓：`min(1 << 100, 5)` ⇒ `TypeError: '<' not supported between instances of 'int' and 'int'` ✗
（参照给 5 ✓）—— 与 `re` 那道墙**逐字相同** ✓。
**根因** ✓（`instance/convert.rs::order_of` ✓）：数值分支走 `int_value` ✗（＝`int_of` 再 `to_i64` ✓），
**大整数给 `None`** ✗ ⇒ 落进"比不了" ✓；顺带还有个"大整数转 `f64` 丢精度"的隐患 ✗。
**修法** ✓：加"**整数优先按整数比**" ✓ —— `bool`／`int`（含子类 ✓）取 `IntValue` ✓ 用现成的
`IntValue::cmp` ✓（`bigint.rs:106` ✓），整数对整数不再经过 `f64` ✓。
**验收** ✓：`min`／`max`／`min(list)`／`sorted(list)` 四条与参照**逐字一致** ✓；0 警告 ✓、`quickcheck` ✓、
`slowcheck` 十项 ✓。

**`re` 现状** ✓：新墙一句 **`NameError: cannot access free variable 'typed' where it is not associated
with a value yet`（作用域 `decorating_function` ✓ 指令 9 ✓）** ✓ —— 是"**闭包自由变量**"那一族 ✓
（我们 VM 自己报的 ✓，带作用域与指令位置 ✓）⇒ 下一手先最小复现"函数里定义内层函数、内层捕获外层的
局部变量、外层还没赋到那一步就被调用" ✓（`re/_compiler.py` 的 `decorating_function` ✓）。

## 本轮（637）：**闭包装配的真 bug 修好** ✓✓ —— 多自由变量的闭包终于都对 ✓

**最小复现** ✓（与 `re` 那道墙同形 ✓）：
```python
def c(first, second=2):
    def inner():
        return first + second     # 两个自由变量
    return inner()
print(c(1))                       # 修前：NameError: free variable 'second' 还没有值 ✗   修后：3 ✓
```
**根因** ✓（`executor/call.rs` 装闭包那几行 ✓）：调用侧是**逐条**调 `install_closure(&[cell])` ✗ —— 而
`install_closure` 自己会从**第一个 `Free` 槽**开始按序装 ✓ ⇒ **每一次调用都装到槽 0** ✗ ⇒ 第二个及之后的
自由变量**永远拿到空 cell** ✓（与"`first` 通、`second` 空"的实测**逐条吻合** ✓）。
**修法** ✓：整条闭包**一次装完**（`let closure = function_closure(...)` ✓ 逐条 `incref` ✓ 再
`install_closure(&closure)` ✓ 一次 ✓）。
**验收** ✓：`c(1)` ⇒ **3** ✓（与参照一致 ✓）、`lru_cache` 装饰器形态**过了这一关** ✓；0 警告 ✓、
`quickcheck` ✓、`slowcheck` **十项全绿**（378 s ✓）。

**`re` 现状** ✓：新墙一句 **`STORE_NAME` 需要命名空间帧（模块／类体）；名字 `hits`；
代码对象 `_lru_cache_wrapper`；位点 `Lib/functools.py:611`** ✓ —— 即在**函数体**里我们发了 `STORE_NAME` ✗
（`hits`／`misses` 是 `_lru_cache_wrapper` 里的**局部** ✓，该发 `STORE_FAST` ✓ 或该是 cell／free ✓）
⇒ 下一手：按那个位点（611 行 ✓）看它是**赋值**还是**闭包写** ✓，修"函数体里名字的落点判定" ✓。

## 本轮（638）：链式赋值里的 **cell 名改走 `STORE_DEREF`** ✓ —— `re` 又推进一层 ✓

**根因** ✓（`compile/emitter.rs` 赋值那条路 ✓）：`Expression::Name` 的分支**只查 `varnames`** ✗ ⇒ 已被
`analyze_cells` **移出 `varnames`** 的 **cell** 名落成 `STORE_NAME` ✗ ⇒ 函数体里当场报
"`STORE_NAME` 需要命名空间帧" ✓（`functools._lru_cache_wrapper` 的 `hits = misses = 0` ＋ 内层
`nonlocal hits, misses` 同型 ✓）。**修法** ✓：cell／free 名**先**走 `deref_slot` ⇒ `STORE_DEREF` ✓；
并给 `STORE_FAST_STORE_FAST` 融合快路加一道"**末位也必须是普通局部**"的闸 ✓（否则会吞掉 cell 那一格 ✗）。
**验收** ✓：`import re` 从"`STORE_NAME hits`" ✗ 推进到 **`AttributeError: 'dict' object has no attribute
'__len__'`** ✓（`Lib/functools.py` 的 `cache_len = cache.__len__` ✓）；0 警告 ✓、`quickcheck` ✓、
`slowcheck` 十项全绿 ✓。
**如实** ✗：我自写的最小复现 `def outer2(): a = b = 0; def inner(): nonlocal a, b` **仍然**报同一句 ✓
⇒ 那条形状另有原因 ✓（非形参的**追加 cell** ＋ 链式赋值 ✓），记在案 ✓。

**`re` 新墙** ✓：`AttributeError: 'dict' object has no attribute '__len__'` ✓
⇒ 下一手：给内建 `dict` 的方法面补 **`__len__`**（照 `list_getattr` 的样式 ✓，`Lib/functools.py` 的
`cache.__len__` 与 `cache.get` 一类用法都要它 ✓）。

## 本轮（639）：内建容器的 **`__len__` 方法面**补上 ✓ —— `functools` **整块过了** ✓✓

**根因** ✓：`dict_getattr`／`list_getattr` 里**都没有 `__len__` 这一格** ✗ ⇒ `Lib/functools.py` 的
`cache_len = cache.__len__`（`_lru_cache_wrapper` ✓）当场报
`AttributeError: 'dict' object has no attribute '__len__'` ✓（`import re` 断在这 ✓）。
**修法** ✓：加 `container_len_native` ✓（复用 `length_with_protocol` ✓ **一处真相** ✓），挂到 `dict`／`list`
两处方法面 ✓。
**验收** ✓：`d.__len__()`／`lst.__len__()` 与参照逐字一致 ✓；`import re` 从"`dict` 没有 `__len__`" ✗
一路推进到 **`ModuleNotFoundError: No module named 'copyreg'`** ✓✓ —— 即 **`functools` 已经整块跑通** ✓；
0 警告 ✓、`quickcheck` ✓、`slowcheck` 十项全绿 ✓。

**如实** ✗：`functools.lru_cache` 的**运行期**还差一处 ✓ —— `lru.py` 报
`指令 93 …：LOAD_NAME 需要命名空间帧（模块／类体）` ✓（函数体里发了 `LOAD_NAME` ✗）；`re` 的导入不碰它 ✓
所以能过 ✓，但这条要记着 ✓。

**下一道墙（搬到 `re` 自己的依赖上了 ✓）**：**`copyreg`** ✓（纯 Python ✓）⇒ 下一手按整包两道必检把它同步进
`Lib/` ✓；随后大概率是 `re` 依赖链上的其它纯 Python 模块 ✓。

## 本轮（641）：**下标赋值的 `__setitem__` 协议**接上 ✓ —— 判据② 的下一刀＝`bytearray` 可变面 ✓

**落** ✓（`executor/subscript.rs` 的 `subscript_set` ✓）：内建那几种之外，走**属性通道**找 `__setitem__` ✓
（与 `__getitem__`／`__call__` 同一条路 ✓；Method／Value 两支都接 ✓）。**验收** ✓：用户类
`c[k] = v` 与参照逐字一致 ✓（`{'a': 1, 2: 'b'}` ✓）；0 警告 ✓、`quickcheck` ✓、`slowcheck` 十项全绿 ✓。
**`import re` 仍是 `ok`** ✓。

**判据② 的下一刀** ✓（已定位、也看清了它为什么不能顺手改 ✓）：`re._compiler` 在 **`bytearray`** 上做
**切片赋值** ✓ —— 而我们的 `bytearray` **借用 `BytesObject`（`value: Vec<u8>` 无内部可变性 ✗）**，
`instance.rs:441` 的注释也写着"**目前只做空 `bytearray()`，可变字节面随后接**" ✓ ⇒ 这一步要**真的做**：
① 给载荷加**内部可变性**（`RefCell<Vec<u8>>` ✓，`value()` 一族跟着改 ✓）；② `bytearray` 挂
`__setitem__`（整数下标＋切片 ✓，越界／非 0..256 照参照报 ✓）；③ 顺带 `append`／`extend`／`+=` ✓。
判据：`target/recon/reparity.py` 的 13 行与参照**逐字一致** ✓（match／search／findall／sub／split／subn／
finditer／escape／purge／flags／groups／named／template ✓）。

## 本轮（642）：**可变 `bytearray`** 落地 ✓（下标／切片赋值 ＋ repr ＋ len 都对 ✓）—— `+=` 是下一刀 ✓

**落** ✓：新增载荷 `BytearrayObject{value: RefCell<Vec<u8>>}` ✓（照 `SliceObject` 只给 `Slots::new(Self::dealloc)` ✓）；
`bytearray_new`（空 ✓／整数 ⇒ 那么多零字节 ✓／`bytes` ⇒ 拷贝 ✓／`str` ⇒ UTF-8 ✓）；
方法面 `__len__`／`__getitem__`（整数 ✓、**切片读给 `bytes`** ✓）／`__setitem__`（整数下标 ✓、切片 ✓、
越界与非 `0..256` 照参照报 ✓）／`append`／`extend` ✓；自己的 `bytearray_repr` ✓
（**不能用 `bytes` 那两个** ✗ —— 载荷不同 ⇒ 实测直接**栈溢出** ✓）；`length_of` 补 `bytearray` 分支 ✓
（并且 `container_len_native` 改成**只用 `length_of`** ✗ 不再走协议 ⇒ 否则 `__len__` 自己递归到栈溢出 ✓）。
**验收** ✓（与参照逐字一致 ✓）：`bytearray(4)` ⇒ `bytearray(b'\x00\x00\x00\x00')` ／`len` 4 ✓；
`b[1] = 65` ⇒ `bytearray(b'\x00A\x00\x00')` ✓；`d[0:0] = [1, 2]` ⇒ `bytearray(b'\x01\x02xy')` ✓；
`d[0:2]` ⇒ `b'\x01\x02'` ✓。**`import re` 仍 `ok`** ✓；0 警告 ✓、`quickcheck` ✓、`slowcheck` 十项全绿 ✓。

**下一刀** ✓（已复现）：**`bytearray + bytes`／`+=`** ✗（`TypeError: unsupported operand type(s) for +:
'bytearray' and 'bytes'` ✓）—— `re._compiler` 的 `data += chunk` 正需要它 ✓；`add_values`
（`instance/constructors.rs:11` ✓）目前只接**数值塔** ✓ ⇒ 补"`bytes`／`bytearray` 的拼接" ✓（结果给
`bytearray` ✓）。之后重跑 `reparity.py` 的 13 行逐字对账 ✓。

## 本轮（643）：`bytearray + bytes`／`+=` 通了 ✓（`data += chunk` 过关 ✓）—— `list(bytearray)` 还差一格 ✓

**落** ✓：`concat_public`（`executor/iter.rs` ✓）里补 **`bytearray` 拼接** ✓ —— 任一操作数是 `bytearray`
⇒ 结果给 **`bytearray`** ✓（另一侧要 `bytes`／`bytearray` ✓）；`add_values` 里也补了同一支 ✓
（数值塔前面先判 ✓，数值行为一律不动 ✓）。**验收** ✓：`d = bytearray(); d += b"xy"` ⇒
`bytearray(b'xy')` ✓（先前报 `unsupported operand type(s) for +: 'bytearray' and 'bytes'` ✗）。
**闸门** ✓：0 警告 ✓、`quickcheck` ✓、`slowcheck` 十项全绿 ✓。

**还没过的一格** ✓（如实 ✗）：`list(bytearray)` 报
`这个对象既不是内建迭代器，也没有 __next__` ✗ —— 我给 `bytearray` 挂了 `__iter__`（借 `bytes` 的迭代器 ✓，
**如实记**：迭代器类型会是 `bytes_iterator` 而不是参照的 `bytearray_iterator` ✗），但这条链在某处没走到
（`list(...)` 那条路走的可能不是 `iter_value` ✗）⇒ 下一手按**站点**查"谁在迭代它" ✓（挂 `PYAWA_ITER_DEBUG` ✓
或查 `iterable_items`／`sequence_items` 的分支 ✓），补上后再跑 `reparity.py` 的 13 行逐字对账 ✓。

## 本轮（644）：`bytearray_iterator` 补进**迭代器名字表** ✓（墙又移一格 ✓）

**根因** ✓：`bytearray_iterator` 一直是**真迭代器类型**（`IteratorObject` 槽 ✓，`instance.rs` 里跟
`bytes_iterator` 一起建的 ✓），但 `ITERATOR_TYPE_NAMES`（`executor.rs:145` ✓）**漏了它** ✗
⇒ `advance_iterator` 把它当"非内建迭代器" ✗ ⇒ 走 `__next__` 协议 ⇒ 报
"这个对象既不是内建迭代器，也没有 `__next__`" ✓。
**修法** ✓：表里补 `"bytearray_iterator"` ✓（数组长度 28 ⇒ 29 ✓）。
**验收** ✓：墙从"既不是内建迭代器…" ✗ 推进到
**`只接线了 tuple／list／dict／set／str／bytes 的内建迭代器（其余走 __iter__ 协议）`** ✓
（即 `iterator_type_for` 的 **else 分支**被走到 ✗ —— 说明那个对象**不是** `bytearray` 本身 ✓）；
0 警告 ✓、`quickcheck` ✓、`slowcheck` 十项全绿 ✓。

**下一手** ✓：把 `iterator_type_for` 的 else 分支**挂上站点／类型名**（或直接看 `list(bytearray())` 那条路的
每一步 ✓）⇒ 弄清被迭代的到底是谁 ✓（大概率是 `bytearray.__iter__` 返回的 `bytes_iterator` 又被包了
一层 ✗），补上后重跑 `reparity.py` 的 13 行逐字对账 ✓。

## 本轮（645）：`iterable_length` 补 **`bytearray`** ✓ ＋ 两处报错**带上类型名** ✓

**查出真身** ✓：那句"只接线了 tuple／list／dict／set／str／bytes 的内建迭代器"**来自 `iterable_length`**
（`executor/iter.rs` ✓）——**不是** `iterator_type_for` ✗（两者文案相同 ✓，先前被人名带偏过一次 ✓）。
**落** ✓：① `iterable_length` 补 `bytearray` 分支 ✓（载荷是 `BytearrayObject` ✗，`bytes` 那条对不上 ✓）；
② 两处同文报错都**带上类型名** ✓（`ctrls.rs` ✓、`iter.rs` ✓）——这条尺子在本题里已经五次一击命中 ✓。
**验收** ✓：0 警告 ✓、`quickcheck` ✓、`slowcheck` 十项全绿 ✓；`len(bytearray(b"xy"))` ⇒ 2 ✓。
**如实** ✗：`list(bytearray(b"xy"))` 仍报同一句 ✓（说明还有**第三处**同样的文案 ✗ 或同一函数又换了个类型 ✓ ——
我的 `grep "的内建迭代器"` 只找到两处 ✓，所以要**把整句话打全**再看 ✓：当前终端显示被截断了 ✓）。

**下一手** ✓：`grep -rn "内建迭代器" crates/` 找全（含 stdlib ✓），把**每一处**都补上 `bytearray`／带类型名 ✓；
然后重跑 `reparity.py` 的 13 行逐字对账 ✓（判据② 的正题 ✓）。

## 本轮（647）：`bytearray` **迭代**接通 ✓ —— `list(bytearray(b"xy"))` ⇒ `[120, 121]` ✓

**真凶是"第三处"文案** ✓✓：那句报错**源码里就是短的** ✗ —— `iterable_item`（`executor/iter.rs:325` ✓）
写的是 `"只接线了 tuple／list／dict／set／str／bytes 的迭代"` ✓ ⇒ 前几轮我加在**句尾**的类型名
**永远显示不出来** ✓（这也是"加了却看不到"的真正原因 ✓：**不是打印截断** ✗ —— 我用 python 原样捕获
也只有 70 字符 ✓，正是这句短文案 ✓）。
**落** ✓：① `iterable_item` 补 `bytearray` 分支（按整数给 ✓，与 `bytes` 同款 ✓）；② 那句报错**补全并带上
类型名（句首 ✓）**；③ 另两处（`iterable_length`／`iterator_type_for`）的类型名也挪到句首 ✓。
**验收** ✓：`list(bytearray(b"xy"))` ⇒ `[120, 121]` ✓（与参照一致 ✓）；0 警告 ✓、`quickcheck` ✓、
`slowcheck` 十项全绿 ✓。

**`re` 新墙** ✓（判据② 又近一步 ✓）：`reparity.py` 现在报
**`局部槽 14 未绑定（UnboundLocalError 未接线）`** ✗（在 `re._compiler` 里 ✓）⇒ 下一手握站点／函数名
（同一套"句首诊断"✓ —— 重要信息一律放句首 ✓）再修 ✓。

## 本轮（651）：dump **带上代码对象名** ✓ + 判据② 的岔口**夹到了码元级** ✓

**落** ✓：`PYAWA_DUMP_CODE=1` 的头部现在打 `name=<co_name>` ✓（`self.unit.name` ✓）—— 先前 dump 里
`Lib/re` 一次就是 **12800 个单元** ✗、**没有名字根本对应不回函数** ✓（判据② 的岔口就是这么找的 ✓）。
**实测** ✓（`reparity.py`）：
```
总单元 12800；name=_parse 的 1（指令 2007 条 ✓）
槽 14 的事件序列（前几条）：码元 469 STORE_FAST 14 → 470 LOAD_FAST_BORROW 14 → 631 LOAD → 645 LOAD → 650 STORE …
```
⇒ **静态上"先写后读"完全正常** ✓ ⇒ 于是"局部槽 14 未绑定"只能是**执行时跳过了 469 直接落在 470** ✗
—— **又是跳转落点那一族** ✓（与先前修好的长跳 `EXTENDED_ARG` 平移同源 ✓，只是这次落在了**同一块里的相邻指令**上 ✓）。
**闸门** ✓：0 警告 ✓、`quickcheck` ✓、`slowcheck` 十项全绿 ✓。

**下一手** ✓：把 `_parse` 里**指向码元 469/470 附近的那条跳转**找出来（`PYAWA_JUMP_DEBUG=1` ✓ 会打
"指令码元 → 目标码元 → 实参" ✓），看它是不是又短了一格 ✗；若是 ⇒ 按同一套（**加宽后平移** ✓）修 ✓，
若不是 ⇒ 那条跳转的**目标**本身算错 ✓ ⇒ 修发射端 ✓。判据：`reparity.py` 的 13 行逐字对账 ✓。

## 本轮（652）：跳转诊断**带上代码对象名** ✓ + `_parse` 那一段**编译结果是对的** ✓（排除掉一大片 ✓）

**落** ✓：`PYAWA_JUMP_DEBUG=1` 的两条输出（每条跳转 ＋ 加宽清单）都带 `name=<co_name>` ✓ —— 先前全仓
**17852 条跳转** ✗ 混在一起，挑不出 `_parse` 的 ✓。
**实测 ✓**（`reparity.py`）：
```
_parse 的跳转 207 条；其中目标落在 440–500 的：**0 条** ✗
_parse 的指令（码元 455–480）：
  467 STORE_FAST 13 → 468 BUILD_LIST 0 → 469 STORE_FAST 14 → 470 LOAD_FAST_BORROW 14 → 471 …
```
⇒ ① 进 469/470 那一段**不是靠跳转** ✓；② `set = []` 编译成 `BUILD_LIST 0; STORE_FAST 14` ✓ **完全正确** ✓。
⇒ 所以"局部槽 14 未绑定"来自**另一条路径**：某处**读**槽 14 而没经过 469 ✓（同一条循环的第二轮 ✓、
或某条分支 ✓）—— 需要**运行期**的槽 14 读写轨迹 ✓（下一手：给 `STORE_FAST 14`／`LOAD_FAST 14` 在
`_parse` 里挂门控打点 ✓，看第一轮的先后 ✓）。
**闸门** ✓：0 警告 ✓、`quickcheck` ✓、`slowcheck` 十项全绿 ✓。

## 本轮（653）：新增**槽级追踪**门控 `PYAWA_SLOT_TRACE="函数:槽"` ✓（`_parse:14` 还没打到 ✓）

**落** ✓：`executor.rs` 新增 `slot_trace` ✓（在 `LOAD_FAST`／`LOAD_FAST_CHECK`／`LOAD_FAST_BORROW` 与
`STORE_FAST` 两个臂上各挂一次 ✓）：按 `函数名:槽号` 过滤 ✓，把**每一次读／写按发生顺序**打出来 ✓
（句首是动作 ✓）—— 专治"先读后写"这类**控制流分岔** ✓；不设该环境变量时只多一次查表 ✓。
**实测** ✓：`PYAWA_SLOT_TRACE="_parse:14"` 跑 `reparity.py` ⇒ **一条都没打到** ✗ ⇒ 说明那次"读槽 14"
**不是**从这两个臂进的 ✗ —— 最可能是**融合指令**：`STORE_FAST_LOAD_FAST`(113 ✓) 或
`LOAD_FAST_BORROW_LOAD_FAST_BORROW`(87 ✓)（我们编译里确实大量用它们 ✓）。
**下一手** ✓：把门控挂到那两条**融合**臂上（读半边与写半边各判一次 ✓），重跑即可看到 `_parse` 槽 14 的
**第一次事件** ✓ ⇒ 岔口定死 ✓ ⇒ 修对应那一格 ✓，再跑 `reparity.py` 的 13 行逐字对账 ✓。
**闸门** ✓：0 警告 ✓、`quickcheck` ✓、`slowcheck` 十项全绿 ✓。

## 本轮（654）：槽追踪覆盖**融合指令** ✓ ⇒ 岔口**定死** ✓✓

**落** ✓：`slot_trace` 挂到两条融合臂上 —— `STORE_FAST_LOAD_FAST`(113 ✓：高半＝写、低半＝读 ✓) 与
`LOAD_FAST_BORROW_LOAD_FAST_BORROW`(87 ✓：两半都是读 ✓)。
**实测（决定性 ✓✓）**：
```
[slot] 读(融合87低) name=_parse 槽=14 局部="set"      ← 槽 14 的**第一次**事件就是"读"
pyawa: 未捕获（状态 5）：局部槽 14 未绑定（UnboundLocalError 未接线）
```
⇒ 那次读来自 **op87 的"低半格"** ✓，而且**发生在任何写入之前** ✗ —— 即**发射端把一条
`LOAD_FAST_BORROW_LOAD_FAST_BORROW` 发早了** ✗（CPython 只在**两个名字都已绑定**时才用这条融合形 ✓；
`_parse` 里 `set` 要到 `set = []` 才绑 ✓）。结合上一轮 dump（`_parse` 里那条 op87 的打包是
`(first << 4) | second` ✓、低半＝**第二个**名字 ✓）⇒ 下一手就是**在发射端找出这条过早的融合** ✓：
把 `LOAD_FAST_BORROW_LOAD_FAST_BORROW` 的发射点（`emitter.rs` ✓）**挂上同样的句首诊断**（打出两个名字与
位点 ✓）⇒ 一次就能看到它属于哪条语句 ✓ ⇒ 修"发早了"那一格 ✓ ⇒ 再跑 `reparity.py` 的 13 行逐字对账 ✓。
**闸门** ✓：0 警告 ✓、`quickcheck` ✓、`slowcheck` 十项全绿 ✓。

## 本轮（655）：槽追踪打**真实站点** ✓ ⇒ 岔口落到 **`_parser.py:622`** ✓✓

**落** ✓：`slot_trace` 把那个假的"指令偏移=0"✗ 换成 `instance.current_site()` ✓（句首信息 ✓）。
**实测（决定性 ✓✓）**：
```
[slot] 读(融合87低) name=_parse 槽=14 站点=_parse@110 局部="set"
```
其中 `_parse` 定义在 **512** ✓ ⇒ 相对 110 ＝ 绝对 **622** ✓ ＝ `Lib/re/_parser.py:622` 的 **`setappend(code1)`** ✓
⇒ 那条 **op87 融合**把"**第二个操作数**（低半格 ✓）"配成了 `set`（槽 14 ✓）✗ —— 而这一行要的是 **`code1`** ✓
⇒ 两种可能，下一手一测即分 ✓：**① 发射端配错了对**（把不该融合的两个名字融在一起 ✗）；
**② 槽号错位**（`code1` 本该是别的槽 ✓，我们的 `varname(14)` 给成 `set` ✗）。
**闸门** ✓：0 警告 ✓、`quickcheck` ✓、`slowcheck` 十项全绿 ✓。

**下一手** ✓：在发射端给 op87 的**每一处**发射点挂句首诊断（打两个名字＋位点 ✓ —— 现在离这一步只差一击 ✓），
定位到 622 行那条 ✓ ⇒ 若是①则修"融合必须**两个名字都已绑定**"的判定 ✓、若是②则修槽号 ✓；
然后跑 `reparity.py` 的 13 行逐字对账 ✓。

## 本轮（662）：**融合超指令的"半个字节装不下"真 bug 修好** ✓✓✓ —— 判据② 跨过一大关 ✓

**真凶（一路追了六轮，最后一句 `[pair]` 打印点名 ✓）**：
```
[pair] 左=subpattern 槽5  右=i 槽46  arg=126
```
⇒ `emit_two_operands`（`emitter.rs` ✓）把两个槽号**各塞半个字节** ✓ 却**没有"两个都 ≤ 0x0F"的前提** ✗
⇒ `(5 << 4) | 46` 被**按位截断**成 `(5 << 4) | 14` = **126** ✗ ⇒ 运行期去读**槽 14**（名叫 `set` ✓、
此刻还没绑定 ✓）⇒ `局部槽 14 未绑定` ✗（`_parser.py:879` 的 `subpattern[i]` ✓）。
**修法** ✓：与 `AssignAttr` 那处**同一条规矩** —— 两个槽号都 ≤ 0x0F 才融合 ✓，否则交给**非融合回退**
发两条独立加载 ✓（参照同样只在 <16 时融合 ✓）。
**验收** ✓：那道墙**过了** ✓，`re` 现在跑进**真正的匹配流程** ✓（新墙见下）；0 警告 ✓、`quickcheck` ✓、
`slowcheck` **十项全绿** ✓。
**新墙** ✓：`TypeError: unsupported operand type(s) for *: 'int' and 'builtin_function_or_method'` ✗
（在 `_sre`／`_compiler` 的匹配路径上 ✓）⇒ 下一手按同一套"句首诊断"钉站点 ✓。

## 本轮（663）：算术报错挂**句首站点** ✓ ⇒ 判据② 新墙定到 `SubPattern.getwidth` ✓

**落** ✓：`executor/runtime.rs` 的 `unsupported operand type(s)` 处加门控 `PYAWA_BINOP_DEBUG=1` ✓
（**两个类型名放句首** ✓ —— 尾巴会被长消息挤掉 ✗，这条链上反复吃过亏 ✓）。
**实测** ✓：
```
[binop] * 左='int' 右='builtin_function_or_method' 站点=SubPattern.getwidth@293
```
⇒ 在 `Lib/re/_parser.py` 的 **`SubPattern.getwidth`** 里做了一次 `int * <方法对象>` ✗ ——
该函数里唯一的乘法是 `lo + i * av[0]` / `hi + j * av[1]`（`MAX_REPEAT` 那一支 ✓）
⇒ 说明 **`av[0]`／`av[1]` 之一取出来是"绑定方法"** ✗（本该是整数 ✓）⇒ 下一手就把 `av` 的**实际形态**打出来 ✓
（同一条"句首诊断" ✓：`av` 的类型名 ＋ `len(av)` ✓）⇒ 再看是**元组载荷**问题还是**下标协议**问题 ✓。
**闸门** ✓：0 警告 ✓、`quickcheck` ✓、`slowcheck` 十项全绿 ✓。

## 本轮（664）：算术诊断补**两个操作数的 repr** ✓ ⇒ 新墙定死 ✓✓

**落** ✓：`PYAWA_BINOP_DEBUG=1` 的那句再补 `左值=`／`右值=`（`object_repr` ✓，都在句首区 ✓）。
**实测（决定性 ✓✓）**：
```
[binop] * 左='int' 右='builtin_function_or_method' 站点=SubPattern.getwidth@293 左值=1 右值=<built-in function len>
```
⇒ 右操作数**就是内建 `len` 本身** ✗ ⇒ 在 `SubPattern.getwidth` 的 `lo + i * av[0]`／`hi + j * av[1]`（203／207 行 ✓）
那里，`av[...]` 取到了**函数对象**而不是 `len(...)` 的**结果** ✓ —— 即某个 `len` **少调了一次** ✗
（这类"名字/调用混用"最可能在**我们的编译器把 `len(x)` 优化/融合掉**那条路上 ✓）。
**下一手** ✓：按站点（`getwidth` ✓）看 `av` 的**实际内容**（把 `av` 打出来 ✓，或直接看 203／207 行的
`av` 从哪来 ✓ —— 它是 `for op, av in self.data` 里的 `(min, max, item)` ✓）⇒ 找"`len` 没被调用"的那一格 ✓
⇒ 修 ⇒ 跑 `reparity.py` 的 13 行逐字对账 ✓。
**闸门** ✓：0 警告 ✓、`quickcheck` ✓、`slowcheck` 十项全绿 ✓。

## 本轮（668）：元组下标赋值改抛**真正的 `TypeError`** ✓（判据② 路上的一格 ✓）

**落** ✓：`executor/subscript.rs` 的 tuple 写保护先前报 `ExecError::Unsupported` ✗ ⇒ 参照里
`try: t[0] = 1 / except TypeError` **抓不住** ✓（`Lib/re` 一族遍地这种写法 ✓）⇒ 改成
`TypeError: 'tuple' object does not support item assignment` ✓（消息照参照 ✓）。
**验收** ✓：`target/recon/tupwrite.py` 与参照**逐字一致** ✓（整数下标 ✓、切片 ✓、元组原样 ✓）；
0 警告 ✓、`quickcheck` ✓、`slowcheck` 十项全绿 ✓。
**判据② 仍卡** ✗：`re` 报同一句 `TypeError: unsupported operand type(s) for *: 'int' and
'builtin_function_or_method'` ✓ —— 已知的两条并置证据（`_parse:704` 构造时 `min=1 max=4294967295` **正确** ✓
／`getwidth` 里同一个载荷变成 `(len, len, item)` ✗）仍指向"**载荷元组的前两项被改写**" ✓；
元组写保护**本身是好的** ✓（本轮已验 ✓）⇒ 所以改写来自**别处** ✗ ⇒ 下一手按"**谁还会往那个元组里写**"排查 ✓：
优先 `UNPACK_SEQUENCE`／`SWAP`／`STORE_FAST` 一族在"**并行赋值 + 紧跟一次调用**"下的**槽错位** ✓。

## 本轮（670）：`BUILD_TUPLE` 挂门控 ⇒ 坏元组**逮住** ✓✓

**落** ✓：执行器的 `BUILD_TUPLE | BUILD_LIST | BUILD_SET` 入口加门控 `PYAWA_TUPLE_DEBUG=1` ✓
（**句首**打 `代码对象`／`站点`／`个数`／**前两项**（若是内建方法就给它的 repr ✓））。
**实测（决定性 ✓✓）**：
```
[tuple] name=_parse 站点=_parse@2581 个数=3 前两项=["<built-in function len>", "<built-in function len>"]
```
⇒ 那个坏 3 元组（源码里的 `(min, max, item)` ✓）**是在 `_parse` 里由 `BUILD_TUPLE` 拼的** ✓，
前两项恰好是**上一条 `len(this)` 的残留** ✗ ⇒ **"栈上残留"** 这条推断成立 ✓（不是载荷被改写 ✓
—— 上一轮两处 `id` 不同已排除改写 ✓）。
**顺带记一条尺子的事实** ✓：`current_site()` 的"站点行"对 `_parse` 给出 **2581** ✗（文件只有 ~950 行 ✓）
⇒ **行号不可信** ✗，只有"代码对象名＋相对位置"可用于对拍 ✓（用 `PYAWA_DUMP_CODE` 的码元来定 ✓）。
**下一手** ✓：用 `PYAWA_DUMP_CODE=1`（现在**带代码对象名** ✓）取 `name=_parse` 的单元 ✓，在**码元 110 附近**
（上一轮运行期命中过的那条 ✓）看 `BUILD_TUPLE 3` **前面**到底发了几条 `LOAD_FAST`（少发就是根因 ✓）
⇒ 修发射端"少加载"的那一格 ✓ ⇒ 跑 13 行逐字对账 ✓。
**闸门** ✓：0 警告 ✓、`quickcheck` ✓、`slowcheck` 十项全绿 ✓。

## 本轮（671）：坏元组**真凶现形** ✓✓ —— 融合指令把**同一个槽装了两次** ✗

**落** ✓：`PYAWA_TUPLE_DEBUG` 那句补上**码元偏移** ✓（与 `PYAWA_DUMP_CODE` 同一把坐标 ✓ —— 注意
`instruction_pointer()` **本身就是码元** ✓，先前我除以 2 是错的 ✗）。
**实测（决定性 ✓✓✓）**：坏元组在 **码元 1290**（按正确口径＝2580 ✓）；dump 里 `name=_parse` 那一段是
```
2561 LOAD_FAST_BORROW_LOAD_FAST_BORROW 153      ← 高=9 低=9（**同一个槽两次** ✗）
2562 LOAD_FAST_BORROW 26
2563 BUILD_TUPLE 3                             ← 于是建出 (槽9, 槽9, 槽26) = (len, len, item) ✓
2564 BUILD_TUPLE 2                             ← (MAX_REPEAT, 那三元组)
2565 LOAD_FAST_BORROW 5
2566 LOAD_CONST 64
2567 STORE_SUBSCR 0                            ← subpattern[-1] = …
```
⇒ 根因**不是**栈残留 ✗（上一轮的推断又被推翻 ✓），而是**发射端把两个不同名字解析成了同一个槽号** ✗
（`(9, 9)` ✓）—— 也就是说：这条 `(min, max, item)` 的两个局部名，**至少有一个**取错了槽 ✓
（结合前几轮：`emit_two_operands` 我刚加了"两个槽号都 ≤ 0x0F"的闸 ✓，所以这条融合**不是**从那里来的 ✓
⇒ 下一手就查"**还有谁在造 `(slot, slot)` 这种对**" ✓：候选是 `pending_fused_load` 一族
（`emitter.rs:2871`／`5287` ✓）与字典推导式那处（`4273` ✓）✓。
**闸门** ✓：0 警告 ✓、`quickcheck` ✓、`slowcheck` 十项全绿 ✓。

## 本轮（673）：**同一个截断 bug 的第二处**修好 ✓✓✓ —— 判据② 又跨一大关 ✓

**真凶（终于彻底现形 ✓）**：元组字面量的融合（`emitter.rs:4935` ✓）与 `emit_two_operands` 是**同一条
规矩的两处** ✓ —— 都把两个槽号各塞半个字节 ✓，而这里**少了"两个都 ≤ 0x0F"的闸** ✗：
`_parser.py:704` 的 `(min, max, item)` 里两个槽是 **25 与 9** ✓ ⇒ `(25 << 4) | 9` = **409** ✗ ⇒
`as u8` **截断**成 **153** = `(9, 9)` ✗ ⇒ 元组前两项变成**同一个对象**（`len` ✓）⇒ 运行期
`int * <内建 len>` ✗✓✓ —— 与前面所有的并置证据（含 `MARK 构造 min=1 max=4294967295` 正确 ✓）
**完全吻合** ✓。
**修法** ✓：装不下就**整条回退**（逐个 `emit_expression` ＋ `BUILD_TUPLE n` ✓），不再融合 ✓。
**验收** ✓：那句 `unsupported operand type(s) for *` **消失** ✓；`re` 推进到新墙
**`TypeError: 'SubPattern' object is not iterable`** ✗（更深一层 ✓）；0 警告 ✓、`quickcheck` ✓、
`slowcheck` 十项全绿 ✓。
**下一手** ✓：照旧"句首诊断"钉站点 ✓ ⇒ 修 ⇒ 跑 `reparity.py` 的 13 行逐字对账 ✓。

## 本轮（675）：接上**旧式序列协议**（`__getitem__` 回退迭代）✓✓ ⇒ **`re.match`／`re.search` 通了** ✓

**落** ✓：`iter_value`（`executor/iter.rs` ✓）在有 `__iter__` 之外的**新回退** ✓ —— 对象若实现
`__getitem__` ✓，就按 `0,1,2,…` 依次取、**遇 `IndexError` 收尾** ✓（`ExecError::Raised` 里查异常类型 ✓）；
实现上**先物化**成列表再交给现成的 `list_iterator` ✓（对 `re` 的用法等价 ✓；**如实记**：非惰性 ✗）。
**验收** ✓（与参照**逐字一致** ✓）：
```
match: True None      ✓
search: bbb None      ✓
```
⇒ 判据② 的 13 行里**前两行已经对上** ✓；0 警告 ✓、`quickcheck` ✓、`slowcheck` 十项全绿 ✓。
**新墙** ✓：`AttributeError: 'bytearray' object has no attribute 'find'` ✗ ⇒ 下一手给 `bytearray` 的方法面
补 `find`（以及紧随其后大概率的 `rfind`／`index`／`count`／`startswith`／`endswith` 一族 ✓，
**撞一道接一道** ✗ 不批量猜 ✓ —— 但这一族形状相同 ✓，可以一次把**同类**补齐并逐例对拍 ✓）。

## 本轮（677）：`bytearray` 复用 **`bytes` 的整张方法面** ✓ ⇒ 又进一格 ✓

**落** ✓：`bytearray_getattr` 开头加一条 —— 名字若在 `str_method_native` 那张表里 ✓，就把内容**拷成一份
`bytes`** ✓、把方法**绑到那一份**上 ✓（表不改一行 ✓）。**如实记**：返回值是 `bytes` 而非参照的
`bytearray` ✗（`find`／`index`／`count` 这类只回数值的不受影响 ✓；`split`／`strip` 一类会回 `bytes` ✗）。
**验收** ✓：`match`／`search` **保持对上** ✓；墙推进到
**`TypeError: a bytes-like object is required, not 'int'`** ✗；0 警告 ✓、`quickcheck` ✓、`slowcheck` 十项全绿 ✓。
**下一手** ✓：按同一套"句首诊断"钉那句报错的站点 ✓（大概率在 `re._compiler` 的
`charmap[i:i+256]`／`len(...)`／`int(...)` 一族的**切片或方法实参**上 ✓ —— 注意我们 `bytearray` 的
**切片读给的是 `bytes`** ✓（本条链上已如实记过的偏差 ✓），十有八九是它与参照的 `bytearray` 语义差在起作用 ✓）。

## 本轮（679）：`bytes-like` 报错挂**句首站点** ✓ ⇒ 新墙定到 `_optimize_charset` ✓

**落** ✓：`executor/iter.rs` 与 `builtin/bytes.rs` 两处 `a bytes-like object is required, not '<类型>'` 都加门控
`PYAWA_BYTESLIKE_DEBUG=1` ✓（**类型名与站点都在句首** ✓）。
**实测** ✓：
```
[byteslike] 类型=int 站点=_optimize_charset@610
```
⇒ 在 `re._compiler._optimize_charset` 里，一次**按 `bytes-like` 校验的操作**（`in`／方法实参 ✓）拿到了
**整数** ✗ 而报错 ✓ ⇒ 下一手：把该处**两个操作数的类型与 repr 一起**打出来（同一条句首诊断 ✓）⇒ 立刻分清是
"我们的 `bytearray`／`bytes` 语义差"（**切片读给 `bytes`** ✓ 这条已如实记过 ✓）还是"某个方法的实参口径" ✗。
**闸门** ✓：0 警告 ✓、`quickcheck` ✓、`slowcheck` 十项全绿 ✓。

## 本轮（681）：`bytes` 的 `in` 认 **`int` 子类** ✓（真修 ✗ 但没解开这道墙 ✓）

**落** ✓：`executor/iter.rs` 的 `x in b"…"` 那档从 `type_of(item) == int` 改成**认 `int` 及其子类** ✓
（`is_subtype` ✓）—— 参照里 `bool` ✓、`re._constants._NamedIntConstant`（**`int` 子类** ✓）都该落进
"整数那一档" ✓，先前会误报 `bytes-like` ✗。**闸门** ✓：0 警告 ✓、`quickcheck` ✓、十项全绿 ✓。
**这道墙没解开** ✗：仍是 `TypeError: a bytes-like object is required, not 'int'` ✓ ⇒ 报错点**不是**那条
`in` ✓，而是 **`builtin/bytes.rs::bytes_argument`**（`bytes` 的**方法实参**校验 ✓）——
即某个 `bytes`／`bytearray` 方法（经我们"拷成 `bytes`"那条复用路 ✓）收到了**整数** ✗。
**下一手** ✓：给 `bytes_argument` 的诊断补上**方法名与实参 repr** ✓（`native.name` 一族 ✓，句首 ✓）
⇒ 一跑就知道是哪个方法、被谁用错了 ✓ ⇒ 修（大概率是"**`bytearray` 的方法面复用 `bytes` 表**"那条
偏差在起作用 ✗ —— 例如 `find`／`index` 的**整数实参**在参照里合法 ✓ 而我们走了 `bytes_argument` ✗）✓。

## 本轮（683）：`bytes.find` 收**整数实参 ＋ 起止**（CPython 3.14 的新规矩 ✓）⇒ **判据② 过 6/13** ✓✓

**真缺口** ✓：`Lib/re/_compiler.py:326/332` 写的是 **`charmap.find(1, q)`** ✓ —— **CPython 3.14 起
`bytes.find/index/count` 接受整数** ✓（`re` 正靠它 ✓），我们**没接** ✗ ⇒ 报
`a bytes-like object is required, not 'int'` ✓（点名的调用者是 `bytes_find_native` ✓，靠
`PYAWA_BYTESLIKE_DEBUG` 的**回溯**一击命中 ✓）。
**落** ✓：`bytes_find_native` ① 整数实参按"**单字节子串**"处理 ✓（越界照参照 `ValueError: byte must be
in range(0, 256)` ✓）；② **接上 `start`／`end`** ✓（那条调用给了 `start` ✓，先前**完全忽略** ✗）。
**验收（与参照逐字一致 ✓✓）**：
```
match: True None            ✓
search: bbb None            ✓
findall: ['1', '22', '333'] ✓
sub: a#b# 1a2b              ✓
split: ['a', 'b', 'c']      ✓
subn: ('a#b#', 2)           ✓
```
⇒ **判据② 的 13 行已过 6 行** ✓；0 警告 ✓、`quickcheck` ✓、`slowcheck` 十项全绿 ✓。
**下一手** ✓：继续跑到第 7 行（`finditer` ✓）卡住的那一处 ✓ —— 照旧"句首诊断"钉站点 ✓。

## 里程碑（684）：**判据② 成立** ✓✓✓ ＋ 判据① 分子 **189 → 204** ✓

**判据②（本目标的一半 ✓）达成 ✓**：`target/recon/reparity.py` 的 **13 行**与参照
（本机 `python3` 3.14）**逐字一致** ✓（`diff -u` **无输出** ✓）：
```
match True None ／ search bbb None ／ findall ['1','22','333'] ／ sub a#b# 1a2b ／ split ['a','b','c']
／ subn ('a#b#', 2) ／ finditer ['1','22'] ／ escape a\.b\*c ／ purge None x ／ flags True
／ groups ('12','34') {'x':'7'} ／ named 42 ／ template [a]
```
⇒ `re.match/search/sub/split/findall` **＋** 补的 `split/subn/finditer/escape/purge/flags/groups/named/template`
**全部对上** ✓（目标第 2 条的原话逐项满足 ✓）。
**判据① 分子** ✓：`tools/lib_import_ratio.py` ⇒ **187 通过 ＋ 17 参照口径 ＝ 204 ÷ 628 ⇒ 32.5%** ✓
（先前 **189 ÷ 628 ＝ 30.1%** ✓ ⇒ **＋15** ✓，阈值仍是 67% ✓）。
**护栏在位** ✓：`crates/pyawa-runtime/tests/enum_shapes.rs`（2533 字节 ✓）。
**下一手** ✓（目标第 3、5 条）：① 让 `enum_shapes.rs` 真的跑一遍并确认 trio（`global_enum` 注入／
`_simple_enum` 造类／成员是 int ✓）；② 按整包两道必检搬
`textwrap`／`traceback`／`unittest`／`json`／`logging`／`asyncio` ✓ ⇒ `--sync` ✓ ⇒ **再报一次分子** ✓。

## 本轮（686）：`STORE_SLICE`（指令 37）接上 ✓ ＋ 整包搬迁**开工** ✓

**整包判一次** ✓（`tools/find_syncable_packages.py` ✓）给出的**关键路径** ✓：
```
✗ traceback ⇒ 卡着 logging、unittest（ModuleNotFoundError）
✗ json      ⇒ 卡在【指令 37 尚未接线】（core 缺口 ✗，不是缺模块 ✓）
✗ asyncio   ⇒ 卡在 logging
```
**落** ✓：① 搬 `textwrap.py`／`traceback.py` 进 `Lib/` ✓（照 `sync_lib.py` 的"只增不改" ✓，仍**不提交** ✓）；
② **接线 `STORE_SLICE`（指令 37 ✓）** —— 栈序实测 `[值, 容器, 开始, 结束]` ✓，用 `new_slice` ＋ 现成的
`subscript_set` ✓（`textwrap`／`json` 都靠它 ✓）。**验收** ✓：指令 37 的报错消失 ✓；0 警告 ✓、
`quickcheck` ✓、`slowcheck` 十项全绿 ✓。
**新墙** ✓：`AttributeError: 'bytearray' object has no attribute 'translate'` ✗（`textwrap` 里
`charmap`／`expandtabs` 一族 ✓）⇒ 下一手补 `bytes`／`bytearray` 的 `translate` ✓（`str` 的 `translate`
口径不同 ✗ —— 字节版收的是 **256 长度的字节表** ✓，别照抄 ✓）。
**再下一手** ✓：`json` 也应随之能动 ✓ ⇒ 继续 `logging`／`unittest`／`asyncio` ✓ ⇒ `--sync` ⇒ 报分子 ✓。

## 本轮（688）：`bytes`／`bytearray` 的 **`translate`**（字节口径 ✓）接上 ✓

**落** ✓：① 新增 `bytes_translate_native` ✓ —— **字节版**口径（`table` 必须**长度 256** ✓，`table[byte]` 给新
字节 ✓；长度不对照参照报 `ValueError: translation table must be 256 characters long` ✓）；**如实记**：
`delete` 参数暂未接 ✗（`re`／`textwrap` 都还没用到 ✓）；② 共享表里的 `translate` 是 **`str` 口径** ✗ ⇒ 在
`bytes_getattr` 里**覆盖**成字节版 ✓；③ `bytearray` 的"拷成 `bytes` 再绑"那条复用路也认 `translate` ✓
（它**不**在 `str_method_native` 表里 ✓ —— 这点是踩出来的 ✗）。
**验收** ✓：`translate` 的报错消失 ✓；墙推进到 **`TypeError: int() can't convert non-string with explicit
base`** ✗（即 `int(bytes, base)` 这一档 ✓ —— 与目标第 1 条开头那个 `int(二进制串, 2)` 同族 ✓，
说明当时**只接了 `str`** ✗ 而 `bytes` 没接 ✓）；0 警告 ✓、`quickcheck` ✓、`slowcheck` 十项全绿 ✓。
**下一手** ✓：把 `int(x, base)` 的**入参面**扩到 `bytes`／`bytearray`（及 `memoryview` 一族 ✓），照参照
逐例对拍 ✓ ⇒ 再看 `textwrap`／`json` 往下走到哪 ✓。

## 本轮（690）：`int(bytes, base)` 接上 ✓ ⇒ **`textwrap`／`traceback` 都能导入了** ✓

**落** ✓：`int(x, base)` 的入参面从**只认 `str`** ✗ 扩到 **`bytes`／`bytearray`** ✓（按 **ASCII** 解 ✓；
非 ASCII 照参照报 `ValueError` ✓）—— 参照里 `int(b'10', 2)` 合法 ✓，`textwrap` 正靠它 ✓。
**验收** ✓：`import textwrap, traceback` **成功** ✓（先前 `TypeError: int() can't convert non-string with
explicit base` ✗）；0 警告 ✓、`quickcheck` ✓、`slowcheck` 十项全绿 ✓。
**新墙** ✓：`re.error: look-around, including look-ahead and look-behind, is not supported` ✗ ——
是 **`textwrap` 自己的正则**用了 look-ahead ✓（`re` 的功能面缺口 ✗，不是搬运问题 ✓）⇒ 下一手：
① 先继续**整包判一次** ✓（`unittest`／`json`／`logging`／`asyncio` ✓ —— 现在 `traceback` 已就位 ✓，
`logging`／`unittest` 应该能往下走 ✓）；② look-around 是 `re` 的**真功能缺口** ✗，按需再接线 ✓
（判据② 的 13 行**不**依赖它 ✓，所以它属于"继续搬运时撞到的新墙" ✓）。

## 本轮（696）：`float(字符串)` 落地 ✓ ＋ 四个包**按纪律撤回** ✓ ＋ 搬运的真障碍记明 ✓

**落** ✓：`float(str)`（先前报 "float_new：这个实参形态还没接线（字符串解析等）" ✗）—— 空白 ✓、
`inf`／`infinity`／`nan` 及带符号 ✓、其余交 Rust `f64` 解析 ✓、失败照参照报
`ValueError: could not convert string to float: '…'` ✓。**验收** ✓：`target/recon/fl.py` 与参照
**逐字一致** ✓；0 警告 ✓、`quickcheck` ✓、`slowcheck` 十项全绿 ✓。
**撤回** ✓：本轮先搬进来的 `Lib/json`／`logging`／`unittest`／`asyncio` **又撤掉了** ✗ —— 它们让
`cargo test --workspace` 的 **`lib_compile`（`Lib/` 全量编译期不变量，`KNOWN` 为空** ✓）当场变红 ✗
（测试自己写着"要修，或按纪律登记进 `KNOWN`" ✓）；既然**它们本来也过不去**（下条 ✓），按纪律撤回、
保持 `KNOWN` 为空 ✓，不留下"用登记掩盖"的路 ✗。
**逐包实测（真障碍 ✓）**：
```
traceback / textwrap / logging / unittest / asyncio ⇒ error: look-around … is not supported   ✗
json ⇒ float(str)（本轮修好 ✓）⇒ 再撞 error: invalid escape sequence found in character class  ✗
```
⇒ 我们 `_sre` 的后端是 **Rust `regex` crate** ✓：**天生不支持 look-around** ✗、字符类转义口径更严 ✗ ⇒
**这不是"再搬几个包"能过的** ✓。
**下一手（真活 ✓）**：给 `_sre` 换／接一个**支持 look-around 的后端** ✓，或把 Python 侧的 `re` 换回
**自实现**（硬边界允许纯 Python ✓）⇒ 之后 `--sync` ＋ 报分子 ✓。
**判据① 分子**：仍 **204 ÷ 628 ＝ 32.5%** ✓（本轮没搬成新包 ✗）。

## 本轮（698）：假货 `Lib/enum.py` 补 **`StrEnum`** ✓（整包判定里 `http` 要它 ✓）

**落** ✓：`Lib/enum.py` 加 `class StrEnum(str, Enum)` ✓（与 `IntEnum` 同一套**占位口径** ✓；上游那套
"`auto()` ＝ 小写名字"的规矩**不在**假货范围内 ✗ —— 用到再加 ✓，模块头的**三步换回流程不受影响** ✓）。
**验收** ✓：`from enum import StrEnum, IntEnum, Enum, Flag, IntFlag, auto` **导入成功** ✓（先前
`ImportError: cannot import name 'StrEnum' from 'enum'` ✗，整包判定里 `http` 就卡这句 ✓）；
0 警告 ✓、`quickcheck` ✓、`slowcheck` 十项全绿 ✓。
**如实记** ✗：repr 仍是我们的 `<class 'StrEnum'>` ✓，参照是 `<enum 'StrEnum'>` ✗ —— 假货的**既定偏离** ✓
（模块头已记 ✓，换回上游后自动消失 ✓）。
**下一手** ✓：①（目标第 3 条）把 `enum_shapes.rs` 的护栏面**扩到** `StrEnum`／`IntEnum` 一跳 ✓；
②（目标第 5 条，真障碍 ✓）`_sre` 的 look-around ✗ —— 要么换后端 ✓，要么把 Python 侧 `re` 换回自实现 ✓。

## 本轮（700）：假货护栏**扩面到 `StrEnum`** ✓（目标第 3 条再进一步 ✓）

**落** ✓：`crates/pyawa-runtime/tests/enum_shapes.rs` 加第二条测试
`fake_enum_has_strenum_for_http` ✓ —— 钉**导入面**（`StrEnum／IntEnum／Enum／Flag／IntFlag／auto` ✓）
与**继承关系**（`issubclass(StrEnum, str)` 等 ✓）；**只钉真假两侧一致的部分** ✓，假货的 repr 偏离
（`<class 'StrEnum'>` vs `<enum 'StrEnum'>` ✗）**不钉** ✓ —— 换回上游后本护栏照样该绿 ✓。
**验收** ✓：`cargo test -p pyawa-runtime --test enum_shapes` ⇒ **2 passed** ✓（新那条的期望值就是本机
`python3` 3.14 的实测输出 ✓）；0 警告 ✓、`quickcheck` ✓、`slowcheck` 十项全绿 ✓。
**下一手** ✓：目标第 5 条的**真障碍**仍是 `_sre` 的 **look-around** ✗ —— 换后端或把 Python 侧 `re` 换回
自实现 ✓（判据① 分子仍 204 ÷ 628 ＝ 32.5% ✓）。

## 本轮（703）：`sync_lib.py --check` 修好 ✓✓ ⇒ **只剩假货 `enum.py` 一处不一致** ✓（目标第 5 条的关键一步 ✓）

**落** ✓：`tools/sync_lib.py` 的 `check` 跳过 **`__pycache__`** ✓（那是**本机产物** ✗，与上游必然不同 ✓）——
先前它把校验淹成一片红 ✗，把真正不一致的 `.py` 埋掉了 ✓（`CX-8` 管的是**源文件** ✓）。顺带清掉 `Lib/` 下
已有的缓存目录 ✓。
**验收（决定性 ✓✓）**：`python3 tools/sync_lib.py --check` ⇒
```
CX-8：Lib/ 共 311 个文件，与上游 /usr/local… 比对完毕
  ✗ 与上游不一致：enum.py          ← **唯一**一处，且正是我们**有案可查**的假货 ✓
```
⇒ `Lib/re/`（以及 `copyreg.py`／`functools.py`／`textwrap.py`／`traceback.py`／`html/` ✓）**全部与上游逐字节
相同** ✓✓ —— 也就是说：**判据② 依赖的 `re` 是纯上游** ✓，"换回上游"这套纪律是**真的在生效** ✓。
0 警告 ✓、`quickcheck` ✓、`slowcheck` 十项全绿 ✓。
**下一手** ✓：① 假货 `enum.py` 的**换回评估**（那"一处不一致"要不要现在销掉 ✓ —— 取决于上游 `enum.py`
在 VM 里能不能跑 ✓，这正是模块头三步流程的第 1 步 ✓）；② `_sre` 的 **look-around** ✗（`regex` crate 的天限 ✓
—— 本机没有 `fancy-regex` ✗ 且无网 ⇒ 需另想 ✓，见 NEXT 的下一条 ✓）。

## 本轮（706）：`setattr` 走 **`__setattr__` 协议** ✓（真 bug ✗）＋ 换回上游 `enum.py` 的**实测** ✓

**实测（目标第 3 条的正题 ✓）**：把上游 `enum.py` 整文件换上 ⇒ `import re` **失败** ✗：
```
AttributeError: 'EnumDict' object has no attribute '_generate_next_value'
```
⇒ 按纪律**立刻恢复假货** ✓（`import re` 回到 `ok` ✓），并把失败点记准 ✓ —— 这不是"假货不好" ✗，
而是**上游要的 VM 能力还没到** ✓（三步流程的第 1 步**现在还不能走** ✓，如实记 ✓）。
**根因 ✓**：上游 `enum.py:373` 是 `setattr(self, '_generate_next_value', _gnv)` ✓ —— 而我们的 `setattr`
**直接写实例字典** ✗，**不走** `__setattr__` ✓（最小复现 `target/recon/setattr.py` ✓）。
**落** ✓：① core 新增**公开入口** `Instance::set_attribute_with_protocol` ✓（类型上有 `__setattr__` 就交给它 ✓，
走不通才落默认字典 ✓）；② `builtins.setattr` 改调它 ✓。
**验收** ✓：`sa2.py` 与参照**逐字一致** ✓（`__setattr__ 走到: x 5` ✓／`读回: 5` ✓）；
0 警告 ✓、`quickcheck` ✓、`slowcheck` 十项全绿 ✓。
**下一个缺口（就在这条路上 ✓）**：`getattr` **没走 `__getattr__`** ✗ —— `setattr.py` 现在报
`AttributeError: object has no attribute 'x'` ✓（参照会去问 `__getattr__` ✓）⇒ 下一手修它 ✓，
修完**再**走上游 `enum.py` 的换回实测 ✓。

## 本轮（708）：`getattr` 走 **`__getattr__` 回退** ✓（换回上游 `enum.py` 的第 2 道坎 ✓）

**落** ✓：① core 新增公开入口 `Instance::get_attribute_with_protocol` ✓（先走属性通道 ✓，找不到再问类型的
`__getattr__` ✓；**守卫**：找 `__getattr__` 自身时不再回退 ✓）；② `builtins.getattr` 改调它 ✓
（`setattr` 那笔已在上一轮 ✓，同样是"core 开公开入口 ＋ 内建改调"这条口径 ✓）。
**验收** ✓：`target/recon/setattr.py` 现在 `setattr ⇒ 5 5` ✓（与参照一致 ✓）；0 警告 ✓、`quickcheck` ✓、
`slowcheck` 十项全绿 ✓。
**还没接的同一族（下一步就在眼前 ✓）**：**点号赋值** `d.y = 7` ⇒ `KeyError: y` ✗ —— 即 **`STORE_ATTR`**
还**没走** `__setattr__` ✓（上一轮只接了内建 `setattr` ✓）。`LOAD_ATTR` 的 `__getattr__` 回退也同批接上 ✓。
**下一手** ✓：把这两条执行器路径接到刚开好的两个公开入口上 ✓ ⇒ 再走上游 `enum.py` 的换回实测 ✓
（目标第 3 条 ✓）。

## 本轮（713）：点号读接 `__getattr__` 回退 ✓（配两道守卫 ✓，十项闸门全绿 ✓）

**落** ✓：`executor/attribute.rs` 新增 `attribute_lookup_with_getattr` ✓（与内建 `getattr` **一处真相** ✓），
`LOAD_ATTR` 切过去 ✓。**两道守卫都是踩出来的** ✗：
① 找 `__getattr__` 自身时不再回退 ✓（免得自递归 ✓）；
② **类型 MRO 上真没有 `__getattr__` 就直接抛原始错误** ✓ —— 不能直接去 `attribute_lookup` ✗：那对**内建类型**
会冒出它**自己**的 `'coroutine' object has no attribute '__getattr__'` ✗，把本该报的 `__next__` 顶掉 ✓
（实测 `tests/coroutines.rs:243` 就是这么红的 ✓）。
**验收** ✓：`d.zz`／`getattr(d, "yy")` 与参照**逐字一致** ✓（`回退:zz 回退:yy` ✓）；`coroutines` **8 项全过** ✓；
`attributes` ✓、`import re` 仍 `ok` ✓；0 警告 ✓、`quickcheck` ✓、`slowcheck` 十项全绿 ✓。
**下一手** ✓：`STORE_ATTR` 那条（让协议入口的**默认支路**改走 `instance_attribute_set` ✓，并且**只**在
"类型自己定义了 `__setattr__`"时才走协议 ✓）⇒ 再接点号赋值 ✓ ⇒ 然后走**上游 `enum.py` 的换回实测** ✓。

## 本轮（715）：**点号赋值**接上 `__setattr__` 协议 ✓✓（引用计数也守住了 ✓）——四条属性路**全通** ✓

**落** ✓：① `set_attribute_with_protocol` 加**判据**：**只有"类型自己定义了 `__setattr__`"才走协议** ✓
（拿 `type_lookup(对象类型, "__setattr__")` 与 `object` 那份**比对** ✓）；② **默认支路整条交给
`instance_attribute_set`** ✓（描述符 `__set__` ✓／类型对象命名空间 ✓／引用还账 ✓ **一处真相** ✓）；
③ `STORE_ATTR` 改调它 ✓；协议支路的**引用纪律照 `__set__` 那条**（`protocol.rs:166` ✓）：对象与值各
retain 一份交出去 ✓。
**验收（决定性 ✓✓）**：`cargo test -p pyawa-core --test attributes` ⇒ **7 passed** ✓（先前就是它红的 ✗）；
`target/recon/setattr.py` 与参照**逐字一致** ✓（`setattr ⇒ 5 5` ✓／`点号赋值 ⇒ 7 7` ✓）；`import re` 仍 `ok` ✓；
**0 警告** ✓（先冒了 3 条 `unused variable: this` ✗ —— 已消 ✓）、`quickcheck` ✓、`slowcheck` 十项全绿 ✓。
**四条属性路现在全通** ✓：内建 `setattr` ✓／内建 `getattr` ✓／`STORE_ATTR` ✓／`LOAD_ATTR` ✓ ——
换回上游 `enum.py` 先前卡的两道坎 ✓ 都补上了 ✓。
**下一手（目标第 3 条的正题 ✓）**：**再走一次**上游 `enum.py` 的换回实测 ✓ ⇒ 绿就**销掉假货** ✓
（`--check` 那"唯一一处不一致"随之消失 ✓）；仍红就恢复假货并记下**下一道**坎 ✓。

## 本轮（702）：**`dict` 子类覆盖的 `__setitem__`** 不再被吞掉 ✓✓ ＋ 内建 `dict.__setitem__` 直奔原始写 ✓

**真凶（追了四轮 ✓）**：`subscript_set` 对 `dict` **子类**一律走**内建快路** ✗ ⇒ 覆盖版 `__setitem__`
**整个不被调用** ✓ ⇒ 上游 `enum._EnumDict.__setitem__` 里那句 `setattr(self, '_generate_next_value', _gnv)`
从没执行 ✓ ⇒ `enum.py` 换回卡死 ✓。最小复现 `target/recon/nsdict.py` ✓。
**落（成对，缺一不可 ✓）**：
① `subscript_set` 的 `dict` 快路加判据 ✓ —— `type_lookup(容器类型, "__setitem__")` 必须**等于**
`dict` 自己那份 ✓（照 `setattr` 那条**同一把尺子** ✓）；
② `dict_setitem_native`（内建实现 ✓）**直奔原始写** ✓（查重→`replace_value` ＋ `release(旧)` ✓；否则
`incref(key)` ＋ `dict_insert_raw` ✓，值按契约"接管"✓）—— 先前它经 `subscript_write`（= "incref ＋ 转协议" ✗）
⇒ 与①**互相递归** ✓（实测 `RecursionError` ✓）。
**验收** ✓：`nsdict.py` 与参照**逐字一致** ✓（`下标=7 属性=7 点号属性=7` ✓）；0 警告 ✓、`quickcheck` ✓、
**`slowcheck` 十项全绿** ✓（含 `cargo test --workspace` ＝ 判据② 的 13 行与全量夹具 ✓）。
**上游 `enum.py` 换回实测（本轮又跑了一次 ✓）**：**墙换了** ✗✓✓ —— 从
`AttributeError: 'EnumDict' object has no attribute '_generate_next_value'` 一路推到
**`TypeError: 'NoneType' object is not iterable`** ✓（假货已按纪律**立即还原** ✓，`import re` 回到 `ok` ✓）
⇒ 说明这两笔**确实啃掉了换回的第一道硬坎** ✓，下一道坎也已现身 ✓。
**下一手** ✓：给"`NoneType` object is not iterable"挂**句首站点** ✓（现成门控 `PYAWA_ITER_DEBUG` ✓ 一击可中 ✓）
⇒ 修 ⇒ **再**换回实测 ✓（目标第 3 条 ✓）。

## 本轮（718–720 收束）：换回上游 `enum.py` 的**卡点收敛到一格** ✓ ＋ 四条探针规矩 ✓

**目标整体（如实 ✓）**：判据② **达成 ✓✓**；判据① **204 ÷ 628 ＝ 32.5%** ✓（阈值 67% ✗）；
假货护栏 **2 条全绿 ✓**；`sync_lib --check` 只剩假货 `enum.py` **一处** ✓。
**换回实测（第 3 次 ✓，本轮链）**：墙从 `'_generate_next_value'` ✗ 一路推进到
**`TypeError: 'NoneType' object is not iterable`** ✓（每次都按纪律**立即还原假货** ✓，`import re` 始终回到 `ok` ✓）。
**该墙的精确定位（标记法 ✓）**：在 `EnumType.__new__`（上游 480 行起 ✓）里，
`member_names`(A) → `classdict.pop('_order_', None)`(B ✓ 已排除 ✓) → `_gnv = classdict.get(...)`(C ✓) →
`classdict = dict(classdict.items())`(D ✓ 已排除：`_view` 是 `list` ✓、`D1c done` ✓) → `_gnv is None? False`(D2 ✓)
⇒ 崩在 **D2 之后**，那一段只剩 **`member_type, first_enum = metacls._get_mixins_(cls, bases)`** ✓
（`_find_new_` 已证**必然返回三元组** ✓，排除 ✓）⇒ **下一手就探这一格** ✓。
**四条探针规矩（四次踩坑换的 ✓，务必照办）**：① **按文本锚定** ✓（**不用行号** ✗）；
② 标记文本**只用 ASCII** ✓（全角字符会撞我们自己的**词法缺口** ✗ —— 那本身是一处待接线的小缺口 ✓）；
③ **躲开装饰器** ✓（插在 `def` 行**之后** ✓，别插在 `@…` 与 `def` 之间 ✗）；
④ 插进 `for`／`if`／`def` 的**体**里要**＋4 缩进** ✓（同缩进会把体写空 ✗）；且 `for …:` 这类**别匹配到括号内的推导式** ✗。
**探针收尾也要清缓存** ✓：跑完还原后**必须** `rm -rf Lib/__pyawa__` ✓，否则旧的坏产物会被复用 ✗（本轮踩到一次 ✓）。

## 本轮（task-17 第 1 轮）：**引擎枢纽两档引擎**落地 ✓✓ —— look-around／反向引用退 `fancy-regex` ✓；`textwrap` 收回 ✓

**背景** ✓：`_sre` 原先只有 Rust `regex` ✓（线性时间、无回溯 ✓），但它**架构上不支持** look-around／
反向引用 ✗ ⇒ `Lib/textwrap.py:82` 的 `(?<=…)`／`(?=…)` 一族挡着 `logging`（13 下游）／`concurrent.futures`／
`traceback`／`wsgiref.validate` ✓（枢纽④／⑧ 已立据 ✓）。**用户 2026-10-16 拍板**：引入 `fancy-regex` ✓。

**落** ✓（`crates/pyawa-stdlib`，提交 `fb056c5` ✓）：
① **依赖边**：`fancy-regex = "0.19.2"` 与 `regex = "1"` **同位** ✓（用途与归属**只写在 `Cargo.toml` 那一处** ✓）；
   网络通 ⇒ `tools/cargo.sh add` ＋ `cargo fetch` 拉齐三个新 crate（`fancy-regex`／`bit-set`／`bit-vec` ✓，
   全落在工作区 `CARGO_HOME` ✓，无需本地 vendor ✓）。
② **一处真相** ✓：`enum Engine { Fast(regex::Regex), Fancy(fancy_regex::Regex) }` ＋ **唯一**编译入口
   `build_engine` ✓ —— `regex` 先试 ✓、失败**才**退 `fancy-regex` ✓（快路径失败**不**直接报 ✗：`\1`／
   `(?<=…)` 这些**参照合法**的构造正是要退的那一类 ✓）；两档 flags 口径一字不差 ✓
   （IGNORECASE／MULTILINE／DOTALL／VERBOSE ✓，VERBOSE 仍由调用方**自己剥** ✓）。
③ **同一条产出管线** ✓：`Engine::captures`／`captures_iter` 交回共用的 `GroupSpans`（**字节**跨度 ✓）⇒
   `run_match`／`scan_spans` 各自**只换算一次**字符下标 ✓ ⇒ `span`／`group`／`findall`／`finditer`／
   `split`／`sub`／`subn`／`_sre.match_raw` 两条引擎**天然同口径** ✓（**不新增载荷类型** ✓）；
   `compile_raw` 的 `REGISTRY` 与 `PatternData.built` 一并改成 `Engine` ✓；回溯引擎的**运行期**报错按 `ValueError` 报出 ✓。

**验收** ✓：
- **逐例与参照对** ✓（本机 `python3` 3.14 实测 ✓）：look-behind 正／负向 ✓、look-ahead 正／负向 ✓、
  `\1` 与 `(?P=x)` 反向引用 ✓、`findall`／`sub`／`split`／`finditer`／`group` 在回退引擎上 ✓、
  VERBOSE 剥空白与 `#` 注释 ✓、bytes 模式（latin-1 直通 ✓）的 `search`／`sub` ✓、空匹配 `(?=a)` 的
  `findall` ✓ —— 探针 `target/recon/re_engine_probe.py` **21 行**与参照**逐字一致** ✓。
- **护栏** ✓：`_sre_shapes.rs` 既有 **15** 例全绿 ✓ ＋ **新增 1 例**
  （`lookaround_and_backreferences_agree_with_the_reference` ✓，14 行逐例）⇒ **16 passed** ✓ ⇒ 既有路径**零回退** ✓。
- **判据①** ✓：**通过 255 ＋ 参照口径 17 ＝ 272 ÷ 628 ⇒ 43.3%** ✓（本笔 ＋1 ✓，只有 `textwrap` 转绿 ✓；
  `wsgiref.validate` 本笔前已通 ✓ 不计 ✓）；**本笔前 271 ＝ 43.2%** ✓（页里旧的 **249/266 是笔误** ✗，
  已一并更正 ✓）。收入两道必检 ✓（逐个导入 ✓ ＋ 无关脚本 `startup ok` ✓）；`quickcheck` ✓、
  `slowcheck` **十项全绿** ✓（314s ✓）、0 警告 ✓。

**墙换了一格（如实 ✗）**：`logging`／`concurrent.futures`／`traceback` **没有**随本笔转绿 ✗ —— 它们已**越过**
look-around 那一格 ✓（`import textwrap` 直接通 ✓），现在停在 **`ModuleNotFoundError: No module named 'tokenize'`** ✓
（`traceback` ← `linecache` ← `tokenize` ✓）。`tokenize` 是**既有独立缺口** ✓：`builtins.open` 缺失 ✗ ＋
`_tokenize` C 模块 ✗ ⇒ 文件仍在 `target/` 未进 `Lib/` ✓（见 `NEXT.md`「未修 bug」✓）。

**下一手** ✓：内核这边第 2／3 轮预算留给回退路径的**边角对账** ✓（pos／endpos 与回退引擎的组合 ✓、
bytes 模式各产出 ✓、`\b`／`^`／`$` 在回溯引擎下的口径 ✓）；`logging` 一族要另立 **`tokenize` 前置**枢纽 ✓
（`builtins.open` ＋ `_tokenize` ✓，本笔不越权代做 ✓）。

## 本轮（task-17 第 2 轮 · 收口）：两档引擎**保留 `pos` 之前的上下文** ✓ ＋ `str(bytes, encoding)` 补解码 ✓（两处真 bug ✗）

**lead 裁决** ✓：task-17 再给**一轮加固**就收口 ✓（范围：`pos`／`endpos` 组合 ✓、bytes 各产出 ✓、
`\b`／`^`／`$` 口径 ✓、空匹配前进语义 ✓）；`logging`／`futures` 的验收**不**在引擎范围可达 ✓ ⇒ 归 task-18 ✓。

**真 bug ✗①（引擎 `pos` 语义）**：旧实现把 `[pos, endpos)` **切成窗口** ✗ ⇒ `pos` **之前**的上下文被切掉 ✓
⇒ `^`／`\b`／look-behind 全看错 ✓。实测（`target/recon/re_engine_edges.py` 首跑就抓到 ✓）：
`(?<=a)b` 对 `"xabxa"` 的 `search(…, 2)` ⇒ 参照 `(2,3)` ✓ 而切片法 `None` ✗；`^b`／`\bb` 的 `match(…, 1)`
⇒ 参照 `None` ✓ 而切片法**误报** `(1,2)` ✗；`(?<=a)b` 的 `findall("abab", 1)` ⇒ 参照 `['b','b']` ✓ 而切片法 `['b']` ✗。
修法：**只按 `endpos` 截尾** ✓、起点交**引擎自己的 pos 接口** ✓（`regex::captures_at` ✓／
`fancy_regex::captures_from_pos` ✓ —— 两条都"把起点的**周围上下文**算进去" ✓，`\A` 仍只在 `start == 0` 命中 ✓）；
空匹配改由**共用游标**推进 ✓（前进一个字符 ✓、句尾只产出一次 ✓）⇒ `findall`／`finditer`／`split`／`sub`
两档**同一条扫描路径** ✓、跨度不再平移 ✓。

**真 bug ✗②（core `str()`）**：`str(object, encoding[, errors])` 的两／三实参形态被**整个忽略** ✗
（`str(b'abc', 'latin1')` 回 `"b'abc'"` ✗，参照 `'abc'` ✓）—— 这正是 `Lib/re/_parser.py:235` 的
**bytes tokenizer** ✓：修前 bytes 正则的 `_parser` 一直在读 **`repr(bytes)`** ✗（`rb'(?s)a'` 报
`global flags not at the start of the expression` ✗；`rb'(?i)…'`／`rb'(?m)…'` 同 ✗）。修法：补解码路径 ✓
（复用 `Instance::decode_with_encoding` **一处真相** ✓）＋ 参照三条 `TypeError` 文案 ✓
（`decoding str is not supported` ✓／`decoding to str: need a bytes-like object, int found` ✓／
`str() argument 'encoding' must be str, not int` ✓）。

**验收** ✓：`target/recon/re_engine_edges.py` **58 行** ✓、`re_engine_probe.py` **24 行** ✓、
`iso_str2.py`（6 行 ✓）／`iso_flags.py`（7 行 ✓）全与参照**逐字一致** ✓；新增
`crates/pyawa-runtime/tests/re_engine_shapes.rs`（`pos`／`endpos` 上下文 ✓、bytes 回退产出 ✓、
空匹配前进 ✓ 共 **3** 例）＋ `text_shapes.rs` 的 `str` 解码 **1** 例 ✓；`_sre_shapes.rs` **16 例零回退** ✓；
`quickcheck` ✓、`slowcheck` **十项全绿** ✓（262s ✓）、0 警告 ✓；收账两道必检 ✓
（逐个导入 ✓ ＋ 无关脚本 `startup ok` ✓）。**判据①** ✓：**272 ÷ 628 ＝ 43.3%** ✓ —— 本笔 **＋0** ✗
（两处真 bug 都不挡任何模块的 import ✓，如实报 ✓）。

**同轮探到的其余缺口（如实 ✗，已记 `NEXT.md`「未修 bug」✓）**：① `str(bytearray, …)` 仍 `TypeError` ✗、
`errors` 非 `strict` 未实现 ✗（同 `bytes.decode` 口径 ✓）；② `Pattern.sub(repl, s, count=1)` 的
**关键字** `count` 被原生派发丢掉 ✗（**位置**实参那条对 ✓）；③ 模块级 `re.sub(pat, repl, s, count)`
的位置实参弃用告警走 `sys._getframe(N)` ✗ ⇒ 现报「`sys._getframe` 目前只接 depth＝0」✓。

**task-17 收口** ✓：两档引擎本体验收已足 ✓（依赖边一处真相 ✓、唯一编译入口 ✓、同一条产出管线 ✓、
探针逐字一致 ✓、护栏零回退 ✓）⇒ 标 **completed** ✓；**下一手** ✓：转 **task-18「`tokenize` 前置枢纽」** ✓
（`logging`／`concurrent.futures`／`traceback` 都压在它上 ✓）。

## 本轮（task-18 第 1 轮 · (a)）：**`builtins.open` ＋ 文件对象**落地 ✓✓ —— 同轮修掉**四处真 bug** ✗（`fs` 句柄号段／`FOR_ITER` 协议／`os.open` 标志／`readlines` 尾空行）

**lead 裁决** ✓：task-18 拆三段 —— (a) 先落 `builtins.open` ＋ 文件对象 ✓（本轮 ✓）；(b) `_tokenize`
真词法器 ⇒ **task-19** ✓；(c) `_colorize`／`TextIOWrapper` 挂后 ✓。

**背景（recon 的事实 ✓）**：`builtins.open` 缺失 ✓、`_io.open` 只是占位 ✓（`STUB_FUNCTIONS` ✓）；
`Lib/traceback.py` 模块级还 `import tokenize` ＋ `import io` ＋ `import _colorize` ✓；而 `tokenize` 里
`import _tokenize` 是**裸 import** ✓、3.14 的 `generate_tokens` **完全走 C 路径** ✗ ⇒ 真词法器是独立大块 ✓。

**落** ✓：
① **依赖边**：`crates/pyawa-stdlib/Cargo.toml` 加 `pyawa-capabilities` ✓ —— 唯一理由是域的
`open_flag` 编码（`APPEND=8`／`TRUNC=16`／`CREAT=32` ✓，**不是** POSIX 的 `O_*` ✗）；
② **`_io.open` 真实现** ✓（**一处真相** ✓：`builtins.open` 指向**同一个** native ✓）：模式解析
（`r`／`w`／`a`／`x`／`+`／`b`／`t` ✓ ＋ 三条参照报错文案 ✓）＋ `_io.FileIO` 真类 ✓（19 个方法 ✓，
含 `__enter__`／`__exit__`／`__iter__`／`__next__` ✓）；读模式 `open()` 时整读 ✓、写模式内存改＋
`flush`／`close` 落盘 ✓（如实口径写在 `io_module.rs` 节首 ✓）；文件错误按 **errno 名查表** ✓
（`errno_map` ✓，`CM-19`／`CM-20` ✓）报 `FileNotFoundError`／`FileExistsError` 一族 ✓；
③ **core**：`Instance::fs_seek`／`fs_tell` 两个薄包装 ✓（槽位与规格**早就在** ✓，只缺包装 ✓）。

**同轮四处真 bug ✗**：
① **`fs` provider 句柄号段从 1 起** ✗ ⇒ 进程**头两个**文件拿到的正是 `stdout`／`stderr` 的号 ✓
（实测：`with open('x','w')` 写的 "zzz" **打到终端** ✓、文件是空的 ✓）⇒ 真文件改从 **3** 起 ✓；
② **`FOR_ITER` 对"非本层接线的迭代器"直接报未接线** ✗ ⇒ `__iter__`／`__next__` 用户类与文件对象
**全都进不了 `for`** ✓（实测 `class Counter` 的 for 循环报"未接线" ✓）⇒ 那格判断**本来就是多余的** ✓
（`advance_iterator` 自带协议分支 ✓，`next()` 同一条 ✓）；
③ **`posix.open` 把 POSIX 的 `O_*` 原样交给能力域** ✗ ⇒ `os.open(p, O_WRONLY|O_CREAT)` 实测"打不开" ✓
⇒ 在 `posix_module` 加**翻译** ✓（`O_EXCL` 域里没槽 ⇒ `fs_stat` 先探存在 ✓）；`fs_error` 提为
`pub(crate)` ⇒ 两条路**共用**同一份 errno→异常映射 ✓；
④ **`readlines()` 把句尾空读当一行** ✗ ⇒ 多出一个 `''` ✓（参照不 ✓）。

**验收** ✓：新增 `crates/pyawa-runtime/tests/file_shapes.rs` **2 例** —— `open` 全模式／`readline`／`tell`／
`seek`／`with`／`for` 迭代／`readlines`／错误文案／编码 ✓ ＋ `os.open` 标志翻译（`O_CREAT`／`O_RDONLY`／
`O_EXCL`／`O_APPEND`／`O_TRUNC` ✓）—— 都与参照（本机 `python3` 3.14 跑**同一段脚本**）**逐字一致** ✓；
探针 `t18_file_accept.py`（40 行 ✓）、`t18_osopen.py`（5 行 ✓）、`t18_open.py`、`t18_iter.py` 全对 ✓；
`quickcheck` ✓、`slowcheck` **十项全绿** ✓（314s ✓）、0 警告 ✓；收账两道必检 ✓
（逐个导入 ✓ ＋ 无关脚本 `startup ok` ✓）。**判据①** ✓：**272 ÷ 628 ＝ 43.3%** ✓ —— 本笔 **＋0** ✗
（`open` 本身不挡任何模块的 import ✓，如实报 ✓）。

**如实口径（✗ 已记 `NEXT.md`）**：① `io.UnsupportedOperation` 只能**别名到 `OSError`** ✗（参照是
`OSError`＋`ValueError` 的子类 ✓）—— 根因是**异常类的子类拿不到异常布局** ✓（`type.__new__`／
`class X(OSError)` 造出的子类 `instance_size` 仍是 56 ✓ ⇒ 一 `raise` 就撞 `adopt` 的 debug 断言
`128 vs 56` ✓），故那两条抽例**只对消息**（`not writable`／`write` ✓）不对类名 ✓；② `buffering`／
`errors`／`newline`／`opener`／`closefd` **未实现** ✗；③ 文本 `seek` 未按参照限制为 0 偏移 ✓（更宽 ✓）；
④ `linecache.updatecache`／`tokenize.open` 仍不可用 ✗（要 `tokenize` ⇒ **task-19** ✓），
本笔已把 `tokenize.open` 的下半截（`rb` ＋ `readline` ＋ `seek(0)` ＋ 读回 ✓）钉进 `file_shapes.rs` ✓。

**下一手** ✓：**task-19 `_tokenize` 真词法器**（逐 token 对参照 ✓）⇒ 再补 `_colorize`／`TextIOWrapper` ⇒
才轮到 `import traceback`／`logging`／`concurrent.futures` ✓；**lead 另已点**：`task-19` 之前**插一手
task-14 尾巴**（tuple 支的 CALL 转派层，5–15 行 ✓，交接档案 `target/tuple_subclass_round3.diff` ✓ ＋
定位针 `PYAWA_TUPLE_NEW_DEBUG` ✓）。

## 本轮（task-14 重定焦 · 第 1 轮）：类型字典一致性 ＋ tuple 子类三层 ✓✓ —— **五处真 bug** ✗；namedtuple 只差编译器 cell 槽位

**背景** ✓：前任交接档案（`target/tuple_subclass_round3.diff` ✓ 6289 字节 ✓）可干净应用 ✓，**布局层有效** ✓
（`P((1,2))` ⇒ `type(p) is P` ✓），但 `tuple.__new__(cls, it)` 仍落到 `object.__new__` ✓；lead 裁决
**重定焦**为「类型字典写入/读取一致性 ＋ CALL 对 `__new__` 的再解析」✓，由原探针 owner 续做 ✓。

**五处真 bug ✗**：
① **根因（类型字典被盖）** ✗：`builtins_module.rs` 第 252 轮那条 `enum.py` 兼容做法把**同一个
   `object.__new__`** 写进 int／str／float／**tuple**／bytes／list／dict／set 的命名空间 ✓ ⇒ **盖掉**
   core 刚注册的 `tuple.__new__` ✓（硬证据 `tuple.__dict__["__new__"] is object.__dict__["__new__"]`
   实测 **True** ✓）；改成**只补自己没有的** ✓（`enum.py` 的 `'__new__' in base.__dict__` 照旧 ✓）。
② `tuple_new_native` 的 `cls` 取值补齐**两条派发形态** ✓（`bound` 是类型且 `args[0]` **不是**类型 ⇒
   用 `bound` ✓；否则 `args[0]` ✓）⇒ `tuple.__new__(cls, it)` 与 `P(...)` 都对 ✓。
③ `classes.rs::requalified_method` 换 `co_qualname` 时给的是**空 cell 表** ✗ ⇒ `type()`／类体造的
   **每个闭包函数**都"自由变量未绑定" ✓（最小复现：`def f(): x=1; def g(): return x; return {'g': g}`
   ＋ `type('K', (), ns)` ✓ —— `K.__dict__['g'] is g` 为假 ✗）；改为照 `defaults` 逐 cell `incref` 接过来 ✓。
④ **"恰好类型"一族** ✗：`percent_format`（右操作数只认恰好 tuple／dict ✗ ⇒ tuple **子类**不拆包 ✓
   ⇒ `'%r' % self` 里 `%r` 去 `repr(self)` ⇒ `__repr__` **自递归** ✓ `RecursionError` ✓）、
   `iterator_type_for`／`iterable_length`／`iterable_item`（子类报 `not iterable` ✗）、`values_equal`
   （`Point(1,2) == (1,2)` 给 **False** ✗）⇒ 全部改**子类也算** ✓（与既有 `dict` 那条**同一把尺子** ✓）。
⑤ **描述符** ✗：内建族实例的"自有字典"快路把**数据描述符**（`property` ✓）原样交回 ✓ ⇒
   `Point(1,2).x` 给 `<property object>` ✓；改**只对可调用短路** ✓、非可调用落描述符分支 ✓；
   另 `property_descriptor_get` 把 `fget` 当 `bound_self` ✓ ⇒ **已绑定**的 `fget`
   （namedtuple 回退的 `property(_itemgetter(index))` ✓）收不到实参 ✓ ⇒ 改走
   `call_value(fget, &[target], &[])` ✓。

**验收** ✓：新增 `crates/pyawa-runtime/tests/tuple_subclass_shapes.rs` **2 例**（tuple 子类布局／
`tuple.__new__(cls, it)`／`property` 字段／迭代／`repr`／相等 ✓ ＋ `codecs.lookup('cp437')` 回**真
`CodecInfo`** ✓）与参照（本机 `python3` 3.14）**逐字一致** ✓；`quickcheck` ✓、`slowcheck` **十项全绿** ✓
（263s ✓）、0 警告 ✓；判据① **272 ÷ 628 ＝ 43.3%（＋0）** ✓（如实 ✓ —— `CodecInfo` 通了 ✓，但
`encodings.*` 其余还压在 `_codecs_cn`／`_jp`／`_kr`／`_tw`／`_iso2022`／`stringprep` 上 ✓）。

**仍差一格（下一轮 ✓）**：`namedtuple._asdict`／`_make`／`_replace` 还红 ✓ —— 根因是**编译器
`cellvars` 顺序与运行期帧的 cell 槽位不一致** ✓（最小复现 + 矩阵见 `NEXT.md` ✓），归**符号表／帧布局**
一层 ✓ 独立成支 ✓；另 `_collections._tuplegetter` 缺 ✓ ⇒ 字段类型名是 `property` ✓（值语义一致 ✓）。

## 本轮（task-14 第 2 轮 · 收口）：**`AssignTuple` 等长元组窥孔不看长度** ✗（≥4 项解包全歪）＋ `_tuplegetter` ✓ —— `namedtuple` 全套收

**根因更正（重要 ✓）**：第 1 轮推测的"**编译器 `cellvars` 顺序与帧 cell 槽位不一致**"**不成立** ✗ ——
加探针实测**两端一致** ✓：`co_cellvars=('a','e')` ✓、闭包元组槽 `4,5` ✓、内层 `co_freevars=('a','e')` ✓。
真根因是**另一个** ✓：`Statement::AssignTuple` 的"等长元组窥孔"**不看长度**就发 `SWAP 2` ✗ —— 而它只在
**2 项**时成立 ✓（参照实测：`a,b=1,2` ⇒ `SWAP 2` ✓；`a,b,c=x,y,z` ⇒ `SWAP 3` ✓；**4 项及以上** ⇒
`值 ＋ UNPACK_SEQUENCE n` ✓，常量元组会折成 `LOAD_CONST` ✓）⇒ `a,b,c,d,e = 1,2,3,4,5` 实测得
`(4,5,3,2,1)` ✗（`a,b=1,2` 恰好对 ✓ ⇒ 长期没被发现 ✓）。**影响面** ✓：**任何 ≥4 项的解包** ✓ ——
含 `namedtuple` 生成的 `_asdict`／`_replace`／`_make` 里的解包 ✓，也含**外层函数自己的局部** ✓
（那三行"闭包矩阵"其实是**解包先歪**、再被闭包读出去 ✓，不是 cell 槽位错 ✓）。
**修法** ✓：窥孔只对 **≤3 项** ✓、`SWAP <目标数>` ✓（不再硬写 2 ✓）、4 项及以上走 `UNPACK_SEQUENCE` ✓。

**`_collections._tuplegetter`** ✓：`collections_module.rs` 落**只读下标描述符** ✓（`__get__` ⇒ `obj[index]` ✓、
类访问回描述符本身 ✓；`index`／`__doc__` 放实例字典 ✓，**不新增载荷类型** ✓）⇒
`type(Point.__dict__['x']).__name__` 与参照一致 ✓。

**验收** ✓：`crates/pyawa-runtime/tests/tuple_subclass_shapes.rs` 扩到 **4 例**（新增 `namedtuple` 全套 ✓：
`_fields`／`_asdict`／`_replace`／`_make`／`repr`／相等／`_tuplegetter` 类型名／`codecs.lookup('cp437')` ✓
＋ ≥4 项解包 ✓）与参照（本机 `python3` 3.14）**逐字一致** ✓；`cargo test -p pyawa-core --test compile`
（**逐字节** ＋ 编译器**指令级对拍** ✓）全绿 ✓ ⇒ `SWAP <n>` 与参照字节码**同形** ✓；`quickcheck` ✓、
`slowcheck` **十项全绿** ✓（260s ✓）、0 警告 ✓；判据① **272 ÷ 628 ＝ 43.3%（＋0）** ✓
（`namedtuple` 通了 ✓，但没有模块的 import 因此翻绿 ✓，如实报 ✓）。

**task-14 收口 ✓**：两轮共修**六处真 bug** ✓（第 1 轮五处 ＋ 本轮 `SWAP` 窥孔 ✓）；tuple 子类三层
（布局／属性／调用 ✓）＋ `namedtuple` 全套 ＋ `_tuplegetter` 全部落地 ✓。**下一手** ✓：回 **task-19
`_tokenize`**（`logging`／`concurrent.futures`／`traceback` 一族 ✓）。

## 本轮（task-19 第 1 轮）：`_tokenize.TokenizerIter` 真词法器 ✓ —— 借编译器词法器（一处真相）＋ 参照投影；同轮修 `splitlines(keepends)` ✗

**recon 结论** ✓：编译器词法器（`compile/lexer.rs` 的 `lex` ✓，947 行 ✓）**可以**当底层 ✓ —— 它给
`(lexeme, span)` 且 **列按字符计** ✓，与 `tokenize` 的 `(row, col)` 同口径 ✓；但它**不产** `COMMENT`／`NL`／
`INDENT` 文本／`ENDMARKER` 位置，且把字符串／数字**解码** ✗ ⇒ 参照的"物理行层"与"源文本"必须由
`_tokenize` 侧补 ✓（纯重写 ✗ 或纯复用 ✗ 都不成立 ✓）。

**落地** ✓：
- 新公开 `pyawa_core::python_tokens`（`compile/lexer.rs` ✓、`lib.rs` 重导出 ✓）：词法单元 ⇒
  `PythonToken { kind, start, end }` ✓（关键字统一成 `Name` ✓、运算符统一成 `Op` ✓）。
- `crates/pyawa-stdlib/src/tokenize_module.rs` ✓：`TokenizerIter` ✓（`__init__` 逐行读 readline ✓ ——
  `str` ✓／`bytes` ＋ `encoding` ✓、`StopIteration` 收尾 ✓；`__iter__`／`__next__` ✓），
  **参照投影**＝类别 ⇒ `token` 数字 ✓、五元组 ✓、物理行层 `COMMENT`／`NL` ✓、`NEWLINE` 一位宽尾 ✓、
  `INDENT` 空白文本 ✓、`DEDENT` **位置＝触发行的缩进宽度 ＋ `line`＝那一行** ✓、多行 token 的
  `line`＝**它吃掉的物理行** ✓、行尾 `COMMENT` 不含 `
` ✓、隐式续行 `NL` ✓、`ENDMARKER` ✓。
- `Lib/tokenize.py` 同步（上游逐字节 ✓）⇒ **`import tokenize` 翻绿** ✓。

**同轮真 bug ✗**：`str.splitlines` **整条 `keepends` 实参没读** ✗、还用 Rust `lines()` 丢掉行尾 ✓
⇒ `'a\nb\n'.splitlines(True)` 给 `['a','b']` ✗（参照 `['a\n','b\n']` ✓）；已按参照口径重写 ✓
（十种行界 ✓、`\r\n` 算一个 ✓、keepends 连界切 ✓、末尾有界不多空项 ✓）。

**验收** ✓：新增 `crates/pyawa-runtime/tests/tokenize_shapes.rs` **4 例**（隐式续行 ✓／运算符・关键字・
行尾注释・`DEDENT` 位置・全角字面量 ✓／注释・空行・缩进・多行字符串・全角 ✓／`splitlines` 四档 ✓）
与参照（本机 `python3` 3.14）**逐行一致** ✓；`quickcheck` ✓、`slowcheck` **十项全绿** ✓（272s ✓）、
0 警告 ✓；判据① **272 ⇒ 273 ÷ 628 ＝ 43.5%（＋1）** ✓（`Lib/` 370 个文件 ⇒ 能 import 257 个 ✓）。

**如实范围 ✗（第 2 轮 ✓）**：**f-string 整段按 `STRING` 出** ✗（参照 `FSTRING_START`／`MIDDLE`／`END` ✓
＋ 内部表达式 token ✓；`!r`／`:` 格式说明／嵌套 `{}`／`{{` 转义 ✓）；**制表符缩进**照底层报未接线 ✓；
另 recon 撞见**独立缺口**：`repr` 不转义控制字符 ✗（`repr('a\x0bb')` 吐原始控制字符 ✓）。

## 本轮（task-19 第 2 轮）：f-string 三段拆分 ✓ ＋ `repr` 不可打印字符转义 ✓ ＋ `len(str)` 按字符 ✓

**f-string** ✓（`tokenize_module.rs::FStringScanner` ✓）：一个 `Lexeme::FStr` ⇒ `FSTRING_START`
（前缀字母 ＋ 开引号 ✓）＋ `FSTRING_MIDDLE` 段 ＋ 字段 token ＋ `FSTRING_END` ✓。口径全按本机
`python3` 3.14 实测 ✓：`{{`／`}}` **只算一个花括号**且**断开 `MIDDLE` 段** ✓（`f"a{{b}}c"` ⇒
`'a{'`／`'b}'`／`'c'` ✓）；字段＝`OP '{'` ＋ **表达式逐 token**（**只把表达式那一段**交给同一个
`python_tokens` ✓ —— 带 `!` 的整段丢进去会词法失败 ✗）＋ `OP '='`／`OP '!'`＋`NAME`／`OP ':'` ＋
**格式说明**（`MIDDLE` 段 ＋ 嵌套字段 ✓、转义 ✓、**收尾必发一个 `MIDDLE`**（空串也算 ✓）
⇒ `f"{x:{w}}"` 的 `MIDDLE ''` (12,12) ✓）＋ `OP '}'` ✓；子词法的**虚拟 token**（`Indent`／`Dedent`／
`Newline`／`End` ✓）不进字段 ✓（否则 `f"{ {1:2} }"` 多两条零宽 `OP ''` ✗）。

**`repr` 不可打印字符** ✓（`instance.rs::quote_str` ✓）：先前只转 `\\ \n \r \t` ＋ 引号 ✗ ⇒
`repr('a\x0bb')` 吐原始 VT ✓。现按参照 ✓：**ASCII 控制字符也转** ✓（`\x00`／`\x7f` ✓）、
`< 0x100` ⇒ `\xNN` ✓（`\x80`／`\x85`／`\xa0` ✓）、`< 0x10000` ⇒ `\uNNNN` ✓（`\u200b`／`\u2028`／
`\u3000`／`\uffff` ✓）、其余 ⇒ `\U…` ✓；**可打印非 ASCII 原样** ✓（`é`／`中`／`😀` ✓）；含 Cf／
Zs（空格除外 ✓）／Zl／Zp／Co（私用区 ✓）／非字符 ✓；`ascii=true` 仍把可打印非 ASCII 也转掉 ✓。

**`len(str)` 按字符** ✓（`instance/containers.rs::length_of` ✓）：先前 `value().len()` ⇒ **UTF-8 字节数** ✗
（`len('中')` 给 **3** ✓、参照 **1** ✓；`len('é')` 给 2 ✓）⇒ 改 `chars().count()` ✓。

**验收** ✓：`crates/pyawa-runtime/tests/tokenize_shapes.rs` **7 例**（第 1 轮 5 例 ＋ **f-string 16 抽例** ✓
＋ **`repr` 21 抽例** ✓ ＋ **`len`** ✓）与参照（本机 `python3` 3.14）**逐行一致** ✓；`quickcheck` ✓、
`slowcheck` **十项全绿** ✓（322s ✓ —— `len`／`repr` 是**广面**改动 ✓，全套护栏照过 ✓）、0 警告 ✓；
判据① **273 ÷ 628 ＝ 43.5%（＋0）** ✓（f-string／`repr`／`len` 都是**语义**面 ✓，没有模块的 import 因此翻绿 ✓，
如实报 ✓）。

**新发现的缺口** ✗：内建 **`ascii()` 未接线** ✓（`ascii('é')` ⇒ `NameError` ✓；core 里 `escape_non_ascii`
已有 ✓，只差 builtins 那个名字 ✓）；**制表符缩进**仍照底层报未接线 ✓。**下一手** ✓：`_colorize` ＋
`io.TextIOWrapper`（traceback 最后两块 ✓）→ logging／concurrent.futures。

## 本轮（task-19 第 3 轮）：`ascii()` ✓ ＋ `_io.TextIOWrapper` 读路径 ✓ ＋ **两处编译器真 bug** ✗；`_colorize` 探明是**一条链**

**落地** ✓：
- **`ascii()`** ✓（`stdlib/builtins_module.rs::ascii_native` ✓）：口径 ＝ `repr(obj)` **再把非 ASCII 转义** ✓
  （`\xNN`／`\uNNNN`／`\U…` ✓）—— 不可打印那半边 `repr` 已转好 ✓（第 2 轮 ✓）。
- **真 bug ✗①：普通串 ＋ 两枚以上 f-string** ✓（`parser.rs` ✓）：`parse_atom` 的 `Str` 臂只吃**一枚**
  后随 f-string ✗ ⇒ `('Field(' f'name={…!r},' f'type={…!r},' ')')` 的**第二枚**留在原地 ✓ ⇒ 括号那一组报
  「括号没有闭合，实际 Some(FStr …)」✗（`Lib/dataclasses.py:314` ✓）。修法 ✓：抽
  `consume_adjacent_string_parts` **一处真相** ✓（`Str` 臂／`FStr` 臂都走它 ✓）＋
  `merge_adjacent_literal_parts` ✓；纯字面量链仍折成**一个常量** ✓。
- **真 bug ✗②：词法器减法溢出 panic** ✓（`lexer.rs` ✓）：单字符前缀（`f'…'` ⇒ `index＝2` ✓）在求 `r`
  前缀时走 `characters.get(index - 3)` ✗ ⇒ **调试构建直接 panic** ✓（最小复现 `f"{'a'}"` ✓，
  `dataclasses` 就是这样撞上的 ✓）⇒ 改 `checked_sub` ✓（语义不变 ✓）。
- **`_io.TextIOWrapper` 读路径** ✓（`io_module.rs` ✓）：占位类型**构造不出**（`new` 槽拒实参 ✓）⇒
  建**真类** ✓（`__init__(buffer, encoding=None, …)` ✓、`read`／`readline`／`readlines`／`__iter__`／
  `__next__`／`__enter__`／`__exit__`／`close` ✓、`encoding` ✓、`closed` 实例镜像 ✓）；解码走
  `decode_with_encoding` **一处真相** ✓；**写路径／`newline=`／`buffering` 如实立支** ✗；
  `type(sys.stdout) is _io.TextIOWrapper` 不再为真 ✓（如实记 ✓）。
- **`tokenize.open` 端到端** ✓：`open(name,'rb')` ⇒ `detect_encoding` ⇒ `seek(0)` ⇒
  `TextIOWrapper(buffer, encoding, line_buffering=True)` ⇒ 迭代／`with` ✓，与参照逐行一致 ✓。

**验收** ✓：新增 `crates/pyawa-runtime/tests/fstring_io_shapes.rs` **3 例**（`ascii` ✓／f-string 相邻链 ＋
panic 回归 ✓／`TextIOWrapper` ＋ `tokenize.open` ✓）与参照（本机 `python3` 3.14）**逐行一致** ✓；
`quickcheck` ✓、`slowcheck` **十项全绿** ✓（330s ✓）、0 警告 ✓；判据① **273 ÷ 628 ＝ 43.5%（＋0）** ✓（如实 ✓）。

**新墙（探明 ✓，独立成支 ✓）**：`_colorize` **不是**单文件同步 ✗ —— `_colorize` ⇒ `dataclasses` ⇒
**`inspect`（未同步）** ⇒ `annotationlib` ⇒ **`type.__dict__["__annotations__"]` 取不到**（`KeyError` ✓：
我们的 `type` 类型字典里没有那个 getset 描述符 ✓）⇒ 实测 `import traceback`／`logging`／
`concurrent.futures` 三族**都停在 `No module named 'inspect'`** ✓。要收它得按序做
**`type.__annotations__` getset ＋ `annotationlib`＋`inspect`＋`dataclasses`＋`_colorize`** ✓
（四件 ✓）；本轮已把三份**不能 import** 的副本撤出 `Lib/` ✓（带了会把进度指标 69.5% 压到 68.9% ✓）。

## 本轮（task-20 第 1 轮）：`type.__annotations__` 描述符 ✓ ＋ `attrgetter` ✓ ＋ `annotationlib` 入手 ✓；`inspect` 现卡两处新真 bug

**链的原文** ✓：`_colorize` ⇒ `dataclasses` ⇒ `inspect` ⇒ `annotationlib` ⇒
`type.__dict__["__annotations__"]`（`annotationlib:1141` ✓）—— 第 3 轮探明、本轮收掉**最里那一格** ✓。

**落地** ✓：
- **`type.__annotations__`** ✓（`core/instance.rs::install_annotations_descriptor` ✓）：一个 **getset 描述符**
  对象（`getset_descriptor` 类型 ＋ `__get__` native ✓ —— 与 stdlib 的 `_tuplegetter` **同一手法** ✓）装进
  **`type` 类型的命名空间** ✓ ⇒ `type.__dict__["__annotations__"].__get__` 可用 ✓。**3.14 口径** ✓
  （`Instance::annotations_of` **一处真相** ✓）：**自有就给** ✓、**没有就建空 `dict` 并缓存** ✓ ——
  类对象写命名空间 ✓、模块对象写实例字典 ✓（两条路实测 ✓）；`attribute.rs` 侧同一处 ✓。
- **`operator.attrgetter`** ✓（`stdlib/operator_module.rs` ✓）：与 `itemgetter` 同一套绑定方法机制 ✓；
  单属性 ⇒ 单值 ✓、多属性 ⇒ 元组 ✓、**点分路径** ✓ 逐层 `getattr` ✓（`inspect` 的
  `from operator import attrgetter` 不再是 `ImportError` ✓）。
- **`Lib/annotationlib.py`** 同步 ✓ ⇒ **import 通过** ✓。

**验收** ✓：新增 `crates/pyawa-runtime/tests/annotations_shapes.rs` **2 例**（描述符两条路 ＋ `__get__` ＋
`get_annotations`（类／模块 ✓）＋ `attrgetter` 单值／多值／property ✓；`import annotationlib` ＋
`operator.attrgetter` ✓）与参照（本机 `python3` 3.14）**逐行一致** ✓；`quickcheck` ✓、
`slowcheck` **十项全绿** ✓（276s ✓）、0 警告 ✓；判据① **273 ⇒ 274 ÷ 628 ＝ 43.6%（＋1）** ✓。

**如实范围 ✗／下一轮** ✓：**类体注解声明**（`x: int`）我方编译器还不落 `__annotations__`／`__annotate__` ✓
（`C.__annotations__` 参照 `{'x': int}`／我们 `{}` ✓）；`import inspect` 撞**两处新真 bug** ✓：
① **`%` 映射取键** ⇒ `KeyError: __mro__` ✓（`executor/runtime.rs` 的 `%` 映射支 ✓；触发点是某处
`'%(…)' % <映射>` ✓）、② **一处 `str + tuple`** ✓（`PYAWA_BINOP_DEBUG=1` 站点实测 ✓、左值＝
`__author__[0]` ✓）。另顺手：`sys.modules["__main__"]` 缺 ✓（`KeyError: __main__` ✓）。

## 本轮（task-20 第 2 轮）：常量表下标 ≥ 256 截断 ✗（`inspect` 的真凶）＋ `type.__dict__` 的 `__mro__`／`__dict__` ＋ `__mro_entries__` ＋ `sys.modules["__main__"]` ⇒ `inspect`／`dataclasses` 通

**真 bug ✗①（本轮大头）**：**`LOAD_CONST` 常量表下标 ≥ 256 时被截断** ✓ —— 延迟入池那条路
（折叠元组／`frozenset`／关键字元组 ✓）在 `flush_pending` 里只写 `index as u8` ✗ ⇒ **高字节丢** ✓。
实测 `Lib/inspect.py:1616` 的 `('frame',) + Traceback._fields` 取到的是**别的常量** ✓
（常量表 260 项、`('frame',)` 真下标 301 ⇒ 低字节 45 ⇒ 读到 `(45,)` ✓）。**定位法** ✓：`PYAWA_BINOP_DEBUG`
只有 `qualname@**ip**`（**字节码下标**，不是行号 ✗）⇒ 临时把 `positions()` 一并打出来 ⇒ 真凶现形 ✓
（后来又发现 position 表对个别语句也会偏 ✓，最终靠"删除语句后仍失败"判定 ✓）。
**修法** ✓：新增 `emit_constant` —— 已入池直发 ✓；没入池时**上界 > 191 就当场入池**并走带 `EXTENDED_ARG`
的那条 ✓；小模块仍按参照顺序延迟 ⇒ **编译器逐字节夹具不动** ✓。另把两处 `as u8` 截断
（异步 `for` 的 `None` ✓、注解单元的 `str` ✓）改成带 `EXTENDED_ARG` ✓。

**真 bug ✗②**：**`type.__dict__["__mro__"]`／`["__dict__"]` 取不到** ✓（`inspect.py:1673/1674` 的
`_static_getmro`／`_get_dunder_dict_of_class` ✓）⇒ 与 `__annotations__` **同一套 getset 描述符** ✓
（一个类型 ＋ 按描述符实例的 `_kind` **分派**：`annotations`／`mro`／`dict` ✓）。
**更正** ✓：第 1 轮猜的"`%` 映射取键"**不成立** ✗ —— `KeyError: __mro__` 就是这一格 ✓。

**真 bug ✗③（PEP 560）**：**`__mro_entries__`** ✓ —— `class T(Mapping[str, str])` 的下标结果是
`GenericAlias` ✗ ⇒ 参照先调 `__mro_entries__` 换出**真正的基类** ✓（先前 `bases must be types` ✓，
`Lib/_colorize.py:115` ✓）⇒ `build_class_from_parts` 先解析非类型基类 ✓、
`GenericAlias`／`UnionType` 交回 `(__origin__,)` ✓（类型**惰性**建 ⇒ 注册也随第一次造别名 ✓）。

**真 bug ✗④**：**`sys.modules["__main__"]` 缺** ✓ ⇒ 脚本模块对象（命名空间**就是**脚本 `globals` ✓）
在 `pa_exec_string`／`pa_exec_bytecode` **两条路**开跑前登记 ✓（CLI 走后者 ✓）。

**验收** ✓：新增 `crates/pyawa-runtime/tests/type_dict_shapes.rs` **3 例**（常量表 > 255 ✓／
`type.__dict__` 三个描述符 ＋ `__mro_entries__` ＋ `import inspect`・`dataclasses` ✓／
`sys.modules["__main__"]` ✓）与参照（本机 `python3` 3.14）**逐行一致** ✓；`quickcheck` ✓、
`slowcheck` **十项全绿** ✓（342s ✓）、0 警告 ✓；判据① **274 ⇒ 276 ÷ 628 ＝ 43.9%（＋2）** ✓
（`Lib/inspect.py` ✓ ＋ `Lib/dataclasses.py` ✓）。

**③ 类体注解声明：只探明、未落** ✓（按 lead 授权 ✓）：3.14 是**惰性注解**（类体／模块级要发
`__annotate__` 函数 ✓）—— 属**编译器**一格 ⇒ **立支** ✓。
**新墙** ✓：`import _colorize` 卡 **`帧操作失败：StackUnderflow`** ✓（编译器／运行期**栈不平衡** ✓）；
`traceback`／`logging`／`concurrent.futures` 随之仍红 ✓ —— 第 3 轮从这格开 ✓。

## 本轮（task-20 第 3 轮）：函数收尾／推导式清理块顺序 ✗（`StackUnderflow` 真凶）＋ `nonlocal x += 1` ✗ ＋ `exec` ✓ ＋ `sys` 递归上限两口 ✓

**真 bug ✗①（本轮大头）**：**函数作用域的推导式清理块排在隐式返回之前** ✗ ⇒ 正常路径**直接落进处理块** ✓
（`SWAP 2` 撞空栈 ⇒ `帧操作失败：StackUnderflow` ✓ —— `Lib/dataclasses.py::_init_fn` 的
`func_builder.add_fn(…)` 之后正是这个形状 ✓）⇒ 改成「**先发隐式 `LOAD_CONST None; RETURN_VALUE`、
再冲刷处理块**」✓（与参照 `dis` 同序 ✓，**编译器逐字节夹具不动** ✓）。
**定位法** ✓：`PYAWA_STACK_DEBUG` 给 Rust 回溯 ⇒ 认到 `swap_from_top` ✓；再在 `SWAP` 臂临时打
**站点＋位置表** ⇒ `_init_fn@432`（注意：站点里的 `@N` 是**字节码下标**不是行号 ✗ —— 先前误读成行号
绕了远路 ✓）；最后**与参照 `dis` 对拍**（CPython 在 `CALL_KW/POP_TOP` 之后就是
`LOAD_CONST None; RETURN_VALUE`，处理块**外提在返回之后** ✓）钉死 ✓。

**真 bug ✗②**：`nonlocal x; x += 1` 先前走 `LOAD_NAME`／`STORE_NAME` ✗（增强赋值的名字目标只认
**局部 varname** ✓）⇒ 函数帧没有命名空间 ⇒ `LOAD_NAME 需要命名空间帧` ✓（`Lib/functools.py` 的
`_lru_cache_wrapper` 一族卡在这 ✓）⇒ **cell／自由变量优先**（`LOAD_DEREF`／`STORE_DEREF` ✓）。

**`exec(源码, globals=None, locals=None)`** ✓（`Instance::exec_source` ✓）：与 `eval_source`
**同一处真相**（`compile` ⇒ `instantiate` ⇒ 命名空间帧 ⇒ `execute` ✓）—— `dataclasses._create_fn`
就是「拼源码 ⇒ `exec` ⇒ 取名」✓。**如实范围** ✗：只收 `str`／`locals` 只收 `dict`／缺命名空间时新建 ✓。

**`sys.getrecursionlimit`／`setrecursionlimit`** ✓（`Instance::recursion_limit` ✓）：默认
`MAX_CALL_DEPTH`（64 ✓）、**只降不升** ✓（本层 Python 调用吃原生栈 ✗ ⇒ 如实夹住 ✓）。

**验收** ✓：新增 `crates/pyawa-runtime/tests/exec_closure_shapes.rs` **3 例**（清理块顺序 ✓／
`nonlocal` 增强赋值 ✓／`exec` ＋ 递归上限 ＋ 无字段 `@dataclass` ✓）与参照（本机 `python3` 3.14）
**逐行一致** ✓；`quickcheck` ✓、`slowcheck` **十项全绿** ✓（298s ✓）、0 警告 ✓；
判据① **276 ÷ 628 ＝ 43.9%（＋0）** ✓（语义面 ✓，如实报 ✓）。

**仍差最后一格** ✓：**类体注解声明**（3.14 惰性注解 ⇒ `__annotate__`）—— `_colorize` 现报
`TypeError: 'argparse' is a field but has no type annotation` ✓（`@dataclass` 读不到字段注解 ✓）
⇒ 那一格落地后 `_colorize` ⇒ `traceback`／`logging`／`concurrent.futures` 应能一串收 ✓
（`Lib/_colorize.py` 那三份副本仍**撤出** ✓ —— 不能 import 的不混进已同步子集 ✓）。

## 本轮（task-21 第 1 轮）：类体注解 ⇒ `__annotate__`（PEP 649）

**解析**：`x: int`／`x: int = 1` 的注解半边**留进 AST** ✓（`Statement::Annotation` ✓ —— 先前解析完就丢 ✗）；
赋值那半边照旧发 ✓（类体的指令序列因此不动 ✓）。

**发射**（照 `dis` 实测 ✓）：类体收尾先发 `LOAD_CONST <__annotate__ 单元>; MAKE_FUNCTION;
STORE_NAME __annotate_func__` ✓，**再**发 `__static_attributes__`／`__classdictcell__` ✓；注解单元 ＝
1 个 `format` 形参 ＋ 守卫（`format > 2` ⇒ `NotImplementedError` ✓）＋ `BUILD_MAP 0` ＋ 每条
「值 ⇒ `COPY 2` ⇒ 键 ⇒ `STORE_SUBSCR`」✓。**嵌套块里的注解也收** ✓（`if True: w: float` ✓ 参照同 ✓）。

**运行期**：`C.__annotate__` 由 `__annotate_func__` 给出 ✓（`C.__dict__` 里的名字不改 ✓）；
`C.__annotations__` 走 `__annotate__(1)` ✓、**缓存进 `__annotations_cache__`** ✓（3.14 实测
`C.__annotations__ is C.__annotations__` 真 ✓ 而 `C.__dict__['__annotations__']` 仍 `KeyError` ✓）；
显式 `C.__annotations__ = {...}` 优先 ✓；**模块**缓存进 `__annotations__` ✓。
⇒ task-20 第 1 轮那笔"类对象建空 dict 并缓存"**口径错** ✗，本笔改成 3.14 真口径 ✓
（`annotations_shapes.rs` 的抽例在新口径下仍与参照逐行一致 ✓）。

**验收** ✓：新增 `crates/pyawa-runtime/tests/class_annotations_shapes.rs` **1 例**与参照（本机 `python3` 3.14）
**逐行一致** ✓；`annotations_shapes.rs` **2 例** ✓；`quickcheck` ✓、`slowcheck` **十项全绿** ✓（292s ✓）、
0 警告 ✓；判据① **276 ÷ 628 ＝ 43.9%（＋0）** ✓（链还差下一格 ✓，如实报 ✓）。

**仍卡下一格** ✗：`@dataclass` **带字段** ⇒ `dataclasses._process_class` 报
`AttributeError: 'bool' object has no attribute '_field_type'` ✓（`ip=953` 的列表推导 ✓）；
最小复现（`[f for f in fields.values() if f._field_type in (1, 2)]`）**跑通** ✗ ⇒ 触发形状还没夹出来 ✓。
**线索** ✗：导入模块里那些 code object 报 `firstlineno=1`、`positions()` 为空 ✓（`_init_fn`／
`_process_class` 都如此 ✓）—— 诊断面先记下 ✓，下一轮先查它是否同一根因 ✓。
**余下两格** ✗：**模块级**注解还没发 `__annotate__` ✓；**类字典优先**的求值口径
（`LOAD_DEREF __classdict__; LOAD_FROM_DICT_OR_GLOBALS` ✓）未接 ✓。

## 本轮（task-21 第 2 轮）：推导式融合指令槽 ≥ 16 截断 ✗（`@dataclass` 那格的真凶）⇒ 收 `_colorize`／`traceback`

**真 bug ✗**：推导式把「存目标」与「紧随其后的那次读」打成超指令 `STORE_FAST_LOAD_FAST` ✓，
oparg ＝ `(存槽 << 4) | 取槽` ✓ —— **只有 4 位** ✗，而代码里直接 `as u8` ✓ ⇒ **槽 ≥ 16 时截断** ✗。
实测：`dataclasses._process_class` 这种**十九个局部**的函数里，推导式目标落在**槽 19** ⇒ oparg
`(19<<4)|19 = 323 as u8 = 51` ＝ 槽 (3,3) ✗ ⇒ **存错槽、取到 `repr`（一个 bool ✓）** ⇒
`'bool' object has no attribute '_field_type'` ✓（整条 `@dataclass` 链卡这 ✓）。
**修法** ✓：两个槽都 < 16 才融合 ✓（参照在槽 ≥ 16 时不融合、改发 `STORE_FAST` ＋ `LOAD_FAST_BORROW` ✓）；
生成器体那条 `((slot << 4) | slot)` 同样加上界 ✓。

**定位法** ✓（可复用）：
① 给 `attr_miss` 加**局部槽转储** ⇒ 一眼看到 `fields` 是对的、**元素却是 bool** ✓ ⇒ 排除"数据错了"、
锁定"取错槽" ✓；
② 与参照 `dis` 对拍**同形状函数**（把局部数抬到 ≥ 16 ✓）⇒ 参照两条、我们融合成 51 ✗ ⇒ 真凶现形 ✓。

**验收** ✓：新增 `crates/pyawa-runtime/tests/high_slot_comprehension_shapes.rs` **2 例**（槽 ≥ 16 推导式 ✓／
`@dataclass(frozen=True, kw_only=True)` 带注解字段造实例 ✓）与参照（本机 `python3` 3.14）**逐行一致** ✓；
编译器**逐字节夹具不动** ✓；`quickcheck` ✓、`slowcheck` **十项全绿** ✓（355s ✓）、0 警告 ✓；
判据① **276 ⇒ 278 ÷ 628 ＝ 44.3%（＋2）** ✓（`_colorize` ✓ ＋ `traceback` ✓）。

**现卡新一格** ✗：`import logging`／`concurrent.futures` ⇒ `re._compiler._optimize_charset` 的
`for op, av in charset:` 里 **`charset` 是个 `ModuleNotFoundError`** ✓（站点 `_optimize_charset@20` ✓）
⇒ 调用方 `for op, av in code:` 传错了 `av` ✓ —— 又是"取错槽/取错值"同族 ✓，第 3 轮从这格开 ✓。

**位置表线索**：先前记的"导入模块 code object `firstlineno=1`／`positions()` 空" ✗ **证据自相矛盾** ✓
（别的站点又能读出真位点 ✓；`pyac` 确实带 `positions` ✓、import 路也是"编译 ⇒ 实例化" ✓）
⇒ 记为**未证实** ✓，别当同根因 ✓（下一个专门探针再判 ✓）。
**另记** ✗：`traceback.print_exc()` 还要 `sys.exception` ✓。

## 本轮（task-21 第 3 轮）：收窄写码点横扫审计（同族第 5 例 ＋ 56＋8 处改走 `emit_indexed`）

**同族第 5 例** ✗：**字典推导式**的 `LOAD_FAST_BORROW_LOAD_FAST_BORROW`（键槽／值槽各半字节 ✓）
先前**没有**上界守卫 ⇒ 槽 ≥ 16 截断 ✗（形状：十六个局部的函数里 `{k: k + 1 for k in items}` ✓）
⇒ 补 `> 0x0F 不融合` ✓（与 `pack`／`fused_pair`／`fused_tail`／生成器那条同一条规矩 ✓）。

**横扫** ✓：`emitter.rs` 里"实参就是**槽号／名字下标／常量下标／计数**"的 `as u8` 写码点
**56＋8 处**统一改走 `emit_indexed` ✓（> 255 自动补 `EXTENDED_ARG` ✓；≤ 255 **逐字节不变** ✓ ——
编译器逐字节夹具全绿 ✓）；`PendingCleanup.depth` 由 `u8` 放宽成 `usize` ✓。

**审计口径（一处真相 ✓）**：`emit_indexed`／`emit_shifted`／`emit_constant` 是**唯三**该做下标写码的
入口 ✓；**4 位半字节融合**（`<< 4 |`）共 8 处，逐处标注：

| 写码点 | 上界守卫 | 结论 |
|---|---|---|
| `EXTENDED_ARG` 名字下标 | 无 → 已修 | 第 725 轮 ✓ |
| `LOAD_CONST` 延迟入池 | 无 → 已修 | task-20 第 2 轮 ✓ |
| `SWAP` 窥孔长度 | 无 → 已修 | task-14 ✓ |
| `STORE_FAST_LOAD_FAST`（推导式）| 无 → 已修 | task-21 第 2 轮 ✓ |
| `LOAD_FAST_BORROW_LOAD_FAST_BORROW`（字典推导式）| 无 → **本轮补** ✓ | 第 5 例 ✓ |
| `STORE_FAST_STORE_FAST`（元组解包／for 目标／推导式目标）| 有（`<= 0x0F` ✓）| 第 289／308 轮 ✓ |
| `LOAD_FAST_BORROW_LOAD_FAST_BORROW`（`pack`／`fused_pair`）| 有 ✓ | 第 308／662 轮 ✓ |
| 生成器体 `STORE_FAST_LOAD_FAST`（`slot<<4|slot`）| 无 → 已修 | task-21 第 2 轮 ✓ |

其余 `as u8` 只剩**天然有界**的：opcode 字节 ✓、`0xFF` 掩码／`>> 8` 拆字节 ✓、`nb_ops` 表内小下标 ✓。

**验收** ✓：新增 `crates/pyawa-runtime/tests/narrowing_guard_shapes.rs` **2 例**（十六个局部的字典推导式
对参照逐行一致 ✓；**名字表 > 255** 的 300 个名字 `STORE_NAME`／`LOAD_NAME` 自证结果 ✓）；
`high_slot_comprehension_shapes.rs` ✓、编译器**逐字节夹具全绿** ✓；`quickcheck` ✓、
`slowcheck` **十项全绿** ✓（302s ✓）、0 警告 ✓；判据① **278 ÷ 628 ＝ 44.3%（＋0）** ✓。

**仍卡（第 4 轮／另支）** ✗：`import logging`／`concurrent.futures` ⇒ `re._compiler._compile` 的
`for op, av in code:` 把**一个 `ModuleNotFoundError`** 当 `av` 传进 `_optimize_charset` ✓
（站点 `_optimize_charset@20` ✓、模块 `re._compiler` ✓）。**已排除**：`_optimize_charset` 本体
**照抄成独立模块后跑通** ✓（对参照一致 ✓）⇒ 是**调用方**那一格 ✓；最小复现
`re.compile(r'[a-z]', re.I)` ✓（`[abc]` ＋ `re.I` 反而没事 ⇒ 与 `RANGE` 那条支线有关 ✓）。

