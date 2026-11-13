# SPEC — 嵌入接口（C ABI）

> 规范性文件 · 状态：v0
> 读者：实现 `pyawa-abi` 的人、以及要嵌入 Pyawa 的宿主作者
> **冲突时的优先级**：`REQUIREMENTS.md`（决策）> `SPEC-*.md`（执行细则）> `DESIGN.md`（架构论证）
> 依赖：`SPEC-object-model.md`（`OM-`）、`SPEC-capabilities.md`（`CP-`，**能力接口形状的唯一归属**）

规范用语：**必须** / **禁止** / **建议** / **可选**。硬约束编号 **`AB-n`**，测试编号 **`T-AB-n`**。

---

## 1. 范围与依赖

**规定**：稳定 C ABI 的**函数面与预算**、**栈规则**、**错误码**、**宿主函数／类型注册**、
**签名元数据的格式与存放**、**版本策略**、**跨 ABI 句柄语义**。

**不规定**：

| 内容 | 归属 |
|---|---|
| 能力接口 vtable 的**形状**（有哪些函数、可否取消、如何表达"未实现"） | `SPEC-capabilities.md`（`CP-`）——本文件只写"如何注册它"（`SPEC-INDEX.md` §4） |
| 对象模型内部（头部、类型对象、引用计数） | `SPEC-object-model.md`；**头部禁止出现在本 ABI 里**（`OM-6`） |
| 宿主对象的**表示**与生命周期 | `OM-34`…`OM-37`；本文件定**注册调用** |
| 用 C 写扩展模块 | **不做**（`REQUIREMENTS.md`：只做嵌入，不做扩展） |

| 依赖 | 用途 |
|---|---|
| `OM-6` | 头部／内部布局不得进入 ABI 签名 |
| `OM-16`／`OM-17` | 引用协议与 RAII 守卫 |
| `OM-14`／`OM-34`…`OM-37` | 宿主类型注册为真实类型、`dealloc`／`traverse` |
| `CP-2`／`CP-3`／`CP-5` | "未实现"的表达方式 |
| `CP-30` | 版本字段 |
| `CX-11` | panic 不得跨 FFI |
| `PLAN-milestones.md` §6 | M1 判据（一个 ≤50 行 C 程序） |

---

## 2. 上游硬契约

**不适用。** Pyawa **不做 C-API**（`REQUIREMENTS.md` C 兼容范围），因此本 ABI **没有**来自
逐字同步 `Lib/` 的义务——它是纯自有接口。与上游有关的唯一约束是**否定的**：`OM-6`
禁止把内部布局（`PyObject` 一类）暴露进来。

---

## 3. 术语

- **实例句柄**：宿主持有的不透明指针，对应一个 `State`／`Instance`。
- **栈**：每实例的虚拟栈，宿主函数与能力函数用它收发值（Lua 风格）。
- **槽位**：栈上的一个位置。
- **宿主函数**：宿主用 C 实现、注册给脚本调用的可调用对象。
- **宿主类型**：宿主注册、脚本侧呈现为真实类型的对象类型（`OM-14`）。
- **签名**：一组类型元数据，供编译期校验与生成 `.pyi`（`DESIGN.md` §8.2）。

---

## 4. 函数面与预算

- **AB-1** 公开函数**总数必须 ≤ 120**（`§13-16` **已决**）。超出预算**必须**视为设计失败，
  而不是"功能进账"。
- **AB-2** 分层与前缀**必须**固定为：核心层 **`pa_`**、辅助层 **`paL_`**
  （与 Lua 的 `lua_`／`luaL_` 同一手法）；函数表按层分两段，宿主**可只实现核心层**。
- **AB-3** 所有导出函数**必须**是 `extern "C"`，且**禁止** panic 跨越边界（`CX-11`）。
- **AB-4** 公开接口**禁止**出现泛型／单态化（`REQUIREMENTS.md` 后果 4）。
- **AB-5** 函数面**至少**覆盖这几类，且**每一类都必须在本规格 §15 的清单里出现**：
  ① 实例生命周期（创建／销毁／中断）；② 执行（脚本／文件／字节码，**含模式参数**）；
  ③ 宿主函数注入；④ 宿主类型注册；⑤ 能力接口注册；⑥ 值转换；⑦ 错误处理；⑧ 版本查询。
- **AB-6** **禁止**导出清单之外的函数；**禁止**隐式导出（例如由宏顺带产生的符号）。
- **AB-7** 执行类函数**必须**显式接受模式（纯 Python 模式／扩展模式）**与编译档位**（`TS-31`／`IM-19`）：
  模式**无默认**、**必须**由宿主表态（`AB-60`）；档位与优化级经 **`pa_options`** 传入（`AB-61`），
  **允许传 `NULL`** 取默认——但默认**只能**是 `TS-31` 的**浅层**（深层**禁止**默认开启）。
- **AB-8** 创建实例**必须**接受宿主提供的**能力接口实现**（`DESIGN.md` §7）；未实现的域即"未提供"（`CP-5`）。

---

## 5. 栈规则（线格式）

- **AB-9** 线格式是**虚拟栈**；索引规则**必须**明确到"正索引自底、负索引自顶"这一级，
  且**必须**在头文件里写死。
- **AB-10** 栈上每个槽位**持有**一个引用（`OM-16`）；宿主**必须**按 §6 的转移规则负责归还。
- **AB-11** 每个函数的"谁 pop／谁负责"**必须**在 §15 的清单里逐函数写明——**禁止**留给约定俗成。
- **AB-12** 栈深上限**必须**存在，越界**必须**返回错误码，**禁止** UB 或崩溃。
- **AB-13** 栈**必须**与实例绑定；**禁止**用 A 实例的栈索引操作 B 实例（`OM-1` 的实例隔离）。

---

## 6. 跨 ABI 句柄语义（`SPEC-INDEX.md` §4 划归本文件的真依赖项）

- **AB-14** 值**必须**以**不透明句柄**跨 ABI；**禁止**暴露头部、类型对象指针或任何内部布局（`OM-6`）。
- **AB-15** "借用"与"转移"**必须**在函数签名上可区分，且**必须**有命名约定（如 `*_borrow`／`*_steal`）。
  **禁止**靠文档口头约定。
- **AB-16** 返回值为**新引用**（`OM-16`）；借出方**禁止**假定宿主会归还。
- **AB-17** 宿主**禁止**在"可能触发释放的调用"之后继续使用借用句柄（`OM-19` 的跨边界版本）。
- **AB-18** 实例销毁后其全部句柄**立即失效**；宿主**禁止**再使用（`CX-13` 的实例隔离）。
- **AB-70** **宿主对象载荷读回**（`AB-35`／`AB-58` 的配套）：
  `int pa_tohandle(pa_state *st, int idx, void **payload_out)`（栈契约 `—`）：
  - 栈上 `idx` 指向**宿主对象**（`pa_newtype` 注册的类型**或其 Python 子类**）⇒ `PA_OK`，
    `*payload_out` 指向 **VM 分配**的载荷（`AB-58`；注册的 `payload_size == 0` ⇒ `NULL`）；
  - **只借出、不转移**：指针有效期同 `AB-15`／`AB-17`（下一次可能触发释放的调用之前有效），
    宿主**禁止** `free`／`realloc`（载荷归 VM，`AB-58`）；
  - `idx` 越界／`payload_out == NULL`／栈上**不是宿主对象** ⇒ `PA_ERR_INVALID`（`AB-22`）；
  - **判定"是不是我的句柄"用本函数本身**：`pa_type` 对 `list`／普通实例也报 `PA_THANDLE`
    （`AB-63`），不能用它猜（那是第二个真相）。
- **AB-71** `pa_set_program_filename(st, name, len)`：**设置后续执行产物**的 `co_filename` 来源
  （CLI 在 `pa_exec_bytecode`／`pa_exec_string` 之前调用；未设置 ⇒ 内部占位名 `<pyawa-test>`）。
  - `name` 指向 `len` 个 **UTF-8** 字节（**不要求** NUL 结尾）；`len < 0`／`name == NULL`／
    非 UTF-8 ⇒ `PA_ERR_INVALID`；栈契约 `—`；
  - **按实例**保存（不影响其它实例）；成功后再执行的 `f_code.co_filename`／traceback 用这份值。
- **AB-72** `pa_call`／`pa_pcall` 的 `nresults` 是**精确结果数**（栈契约 `−nargs+nresults`）：
  - 被调用方产出 **M** 个结果（v1 的调用 **M = 1**）⇒ `nresults < M` **截断**、`> M` 用 `None`
    **补齐**、`0` **全部丢弃**（v1 的 `0` 即丢弃那唯一结果）；
  - `nargs < 0` 或 `nresults < 0` ⇒ `PA_ERR_INVALID`；
  - **v1 边界（如实）**：本层调用只产出 1 个结果 ⇒ 多值形态的"多截"当前不可观测，规则先钉住。

---

## 7. 错误处理

- **AB-19** 每个函数**必须**返回稳定的**状态码**；详细信息经实例查询，**禁止**用全局错误变量。
- **AB-20** 状态码枚举**必须**稳定且可扩展：既有取值**禁止**改变含义，新增**必须**追加到预留区。
- **AB-21** **异常绝不跨边界逃逸**（`DESIGN.md` §3 不变量 3、`CX-11`）：
  脚本异常在边界上**必须**被转成状态码 ＋ 可由宿主取回的信息。
- **AB-22** "**未实现**"与"**已实现但拒绝**"**必须**可区分（`DESIGN.md` §2、`CP-6`）：
  前者是关于"该嵌入提供了什么"的事实，后者是真实机器语义的错误。
- **AB-23** 错误信息的**生命周期必须明确**（属于实例还是属于调用）——在 §15 里逐函数写明。

---

## 8. 宿主函数注册

- **AB-24** 宿主函数**必须**经虚拟栈收发参数（`DESIGN.md` §8.1）；方法形态下 `self` 占固定槽位。
- **AB-25** 注册**必须**同时提供签名（§9）；**禁止**注册无名签名的宿主函数。
- **AB-26** 宿主函数内部的 panic **必须**被捕获并转成状态码（`AB-3`）。

---

## 9. 签名元数据的格式与存放（`SPEC-INDEX.md` §4 划归本文件的真依赖项）

- **AB-27** 签名**必须**能表达：参数名、参数类型、默认值、可变参数、仅关键字参数、返回类型、
  是否为方法、是否为静态／类方法。
- **AB-28** 签名**必须**能生成 **`.pyi` 存根**——**禁止**发明新的 IDL 文件格式（`DESIGN.md` §8.2）。
- **AB-29** 签名**必须**能被类型检查器在**跨模块**场景读到：**进程内**检查器读**注册信息的内存投影**
  （`AB-69` 的 `HostFunctionSignature`，编译期用）；**进程外**工具（LSP／独立 `check`）**可以**读
  导出的 `.pyi`（`AB-53`）——`.pyi` 是**可选路径**，不是必经依赖（`AB-31`／`AB-54`）。
- **AB-30** 签名与栈的对应关系**必须**确定：stub 从栈取参的顺序**必须**与 §5 的索引规则一致。
- **AB-31** 签名的**权威在运行时注册**（`§13-3` **已决**）：注册时**内联提供** `pa_sig`（`AB-51`），
  **运行期不读 fs**；`.pyi` 是**导出产物**，不是运行期输入。
- **AB-51** `pa_sig` **必须**是**自带尺寸**的结构体（与 `AB-43` 同一机制）：
  `pa_sig { size, flags, ret_expr, nparams, params[nparams] }`。
  `size` 让运行时按**宿主编译时的**形状读取，**禁止**假定两侧结构体等宽。
- **AB-52** `pa_param { name, type_expr, flags, default_handle }`：
  - `name`：NUL 结尾的 UTF-8；
  - `type_expr`：**注解表达式字符串**（如 `"list[int]"`、`"int | None"`），由 Pyawa **自己的
    注解解析器**求值——**禁止**发明新 IDL（`AB-28`）；
  - `flags`：位置／仅关键字／可变位置／可变关键字／有无默认值（`AB-27` 的七项能力由此表达）；
  - `default_handle`：默认值以**不透明句柄**给出（`AB-14`），无默认时为 NULL。
- **AB-53** `.pyi` **必须**能由注册信息**导出**（遍历而成）：导出是**单向投影**——运行期与
  进程内检查器**禁止**依赖 `.pyi` 文件存在（沙箱下可能无 fs）；**禁止**把导出物当第二个签名源
  （宿主手写 stub 只属**进程外消费端**，不参与权威校验，也不得反馈进运行期）。
- **AB-54** 签名的**唯一权威**是注册信息（`AB-31`）：进程内检查器读它的**内存投影**；运行期
  `inspect` 一类读**注册信息本身**（`__text_signature__` 由 `pa_sig` 投影）；进程外工具读
  **导出投影**（`.pyi`）。各投影**禁止**不一致——导出是同一份数据的**投影**，不是第二来源。
> **`AB-54` 的运行期投影规则**（实现口径，随 2026-11-14 的 `__text_signature__` 落地）：
> - 运行期 `__text_signature__` 由注册时的 `pa_sig` **投影**（不读 fs、与 `.pyi` 无关）；
> - 文本签名**不含注解**（`inspect` 的 `_signature_fromstr` 对带注解形参直接报错——注解走
>   `AB-69` 的 `pa_type_expr` 通道）；
> - 默认值只有在能还原成 `literal_eval` 常数（`None`／`bool`／`int`）时才进文本签名；
>   其它（含"`HAS_DEFAULT` 但默认值句柄不可渲染"）⇒ **整条文本签名不提供**（如实 `None`，
>   `inspect` 报"no signature"），**禁止**编一个假值；
> - 宿主函数对象带 `__call__`（委托 call 槽）与普通函数的 `__get__`（类属性绑定）⇒ 上游
>   `inspect` 的 `_signature_is_builtin` 走方法描述符分支，读到上面的 `__text_signature__`。
> 验收：`crates/pyawa-runtime/tests/abi_text_signature.rs`（端到端：真装 stdlib ＋ 真注册）。

- **AB-55** `pa_create` 的签名**必须**是 `int pa_create(const pa_host *host, pa_state **out)`：
  实例经**出参**交回（`AB-49` 的"经栈"在此不适用）。`pa_create` **不接触栈**（栈契约 `—`）。
- **AB-56** **ABI 不匹配时仍交出实例**：`pa_create` 返回 `PA_ERR_ABI`，且 `*out` 指向一个
  **诊断实例**——该实例**只有** `pa_errmsg` 与 `pa_destroy` 可用，其余调用**必须**返回 `PA_ERR_ABI`。
  理由：`AB-48` 要求错误信息**归属实例**，而版本不匹配时诊断**无处可放**（那时还没有实例）。
  宿主**必须**在 `*out != NULL` 时调用 `pa_destroy`（`AB-18`）。
- **AB-57** `pa_destroy` **释放实例本身**；`pa_state *` 随即**不可用**（**禁止**解引用或复用），
  实例内全部句柄同时失效（`AB-18`）。**禁止**把它做成"仅置失效标记而不释放"。
- **AB-58** **宿主载荷由 VM 分配**（`AB-35`／`OM-3`／`AB-18` 三者的落点）：
  - 载荷尺寸**必须**在**注册时**声明：`pa_newtype(st, name, payload_size, dealloc, traverse, sig)`
  - `pa_newhandle(st, type, void **payload_out)`（栈契约 `+1`）：`*payload_out` 指向 VM 分配的
    `payload_size` 字节，宿主**必须**在对象**对脚本可见之前**填完
  - 载荷**归 VM 所有**：随实例（`AB-18`）与类型一并回收，**由 VM 释放**；宿主**禁止** `free`／`realloc`
  - `payload_size == 0` ⇒ `*payload_out` **可以**为 `NULL`
  - **禁止**"宿主自己分配、只把指针交进来"——那样 `OM-3` 无法记账、`AB-18` 也管不到它
    （泄漏与悬垂都无人负责）；**禁止**把载荷指针另作一个不透明句柄走栈（栈上会出现两个真相）
  - **Python 子类实例**（`AB-37`）的载荷**必须**按**同一尺寸**由 VM 分配，宿主无需参与
- **AB-59** **类型对象经栈上的句柄传递**（补 `pa_newhandle` 的 `type` 参数域，也是 `AB-14` 的落点）：
  - `pa_newtype` 的栈契约由 `—` 改为 **`+1`**：注册成功后**必须**把**类型对象**压栈
    （不透明句柄，`AB-14`）。**禁止**回传类型指针；**禁止**另立"注册序号"这类第二套标识
  - `pa_newhandle(st, type, void **payload_out)` 的 `type` **是栈索引**（`AB-9`：正索引自底、
    负索引自顶），指向一个**类型对象**句柄；该槽**不消耗**（宿主负责 pop，`AB-11`），
    函数再压入新实例（净 `+1`）
  - 于是"**压一次类型、建多个实例**"可行，且"注册了却拿不到类型"的缺口被堵住
- **AB-60** **`pa_exec_*` 的 `mode` 取值域**（把 `AB-7` 的"显式必填、无默认"落到具体取值上）：
  - 取值**只有两个串**：**`"python"`**（`IM-1` 的纯 Python 模式——`.py` 那一档，CPython 3.14 语义的
    子集）与 **`"pyawa"`**（`IM-1` 的扩展模式——Pyawa 的**完整形态**）。
  - **大小写敏感、全串匹配、不接受别名**：`"py"`／`"Python"`／`".py"`／`".pyawa"` **一律不合法**。
  - 空串、`NULL` 与任何其他值 ⇒ **`PA_ERR_INVALID`(6)**；**禁止**从路径后缀或来源内容**推断**模式（`AB-7`）。
  - 这两个串是 **`mode` 参数的两个取值**，**不表示两个方言**——层次仍按 `DESIGN.md` §2.1：
    **Pyawa 是唯一的方言**，`.py`／`.pyawa` 是**同一方言内的两种模式**（子集关系 `.py ⊂ .pyawa`）。
  - 错误码分工：**`mode` 不合法 ⇒ `6`**；**源码本身解析失败 ⇒ `PA_ERR_SYNTAX`(2)**。
    宿主据此可分辨"**我传错了参数**"与"**脚本自己有问题**"。
- **AB-61** **编译输入经 `pa_options` 过界**（`AB-7` 的"档位"子句与**优化级**的落地形状）：
  - `pa_options` **必须**是**尺寸标记**结构（首字段 `size`，与 `AB-43`／`AB-51` 同一惯例）
    ⇒ 以后追加字段**不改签名**（`AB-44` 的追加式）。
  - 至少两个字段：**检查档位**（`TS-31`；取值至少含 浅层／深层）与**优化级**（`IM-19`）。
  - `pa_exec_string`／`pa_exec_file` **必须**接受 `const pa_options *`，且**允许 `NULL`**——
    `NULL` ＝ 档位取 `TS-31` 的**默认（浅层）**、优化级取默认。
  - 宿主给出的档位与优化级**必须**进入产物的决定要素（`IM-21`）与 `.pyac` 头部（`IM-19`）。
  - **`pa_exec_bytecode` 不接受 `pa_options`**：模式／优化级／档位**三样都在产物头部**
    （`IM-19`）——宿主另行指定会造出**两个真相**。
- **AB-62** **整数的字符串桥**（整数是**唯一**需要它的一类：值域**无界**，而 `pa_tointeger` 只有 `i64`）：
  - **取出** `pa_tointstring(st, idx, len*)`：把**任意整数**渲染成**十进制**，给**只读借用视图**
    （同 `pa_tostring` 的借用约定，`AB-15`）。**非整数** ⇒ `PA_ERR_INVALID`。
  - **送进** `pa_pushintstring(st, s, len)`：解析**十进制**串并压栈（`+1`）；`len == -1` 表示
    NUL 结尾（与 `pa_exec_string` 的 `len` 约定一致）。**解析失败** ⇒ `PA_ERR_INVALID`。
  - **两条桥的语义 ＝ 参照的 `str(int)` 与 `int(s)`**（一处真相）：前导 `+`／`-`、前后空白、
    下划线的接受与否**照参照**；位数上限**必须**照 `TS-45` 的 **4300**（超限 ⇒ `ValueError`，
    消息以探测为准）。
  - **与 `pa_tointeger` 的分工**：`pa_tointeger` ＝"我要 `i64`"，越界**必须如实失败**
    （**禁止**静默截断）；**整数桥覆盖全部整数**（`i64` 内的也走它）⇒ 宿主有一条**统一**路径，
    不必"失败后再回落"。
  - **`bytes` 不适用本桥**：二进制**不能**无损穿过十进制文本 ⇒ 它用**既有**的 `pa_pushbytes`
    （**复制**）与 `pa_tobytes`（**借用**视图），**不新增**函数（`§15` 已登记，落地即可）。
  - **禁止**把本桥推广成"任意值的通用文本桥"：目前**只有整数**需要它；别的类型**需要时**再按
    同一手法加对应的桥（**不做推测性设计**，与 `CM-10` 的"禁止预先写清单"同理）。
- **AB-63** **`pa_type` 的标签取值域**（宿主判定"这是什么"的**唯一**机制）：
  - 取值**必须**是**末尾追加**的整数枚举：`PA_TNIL=0`、`PA_TBOOLEAN=1`、`PA_TINTEGER=2`、
    `PA_TNUMBER=3`、`PA_TSTRING=4`、`PA_TTABLE=5`、`PA_TFUNCTION=6`、`PA_THANDLE=7`、
    **`PA_TBYTES=8`**。**禁止**改动已有编号；新增类型**只能**取下一个值（`AB-44` 的追加式）。
  - **判类型用 `pa_type` 与 `pa_is*`；取值用 `pa_to*`**——**禁止**用"某个 `pa_to*` 是否成功"
    间接判定类型（那是第二个真相；`bytearray`／`memoryview` 一类进来后还会**歧义**）。
  - **`bytes` 必须**报 `PA_TBYTES`（此前无标签，宿主只能靠"`pa_tobytes` 非 NULL"猜——已禁止）。

---

## 10. 能力接口注册

- **AB-32** 本 ABI **只提供注册入口**；接口形状一律引 `CP-`（`SPEC-INDEX.md` §4）。
- **AB-33** 注册**必须**按**能力域**进行；未注册的域即"未提供"（`CP-5`）。
- **AB-34** 注册**必须**携带异步分类声明（`§13-4` 已决的取值：**`PA_ASYNC_OK`**（可异步化）／
  **`PA_ASYNC_NO`**（不可异步化），**按域二值**）；**缺失即注册失败**，**禁止**落默认值
  （`CP-25`、`CP-37`）。

---

## 11. 宿主类型注册

- **AB-35** 宿主类型**必须**注册为**真实类型**（`OM-14`）；**禁止**另立一套对象表示。
- **AB-36** 注册**必须**提供 `dealloc` 与 `traverse`（`OM-34`／`OM-36`）、**载荷尺寸**（`AB-58`）
  以及 §9 的签名。
- **AB-37** 宿主类型**可被脚本继承**（`§13-1` **已决**）：`pa_newtype` 注册的类型
  **默认可作基类**。因此 vtable **必须**含**子类分派槽位**——`tp_new`／`tp_dealloc`／`tp_traverse`
  得能被 Python 覆写；且注册时**必须**能声明该类型是否需要**实例字典**
  （宿主对象布局固定，字典另行挂载）。
  `pa_sig.flags` 里**保留** `PA_TYPE_FINAL` 位：宿主**可以**用它**反向**选择"本类型不可继承"
  （与 CPython 的 `Py_TPFLAGS_BASETYPE` 反向等价）；**未设该位即允许继承**。
  **载荷尺寸在此声明**（`AB-58`）：子类实例的载荷按**同一尺寸**由 VM 分配。
- **AB-38** 实例**必须**携带 ABI 版本标识，`create` 时与宿主期望版本比对（`CP-30`）。

---

## 12. 版本策略（机制：尺寸 ＋ 追加式函数表）

- **AB-39** 三份契约——**嵌入 API ＋ 能力接口 ＋ 宿主对象契约**——**共用一个版本号**；
  它就是 `CP-30` 的版本字段，也是 `AB-38` 在 `create` 时校验的对象。
- **AB-40** 版本不兼容时 `create` **必须**失败，并给出**可诊断**的版本信息；**禁止**静默降级。
- **AB-41** 「允许的改动」矩阵**必须**如下，且**必须**可被宿主**静态**检查到：

  | 改动 | 版本 | 旧宿主 |
  |---|---|---|
  | 末尾**追加**函数 | **次版本 +1** | 仍可用 |
  | 改既有槽位语义／删函数／改结构体字段形状 | **主版本 +1** | `create` **必须**失败 |

- **AB-43** 宿主在 `create` 时传入 `(abi_size, abi_version)`：`abi_size` 是**宿主编译时**的
  函数表字节数。运行时**必须**以 `min(宿主 size, 自身 size)` 为界读取，**禁止**越界读。
- **AB-44** 函数**只能追加在表尾**；**禁止**改动既有槽位语义，**禁止**改变既有函数的参数或返回值形状。

> **当前版本：1.10**（`PA_ABI_MAJOR` 仍 **1**／`PA_ABI_MINOR` **10**）——`AB-41` 的追加式改动
> 只升次版本、**旧宿主仍可用**（`AB-43` 的界读：读不到新函数／新槽 ⇒ "未实现"，`CP-3`）。
> 逐版追加历史见 `pa.h` 头注释的版本历史（**一处真相** ✓；含 `fs` 的 `stat_times`／`pipe`、
> `clock` 的 `localtime` 一族、`AB-63` 类型标签、`AB-64` 并发契约、`AB-65`／`AB-66` 配额、
> `AB-67` 宿主模块四函数、`AB-68` `pa_setpath`、`AB-69` 签名句柄、`AB-70` `pa_tohandle`／
> `AB-71` `pa_set_program_filename`）。`CP-30` 的注册校验口径**不变**：公共头的 version／size
> 规则不动（vtable 的形状归 `CP-`，本文件只管"如何注册"）。

- **AB-45** 头文件**必须**是**单一** `pa.h`，并提供宏 `PA_ABI_VERSION` 与 `PA_ABI_SIZE`；
  **禁止**分层头。
- **AB-46** `pa.h` **必须**能被 **C 与 C++** 同时包含：**必须**有 `extern "C"` 守卫
  （`#ifdef __cplusplus`），且版本宏在 C 下也可用——**禁止**让 `PA_ABI_VERSION`／`PA_ABI_SIZE`
  依赖 C++ 特性。
- ~~AB-42~~ **作废**：其内容（"禁止在本规格里先定具体方案"）随 `§13-2` 裁决失效；编号保留，不重用。

---

## 13. 验收（可测性质）

| 编号 | 测试 |
|---|---|
| `T-AB-1` | 一个 **≤50 行**的 C 程序完成 `PLAN-milestones.md` §6 的 M1 判据（且该程序**必须**入库，`MS-21`） |
| `T-AB-2` | 宿主函数故意 panic：被捕获、返回状态码、进程不崩（`AB-3`／`AB-26`） |
| `T-AB-3` | "未实现"与"已实现但拒绝"返回**可区分**的结果（`AB-22`） |
| `T-AB-4` | 版本不匹配时 `create` 失败且信息可诊断（`AB-40`） |
| `T-AB-5` | 静态统计导出符号数 ≤ 预算（`AB-1`）；机检 `T-CX-19`（`CX-29`）：导出 ⊆ `pa.h` ⊆ 规格提及 ＋ 总数 ≤ 120 |
| `T-AB-6` | 未声明异步分类的能力域注册失败，不落默认值（`AB-34`） |
| `T-AB-7` | 实例销毁后使用其句柄不导致 UB（`AB-18`） |

---

## 14. 未决（引用 `DESIGN.md` §13）

**本规格当前无未决项。** 原先列出的各项（`§13-1`／`§13-2`／`§13-3`／`§13-4`／`§13-13`／
`§13-16`／`§13-17`）**均已决**，见 `DESIGN.md` §13 的"已决"。

---

## 15. 函数清单与状态码

> 本表**就是** `AB-6` 说的权威清单：**禁止**导出表外函数。一函数一行，便于 `T-AB-5` 计数。

### 15.1 通用约定

- **AB-47** **栈契约记号**（`AB-11` 要求的逐函数说明以此列表示）：
  `+n` ＝ 压入 n 项；`−n` ＝ 弹出 n 项；`±n` ＝ 就地替换 n 项；`—` ＝ 不动栈。
- **AB-48** **错误信息生命周期**（`AB-23`）：错误信息**归属实例**，**必须**保留到下一次可能改写它的
  调用；`pa_errmsg` 返回**借用**句柄（`AB-15`），宿主**禁止**在后续 API 调用之后继续使用它。
  下表不再逐行重复这一约定（它只在本行定义一次）。
- **AB-49** 每个函数**必须**返回状态码（`AB-19`）；**返回值不经状态码传递**，它经栈传递。
  **唯一例外**：`pa_create` 经**出参**交回实例（`AB-55`）——创建那一刻**还没有栈**
  （`AB-13` 把栈绑在实例上）。
- **AB-50** 函数总数**必须** ≤ 120（`AB-1`）。当前 §15.3＋§15.4 共列出 **93** 个（核心 **75** ＋辅助 18），
  余 **27** 个预算；新增**必须**从表尾追加（`AB-44`）。

### 15.2 状态码枚举（`AB-20`：既有取值**禁止**改变含义，新增**必须**追加到预留区）

| 取值 | 名字 | 含义 |
|---|---|---|
| 0 | `PA_OK` | 成功 |
| 1 | `PA_ERR_RUNTIME` | 脚本异常；信息经 `pa_errmsg` 取回（`AB-21`） |
| 2 | `PA_ERR_SYNTAX` | 编译期错误 |
| 3 | `PA_ERR_MEMORY` | 分配失败 |
| 4 | `PA_ERR_INTERRUPT` | 被 `pa_interrupt` 中断 |
| 5 | `PA_ERR_NOTIMPLEMENTED` | 该嵌入**未提供**所要求的能力槽位（`CP-5`）——**必须**与"已实现但拒绝"区分（`AB-22`） |
| 6 | `PA_ERR_INVALID` | 宿主用法错误（栈越界、类型不符、句柄失效、`.pyac` 版本不符等） |
| 7 | `PA_ERR_ABI` | ABI 版本或尺寸不兼容（`AB-40`） |
| 8–31 | **预留** | 新增状态码**必须**落在此区，且**禁止**在既有取值之前插入 |

### 15.3 核心层 `pa_`（75 个；其中 9 个 `pa_type_expr_*` 构造的签名见 `AB-69`）

| 函数 | 栈契约 | 说明 |
|---|---|---|
| `pa_version()` | — | Pyawa 版本字符串（静态） |
| `pa_abi_version()` | — | ABI 版本号（`AB-45` 的 `PA_ABI_VERSION`） |
| `pa_abi_size()` | — | 函数表字节数（`AB-45` 的 `PA_ABI_SIZE`） |
| `pa_create(const pa_host *host, pa_state **out)` | — | 创建实例，经**出参**交回（`AB-55`）。`pa_host` 含**能力接口实现**（`AB-8`）与 `(abi_size, abi_version)`（`AB-43`）；**ABI 不匹配**时返回 `PA_ERR_ABI` 并交出**诊断实例**（`AB-56`） |
| `pa_destroy(pa_state *)` | — | **释放实例本身**（`AB-57`）；此后全部句柄失效（`AB-18`） |
| `pa_interrupt(pa_state *)` | — | 请求中断；执行类函数随即返回 `PA_ERR_INTERRUPT` |
| `pa_exec_string(st, src, len, chunkname, mode, const pa_options *opts)` | — | 执行字符串。`mode` **显式必填、无默认**（`AB-7`／`AB-60`）；`opts` **可传 `NULL`**（＝浅层 ＋ 默认优化级），见 `AB-61` |
| `pa_exec_file(st, path, mode, const pa_options *opts)` | — | 执行文件；I/O 经能力层（`IM-15`）。`mode` 见 `AB-60`；`opts` 见 `AB-61` |
| `pa_exec_bytecode(st, buf, len)` | — | 执行 `.pyac`；指令集版本不符返 `PA_ERR_INVALID`（`BC-29`）。**`mode`／优化级／档位三样都不作参数**——都在产物头部（`IM-19`），宿主**不得**另行指定（否则两个真相，`AB-61`） |
| `pa_set_program_filename(st, name, len)` | — | 设置后续执行的 `co_filename`（`AB-71`；按实例；未设置 ⇒ 内部占位名；非 UTF-8 ⇒ `PA_ERR_INVALID`） |
| `pa_set_memory_limit(st, limit)` | — | 每实例内存配额（`AB-65`／`OM-41`）；`0` ＝ 取消。超限 ⇒ 脚本内可捕获的 `MemoryError` |
| `pa_memory_used(st, u64 *out)` | — | 读取当前占用字节（`AB-66`）；`out == NULL` ⇒ `PA_ERR_INVALID` |
| `pa_setpath(st, const char *const *dirs, size_t n)` | — | 设每实例 `sys.path`（`AB-68`）；`dirs == NULL` 或 `n == 0` ⇒ 清空。路径可读性仍由 `fs` 能力域决定（`CP-21`）|

> **`AB-7` 的档位／优化级子句：已满足** ✓（`AB-61` 的 `pa_options`，`P1-13` 已落地）。
> `pa_exec_string`／`pa_exec_file` 收 `const pa_options *`（`NULL` ⇒ 浅层 ＋ 默认优化级），
> 宿主给的**档位真的改变发射**——深层按 `BC-25`② 发边界检查，验收在
> `crates/pyawa-abi/tests/abi.rs`。**优化级**同样是显式编译输入（`IM-19`／`IM-21`），但本层
> **还没有优化器** ⇒ 取值目前**不改变发射**（口径写在 `compile` 的文档里）。
> `.pyac` 头部早已承载档位与优化级（`IM-19`）；"从源码到产物"的驱动链归 `P3-12`。

| `pa_gettop(st)` | — | 当前栈深 |
| `pa_settop(st, n)` | ± | 设置栈深；越界返 `PA_ERR_INVALID`，**禁止** UB（`AB-12`） |
| `pa_pushvalue(st, idx)` | +1 | 压入栈上某项的副本（持有一个引用，`AB-10`） |
| `pa_pop(st, n)` | −n | 弹出并释放（`AB-10`／`OM-20`） |
| `pa_type(st, idx)` | — | 类型标签；取值域与追加规则见 `AB-63`（**判类型用它／`pa_is*`，取值用 `pa_to*`**） |
| `pa_isnil(st, idx)` | — | 类型判定 |
| `pa_isboolean(st, idx)` | — | 同上 |
| `pa_isinteger(st, idx)` | — | 同上 |
| `pa_isnumber(st, idx)` | — | 同上 |
| `pa_isstring(st, idx)` | — | 同上 |
| `pa_istable(st, idx)` | — | 同上 |
| `pa_isfunction(st, idx)` | — | 同上 |
| `pa_pushnil(st)` | +1 | 压入 `None` |
| `pa_pushboolean(st, b)` | +1 | 压入布尔 |
| `pa_pushinteger(st, i)` | +1 | 压入整数 |
| `pa_pushnumber(st, d)` | +1 | 压入浮点 |
| `pa_pushstring(st, s, len)` | +1 | 压入字符串（**复制**语义） |
| `pa_pushbytes(st, p, len)` | +1 | 压入字节串（**复制**语义） |
| `pa_pushintstring(st, s, len)` | +1 | 从**十进制**串构造整数并压入（`AB-62`）；`len == -1` 表示 NUL 结尾；**解析失败 ⇒ `PA_ERR_INVALID`** |
| `pa_pushhandle(st, h)` | +1 | 压入已有对象句柄（不透明，`AB-14`） |
| `pa_newhandle(st, type, void **payload_out)` | +1 | 新建宿主对象句柄，**交 VM 记账**（`OM-3`）；载荷由 **VM 分配**并经出参交回（`AB-58`）。`type` 是**栈索引**，指向类型对象句柄，**不消耗**（`AB-59`） |
| `pa_toboolean(st, idx)` | — | 真值转换 |
| `pa_tointeger(st, idx)` | — | 整数转换；失败返 `PA_ERR_INVALID`。**越 `i64` 必须如实失败**（禁止截断）——任意精度走 `pa_tointstring`（`AB-62`） |
| `pa_tointstring(st, idx, len*)` | — | 取**任意整数**的十进制只读**借用**视图（`AB-62`）；**非整数 ⇒ `PA_ERR_INVALID`** |
| `pa_tonumber(st, idx)` | — | 浮点转换；失败返 `PA_ERR_INVALID` |
| `pa_tostring(st, idx, len*)` | — | 取只读视图（**借用**，`AB-15`） |
| `pa_tobytes(st, idx, len*)` | — | 取只读字节视图（**借用**） |
| `pa_tohandle(st, idx, payload_out)` | — | **宿主对象**载荷读回（`AB-70`；借用、仅宿主对象；NULL／类型不符 ⇒ `PA_ERR_INVALID`） |
| `pa_newtable(st)` | +1 | 新建表 |
| `pa_newlist(st, n)` | +1 | 新建长度 n 的列表 |
| `pa_getfield(st, idx, name)` | ±1 | 属性访问；走 `getattr` 槽位（`TS` §8） |
| `pa_setfield(st, idx, name)` | ±1 | 属性写入；走 `setattr` 槽位 |
| `pa_gettable(st, idx)` | ±1 | 下标访问；走 `BINARY_OP`＋`NB_SUBSCR` 语义（`BC-39`） |
| `pa_settable(st, idx)` | ±1 | 下标写入；走 `STORE_SUBSCR` 语义 |
| `pa_rawget(st, idx)` | ±1 | 下标访问但**不触发**槽位 |
| `pa_rawset(st, idx)` | ±1 | 下标写入但**不触发**槽位 |
| `pa_getglobal(st, name)` | +1 | 读模块全局 |
| `pa_setglobal(st, name)` | −1 | 写模块全局 |
| `pa_call(st, nargs, nresults)` | −nargs+nresults | 调用；异常经状态码 ＋ `pa_errmsg`；`nresults` 精确（`0`/`N`，多截少补 `None`，`AB-72`） |
| `pa_pcall(st, nargs, nresults)` | −nargs+nresults | 受保护调用（语义同 `pa_call`，显式区分调用点；`AB-72`） |
| `pa_errmsg(st)` | — | 取错误信息（**借用**，`AB-48`） |
| `pa_error(st, msg)` | — | 宿主主动抛错 |
| `pa_register(st, name, fn, sig)` | — | 注入宿主函数（`AB-24`／`AB-25`）；`sig` 形态见 §15.5 |
| `pa_add_host_module(st, name, pa_module *out)` | — | 建宿主模块（`AB-67`）；名字须简单标识符、不得是保留名／重名 ⇒ `PA_ERR_INVALID` |
| `pa_module_addfunction(st, module, name, fn, const pa_sig *sig)` | — | 往宿主模块装函数（`AB-67`）；签名必须同时给（`AB-25`／`AB-51`） |
| `pa_module_setconst(st, module, name)` | — | 往宿主模块装常量（`AB-67`）；值取自**栈顶**并消费 |
| `pa_module_addsubmodule(st, parent, name, pa_module *out)` | — | 在宿主模块下建子模块（`AB-67`）；登记 `sys.modules["<parent>.<name>"]` 与父属性 |
| `pa_type_expr_*`（9 个构造：`runtime_tag`／`named`／`list_of`／`dict_of`／`optional`／`union`／`callable`／`var`／`apply`） | — | 建 `AB-69` 的类型表达式句柄（每实例、不透明）；失败 ⇒ `NULL` ＋ `pa_errmsg` |
| `pa_func_set_signature(st, fn, params, n, ret, tparams, nt, flags)` | — | 给已注册宿主函数声明签名（`AB-69`）；注册后、任何执行前；不符 ⇒ `PA_ERR_INVALID`；声明后边界核参数／返回（脚本侧 `TypeError`，归责插件） |
| `pa_newtype(st, name, payload_size, dealloc, traverse, sig)` | +1 | 注册宿主类型（`AB-35`／`AB-36`）；声明**载荷尺寸**（`AB-58`）；**压入类型对象**句柄（`AB-59`）；**默认可被继承**（`AB-37`），故**必须**提供子类分派槽位与实例字典声明 |
| `pa_setcapability(st, domain, impl)` | — | 注册能力域实现（`AB-32`／`AB-33`） |
| `pa_setcapability_async(st, domain, cls)` | — | 异步分类声明；**缺失即注册失败**（`AB-34`） |
| `pa_retain(st, idx)` | — | 借用 → 持有（`AB-15`） |
| `pa_release(st, idx)` | — | 持有 → 释放 |

### 15.4 辅助层 `paL_`（18 个，**不含**新语义）

| 函数 | 说明 |
|---|---|
| `paL_newstate()` | `pa_create` ＋ 真实机器 provider 的便捷入口（实现属 `pyawa-runtime`） |
| `paL_openlibs(st)` | 打开标准库 |
| `paL_dostring(st, s, mode)` | 执行字符串并统一收尾 |
| `paL_dofile(st, path, mode)` | 执行文件并统一收尾 |
| `paL_setfuncs(st, const pa_reg *, n)` | 批量注册 |
| `paL_checkinteger(st, idx)` | 参数校验：不符即抛错 |
| `paL_optinteger(st, idx, def)` | 同上，缺省可给 |
| `paL_checkstring(st, idx)` | 同上 |
| `paL_optstring(st, idx, def)` | 同上 |
| `paL_len(st, idx)` | 长度 |
| `paL_getsubtable(st, idx, name)` | 取或建子表 |
| `paL_ref(st, idx)` | 把栈项存入注册表，返回整数键 |
| `paL_unref(st, ref)` | 释放注册表键 |
| `paL_traceback(st, msg)` | 附加 traceback |
| `paL_where(st, level)` | 当前位置串 |
| `paL_error(st, fmt, ...)` | 格式化抛错 |
| `paL_execresult(st, status)` | 状态码 → 统一收尾 |
| `paL_requiref(st, name, openf, glb)` | 取或加载模块 |

> **禁止**靠辅助层绕过任何硬约束——辅助层**不得**引入核心层没有的语义（`AB-4`／`AB-6`）。

### 15.5 签名的提供与导出（`§13-3` **已决**）

- `pa_register`／`pa_newtype` 的 `sig` 参数**必须**是 `pa_sig`（`AB-51`／`AB-52`）；
  **运行期不读 fs**，`.pyi` 是**导出产物**（`AB-53`／`AB-54`）。
- 覆盖率报告（`TS-38`）**同样**是可导出产物，权威数据在编译产物内。

---

- **AB-64** **实例的并发边界**（宿主契约；与不变量 2／4 配套）：
  - **同一实例禁止并发进入**：一个实例**同一时刻只允许一个宿主线程**在执行（`pa_exec*`／`pa_call*`
    以及任何触碰该实例 VM 状态的入口）；宿主**必须**自行串行化（`lua_State` 的同款约定）。
  - **实例之间可以并行**：不同实例之间**没有**共享的锁或全局状态 ⇒ 两个宿主线程各跑一个实例**允许**。
  - **禁止跨实例传递 VM 对象与句柄**（`OM-1`／`AB-18`）：句柄**必须**只在其所属实例上使用。
  - **锁的粒度是"每实例一把"**（**不是**进程级 GIL）：脚本级 `threading` 线程在同一实例内**串行**。
  - **阻塞的能力调用必须让出实例**：`DESIGN.md` §7.2 的"可异步化"一类（文件／socket／时钟／随机／进程）
    送 worker 线程，**等待期间必须**允许同实例的其它线程推进（否则该实例会被一次阻塞 I/O 拖死）。

- **AB-65** **每实例内存配额**：`pa_set_memory_limit(st, limit)`（`limit == 0` ⇒ 取消）。
  超限时脚本里抛**可捕获的** `MemoryError`（执行器在指令边界派发；**不是** abort／panic ✗）；
  抛错后水位抬高一档 `slack = max(64 KiB, limit/16)`（处理块还能分配），继续增长则每 `slack` 字节
  再抛一次 ⇒ 宿主占用上界 ≈ `limit + slack` 量级。记账口径与已知不精确见 `OM-41`。
  配额是**宿主侧钩子**（ABI-adjacent）⇒ 与 ABI 发布前的钩子族一起定 ✓。
- **AB-66** **读取当前占用**：`pa_memory_used(st, u64 *out)`（`out == NULL` ⇒ `PA_ERR_INVALID`）。
  宿主用它做指标／看门狗；数值与 `OM-41` 的记账口径一致。

- **AB-67** **宿主模块注入**（宿主库不是 Pyawa 写的时的正解 ✓）：宿主用 `pa_add_host_module(st, name)` 得到一个**模块句柄**，再用原子 API 往里装 **宿主函数**（复用 `AB-24` 的虚拟栈注册 ✓）、**常量**、**子模块**（树形命名空间 ✓）。约束：
  - **按实例**注册 ✓（与 `AB-64` 的并发边界同用）；**guest 不可调用**这些 API ✓（只有宿主调 ✓）；
  - 装进去的宿主函数参数／返回仍是**裸数据 ＋ 不透明句柄** ✓（`CP-10`）——宿主库内部状态对脚本**不可见** ✓，沙箱边界不破 ✓；
  - 模块对象就是普通 Python 模块（`__name__`／`sys.modules`／`from X import y` 一致 ✓）；
  - **保留名不得覆盖** ✗（`sys`／`builtins`／`io` 等内置模块名 ⇒ `PA_ERR_INVALID`）；
  - 插件侧**零 Pyawa 胶水** ✓：`import myapp` 直接拿到宿主的东西 ✓。
  **生命周期**：注入发生在**实例 setup 阶段**（`AB-65` 之后、**任何脚本执行之前** ✓）——插件的 `import myapp` 可能出现在第一行，模块必须先进 `sys.modules`；
  - **两次宿主调用之间**可以继续往模块里加函数 ✓（模块对象是**活的** ✓，已 import 的同一对象对后续调用可见 ✓）；
  - **宿主函数回调内部**（VM 正在执行）**禁止**改模块结构 ✗（属 `AB-64` 的并发进入 ＋ 状态改动，只为可查性保留 setup 阶段这一个写窗口 ✓）；
  - **签名与函数同一时刻给** ✗（见 `AB-69`），禁止先注入后补签名。
  **API 形状（v1，round 271 落地 ✓）**：

  ```
  typedef struct pa_module_s *pa_module;   /* 每实例不透明句柄；pa_destroy 后失效（AB-18） */
  int pa_add_host_module(pa_state *st, const char *name, pa_module *out);
  int pa_module_addfunction(pa_state *st, pa_module module, const char *name,
                            pa_host_fn fn, const pa_sig *sig);   /* 签名必须同时给（AB-25/51） */
  int pa_module_setconst(pa_state *st, pa_module module, const char *name); /* 值取自栈顶，消费 */
  int pa_module_addsubmodule(pa_state *st, pa_module parent, const char *name, pa_module *out);
  ```

  约束细化：模块名必须是**简单标识符**（点分名走子模块 API ✗）；保留名＝静态名单
  （`sys`／`builtins`／`_io`／`io`／`importlib`／`_imp`）**＋**本实例 `sys.modules` 里已有的名字，
  重复名 ⇒ `PA_ERR_INVALID`；坏句柄／空出参／**回调期间** ⇒ `PA_ERR_INVALID`（写 `pa_errmsg`，
  不 UB ✓）；`pa_module_setconst` 的值可以是裸数据或不透明句柄 ✓。
  验收：`crates/pyawa-abi/tests/host_module.rs`（8 例：三种 import 形态／保留与非法名／两实例隔离／
  guest 无 `pa_*`／两次调用之间可加／宿主错误与 panic 不穿 FFI／回调内注册返错误码／不透明句柄往返）。
  **状态**：**已实现 ✓（round 271）**；`AB-69` 的 `pa_type_expr` 签名通道已随 round 272–273 落地 ✓（检查期投影见 `d4847368`）。
- **AB-68** **宿主设置每实例 `sys.path`**：`pa_setpath(st, const char *const *dirs, size_t n)`（`dirs == NULL` 或 `n == 0` ⇒ 清空）。语义**完全**沿用 `SPEC-imports-and-modes` 的 `IM-*`（**禁止**另立一套路径逻辑 ✗，禁令照该规格 ✓）；实现上就是把 `sys.path` 换成本实例的这份列表（**拷贝**宿主缓冲区 ✓，宿主返回后可释放 ✓）。约束：**按实例** ✓；路径访问仍受 **`fs` 能力域**夹根 ✓（`CP-21`／`AB-32`）——`sys.path` 只决定**在哪找** ✓，**能不能读**由能力面决定 ✓；脚本自己改 `sys.path` 的语义不变 ✓（`sys.path` 本就可变 ✓）。
  **实现 ✓（round 270）**：`pa_setpath(st, dirs, n)` 已接线；`pa_create` 的实例带**最小 `sys` 骨架**
  （`sys.path` 空表 ＋ `sys.modules`；`paL_openlibs` 仍如实"未提供"，P3-14）。验收
  `crates/pyawa-abi/tests/host_path.rs`（5 例：两目录可 import／脚本改 `sys.path` 语义不变／
  `fs` 夹根后越根模块 `ImportError`／两实例路径隔离／参数边界与清空）。`pa.h` 已声明；次版本 6→7。
- **AB-69** **宿主函数的类型签名**（`AB-67` 配套；卖点①「类型检查真正生效」的边界延伸 ✓）：
  宿主注入函数时**一并**声明签名。**类型表达式用不透明句柄**（不是扁平标签 ✗）：

  ```
  typedef struct pa_type_expr_s *pa_type_expr;              /* 每实例不透明句柄；pa_destroy 后失效（AB-18） */
  pa_type_expr pa_type_expr_runtime_tag(pa_state *st, int tag);       /* AB-63 的取值域 */
  pa_type_expr pa_type_expr_named(pa_state *st, const char *name);    /* 具名／宿主注册类型（同一性＝类型对象） */
  pa_type_expr pa_type_expr_list_of(pa_state *st, pa_type_expr elem);
  pa_type_expr pa_type_expr_dict_of(pa_state *st, pa_type_expr k, pa_type_expr v);
  pa_type_expr pa_type_expr_optional(pa_state *st, pa_type_expr t);          /* T? */
  pa_type_expr pa_type_expr_union(pa_state *st, const pa_type_expr *ts, size_t n);
  pa_type_expr pa_type_expr_callable(pa_state *st, const pa_type_expr *params, size_t n, pa_type_expr ret);
  pa_type_expr pa_type_expr_var(pa_state *st, const char *name);      /* 类型参数 T（配 tparams） */
  pa_type_expr pa_type_expr_apply(pa_state *st, pa_type_expr base, const pa_type_expr *args, size_t n); /* 宿主泛型 Result[T] */
  int pa_func_set_signature(pa_state *st, pa_host_fn f, const pa_type_expr *params, size_t n,
                            pa_type_expr ret, const pa_type_expr *tparams, size_t ntparams, unsigned flags);
  ```

  **命名**：句柄叫 **`pa_type_expr`**（**不是** `pa_ty`）：`pa_type` 已是 `AB-63` 的运行时标签函数
  （C 的 typedef 与函数同属一个标识符命名空间）⇒ 签名里的**类型表达式**用全名区分；
  **`pa_type_expr` ≠ `pa_type()` 的返回值**（前者是签名元数据，后者是运行时值标签）。

  **与用户草案的两处必要修正**（`ABI` 未发布 ⇒ 现在定最便宜 ✓）：
  - `pa_type_expr` 的构造与 `pa_func_set_signature` 都**收 `pa_state *`**：句柄与签名是**按实例**存的
    （`AB-64`），无 `state` 版无法满足；
  - `pa_func_set_signature` **返回 `int` 状态码**（`AB-19`），不返回 `void`；`pa_type_expr_*` 构造失败返回
    `NULL` ＋ `pa_errmsg`（与 `AB-58` 的"载荷指针"同口径）。
  - `pa_fn` 在头文件里**不存在**（只有 `pa_host_fn`）⇒ 签名按 **`pa_host_fn` 函数指针**绑定；
    **同一 C 函数可注册多次**（同实现同签名 ✓），声明一次覆盖全部。

  约束：
  - **与函数同一时刻给** ✓（v1 可执行口径）：必须在该函数**注册之后、任何执行之前**声明——
    注册时记下执行序号，声明时序号必须未变；晚声明 ⇒ `PA_ERR_INVALID` ✗（禁止先注入后补签名）；
  - `params` 与注册时的 `pa_sig.nparams` **一一对应**：**类型**只在 `pa_type_expr` 里；**可选／变参的
    个数语义**仍由 `pa_sig` 的 `PA_PARAM_HAS_DEFAULT`／`PA_PARAM_VARARGS` 表达 ✓；
    `flags` v1 **保留**（必须 0）；
  - **不声明 ＝ 未标注** ✓（按 `object` 处理、**不检查** ✓，行为与今天一致 ✓）；
  - **边界运行期检查** ✓：声明后调用边界核**参数**（个数／类型）与**非 NULL 返回类型**；不符 ⇒
    **脚本侧可捕获的 `TypeError`**（消息带宿主函数名 ⇒ **归责插件** ✓）。**检查期调用点检查**
    也**已落地** ✓（round 273：签名投影进 `TS-47` 的调用点检查；见本节末"实现"）。
  - **宿主类型**：`pa_type_expr_named` 的同一性 ＝ **类型注册得到的那个类型对象** ✓（不按字符串 ✓；
    解析时机＝声明签名时 ⇒ 宿主类型须先 `pa_newtype` ✓）；签名含宿主类型**不暴露内部** ✓
    （`CP-10`：只能当不透明句柄传；边界只核类型对象身份 ✓）；
  - **泛型擦除口径**（沿用 `TS-35`／`TS-30` ✓）：`list[int]`／`dict[K,V]`／`pa_type_expr_apply` 的宿主
    泛型在**边界只核容器／基底身份**，元素类型**不核** ✓（运行期 `isinstance(x, list[int])` 仍按
    `TS-35` 与 CPython 一致报 `TypeError` ✓）；**注解检查不擦除**（`TS-47` 插入点）✓；
    **不做 reified generics** ✗；
  - **类型变量**（`pa_type_expr_var`）：同一次调用的同名变量**统一**（首个实参定下类型，后续须相容）；
    `tparams` 里必须声明参数里用到的变量（未声明 ⇒ `TypeError` 归责插件）✓；
  - `pa_type_expr_callable` v1 只核"可调用"（`PA_TFUNCTION` 标签），形参／返回类型暂不核（如实 ✗）。

  **实现 ✓（round 272 数据模型＋边界；round 273 检查期）**：
  - **边界运行期**：参数（个数／类型）与返回类型检查、归责插件的 `TypeError`、未声明不检查、
    宿主不透明类型、可选＋变参 flags、泛型擦除、类型变量统一；
  - **检查期**：宿主签名投影成静态标签表送进 `TS-47` 的**调用点检查**（扩展模式）——实参静态
    类型可判且与声明不符 ⇒ **编译期** `PA_ERR_SYNTAX`（消息含宿主函数名＋`AB-69`）；判不出的实参
    仍交边界运行期兜底（两档分工 ✓）。实现：`compile_with_host_functions`（`pyawa-core`，只影响
    扩展模式的静态检查、不改发射）＋ ABI 侧签名投影。
  - 验收：`crates/pyawa-abi/tests/host_signature.rs`（10 例：上述全部 ＋ 编译期／运行期两档分工）。

## 16. 尚未写出（本规格自己缺的节）

`SPEC-INDEX.md` §5 第 6 条要求 `v0` 规格显式列出缺口。

**本规格当前无未实现条目**（缺口清空 ✓）：`AB-67`／`AB-68`／`AB-69` 均已实现（round 271／270／272–273）。其余无未写小节，也无未决推迟项。 原先列的四处（`§13-1` 宿主类型继承／`§13-4` 异步分类内容／
`§13-13` 交互模式默认值／`§13-17` 足迹指标）**均已决**，见 `DESIGN.md` §13 的"已决"。
