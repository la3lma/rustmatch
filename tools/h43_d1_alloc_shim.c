#define _GNU_SOURCE

#include <fcntl.h>
#include <malloc.h>
#include <stdatomic.h>
#include <stdint.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <unistd.h>

extern void *__libc_calloc(size_t count, size_t size);
extern void __libc_free(void *pointer);
extern void *__libc_malloc(size_t size);
extern void *__libc_memalign(size_t alignment, size_t size);
extern void *__libc_realloc(void *pointer, size_t size);

static _Atomic uint64_t malloc_calls;
static _Atomic uint64_t calloc_calls;
static _Atomic uint64_t aligned_calls;
static _Atomic uint64_t realloc_calls;
static _Atomic uint64_t free_calls;
static _Atomic uint64_t requested_bytes;
static _Atomic uint64_t live_usable_bytes;
static _Atomic uint64_t peak_live_usable_bytes;

static void update_peak(uint64_t live) {
    uint64_t peak = atomic_load_explicit(&peak_live_usable_bytes, memory_order_relaxed);
    while (live > peak && !atomic_compare_exchange_weak_explicit(
                              &peak_live_usable_bytes,
                              &peak,
                              live,
                              memory_order_relaxed,
                              memory_order_relaxed)) {
    }
}

static void record_allocation(void *pointer, size_t requested) {
    if (pointer == NULL) {
        return;
    }
    atomic_fetch_add_explicit(&requested_bytes, requested, memory_order_relaxed);
    uint64_t usable = malloc_usable_size(pointer);
    uint64_t live = atomic_fetch_add_explicit(&live_usable_bytes, usable, memory_order_relaxed) + usable;
    update_peak(live);
}

void *malloc(size_t size) {
    void *pointer = __libc_malloc(size);
    atomic_fetch_add_explicit(&malloc_calls, 1, memory_order_relaxed);
    record_allocation(pointer, size);
    return pointer;
}

void *calloc(size_t count, size_t size) {
    void *pointer = __libc_calloc(count, size);
    atomic_fetch_add_explicit(&calloc_calls, 1, memory_order_relaxed);
    record_allocation(pointer, count * size);
    return pointer;
}

void *aligned_alloc(size_t alignment, size_t size) {
    void *pointer = __libc_memalign(alignment, size);
    atomic_fetch_add_explicit(&aligned_calls, 1, memory_order_relaxed);
    record_allocation(pointer, size);
    return pointer;
}

int posix_memalign(void **result, size_t alignment, size_t size) {
    void *pointer = __libc_memalign(alignment, size);
    if (pointer == NULL) {
        return 12;
    }
    *result = pointer;
    atomic_fetch_add_explicit(&aligned_calls, 1, memory_order_relaxed);
    record_allocation(pointer, size);
    return 0;
}

void *realloc(void *pointer, size_t size) {
    uint64_t old_usable = pointer == NULL ? 0 : malloc_usable_size(pointer);
    void *replacement = __libc_realloc(pointer, size);
    atomic_fetch_add_explicit(&realloc_calls, 1, memory_order_relaxed);
    atomic_fetch_add_explicit(&requested_bytes, size, memory_order_relaxed);
    if (replacement != NULL) {
        uint64_t new_usable = malloc_usable_size(replacement);
        uint64_t live;
        if (new_usable >= old_usable) {
            live = atomic_fetch_add_explicit(
                       &live_usable_bytes, new_usable - old_usable, memory_order_relaxed) +
                   new_usable - old_usable;
        } else {
            live = atomic_fetch_sub_explicit(
                       &live_usable_bytes, old_usable - new_usable, memory_order_relaxed) -
                   old_usable + new_usable;
        }
        update_peak(live);
    }
    return replacement;
}

void free(void *pointer) {
    if (pointer != NULL) {
        uint64_t usable = malloc_usable_size(pointer);
        atomic_fetch_sub_explicit(&live_usable_bytes, usable, memory_order_relaxed);
        atomic_fetch_add_explicit(&free_calls, 1, memory_order_relaxed);
    }
    __libc_free(pointer);
}

__attribute__((destructor)) static void write_receipt(void) {
    const char *path = getenv("RMATCH_ALLOC_RECEIPT");
    if (path == NULL || path[0] == '\0') {
        return;
    }
    char buffer[1024];
    int length = snprintf(
        buffer,
        sizeof(buffer),
        "{\"schema_version\":1,\"allocator\":\"glibc-interposition-v1\","
        "\"malloc_calls\":%llu,\"calloc_calls\":%llu,\"aligned_calls\":%llu,"
        "\"realloc_calls\":%llu,\"free_calls\":%llu,\"requested_bytes\":%llu,"
        "\"live_usable_bytes_at_exit\":%llu,\"peak_live_usable_bytes\":%llu}\n",
        (unsigned long long)atomic_load_explicit(&malloc_calls, memory_order_relaxed),
        (unsigned long long)atomic_load_explicit(&calloc_calls, memory_order_relaxed),
        (unsigned long long)atomic_load_explicit(&aligned_calls, memory_order_relaxed),
        (unsigned long long)atomic_load_explicit(&realloc_calls, memory_order_relaxed),
        (unsigned long long)atomic_load_explicit(&free_calls, memory_order_relaxed),
        (unsigned long long)atomic_load_explicit(&requested_bytes, memory_order_relaxed),
        (unsigned long long)atomic_load_explicit(&live_usable_bytes, memory_order_relaxed),
        (unsigned long long)atomic_load_explicit(&peak_live_usable_bytes, memory_order_relaxed));
    if (length <= 0 || (size_t)length >= sizeof(buffer)) {
        return;
    }
    int descriptor = open(path, O_WRONLY | O_CREAT | O_TRUNC, 0644);
    if (descriptor >= 0) {
        ssize_t written = write(descriptor, buffer, (size_t)length);
        if (written != length) {
            (void)close(descriptor);
            return;
        }
        (void)close(descriptor);
    }
}
