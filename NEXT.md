# NEXT.md —— 续做状态（单页；长复盘进台账 `docs/rounds/`）

> 规则见 `agents-rules/round-rule.md`：**一轮一落地**（每轮以「提交」或「撤回＋一条改变计划的事实」结束 ✓）。

## 现在

- **HEAD**：`7924f4a`（`dev`）＋**未提交**：假货 `Lib/enum.py` ✓、`int(x, base)` ✓、两个新护栏 ✓（`Lib/re/` 已同步进工作区但**不进本笔** ✗）
- **stash**：**已清空** ✓（`stash@{0}` 已在第 579 轮 `pop` ✓）
- **判据①**：**189／628 ＝ 30.1%** ✗（阈值 67%；起点 187／628 ≈ 29.8% ✓；单次读数 ±1 ⇒ 按**区间**读 ✓）
  进度指标（不作判据 ✓）：`Lib/` 294 个文件 ⇒ 能 import **173** 个（58.8% ✓）
- **①（终结器/`super` 那条 UAF）**：**可观测失败已消失** ✓（重压 6×6：修前 27/36 红 ⇒ 修后 0/36 绿 ✓）；
  底层"字典是否被重复释放"**仍未证明** ✗（见台账第 578 轮"如实" ✓）。

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

## 下一条命令（继续顶 `import re` ✓）

**上游真实用法**（`/usr/lib/python3.14/re/` 逐行读出 ✓，非印象 ✗）：`_compiler.py:778` 调
`_sre.compile(pattern, flags|state.flags, code, groups-1, groupindex, tuple(indexgroup))` ✓；
`re/__init__.py:128` `import _sre` ✓、`:377` 用 `_sre.template` ✓；`_constants.py:18` 从 `_sre` 取
`MAXREPEAT`／`MAXGROUPS` ✓；`_compiler.py:397/408/673` 用 `CODESIZE` ✓、`:52-57/446-448` 用四个 cased/tolower ✓。
`re` 侧还要 `Pattern` 有：`match/search/fullmatch/split/findall/finditer/sub/subn` ＋ `scanner` ✓
（`re/__init__.py` 里逐个用到 ✓）；`re/__init__.py:315` `Pattern = type(_compiler.compile('', 0))` ✓。

```bash
# 1) 在 `_sre_module.rs` 里（**Rust 侧** ✓，照 `SPEC-c-modules.md:470`「`_sre` 必须用 Rust 重写」✓）：
#    `compile(...)` 忽略 code ✓、内部走 compile_raw ✓，返回**自建类**的实例：
#      Pattern（`new_attribute_type` ✓ + 类字典挂原生方法 ✓）＋ Match（同法 ✓）
#      实例字典存：`_id`／`pattern`／`flags`／`groups`／`groupindex` ✓
#    先落 `match/search/fullmatch` ＋ Match 的 `group/groups/span/start/end` ✓，再补 `findall/finditer/split`
#    与 `_sre.template`（`sub` 用 ✓）—— 增量落、每步都验 ✓
# 2) 验证：与参照**逐例**一致（`re.compile(p).match(s).span()/group()` ✓，含非 ASCII ✓）
cargo test -p pyawa-runtime --test finalize_shapes --test meta_path_shapes    # 两族护栏先绿 ✓
tools/slowcheck.sh                                                          # 十项闸门（只看 exit code ✓）
python3 tools/lib_import_ratio.py                                           # 报前后分子 ✓
```
**仍等你拍** ✓：`re/__init__.py` 开头 `import enum` ✗ ⇒ `_sre` 通了 `re` 也进不来 ✓
（重启 `enum` 支 ✓ 或给 `Lib/` 放最小自有 `enum.py` ✓，代价见台账第 564 轮 ✓）。

## 纪律提醒

- 红灯不提交 ✓；代码＋台账＋`NEXT.md` **同笔** ✓；闸门**只看 exit code**（不接管道 ✓）；
- 判据① **5 轮不动**即报停滞 ✓；**禁止空转**：一轮内不能只写台账 ✓；
- 判据① 单次读数 ±1 ⇒ 报"区间" ✓（第 512 轮实测 ✓）。
