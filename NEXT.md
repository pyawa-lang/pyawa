# NEXT.md —— 续做状态（单页；长复盘进台账 `docs/rounds/`）

> 规则见 `agents-rules/round-rule.md`：**一轮一落地**（每轮以「提交」或「撤回＋一条改变计划的事实」结束 ✓）。
> **本页是会话交接单页**（2026-10-24 ✓）：会话 10＝**M6**（round 14 开局：`P3-32`＋`P3-33`；round 15：修 135 个官方测试阻塞的
> `else`＋`with` 编译器格 ＋ `test_decorators` 两个崩溃点）。逐轮复盘见 `docs/rounds/03-m3-progress-05.md` 的
> **round 14／round 15** 节 ✓（本笔追加 round 15）。

## 现在

- **HEAD**：`3b52bbb`（`dev`，本笔前）—— 本笔＝**round 15**：`else` 尾位 `with` 误发作用域收尾（编译器格）＋
  `setattr` 少 incref（崩溃）＋ `classmethod(A().foo)` 多层方法剥离（崩溃）＋ `fs.stat_times`（`st_mtime` 族，**ABI 1.1→1.2**）；细节见台账 round 15。
- **工作区**：本笔待提交 ＝ **19 项**（18 个修改文件：源码/测试/三份 `SPEC` ＋ `NEXT.md` ＋ 台账 round 15 ＋ `ROUNDS.md` 索引；＋ 1 个新增测试 `crates/pyawa-runtime/tests/stat_result.rs`；`git status --porcelain` 与之一致；无其他未跟踪项）。
- **stash**：**空** ✓
- **round 14（M6 开局）五条验收仍成立**：`sys.exit(3)`⇒3；最小 `unittest.main()`（1／2 测试方法）⇒0；
  `PYTHONPATH` 进 `sys.path`；官方套件跑通非零；`check.py`／slowcheck 绿、`.py` 逐字节不变。
- **round 15 新增验收（全 ✓）**：
  1. `if/else` 的 `else` 尾位 `with`（函数中段，`with` 后面还有语句）⇒ 返回值与 `python3` 一致（修前 `None`）：
     `target/recon/task17_else_with.py` 六形态逐行一致；`sysconfig.get_config_var('CFLAGS'/'CC')` 与参照**逐字一致**。
  2. `setattr(f, 'dbval', (args, kwds))` 后读回 ⇒ `(('a','b'), {'one':1})`；plain／`PYAWA_DANGLING=1`／
     `PYAWA_QUARANTINE=1` ×3 全 exit 0 且输出一致（修前 SIGSEGV／misaligned pointer）。
  3. 官方 `test_decorators` **不再崩溃**（`test_argforms`／`test_bound_function_inside_classmethod` 过；
     首个 fail 换成 `test_classmethod` 的独立行为缺口：`classmethod`／`staticmethod` 包装体的
     `__module__`／`__qualname__`／`__name__`／`__annotations__` 委派与 `repr` 未接 —— 新列小格）。
  4. `fs.stat_times`（ABI 1.2）：`os.stat('.')` 的 `st_size/st_mode/st_mtime/st_mtime_ns` 与 `python3` 逐行相同
     （`st_mtime == st_mtime_ns/1e9`、两次文件 `st_size` 不串）；`warnings_catch` **3 passed**、`linecache` 一致。
- **数字**（round 15 终审实测）：判据① **466/628＝74.2%**（通过 449 ＋参照口径 17；**未变**）；`PA_ABI_VERSION` **1.2**；
  `Lib/` 同步 **544/628**（进度指标 450/544＝82.7%，不作判据）；语料 **407**、在册差异 **3** 未变。
  `Lib/test` 体检：可编译 **311/397＝78.3%**、**跑通 5/397＝1.3%**（名单 `test_colorsys`／`test_errno`／`test_html`／
  `test_int_literal`／`test_typechecks` 未变）；运行期错 250／超时 2（`test_configparser`／`test_zstd`）。
- **官方套件缺口榜（下一轮取活顺序的实测依据，`target/libtests_survey.tsv`）**：
  ① 运行期 `AttributeError: 'module' object has no attribute 'strftime'` ×**135** —— 即 round 14 的 135 个
  `'NoneType'…get'` 被**整体推进了一步**（`sysconfig` 已通，现在卡在 **`time` 面**：`time.strftime` 及
  `localtime`／`struct_time` 一族缺）。**下一轮第一格：`time` 面**（一次压 135 个，比 P3-34 的 52 更多）。
  ② `ModuleNotFoundError` ×64（`doctest` 15／`unittest.mock` 12／`gc` 10／`unicodedata` 4／`hashlib` 4 → **`P3-37`**）
  ③ 语法错：`invalid unicode escape \ud800` 族 ×12 ＋ 同行 `try:`·`for:` ×8（→ **`P3-35`**）
  ④ 未实现：`complex` 的 `1j` 一族（→ **`P3-34`**）
  ⑤ `BadCodeSection` ×6（→ **`P3-36`**）；超时 3（`test_configparser`／`test_ssl`／`test_zstd`）
- **本轮修掉的两格不再出现在缺口榜**：`else`＋`with` 全局读 `None`（编译器/作用域）、`test_decorators` 内存损坏。

## 下一阶段（M6 兼容性验收；`DESIGN.md` §12）

1. **M6 判据（`PLAN-milestones.md` §6）**：`MS-14` 的报告**可发布**，且"新差异"为零（对拍语料＋在册差异清单的复核；
   `MS-9` 归一规则、`MS-13` 语料下限按 `§13-10` 的剩余开放收口）。
2. **取活顺序**：**`time` 面**（`strftime`／`localtime`／`struct_time` 族，一次解锁 **135** 个官方测试签名；
   需要先定"时区/格式化走哪条路"：能力域加槽 or 运行时平台层，按 `CP-`／`AB-` 的规则来）→ **`P3-34`**（complex，52 个）
   → **`P3-35`**（合法 Python 被拒：surrogate 20／同行 `try:`·`for:` 8；含 7 个暂不同步的 `Lib/test` 骨架同族缺口）
   → **`P3-36`**（`.pyac` `BadCodeSection`）→ **`P3-37`**（缺模块）→ **报用户批准后 M6 收口**（`--no-ff` ＋ tag `m6`）。
3. **另立小格（按需插队）**：`classmethod`／`staticmethod` 包装体属性委派＋`repr`；`issubclass() arg 1 must be a class`
   （`test_decorators` 次级症状，疑同源）；`_functools` 模块（现为 Python 回退，行为一致、键类名差异＝实现观测面）。

## 下一条命令（下一轮开工）

```text
git status --short                              # 应干净（round 15 已提交）
import time / dir(time)                         # 量 time 面缺什么：time.strftime / localtime / gmtime / strftime 族
python3 tools/libtests_survey.py --jobs 8       # 重跑基线（默认目录已指向 target/upstream/extracted/...）
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
- **ABI／接口版本**：`PA_ABI_VERSION`＝**1.2**（`PA_ABI_MAJOR=1`／`PA_ABI_MINOR=2`；`CP-30`／`AB-39` 共用号；
  次版本 2＝round 15 给 `fs` vtable 表尾追加 `stat_times` 槽，按 `AB-41` 只升次版本）；
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
