# NEXT.md —— 续做状态（单页；长复盘进台账 `docs/rounds/`）

> 规则见 `agents-rules/round-rule.md`：**一轮一落地**（每轮以「提交」或「撤回＋一条改变计划的事实」结束 ✓）。
> 第 579–720 轮的长复盘已**收口归档**进 `docs/rounds/03-m3-progress-03.md` ✓（本笔同笔提交 ✓）。

## 现在

- **HEAD**：`382bd57`（`dev`）—— `ds-lib-sync` 的**路线 2 枢纽⑤**（`_random`／random／tempfile）那笔 ✓（提交前已核对 `git rev-parse --short HEAD` ✓）
- **工作区**：**干净** ✓（`git status --porcelain` 为空 ✓ —— **以本笔收口提交为准** ✓；本单页随其后一笔 docs 提交落地 ✓）
- **stash**：**空** ✓
- **判据①**（`python3 tools/lib_import_ratio.py`，本笔实测 ✓）：**通过 242 ＋ 参照口径 17 ＝ 259 ÷ 628 ⇒ 41.2%** ✗（阈值 67%）
  - 上一笔 **252 ÷ 628 ⇒ 40.1%** ⇒ 本笔 **＋7** ✓（random／tempfile／shutil／fnmatch／email.generator／urllib.error／urllib.response ✓）
  - 判据② **已达成** ✓：`target/recon/reparity.py` 的 **13 行**与参照（本机 `python3` 3.14）逐字一致 ✓
  - 进度指标（**不作判据** ✓，`CM-15`）：`Lib/` 已同步子集 **352 个文件 ⇒ 能 import 243 个 ⇒ 69.0%** ✓

## 下一条命令（Lead 队列 ⑥：pathlib／glob／zipfile／typing／inspect 簇）

**用户已拍板路线 2** ✓：按"**阻塞最多模块的报错签名**"队列抬判据① 分子，目标 **≥ 421/628 ＝ 67%** ✓
（`enum.py` 换回支降为**低优先** ✓，不再主攻 ✓）。**排活口径＝全分母 628** ✓。

**枢纽⑤本笔落地** ✓：`_random.Random` 真类（xoshiro256\*\*）＋ `Lib/random.py`／`tempfile.py`／
`shutil.py`／`fnmatch.py`（逐字节一致 ✓）；`math.lgamma`、`posix` 的 `urandom`（走 fs 能力域 ✓）／
`unlink`／`rmdir`／`getpid`／`stat_result`／`F_OK` 一族／`open(2)` 标志位、
`object.__init_subclass__` ＋ `call_init_subclass`（隐式类方法语义 ✓）、整数幂负指数 ✓
—— 详见本笔提交正文 ✓（`_random` 与参照 MT19937 的差异也如实记了 ✓）。

**枢纽⑤遗留（已立据 ✓，见「未修 bug」）** ✗：`random.sample` 卡在 **ABC 虚拟子类注册**
（`isinstance([1], Sequence)` 为假 ✓）、`random.shuffle` 卡在 **`reversed` 只接 list**、
`random.randbytes` 卡在 **`int.to_bytes` 只接 `i64`**、`tempfile.gettempdir()` 运行期还需补 posix 面 ✓。

**下一手** ✓（Lead 已排定）：⑥ `pathlib`＋`glob`＋`zipfile`＋`typing`＋`inspect` 簇
（`importlib.resources`／`metadata` 收尾 ✓；`glob` 要 `fnmatch` ✓ —— 本笔已同步 ✓）→
⑦ `logging`＋`threading` → **socket 立支评估** → ⑧ `asyncio` 探墙 → ⑨ CJK `_codecs_*` 专支 →
⑩ `multiprocessing` → ⑪ `test.support` 殿后 ✓。

**四条探针规矩**（四次踩坑换的 ✓，务必照办）：
① **按文本锚定** ✓（**不用行号** ✗）；
② 标记文本**只用 ASCII** ✓（全角字符会撞我们自己的**词法缺口** ✗ —— 那本身是一处待接线的小缺口 ✓）；
③ **躲开装饰器** ✓（插在 `def` 行**之后** ✓，别插在 `@…` 与 `def` 之间 ✗）；
④ 插进 `for`／`if`／`def` 的**体**里要**＋4 缩进** ✓（同缩进会把体写空 ✗）；且 `for …:` 这类**别匹配到括号内的推导式** ✗。

**探针收尾也要清缓存** ✓：跑完还原后**必须** `rm -rf Lib/__pyawa__` ✓，否则旧的坏产物会被复用 ✗（第 718–720 轮踩到一次 ✓）。

## 仍等你拍

- （暂无 ✓）路线 2 已拍板（目标分子 ≥ 421/628 ＝ 67% ✓）；`enum.py` 换回支保留为低优先 ✓。

## 未修 bug ＋ 判据

- **终结器／`super` 那条 UAF** ✗：**可观测失败已消失** ✓（重压 6×6：修前 27/36 红 ⇒ 修后 0/36 绿 ✓）；
  底层"**字典是否被重复释放**"**仍未证明** ✗（见归档卷第 578 轮 ✓）。
- **判据②** ✓：`re` 的 **13 行**逐字对账**已达成** ✓（见「现在」✓）。
- **判据①** ✗：**259 ÷ 628 ＝ 41.2%**（阈值 67%）—— 路线 2 推进中，见「下一条命令」✓。
- **假货 `Lib/enum.py`** ✗：`python3 tools/sync_lib.py --check` 的**唯一一处**与上游不一致 ✓（`Lib/` 其余源文件逐字节相同 ✓）。
- **ABC 虚拟子类注册不生效** ✗（枢纽⑤实测，**影响面广** ✓）：`_collections_abc` 里
  `Sequence.register(list)` 等注册**做了也不算数** ✗ —— `isinstance([1], Sequence)` 为 `False` ✓
  （`random.sample` 因此报 `Population must be a sequence` ✓）。这是**独立的一块**（`ABCMeta.register`
  ＋ `__instancecheck__` 的注册表）⇒ 建议单独立支 ✓。
- **`reversed` 只接 list** ✗（枢纽⑤实测）：`random.shuffle` 的 `reversed(range(1, n))` 报
  「reversed 目前只接线了 list（tuple／str／range 一族随后补）」✓。
- **`int.to_bytes` 只接 `i64`** ✗（枢纽⑤实测）：`random.randbytes` 的
  `getrandbits(n*8).to_bytes(n, 'little')` 报 `descriptor needs an int` ✓（64 位以上的整数过不去 ✓）。
- **`tempfile.gettempdir()` 运行期** ✗（枢纽⑤实测）：`import tempfile` 已过 ✓，但调 `gettempdir()`
  报 `expected str, bytes or os.PathLike object, not int` ⇒ posix／`os.path` 面还要补 ✓。
- **`_random` 与参照 MT19937 的差异** ✗（枢纽⑤，**已在提交正文如实登记** ✓）：`getstate()` 是 **4** 元组
  （参照 625 ✓）、`seed(None)` 熵源是时钟＋计数（参照 `os.urandom` ✓）、同种子不同序列 ✓；
  **语义**对齐 ✓。
- **parser 缺「单行 `for` 体」** ✗（枢纽④实测）：`Lib/http/cookiejar.py:1760` 的
  `for cookie in self: i = i + 1` 报 `` `for` 的冒号后面要换行 `` ⇒ 该文件因此**没进 `Lib/`** ✓
  （`def f(): pass` 那条单行体**已通** ✓，缺的是 `for`／`while` 一族 ✓）。
- **编译期 cell／free 槽位不变量** ✗（枢纽④实测）：`Lib/http/server.py` 的
  `class BaseHTTPRequestHandler` 报「MAKE_CELL 要槽 0，而那一格是 Some(Local)」
  （`nlocals=2`、`cellvars=['__classdict__']` ✓）⇒ 该文件因此**没进 `Lib/`** ✓。
- **其余已知未接的小口子** ✗（均在归档卷如实记 ✓，不假装全好 ✗）：
  `functools.lru_cache` **运行期**的 `LOAD_NAME`（函数体里发 `LOAD_NAME` ✗，第 639 轮 ✓）；
  `target/recon/bisect4.py` 变体 B 仍 `StackUnderflow` ✗（第 614 轮 ✓）；
  默认 `repr` 的 **qualname** 那半仍缺 ✗（第 599 轮 ✓）；
  全角字符的**词法缺口** ✗（第 718–720 轮探针规矩 ② ✓）。

## 纪律提醒

- 红灯不提交 ✓；代码＋台账＋`NEXT.md` **同笔** ✓；闸门**只看 exit code**（不接管道 ✓）；
- 判据① **5 轮不动**即报停滞 ✓；**禁止空转**：一轮内不能只写台账 ✓；
- 判据① 单次读数 ±1 ⇒ 报"区间" ✓（第 512 轮实测 ✓）。
