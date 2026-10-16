# NEXT.md —— 续做状态（单页；长复盘进台账 `docs/rounds/`）

> 规则见 `agents-rules/round-rule.md`：**一轮一落地**（每轮以「提交」或「撤回＋一条改变计划的事实」结束 ✓）。
> 第 579–720 轮的长复盘已**收口归档**进 `docs/rounds/03-m3-progress-03.md` ✓（本笔同笔提交 ✓）。

## 现在

- **HEAD**：`d42ebca`（`dev`）—— `ds-lib-sync` 的**路线 2 枢纽④**（json／http）那笔 ✓（提交前已核对 `git rev-parse --short HEAD` ✓）
- **工作区**：**干净** ✓（`git status --porcelain` 为空 ✓ —— **以本笔收口提交为准** ✓；本单页随其后一笔 docs 提交落地 ✓）
- **stash**：**空** ✓
- **判据①**（`python3 tools/lib_import_ratio.py`，本笔实测 ✓）：**通过 235 ＋ 参照口径 17 ＝ 252 ÷ 628 ⇒ 40.1%** ✗（阈值 67%）
  - 上一笔 **246 ÷ 628 ⇒ 39.2%** ⇒ 本笔 **＋6** ✓（json×4 ＋ http ＋ http.cookies ✓）
  - 判据② **已达成** ✓：`target/recon/reparity.py` 的 **13 行**与参照（本机 `python3` 3.14）逐字一致 ✓
  - 进度指标（**不作判据** ✓，`CM-15`）：`Lib/` 已同步子集 **348 个文件 ⇒ 能 import 236 个 ⇒ 67.8%** ✓

## 下一条命令（Lead 队列 ⑤：`_random.Random` ＋ tempfile）

**用户已拍板路线 2** ✓：按"**阻塞最多模块的报错签名**"队列抬判据① 分子，目标 **≥ 421/628 ＝ 67%** ✓
（`enum.py` 换回支降为**低优先** ✓，不再主攻 ✓）。**排活口径＝全分母 628** ✓。

**枢纽④本笔落地** ✓：`Lib/json/`（6）＋ `Lib/http/{__init__,client,cookies}` 已同步（逐字节一致 ✓），
新增通过 `json`／`json.decoder`／`json.encoder`／`json.scanner`／`http`／`http.cookies` ✓；
三处核心 bug 同轮修（`_sre` 的 VERBOSE 类内空白 ✓、类内 `\b`／`\a`／`\v` ✓、
`RAISE_VARARGS` 的异常载荷硬转 ✓ —— 详见本笔提交正文 ✓）。
**如实不同步** ✗：`http/cookiejar.py`（parser 缺**单行 `for` 体**）、`http/server.py`
（编译期 **cell／free 槽位**不变量不过）—— 两个都进不了 `lib_compile` 闸门 ✓ ⇒ 按纪律先不放进来 ✓
（这两个缺口本身**已立据**，见「未修 bug」✓）。

**枢纽④遗留的依赖关系** ✓：`json.tool`／`json.__main__` ⇒ 缺 `argparse`（另需 `_colorize`
→ `dataclasses` → `inspect`）；`http.client`／`http.server` ⇒ 缺 `socket`；`http.cookiejar` ⇒
缺 `urllib.request`／`threading`／`calendar`；`wsgiref.simple_server` ⇒ 缺 `http.server` ✓。

**下一手** ✓（Lead 已排定）：⑤ `_random.Random` 支 ＋ `tempfile`（＋`shutil` 视情况）→
⑥ `pathlib`＋`glob`＋`zipfile`＋`typing`＋`inspect` 簇（importlib.resources／metadata 收尾）→
⑦ `logging`＋`threading` → ⑧ `asyncio` 探墙 → ⑨ CJK `_codecs_*` 专支 → ⑩ `multiprocessing` →
⑪ `test.support` 殿后 ✓。

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
- **判据①** ✗：**252 ÷ 628 ＝ 40.1%**（阈值 67%）—— 路线 2 推进中，见「下一条命令」✓。
- **假货 `Lib/enum.py`** ✗：`python3 tools/sync_lib.py --check` 的**唯一一处**与上游不一致 ✓（`Lib/` 其余源文件逐字节相同 ✓）。
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
