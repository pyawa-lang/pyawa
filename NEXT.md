# NEXT（会话 13 · 优先队列收口 · 第二轮）

> **本轮落地（2026-11-14，第二轮）**：「建议优先」三项全部完成 —
> **①** `_sre` 五张进程级静态表改**每实例**（同进程多实例 `import re`/`json` 栈溢出修复；仅留纯值有界
> `ENGINE_CACHE`）· **②** 性能复核：`attribute_lookup` 的 `__text_signature__`／`__module__` 两条投影
> **下沉到类型 `getattr` 槽**，A/B 四档 **-3.9%～-10.7%**、`bench_gate` ✓ · **③** 小 bug 清账：a) `nonlocal`
> 无外层绑定⇒编译期 `SyntaxError`；b) `__slots__` 字符串槽名私有名改写；c) 嵌套/函数内类 `__qualname__`；
> d) `type.__dict__` 四个类型属性真 getset（`getattr_static` 返回描述符）；e) `newline=` **写**翻译／
> `.newlines`／`StringIO(newline=)`；f) `iter_value`/Err 分支引用泄漏（GC 收支转平）；g) `with` 的
> `__exit__` 补 `tb`（`test_with` 12F/7E⇒6F/7E）。六笔代码提交＋文档提交。**ABI 未动、未 push**。
> **本页规矩**（`agents-rules/round-rule.md` §5）：**≤ 40 行**；只写 状态／下一条命令／未修 bug＋判据。

- **HEAD**：`65e81588`（代码最后一笔；文档提交见 `git log`）· **工作区**：干净 ✓
- **下一条命令**：`tools/slowcheck.sh --release`
- **闸门**：`check.py` 19/19；`slowcheck --release` 十一项全绿（510 s）；core 81／runtime 123 套件／
  abi（含 conformance 408）全绿；上游 `test_csv` 127 例不变（4E 均为既有 `array` 缺口）。
- **下一批建议（收益排序）**：①**同族 Err 分支泄漏（已量化）**：`builtin/int.rs:31`、`builtin/set.rs::set_new`、
  `instance/convert.rs` 排序探针、`instance/accessors.rs` 惰性注解、`builtin_objects/callables.rs` 终结器；
  判据 `PYAWA_GC_LOG` 收支（N=10000/20000 缩放）②描述符 `__set__`/`__delete__`（要动类型/函数存储字段）
  ③`nonlocal` pass-1 文案族＋既有 emit bug（`def g(a=(w:=1))`＋内层 `nonlocal w` panic）
  ④symbols 既有缺口：annotation-only 外层绑定缺 cell、类 `__annotations__` 读取报错
  ⑤ABI：`#1` `type_expr` 求值（等 TS 注解解析器）、`#2` `pa_exec_file`（等 IM-15）、发布＝用户决定。
- **本轮改动边界（如实）**：`_sre` 的 `compile_raw`/`match_raw` 改 `key:str`（自有扩展）、`vars(pattern/match)`
  多隐藏键；`C.__dict__` 含私有键 `__pyawa_qualname__`（公开面一致）；`__slots__` 的 `__dict__/__weakref__`
  槽是 member_descriptor（参照 getset）；`.newlines` 整块追踪、文本 `tell` 仍字符下标。
- **口径**：改编译器/AST 后清缓存 `find target Lib -name __pyawa__ -prune -exec rm -rf {} +` ＋
  `find target -name '*.pyac' -delete`；`PA_ABI_VERSION`＝1.10、指令集 **2**；编号现最大
  `BC-67`／`AB-72`／`TS-50`／`CP-39`／`CX-29`／`P3-42`。
- **提交纪律**：`git commit -F - -- <显式路径>`；`&&` 只看 exit code；slowcheck 十一项全绿才提交。
