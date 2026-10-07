# NEXT.md —— 续做状态（单页；长复盘进台账 `docs/rounds/`）

> 规则见 `agents-rules/round-rule.md`：**一轮一落地**（每轮以「提交」或「撤回＋一条改变计划的事实」结束 ✓）。

## 现在

- **HEAD**：`2b0e393`（`dev`）＋本轮未提交的 `Pattern` 属性面（见下 ✓）
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
`_sre.template`（`re.sub` 的内部用法 ✓）、模板未知转义报错 ✗、表项回收 ✗、迭代器 `__next__` 属性面 ✗（core 侧 ✓）。
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

## 下一条命令（`re` 剩余面 ＋ core 侧 `__next__` ✓）

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
