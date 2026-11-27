/* **P0-3 宿主 loop 参考适配器（Linux）**：真实 `poll(2)` 事件源＋任务表＋超时，
 * 并用宿主自己的循环驱动含 `await` 的协程（S3）。演示的场景：
 *   1. **正常完成**：脚本 `await` 宿主对象 ⇒ VM 让出 token（此处＝真实 fd），宿主 loop 用
 *      event provider 的 `wait` 等就绪，再 `pa_resume` 送值 ⇒ `PA_COMPLETED`；
 *   2. **多次 resume**：同一协程连续 `await` 两次 ⇒ 两次 `PA_YIELDED`（同一 token）后完成；
 *   3. **错误传播**：恢复后脚本自己抛异常 ⇒ `PA_ERR_RUNTIME`＋`pa_errmsg`；
 *   4. **THROW 传播**：`pa_throw` 把宿主异常送回挂起的 `await` ⇒ 未捕获 ⇒ `PA_ERR_RUNTIME`，
 *      且 VM 派发 `await_cancel_fn(THROW)`（`AB-89`／`AB-90`）；
 *   5. **取消/INTERRUPT**：`pa_interrupt` 在恢复边界生效 ⇒ `PA_ERR_INTERRUPT`＋
 *      `await_cancel_fn(INTERRUPT)`；取消后不可再 resume（`PA_ERR_INVALID`）；按 `AB-92`
 *      `pa_clear_interrupt` 后同实例其他协程仍可继续驱动；
 *   6. **销毁**：协程仍挂起在宿主 awaitable 上时 `pa_destroy` ⇒ `await_cancel_fn(DESTROY)`。
 *
 * 不把事件循环塞进 VM：宿主注册 `event` provider，`await` 宿主对象让出 token（此处＝fd），
 * 宿主 loop 用 provider 的 `wait` 等就绪，再 `pa_resume` 送值；取消/异常由宿主按需注入。
 *
 * 构建：
 *   cargo build --release -p pyawa-runtime --lib
 *   cc examples/reference_host_loop.c -I crates/pyawa-abi/include \
 *      -L target/release -lpyawa_runtime -Wl,-rpath,$PWD/target/release \
 *      -lpthread -ldl -lm -o /tmp/reference_host_loop
 */
#include "pa.h"
#include "pa_capabilities.h"
#include <errno.h>
#include <poll.h>
#include <stdint.h>
#include <stdio.h>
#include <string.h>
#include <unistd.h>

typedef struct AwaitPayload {
    int fd; /* 就绪事件 key ⇒ 真实 fd */
} AwaitPayload;

/* 每个宿主 awaitable 一个（`OBJ_*` 是下标）：脚本全局名 → host object → pipe。 */
enum {
    OBJ_F = 0,       /* 正常完成（f） */
    OBJ_G,           /* 正常完成（g） */
    OBJ_INTERRUPT,   /* 取消/INTERRUPT（h） */
    OBJ_MULTI,       /* 多次 await/resume（multi） */
    OBJ_ERROR,       /* 恢复后脚本抛异常（raise_after） */
    OBJ_THROW,       /* pa_throw 注入异常（uncaught） */
    OBJ_RECOVER,     /* 中断清理后继续驱动（g） */
    OBJ_DESTROY,     /* 销毁时仍挂起（h） */
    OBJ_COUNT
};
static const char *OBJ_NAMES[OBJ_COUNT] = {"x1", "x2", "x3", "x4", "x5", "x6", "x7", "x8"};

static const char *SRC =
    "async def f(x):\n"
    "    return (await x) + 1\n"
    "async def g(x):\n"
    "    return (await x) + 2\n"
    "async def h(x):\n"
    "    return (await x) + 3\n"
    "async def multi(x):\n"
    "    a = await x\n"
    "    b = await x\n"
    "    return a + b + 1\n"
    "async def raise_after(x):\n"
    "    await x\n"
    "    raise LookupError('after')\n"
    "async def uncaught(x):\n"
    "    return (await x) + 3\n"
    "exc = RuntimeError('cancelled')\n";

/* event provider：把 key 当 fd，用 poll(2) 真等。 */
static pa_cap_status native_wait(void *state, const pa_cap_event_entry *entries, size_t count,
                                 int64_t timeout_ns, pa_cap_event_entry *out, size_t capacity,
                                 size_t *out_len, int32_t *errno_out)
{
    (void)state;
    if (count > 64) {
        if (errno_out) *errno_out = EINVAL;
        return PA_CAP_MACHINE;
    }
    struct pollfd fds[64];
    for (size_t i = 0; i < count; i++) {
        fds[i].fd = (int)entries[i].key;
        fds[i].events = (short)entries[i].events;
        fds[i].revents = 0;
    }
    int timeout_ms = -1;
    if (timeout_ns >= 0) timeout_ms = (int)(timeout_ns / 1000000);
    int rc = poll(fds, (nfds_t)count, timeout_ms);
    if (rc < 0) {
        if (errno_out) *errno_out = errno;
        return PA_CAP_MACHINE;
    }
    size_t n = 0;
    for (size_t i = 0; i < count; i++) {
        if (fds[i].revents != 0) {
            if (n < capacity) {
                out[n] = entries[i];
                out[n].revents = fds[i].revents;
            }
            n++;
        }
    }
    *out_len = n;
    return PA_CAP_OK;
}

static pa_cap_status native_notify(void *state, uint64_t key, int32_t events, int32_t *errno_out)
{
    (void)state; (void)events;
    unsigned char byte = 1;
    ssize_t n = write((int)key, &byte, 1);
    if (n != 1) {
        if (errno_out) *errno_out = errno;
        return PA_CAP_MACHINE;
    }
    return PA_CAP_OK;
}

static pa_cap_status native_cancel(void *state, uint64_t key, int32_t *errno_out)
{
    (void)state; (void)key; (void)errno_out;
    return PA_CAP_OK;
}

static void host_dealloc(void *payload) { (void)payload; }
static void host_traverse(void *payload, void *context, pa_host_visit visit)
{
    (void)payload; (void)context; (void)visit;
}

static int host_await(pa_state *state, void *payload, void *context, uint64_t *token_out)
{
    (void)state; (void)context;
    *token_out = (uint64_t)((AwaitPayload *)payload)->fd;
    return PA_OK;
}

/* `AB-89`：取消/销毁/THROW 时 VM 回调；只记录最近一次 reason，演示按帧派发。 */
static int last_cancel_reason = -1;
static void host_await_cancel(pa_state *state, void *payload, void *context, int reason)
{
    (void)state; (void)payload; (void)context;
    last_cancel_reason = reason;
}

static int fail(pa_state *st, const char *what, int rc)
{
    printf("ref-loop: FAIL %s rc=%d msg=%s\n", what, rc, st ? pa_errmsg(st) : "(no state)");
    return 1;
}

/* 栈顶是 `pa_resume` 交回的 await token ⇒ 读成 u64 并清栈。
 * token 是 **u64**（`AB-88`：不按 i64 截断）⇒ 走 `pa_tointstring` 读十进制全量；
 * 该视图是"指针＋长度"借用（`AB-15`／`AB-62`，**不保证 NUL 结尾**）⇒ 按 len 逐位解析。 */
static int pop_token(pa_state *st, uint64_t *token_out)
{
    size_t len = 0;
    const char *text = pa_tointstring(st, -1, &len);
    if (text == NULL || len == 0) return 0;
    uint64_t value = 0;
    for (size_t i = 0; i < len; i++) {
        char digit = text[i];
        if (digit < '0' || digit > '9') return 0;
        if (value > (UINT64_MAX - (uint64_t)(digit - '0')) / 10) return 0;
        value = value * 10 + (uint64_t)(digit - '0');
    }
    *token_out = value;
    pa_settop(st, 0);
    return 1;
}

/* 启动协程：栈 `[fn, obj]` ⇒ `pa_call` ⇒ 协程存全局 `cg`（任务表的"任务句柄"）。 */
static int spawn(pa_state *st, const char *fn, const char *obj, const char *cg)
{
    if (pa_getglobal(st, fn) != PA_OK) return fail(st, "getglobal fn", -1);
    if (pa_getglobal(st, obj) != PA_OK) return fail(st, "getglobal obj", -1);
    if (pa_call(st, 1, 1) != PA_OK) return fail(st, "call", -1);
    if (pa_setglobal(st, cg) != PA_OK) return fail(st, "setglobal coroutine", -1);
    pa_settop(st, 0);
    return 0;
}

/* 首次恢复（send None）：必须让出宿主 await token。 */
static int first_yield(pa_state *st, const char *cg, uint64_t *token_out)
{
    if (pa_getglobal(st, cg) != PA_OK) return fail(st, "getglobal coroutine", -1);
    int rc = pa_resume(st, 0, 1);
    if (rc != PA_YIELDED) return fail(st, "first resume", rc);
    if (!pop_token(st, token_out)) return fail(st, "token", -1);
    return 0;
}

/* 送一个整数并断言状态码；栈保持（`PA_YIELDED` 栈顶＝token，`PA_COMPLETED` 栈顶＝完成值）。 */
static int resume_expect(pa_state *st, const char *cg, int64_t value, int expected_rc)
{
    if (pa_getglobal(st, cg) != PA_OK) return fail(st, "getglobal coroutine", -1);
    if (pa_pushinteger(st, value) != PA_OK) return fail(st, "push value", -1);
    int rc = pa_resume(st, 1, 1);
    if (rc != expected_rc) return fail(st, "resume", rc);
    return 0;
}

/* 读栈顶完成值并清栈。 */
static int top_completed(pa_state *st, int64_t *value_out)
{
    if (pa_tointeger(st, -1, value_out) != PA_OK) return fail(st, "result", -1);
    pa_settop(st, 0);
    return 0;
}

/* 宿主 loop 的"等一批 token 就绪"：走注册的 `event` provider（真 poll）。 */
static int wait_keys(const pa_event_vtable *ev, const uint64_t *keys, size_t count,
                     int64_t timeout_ns, size_t *ready_out)
{
    if (count > 8) return 0;
    pa_cap_event_entry in[8] = {{0}};
    pa_cap_event_entry out[8] = {{0}};
    for (size_t i = 0; i < count; i++) {
        in[i].key = keys[i];
        in[i].events = POLLIN;
    }
    if (ev->wait(NULL, in, count, timeout_ns, out, count, ready_out, NULL) != PA_CAP_OK)
        return 0;
    return 1;
}

/* 外部就绪在本示例里用"往 pipe 写一个字节"模拟。 */
static int signal_fd(int write_fd)
{
    unsigned char byte = 1;
    return write(write_fd, &byte, 1) == 1;
}

int main(void)
{
    pa_state *st = paL_newstate();
    if (!st) { printf("ref-loop: FAIL newstate\n"); return 1; }
    if (pa_exec_string(st, SRC, -1, "ref-loop", "python", NULL) != PA_OK)
        return fail(st, "exec", -1);

    pa_event_vtable ev = {0};
    ev.version = PA_ABI_VERSION;
    ev.size = (uint32_t)sizeof(ev);
    ev.wait = native_wait;
    ev.notify = native_notify;
    ev.cancel = native_cancel;
    if (pa_setcapability_async(st, PA_DOMAIN_EVENT, PA_ASYNC_OK) != PA_OK)
        return fail(st, "event async", -1);
    if (pa_setcapability(st, PA_DOMAIN_EVENT, &ev) != PA_OK)
        return fail(st, "event register", -1);

    /* 宿主类型：payload 里放 fd；await_fn 返回 fd 作 token。 */
    pa_sig sig = {0};
    sig.size = sizeof(sig);
    if (pa_newtype(st, "HostAwaitFd", 0, sizeof(AwaitPayload), host_dealloc, host_traverse, &sig) != PA_OK)
        return fail(st, "newtype", -1);
    pa_type_ops ops = {0};
    ops.size = sizeof(ops);
    ops.await_fn = host_await;
    ops.await_cancel_fn = host_await_cancel;
    if (pa_set_type_ops(st, -1, &ops) != PA_OK) return fail(st, "type-ops", -1);

    /* 每个场景一个宿主对象（类型还在栈上时创建），按需创建 pipe。 */
    int pipes[OBJ_COUNT][2];
    AwaitPayload *payloads[OBJ_COUNT];
    for (int i = 0; i < OBJ_COUNT; i++) {
        if (pipe(pipes[i]) != 0) return fail(st, "pipe", -1);
        void *payload = NULL;
        if (pa_newhandle(st, -1, &payload) != PA_OK) return fail(st, "newhandle", -1);
        payloads[i] = (AwaitPayload *)payload;
        payloads[i]->fd = pipes[i][0];
        if (pa_setglobal(st, OBJ_NAMES[i]) != PA_OK) return fail(st, "setglobal host object", -1);
    }
    int timeout_pipe[2];
    if (pipe(timeout_pipe) != 0) return fail(st, "pipe timeout", -1);
    pa_settop(st, 0);

    /* ---- 1) 正常完成：三个任务让出 token（fd），宿主 poll 就绪后 resume 送值。 ---- */
    if (spawn(st, "f", "x1", "c1")) return 1;
    if (spawn(st, "g", "x2", "c2")) return 1;
    if (spawn(st, "multi", "x4", "c4")) return 1;
    uint64_t tokens[3] = {0, 0, 0};
    if (first_yield(st, "c1", &tokens[0])) return 1;
    if (first_yield(st, "c2", &tokens[1])) return 1;
    if (first_yield(st, "c4", &tokens[2])) return 1;
    printf("ref-loop: tasks=3\n");

    /* 外部就绪：f/g/multi 的 pipe 各写一个字节；multi 第二次 await 复用同一可读 fd。 */
    if (!signal_fd(pipes[OBJ_F][1]) || !signal_fd(pipes[OBJ_G][1]) ||
        !signal_fd(pipes[OBJ_MULTI][1]))
        return fail(st, "write pipe", -1);
    size_t ready = 0;
    if (!wait_keys(&ev, tokens, 3, 0, &ready)) return fail(st, "event wait", -1);
    if (ready != 3) return fail(st, "event wait ready", (int)ready);
    printf("ref-loop: ready=%llu\n", (unsigned long long)ready);

    int64_t value = 0;
    if (resume_expect(st, "c1", 41, PA_COMPLETED)) return 1;
    if (top_completed(st, &value)) return 1;
    printf("ref-loop: f=%lld\n", (long long)value);
    if (resume_expect(st, "c2", 42, PA_COMPLETED)) return 1;
    if (top_completed(st, &value)) return 1;
    printf("ref-loop: g=%lld\n", (long long)value);

    /* ---- 2) 多次 resume：multi 第二次 await 再次让出同一 token，宿主再等再送值。 ---- */
    if (resume_expect(st, "c4", 1, PA_YIELDED)) return 1;
    uint64_t token2 = 0;
    if (!pop_token(st, &token2)) return fail(st, "second token", -1);
    if (token2 != tokens[2]) return fail(st, "second token identity", -1);
    printf("ref-loop: multi-yields=2\n");
    if (!wait_keys(&ev, &token2, 1, 0, &ready)) return fail(st, "event wait multi", -1);
    if (ready != 1) return fail(st, "event wait multi ready", (int)ready);
    if (resume_expect(st, "c4", 2, PA_COMPLETED)) return 1;
    if (top_completed(st, &value)) return 1;
    printf("ref-loop: multi=%lld\n", (long long)value);

    /* ---- 3) 错误传播：恢复后脚本抛 LookupError ⇒ PA_ERR_RUNTIME＋pa_errmsg。 ---- */
    if (spawn(st, "raise_after", "x5", "c5")) return 1;
    uint64_t error_token = 0;
    if (first_yield(st, "c5", &error_token)) return 1;
    if (!signal_fd(pipes[OBJ_ERROR][1])) return fail(st, "write pipe error", -1);
    if (!wait_keys(&ev, &error_token, 1, 0, &ready) || ready != 1)
        return fail(st, "event wait error", -1);
    if (resume_expect(st, "c5", 1, PA_ERR_RUNTIME)) return 1;
    printf("ref-loop: error-status=%d\n", PA_ERR_RUNTIME);
    printf("ref-loop: error-msg=%s\n", pa_errmsg(st)); /* 借用视图：不再调 pa_* 之后才失效（AB-48） */
    pa_settop(st, 0);

    /* ---- 4) THROW 传播：宿主把 RuntimeError 送回挂起的 await ⇒ 未捕获 ⇒ PA_ERR_RUNTIME。 ---- */
    last_cancel_reason = -1;
    if (spawn(st, "uncaught", "x6", "c6")) return 1;
    uint64_t throw_token = 0;
    if (first_yield(st, "c6", &throw_token)) return 1;
    if (pa_getglobal(st, "c6") != PA_OK) return fail(st, "getglobal c6", -1);
    if (pa_getglobal(st, "exc") != PA_OK) return fail(st, "getglobal exc", -1);
    int rc = pa_throw(st, 1, 1);
    if (rc != PA_ERR_RUNTIME) return fail(st, "throw", rc);
    printf("ref-loop: throw-status=%d\n", rc);
    printf("ref-loop: throw-reason=%d\n", last_cancel_reason);
    pa_settop(st, 0);

    /* ---- 5) 取消/INTERRUPT：中断在恢复边界生效，取消后协程不可再 resume。 ---- */
    last_cancel_reason = -1;
    if (spawn(st, "h", "x3", "c3")) return 1;
    uint64_t interrupt_token = 0;
    if (first_yield(st, "c3", &interrupt_token)) return 1;
    if (pa_interrupt(st) != PA_OK) return fail(st, "interrupt", -1);
    if (resume_expect(st, "c3", 7, PA_ERR_INTERRUPT)) return 1;
    printf("ref-loop: interrupt-status=%d\n", PA_ERR_INTERRUPT);
    printf("ref-loop: interrupt-reason=%d\n", last_cancel_reason);
    pa_settop(st, 0);
    /* AB-92：中断是**实例级**的，一个任务被中断后必须先清掉，才能继续驱动同实例其他协程。 */
    if (pa_clear_interrupt(st) != PA_OK) return fail(st, "clear interrupt", -1);
    if (resume_expect(st, "c3", 7, PA_ERR_INVALID)) return 1;
    printf("ref-loop: after-cancel=%d\n", PA_ERR_INVALID);
    pa_settop(st, 0);
    /* 取消只作用于目标协程：清掉中断后，同实例的新协程照常完成。 */
    if (spawn(st, "g", "x7", "c7")) return 1;
    uint64_t recover_token = 0;
    if (first_yield(st, "c7", &recover_token)) return 1;
    if (!signal_fd(pipes[OBJ_RECOVER][1])) return fail(st, "write pipe recover", -1);
    if (!wait_keys(&ev, &recover_token, 1, 0, &ready) || ready != 1)
        return fail(st, "event wait recover", -1);
    if (resume_expect(st, "c7", 42, PA_COMPLETED)) return 1;
    if (top_completed(st, &value)) return 1;
    printf("ref-loop: recovered=%lld\n", (long long)value);

    /* ---- 超时：fd 无数据 ⇒ 到点返回空集合（不是取消）。 ---- */
    uint64_t timeout_key = (uint64_t)timeout_pipe[0];
    if (!wait_keys(&ev, &timeout_key, 1, 10 * 1000000LL, &ready)) return fail(st, "timeout wait", -1);
    if (ready != 0) return fail(st, "timeout ready", (int)ready);
    printf("ref-loop: timeout=0\n");

    /* ---- 6) 销毁：仍挂起在宿主 awaitable 上的协程 ⇒ pa_destroy 派发 DESTROY。 ---- */
    last_cancel_reason = -1;
    if (spawn(st, "h", "x8", "c8")) return 1;
    uint64_t destroy_token = 0;
    if (first_yield(st, "c8", &destroy_token)) return 1;
    pa_destroy(st);
    printf("ref-loop: destroy-reason=%d\n", last_cancel_reason);
    printf("ref-loop: OK\n");
    return 0;
}
