/* **通用参考宿主**（PLAN 收口后下一阶段 ④；2026-11-13）—— **与具体宿主身份无关**：
 *
 * 这不是"某个应用的宿主示例" ✗，而是**照抄即可**的规格级示例 ✓：一个最小的 C 宿主把
 * `PA_*` ABI 用对的**顺序、责任与边界**演示一遍。下游宿主**不需要**改这里的业务逻辑——
 * 这里**没有**业务逻辑 ✓（没有网络、没有 UI、没有特定框架、没有命名/身份假设 ✓）。
 *
 * 覆盖六件事（对应 `DESIGN.md` §7.5 的宿主责任）：
 *   1. **创建**：版本兼容的 `pa_host` → `pa_create`（AB-55）。
 *   2. **宿主定义脚本 API**：`pa_register` 注入 `host_sum`/`host_echo`/`host_read`/`host_tick`
 *      （AB-24／AB-51）；脚本 → 宿主回调 → 再回脚本。
 *   3. **沙箱拒绝越界**：`host_read` 只允许宿主根目录下的**单段名字**（`..`／`/`／绝对路径⇒
 *      `pa_error` ⇒ 脚本收到 `RuntimeError`）。**如实边界**：最小 ABI 产物不装 stdlib（`paL_openlibs`
 *      在组合根产物里，`§12.13 #1`）⇒ 示例脚本不调 `open()`/`import`；能力域 vtable 样例见本文件
 *      第 7 项（`clock` 域）。
 *   4. **超时由宿主兜**：VM 只有协作式中断（AB-5①）——`host_tick` 检查单调时钟，超预算
 *      `pa_interrupt` ⇒ `pa_exec_string` 返回 `PA_ERR_INTERRUPT`（不是 abort ✓）。
 *   5. **每实例内存配额**：创建后 `pa_set_memory_limit`（AB-65），脚本里超限是**可捕获的**
 *      `MemoryError`；`pa_memory_used`（AB-66）用于看门狗/指标。
 *   6. **实例隔离与销毁**：两个实例互不可见（OM-1）；`pa_destroy` 释放实例（AB-57）。
 *   7. **能力域 vtable**（§12.17 A2）：注册最小 `clock` 域（`pa_capabilities.h` 的
 *      `pa_clock_vtable`），宿主函数 `host_now_ns()` 调它的 `now_ns` 槽（不依赖 guest 标准库）。
 *
 * 构建与运行（与 `tests/ci/t_ref_host.py` 同一串；缺 `cc` 就是红）：
 *   cargo build -p pyawa-abi
 *   cc examples/reference_host.c -I crates/pyawa-abi/include target/debug/libpyawa_abi.a \
 *      -lpthread -ldl -lm -o /tmp/reference_host && /tmp/reference_host
 *   （Windows/MSVC（`PLAN §12.20` W6）：`cl /nologo /I crates\pyawa-abi\include
 *      examples\reference_host.c target\debug\pyawa_abi.lib /Fe:reference_host.exe`；
 *      CI 由 `tests/ci/t_ref_host.py --release` 真编译真运行）
 *
 * 头文件 `pa.h` 是**唯一**的 ABI 依赖；本文件之外不需要任何 pyawa 内部头（AB-6）。
 * 宿主 API 的参数都按 `pa_sig` 内联声明（AB-51）；注解表达式留 NULL（宿主可不声明类型）。
 */
#include "pa.h"
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <time.h>
#ifdef _WIN32
#include <windows.h> /* `PLAN §12.20` W6：Windows 上 clock／临时目录用 Win32（无 POSIX） */
#define PY_PATH_SEP "\\"
#else
#include <unistd.h>
#define PY_PATH_SEP "/"
#endif

/* ---- 宿主自己的状态（provider 状态、配额、超时预算都在宿主侧 ✓） ------------------ */
typedef struct {
    char root[512];       /* 沙箱根（本示例用 mkdtemp 造临时目录，不是业务配置） */
    long long deadline_ms;/* host_tick 的协作式超时预算（毫秒） */
    long long started_ms; /* 首次调用 host_tick 的时刻 */
    int tick_calls;
    const char *message;  /* pa_error 的消息来源（指向静态串即可） */
} host_ctx;

static long long now_ms(void)
{
#ifdef _WIN32
    /* 单调毫秒；参考宿主只需要"协作式超时"的量级（与 POSIX 分支同口径）。 */
    return (long long)GetTickCount64();
#else
    struct timespec ts;
    clock_gettime(CLOCK_MONOTONIC, &ts);
    return (long long)ts.tv_sec * 1000 + ts.tv_nsec / 1000000;
#endif
}

/* 从一个 NULL 结尾的字符串参数取「可比较的借用」（pa_to* 的视图只在下一次调用前有效 ✓）。 */
static int string_arg(pa_state *state, int index, char *out, size_t cap)
{
    size_t len = 0;
    const char *text = pa_tostring(state, index, &len);
    if (text == NULL || len + 1 > cap) return PA_ERR_INVALID;
    memcpy(out, text, len);
    out[len] = '\0';
    return PA_OK;
}

/* 本示例是**单宿主进程**：宿主状态放静态量。多实例共享同一份沙箱配置 ✓；
 * 若宿主需要"每实例不同配置"，用 ABI 的宿主对象/句柄机制（AB-35）或自建映射。 */
static host_ctx g_ctx;

static int push_ok_string(pa_state *state, const char *text)
{
    return pa_pushstring(state, text, (ptrdiff_t)strlen(text));
}

/* ---- 能力域 vtable（§12.17 A2）：最小 `clock` 域实现 ----------------------------- */
/* 布局来自 `pa_capabilities.h`（自动生成，与 Rust `CpClockVtable` 逐字段一致）。
 * A2 取舍：参考宿主**不换产物**（仍链最小 ABI 静态库）⇒ 不依赖 guest `import time`；
 * guest 通过 `host_now_ns()` 调本 vtable 的 `now_ns` 槽取回时间。 */
static pa_cap_status host_clock_now_ns(void *state, int64_t *out_ns)
{
    (void)state; /* provider 状态：本示例无 */
#ifdef _WIN32
    /* FILETIME：1601-01-01 起的 100ns；与 Unix 纪元差 11644473600 s。 */
    FILETIME file_time;
    ULARGE_INTEGER value;
    GetSystemTimeAsFileTime(&file_time);
    value.LowPart = file_time.dwLowDateTime;
    value.HighPart = file_time.dwHighDateTime;
    if (value.QuadPart < 116444736000000000ULL) return PA_CAP_MACHINE;
    *out_ns = (int64_t)(value.QuadPart - 116444736000000000ULL) * 100LL;
#else
    struct timespec ts;
    if (clock_gettime(CLOCK_REALTIME, &ts) != 0) return PA_CAP_MACHINE;
    *out_ns = (int64_t)ts.tv_sec * 1000000000LL + (int64_t)ts.tv_nsec;
#endif
    return PA_CAP_OK;
}

static pa_clock_vtable g_clock_vtable = {
    PA_ABI_VERSION,          /* version：CP-30／AB-39（三份契约共用的号） */
    (uint32_t)sizeof(pa_clock_vtable), /* size：旧宿主有界读取 */
    NULL,                    /* state：本示例无 provider 状态 */
    host_clock_now_ns,       /* now_ns */
    NULL,                    /* monotonic_ns（本示例不给） */
    NULL,                    /* sleep_ns */
    NULL,                    /* localtime */
    NULL,                    /* strftime */
    NULL,                    /* mktime */
};

/* ---- 宿主函数：脚本可以调用的 API（AB-24；参数借用，返回值压栈） ------------------ */

/* `host_sum(left, right) -> int`：最小回调示例。 */
static int host_sum(pa_state *state)
{
    int64_t left = 0, right = 0;
    if (pa_tointeger(state, -2, &left) != PA_OK) return PA_ERR_INVALID;
    if (pa_tointeger(state, -1, &right) != PA_OK) return PA_ERR_INVALID;
    if (pa_pop(state, 2) != PA_OK) return PA_ERR_RUNTIME;
    return pa_pushinteger(state, left + right);
}

/* `host_echo(text) -> str`：字符串往返。 */
static int host_echo(pa_state *state)
{
    char text[256];
    if (string_arg(state, -1, text, sizeof text) != PA_OK) return PA_ERR_INVALID;
    if (pa_pop(state, 1) != PA_OK) return PA_ERR_RUNTIME;
    return push_ok_string(state, text);
}

/* `host_read(name) -> str`：**沙箱边界演示** —— 只允许宿主根下的单段文件名。
 * 越界（`..`、`/`、绝对路径、空名）⇒ `pa_error` ⇒ 脚本收到 `RuntimeError`（可捕获 ✓）。 */
static int host_read(pa_state *state)
{
    host_ctx *ctx = &g_ctx;
    char name[256];
    if (string_arg(state, -1, name, sizeof name) != PA_OK) return PA_ERR_INVALID;
    if (pa_pop(state, 1) != PA_OK) return PA_ERR_RUNTIME;
    if (name[0] == '\0' || strchr(name, '/') != NULL || strchr(name, '\\') != NULL ||
        strstr(name, "..") != NULL) {
        ctx->message = "sandbox: path escapes the host root";
        /* §12.18 C2：宿主侧格式化（C 头 static inline，不新增导出符号）。 */
        return paL_errorf(state, "%s (rule: single-segment)", ctx->message);
    }
    char path[1024];
    if ((size_t)snprintf(path, sizeof path, "%s" PY_PATH_SEP "%s", ctx->root, name) >= sizeof path)
        return pa_error(state, "sandbox: path too long");
    FILE *file = fopen(path, "rb");
    if (file == NULL) return pa_error(state, "sandbox: file not in root");
    char content[4096];
    size_t read = fread(content, 1, sizeof content - 1, file);
    fclose(file);
    content[read] = '\0';
    return push_ok_string(state, content);
}

/* `host_now_ns() -> int`：A2 的验收点 —— 宿主函数内部调 clock vtable 的 `now_ns` 槽。 */
static int host_now_ns(pa_state *state)
{
    int64_t now_ns = 0;
    if (g_clock_vtable.now_ns == NULL) return PA_ERR_NOTIMPLEMENTED;
    if (g_clock_vtable.now_ns(g_clock_vtable.state, &now_ns) != PA_CAP_OK)
        return PA_ERR_RUNTIME;
    return pa_pushinteger(state, now_ns);
}

/* `host_tick() -> int`：**协作式超时** —— 第一次调用开始计时；超预算就 `pa_interrupt`。
 * VM 在下一指令边界返回 `PA_ERR_INTERRUPT`（AB-5①；VM 自己不设壁钟 ✗）。 */
static int host_tick(pa_state *state)
{
    host_ctx *ctx = &g_ctx;
    if (ctx->tick_calls == 0) ctx->started_ms = now_ms();
    ctx->tick_calls += 1;
    if (now_ms() - ctx->started_ms > ctx->deadline_ms) {
        if (pa_interrupt(state) != PA_OK) return PA_ERR_RUNTIME;
    }
    return pa_pushinteger(state, ctx->tick_calls);
}

/* ---- 注册一个宿主函数：名字 + 函数指针 + 内联签名（AB-51） ------------------------ */
static int register_api(pa_state *state, const char *name, pa_host_fn function, int nparams,
                        const char *const *param_names)
{
    pa_param params[4];
    for (int i = 0; i < nparams; ++i) {
        params[i].size = sizeof(pa_param);
        params[i].type_expr = NULL; /* 宿主可省略注解表达式（AB-52） */
        params[i].flags = PA_PARAM_POSITIONAL;
        params[i].default_handle = NULL;
        /* `name` 是 `char *`（AB-52 的 const 语义由运行时只读保证） */
        params[i].name = (char *)param_names[i];
    }
    pa_sig sig = { sizeof(sig), 0, NULL, (uint32_t)nparams, params };
    return pa_register(state, name, function, &sig);
}

/* ---- 小工具：读脚本里设的整型/字符串全局（每个实例独立 ✓） ----------------------- */
static int read_int_global(pa_state *state, const char *name, int64_t *out)
{
    if (pa_getglobal(state, name) != PA_OK) return PA_ERR_RUNTIME;
    if (pa_tointeger(state, -1, out) != PA_OK) return PA_ERR_RUNTIME;
    pa_settop(state, 0);
    return PA_OK;
}

static int read_text_global(pa_state *state, const char *name, char *out, size_t cap)
{
    size_t len = 0;
    if (pa_getglobal(state, name) != PA_OK) return PA_ERR_RUNTIME;
    const char *text = pa_tostring(state, -1, &len);
    if (text == NULL || len + 1 > cap) return PA_ERR_RUNTIME;
    memcpy(out, text, len);
    out[len] = '\0';
    pa_settop(state, 0);
    return PA_OK;
}

static int run_python(pa_state *state, const char *source, const char *chunk)
{
    return pa_exec_string(state, source, -1, chunk, "python", NULL);
}

#ifdef _WIN32
/* Windows 没有 `mkdtemp(3)`：在 `%TEMP%` 下用"时间戳＋序号"建唯一目录（沙箱根）。 */
static int make_temp_dir(char *out, size_t capacity)
{
    char base[MAX_PATH];
    DWORD written = GetTempPathA((DWORD)sizeof base, base);
    if (written == 0 || written >= (DWORD)sizeof base) return -1;
    unsigned long long stamp = (unsigned long long)GetTickCount64();
    for (int attempt = 0; attempt < 64; ++attempt) {
        int length = snprintf(out, capacity, "%spyawa-reference-host-%llu-%d", base, stamp, attempt);
        if (length < 0 || (size_t)length >= capacity) return -1;
        if (CreateDirectoryA(out, NULL) != 0) return 0;
        if (GetLastError() != ERROR_ALREADY_EXISTS) return -1;
    }
    return -1;
}
#endif

int main(void)
{
    memset(&g_ctx, 0, sizeof g_ctx);
#ifdef _WIN32
    if (make_temp_dir(g_ctx.root, sizeof g_ctx.root) != 0) return 1;
#else
    snprintf(g_ctx.root, sizeof g_ctx.root, "/tmp/pyawa-reference-host-XXXXXX");
    if (mkdtemp(g_ctx.root) == NULL) return 1; /* 沙箱根（临时目录，不是业务假设） */
#endif
    g_ctx.deadline_ms = 50;

    char hello_path[1024];
    snprintf(hello_path, sizeof hello_path, "%s" PY_PATH_SEP "hello.txt", g_ctx.root);
    FILE *hello = fopen(hello_path, "wb");
    if (hello == NULL) return 1;
    fputs("hello from host", hello);
    fclose(hello);

    /* ① 创建（版本兼容的宿主结构；能力域一个都不注册 ⇒ 默认封闭） */
    pa_host host = { sizeof(host), PA_ABI_VERSION, NULL };
    pa_state *state = NULL;
    if (pa_create(&host, &state) != PA_OK || state == NULL) return 2;
    /* 版本检查：宿主按自己的头文件校验运行时（主版本相同、次版本不高于运行时 ✓，AB-41）。 */
    if ((pa_abi_version() >> 16) != PA_ABI_MAJOR || pa_abi_version() < PA_ABI_VERSION) return 2;
    printf("abi=ok\n");

    /* ⑦ 能力域 vtable（§12.17 A2）：先声明异步分类（CP-25／AB-34），再注册 clock 域。 */
    if (pa_setcapability_async(state, PA_DOMAIN_CLOCK, PA_ASYNC_OK) != PA_OK) return 20;
    if (pa_setcapability(state, PA_DOMAIN_CLOCK, &g_clock_vtable) != PA_OK) return 20;

    /* ② 宿主定义脚本 API（脚本 → 宿主 → 脚本） */
    static const char *const one_arg[] = { "text" };
    static const char *const two_args[] = { "left", "right" };
    if (register_api(state, "host_sum", host_sum, 2, two_args) != PA_OK) return 3;
    if (register_api(state, "host_echo", host_echo, 1, one_arg) != PA_OK) return 4;
    if (register_api(state, "host_read", host_read, 1, one_arg) != PA_OK) return 5;
    if (register_api(state, "host_tick", host_tick, 0, NULL) != PA_OK) return 6;
    if (register_api(state, "host_now_ns", host_now_ns, 0, NULL) != PA_OK) return 20;

    /* ③⑤ 内存配额：脚本超限时是**可捕获的** MemoryError（AB-65）。 */
    if (pa_set_memory_limit(state, 4 * 1024 * 1024) != PA_OK) return 7;

    /* 脚本：回调宿主、沙箱越界（宿主 API 与未注册能力域两条路）、配额。 */
    const char *main_script =
        "callback = host_sum(40, 2)\n"
        "echoed = host_echo(\"hello\")\n"
        "try:\n"
        "    host_read(\"../etc/passwd\")\n"
        "except:\n"
        "    escape = \"rejected\"\n"
        "try:\n"
        "    inside = host_read(\"hello.txt\")\n"
        "except:\n"
        "    inside = \"error\"\n"
        "try:\n"
        "    big = \"x\" * (1 << 30)\n"
        "except:\n"
        "    quota = \"caught\"\n"
        "now_ns = host_now_ns()\n";
    int main_status = run_python(state, main_script, "reference-host");
    if (main_status != PA_OK) {
        fprintf(stderr, "main_script status=%d msg=%s\n", main_status, pa_errmsg(state));
        return 8;
    }

    int64_t callback = 0;
    char echoed[256], escape[64], inside[256], quota[64];
    if (read_int_global(state, "callback", &callback) != PA_OK) return 9;
    if (read_text_global(state, "echoed", echoed, sizeof echoed) != PA_OK) return 10;
    if (read_text_global(state, "escape", escape, sizeof escape) != PA_OK) return 11;
    if (read_text_global(state, "inside", inside, sizeof inside) != PA_OK) return 12;
    if (read_text_global(state, "quota", quota, sizeof quota) != PA_OK) return 14;
    int64_t now_ns = 0;
    if (read_int_global(state, "now_ns", &now_ns) != PA_OK || now_ns <= 0) return 20;
    printf("callback=%lld\n", (long long)callback);
    printf("echoed=%s\n", echoed);
    printf("sandbox-escape=%s\n", escape);
    printf("sandbox-inside=%s\n", inside);
    printf("quota=%s\n", quota);
    printf("clock=ok\n");

    /* ⑤ 指标：每实例占用（宿主可做看门狗）。 */
    uint64_t used = 0;
    if (pa_memory_used(state, &used) != PA_OK || used == 0) return 16;
    printf("memory-used=%s\n", used > 0 ? "positive" : "zero");

    /* ⑥ 实例隔离：另一个实例看不到本实例的全局（OM-1）。 */
    pa_state *other = NULL;
    if (pa_create(&host, &other) != PA_OK || other == NULL) return 17;

    if (run_python(state, "marker = 7\n", "isolation-a") != PA_OK) return 18;
    if (run_python(other, "marker = marker + 1\n", "isolation-b") != PA_OK)
        printf("instances=isolated\n");
    else
        return 19;

    /* ④ 超时：宿主侧的壁钟预算（VM 只提供协作式中断）。 */
    g_ctx.tick_calls = 0;
    if (run_python(state, "while True:\n    host_tick()\n", "timeout") == PA_ERR_INTERRUPT)
        printf("timeout=interrupted\n");
    else
        return 15;

    /* ⑦ 销毁（AB-57）；即便还有存活实例，宿主负责顺序与时机。 */
    pa_destroy(other);
    pa_destroy(state);
#ifdef _WIN32
    DeleteFileA(hello_path);
    RemoveDirectoryA(g_ctx.root);
#else
    unlink(hello_path);
    rmdir(g_ctx.root);
#endif
    printf("done\n");
    return 0;
}
