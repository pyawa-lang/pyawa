# NEXT.md —— 续做状态（单页；长复盘进台账 `docs/rounds/`）

> 规则见 `agents-rules/round-rule.md`：**一轮一落地**（每轮以「提交」或「撤回＋一条改变计划的事实」结束 ✓）。
> **本页是会话交接单页**（2026-10-25 ✓）：会话 10＝**M6**（r14：`P3-32`/`P3-33`；r15：`else`＋`with` 编译器格；r16：`time` 面 ⇒
> `import test.support` 通过；r17：P3-35 批 A＋`sys.exc_info`＋`gc`＋4 骨架；r18：**P3-34 complex**＋mock 同步＋**try/with 余部重放修**；
> r19：**P3-36 `.pyac` 修**＋**match 全形**＋`sys.monitoring`＋**`unicodedata`** ⇒ `import pdb`/`doctest`；
> r20：**doctest 链同步（×20→0）**＋批 B/C 语法＋**`compile()`/`f_back`**＋**`hashlib`（×4→0）**；
> r21：**非 surrogate 语法归零**（可编译 92.4%）＋core 小格批 ⇒ **`test_decorators` 16/16 OK**；
> r22：用户拍板**启动 P3-38 surrogate epic** ⇒ 立项（S1…S5）＋**S1 WTF-8 基础模块**（23 测试）＋S2/S3 落地审计（28KB）。
> 逐轮复盘见 `docs/rounds/03-m3-progress-05.md` 的 **round 14…22** 节 ✓（本笔追加 round 22）。

## 现在

- **HEAD**：`926c2bb`（`dev`，本笔前）—— 本笔＝**round 22**：用户拍板**启动 surrogate epic** ⇒ **`P3-38` 立项**（PLAN §9.2 S1…S5＋§9.4 开工指示）＋ **S1 WTF-8 编解码基础模块**（新 `wtf8.rs`，23 测试全绿，零风险）＋ **S2/S3 落地审计**（`target/recon/t48_str_audit.md`：存储/构造点计数、S2a…S3e 切片与风险、§7 六问）；细节见台账 round 22。
- **工作区**：本笔待提交 ＝ **7 项**（`git status --porcelain` 实测：`crates/pyawa-core/src/wtf8.rs`、`crates/pyawa-core/tests/wtf8.rs`（新增）、`crates/pyawa-core/src/lib.rs`（`pub mod wtf8;` 一行）、`docs/PLAN-milestones.md`（P3-38 行＋开工指示）、`NEXT.md`、台账 round 22、`ROUNDS.md`）。
- **stash**：**空** ✓
- **round 22 新增验收（全 ✓）**：
  1. **P3-38 立项**：`PLAN-milestones.md` §9.2 `P3-38`（surrogate epic，S1…S5 每阶段独立提交＋逐阶段验收）＋ §9.4 开工指示；
     `check_p3_numbers.py` 24 编号 0 重复、`P3-38` 定义一次。
  2. **S1 WTF-8 基础模块**：`encode_code_point`（1/2/3/4 字节，**允许孤立代理**）／`decode`（严格、保留代理）／`validate`／
     `is_valid_wtf8`／`code_points`／`to_string_lossy`／`Wtf8Error{offset,kind}`（6 kind）；**23 测试全绿**
     （集成 13＋模块内 10：全部边界码点与孤立代理往返、编码→解码→再编码逐字节相同、截断/过长/非法首字节/坏续字节/超界逐条 Err）。
  3. **S2/S3 审计**：`target/recon/t48_str_audit.md`（28KB）—— 存储定义 1／`new_str` **387**／`text_of` **170**／名字面 **682**／
     编译期字面量点 76／上游含代理 59 文件（30 即当前语法错）；切片 S2a→S2b→S2c→S3a→S3b→S3d→S3e 与逐片文件清单/风险/回归面；
     最大风险 3 条（`text_of`/`&str` 边界、repr/print 输出约定、codecs/fs 四套实现）；§7 六问待拍板。
  4. 既有验收仍全过：`test_decorators` 16/16；`import test.support` ok；`--test compile` 逐字节 4 passed；check.py 14/14。
- **数字**：本轮**零运行期行为改动**（新模块＋文档），沿用 round 21 干净基线：**可编译 367/397＝92.4%**、跑通 9/397、
  语法错 30（全部 surrogate）、未实现 48、运行期错 271、超时 39；判据① **477/628＝76.0%**；语料 407／新差异 0（MS-14 407/407）；
  `Lib/` 564（`--check` 568）。P3-38 完成后预期：30 语法错→0、`os_helper` ×106→0、`multibytecodec_support` ×11 可同步。
- **M6 收口结论**：MS-14 ✓／新差异 0 ✓；**但 P3-38 未完成 ⇒ 按 `MS-19` 判据补充不满足"M6 收口"**（合法 Python 被拒必须修）。
  用户已拍板启动 epic；当前进度 **S1 完成、S2/S3 审计完成、S2a 待开工**。
- **下一轮（S2a 起）**：先等 §7 六问拍板（`text_of` 策略／输出边界／正则策略／pyac 版本／S3d 是否本期／epic 验收口径）→
  **S2a**（`scalars.rs`/`alloc.rs`/`accessors.rs`/`bootstrap.rs`/`state.rs`/`quote.rs`/`representations.rs`/`constructors.rs`：
  `new_wtf8`/`wtf8_of`，`text_of` 保留合法视图）→ S2b（lexer/AST/Constant/emitter/pyac，须过 compile 逐字节＋lib_compile）→
  S2c → S3a（codecs，解锁 os_helper）→ S3b（fs/io/argv/ABI）→ S3e（Lib 同步回归）；之后才能报 M6 收口。

## 下一阶段（M6 兼容性验收；`DESIGN.md` §12）

1. **M6 判据**：`MS-14` 报告可发布 ✓（407/407、新差异 0）但 **`P3-38` 必须完成**（`MS-19` 补充）才能收口。
2. **P3-38 阶段**：S1 ✓ → S2a → S2b → S2c → S3a → S3b → S3e；每阶段独立提交＋十项闸门＋survey 对应格归零。
3. **收口动作**（epic 完成后）：复核 MS-14／新差异 → 报用户批准 `--no-ff` 收 `main` ＋ tag `m6`。

## 下一条命令（下一轮开工）

```text
git status --short                      # 应干净（round 22 已提交）
# 下一步：按 t48_str_audit.md §5 开 S2a；先读 §7 六问并等用户拍板（S2a 动存储层前必须定 text_of 策略）
python3 tools/sync_lib.py --check
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
