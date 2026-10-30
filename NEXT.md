# NEXT.md —— 续做状态（单页；长复盘进台账 `docs/rounds/`）

> 规则见 `agents-rules/round-rule.md`：**一轮一落地**（每轮以「提交」或「撤回＋一条改变计划的事实」结束 ✓）。
> **本页是会话交接单页**（2026-10-25 ✓）：会话 10＝**M6**（r14：`P3-32`/`P3-33`；r15：`else`＋`with` 编译器格；r16：`time` 面 ⇒
> `import test.support` 通过；r17：P3-35 批 A＋`sys.exc_info`＋`gc`＋4 骨架；r18：**P3-34 complex**＋mock 同步＋**try/with 余部重放修**；
> r19：**P3-36 `.pyac` 修**＋**match 全形**＋`sys.monitoring`＋**`unicodedata`** ⇒ `import pdb`/`doctest`；
> r20：**doctest 链同步（×20→0）**＋批 B/C 语法＋**`compile()`/`f_back`**＋**`hashlib`（×4→0）**；
> r21：**非 surrogate 语法归零**（可编译 92.4%）＋core 小格批 ⇒ **`test_decorators` 16/16 OK**；
> r22：用户拍板**启动 P3-38 surrogate epic** ⇒ 立项（S1…S5）＋**S1 WTF-8 基础模块**（23 测试）＋S2/S3 落地审计（28KB）；
> r23：**S2a-1** `StrObject` 存储升 WTF-8 表示（`StrRepr` 双分支、`new_str_wtf8`/`wtf8_of`/`as_utf8`，**零行为改动**＋8 测试）。
> **M6 收口结论（2026-10-25）**：`MS-14` ✓（407/407、新差异 0）；但 `MS-19` 补充下 **surrogate 未解 ⇒ M6 不标记完成**；
> 用户已拍板并指示继续 P3-38 epic；进度 **S1 ✓ → S2a-1 ✓ → 批1 ✓ → 批2 ✓ → S2b ✓ → S3a ✓ → S3b-1 ✓ → S3e-1 ✓ → S3b-2 ✓（fs surrogateescape 路径）→ S3e-2 ✓（小格批）**；
> **语法错 0、可编译 397/397＝100%、跑通 17/397**；闸门 release 档 ≈17min；**未合并 main、未 tag m6**。
> 逐轮复盘见 `docs/rounds/03-m3-progress-05.md` 的 **round 14…22** 节 ✓（本笔追加 round 22）。

## 现在

- **HEAD**：`e70644d`（`dev`，本笔前）—— 本笔＝**round 26**：P3-38 **S2b**（lexer/AST/emitter/pyac 走 WTF-8 ⇒ `'\ud800'` 字面量可编译，**语法错 30→1、可编译 396/397＝99.7%**）＋ release 档两处测试门控 ＋ **闸门提速 `slowcheck --release`**（≈4.2×：③163s/⑤490s/⑥385s/⑨107s/⑩66s，合计 ≈21min）＋ release 全量 survey（2m24s，跑通 10/397）；细节见台账 round 26。
- **工作区**：本笔待提交 ＝ **16 项**（以 `git status --porcelain` 实测为准：BI 编译器 `stmt_compound.rs`＋`classes.rs`；BJ fs `posix_module.rs`/`instance/fs.rs`/`fs_posix.rs`/`io_module.rs`；BK `sys_module.rs`/`imp_module.rs`/`math_module.rs`/`operator_module.rs`/新 `faulthandler_module.rs`/`lib.rs`＋`small_cells.rs`（新增）；`NEXT.md`＋台账 round 28＋`ROUNDS.md`）。
- **stash**：**空** ✓
- **round 28 新增验收（全 ✓）**：
  1. **编译器预存 bug 修复**：`condition_landings` 共享队列被体内嵌套 `if` 的 `take` 偷走 ⇒ 外层尾位 `if A and B and C:` 假出口落到体内
     ⇒ 第二次调用 `UnboundLocalError: lines`；改为"本条 if 私有落点 `split_off`"⇒ 假出口正确落收尾副本。回归测试（跑方法两次，回退即失败）、
     多种形状与 python3 逐行一致；`test_hmac.py`/`test_traceback.py` 不再崩（全文 0 条该错）。
  2. **S3b-2 fs 路径面**：`path_of` 改原始字节、补 `listdir`/`symlink` 槽、`fs_listdir/fs_rename/fs_symlink`、posix `path_argument`/`decode_path_text`
     （surrogateescape 真路径）＋ `os.listdir/rename/symlink/access`；端到端探针与 python3 **0 diff**（`b'\xff'` 文件 listdir/open/stat/rename/symlink/readlink/access；
     `ls -b`/xxd 字节证据）；合法 UTF-8 路径零变化。如实边界：`access` 按 mode 启发、`getcwd` lossy、受限 symlink 未实现、含代理 str 的 `sorted`/`<` 仍 panic（留下一轮）。
  3. **S3e-2 小格批**：`sys.path_importer_cache`/`path_hooks` 空 dict/list（`test_hmac` 真跑起来）、`_imp` override 契约（×9→0）、
     **`faulthandler` 模块**（15 名字、enable/disable/register/dump_traceback 真实现，信号/定时器/C 栈如实未接）、`math.isclose`/`nextafter` 全语义
     ＋ math TypeError 文案修正、`operator` 28 个 dunder 别名；新增 `tests/small_cells.rs`（5 项）。
  4. **release survey（2m31s）**：可编译 **397/397＝100.0%**、**跑通 17/397＝4.3%**（新入 `test___all__`/`test_math_property` 等）、
     未实现 80、运行期错 293、超时 7；`UnboundLocalError lines` ×6→0、`faulthandler` 缺模块 ×3→0、`_imp override` ×9→0、`path_importer_cache` 0。
     **MS-14：407/407、已知差异 0、新差异 0**；红线 compile 逐字节 4 passed；core 75／stdlib 19 全绿；0 warning；check.py 14/14。
- **数字**：判据① 待复核（next round）；语料 407／新差异 0。
- **M6 收口结论**：`MS-14` ✓、新差异 0 ✓、**可编译 100%、语法错 0**（`MS-19` 补充的"合法 Python 被拒"已消除）；
  P3-38 剩余：含代理 str 排序/比较、`\N{…}` 名字表 15、`except*` 7、async 推导式 4、PEP 695、下标删除 5、`import *` 属性字典 4、socket I/O 17、
  `__traceback__` 上下文 11（triage 首选）、fs/io 其余面 ⇒ **仍未到 M6 收口**。
- **下一轮**：① `_AssertRaisesContext`/`_GeneratorContextManager.__traceback__` ×11 triage ② 含代理 str 的 `sorted`/`<` panic（core）③ `\N{…}` 名字表评估
  ④ `except*`/async 推导式 ⑤ 判据①／语料复核。

## 下一阶段（M6 兼容性验收；`DESIGN.md` §12）

1. **M6 判据**：`MS-14` 报告可发布 ＋ 新差异 0 ＋ `MS-19` 补充——**语法错 0/可编译 100% 已达成**，主要剩运行时形态与少数未实现面。
2. **P3-38 剩余**：排序/比较面 → `\N{…}` 名字表 → `except*`/async 推导式/PEP 695/下标删除 → `import *` → socket I/O → fs/io 其余。
3. **收口动作**（全部绿后）：复核 MS-14／新差异 → 报用户批准 `--no-ff` 收 `main` ＋ tag `m6`。

## 下一条命令（下一轮开工）

```text
git status --short                      # 应干净（round 28 已提交）
tools/slowcheck.sh --release            # ≈17min
python3 tools/libtests_survey.py --release   # ≈2.5min
# 下一格：① `__traceback__` 上下文 ×11 ② 含代理 str 排序/比较 ③ `\N{…}` 名字表评估
```

## 本战役的关键口径（新会话必读）

- **判据① 分子测量**：先 `tools/cargo.sh test -p pyawa-abi --test conformance --no-run`（重建）再 `rm -rf Lib/__pyawa__`，
  然后 `python3 tools/lib_import_ratio.py`；**禁止 `| tail`**（汇总在失败清单**之前**）；单次 ±1 ⇒ 报区间；并发测量无效。
  **当前 477/628＝76.0%**（通过 460 ＋ 参照口径 17）。
- **`Lib/test` 体检口径**：`tools/libtests_survey.py`（397 个顶层 `test_*.py`；默认目录＝
  `target/upstream/extracted/usr/lib/python3.14/test`）。两个比例是**报告级**、**不得当验收或完成度**（`CM-15`）。
  **改编译器之后必须清 `.pyac` 缓存**：`find target Lib -name __pyawa__ -type d -prune -exec rm -rf {} +`
  ＋ `find target/upstream -name '*.pyac' -delete`（陈旧判定只看源字节，看不到编译器版本 ✗）。
- **阶段边界**：`main` 只在阶段完成时以 `--no-ff` 收录（现 tag `m1`…`m5`）；**不 reset／不重写历史／不 force-push**
  （`agents-rules/branch-rule.md`）。合并后应复核 `git diff main dev` 为空。
- **ABI／接口版本**：`PA_ABI_VERSION`＝**1.3**（`PA_ABI_MAJOR=1`／`PA_ABI_MINOR=3`；`CP-30`／`AB-39` 共用号；
  次版本 2＝round 15 `fs` 表尾 `stat_times`；次版本 3＝round 16 `clock` 表尾 `localtime`/`strftime`/`mktime`；按 `AB-41` 只升次版本）；
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
