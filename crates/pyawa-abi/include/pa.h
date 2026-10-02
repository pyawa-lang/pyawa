/* Pyawa 稳定 C ABI —— 单一头文件（AB-45：禁止分层头）。
 *
 * 权威清单：docs/SPEC-c-abi.md §15。函数总数必须 ≤ 120（AB-1）；
 * 禁止导出清单之外的函数、禁止隐式导出（AB-6）。
 *
 * 本头文件声明**已经接线**的部分，以及三条执行入口：`pa_exec_string` 已落地；
 * `pa_exec_file`／`pa_exec_bytecode` 按 `AB-22` 如实报"**未提供**"（`PA_ERR_NOTIMPLEMENTED`）——
 * 签名在这里固定，宿主不必等实现。
 */
#ifndef PAWA_PA_H
#define PAWA_PA_H

#include <stddef.h>
#include <stdint.h>

/* AB-45：版本宏。编码：主版本在高 16 位（见 crates/pyawa-abi/src/lib.rs）。 */
#define PA_ABI_MAJOR 1u
#define PA_ABI_MINOR 0u
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
    PA_ERR_RESERVED_FIRST = 8,
    PA_ERR_RESERVED_LAST = 31
} pa_status;

/* ---- 不透明句柄（AB-14：禁止暴露头部、类型对象或任何内部布局）---- */
typedef struct pa_state pa_state;

/* 类型标签（取值由实现定；本头文件与 crates/pyawa-abi/src/stack.rs 的 tag 模块必须一致） */
typedef enum pa_tag {
    PA_TNIL = 0,
    PA_TBOOLEAN = 1,
    PA_TINTEGER = 2,
    PA_TNUMBER = 3,
    PA_TSTRING = 4,
    PA_TTABLE = 5,     /* 本层就是 dict */
    PA_TFUNCTION = 6,
    PA_THANDLE = 7     /* 宿主对象句柄（OM-34，尚未接线） */
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
const char *pa_errmsg(pa_state *state);   /* 借用；AB-48：后续 API 调用之后禁止继续使用 */

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
 * 另两条**如实报"未提供"**（`PA_ERR_NOTIMPLEMENTED`，`AB-22`）：
 *   - `pa_exec_file`：文件 I/O 经能力层（`IM-15`），能力层尚未接线
 *   - `pa_exec_bytecode`：`.pyac` 装载器尚未接线（`P3-12`）；本条**没有 `mode`、也没有
 *     `pa_options`**（`AB-60`／`AB-61`：模式／优化级／档位三样都随产物头部走，`IM-19`），
 *     宿主**不得**另行指定（否则两个真相）
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
int pa_pushbytes(pa_state *state, const void *p, ptrdiff_t len);   /* 字节串类型未落地 ⇒ NOTIMPLEMENTED */
int pa_pushhandle(pa_state *state, void *h);
/* AB-58／AB-59：按 type（**栈索引**，AB-9）新建宿主对象：+1；该槽**不消耗**（宿主负责 pop，
 * AB-11），故"压一次类型、建多个实例"可行。载荷经出参交回（payload_size == 0 ⇒ NULL） */
int pa_newhandle(pa_state *state, int type_index, void **payload_out);
int pa_toboolean(pa_state *state, int idx);
int pa_tointeger(pa_state *state, int idx, int64_t *out);
int pa_tonumber(pa_state *state, int idx, double *out);
const char *pa_tostring(pa_state *state, int idx, size_t *len);    /* 只读视图（借用，AB-15） */
const char *pa_tobytes(pa_state *state, int idx, size_t *len);
int pa_newtable(pa_state *state);
int pa_newlist(pa_state *state, int n);
int pa_retain(pa_state *state, int idx);
int pa_release(pa_state *state, int idx);

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
int pa_newtype(pa_state *state, const char *name, size_t payload_size,
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
