/* Main number controller: scalar bounded source/state and dictionary primitives.
 * SPDX-License-Identifier: GPL-3.0-or-later */
#ifndef ESPEAK_NG_RUST_NUMBER_FRONTEND_H
#define ESPEAK_NG_RUST_NUMBER_FRONTEND_H
#include <stddef.h>
typedef struct {
    void *context;
    unsigned char (*byte)(void *, ptrdiff_t);
    void (*write)(void *, size_t, unsigned char);
    int (*value)(void *, unsigned);
    unsigned (*word)(void *, size_t);
    int (*lookup)(void *, const char *, char *);
    int (*list)(void *, ptrdiff_t, char *, unsigned *);
    const char *(*text)(void *, unsigned);
    int (*store_text)(void *, unsigned, const char *, size_t);
    int (*classify)(void *, unsigned, unsigned);
    unsigned (*translate)(void *, size_t);
    void (*missing)(void *, int);
    void (*skip)(void *, int);
    int (*phoneme_type)(void *, unsigned char);
} RustNumberFrontend;
/* All callbacks/context required. Source has three initialized predecessors,
 * length initialized bytes (<=800), including embedded NUL separators and the
 * final terminator, and virtual NUL outside that span; word
 * projection admits current row even with remaining=0. Dictionary output is
 * initialized 200-byte scratch; text slots admit ordinal 12/indicator prefix32.
 * Immutable table, source/state and writable output/two initialized flags are
 * disjoint, live and serialized. Return -1 preserves output/flags; applied
 * primitive effects remain. Return 0 declines; 1 publishes a pronunciation. */
int espeak_rs_translate_number(const RustNumberFrontend *, size_t, int, int, char *, size_t, unsigned *);
#endif
