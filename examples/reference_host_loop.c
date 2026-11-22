/* **P0-3 宿主 loop 参考适配器（Linux）**：真实 `poll(2)` 事件源＋任务表＋超时。
 *
 * 不把事件循环塞进 VM：宿主注册 `event` provider，`await` 宿主对象让出 token（此处＝fd），
 * 宿主 loop 用 provider 的 `wait` 等就绪，再 `pa_resume` 送值。演示多任务、超时。
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

static const char *SRC =
    "async def f(x):\n"
    "    return (await x) + 1\n"
    "async def g(x):\n"
    "    return (await x) + 2\n"
    "async def h(x):\n"
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
static int cancel_calls = 0;
static void host_await_cancel(pa_state *state, void *payload, void *context, int reason)
{
    (void)state; (void)payload; (void)context; (void)reason;
    cancel_calls++;
}

static int fail(pa_state *st, const char *what, int rc)
{
    printf("ref-loop: FAIL %s rc=%d msg=%s\n", what, rc, st ? pa_errmsg(st) : "(no state)");
    return 1;
}

/* 一个任务：全局名＋目的值。 */
typedef struct Task {
    const char *global;
    const char *name;
    int64_t value;
} Task;

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

    int pipes[2][2];
    int timeout_pipe[2];
    AwaitPayload *payloads[2];
    const char *globals[2] = {"x1", "x2"};
    Task tasks[2] = {{"c1", "f", 41}, {"c2", "g", 42}};
    for (int i = 0; i < 2; i++) {
        if (pipe(pipes[i]) != 0) return fail(st, "pipe", -1);
        void *payload = NULL;
        if (pa_newhandle(st, -1, &payload) != PA_OK) return fail(st, "newhandle", -1);
        payloads[i] = (AwaitPayload *)payload;
        payloads[i]->fd = pipes[i][0];
        if (pa_setglobal(st, globals[i]) != PA_OK) return fail(st, "setglobal", -1);
    }
    /* 超时示例的第三个宿主对象也要在类型还在栈上时创建。 */
    if (pipe(timeout_pipe) != 0) return fail(st, "pipe timeout", -1);
    void *timeout_payload = NULL;
    if (pa_newhandle(st, -1, &timeout_payload) != PA_OK) return fail(st, "newhandle timeout", -1);
    ((AwaitPayload *)timeout_payload)->fd = timeout_pipe[0];
    if (pa_setglobal(st, "x3") != PA_OK) return fail(st, "setglobal x3", -1);
    pa_settop(st, 0);

    /* 启动两个协程（任务表）。 */
    for (int i = 0; i < 2; i++) {
        const char *fn = i == 0 ? "f" : "g";
        if (pa_getglobal(st, fn) != PA_OK) return fail(st, "getglobal fn", -1);
        if (pa_getglobal(st, globals[i]) != PA_OK) return fail(st, "getglobal arg", -1);
        if (pa_call(st, 1, 1) != PA_OK) return fail(st, "call", -1);
        if (pa_setglobal(st, tasks[i].global) != PA_OK) return fail(st, "setglobal task", -1);
    }
    pa_settop(st, 0);

    /* 每个任务让出 token（fd）；宿主登记到任务表。 */
    int64_t tokens[2] = {0, 0};
    for (int i = 0; i < 2; i++) {
        if (pa_getglobal(st, tasks[i].global) != PA_OK) return fail(st, "getglobal task", -1);
        if (pa_resume(st, 0, 1) != PA_YIELDED) return fail(st, "first resume", -1);
        if (pa_tointeger(st, -1, &tokens[i]) != PA_OK || tokens[i] < 0)
            return fail(st, "token", -1);
        pa_settop(st, 0);
    }
    printf("ref-loop: tasks=2\n");

    /* 宿主 loop：等两个 fd 就绪。先写一个字节到两个 pipe。 */
    unsigned char byte = 1;
    for (int i = 0; i < 2; i++) {
        if (write(pipes[i][1], &byte, 1) != 1) return fail(st, "write pipe", -1);
    }
    pa_cap_event_entry in[2] = {{0}}, out[2] = {{0}};
    for (int i = 0; i < 2; i++) {
        in[i].key = (uint64_t)tokens[i];
        in[i].events = POLLIN;
    }
    size_t out_len = 0;
    if (ev.wait(NULL, in, 2, 0, out, 2, &out_len, NULL) != PA_CAP_OK || out_len != 2)
        return fail(st, "event wait", -1);
    printf("ref-loop: ready=%llu\n", (unsigned long long)out_len);

    /* 恢复两个任务。 */
    for (int i = 0; i < 2; i++) {
        if (pa_getglobal(st, tasks[i].global) != PA_OK) return fail(st, "getglobal task2", -1);
        if (pa_pushinteger(st, tasks[i].value) != PA_OK) return fail(st, "push", -1);
        if (pa_resume(st, 1, 1) != PA_COMPLETED) return fail(st, "complete", -1);
        int64_t value = 0;
        if (pa_tointeger(st, -1, &value) != PA_OK) return fail(st, "value", -1);
        printf("ref-loop: %s=%lld\n", tasks[i].name, (long long)value);
        pa_settop(st, 0);
    }

    /* 超时示例：第三个 fd 无数据 ⇒ 10ms 后返回 0 项。 */
    pa_cap_event_entry t_in = {0}, t_out = {0};
    t_in.key = (uint64_t)timeout_pipe[0];
    t_in.events = POLLIN;
    if (ev.wait(NULL, &t_in, 1, 10 * 1000000LL, &t_out, 1, &out_len, NULL) != PA_CAP_OK || out_len != 0)
        return fail(st, "timeout wait", -1);
    printf("ref-loop: timeout=0\n");

    /* 取消示例：启动第三个任务，让出 token 后用 pa_throw 注入异常取消。 */
    if (pa_getglobal(st, "h") != PA_OK) return fail(st, "getglobal h", -1);
    if (pa_getglobal(st, "x3") != PA_OK) return fail(st, "getglobal x3", -1);
    if (pa_call(st, 1, 1) != PA_OK) return fail(st, "call h", -1);
    if (pa_setglobal(st, "c3") != PA_OK) return fail(st, "setglobal c3", -1);
    pa_settop(st, 0);
    if (pa_getglobal(st, "c3") != PA_OK) return fail(st, "getglobal c3", -1);
    if (pa_resume(st, 0, 1) != PA_YIELDED) return fail(st, "c3 token", -1);
    pa_settop(st, 0);
    if (pa_getglobal(st, "c3") != PA_OK) return fail(st, "getglobal c3b", -1);
    if (pa_getglobal(st, "exc") != PA_OK) return fail(st, "getglobal exc", -1);
    int cancelled = pa_throw(st, 1, 1);
    if (cancelled == PA_OK) return fail(st, "pa_throw returned OK", -1);
    printf("ref-loop: cancel-status=%d\n", cancelled);
    printf("ref-loop: cancel-callback=%d\n", cancel_calls);
    pa_settop(st, 0);

    pa_destroy(st);
    printf("ref-loop: OK\n");
    return 0;
}
