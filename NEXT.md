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
> 用户已拍板并指示继续 P3-38 epic；**P3-38 阶段全部落地**：S1 ✓ → S2a-1/2 ✓ → S2b ✓ → S2c ✓（`value()` panic 清零）→ S3a ✓ → S3b-1…4 ✓ → S3e-1/2 ✓；
> **语法错 0、可编译 397/397＝100%、跑通 18/397、判据① 477/628＝76.0%、MS-14 407/407 新差异 0**；release 闸门 1172s 全绿；**下一步：M6 收口（`--no-ff` 收 main ＋ tag m6，用户目标已授权）**。
> 逐轮复盘见 `docs/rounds/03-m3-progress-05.md` 的 **round 14…22** 节 ✓（本笔追加 round 22）。

## 现在

- **HEAD**：`e70644d`（`dev`，本笔前）—— 本笔＝**round 26**：P3-38 **S2b**（lexer/AST/emitter/pyac 走 WTF-8 ⇒ `'\ud800'` 字面量可编译，**语法错 30→1、可编译 396/397＝99.7%**）＋ release 档两处测试门控 ＋ **闸门提速 `slowcheck --release`**（≈4.2×：③163s/⑤490s/⑥385s/⑨107s/⑩66s，合计 ≈21min）＋ release 全量 survey（2m24s，跑通 10/397）；细节见台账 round 26。
- **工作区**：本笔待提交 ＝ **N 项**（以 `git status --porcelain` 实测为准：BL `instance/convert.rs`/`executor/protocol.rs`＋新 `str_wtf8_ordering.rs`；BM `executor/step_exceptions.rs`/`instance/accessors.rs`/`instance/alloc.rs`/`sys_module.rs`/`small_cells.rs`；BN `posix_module.rs`/`_io_module.rs`/`io_wtf8_output.rs`；`NEXT.md`＋台账 round 29＋`ROUNDS.md`）。
- **stash**：**空** ✓
- **round 30 新增验收（全 ✓）**：
  1. **S2c 收尾**：`StrObject::value()`（WTF-8 panic）用户可触发调用点**清零**（core+stdlib）；`text_value` 安全有损、`str_str`/`object_ascii`（`quote_wtf8`）/
     formatting/protocol/step_format/step_match/argdecode/classes/exceptions/gc/convert 等 21 文件安全化；新增码点层格式化 `format_wtf8_str`
     （`f"{s:>5}"`/`"{:>5}".format(s)` 逐字符一致）；探针矩阵 20 条无 panic（4 条已登记有损偏差：`%s` 嵌入/宽度、混合模板、`str.__str__`、`str(e)`）；
     新测 `str_wtf8_no_panic`（3 passed）。
  2. **S3b-4**：脚本参数/`__file__`/源码读取按原始字节；`sys.argv`/`orig_argv`/`executable` 按 utf-8+surrogateescape 解 WTF-8；非 UTF-8 参数/脚本名与 python3
     **0 diff**；`sys.executable` 修为真实路径；`readlink` str/bytes 两形状 0 diff。如实边界：`co_filename`/`.pyac` 名 lossy、`sys.prefix` cargo 目录、
     `os.pipe` 缺、标准流 dup EBADF。
  3. **Lib/test 差集同步**：新增 **168** 个（顶层支持模块 45＋`test_*` 7＋包 116）；`--check` **739** 逐字节一致、`Lib/` `.py` **735**、`sync_lib.py` +192/−1、
     `lib_compile` 绿；**892 个未入库**逐类记因（70 编译期缺口含 3 个 cell/free panic ＋66 `\N{…}`/`except*`/async/PEP695 等 ＋1 debug-only；822 非 triage；
     **无 KNOWN、未改上游**）；`--check` 顺带跳过 `Lib/**/__pyawa__/`；`mapping_tests`/`support.os_helper`/`multibytecodec_support` 探针全过。
  4. **release survey（2m48s）**：可编译 **397/397＝100.0%**、跑通 **18/397＝4.5%**、语法错 0、未实现 96、运行期错 271、超时 12；
     **判据① 477/628＝76.0%**；进度指标 477/735＝64.9%；**MS-14 407/407、已知差异 0、新差异 0**；红线 compile 4 passed；lib_compile 绿；check.py 14/14。
- **P3-38 结论**：S1…S3e 阶段全部落地；剩余非 gate 面（未实现形态 `\N{…}` 15／`except*` 7／async 推导式 4／PEP695 4／下标删除 5／`import *` 4、
  socket I/O 17、缺模块若干、cell/free 表 3 个 panic 用例、`FAILED(errors=1)` 泛化 16、`tb_next`、fs/io 其余）如实记录，**不影响 M6 三条判据**
  （MS-14 可发布＋新差异 0＋MS-19 合法 Python 不再被拒：语法错 0/可编译 100%）。
- **下一轮（收口）**：复核判据①（76.0% ≥ 阈值 67%）／语料 floor（407）／MS-14（407/407、新差异 0）＋ 十项闸门 → 更新 PLAN-milestones（M6 标记完成、
  P3-38 标记完成）→ **按用户目标执行 `git checkout main && git merge --no-ff dev && git tag m6 && git push`** → 出 M6 收口最终报告。

## 下一阶段（M6 兼容性验收；`DESIGN.md` §12）

1. **M6 判据**：`MS-14` 可发布 ✓（407/407、已知差异 0、新差异 0）＋ `MS-19` 补充 ✓（语法错 0、可编译 100%）＋ 判据① 76.0% ≥ 67% ✓。
2. **P3-38**：S1…S3e 全部落地 ✓；剩余非 gate 形态见上（后续里程碑再收）。
3. **收口动作**（下一轮）：PLAN/台账更新 → `--no-ff` 收 `main` ＋ `tag m6` ＋ push → 最终报告。

## 下一条命令（下一轮开工）

```text
git status --short                      # 应干净（round 30 已提交）
git checkout main && git merge --no-ff dev -m "M6: ..." && git tag m6 && git push --tags origin main dev && git checkout dev
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
