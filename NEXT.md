# NEXT.md —— 续做状态（单页；长复盘进台账 `docs/rounds/`）

> 规则见 `agents-rules/round-rule.md`：**一轮一落地**（每轮以「提交」或「撤回＋一条改变计划的事实」结束 ✓）。
> **本页是会话交接单页**（2026-10-25 ✓）：会话 10＝**M6**（r14：`P3-32`/`P3-33`；r15：`else`＋`with` 编译器格＋两处崩溃；
> r16：`time` 面 ⇒ `import test.support` 通过＋包装体面；r17：P3-35 语法批 A＋`sys.exc_info`＋`gc`＋4 骨架；
> r18：**P3-34 complex 全链路**（1j／对象类型／构造器／`hash()`…）＋**`unittest/mock` 同步**＋**try/with 余部重放指数爆炸修**＋
> deque/list.pop/getattr 补齐）。逐轮复盘见 `docs/rounds/03-m3-progress-05.md` 的 **round 14…18** 节 ✓（本笔追加 round 18）。

## 现在

- **HEAD**：`52058b4`（`dev`，本笔前）—— 本笔＝**round 18**：P3-34 complex 全链路（虚部字面量／真类型／`complex()` 构造器／`hash()`＋`abs/int/float/divmod` 面／规格）＋ `unittest/mock.py` 同步 ＋ **try/with「余部重放」指数爆炸修**（连续 ≥16 块 StackUnderflow／挂死）＋ `deque` 真值／`list.pop(index)`／`getattr(obj,name,default)` 修正；细节见台账 round 18。
- **工作区**：本笔待提交 ＝ **36 项**（30 个修改文件：编译器/执行器/stdlib/测试/`tools/sync_lib.py`/`docs/SPEC-c-modules.md` ＋ `NEXT.md` ＋ 台账 round 18 ＋ `ROUNDS.md` 索引；＋ 6 个新增文件：`Lib/unittest/mock.py`、`crates/pyawa-core/src/builtin_objects/complex.rs`、`crates/pyawa-core/tests/complex.rs` 等；`git status --porcelain` 与之一致）。
- **stash**：**空** ✓
- **round 18 新增验收（全 ✓）**：
  1. **P3-34 complex 全链路**：`1j`／`1.5j`／`1e3j`／`0j`／`1_0j` 字面量与常量形状（含折叠）与参照 `co_consts` 一致；`.pyac` round-trip 全等；
     complex 124+ 条语义探针（表示/属性/构造/字符串/关键字/-0.0/算术/比较/真值/类型面/hash/`__complex__`/identity/stdlib 面）与
     python3 3.14.4 **diff 为空**；`complex(z) is z`、`complex(0.0,-0.0)`⇒`-0j`、`divmod` 文案均一致。
  2. **`unittest/mock.py` 同步**：`Lib/unittest/mock.py` 与上游逐字节一致（`Lib/` **549**、`--check` **553 文件**）；`import unittest.mock` 可用、
     `patch(...).start()/stop()`／`with patch`／`MagicMock`／`mock(1,2)+assert_called_with` 全通；`pyawa …/unittest/mock.py` **exit 0**。
  3. **try/with 余部重放指数爆炸**（预存 bug：连续 `try` 块 ≥16 ⇒ StackUnderflow、≥17 ⇒ 挂死；连续 `with` 同款）：修后 N=13/16/17/30/64/200
     全 exit 0（≤0.26s），模块级/函数级/长体/try-finally 链/真实异常路径/连续 with 均与参照一致；`widen_extended_args` 改为迭代到不动点。
  4. **容器与 `getattr`**：`bool(deque())`⇒False（`ExitStack` 死循环根因）、`list.pop(index)` 全形态、`getattr(obj,name,default)` 吞
     属性链 AttributeError 且非 AttributeError 上抛 —— 与参照逐行一致。
  5. 既有验收仍全过：`import test.support` **ok**；`sys.exit(3)`⇒3；`unittest.main()`⇒0；`PYTHONPATH` 进 `sys.path`。
- **数字**（round 18 终审实测）：判据① **469/628＝74.7%**（通过 452＋参照口径 17，**+1**：`unittest.mock` 可 import）；`Lib/` **549**、
  进度指标 **453/549＝82.5%**；语料 407／在册差异 3 未变。`Lib/test` 体检：**可编译 338/397＝85.1%**、**跑通 7/397＝1.8%**（持平）、
  语法错 59（**28 surrogate**＋具名转义 11＋批 B/C 若干）、未实现 39／运行期错 270／**超时 22**（中途一度 132，主因即余部重放 bug，已回落）。
  **分类说明**：可编译 347→338 是 **`1j` 打通后文件解析得更远**撞上批 B/C 缺口（旧基线这些文件全卡在 `1j`），不是支持回退（已逐文件 tsv 比对）。
- **官方套件缺口榜（下一轮取活）**：① `test.support.os_helper` ×72 等 surrogate 族（**P3-35 剩余，需 string repr 级改造 ⇒ 独立立项评估**，不以假数据绕过）；
  ② `doctest` 18（导入 `pdb` ⇒ **match 序列/映射模式 opcode** 未接）；③ `.pyac BadCodeSection` 10（**P3-36**）；④ `unicodedata`/`hashlib`/
  `multibytecodec_support`（P3-37 剩余，第二个来源根待拍板）；⑤ `\N{…}` 具名转义 11（Unicode 名字表数据面）；⑥ 超时 22（多为 socket/server/long 类，逐条甄别）。
- **本轮修掉的格不再出现在缺口榜**：`1j`（P3-34）、`unittest.mock` 缺模块、连续 try/with 挂死、deque 真值、`list.pop(index)`、`getattr` 三参。

## 下一阶段（M6 兼容性验收；`DESIGN.md` §12）

1. **M6 判据（`PLAN-milestones.md` §6）**：`MS-14` 的报告**可发布**，且"新差异"为零（对拍语料＋在册差异清单的复核；
   `MS-9` 归一规则、`MS-13` 语料下限按 `§13-10` 的剩余开放收口）。
2. **取活顺序**：**`P3-36`（`.pyac BadCodeSection` 10）** → **doctest 的 match 模式 opcode（18）** → P3-37 剩余（`unicodedata`/`hashlib`；
   `multibytecodec_support` 待拍板）→ `\N{…}` 名字表 → 批 B/C 语法缺口（`~T` 类型表达式／star 表达式／augassign 链／参数注解表达式等）
   → 超时 22 甄别 → **surrogate 独立立项**（string repr 改造）评估 → 报用户批准 M6 收口（`--no-ff` ＋ tag `m6`）。
3. **另立小格**：`builtins.compile` 缺失；嵌套类装饰器属性丢失；`test_memoize 3!=4`；`f_back` 链；`class C(complex)` 子类构造；
   类型级描述符；哈希随机化；`list.__delitem__` 下标面。

## 下一条命令（下一轮开工）

```text
git status --short                      # 应干净（round 18 已提交）
python3 tools/libtests_survey.py --jobs 8
# 下一格：P3-36 —— 用 test_dis/test_types/test_typing 等 10 个 BadCodeSection 文件定位 .pyac 代码段读写 bug（IM-18…IM-21 容器契约）
```

## 本战役的关键口径（新会话必读）

- **判据① 分子测量**：先 `tools/cargo.sh test -p pyawa-abi --test conformance --no-run`（重建）再 `rm -rf Lib/__pyawa__`，
  然后 `python3 tools/lib_import_ratio.py`；**禁止 `| tail`**（汇总在失败清单**之前**）；单次 ±1 ⇒ 报区间；并发测量无效。
  **当前 469/628＝74.7%**（通过 452 ＋ 参照口径 17）。
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
