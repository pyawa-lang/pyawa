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
> 用户已拍板并指示继续 P3-38 epic；进度 **S1 ✓ → S2a-1 ✓ → 批1 ✓ → 批2 ✓ → S2b ✓ → S3a ✓（surrogateescape/pass）→ S3b-1 ✓（输出/fs 属性）→ S3e-1 ✓（os_helper 解禁）**；
> **语法错已 0、可编译 397/397＝100%、跑通 15/397**；闸门已提速（`slowcheck --release` ≈18min；release survey ≈2.5min）；**未合并 main、未 tag m6**。
> 逐轮复盘见 `docs/rounds/03-m3-progress-05.md` 的 **round 14…22** 节 ✓（本笔追加 round 22）。

## 现在

- **HEAD**：`e70644d`（`dev`，本笔前）—— 本笔＝**round 26**：P3-38 **S2b**（lexer/AST/emitter/pyac 走 WTF-8 ⇒ `'\ud800'` 字面量可编译，**语法错 30→1、可编译 396/397＝99.7%**）＋ release 档两处测试门控 ＋ **闸门提速 `slowcheck --release`**（≈4.2×：③163s/⑤490s/⑥385s/⑨107s/⑩66s，合计 ≈21min）＋ release 全量 survey（2m24s，跑通 10/397）；细节见台账 round 26。
- **工作区**：本笔待提交 ＝ **26 项**（以 `git status --porcelain` 实测为准：编译/推导式 5 文件；S3a core `instance/convert.rs`/`builtin/str.rs`/`constructors.rs`/`scalars.rs`/`accessors.rs`＋stdlib `codecs_module.rs`/`multibyte_module.rs`＋2 新测试；S3b `_io_module.rs`/`builtins_module.rs`/`sys_module.rs`＋2 新测试；S3e `tools/sync_lib.py`＋3 个 `Lib/` 新文件；`NEXT.md`＋台账 round 27＋`ROUNDS.md`）。
- **stash**：**空** ✓
- **round 27 新增验收（全 ✓）**：
  1. **语法错归零 & 可编译 100%**：推导式目标支持星号（`[id for id, *_ in …]`，`UNPACK_EX`＋`STORE_FAST_STORE_FAST` 与参照 `dis` 逐条一致）⇒
     release survey **可编译 397/397＝100.0%**、语法错 **0**。
  2. **S3a codecs**：`surrogateescape`/`surrogatepass` 在 utf-8/ascii/latin-1/cp437/multibyte/`_codecs`/薄包装四套实现统一，
     170+ 行探针除允许未接的 utf-16/32 外 **0 diff**；顺修 UTF-8 `replace` 错误单元跨度真 bug（截断前缀 1 个 U+FFFD）。
  3. **S3b 输出/fs 属性**：`print`/`sys.stdout.write`/`stderr` 写 WTF-8 原始字节（`print(chr(0xD800))`⇒`ed a0 80`；合法 UTF-8 与 python3 字节 0 diff）；
     `print` 的 `sep/end/file/flush` 补齐；`sys.getfilesystemencodeerrors` 真 bug 修复（`surrogateescape`）；`os.fsdecode(b'\xff')⇒'\udcff'`、`os.fsencode('\udcff')⇒b'\xff'`。
  4. **S3e-1 同步**：`Lib/test/support/os_helper.py`、`libregrtest/utils.py`、`multibytecodec_support.py`（第二来源根）⇒ `Lib/` 571 文件逐字节一致、
     `lib_compile` 绿；**仓库 Lib 下 `import test.support.os_helper` ✓**（×100+ 缺模块归零）、`test.support`/`doctest` 不回归。
  5. **survey（release，2m28s）**：可编译 **397/397＝100.0%**、**跑通 15/397＝3.8%**、语法错 **0**、未实现 69、运行期错 305、超时 8；
     `ModuleNotFoundError` 158→**54**、`SkipTest` 56。
- **数字**：判据① 待下一轮复核（本轮 Lib 新增 3 文件、编译面变化，预期微升）；MS-14 待本轮树重跑（预期 407/407、新差异 0）。
- **M6 收口结论**：MS-14 ✓（上轮 407/407）；**P3-38 剩余 S3b-2（fs/io/argv/ABI 真实文件路径面）、S3c（正则/unicodedata/marshal 策略）、
  S3d（动态名字面，可后置）、S3e-2（官方测试抽样回归/剩余 ImportError）** ⇒ **仍未到 M6 收口**。
- **下一轮**：S3b-2 fs 路径面（posix `stat`/`open`/`listdir`/`readlink` 对含代理路径；`os.fspath`/`fsencode` 真实往返）→ S3e-2 官方测试抽样
  → 判据①／语料／MS-14 复核 → 报用户批准 M6 收口（`--no-ff`＋tag m6）。

## 下一阶段（M6 兼容性验收；`DESIGN.md` §12）

1. **M6 判据**：`MS-14` 报告可发布 ＋ 新差异 0 ＋ `MS-19` 补充（合法 Python 不再被拒）——**语法错已 0、surrogate 字面量已可编译**，主要剩 fs/io 运行时面与少数未实现形态。
2. **P3-38 剩余**：S3b-2 → S3c（策略待定：正则引擎只吃 `&str`，代理遇到如实未实现即可）→ S3d 后置 → S3e-2 回归。
3. **收口动作**（全部绿后）：复核 MS-14／新差异 → 报用户批准 `--no-ff` 收 `main` ＋ tag `m6`。

## 下一条命令（下一轮开工）

```text
git status --short                      # 应干净（round 27 已提交）
tools/slowcheck.sh --release            # ≈18min
python3 tools/libtests_survey.py --release   # ≈2.5min
# 下一格：S3b-2 fs 路径面（含代理文件名的 stat/open/listdir/readlink；os.fspath/fsencode 真实往返）+ S3e-2 官方测试抽样回归
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
