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
> 用户已拍板 P3-38 epic，进度 **S1 ✓ → S2a-1 ✓ → S2a-2 起继续**；**未合并 main、未 tag m6**。
> 逐轮复盘见 `docs/rounds/03-m3-progress-05.md` 的 **round 14…22** 节 ✓（本笔追加 round 22）。

## 现在

- **HEAD**：`b9061db`（`dev`，本笔前）—— 本笔＝**round 23**：P3-38 **S2a-1** —— `StrObject` 存储升 WTF-8 表示（`StrRepr{Utf8(String), Wtf8(Vec<u8>)}`、`new_wtf8`、`Instance::new_str_wtf8`/`wtf8_of`、`as_utf8()`；`value()` 守卫、`text_of` 只对合法 UTF-8 给值 ⇒ **零可观测行为改动**）；新 `tests/str_wtf8.rs` 8 passed；细节见台账 round 23。
- **工作区**：本笔待提交 ＝ **7 项**（`git status --porcelain` 实测：`builtin_objects/scalars.rs`、`instance/accessors.rs`、`instance/alloc.rs`、`crates/pyawa-core/tests/str_wtf8.rs`（新增）、`NEXT.md`、台账 round 23、`ROUNDS.md`）。
- **stash**：**空** ✓
- **round 23 新增验收（全 ✓）**：
  1. **S2a-1 落地**：`StrObject` 载荷改 `StrRepr{Utf8(String), Wtf8(Vec<u8>)}`（手写保持既有 8 处 `new(ty,String)` 签名；`#[repr(C)]`/header 首字段/dealloc 同形）；
     `new_wtf8`（`wtf8::validate` 失败⇒None；合法 UTF-8 仍存 Utf8 ⇒ 零行为改动）、`wtf8_bytes()`、`as_utf8()`、`value()` 守卫；
     `Instance::new_str_wtf8`（空串单例）、`Instance::wtf8_of`；`text_of` 改走 `as_utf8()`（Wtf8⇒None，唯一允许的行为改动点，已点名）。
  2. **测试**：新 `tests/str_wtf8.rs` **8 passed**（旧面不变／空串单例同指针／合法 UTF-8 留 Utf8／孤立代理 `a\xED\xA0\x80b` ⇒
     `wtf8_of` 原字节、`text_of`=None、`#[should_panic]` 守卫／WTF-8 形状往返／非法 WTF-8 六类 None／非 str ⇒ None）；`--test wtf8` 13 passed。
  3. 既有验收仍全过：`test_decorators` 16/16；`import test.support` ok；`--test compile` 逐字节 4 passed；check.py 14/14。
- **数字**：本轮零可观测行为改动（WTF-8 分支尚未被生产路径构造），沿用 round 21/22 基线：**可编译 367/397＝92.4%**、跑通 9/397、
  语法错 30（全部 surrogate）、未实现 48、运行期错 271、超时 39；判据① **477/628＝76.0%**；语料 407／新差异 0（MS-14 407/407）。
- **M6 收口结论**：MS-14 ✓／新差异 0 ✓；**P3-38 未完成 ⇒ 不满足 M6 收口**（MS-19 补充）。epic 进度：**S1 ✓ → S2a-1 ✓ → S2a-2/S2b 待开工**。
- **下一轮**：**S2a-2**（按审计 §2.3 分批把必须 surrogate-aware 的调用点迁到 `wtf8_bytes()`/`as_utf8()`，不改语义、逐批提交）→
  **S2b**（lexer 转义/AST/Constant/emitter/pyac 走 WTF-8；须过 compile 逐字节＋`lib_compile`＝epic 编译闸）→ S2c → S3a（codecs，解锁 os_helper）→
  S3b（fs/io/argv/ABI）→ S3e（`os_helper`/`multibytecodec_support` 同步回归）→ 才能报 M6 收口（`--no-ff`＋tag `m6`）。

## 下一阶段（M6 兼容性验收；`DESIGN.md` §12）

1. **M6 判据**：`MS-14` ✓（407/407、新差异 0）＋ **`P3-38` 必须完成**（`MS-19` 补充）才能收口。
2. **P3-38 阶段**：S1 ✓ → S2a-1 ✓ → S2a-2 → S2b → S2c → S3a → S3b → S3e；每阶段独立提交＋十项闸门＋survey 对应格归零。
3. **收口动作**（epic 完成后）：复核 MS-14／新差异 → 报用户批准 `--no-ff` 收 `main` ＋ tag `m6`。

## 下一条命令（下一轮开工）

```text
git status --short                      # 应干净（round 23 已提交）
# 下一步：S2a-2 —— 按 target/recon/t48_str_audit.md §2.3 的"必须 surrogate-aware"清单，分批迁调用点（先 repr/print/编码面，再名字面）
cargo test -p pyawa-core --test str_wtf8
# M6 收口（仅当 P3-38 完成并复核 MS-14/新差异后）：报用户批准 → git checkout main && git merge --no-ff dev && git tag m6 && git push --tags
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
