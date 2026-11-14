/* Pyawa 稳定 C ABI —— 单一头文件（AB-45：禁止分层头）。
 *
 * 权威清单：docs/SPEC-c-abi.md §15。函数总数必须 ≤ 120（AB-1）；
 * 禁止导出清单之外的函数、禁止隐式导出（AB-6）。
 *
 * 本头文件声明**已经接线**的部分，以及三条执行入口：`pa_exec_string` 与
 * `pa_exec_bytecode`（`.pyac` 解码 + `BC-29` 校验）已落地；`pa_exec_file` 的文件 I/O 经能力层
 * （`IM-15`）尚未接线 ⇒ 按 `AB-22` 如实报"**未提供**"（`PA_ERR_NOTIMPLEMENTED`）——
 * 签名在这里固定，宿主不必等实现。
 */
#ifndef PAWA_PA_H
#define PAWA_PA_H

#include <stddef.h>
#include <stdint.h>

/* AB-45：版本宏。编码：主版本在高 16 位（见 crates/pyawa-abi/src/lib.rs）。
 * 次版本 2：fs 域 vtable 表尾追加 stat_times 槽（AB-41：旧宿主仍可用）。
 * 次版本 3：clock 域 vtable 表尾追加 localtime／strftime／mktime 三槽（同上，追加式）。
 * 次版本 4：`AB-63` 类型标签追加 `PA_TBYTES`。
 * 次版本 5：`AB-64` 实例并发边界（宿主契约，无 ABI 面变化）。
 * 次版本 6：`AB-65`／`AB-66` 每实例内存配额两函数（追加式）。
 * 次版本 7：`AB-68` `pa_setpath`（追加式）。
 * 次版本 8：`AB-67` 宿主模块注入四函数（追加式）。
 * 次版本 9：`AB-69` 宿主函数签名（`pa_type_expr` 句柄版，追加式）。
 * 次版本 10：`AB-70` `pa_tohandle`（宿主对象载荷读回）＋ `AB-71` `pa_set_program_filename`
 * （均追加式）；同版补齐 `AB-72` 的 `pa_call`／`pa_pcall` `nresults` 语义（未发布前的补齐）。
 * 次版本 11：`AB-73` 宿主类型属性/方法派发（`pa_set_type_ops`＋新状态码 `PA_ERR_NOTFOUND`）
 * ＋ `AB-74` 宿主异常映射（`pa_raise_host_exception`）—— 均追加式。
 * 次版本 12：`AB-76` `pa_typeid`（追加式）＋ `AB-77` `pa_newtype` 的 `base_index`（未发布窗口内
 * 签名改写；旧签名不留）。 */
#define PA_ABI_MAJOR 1u
#define PA_ABI_MINOR 12u
#define PA_ABI_VERSION ((PA_ABI_MAJOR << 16) | PA_ABI_MINOR)

/* AB-45：函数表字节数 —— 即宿主结构体在本头文件下的字节数（AB-43 的 abi_size）。 */
#define PA_ABI_SIZE (sizeof(pa_host))

/* **AB-46**：本头文件必须能被 **C 与 C++** 同时包含 ⇒ 声明外面套 `extern "C"`。
 * 系统头（`<stddef.h>`／`<stdint.h>`）留在守卫**外面**——它们自己带 `extern "C"`，
 * 再套一层反而可能出问题。版本宏只用 C 预处理器能算的东西（AB-45／AB-46）。 */
#ifdef __cplusplus
extern "C" {
#endif

/* ---- 状态码（AB-19／AB-20；既有取值禁止改含义，新增必须落在预留区）---- */
typedef enum pa_status {
    PA_OK = 0,
    PA_ERR_RUNTIME = 1,
    PA_ERR_SYNTAX = 2,
    PA_ERR_MEMORY = 3,
    PA_ERR_INTERRUPT = 4,
    PA_ERR_NOTIMPLEMENTED = 5, /* 未提供该能力槽位（CP-5）——必须与"已实现但拒绝"区分（AB-22） */
    PA_ERR_INVALID = 6,
    PA_ERR_ABI = 7,
    PA_ERR_NOTFOUND = 8,   /* AB-73：属性派发回调“未处理”（VM 内部回退，不暴露给 guest） */
    PA_ERR_RESERVED_FIRST = 9,
    PA_ERR_RESERVED_LAST = 31
} pa_status;

/* ---- 不透明句柄（AB-14：禁止暴露头部、类型对象或任何内部布局）---- */
typedef struct pa_state pa_state;

/* 类型标签（`AB-63`：取值域**末尾追加**，既有编号禁止改；本头文件与
   crates/pyawa-abi/src/stack.rs 的 tag 模块必须一致）。
   **判类型用 pa_type/pa_is\*，取值用 pa_to\***——禁止用"某个 pa_to\* 是否成功"间接判类型。 */
typedef enum pa_tag {
    PA_TNIL = 0,
    PA_TBOOLEAN = 1,
    PA_TINTEGER = 2,
    PA_TNUMBER = 3,
    PA_TSTRING = 4,
    PA_TTABLE = 5,     /* 本层就是 dict */
    PA_TFUNCTION = 6,
    PA_THANDLE = 7,    /* 宿主对象句柄（OM-34，尚未接线） */
    PA_TBYTES = 8      /* bytes（AB-63 追加；bytearray/memoryview 将来只能继续往后取） */
} pa_tag;

/* ---- 宿主结构（AB-8／AB-43）----
 *
 * abi_size 必须放在偏移 0：运行时以 min(宿主 size, 自身 size) 为界读取、禁止越界读（AB-43），
 * 所以"宿主声明的尺寸"要在读任何其它字段之前先读到。
 * 能力接口 vtable 的形状归属 SPEC-capabilities.md（CP-）；本 ABI 只提供注册入口（AB-32）。
 */
typedef struct pa_host {
    size_t abi_size;              /* 宿主编译时本结构体的字节数 */
    uint32_t abi_version;         /* AB-39：三份契约共用的版本号 */
    const void *capabilities;     /* 能力接口实现；NULL ＝ 一个域都没提供（CP-5） */
} pa_host;

/* ---- 版本查询（§15.3 里不依赖 pa_create 的三条）---- */
const char *pa_version(void);      /* 静态字符串，进程存活期内有效 */
uint32_t pa_abi_version(void);
size_t pa_abi_size(void);

/* ---- 实例生命周期（§15.3）----
 *
 * AB-55：实例经**出参**交回（创建那一刻还没有栈，AB-49 的"经栈"在此不适用）。
 * AB-56：ABI 不匹配时仍交出实例 —— 那是**诊断实例**，只有 pa_errmsg 与 pa_destroy 可用，
 *        其余调用一律返回 PA_ERR_ABI；宿主必须在 *out != NULL 时调用 pa_destroy。
 * AB-57：pa_destroy 释放实例本身；此后 pa_state * 不可用（禁止解引用或复用）。
 */
int pa_create(const pa_host *host, pa_state **out);
int pa_destroy(pa_state *state);
int pa_interrupt(pa_state *state);
const char *pa_errmsg(pa_state *state);

/* ---- 每实例内存配额（AB-65／AB-66；`OM-41`；ABI-adjacent、发布前定）----
 *
 * AB-65：`limit == 0` 取消限制；活分配字节超过上限时脚本里抛**可捕获的** `MemoryError`
 *        （执行器在指令边界派发；**不是** abort／panic）。抛错后水位抬高一档
 *        `slack = max(64 KiB, limit/16)`（处理块还能分配），继续增长则每 `slack` 字节再抛一次。
 * AB-66：`out` 必须可写（`NULL` ⇒ `PA_ERR_INVALID`）；宿主用它做指标／看门狗。
 */
int pa_set_memory_limit(pa_state *state, uint64_t limit);
int pa_memory_used(pa_state *state, uint64_t *out);

/* ---- 每实例 `sys.path`（AB-68；ABI-adjacent、发布前定）----
 *
 * `AB-68`：`dirs == NULL` 或 `n == 0` ⇒ **清空**；否则逐项拷贝成 `sys.path`（宿主缓冲区返回后
 * 可释放 ✓）。路径访问仍受 `fs` 能力域夹根（`CP-21`／`AB-32`）——本函数只决定"在哪找" ✓。
 */
int pa_setpath(pa_state *state, const char *const *dirs, size_t n);

/* ---- 执行（§15.3；AB-5②／AB-7／AB-60／AB-61）----
 *
 * **AB-60**：`mode` 取值**只有两个串**——"python"（IM-1 的纯 Python 模式）／"pyawa"
 * （IM-1 的扩展模式，Pyawa 的完整形态）；**大小写敏感、全串匹配、不接受别名**；
 * 空串／NULL／任何其他值 ⇒ `PA_ERR_INVALID`(6)；**禁止**从路径后缀或来源内容推断模式。
 * 错误码分工：**mode 不合法 ⇒ 6**、**源码解析失败 ⇒ `PA_ERR_SYNTAX`(2)**——宿主据此分辨
 * "我传错了参数"与"脚本自己有问题"。
 *
 * **AB-61**：`mode` 之外的编译输入（**检查档位**与**优化级**）经 `pa_options` 过界——
 * **尺寸标记**结构（首字段 `size`，惯例同 `AB-43`／`AB-51`；以后追加字段不改签名）；
 * `pa_exec_string`／`pa_exec_file` 收 `const pa_options *`，**允许 NULL**（⇒ 浅层的
 * `TS-31` 默认 ＋ 默认优化级）。宿主给了就以宿主的为准：**深层**会按 `BC-25` ② 发边界检查。
 * ← `AB-7` 的档位子句**由此满足**（`SPEC-c-abi.md` §15.3）。
 *
 * 栈契约一律 `—`（§15.3）：执行结果**不进栈**；脚本在**本实例的全局命名空间**里跑
 * （与 `pa_getglobal`／`pa_setglobal`／`pa_register` 同一份），失败信息经 `pa_errmsg` 取（`AB-48`）。
 * **脚本语义**：模块全局里 `__name__` 未绑定时补 `"__main__"`（`python3 -c`／脚本同款；类体
 * 序言要读它），宿主绑过就**不覆盖**。
 * `len < 0` ⇒ `source` 按 NUL 结尾算（口径同 `pa_pushstring`）；`chunkname` 为空／NULL ⇒
 * 取 `<string>`（它现在还进不了产物）。
 *
 * 一条**如实报"未提供"**（`PA_ERR_NOTIMPLEMENTED`，`AB-22`）：
 *   - `pa_exec_file`：文件 I/O 经能力层（`IM-15`），能力层尚未接线
 *
 * `pa_exec_bytecode` **已提供**：按 `.pyac` 容器解码，指令集版本不符 ⇒ `PA_ERR_INVALID`（`BC-29`）；
 * 本条**没有 `mode`、也没有 `pa_options`**（`AB-60`／`AB-61`：模式／优化级／档位三样都随产物
 * 头部走，`IM-19`），宿主**不得**另行指定（否则两个真相）。
 */
typedef struct pa_options {
    size_t size;             /* 本结构体的字节数（AB-61：尺寸标记） */
    uint32_t check_tier;     /* 检查档位（TS-31）：0 ＝ 浅层（默认）、1 ＝ 深层；其他 ⇒ PA_ERR_INVALID */
    uint32_t optimization;   /* 优化级（IM-19）：0 ＝ 默认（本层没有优化器 ⇒ 目前不改发射）；> 255 ⇒ PA_ERR_INVALID */
} pa_options;

int pa_exec_string(pa_state *state, const char *source, ptrdiff_t length,
                   const char *chunkname, const char *mode, const pa_options *options);
int pa_exec_file(pa_state *state, const char *path, const char *mode,
                 const pa_options *options);
int pa_exec_bytecode(pa_state *state, const void *buffer, ptrdiff_t length);
/* AB-71：设置**后续执行产物**的 `co_filename` 来源（CLI 在执行前调用；未设置 ⇒ 内部占位名）。
 * `name` 指向 `len` 个 UTF-8 字节（**不要求** NUL 结尾）；`len < 0`／`NULL`／非 UTF-8 ⇒
 * `PA_ERR_INVALID`；按实例保存（不影响其它实例）；栈契约 `—`。 */
int pa_set_program_filename(pa_state *state, const char *name, ptrdiff_t len);

/* ---- 虚拟栈（AB-9…AB-13）----
 *
 * AB-9：索引规则写死在这里 —— 正索引自底（1 起）、负索引自顶（−1 是栈顶）、0 非法。
 * AB-10：每个槽位持有一个引用；宿主按 SPEC-c-abi §6 的转移规则归还。
 * AB-12：越界返回 PA_ERR_INVALID，禁止 UB。AB-13：栈与实例绑定，禁止跨实例使用索引。
 * AB-15：借用与持有可区分（pa_retain 借用→持有、pa_release 持有→释放）。
 */
int pa_gettop(pa_state *state);
int pa_settop(pa_state *state, int n);
int pa_pushvalue(pa_state *state, int idx);
int pa_pop(pa_state *state, int n);
int pa_type(pa_state *state, int idx);
/* AB-76：真实类型身份（不透明类型句柄，借用，AB-15）。pa_type 仍只给 AB-63 的值类别。
 * 越界 / typeid_out == NULL ⇒ PA_ERR_INVALID。 */
int pa_typeid(pa_state *state, int idx, void **typeid_out);
int pa_isnil(pa_state *state, int idx);
int pa_isboolean(pa_state *state, int idx);
int pa_isinteger(pa_state *state, int idx);
int pa_isnumber(pa_state *state, int idx);
int pa_isstring(pa_state *state, int idx);
int pa_istable(pa_state *state, int idx);
int pa_isfunction(pa_state *state, int idx);
int pa_pushnil(pa_state *state);
int pa_pushboolean(pa_state *state, int b);
int pa_pushinteger(pa_state *state, int64_t i);
int pa_pushnumber(pa_state *state, double d);
int pa_pushstring(pa_state *state, const char *s, ptrdiff_t len);  /* len < 0 ⇒ 按 NUL 结尾 */
int pa_pushbytes(pa_state *state, const void *p, ptrdiff_t len);   /* 字节串（复制）；len < 0 ⇒ 按 NUL 结尾 */
int pa_pushintstring(pa_state *state, const char *s, ptrdiff_t len);
/* AB-62：从**十进制**串构造整数并压栈（+1）；len < 0 ⇒ 按 NUL 结尾。
 * 语义 ＝ 参照的 `int(s)`（前导 +/-、前后空白、下划线一如下）；**任意精度**。
 * 失败：**解析失败 ⇒ PA_ERR_INVALID**；**位数超 `sys.get_int_max_str_digits()`（默认 4300）⇒ 抛
 * ValueError 一类**（本实现返 `PA_ERR_RUNTIME`，消息经 `pa_errmsg` 取，`AB-48`）。
 * **禁止**用本桥搬非整数（`bytes` 用 `pa_pushbytes`／`pa_tobytes`；别的类型需要时再按同一手法加）。 */
int pa_pushhandle(pa_state *state, void *h);
/* AB-58／AB-59：按 type（**栈索引**，AB-9）新建宿主对象：+1；该槽**不消耗**（宿主负责 pop，
 * AB-11），故"压一次类型、建多个实例"可行。载荷经出参交回（payload_size == 0 ⇒ NULL） */
int pa_newhandle(pa_state *state, int type_index, void **payload_out);
int pa_toboolean(pa_state *state, int idx);
int pa_tointeger(pa_state *state, int idx, int64_t *out);
/* AB-62：**整数**的十进制只读**借用**视图（借用约定同 `pa_tostring`，`AB-15`／`AB-48`）。
 * **覆盖全部整数**（`i64` 内的也走它 ⇒ 宿主只需这一条统一路径）。
 * 失败返 **NULL**（借用型返回没有状态码通道）：**非整数**（含 `bool`——它的 i64 视图走
 * `pa_tointeger`）与**位数超上限**都是 NULL，原因经 `pa_errmsg` 取。 */
const char *pa_tointstring(pa_state *state, int idx, size_t *len);
int pa_tonumber(pa_state *state, int idx, double *out);
const char *pa_tostring(pa_state *state, int idx, size_t *len);    /* 只读视图（借用，AB-15） */
const char *pa_tobytes(pa_state *state, int idx, size_t *len);     /* 只读字节视图（借用） */
/* AB-70：**宿主对象**的载荷读回（栈契约 `—`）。成功 ⇒ *payload_out 指向 VM 分配的载荷
 * （AB-58；payload_size == 0 ⇒ NULL）；**只借出、不转移**（AB-15／AB-17，宿主禁止 free）。
 * 栈上不是宿主对象／idx 越界／payload_out == NULL ⇒ PA_ERR_INVALID。
 * 注意：list／普通实例的 pa_type 也是 PA_THANDLE ⇒ **判定用本函数，不要用 pa_type 猜**。 */
int pa_tohandle(pa_state *state, int idx, void **payload_out);
int pa_newtable(pa_state *state);
int pa_newlist(pa_state *state, int n);
int pa_retain(pa_state *state, int idx);
int pa_release(pa_state *state, int idx);

/* ---- 宿主类型属性/方法派发（AB-73；追加式，次版本 11）----
 *
 * 回调用途：getattr_fn 处理 ⇒ 恰好压入 1 个值（净 +1）并返回 PA_OK；不处理 ⇒ PA_ERR_NOTFOUND
 * （不压栈，VM 回退到默认，最终 AttributeError）；出错 ⇒ 其它 PA_ERR_*（VM 抛脚本可捕获异常，
 * 不静默回退）。setattr_fn 的值取栈上 value_index（借用、不消耗）。
 * 回调禁止保存 payload／context／state 供返回后使用（AB-17）；payload 归 VM（AB-58）。 */
typedef int (*pa_type_getattr)(pa_state *state, void *payload,
                               const char *name, size_t len, void *context);
typedef int (*pa_type_setattr)(pa_state *state, void *payload,
                               const char *name, size_t len, int value_index, void *context);

typedef struct pa_type_ops {
    size_t size;                 /* 本结构体字节数（自带尺寸，AB-43 界读） */
    void *context;               /* 透传给回调；禁止保存 */
    pa_type_getattr getattr_fn;  /* 可为 NULL */
    pa_type_setattr setattr_fn;  /* 可为 NULL */
} pa_type_ops;

/* AB-73：给 pa_newtype 注册的类型装回调（NULL ⇒ 清空/不派发）；type_index 是栈索引且不消耗。
 * 失败（越界/不是宿主类型/size 非法）⇒ PA_ERR_INVALID。 */
int pa_set_type_ops(pa_state *state, int type_index, const pa_type_ops *ops);

/* AB-74：宿主抛**带类型**的异常（追加式，次版本 11）。
 * type_index 是栈索引（AB-9），指向 BaseException 子类（不消耗）；VM 以该类型＋ msg 实例化并
 * 作为 guest 异常重抛。成功 ⇒ PA_ERR_RUNTIME（宿主直接 return）；非法 ⇒ PA_ERR_INVALID。
 * msg 借用、UTF-8；len < 0 ⇒ 以 NUL 结尾计长，msg == NULL ⇒ 空串。 */
int pa_raise_host_exception(pa_state *state, int type_index, const char *msg, ptrdiff_t len);

/* ---- 宿主函数与签名（AB-24…AB-26、AB-51／AB-52）----
 *
 * AB-25：注册必须同时提供签名，禁止无名签名的宿主函数。
 * AB-26：宿主函数内部的 panic 必须被捕获并转成状态码（Rust 侧用 `extern "C-unwind"` +
 *        边界 `catch_unwind`；C 宿主本来不 panic，不受影响）。
 *
 * **返回值约定**（规格未钉，本实现定，写在这里）：宿主函数返回**状态码**；
 * 它把结果留在**栈顶**，调用方取走栈顶那一项当返回值（PA_OK 且栈空 ⇒ 结果按 nil 处理）。
 * 参数从虚拟栈取（AB-24）：调用时实参逐个压栈，`-1` 是最后一个实参。
 */
typedef int (*pa_host_fn)(pa_state *state);

typedef struct pa_param {
    size_t size;              /* 本结构体字节数（AB-51 的自带尺寸） */
    const char *name;         /* NUL 结尾的 UTF-8；可为 NULL */
    const char *type_expr;    /* 注解表达式字符串（由 Pyawa 自己的注解解析器求值）；可为 NULL */
    uint32_t flags;           /* 位置／仅关键字／可变位置／可变关键字／有无默认值 */
    void *default_handle;     /* 默认值（不透明句柄）；没有则 NULL */
} pa_param;

/* pa_param.flags 的位（AB-27／AB-52） */
#define PA_PARAM_POSITIONAL (1u << 0)
#define PA_PARAM_KEYWORD_ONLY (1u << 1)
#define PA_PARAM_VARARGS (1u << 2)
#define PA_PARAM_VARKW (1u << 3)
#define PA_PARAM_HAS_DEFAULT (1u << 4)

typedef struct pa_sig {
    size_t size;              /* 本结构体字节数（AB-51 的自带尺寸） */
    uint32_t flags;           /* 签名级标志（PA_TYPE_FINAL 等） */
    const char *ret_expr;     /* 返回注解；可为 NULL */
    size_t nparams;
    const pa_param *params;
} pa_sig;

/* pa_sig.flags 的保留位（AB-37）：宿主用它反向选择"本类型不可继承" */
#define PA_TYPE_FINAL (1u << 0)

int pa_getglobal(pa_state *state, const char *name);
int pa_setglobal(pa_state *state, const char *name);
int pa_register(pa_state *state, const char *name, pa_host_fn fn, const pa_sig *sig);

/* ---- 宿主模块注入（`AB-67`；ABI-adjacent、发布前定）----
 *
 * 宿主库不是 Pyawa 写的时的正解：`pa_add_host_module` 拿模块句柄，再用原子 API 装
 * 宿主函数／常量／子模块（树形命名空间）。模块对象是**普通 Python 模块**（`__name__`／
 * `sys.modules`／`from X import y` 一致 ✓）；注入发生在 setup 阶段／两次宿主调用之间；
 * **宿主函数回调内部禁止改模块结构**（返回 `PA_ERR_INVALID`，不是 UB ✓）。
 * 保留名（`sys`／`builtins`／`io` 等内置模块名）不得覆盖 ⇒ `PA_ERR_INVALID`。
 * 宿主函数签名**必须同时给**（`AB-25`／`AB-51`；`AB-69` 的 `pa_type_expr` 签名走另一 API）。
 * 句柄是**每实例**不透明指针，`pa_destroy` 后失效（`AB-18`）。
 */
typedef struct pa_module_s *pa_module;
int pa_add_host_module(pa_state *state, const char *name, pa_module *out);
int pa_module_addfunction(pa_state *state, pa_module module, const char *name,
                          pa_host_fn fn, const pa_sig *sig);
int pa_module_setconst(pa_state *state, pa_module module, const char *name);
int pa_module_addsubmodule(pa_state *state, pa_module parent, const char *name,
                           pa_module *out);

/* ---- 宿主函数签名（`AB-69`；`pa_type_expr` 句柄版；ABI-adjacent、发布前定）----
 *
 * 类型表达式是**不透明句柄** `pa_type_expr`（不是扁平标签 ✗）：构造 API 见下；句柄**按实例**存，
 * `pa_destroy` 后失效（`AB-18`）。构造失败 ⇒ 返回 `NULL` 并经 `pa_errmsg` 说明。
 * **`pa_type_expr` ≠ `pa_type()` 的返回值**：前者是**签名里的类型表达式句柄**（构造/组合用），
 * 后者是**运行时值的标签**（`PA_T*`，`AB-63`）——两者不要混用。
 *
 * `pa_func_set_signature`：给**已注册**的宿主函数声明签名；返回状态码（`AB-19`）。
 * **与函数同一时刻给**：必须在该函数注册之后、**任何执行之前**（晚声明 ⇒ `PA_ERR_INVALID`）。
 * `params` 与注册时 `pa_sig.nparams` 一一对应（类型只在 `pa_type_expr`；可选／变参的**个数**语义仍由
 * `pa_sig` 的 flags 表达）；`flags` v1 保留（必须 0）；不声明 ⇒ 不检查（与今天一致）。
 * 声明后：调用边界核**参数**（及非 NULL 返回类型）⇒ 不符是**脚本侧可捕获的 `TypeError`**
 * （消息归责插件）。**泛型擦除**（`TS-35`）：`list[T]`／`dict[K,V]`／宿主泛型只核容器／基底身份，
 * 元素类型不核 ✓（不做 reified generics ✗）。
 */
typedef struct pa_type_expr_s *pa_type_expr;
pa_type_expr pa_type_expr_runtime_tag(pa_state *state, int tag);
pa_type_expr pa_type_expr_named(pa_state *state, const char *name);
pa_type_expr pa_type_expr_list_of(pa_state *state, pa_type_expr elem);
pa_type_expr pa_type_expr_dict_of(pa_state *state, pa_type_expr key, pa_type_expr value);
pa_type_expr pa_type_expr_optional(pa_state *state, pa_type_expr inner);
pa_type_expr pa_type_expr_union(pa_state *state, const pa_type_expr *members, size_t count);
pa_type_expr pa_type_expr_callable(pa_state *state, const pa_type_expr *params, size_t count, pa_type_expr ret);
pa_type_expr pa_type_expr_var(pa_state *state, const char *name);
pa_type_expr pa_type_expr_apply(pa_state *state, pa_type_expr base, const pa_type_expr *arguments, size_t count);
int pa_func_set_signature(pa_state *state, pa_host_fn function,
                          const pa_type_expr *params, size_t nparams, pa_type_expr ret,
                          const pa_type_expr *tparams, size_t ntparams, unsigned flags);
/* AB-72：nresults 是**精确**结果数（栈契约 −nargs+nresults）：0 ⇒ 丢弃结果；≥1 ⇒ 产出
 * nresults 项（v1 的调用固定产出 1 个结果，其余补 None；未来多值通道按「多截、少补 None」）。
 * nargs < 0 或 nresults < 0 ⇒ PA_ERR_INVALID。 */
int pa_call(pa_state *state, int nargs, int nresults);
int pa_pcall(pa_state *state, int nargs, int nresults);
int pa_error(pa_state *state, const char *msg);

/* ---- 宿主类型注册（AB-35…AB-38、AB-58）----
 *
 * AB-35：注册为**真实类型**（禁止另立一套对象表示）。AB-36：必须提供 dealloc 与 traverse。
 * AB-37：默认可被继承；sig->flags 里设 PA_TYPE_FINAL 才表示"本类型不可继承"。
 *        宿主对象布局固定 ⇒ 实例字典由 VM 另行挂载；子类实例的载荷按**同一尺寸**由 VM 分配。
 * AB-36／OM-36：traverse 是"上下文 ＋ 回调"形态（C 侧不能传闭包）：
 *        宿主对每个直接引用调用 visit(句柄, context)。禁止把 context／visit 存起来后用。
 *
 * AB-58：**载荷由 VM 分配、归 VM 所有**
 *   - payload_size 在**注册时**声明；pa_newhandle 交回的指针指向 VM 分配的这么多字节
 *   - 载荷随实例（与类型）一并回收，**由 VM 释放**；宿主**禁止** free／realloc
 *   - payload_size == 0 ⇒ 出参为 NULL
 *   - 宿主的 dealloc 只放掉它**塞在载荷里面**的东西（内部资源），不碰载荷存储本身
 *   - 禁止"宿主自己分配、只把指针交进来"（OM-3 记不了账、AB-18 管不到）
 */
typedef void (*pa_host_dealloc)(void *payload);
typedef void (*pa_host_visit)(void *handle, void *context);
typedef void (*pa_host_traverse)(void *payload, void *context, pa_host_visit visit);

/* AB-59：注册成功后把**类型对象**压栈（栈契约 +1，不透明句柄，AB-14）——这是宿主拿到类型的
 * 唯一通道：禁止回传类型指针，禁止另立"注册序号"这类第二套标识 */
int pa_newtype(pa_state *state, const char *name, int base_index, size_t payload_size,
               pa_host_dealloc dealloc, pa_host_traverse traverse, const pa_sig *sig);

/* ---- 属性与下标（§15.3）----
 *
 * 语义引核心：getfield／setfield 走 OM-11 的 getattr／setattr（SPEC-type-system §8），
 * gettable／settable 走 BC-39 的 NB_SUBSCR／STORE_SUBSCR 语义，rawget／rawset 不触发协议。
 *
 * **栈契约的差异**（规格 §15.3 把这几行记为 ±1 "就地替换"，本实现按自然语义取）：
 *   - getfield：就地替换栈顶（对象在 idx）⇒ 深度不变（±1）
 *   - setfield：值在栈顶、对象在 idx；写入后**弹掉值**（−1，失败也弹）
 *   - gettable：键在栈顶、容器在 idx；键被消耗、值就地放上（±1）
 *   - settable／rawset：栈是 [容器(idx), 键, 值]；**键值都被消耗**（−2）
 * 差异写在 crates/pyawa-abi/README.md 的"尚未落地／已知差异"一节。
 */
int pa_getfield(pa_state *state, int idx, const char *name);
int pa_setfield(pa_state *state, int idx, const char *name);
int pa_gettable(pa_state *state, int idx);
int pa_settable(pa_state *state, int idx);
int pa_rawget(pa_state *state, int idx);
int pa_rawset(pa_state *state, int idx);

/* ---- 能力接口注册（AB-32…AB-34）----
 *
 * AB-32：本 ABI 只提供注册入口；vtable 的形状一律引 SPEC-capabilities.md（CP-）。
 * AB-33：按能力域注册；未注册的域即"未提供"（CP-5）。
 * AB-34／CP-25：注册前必须显式声明该域的异步分类（二值，CP-37：禁止域内混合）——
 *               缺失即注册失败，禁止落默认值。
 */
typedef enum pa_domain {
    PA_DOMAIN_FS = 0,
    PA_DOMAIN_NET = 1,
    PA_DOMAIN_PROC = 2,
    PA_DOMAIN_CLOCK = 3,
    PA_DOMAIN_RANDOM = 4,
    PA_DOMAIN_ENV = 5,
    PA_DOMAIN_TTY = 6,
    PA_DOMAIN_LOCALE = 7,
    PA_DOMAIN_IPC = 8
} pa_domain;

#define PA_DOMAIN_COUNT 9

typedef enum pa_async {
    PA_ASYNC_OK = 0,   /* 可异步化 */
    PA_ASYNC_NO = 1    /* 不可异步化 */
} pa_async;

int pa_setcapability(pa_state *state, int domain, const void *impl);
int pa_setcapability_async(pa_state *state, int domain, int classification);

/* ---- 辅助层 paL_（§15.4，18 个；不引入核心层没有的语义，AB-4／AB-6）----
 *
 * §15.4 只给名字与语义、**没有给签名** ⇒ 本层按 AB-19（跨边界函数必须返回状态码）统一取
 * "状态码 ＋ 出参"形态，**本头文件就是那唯一处定义**。返回值一律是 pa_status。
 *
 * 结构性限制（README 里同样写明）：
 *   - paL_error 的 C 变参格式化在稳定版 Rust 里做不到（要 vsnprintf）⇒ 只收拼好的消息；
 *     format 那一路等 AB-44 的版本策略裁定
 *   - paL_openlibs／paL_dostring／paL_dofile 分别缺标准库（P3-14）与编译器（P3-12）
 *   - paL_where 缺 traceback（OM-28）、paL_requiref 缺模块系统（IM-）
 */
typedef struct pa_reg {
    const char *name;        /* NUL 结尾的 UTF-8 */
    pa_host_fn function;     /* AB-25：每个函数都要带签名 */
    const pa_sig *sig;
} pa_reg;

int paL_checkinteger(pa_state *state, int idx, int64_t *out);
int paL_optinteger(pa_state *state, int idx, int64_t def, int64_t *out);
int paL_checkstring(pa_state *state, int idx, const char **out, size_t *len);
int paL_optstring(pa_state *state, int idx, const char *def, const char **out, size_t *len);
int paL_len(pa_state *state, int idx, size_t *out);
int paL_getsubtable(pa_state *state, int idx, const char *name);
int paL_ref(pa_state *state, int idx, int *out_ref);       /* 注册表键从 1 起 */
/* AB-73：把注册表键的值压栈（+1，新引用）；非法键 ⇒ PA_ERR_INVALID。 */
int paL_getref(pa_state *state, int ref);
int paL_unref(pa_state *state, int ref);                   /* 幂等 */
int paL_traceback(pa_state *state, const char *msg);
int paL_where(pa_state *state, int level, const char **out, size_t *len);
int paL_error(pa_state *state, const char *msg);
int paL_execresult(pa_state *state, int status);           /* PA_OK ⇒ 压 True（+1） */
int paL_requiref(pa_state *state, const char *name, const void *openf, int glb);
int paL_setfuncs(pa_state *state, const pa_reg *regs, int n);  /* n < 0 ⇒ 以 NULL 名字结尾 */

int paL_openlibs(pa_state *state);
int paL_dostring(pa_state *state, const char *s, int mode);
int paL_dofile(pa_state *state, const char *path, int mode);

/* paL_newstate()：pa_create ＋ 真实机器 provider 的便捷入口（实现属 pyawa-runtime）。
 * 交回的指针必须用 pa_destroy 释放；失败给 NULL。 */
pa_state *paL_newstate(void);

/* 宿主可用的版本兼容判定（AB-41：主版本相同即可用；次版本差异只是表尾追加）：
 *   (host.abi_version >> 16) == PA_ABI_MAJOR
 */

#ifdef __cplusplus
}
#endif

#endif /* PAWA_PA_H */
