/* MS-21／T-AB-1：M1 判据的 ≤50 行 C 示例（PLAN-milestones.md §6 ①）。
 *
 * 创建实例 → 执行脚本 → 注入宿主函数 → 再执行一段脚本（它调到刚注入的宿主函数）
 * → 取回一个值 → 销毁实例。判据里的箭头是**能力清单**、不是严格次序：要验
 * "脚本调到宿主函数"，注入就只能发生在**被调用的那次执行之前**（AB-24）。
 *
 * 构建与运行（T-AB-1 的验收就是这一串；见 tests/ci/t_ab_1.py）：
 *   cargo build -p pyawa-abi
 *   cc examples/m1.c -I crates/pyawa-abi/include target/debug/libpyawa_abi.a \
 *      -lpthread -ldl -lm -o /tmp/m1 && /tmp/m1
 */
#include "pa.h"
#include <stdio.h>

static int host_add(pa_state *state) /* 宿主函数：两整数相加，结果留在栈顶 */
{
    int64_t a = 0, b = 0;
    if (pa_tointeger(state, -2, &a) != PA_OK) return PA_ERR_INVALID;
    if (pa_tointeger(state, -1, &b) != PA_OK) return PA_ERR_INVALID;
    if (pa_pop(state, 2) != PA_OK) return PA_ERR_RUNTIME;
    return pa_pushinteger(state, a + b);
}

int main(void)
{
    pa_host host = { sizeof(host), PA_ABI_VERSION, NULL };
    pa_state *state = NULL;
    int64_t value = 0;
    if (pa_create(&host, &state) != PA_OK || state == NULL) return 1;

    if (pa_exec_string(state, "x = 40\n", -1, "m1", "python") != PA_OK) return 2;
    pa_param params[2] = {
        { sizeof(pa_param), "left", NULL, PA_PARAM_POSITIONAL, NULL },
        { sizeof(pa_param), "right", NULL, PA_PARAM_POSITIONAL, NULL },
    };
    pa_sig sig = { sizeof(sig), 0, NULL, 2, params };
    if (pa_register(state, "host_add", host_add, &sig) != PA_OK) return 3;

    if (pa_exec_string(state, "y = host_add(x, 2)\n", -1, "m1", "pyawa") != PA_OK) return 4;
    if (pa_getglobal(state, "y") != PA_OK) return 5;
    if (pa_tointeger(state, -1, &value) != PA_OK) return 6;
    printf("%lld\n", (long long)value);
    pa_destroy(state);
    return value == 42 ? 0 : 7;
}
