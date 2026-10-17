# NEXT.md —— 续做状态（单页；长复盘进台账 `docs/rounds/`）

> 规则见 `agents-rules/round-rule.md`：**一轮一落地**（每轮以「提交」或「撤回＋一条改变计划的事实」结束 ✓）。
> **本页是会话交接单页**（2026-10-17 ✓）：上一个会话以「Lead＋队员」团队模式跑完路线 2 枢纽战役前半程；
> 战役逐轮复盘见 `docs/rounds/03-m3-progress-04.md`（＆ `-03` 尾部）✓。

## 现在

- **HEAD**：`6b580e8`（`dev`，**已推送** origin ✓）—— task-25 第 1 轮（`vars` 内建＋生成器表达式闭包＋`isinstance` 元类钩子 ⇒ **`importlib.metadata` 一族 8 个通**）
- **工作区**：**干净** ✓（有一剂**已保全的半成品**：`target/wip_param_cell.diff`（94 行）＋ `target/tmp_unpack_debug.rs`——见「下一手」①，重放即用）
- **stash**：**空** ✓
- **判据①**（`python3 tools/lib_import_ratio.py`，上会话末实测 ✓）：**通过 279 ＋ 参照口径 17 ＝ 296 ÷ 628 ⇒ 47.1%** ✗（阈值 67% ＝ 421，还差 125）
  - 战役累计：**208 → 296**（+88 ✓，自用户拍板路线 2 起）✓；进度指标：`Lib/` 377 文件 ⇒ 280 能 import ＝ 74.3%
  - 判据② **已达成** ✓（`target/recon/reparity.py` 13 行与参照逐字一致）
- **用户已拍板**：**路线 2**——按「阻塞最多模块的报错签名」队列抬判据① 分子（全分母 628 口径），目标 **≥ 421／628＝67%**；`enum.py` 换回支降为**低优先**；**look-around 正则已定案引入 fancy-regex**（已落地，两档引擎）

## 下一手（按序）

**① 先重放那剂半成品（最接近收口的一格）**：`target/wip_param_cell.diff` 修的是 **「形参 cell 不能走 `STORE_FAST`／`LOAD_FAST` 快路」**——`Lib/functools.py::singledispatch.register` 的 `argname, cls = next(...)`（`cls` 被内部 lambda 捕获 ⇒ 形参 cell ✓）先前把 cell 对象打掉 ⇒ `typing.get_type_hints` 拿到 `<cell object>` ⇒ **`importlib.resources` 一族就卡这** ✗。重放（`git apply target/wip_param_cell.diff`）后：
   - 用 `target/tmp_unpack_debug.rs` 的 dis 转储对照参照（参照对 cell 目标发 `STORE_DEREF`／`LOAD_DEREF` ✓）；
   - 补一条逐字节夹具／护栏抽例；过闸门（quickcheck 轮内／slowcheck 提交前，`&&` 只看 exit code）⇒ 一笔提交 ⇒ 收 `importlib.resources` 一族（8＋）。
**② `pathlib.types`** ⇒ `TypeError: 'getset_descriptor' object is not iterable`（多半是把描述符当值序列用／`__annotations__` getset 在类上被直接迭代；按四条探针规矩夹具体语句）。
**③ `typing` 尾巴**：`typing.Optional[int]` 的 repr 面（`int | None`）＋ `typing.ForwardRef` 懒建面。
**④ 队列（之后按序）**：`pickle` 支（`logging.config`／`handlers` 在等）→ `socket` 立支评估（http.client／urllib.request／socketserver／asyncio 的共同前置，工作量大、可能要用户拍板）→ `asyncio` 探墙 → **CJK `_codecs_*` 专支**（全分母口径 42 下游）→ `multiprocessing` → `test.support` 殿后（45 下游，依赖汇）。
   - 挂账小件：`concurrent.futures.process`（要 multiprocessing）；`traceback.print_exc` 要 `sys.exception`；`sys._getframe(depth>0)`；`builtins.dir`；位置表（`firstlineno`/`positions()` 证据自相矛盾，未证实）。

## 团队模式（上会话的工作方式，建议沿用）

- **Lead 不写代码**：拆任务上板（`team_task_create`，写死写域）→ 队员执行（认领→落地→汇报）→ Lead 审查 diff、跑终审闸门、统一推送。
- **队员用 `ds-` 前缀命名**（用户已在本机 DSH 插件 `dsh-subagent-model-config` 配了 `ds-*` ⇒ deepseek 路由）。
- **队员上下文会耗尽**：一个队员跑 20＋轮后会话到极限 ⇒ 把配方写进本页／台账后换新队员接棒（上会话换了 5 棒：ds-lib-sync → ds-regex-engine → ds-typing-shell → ds-typing-close → ds-typing-tail）。**注意成员上限 8**——旧成员占满名额会挡新 spawn，且 Lead 没有移除成员的工具（上会话末正是因此阻塞，等用户清了旧成员/换会话才解除）。
- **同墙 3 轮上限**：修不通就停手立据升级，把选项交用户；**红灯不提交**；解析器／编译器改动**不与语义分离落地**（只让语法过却丢语义＝说谎）。

## 本战役的关键口径（新会话必读）

- **分子测量**：`lib_import_ratio.py` 跑的是 **conformance 测试二进制** ⇒ 改 core/stdlib 后必须先 `tools/cargo.sh test -p pyawa-abi --test conformance --no-run`（或跑 slowcheck 第 9 项）**再量**，否则量到旧二进制（踩过两次 ✓）。
- **四条探针规矩**（插桩定位用，全文见台账卷）：① 按文本锚定不用行号；② 标记只用 ASCII（全角字符有词法缺口）；③ 躲开装饰器（插 `def` 行后）；④ 插进体里要 +4 缩进。收尾**必须** `rm -rf Lib/__pyawa__`（旧坏产物会被复用）。
- **收账两道必检**：逐个导入＋无关脚本 `startup ok`；只同步**能 import** 的文件进 `Lib/`（不能 import 的撤到 `target/` 等前置）。
- **内建类型建法**（`alloc_type_raw` 型）：方法挂类型的 **`getattr` 槽**（不走 MRO 查字典）；`new` 槽收 kwargs 用 `type.__call__` 覆写路（`TypeVar.__new__` 是范本）。
- **上会话战果**：~30 处核心/编译器/内存安全真 bug 全修（`as u8` 截断族 6 例＋横扫 56＋8 处、`POP_EXCEPT` 漏发、`str.find` 忽略 start/end（死循环根因）、`len(str)` 数字节、两处 UAF、借用 vs 吃引用族等）——**每处都有最小复现＋逐字节/逐例对拍＋护栏**钉在 `crates/pyawa-runtime/tests/*_shapes.rs`。

## 纪律提醒

- 红灯不提交 ✓；代码＋台账＋`NEXT.md` **同笔** ✓；闸门**只看 exit code**（不接管道 ✓）；
- 判据① **5 轮不动**即报停滞 ✓；**禁止空转**：一轮内不能只写台账 ✓；
- 判据① 单次读数 ±1 ⇒ 报"区间" ✓；`slowcheck` 十项全绿（含 stability 连跑口径 MS-25）才许提交 ✓。
