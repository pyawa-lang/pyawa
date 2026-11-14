/* Pyawa 能力域 vtable 的 C 布局（AB-45 允许的唯一伴随头；`pa.h` include 它）。
 *
 * **自动生成，勿手改**：`tools/gen_pa_capabilities.py` 从 `crates/pyawa-capabilities/src/`
 * 的 `#[repr(C)]` 形状生成；`--check` 漂移即红（PLAN §12.17 A1/A3）。
 * **只含已有 Rust 布局的 3 域**（fs/clock/net）；其余 6 域本 ABI 不定义布局（只能传 NULL）。
 */
#ifndef PAWA_PA_CAPABILITIES_H
#define PAWA_PA_CAPABILITIES_H

#include <stdbool.h>
#include <stddef.h>
#include <stdint.h>

#ifdef __cplusplus
extern "C" {
#endif

/* CP-5：能力槽位的三态结果（不是 `pa_status`；0/1/2 与 Rust `CapStatus` 同值）。 */
typedef enum pa_cap_status {
    PA_CAP_OK = 0,
    PA_CAP_MACHINE = 1,
    PA_CAP_UNIMPLEMENTED = 2,
} pa_cap_status;
/* Rust `CapStatus` 是 repr(C) enum（4 字节）；C99 下用编译期负数组长度断言。 */
typedef char pa_cap_status_must_be_4_bytes[(sizeof(pa_cap_status) == 4) ? 1 : -1];

typedef struct pa_cap_handle {
    uint64_t value;
} pa_cap_handle;

typedef struct pa_cap_socket_handle {
    uint64_t value;
} pa_cap_socket_handle;

typedef struct pa_cap_file_info {
    uint64_t size;
    uint32_t mode;
    bool is_dir;
    uint64_t dev;
    uint64_t ino;
} pa_cap_file_info;

typedef struct pa_cap_file_times {
    int64_t atime_ns;
    int64_t mtime_ns;
    int64_t ctime_ns;
} pa_cap_file_times;

typedef struct pa_cap_tm {
    int32_t tm_sec;
    int32_t tm_min;
    int32_t tm_hour;
    int32_t tm_mday;
    int32_t tm_mon;
    int32_t tm_year;
    int32_t tm_wday;
    int32_t tm_yday;
    int32_t tm_isdst;
    int64_t tm_gmtoff;
    uint8_t tm_zone[16];
    uint32_t tm_zone_len;
} pa_cap_tm;

typedef struct pa_cap_poll_fd {
    pa_cap_socket_handle handle;
    int32_t events;
    int32_t revents;
} pa_cap_poll_fd;

typedef struct pa_fs_vtable {
    uint32_t version;
    uint32_t size;
    void * state;
    pa_cap_status (*open)(void *, const uint8_t *, size_t, int32_t, uint32_t, pa_cap_handle *, int32_t *);
    pa_cap_status (*stat)(void *, const uint8_t *, size_t, pa_cap_file_info *, int32_t *);
    pa_cap_status (*fstat)(void *, pa_cap_handle, pa_cap_file_info *, int32_t *);
    pa_cap_status (*listdir)(void *, pa_cap_handle, uint8_t *, size_t, size_t *, int32_t *);
    pa_cap_status (*mkdir)(void *, const uint8_t *, size_t, uint32_t, int32_t *);
    pa_cap_status (*rmdir)(void *, const uint8_t *, size_t, int32_t *);
    pa_cap_status (*unlink)(void *, const uint8_t *, size_t, int32_t *);
    pa_cap_status (*rename)(void *, const uint8_t *, size_t, const uint8_t *, size_t, int32_t *);
    pa_cap_status (*readlink)(void *, const uint8_t *, size_t, uint8_t *, size_t, size_t *, int32_t *);
    pa_cap_status (*symlink)(void *, const uint8_t *, size_t, const uint8_t *, size_t, int32_t *);
    pa_cap_status (*chmod)(void *, const uint8_t *, size_t, uint32_t, int32_t *);
    pa_cap_status (*utime)(void *, const uint8_t *, size_t, int64_t, int64_t, int32_t *);
    pa_cap_status (*read)(void *, pa_cap_handle, uint8_t *, size_t, size_t, size_t *, int32_t *);
    pa_cap_status (*write)(void *, pa_cap_handle, const uint8_t *, size_t, size_t *, int32_t *);
    pa_cap_status (*seek)(void *, pa_cap_handle, int64_t, int32_t, uint64_t *, int32_t *);
    pa_cap_status (*tell)(void *, pa_cap_handle, uint64_t *, int32_t *);
    pa_cap_status (*truncate)(void *, pa_cap_handle, uint64_t, int32_t *);
    pa_cap_status (*fsync)(void *, pa_cap_handle, int32_t *);
    pa_cap_status (*close)(void *, pa_cap_handle, int32_t *);
    pa_cap_status (*stat_times)(void *, const uint8_t *, size_t, pa_cap_file_times *, int32_t *);
    pa_cap_status (*pipe)(void *, pa_cap_handle *, pa_cap_handle *, int32_t *);
    pa_cap_status (*lutime)(void *, const uint8_t *, size_t, int64_t, int64_t, int32_t *);
    pa_cap_status (*utime_ns)(void *, const uint8_t *, size_t, int64_t, int64_t, int32_t *);
} pa_fs_vtable;

typedef struct pa_clock_vtable {
    uint32_t version;
    uint32_t size;
    void * state;
    pa_cap_status (*now_ns)(void *, int64_t *);
    pa_cap_status (*monotonic_ns)(void *, int64_t *);
    pa_cap_status (*sleep_ns)(void *, int64_t, int32_t *);
    pa_cap_status (*localtime)(void *, int64_t, pa_cap_tm *, int32_t *);
    pa_cap_status (*strftime)(void *, const uint8_t *, size_t, const pa_cap_tm *, uint8_t *, size_t, size_t *, int32_t *);
    pa_cap_status (*mktime)(void *, const pa_cap_tm *, int64_t *, int32_t *);
} pa_clock_vtable;

typedef struct pa_net_vtable {
    uint32_t version;
    uint32_t size;
    void * state;
    pa_cap_status (*socket)(void *, int32_t, int32_t, int32_t, pa_cap_socket_handle *, int32_t *);
    pa_cap_status (*connect)(void *, pa_cap_socket_handle, int32_t, const uint8_t *, size_t, int32_t *);
    pa_cap_status (*bind)(void *, pa_cap_socket_handle, int32_t, const uint8_t *, size_t, int32_t *);
    pa_cap_status (*listen)(void *, pa_cap_socket_handle, int32_t, int32_t *);
    pa_cap_status (*accept)(void *, pa_cap_socket_handle, pa_cap_socket_handle *, int32_t *, uint8_t *, size_t, size_t *, int32_t *);
    pa_cap_status (*send)(void *, pa_cap_socket_handle, const uint8_t *, size_t, size_t *, int32_t *);
    pa_cap_status (*recv)(void *, pa_cap_socket_handle, uint8_t *, size_t, size_t *, int32_t *);
    pa_cap_status (*shutdown)(void *, pa_cap_socket_handle, int32_t, int32_t *);
    pa_cap_status (*getsockname)(void *, pa_cap_socket_handle, int32_t *, uint8_t *, size_t, size_t *, int32_t *);
    pa_cap_status (*getpeername)(void *, pa_cap_socket_handle, int32_t *, uint8_t *, size_t, size_t *, int32_t *);
    pa_cap_status (*getsockopt)(void *, pa_cap_socket_handle, int32_t, int32_t, uint8_t *, size_t, size_t *, int32_t *);
    pa_cap_status (*setsockopt)(void *, pa_cap_socket_handle, int32_t, int32_t, const uint8_t *, size_t, int32_t *);
    pa_cap_status (*poll)(void *, const pa_cap_poll_fd *, size_t, int64_t, pa_cap_poll_fd *, size_t, size_t *, int32_t *);
    pa_cap_status (*socketpair)(void *, int32_t, int32_t, int32_t, pa_cap_socket_handle *, pa_cap_socket_handle *, int32_t *);
    pa_cap_status (*close)(void *, pa_cap_socket_handle, int32_t *);
} pa_net_vtable;

#ifdef __cplusplus
}
#endif

#endif /* PAWA_PA_CAPABILITIES_H */
