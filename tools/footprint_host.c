/* M1 ② 的足迹测量宿主（`PLAN-milestones.md` §6 ②；`DESIGN.md` §13-17 的提示项）。
 *
 * 量三样东西（口径写在 `tools/measure_footprint.py` 的报告里，本文件只负责采数）：
 *   1. **VM 引导延迟**：`pa_create` ＋ `pa_destroy` 的单调时钟时长（N 次，报 median／min）
 *   2. **执行延迟**：同一实例上 `pa_exec_string("x = 1")` 的时长（N 次，报 median／min）
 *   3. **常驻内存**：本进程的 RSS（起点／建实例后／执行后／收尾）与峰值 `VmHWM`
 *
 * 用法：`footprint_host [iterations]`（默认 1000）——打一行 `key=value`，供驱动解析。
 * 构建与运行见 `tools/measure_footprint.py`（缺 `cc` 即红，不降级）。
 */
#include "pa.h"
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <time.h>
#include <unistd.h>

static double now_ns(void)
{
    struct timespec ts;
    clock_gettime(CLOCK_MONOTONIC, &ts);
    return (double)ts.tv_sec * 1e9 + (double)ts.tv_nsec;
}

/* `/proc/self/statm` 的第二个字段是常驻页数 ⇒ 换算成 KiB。 */
static long rss_kb(void)
{
    FILE *file = fopen("/proc/self/statm", "r");
    long total = 0, resident = 0;
    if (file == NULL) return -1;
    if (fscanf(file, "%ld %ld", &total, &resident) != 2) resident = -1;
    fclose(file);
    return resident < 0 ? -1 : resident * (sysconf(_SC_PAGESIZE) / 1024);
}

/* `/proc/self/status` 里的某个 `Foo:` 字段（KiB）。找不到给 -1。 */
static long status_kb(const char *key)
{
    FILE *file = fopen("/proc/self/status", "r");
    char line[256];
    long value = -1;
    if (file == NULL) return -1;
    size_t key_len = strlen(key);
    while (fgets(line, sizeof line, file) != NULL) {
        if (strncmp(line, key, key_len) == 0) {
            if (sscanf(line + key_len, "%ld", &value) != 1) value = -1;
            break;
        }
    }
    fclose(file);
    return value;
}

static int by_double(const void *left, const void *right)
{
    double a = *(const double *)left, b = *(const double *)right;
    return (a > b) - (a < b);
}

static double median(double *samples, long count)
{
    qsort(samples, (size_t)count, sizeof(double), by_double);
    return count % 2 == 1 ? samples[count / 2]
                          : (samples[count / 2 - 1] + samples[count / 2]) / 2.0;
}

int main(int argc, char **argv)
{
    long iterations = argc > 1 ? atol(argv[1]) : 1000;
    if (iterations < 1) iterations = 1;
    double *create_ns = malloc(sizeof(double) * (size_t)iterations);
    double *exec_ns = malloc(sizeof(double) * (size_t)iterations);
    if (create_ns == NULL || exec_ns == NULL) return 1;

    pa_host host = { sizeof(host), PA_ABI_VERSION, NULL };
    pa_state *state = NULL;
    long rss_before = rss_kb();

    for (long index = 0; index < iterations; index++) {
        double start = now_ns();
        if (pa_create(&host, &state) != PA_OK) return 2;
        create_ns[index] = now_ns() - start;
        if (pa_destroy(state) != PA_OK) return 2;
    }
    long rss_after_create = rss_kb();

    /* 常驻一个实例：执行延迟与内存读数都在它上面取 */
    if (pa_create(&host, &state) != PA_OK) return 3;
    for (long index = 0; index < iterations; index++) {
        double start = now_ns();
        if (pa_exec_string(state, "x = 1\n", -1, "footprint", "python") != PA_OK) return 4;
        exec_ns[index] = now_ns() - start;
    }
    long rss_after_exec = rss_kb();
    long peak = status_kb("VmHWM:");
    if (pa_destroy(state) != PA_OK) return 3;
    long rss_end = rss_kb();

    /* 先算完再打印：`median` 会就地排序，最小值就是排序后的首项；printf 的实参求值顺序未定，
     * 不能把 `median(...)` 与 `samples[0]` 写进同一次调用。 */
    double create_median = median(create_ns, iterations);
    double create_min = create_ns[0];
    double exec_median = median(exec_ns, iterations);
    double exec_min = exec_ns[0];

    printf("iterations=%ld abi_size=%zu create_median_ns=%.0f create_min_ns=%.0f "
           "exec_median_ns=%.0f exec_min_ns=%.0f "
           "rss_before_kb=%ld rss_after_create_kb=%ld rss_after_exec_kb=%ld "
           "rss_end_kb=%ld rss_peak_kb=%ld\n",
           iterations, pa_abi_size(), create_median, create_min,
           exec_median, exec_min,
           rss_before, rss_after_create, rss_after_exec, rss_end, peak);
    free(create_ns);
    free(exec_ns);
    return 0;
}
