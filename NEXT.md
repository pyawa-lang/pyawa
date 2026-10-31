# NEXT（会话 11 进行中 —— M6 后 P3 批次）

> **M6 已收口（2026-10-31）**：`MS-14` 407/407、已知差异 0、新差异 0；`MS-19` 补充随 `P3-38` surrogate epic 收口
> （`Lib/test` 语法错 0／可编译 397/397＝100%）；判据① 477/628＝76.0%；`Lib/` 逐字节同步 739 文件；
> `main` 已 `--no-ff` 收录 ＋ tag `m6`。**本战役（会话 11）只抬"官方套件能跑通的面"，不动 M6 的 gate ✓**。

- **HEAD**：`dev` 见 `git log -1`（round 38：`P3-37` 剩余 `pyexpat` 形状面＝**本批 7 轮全部落地**）；`main` ＝ M6 合并提交（tag `m6`）。
  **批次进度**：7 轮全部完成 —— round 32（`P3-41` 崩溃面）、round 33（`P3-39` StringIO）、round 34（`P3-40` os 面）、
  round 35（`P3-42` 断言签名 5/6 组）、round 36（`P3-35` 下标删除 1/6）、round 37（`P3-29` socket I/O 主体）、
  round 38（`P3-37` 剩余 `pyexpat` 形状面）。**官方套件跑通 18⇒21/397**。
- **工作区**：干净（`git status --porcelain` 空）。
- **下一条命令**（续做时）：`git fetch && git reset --hard origin/dev`（历史会被重写 ✗，先核对）；
  下一批（按用户取活顺序）：**`P3-42` 剩余两格**——(a) 特殊方法派发缺口（`==`/`!=` 不走 `__eq__`、
  `__add__` 等不派发；`executor/values.rs::values_equal` ＋ `protocol::compare_public`）；
  (b) `unittest` contextmanager 里 `KeyError` 污染断言异常 `__context__`（证据见 round 35 台账 §3）。
  然后 **`P3-35` 剩余 5 形态**：`\N{…}`（要 Unicode 名字表生成进 core）／`except*`（PEP 654）／
  `import *`（模块帧失败导入静默成 None 的 import 机制缺口，证据见 round 36 台账）／async 推导式／PEP 695 `*Ts`。**`P3-35` 剩余 5 形态（未清，见 round 36 台账）**：`\N{…}` 15（要 Unicode 名字表
  生成进 core）／`except*` 7（PEP 654）／`import *` 4（模块帧失败导入静默成 None 的 import 机制缺口，
  证据在台账）／async 推导式 4／PEP 695 `*Ts` 2。**`P3-42` 剩余两格（未清）**：
  `KeyError` 族 12 行 = (a) **特殊方法派发缺口**（`==`/`!=` 不走 `__eq__`、`__add__` 等不派发，
  `executor/values.rs::values_equal` ＋ `protocol::compare_public`）＋ (b) `unittest` 的 contextmanager
  生成器里 `KeyError` 在测试体执行期间仍处"正在处理"⇒ 断言异常的 `__context__` 被污染；
  以及 `time.struct_time` 不是 `tuple` 子类（`test_structseq.test_tuple`）。
- **未修 bug＋判据**（如实，非 M6 gate）：
  - **官方套件（release）**：跑通 **20/397＝5.0%**（可编译 397/397、未实现 100／运行期错 269／超时 8）；语料 `MS-14` 407/407 **未动** ✓。
  - **真 hang（1）**：`test_tempfile` —— `map()` 急求值（`type_runtime.rs:485`）消费无限 `_RandomNameSequence`
    （`Lib/tempfile.py:373`）⇒ 无限循环＋无界内存；复现 `target/recon/hang2/repro_tempfile_min.py`。
    修法方向：`map`／`filter`／`zip` 改正惰性迭代器（未修 ✓）。
  - **慢（不是 hang，已实测）**：`test_dict`／`test_weakref`＝dict 线性扫描 O(n²)（`dict.rs:75`）；
    `test_long`／`test_peepholer`＝`unittest.subTest` 机器开销＋对象不释放；`test_userstring`≈15 min
    （次因 `startswith/endswith` **忽略 `start`／`end`**）。复现脚本：`target/recon/hang1/`、`hang2/`。
  - **`P3-42` 剩余（如实）**：`KeyError` 族 12 行（两层根因见上）；`time.struct_time` 非 tuple 子类；
    `str.startswith/endswith` 的 `start`/`end` 仍不收；`IntLike`（`int.__index__`）实参族。
  - **`P3-39` 剩余（如实）**：`StringIO` 的 `newline` 换行翻译、`__getstate__`／`__setstate__`／pickle。
  - **`P3-40` 剩余（如实）**：`os.close` 已关 fd 仍静默（参照 `EBADF`）、`get/set_inheritable`、
    `os.chmod`／`os.scandir`（顺带暴露的新层缺口）。
  - **`P3-35` 剩余**：`\N{…}` 15／`except*` 7／`import *` 4／async 推导式 4／PEP 695 `*Ts` 2（下标删除已落地 round 36）。
  - **`P3-29` 剩余（如实）**：IPv6／AF_UNIX 地址线、真超时语义（`settimeout` 只记值）、
    `_socket.poll`／`select` 的 socket 分流、`sendto`／`recvfrom`／`getsockopt`、`getaddrinfo` 一族（A2）。
  - **`P3-37` 剩余（如实）**：`pyexpat` 的**解析本体**（整块 expat C 扩展：XML/DTD/实体/命名空间/错误码语义）未移植；
    形状面已落（round 38），`test_pyexpat` 仍到不了断言层。
  - 其余旧账：`zipimport`／`urllib.request` 等缺模块、3 个编译器 cell/free 表 panic 用例、
    `FAILED(errors=1)` 泛化、"`tb_next` 链"、fs/io 其余（标准流 `dup`／`sys.prefix`）、`P3-38` 的 4 条有损输出通道边界。
- **闸门命令**：`tools/slowcheck.sh --release`（十项 ≈15–20 min；round 32–35 均全绿）＋
  `python3 tools/libtests_survey.py --release`（≈2.5 min）＋ `python3 tools/lib_import_ratio.py`（≈5–10 min）＋
  `python3 tests/ci/check.py`（14/14）。

## 本战役的关键口径（新会话必读）

- **一轮一落地**（`agents-rules/round-rule.md`）：每轮一笔提交（代码＋台账＋本页同笔）；闸门用 `&&` 只看 exit code；
  **slowcheck 十项全绿才提交**；提交 `git commit -F - -- <显式路径>`。报告三行：① 改了哪个 P3-xx ② 官方套件跑通数（前→后）③ 分诊里该签名是否消失。
- **`Lib/test` 体检口径**：`tools/libtests_survey.py`（397 个顶层 `test_*.py`；目录
  `target/upstream/extracted/usr/lib/python3.14/test`）＋ `tools/libtests_triage.py`。两个比例是**报告级**、不得当验收/完成度（`CM-15`）。
  **改编译器后必须清 `.pyac` 缓存**：`find target Lib -name __pyawa__ -type d -prune -exec rm -rf {} +`
  ＋ `find target/upstream -name '*.pyac' -delete`（陈旧判定只看源字节，看不到编译器版本 ✗）。
- **阶段边界**：`main` 只在阶段完成时以 `--no-ff` 收录（现 tag `m1`…`m6`）；**不 reset／不重写历史／不 force-push**
  （`agents-rules/branch-rule.md`）；历史已被重写过 ⇒ 续做先 `git fetch && git reset --hard origin/dev`（有未推送活儿则 `git rebase origin/dev`）。
- **ABI／接口版本**：`PA_ABI_VERSION`＝**1.4**（round 34 的 `fs.pipe` 表尾追加 ⇒ `PA_ABI_MINOR` 3→4；`AB-41`）；
  指令集版本 `INSTRUCTION_SET_VERSION`＝**2**。
- **`fd` 语义（`CP-31`）**：`fd`＝实例能力表下标，动态段从 **3** 起（0／1／2 标准流不占槽）；fs／net 共用一张表。
- **路径解析（`CP-21`/`CP-32`）**：VM 侧 guest 绝对路径空间解析；`sys.path` 最终归 `site.py`（`IM-24`），现在是过渡半。
- **编号现最大**：`BC-65`／`TS-50`／`T-BC-22`／`CP-39`／`CX-22`／**`P3-42`**（新增前先查已定义；墓碑算定义）。
- **`__pyawa__` 缓存坑**：量之前 `rm -rf Lib/__pyawa__`（＋ `target/**/__pyawa__` 与 `target/upstream/**/__pyawa__`）。
- **共享工作区**：代码＋台账＋本页**同笔**提交；提交前核对本页 HEAD／工作区两行；成员上限 8；`pyawa-core` 同时至多**两名**写手（不同子目录）。
