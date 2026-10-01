/* Pyawa 稳定 C ABI —— 单一头文件（AB-45：禁止分层头）。
 *
 * 权威清单：docs/SPEC-c-abi.md §15。函数总数必须 ≤ 120（AB-1）；
 * 禁止导出清单之外的函数、禁止隐式导出（AB-6）。
 *
 * 本头文件当前只声明**已落地**的部分（版本查询与实例生命周期）；
 * 其余函数（执行／栈／值转换／宿主注册／能力注册）待各自的下一步落地后再声明。
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
int pa_newhandle(pa_state *state, int kind);                       /* 宿主对象未接线 ⇒ NOTIMPLEMENTED */
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

/* 宿主可用的版本兼容判定（AB-41：主版本相同即可用；次版本差异只是表尾追加）：
 *   (host.abi_version >> 16) == PA_ABI_MAJOR
 */

#ifdef __cplusplus
}
#endif

#endif /* PAWA_PA_H */
