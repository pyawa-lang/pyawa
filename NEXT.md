# NEXT.md —— 续做状态（单页；长复盘进台账 `docs/rounds/`）

> 规则见 `agents-rules/round-rule.md`：**一轮一落地**（每轮以「提交」或「撤回＋一条改变计划的事实」结束 ✓）。
> 第 579–720 轮的长复盘已**收口归档**进 `docs/rounds/03-m3-progress-03.md` ✓（本笔同笔提交 ✓）。

## 现在

- **HEAD**：`2dd0655`（`dev`）—— `ds-lib-sync` 的**路线 2 枢纽③**（importlib 子包）那笔 ✓（提交前已核对 `git rev-parse --short HEAD` ✓）
- **工作区**：**干净** ✓（`git status --porcelain` 为空 ✓ —— **以本笔收口提交为准** ✓；本单页随其后一笔 docs 提交落地 ✓）
- **stash**：**空** ✓
- **判据①**（`python3 tools/lib_import_ratio.py`，本笔实测 ✓）：**通过 229 ＋ 参照口径 17 ＝ 246 ÷ 628 ⇒ 39.2%** ✗（阈值 67%）
  - 上一笔 **243 ÷ 628 ⇒ 38.7%** ⇒ 本笔 **＋3** ✓（枢纽③被依赖墙挡住，见「下一条命令」✓）
  - 判据② **已达成** ✓：`target/recon/reparity.py` 的 **13 行**与参照（本机 `python3` 3.14）逐字一致 ✓
  - 进度指标（**不作判据** ✓，`CM-15`）：`Lib/` 已同步子集 **339 个文件 ⇒ 能 import 230 个 ⇒ 67.8%** ✓

## 下一条命令（路线 2 枢纽③ 被依赖墙挡住 ⇒ 先按 Lead 队列 ⑤⑥ 推）

**用户已拍板路线 2** ✓：按"**阻塞最多模块的报错签名**"队列抬判据① 分子，目标 **≥ 421/628 ＝ 67%** ✓
（`enum.py` 换回支降为**低优先** ✓，不再主攻 ✓）。**排活口径＝全分母 628** ✓。

**枢纽③本笔落地** ✓：importlib 缺的 20 个 `.py` ＋ `contextlib.py` 已同步（逐字节一致 ✓）；
`ModuleNotFoundError` 补上 `name`／`path` ✓ ⇒ `importlib.abc` 由红转绿 ✓（新增通过：
`importlib.abc`／`importlib.util`／`contextlib` ✓）。

**如实登记的阻塞事实** ✗（**改变排活** ✓）：`importlib.resources` 的**导入期**依赖链是
`pathlib`（→ `glob` 缺 ✗）＋ `tempfile`（→ `shutil`／`random`(`_random`) 缺 ✗）＋ `inspect`
（→ `dis`／`tokenize`／`token`／`annotationlib` 缺 ✗）＋ `typing`（→ `_typing` C 模块缺 ✗）；
`importlib.metadata` 另需 `json`（`json/scanner.py:5` 的 `_json` 加速器在 `try/except` 里 ✓，
但 `pathlib`／`zipfile`→`shutil`／`threading` 仍缺 ✗）＋ `typing`。
⇒ 枢纽③的 **16 个下游**要**等这些前置**，本轮解锁不了（不硬塞 ✗）。

**下一手** ✓（Lead 已排定，这里只补依赖关系）：④ `logging＋threading`（22）→ ⑤ `json＋http`（13，
**json 建议先探**：`_json` 只是可选加速器 ✓，若 json 本体能过 ⇒ `importlib.metadata` 只剩
`pathlib`／`zipfile`／`typing`）→ ⑥ `_random.Random` 支（`email.generator`＋`random`＋`tempfile`）→
⑦ `asyncio`（先探墙）→ ⑧ CJK `_codecs_*` 专支 → ⑨ `multiprocessing` → ⑩ `test.support` 殿后；
`pathlib`／`inspect`／`typing` 三块可并入 ⑤⑥ 之间，做完再回枢纽③收尾 ✓。

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
- **判据①** ✗：**246 ÷ 628 ＝ 39.2%**（阈值 67%）—— 路线 2 推进中，见「下一条命令」✓。
- **假货 `Lib/enum.py`** ✗：`python3 tools/sync_lib.py --check` 的**唯一一处**与上游不一致 ✓（`Lib/` 其余源文件逐字节相同 ✓）。
- **其余已知未接的小口子** ✗（均在归档卷如实记 ✓，不假装全好 ✗）：
  `functools.lru_cache` **运行期**的 `LOAD_NAME`（函数体里发 `LOAD_NAME` ✗，第 639 轮 ✓）；
  `target/recon/bisect4.py` 变体 B 仍 `StackUnderflow` ✗（第 614 轮 ✓）；
  默认 `repr` 的 **qualname** 那半仍缺 ✗（第 599 轮 ✓）；
  全角字符的**词法缺口** ✗（第 718–720 轮探针规矩 ② ✓）。

## 纪律提醒

- 红灯不提交 ✓；代码＋台账＋`NEXT.md` **同笔** ✓；闸门**只看 exit code**（不接管道 ✓）；
- 判据① **5 轮不动**即报停滞 ✓；**禁止空转**：一轮内不能只写台账 ✓；
- 判据① 单次读数 ±1 ⇒ 报"区间" ✓（第 512 轮实测 ✓）。
