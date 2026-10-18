/* 足迹测量宿主（M1 ② 的裸档 ＋ M5 `P3-28` ③a 的单进程多实例档）。
 *
 * 三种模式，都只采数、不打结论（口径写在 `tools/measure_footprint.py` 的报告里）：
 *
 * **① 裸档**（`footprint_host [iterations]`，默认 1000）——M1 的既有口径：
 *   1. **VM 引导延迟**：`pa_create` ＋ `pa_destroy` 的单调时钟时长（N 次，报 median／min）
 *   2. **执行延迟**：同一实例上 `pa_exec_string("x = 1")` 的时长（N 次，报 median／min）
 *   3. **常驻内存**：本进程的 RSS（起点／建实例后／执行后／收尾）与峰值 `VmHWM`
 *
 * **③a 单进程多实例档**（`footprint_host --multi <instances> <iterations>`）：
 *   同时建 N 个 `pa_state` 并全存活 ⇒ 逐实例建完后的 RSS 曲线（`rss_after_<k>_kb`，
 *   供派生"每实例边际"）＋ 总常驻 ＋ 建 N 个实例的墙钟 ＋ 每个实例各跑 `iterations` 次
 *   `pa_exec_string("x = 1")` 的墙钟。**顺序执行、不开线程** ⇒ 这一档没有并行度（并行度=1）；
 *   并行度由驱动用"多进程各一实例"那档量（见 `tools/measure_footprint.py`）。
 *
 * 用法：`footprint_host [iterations]`、`footprint_host --multi <instances> <iterations>`、
 * `footprint_host --spawn <program> [参数…]`——打一行 `key=value`，供驱动解析。
 *
 * **②／③b 的进程级峰值 RSS 为什么要 `--spawn`**：驱动是 Python，若直接 `fork` 再 `exec`，
 * `wait4` 的 `ru_maxrss` 会把**父进程（Python 解释器）fork 时的 RSS** 也算进子进程的
 * high-water mark（实测：裸 CLI 真峰 ~6.5 MB，从 13 MB 的 Python 父进程 fork 后报 12.6 MB ✗）。
 * 本宿主是 ~2.4 MB 的小 C 进程 ⇒ 由它 fork/exec 目标，`ru_maxrss = max(2.4 MB, 目标真峰)`
 * ＝目标真峰（本档目标都 ≥6 MB ✓）。它是**测量脚手架**，不是被测对象。
 *
 * 构建与运行见 `tools/measure_footprint.py`（缺 `cc` 即红，不降级）。
 */
#include "pa.h"
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <sys/resource.h>
#include <sys/wait.h>
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

/* **③a 单进程多实例档**（`--multi <instances> <iterations>`）：同时建 N 个实例并全存活，
 * 采"逐实例建完后的 RSS 曲线 ＋ 总常驻 ＋ 建 N 个的墙钟 ＋ 各自干活的墙钟"。
 * 全部在**一个线程**里顺序跑 ⇒ 无并行（并行度=1）；并行度由驱动的多进程档量。 */
static int multi_instance_mode(long instances, long iterations)
{
    if (instances < 1) instances = 1;
    if (iterations < 1) iterations = 1;
    double *create_ns = malloc(sizeof(double) * (size_t)instances);
    double *exec_ns = malloc(sizeof(double) * (size_t)instances * (size_t)iterations);
    long *rss_after = malloc(sizeof(long) * (size_t)instances);
    pa_state **states = calloc((size_t)instances, sizeof(pa_state *));
    if (create_ns == NULL || exec_ns == NULL || rss_after == NULL || states == NULL) return 1;

    pa_host host = { sizeof(host), PA_ABI_VERSION, NULL };
    long rss_before = rss_kb();
    double create_total = 0.0;
    for (long index = 0; index < instances; index++) {
        double start = now_ns();
        if (pa_create(&host, &states[index]) != PA_OK) return 2;
        create_ns[index] = now_ns() - start;
        create_total += create_ns[index];
        rss_after[index] = rss_kb();
    }
    long rss_after_all = rss_after[instances - 1];

    /* 每个实例各干 iterations 份活：同一份最小脚本，口径与裸档一致。 */
    double work_total = 0.0;
    long sample = 0;
    for (long index = 0; index < instances; index++) {
        for (long round = 0; round < iterations; round++) {
            double start = now_ns();
            if (pa_exec_string(states[index], "x = 1\n", -1, "footprint", "python", NULL) != PA_OK) return 4;
            exec_ns[sample] = now_ns() - start;
            work_total += exec_ns[sample];
            sample++;
        }
    }
    long peak = status_kb("VmHWM:");
    for (long index = instances; index-- > 0;) {
        if (pa_destroy(states[index]) != PA_OK) return 3;
    }
    long rss_end = rss_kb();

    /* 先算完再打印：`median` 会就地排序，最小值＝排序后首项；printf 实参求值顺序未定。 */
    double create_median = median(create_ns, instances);
    double create_min = create_ns[0];
    double exec_median = median(exec_ns, instances * iterations);
    double exec_min = exec_ns[0];

    printf("mode=multi abi_size=%zu instances=%ld iterations=%ld "
           "create_total_ns=%.0f create_median_ns=%.0f create_min_ns=%.0f "
           "work_total_ns=%.0f exec_median_ns=%.0f exec_min_ns=%.0f "
           "rss_before_kb=%ld rss_after_all_kb=%ld rss_peak_kb=%ld rss_end_kb=%ld",
           pa_abi_size(), instances, iterations,
           create_total, create_median, create_min,
           work_total, exec_median, exec_min,
           rss_before, rss_after_all, peak, rss_end);
    for (long index = 0; index < instances; index++) {
        printf(" rss_after_%ld_kb=%ld", index + 1, rss_after[index]);
    }
    printf("\n");
    free(create_ns);
    free(exec_ns);
    free(rss_after);
    free(states);
    return 0;
}

/* **测量脚手架**（`--spawn <program> [args…]`）：本进程 fork/exec 目标子进程，
 * 报**目标**的墙钟（fork 前 → wait4 返回，单调时钟）与 `wait4` 的 `ru_maxrss`（KiB）。
 * 为什么需要它见文件头（Python 父进程的 RSS 会污染 `ru_maxrss`）。目标输出直接继承本进程的
 * stdout/stderr（驱动捕获），本行是最后一行。目标退出码>0 也照实打出来，由驱动判红。 */
static int spawn_mode(char **command)
{
    double start = now_ns();
    pid_t pid = fork();
    if (pid < 0) return 1;
    if (pid == 0) {
        execv(command[0], command);
        _exit(127);
    }
    int status = 0;
    struct rusage usage;
    if (wait4(pid, &status, 0, &usage) < 0) return 1;
    double wall = now_ns() - start;
    int code = WIFEXITED(status) ? WEXITSTATUS(status) : 128 + WTERMSIG(status);
    printf("mode=spawn abi_size=%zu wall_ns=%.0f maxrss_kb=%ld exit_code=%d\n",
           pa_abi_size(), wall, (long)usage.ru_maxrss, code);
    return 0;
}

int main(int argc, char **argv)
{
    /* **测量脚手架**：`--spawn <program> [args…]`（先判，免得参数被当次数）。 */
    if (argc >= 3 && strcmp(argv[1], "--spawn") == 0)
        return spawn_mode(&argv[2]);

    /* **③a 单进程多实例**：`--multi <instances> <iterations>`（先判，免得把 `--multi` 当次数）。 */
    if (argc >= 4 && strcmp(argv[1], "--multi") == 0)
        return multi_instance_mode(atol(argv[2]), atol(argv[3]));

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
        if (pa_exec_string(state, "x = 1\n", -1, "footprint", "python", NULL) != PA_OK) return 4;
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
