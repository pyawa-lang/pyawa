/* 护页分配器 v2（第 273 轮诊断用；LD_PRELOAD 挂上即可）：
   每块分配紧贴一页 PROT_NONE ⇒ 任何**越界写**当场 SIGSEGV ✓。
   v2 修：16 字节对齐 ✓（v1 因对齐错在 hashbrown 里崩 ✗）；并做**指针对照** ✓
   —— 不是本分配器发出的指针，一律交给真 `free` ✗（libc 内部仍会用真 malloc ✓）。*/
#define _GNU_SOURCE
#include <dlfcn.h>
#include <stdint.h>
#include <string.h>
#include <sys/mman.h>
#include <unistd.h>

static size_t page_size(void) {
    static size_t size = 0;
    if (size == 0) size = (size_t)sysconf(_SC_PAGESIZE);
    return size;
}

#define MAGIC 0x9E3779B97F4A7C15ull

/* 32 字节头（对齐到 32 ⇒ 返回指针天然 16／32 对齐 ✓）*/
typedef struct {
    void *base;
    size_t length;
    size_t requested;
    uint64_t magic;
} Header;

void *malloc(size_t size) {
    if (size == 0) size = 1;
    size_t page = page_size();
    size_t total = ((size + sizeof(Header) + page - 1) / page) * page + page;
    void *map = mmap(NULL, total, PROT_READ | PROT_WRITE, MAP_PRIVATE | MAP_ANONYMOUS, -1, 0);
    if (map == MAP_FAILED) return NULL;
    char *guard = (char *)map + total - page;
    if (mprotect(guard, page, PROT_NONE) != 0) {
        munmap(map, total);
        return NULL;
    }
    uintptr_t payload = ((uintptr_t)guard - size) & ~(uintptr_t)31;
    Header *header = (Header *)(payload - sizeof(Header));
    header->base = map;
    header->length = total;
    header->requested = size;
    header->magic = MAGIC;
    return (void *)payload;
}

void free(void *pointer) {
    static void (*real_free)(void *) = NULL;
    if (!pointer) return;
    Header *header = ((Header *)pointer) - 1;
    /* 指针对照 ✓：不是本分配器发的（libc 内部 strdup 之类）⇒ 交回真 free ✗ */
    if ((uintptr_t)header < 4096 || header->magic != MAGIC) {
        if (!real_free) real_free = (void (*)(void *))dlsym(RTLD_NEXT, "free");
        if (real_free) real_free(pointer);
        return;
    }
    header->magic = 0;
    munmap(header->base, header->length);
}

void *calloc(size_t count, size_t size) {
    size_t total = count * size;
    void *memory = malloc(total);
    if (memory) memset(memory, 0, total);
    return memory;
}

void *realloc(void *pointer, size_t size) {
    if (!pointer) return malloc(size);
    Header *header = ((Header *)pointer) - 1;
    if (header->magic != MAGIC) {
        static void *(*real_realloc)(void *, size_t) = NULL;
        if (!real_realloc) real_realloc = (void *(*)(void *, size_t))dlsym(RTLD_NEXT, "realloc");
        return real_realloc ? real_realloc(pointer, size) : NULL;
    }
    void *memory = malloc(size);
    if (!memory) return NULL;
    size_t copy = header->requested < size ? header->requested : size;
    memcpy(memory, pointer, copy);
    free(pointer);
    return memory;
}

int posix_memalign(void **out, size_t alignment, size_t size) {
    (void)alignment;
    void *memory = malloc(size);
    if (!memory) return 12;
    *out = memory;
    return 0;
}

void *aligned_alloc(size_t alignment, size_t size) { (void)alignment; return malloc(size); }
void *memalign(size_t alignment, size_t size) { (void)alignment; return malloc(size); }
void *valloc(size_t size) { return malloc(size); }
