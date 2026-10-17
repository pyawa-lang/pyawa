# NEXT.md —— 续做状态（单页；长复盘进台账 `docs/rounds/`）

> 规则见 `agents-rules/round-rule.md`：**一轮一落地**（每轮以「提交」或「撤回＋一条改变计划的事实」结束 ✓）。
> **本页是会话交接单页**（2026-10-17 ✓）：上一会话（Lead＋队员团队模式）收完路线 2 的
> task-26/27/28/29 四笔；逐轮复盘见 `docs/rounds/03-m5-progress-05.md` ✓。

## 现在

- **HEAD**：`b93ea6d`（`dev`，**已推送** origin ✓）—— task-26/27/28/29 四笔 ＋ docs 收口
- **工作区**：**干净** ✓（`git status --porcelain` 为空 ✓）
- **stash**：**空** ✓
- **判据①**（`python3 tools/lib_import_ratio.py`，**先重建 conformance 二进制**再量 ✓）：
  **通过 289 ＋ 参照口径 17 ＝ 306 ÷ 628 ⇒ 48.7%**（区间 305–307；阈值 67% ＝ 421，还差 115）
  - 战役累计：208 → 296 → **306**；进度指标：`Lib/` 377 文件 ⇒ 290 能 import ＝ 76.9%
  - 判据② **推进中**：用户拍板**补语料** ⇒ 第一批 30 条（**182 → 212**；conformance 212 通过 ·
    0 已知差异 · 0 新差异 ✓）⇒ **尚未能裁定**（P3-27 未完，如实 ✓）
- **上限诊断**（排期仪器，不作判据 ✓，overlay 口径）：**312/628**——大族排队见台账
  「上限诊断」表 ✓（`\|=` 42 ／ `_socket` 41 ／ multiprocessing 跳转标签 26 ／ difflib 嵌套元组 17 ／
  CJK 24 ／ …）

## 下一手（wave-2，按「阻塞最多模块的报错签名」队列）

1. **`set`／`dict`／`bytearray` 的 `|=` 一族就地运算**（42 下游）：最小复现
   `s = {1}; s |= {2}` ⇒ 「可变容器的就地运算尚未接线」✗；修 executor（语义照参照：
   `dict |=` 走 update、`set` 六个位运算、`bytearray |=`）。
2. **difflib 嵌套元组目标**（17 下游＋`unittest`/`test.support` 链）：`(a,b),(c,d) = x,y` 解析缺口
   （`Lib/difflib.py:1835` ✗）⇒ 修解析器 ⇒ 同步 `Lib/difflib.py`＋`Lib/unittest/**`＋
   `Lib/test/support/__init__.py`（**先重建 conformance 二进制**再量 ✓）。
3. **multiprocessing 跳转标签**（26 下游）：「跳转目标标签 N 从未落点」⇒ 先钉最小复现。
4. **CJK `_codecs_*` 专支**（24 下游，既有挂账）。
5. **socket 支**（`P3-29`，用户已拍板立项，排 typing 收口后；http.client／urllib.request／
   socketserver／asyncio 的共同前置）。
   - 挂账小件：`random.Random(42)` 序列与参照不同（真差异，`P3-30`）；`enum` 成员／`copy.copy`／
     `types.SimpleNamespace`／`operator.attrgetter`／`@dataclass`／`fnmatch`／`urllib.parse.quote`／
     `str.capitalize`／`typing` 泛型别名 `str()` 渲染／`itertools.groupby`（task-29 实测清单，见台账）；
     行内 `else:`／`with`／`match` 的 suite（带换行同样报错）；`sys._getframe(depth>0)`；
     `concurrent.futures.process` 要 multiprocessing；`traceback.print_exc` 要 `sys.exception`。

## 团队模式（本会话的工作方式，建议沿用）

- **Lead 不写代码**：拆任务上板（`team_task_create`，写死写域）→ 队员执行（认领→落地→汇报）→
  Lead 审查 diff、跑终审闸门（slowcheck 十项）、统一提交（`git commit -F - -- <显式路径>`）与推送。
- **队员用 `ds-` 前缀命名**（⇒ deepseek 路由，本会话实测 ✓）；**Rust 写手同时至多一名**
  （共享工作区＋共享 `target/`，多名并发会互相编译对方的半成品 ✗）；非 Rust 面（语料／工具／docs）可并行。
- **队员上下文会耗尽**：一个队员跑 20＋轮后会话到极限 ⇒ 配方写进本页／台账后换新队员接棒。
  **注意成员上限 8**——旧成员占满名额会挡新 spawn，且 Lead 没有移除成员的工具。
- **同墙 3 轮上限**：修不通就停手立据升级，把选项交用户；**红灯不提交**；解析器／编译器改动
  **不与语义分离落地**（只让语法过却丢语义＝说谎——本会话 task-27 就是一例：只修 panic 不修
  「合法用例编译成功」＝半成品，当场补完 ✓）。

## 本战役的关键口径（新会话必读）

- **分子测量**：`lib_import_ratio.py` 跑的是 **conformance 测试二进制** ⇒ 改 core/stdlib 后必须先
  `tools/cargo.sh test -p pyawa-abi --test conformance --no-run` **再量**（踩过两次 ✓）。
- **`__pyawa__` 缓存坑**（task-26 踩的 ✓）：按**脚本目录**复用旧编译产物 ⇒ 目录内改动跑的还是旧
  字节码 ⇒ 量之前 `rm -rf Lib/__pyawa__`（＋ `target/**/__pyawa__`）✓。
- **上限诊断口径修正**（task-29 后实测 ✗）：`--ceiling` 把上游 `lib-full` 插 `sys.path[0]` ⇒
  上游**原版**遮住本仓**适配版**（`Lib/enum.py` ⇒ 全红假象 ✗）；正确做法＝overlay（上游全量 ⊕
  本仓 `Lib/` 覆盖）⇒ 312/628 ✓。
- **四条探针规矩**（插桩定位用，全文见台账卷）：① 按文本锚定不用行号；② 标记只用 ASCII；
  ③ 躲开装饰器（插 `def` 行后）；④ 插进体里要 +4 缩进。
- **收账两道必检**：逐个导入＋无关脚本 `startup ok`；只同步**能 import** 的文件进 `Lib/`。
- **conformance 会补末尾换行**（task-27 实测 ✓）⇒ corpus 测不到「源末无换行」的路径；这类回归
  必须 Rust 侧直测（范本 `crates/pyawa-core/tests/eof_inline_suite.rs` ✓）。

## 纪律提醒

- 红灯不提交 ✓；代码＋台账＋`NEXT.md` 同轮提交 ✓；闸门**只看 exit code** ✓；
- 判据① **5 轮不动**即报停滞 ✓；判据① 单次读数 ±1 ⇒ 报"区间" ✓；
  `slowcheck` 十项全绿（含 stability 连跑口径 MS-25）才许提交 ✓；
- 新增 `P*-` 条目号前先重算当前最大号（墓碑算定义，`CX-19`；现最大 **P3-30** ✓）。
