/* Serialized initialized source/state primitives for dot ordinals.
 * SPDX-License-Identifier: GPL-3.0-or-later */
#ifndef ESPEAK_NG_RUST_NUMBER_ORDINAL_H
#define ESPEAK_NG_RUST_NUMBER_ORDINAL_H
#include <stddef.h>
#include <stdint.h>
typedef struct {
    void *context;
    unsigned char (*byte)(void *, ptrdiff_t);
    void (*space)(void *, size_t);
    unsigned (*value)(void *, unsigned);
    int (*classify)(void *, unsigned, unsigned);
    unsigned (*translate)(void *, size_t);
} RustNumberOrdinal;
/* length includes the initialized NUL; end is inside it. All callbacks are
 * required. byte provides virtual NUL outside [-2,length); translation and
 * mutation are serialized. Return -1 for invalid admission, otherwise 0/2/34. */
int espeak_rs_number_dot(const RustNumberOrdinal *, size_t, size_t, int);
#endif
