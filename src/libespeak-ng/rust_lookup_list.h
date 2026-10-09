/* Native dictionary-list policy and serialized frontend primitives.
 * SPDX-License-Identifier: GPL-3.0-or-later */
#ifndef ESPEAK_NG_RUST_LOOKUP_LIST_H
#define ESPEAK_NG_RUST_LOOKUP_LIST_H
#include <stddef.h>
typedef struct {
    void *context;
    int (*byte)(void *, size_t);
    int (*lookup)(void *, const char *, size_t, unsigned *, char *, size_t *);
    int (*repeat)(void *, char *);
    void (*set_repeat)(void *, const char *, int);
    int (*text_mode)(void *);
    void (*skip)(void *, int);
    void (*accent)(void *, unsigned, size_t, char *);
    void (*replacement)(void *, const char *);
    void (*trace)(void *, size_t);
} RustLookupList;
/* Every callback required, context live and serialized. Lookup receives an
 * initialized terminated 160-byte key, two flags and 200-byte scratch; returns
 * 0 miss, 1 match with numeric original-source end, -1 admission failure.
 * Other scratch: 20-byte repeat state; 160-byte padded replacement. All copied
 * by callbacks, never retained. Byte projection rejects outside its source.
 * Flags and output disjoint from table, owner and source. Capacity 1..200 is the
 * actual writable extent. -1 preserves flags/output with executed primitive
 * effects retained; 0/1 publish initialized terminated prefix and two flags. */
int espeak_rs_lookup_list(const RustLookupList *, unsigned, unsigned *, char *, size_t);
#endif
