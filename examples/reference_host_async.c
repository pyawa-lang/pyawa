/* **S3 参考宿主（异步）**：不阻塞 `pa_exec`，宿主 loop 用 `pa_resume` 驱动生成器；
 * 同时注册最小 `event` 能力域 provider（仅演示注册/布局，不参与 VM 调度）。
 * 构建（与 CI fast 的步骤同口径）：
 *   cargo build --release -p pyawa-runtime --lib
 *   cc examples/reference_host_async.c -I crates/pyawa-abi/include \
 *      -L target/release -lpyawa_runtime -Wl,-rpath,$PWD/target/release \
 *      -lpthread -ldl -lm -o /tmp/reference_host_async
 */
#include "pa.h"
#include "pa_capabilities.h"
#include <stdint.h>
#include <stdio.h>

static const char *SRC =
    "def gen():\n"
    "    a = yield 1\n"
    "    b = yield a + 2\n"
    "    return b + 3\n"
    "g = gen()\n";

static pa_cap_status mock_wait(void *state, const pa_cap_event_entry *entries, size_t count,
                               int64_t timeout_ns, pa_cap_event_entry *out, size_t capacity,
                               size_t *out_len, int32_t *errno_out)
{
    (void)state; (void)timeout_ns; (void)errno_out;
    *out_len = 0;
    for (size_t i = 0; i < count; i++) {
        if (entries[i].events & 0x1) { /* POLLIN */
            if (*out_len < capacity) {
                out[*out_len] = entries[i];
                out[*out_len].revents = 0x1;
            }
            (*out_len)++;
        }
    }
    return PA_CAP_OK;
}

static int fail(pa_state *st, const char *what, int rc)
{
    printf("ref-async: FAIL %s rc=%d msg=%s\n", what, rc, st ? pa_errmsg(st) : "(no state)");
    return 1;
}

int main(void)
{
    pa_state *st = paL_newstate();
    if (!st) { printf("ref-async: FAIL newstate\n"); return 1; }
    if (pa_exec_string(st, SRC, -1, "ref-async", "python", NULL) != PA_OK)
        return fail(st, "exec", -1);

    pa_event_vtable ev = {0};
    ev.version = PA_ABI_VERSION;
    ev.size = (uint32_t)sizeof(ev);
    ev.wait = mock_wait;
    if (pa_setcapability_async(st, PA_DOMAIN_EVENT, PA_ASYNC_OK) != PA_OK)
        return fail(st, "event async", -1);
    if (pa_setcapability(st, PA_DOMAIN_EVENT, &ev) != PA_OK)
        return fail(st, "event register", -1);

    int64_t value = 0;
    if (pa_getglobal(st, "g") != PA_OK) return fail(st, "getglobal", -1);
    if (pa_resume(st, 0, 1) != PA_YIELDED) return fail(st, "resume1", -1);
    if (pa_tointeger(st, -1, &value) != PA_OK || value != 1) return fail(st, "yield1", -1);
    printf("ref-async: yield1=%lld\n", (long long)value);

    pa_settop(st, 0);
    if (pa_getglobal(st, "g") != PA_OK) return fail(st, "getglobal", -1);
    if (pa_pushinteger(st, 1) != PA_OK) return fail(st, "push", -1);
    if (pa_resume(st, 1, 1) != PA_YIELDED) return fail(st, "resume2", -1);
    if (pa_tointeger(st, -1, &value) != PA_OK || value != 3) return fail(st, "yield2", -1);
    printf("ref-async: yield2=%lld\n", (long long)value);

    pa_settop(st, 0);
    if (pa_getglobal(st, "g") != PA_OK) return fail(st, "getglobal", -1);
    if (pa_pushinteger(st, 4) != PA_OK) return fail(st, "push", -1);
    if (pa_resume(st, 1, 1) != PA_COMPLETED) return fail(st, "resume3", -1);
    if (pa_tointeger(st, -1, &value) != PA_OK || value != 7) return fail(st, "done", -1);
    printf("ref-async: done=%lld\n", (long long)value);

    pa_settop(st, 0);
    pa_cap_event_entry in = {0}, out = {0};
    in.key = 7;
    in.events = 0x1;
    size_t out_len = 0;
    if (mock_wait(NULL, &in, 1, 0, &out, 1, &out_len, NULL) != PA_CAP_OK || out_len != 1)
        return fail(st, "event wait", -1);
    printf("ref-async: event=%llu\n", (unsigned long long)out_len);

    pa_destroy(st);
    printf("ref-async: OK\n");
    return 0;
}
