# NEXT.md —— 续做状态（单页；长复盘进台账 `docs/rounds/`）

> 规则见 `agents-rules/round-rule.md`：**一轮一落地**（每轮以「提交」或「撤回＋一条改变计划的事实」结束 ✓）。
> **本页是会话交接单页**（2026-10-25 ✓）：会话 10＝**M6**（r14：`P3-32`/`P3-33`；r15：`else`＋`with` 编译器格＋两处崩溃；
> r16：`time` 面 ⇒ `import test.support` 通过＋包装体面；r17：P3-35 语法批 A＋`sys.exc_info`＋`gc`＋4 骨架；
> r18：**P3-34 complex 全链路**＋**`unittest/mock` 同步**＋**try/with 余部重放指数爆炸修**＋容器/getattr；
> r19：**P3-36 `.pyac` 修**（Ellipsis 标签撞车）＋**match 全形模式**＋`sys.monitoring`＋**`unicodedata`** ⇒ **`import pdb`/`doctest` 打通**
> ＋`sync_lib` 第二来源根）。逐轮复盘见 `docs/rounds/03-m3-progress-05.md` 的 **round 14…19** 节 ✓（本笔追加 round 19）。

## 现在

- **HEAD**：`e5f7326`（`dev`，本笔前）—— 本笔＝**round 19**：P3-36 `.pyac` `BadCodeSection`（Ellipsis pyac 标签与 Bool 撞车 ⇒ 独占标签 20；10→0）＋ **match 全形模式**（序列/映射/子模式或/`MATCH_CLASS` 位置形参）＋ `sys.monitoring` 最小面＋ **`unicodedata` 模块**（复用 unicode_tables）⇒ **`import bdb`/`pdb`/`doctest` 全通** ＋ `sync_lib` 第二来源根机制；细节见台账 round 19。
- **工作区**：本笔待提交 ＝ **17 项**（13 个修改文件：`pyac.rs`/`patterns` 三件/`symbols.rs`/`step_match.rs`/`sys_module.rs`/`lib.rs`/`tests`/`tools/sync_lib.py`/`pyawa-runtime/tests/pyac.rs` ＋ `NEXT.md` ＋ 台账 round 19 ＋ `ROUNDS.md` 索引；＋ 4 个新增文件：`crates/pyawa-stdlib/src/unicodedata_module.rs`、`tests/unicodedata.rs`、`tests/sys_monitoring.rs`；`git status --porcelain` 与之一致）。
- **stash**：**空** ✓
- **round 19 新增验收（全 ✓）**：
  1. **P3-36 `.pyac BadCodeSection`**：根因＝`Constant::Ellipsis` 的 pyac 标签 `0x07` 与 `Bool` 的 7 撞车（解码多读一字节 ⇒
     其后常量池/代码段错位）；改独占标签 20、旧产物走既有「INVALID ⇒ 按陈旧重编重写」自愈；`test_dis`/`test_types` 连跑 2 次无报错、
     第 3 次 mtime 不变（真命中缓存）；**survey `BadCodeSection` 10 → 0**。
  2. **match 全形模式**：序列 `[a,b]`/`[a,*rest]`/`[*rest]`/`()`/组模式、映射 `{"k":v}`/`{}`/值模式键、**子模式或模式**、
     `MATCH_CLASS` 位置形参（`__match_args__` 校验/错误文案、MATCH_SELF 单参回退）—— 36 档探针与 python3 **0 diff**、`--test patterns` 10 passed。
  3. **`sys.monitoring` 最小面**（pdb 的闸）：工具 id/哨兵/20 个 events 位照参照，10 个工具 API 名字在位、真调用如实 NotImplementedError；`import bdb` ✓。
  4. **`unicodedata` 模块**（`_pyrepl` 的闸，复用既有 `unicode_tables.rs`）：`unidata_version='16.0.0'`、`category`/`combining`/`east_asian_width`/
     `bidirectional`/`decimal`/`digit`/`numeric`/`decomposition` 真实现，`name`/`lookup`/`normalize`/`mirrored` 如实 NotImplementedError；
     值/文案探针 0 diff。⇒ **`import bdb`／`pdb`／`doctest` 全成功**。
  5. **`sync_lib` 第二来源根机制**（testsuite deb 解包根；只对主来源缺失文件用；`--check` 回退比对）；`multibytecodec_support` 因
     `'\udeee'` 合法孤立代理（surrogate epic）**按纪律不入库**，`--check` 553 文件全一致、`lib_compile` 绿。
- **数字**（round 19 干净基线实测）：判据① **470/628＝74.8%**（通过 453＋参照口径 17，**+1**）；`Lib/` **549**、进度指标 **454/549＝82.7%**；
  语料 407／在册差异 3 未变。`Lib/test` 体检：**可编译 337/397＝84.9%**、**跑通 7/397＝1.8%**、语法错 60、未实现 38、运行期错 260、
  **超时 32**（并发编译期间的 40/166/210 是噪声；本基线为无并发作业时测得；`test_ssl`/`test_urllib2net` 等网络类待甄别）。
- **缺口榜（下一轮取活）**：① `test.support.os_helper` **×93**（surrogate epic ⇒ 需字符串表示层 WTF-8 级改造，**独立立项**，不以假数据绕过）；
  ② `doctest` ×20（`Lib/doctest.py` 未同步；已实测它可编译执行 ⇒ **下一轮同步链** `doctest.py`＋`pdb.py`＋`bdb.py`＋`_pyrepl/*` 等，由 lib-sync 做）；
  ③ `hashlib` ×4（P3-37 剩余）；④ `\N{…}` 具名转义 11（Unicode 名字表数据面）；⑤ 批 B/C 语法缺口；⑥ 超时 32 甄别。
- **本轮修掉的格不再出现在缺口榜**：`BadCodeSection` ×10、`import pdb`/`doctest` 的 match/monitoring/unicodedata 闸。

## 下一阶段（M6 兼容性验收；`DESIGN.md` §12）

1. **M6 判据（`PLAN-milestones.md` §6）**：`MS-14` 的报告**可发布**，且"新差异"为零（对拍语料＋在册差异清单的复核；
   `MS-9` 归一规则、`MS-13` 语料下限按 `§13-10` 的剩余开放收口）。
2. **取活顺序**：**doctest/pdb/bdb/_pyrepl 同步链**（doctest ×20）→ `sys._getframe` f_back（`test_unicodedata` 下一格）→
   `hashlib`（×4）/`\N{…}` 名字表 → 批 B/C 语法缺口 → 超时 32 甄别 → **surrogate 独立立项**（string repr 改造；os_helper ×93 的头号解）→
   报用户批准 M6 收口（`--no-ff` ＋ tag `m6`）。**注意**：surrogate epic 是 MS-19 判据补充下的硬前提，需你拍板是否本轮启动。
3. **另立小格**：`sys._getframe` f_back；`builtins.compile`；嵌套类装饰器属性；`test_memoize 3!=4`；`class C(complex)` 子类构造；
   类型级描述符；哈希随机化；`list.__delitem__` 下标面；`events` 命名空间 `types.SimpleNamespace` 形状偏差。

## 下一条命令（下一轮开工）

```text
git status --short                      # 应干净（round 19 已提交）
# 下一格：lib-sync 同步链 —— doctest.py / pdb.py / bdb.py / _pyrepl/*（先逐个 pyawa 编译验证再入 SLICE，CX-8 逐字节）
python3 tools/sync_lib.py --check
```

## 本战役的关键口径（新会话必读）

- **判据① 分子测量**：先 `tools/cargo.sh test -p pyawa-abi --test conformance --no-run`（重建）再 `rm -rf Lib/__pyawa__`，
  然后 `python3 tools/lib_import_ratio.py`；**禁止 `| tail`**（汇总在失败清单**之前**）；单次 ±1 ⇒ 报区间；并发测量无效。
  **当前 470/628＝74.8%**（通过 453 ＋ 参照口径 17）。
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
