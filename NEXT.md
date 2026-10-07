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
