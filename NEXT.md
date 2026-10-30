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
> 用户已拍板并指示继续 P3-38 epic；进度 **S1 ✓ → S2a-1 ✓ → S2a-2 批 1 ✓（repr/eq/len）→ 批 2 ✓（码点操作面＋chr/ord）→ 批 3/S2b 起继续**；**未合并 main、未 tag m6**。
> 逐轮复盘见 `docs/rounds/03-m3-progress-05.md` 的 **round 14…22** 节 ✓（本笔追加 round 22）。

## 现在

- **HEAD**：`6c25663`（`dev`，本笔前）—— 本笔＝**round 25**：P3-38 **S2a-2 批 2** —— 码点操作面（迭代/下标/切片/拼接/重复/`in`/`len`/`str()`/`format` 直通）＋ **`chr`/`ord` 支持孤立代理**；`str_wtf8` 18 passed、`surrogate_chr_ord` 4 passed；e2e `chr(0xD800)` 全链与 python3 逐行一致；细节见台账 round 25。
- **工作区**：本笔待提交 ＝ **16 项**（`git status --porcelain` 实测：core `executor/{iter,subscript,ctrls,step_containers,step_format,format,runtime}.rs`、`builtin/str.rs`、`instance/{accessors,containers}.rs`、`tests/str_wtf8.rs`；stdlib `builtins_module.rs`、`tests/surrogate_chr_ord.rs`；`NEXT.md`＋台账 round 25＋`ROUNDS.md`）。
- **stash**：**空** ✓
- **round 25 新增验收（全 ✓）**：
  1. **码点操作面**（S2a-2 批 2）：str 迭代按码点（代理⇒`new_str_code_point` WTF-8）、下标/切片按码点、`+`/`+=`/`*` 与 f-string `BUILD_STRING`
     在 WTF-8 字节层、`in` 改 `str_contains`、`len` 走 `str_code_point_len`（`len(chr(0xD800))==1` 不再 panic）；
     `str()`/`FORMAT_SIMPLE`/`!s`/`"{}".format`/`"{0}"`/空规格/`"%s"` 对恰 str 身份直通（非空规格/嵌入 `%s` 暂有损 U+FFFD 保证不 panic，如实）。
  2. **`chr`/`ord` 支持孤立代理**（stdlib）：`chr(0xD800)` 真构造 WTF-8 串（0–0x10FFFF 全范围、合法标量零行为改动）、
     `ord` 按码点计数（含代理、str 子类兜底）；范围/类型/长度文案照 python3 3.14.4。
  3. **e2e 对拍**：`chr(0xD800)` 全链（len/eq/repr/concat/mul/idx/slice/iter/in/str identity/format/percent/f-string/iadd）与 python3 **逐行一致**；
     `--test str_wtf8` **18 passed**、`slicing` 4 passed、`surrogate_chr_ord` 4 passed；红线 compile 逐字节 4 passed；core 全量 0 failed；0 warning；check.py 14/14。
  4. **未覆盖站点清单**已入台账（`scalars.rs`/`protocol.rs`/`convert.rs`/`str_str` 槽/`FORMAT_WITH_SPEC` 规格串/`%` 另一处等）。
- **数字**：本轮仍无编译/Lib 面行为改动（WTF-8 串仅经 `chr` 构造），沿用 round 21 基线：**可编译 367/397＝92.4%**、跑通 9/397、
  语法错 30（全 surrogate）、未实现 48、运行期 271、超时 39；判据① **477/628＝76.0%**；语料 407／新差异 0（MS-14 407/407）。
- **M6 收口结论**：MS-14 ✓／新差异 0 ✓；**P3-38 未完成 ⇒ 不满足 M6 收口**。epic 进度：S1 ✓ → S2a-1 ✓ → S2a-2 批 1 ✓ → 批 2 ✓。
- **下一轮（S2a-2 批 3 / S2b 起）**：优先 **S2b 编译管线**（lexer 转义/AST/Constant/emitter/pyac 走 WTF-8；守 compile 逐字节＋`lib_compile`＝epic 编译闸），
  让 `\\ud800` 字面量从源码可用（届时 30 个语法错可开始归零）；同时补 `print`/`stdout.write` 写端（S3b）与剩余 `str` 方法面（trim/split/join/find 等）。
  之后 S3a codecs → S3e 同步 os_helper/multibytecodec_support 回归 → M6 收口（报用户批准）。

## 下一阶段（M6 兼容性验收；`DESIGN.md` §12）

1. **M6 判据**：`MS-14` ✓（407/407、新差异 0）＋ **`P3-38` 必须完成**（`MS-19` 补充）才能收口。
2. **P3-38 阶段**：S1 ✓ → S2a-1 ✓ → S2a-2 批 1 ✓ → 批 2 ✓ → **S2b（编译管线，epic 编译闸）** → S2c → S3a → S3b → S3e。
3. **收口动作**（epic 完成后）：复核 MS-14／新差异 → 报用户批准 `--no-ff` 收 `main` ＋ tag `m6`。

## 下一条命令（下一轮开工）

```text
git status --short                      # 应干净（round 25 已提交）
# 下一步：S2b —— lexer 转义返回 WTF-8 字节（\\ud800/\\uDC80 合法），Lexeme/Expression/Constant/AnnString/f-string 字面段/emitter/pyac
#   全部改 WTF-8 字节流；守 --test compile 逐字节 4 passed 与 lib_compile 绿＝epic 编译闸
cargo test -p pyawa-core --test str_wtf8
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
