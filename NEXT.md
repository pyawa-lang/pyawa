# NEXT（会话 11 进行中 —— M6 后 P3 批次）

> **M6 已收口（2026-10-31）**：`MS-14` 407/407、已知差异 0、新差异 0；`MS-19` 补充随 `P3-38` surrogate epic 收口
> （`Lib/test` 语法错 0／可编译 397/397＝100%）；判据① 477/628＝76.0%；`Lib/` 逐字节同步 739 文件；
> `main` 已 `--no-ff` 收录 ＋ tag `m6`。**本战役（会话 11）只抬"官方套件能跑通的面"，不动 M6 的 gate ✓**。

- **HEAD**：`dev` 见 `git log -1`（round 34：`P3-40` os 面落地）；`main` ＝ M6 合并提交（tag `m6`）。
  **批次进度**：7 轮里已完成 round 32（`P3-41` 崩溃面）、round 33（`P3-39` StringIO）、round 34（`P3-40` os 面）；
  下一轮第 4/7（`P3-42` 断言失败签名批次）。
- **工作区**：干净（`git status --porcelain` 空）。
- **下一条命令**（续做时）：`git fetch && git reset --hard origin/dev`（历史会被重写 ✗，先核对）；
  然后 `python3 tools/libtests_triage.py --release` 复核签名 → **第 2 轮 `P3-39`（`io.StringIO`）**：
  `TypeError: cannot create 'StringIO' instances` 是当前头号签名（15 个官方测试阻塞；round 32 修的
  `test_frame` 的那 3 个 subTest ERROR 也是它）⇒ `crates/pyawa-stdlib/src/` 的 `_io`／`io` 面补
  `StringIO`（可实例化；`write`／`getvalue`／`close` 语义对齐；`test_io` 对应子集不再因 `TypeError` 失败）。
- **未修 bug＋判据**（如实，非 M6 gate）：
  - **官方套件（release）**：跑通 **18/397**（可编译 397/397、超时 7／未实现 98／运行期错 274）；语料 `MS-14` 407/407 **未动** ✓。
  - **真 hang（1）**：`test_tempfile` —— `map()` 急求值（`type_runtime.rs:485`）消费无限 `_RandomNameSequence`
    （`Lib/tempfile.py:373`）⇒ 无限循环＋无界内存；复现 `target/recon/hang2/repro_tempfile_min.py`
    （`map(str, itertools.count())`）。修法方向：`map`／`filter`／`zip` 改正惰性迭代器（未修 ✓）。
  - **慢（不是 hang，已实测）**：`test_dict`／`test_weakref`＝dict 线性扫描 O(n²)（`dict.rs:75`，
    64k 建表 123.6 s／70k 弱键约 10–12 min）；`test_long`／`test_peepholer`＝`unittest.subTest` 机器开销
    ＋对象不释放（`test_peepholer` 全量 442 s 跑完；`test_long` 单方法 259.6 s）；`test_userstring`≈15 min
    （次因 `startswith/endswith` **忽略 `start`／`end`** ⇒ 87–91% 断言失败，`str.rs:986`／`scalars.rs:805`）。
    复现脚本：`target/recon/hang1/`、`hang2/`（细节见 `docs/rounds/04-post-m6-01.md` round 32）。
  - **断言失败签名批次（`P3-42`，下一批逐条）**：`int.from_bytes` 缺 `byteorder`(2)／`time.struct_time` 缺(2)／
    `compile()` 不认 flags(2)／`Exception.__setstate__` 缺(2)／`_opcode.is_valid` 缺(1)／`KeyError: 'closed'`(3)／
    `KeyError: N`(8)；另本批新量：`str.startswith/endswith` 的 `start`/`end`、`IntLike`（`int.__index__`）。
  - **`P3-39` 剩余（如实）**：`StringIO` 的 `newline` 换行翻译、`__getstate__`／`__setstate__`／pickle。
  - **`P3-40` 剩余（如实）**：`os.close` 已关 fd 仍静默（参照 `EBADF`）、`get/set_inheritable`、
    `os.chmod`／`os.scandir`（本轮顺带暴露的新层缺口）。
  - **`P3-35` 剩余**：`\N{…}` 15／下标删除 8／`except*` 7／`import *` 4／async 推导式 4／PEP 695 `*Ts` 2。
  - **`P3-29`**：socket I/O 18（getaddrinfo 按 A2 推迟）；**`P3-37` 剩余**：`pyexpat`。
  - 其余旧账（不阻塞 M6 gate）：`zipimport`／`urllib.request` 等缺模块、3 个编译器 cell/free 表 panic 用例、
    `FAILED(errors=1)` 泛化、"`tb_next` 链"、fs/io 其余（标准流 `dup`／`sys.prefix`）、`P3-38` 的 4 条有损输出通道边界。
- **闸门命令**：`tools/slowcheck.sh --release`（十项 ≈15–20 min；round 32 全绿、round 33 全绿）＋
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
