# NEXT.md —— 续做状态（单页；长复盘进台账 `docs/rounds/`）

> 规则见 `agents-rules/round-rule.md`：**一轮一落地**（每轮以「提交」或「撤回＋一条改变计划的事实」结束 ✓）。
> **本页是会话交接单页**（2026-10-24 ✓）：会话 10＝**M6 开局**（用户 2026-10-24 指示：唯一目标 `P3-32`＋`P3-33`）。
> 逐轮复盘见 `docs/rounds/03-m3-progress-05.md` 的 **round 14** 节 ✓（本笔追加）。

## 现在

- **HEAD**：`d6be4ea`（`dev`，本笔前）—— 本笔＝**round 14：M6 开局，`P3-32`＋`P3-33` 落地**（细节与逐条证据见台账 round 14；本页只留续做状态）。
- **工作区**：本笔待提交 ＝ **34 个修改文件 ＋ 21 个 `Lib/` 新增**（`git status --porcelain` 共 **55** 项；全是本轮改动，无其他未跟踪项）。
- **stash**：**空** ✓
- **本轮五条验收（全 ✓，2026-10-24 实测）**：
  1. `pyawa /tmp/x.py`（`import sys; sys.exit(3)`）⇒ **退出码 3**。
  2. 最小 `unittest.TestCase` ＋ `unittest.main()` ⇒ **退出码 0**（1 个与 2 个测试方法两档都过）。
  3. `PYTHONPATH=/tmp/pp` ⇒ `sys.path` 含 `/tmp/pp` 且 `import ppmod` 成功。
  4. `tools/libtests_survey.py`（397 个 `test_*.py`，清缓存后）⇒ 可编译 **311/397＝78.3%**；
     **跑通 5/397＝1.3%**（上轮 0 ⇒ **首次非零**）。跑通名单：`test_colorsys`／`test_errno`／`test_html`／
     `test_int_literal`／`test_typechecks`。语法错 86／未实现 54／运行期错 250／超时 2。
  5. `tools/check.py` 14/14 ✓；`slowcheck` 十项全绿 ✓；`.py` 产物逐字节不变（`cargo test -p pyawa-core --test compile`）✓。
- **数字**（终审实测）：判据① **通过 449 ＋参照口径 17 ＝ 466 ÷ 628 ⇒ 74.2%**（**未变**；阈值 421/67% ✓；
  仪器见下「关键口径」）；`Lib/` 同步 **544/628**（进度指标：同步子集 **450/544＝82.7%**，**不作判据**）；
  语料 **407** 条、在册差异 **3** 条未变。
- **官方套件缺口榜（下一轮取活顺序的实测依据，`target/libtests_survey.tsv`）**：
  ① 运行期 `AttributeError: 'NoneType' object has no attribute 'get'` ×**135**（**函数体 `if/else` 的 `else` 含 `with`
  ⇒ 其后对模块全局的读变 `None`**；最小复现 `target/recon/helper_else_with.py`；命中 `Lib/sysconfig` ⇒ `test.support` 链仍卡）
  ② `ModuleNotFoundError` ×65（`doctest` 15／`unittest.mock` 12／`gc` 10／`unicodedata` 4／`hashlib` 4 → **`P3-37`**）
  ③ 语法错：`invalid unicode escape \ud800` 族 ×12 ＋ 同行 `try:`·`for:` ×8（→ **`P3-35`**）
  ④ 未实现：`complex` 的 `1j` 一族（→ **`P3-34`**）
  ⑤ `test_decorators` 触发**内存损坏**（`misaligned pointer dereference`／段错误；函数属性 `setattr(f, name, (args, kwds))`
  后读回即崩）——疑与 `P3-18`／`P3-19` 同族，单独立项。
- **`P3-32` 的"框架面"已成立**（`sys.exit`／`unittest.main()`／官方套件首次跑通）；**`P3-33` 已成立**
  （`PYTHONPATH` 进 `sys.path` ＋ `test/support` 骨架补同步 21 个上游文件）。`test.support` 导入链**仍未打通**，
  但卡点已换成本页缺口榜 ①（编译器格），不是 `P3-33` 的路径问题了。

## 下一阶段（M6 兼容性验收；`DESIGN.md` §12）

1. **M6 判据（`PLAN-milestones.md` §6）**：`MS-14` 的报告**可发布**，且"新差异"为零（对拍语料＋在册差异清单的复核；
   `MS-9` 归一规则、`MS-13` 语料下限按 `§13-10` 的剩余开放收口）。
2. **取活顺序（按序，逐条见 `PLAN-milestones.md` §9.2）**：**`P3-34`**（complex，52 个测试阻塞）→ **`P3-35`**
   （合法 Python 被拒：surrogate 20／同行 `try:`·`for:` 8；含本轮 7 个暂不同步的 `Lib/test` 骨架同族缺口）→
   **`P3-36`**（`.pyac` `BadCodeSection`，6 个官方测试）→ **`P3-37`**（缺模块，按 `CM-14` fan-in：doctest／`unittest.mock`／
   `gc`／`unicodedata`／`hashlib`）→ **报用户批准后 M6 收口**（`--no-ff` 收录 `main` ＋ tag `m6`）。
3. **另立专项（本轮新量出，不进上面的 P3 队列）**：① 函数体 `else`＋`with` 的全局读 `None`（编译器/作用域）；
   ② `test_decorators` 的函数属性内存损坏；③ `_functools` 模块（现在是 Python 回退，行为一致、键类名是 `K` vs 参照 `KeyWrapper`）。

## 下一条命令（下一轮开工）

```text
git status --short            # 应干净（round 14 已提交）
python3 tools/libtests_survey.py --jobs 8   # 重跑基线（默认目录已指向 target/upstream/extracted/...）
```

## 本战役的关键口径（新会话必读）

- **判据① 分子测量**：先 `tools/cargo.sh test -p pyawa-abi --test conformance --no-run`（重建）再 `rm -rf Lib/__pyawa__`，
  然后 `python3 tools/lib_import_ratio.py`；**禁止 `| tail`**（汇总在失败清单**之前**）；单次 ±1 ⇒ 报区间；并发测量无效。
  **当前 466/628＝74.2%**（通过 449 ＋ 参照口径 17）。
- **`Lib/test` 体检口径**：`tools/libtests_survey.py`（397 个顶层 `test_*.py`；默认目录＝
  `target/upstream/extracted/usr/lib/python3.14/test`）。两个比例是**报告级**、**不得当验收或完成度**（`CM-15`）。
  **改编译器之后必须清 `.pyac` 缓存**：`find target Lib -name __pyawa__ -type d -prune -exec rm -rf {} +`
  ＋ `find target/upstream -name '*.pyac' -delete`（陈旧判定只看源字节，看不到编译器版本 ✗）。
- **阶段边界**：`main` 只在阶段完成时以 `--no-ff` 收录（现 tag `m1`…`m5`）；**不 reset／不重写历史／不 force-push**
  （`agents-rules/branch-rule.md`）。合并后应复核 `git diff main dev` 为空。
- **ABI／接口版本**：`PA_ABI_VERSION`＝**1.1**（`PA_ABI_MAJOR=1`／`PA_ABI_MINOR=1`；`CP-30`／`AB-39` 共用号）；
  `pa_setcapability` **注册时校验** vtable 公共头（`version` 主版本＋次版本方向、`size` 8–4096）；**指令集版本** `INSTRUCTION_SET_VERSION`＝**2**。
- **`fd` 语义（`CP-31`）**：`fd`＝实例能力表下标，动态段从 **3** 起（0／1／2 标准流不占槽）；fs／net 共用一张表；
  `dup`／`dup2` 表项复制＋按 `(domain, provider_handle)` 引用计数（归零才调 provider close）；`dup2` 到 0／1／2 **未接** ⇒ EBADF。
- **路径解析（`CP-21`/`CP-32`）**：VM 侧 guest 绝对路径空间解析（cwd＋`.`/`..` 夹根＋逐段 `readlink`、深度 40）；
  provider 只做根映射、不再夹根；`readlink` 槽缺失时降级纯词法（如实记）。`sys.path` 最终归 `site.py`（`IM-24`），
  现在是组合根的**过渡半**（`[脚本目录, PYTHONPATH…, Lib]`）。
- **取消（`T-CP-10`）**：VM 侧 10 ms 时间片轮询式可取消等待；中断 ⇒ `ExecError::Interrupted` ⇒ `PA_ERR_INTERRUPT`；
  `select` 空列表＋正 timeout 真睡；无 timeout 的阻塞 `recv/send` 尚不可取消（如实记）。
- **编号现最大**：`BC-65`／`TS-50`／`T-BC-22`／`CP-39`／`CX-22`／**`P3-37`**（新增前先查已定义；墓碑算定义）。
- **扩展特性边界**：`.py` 下扩展语法是词法期 `Syntax`（`BC-15`）；扩展模式**没有参照实现**（`MS-13` ③）；
  `.py` 产物**逐字节不变**（`TS-5`，`cargo test -p pyawa-core --test compile` 守）。
- **四条探针规矩**：① 按文本锚定不用行号；② 标记只用 ASCII；③ 躲开装饰器；④ 插进体里 +4 缩进。
- **`__pyawa__` 缓存坑**：量之前 `rm -rf Lib/__pyawa__`（＋ `target/**/__pyawa__` 与 `target/upstream/**/__pyawa__`）。
- **共享工作区**：`git commit -F - -- <显式路径>`；代码＋台账＋本页 **同笔**提交；提交前核对本页 HEAD／工作区两行；
  闸门用 `&&` 只看 exit code；slowcheck 十项全绿才提交。
- **团队**：成员上限 8（队员已满 ⇒ 只唤醒旧成员，不再新 spawn）；`pyawa-core` 同时至多**两名**写手（不同子目录）。
