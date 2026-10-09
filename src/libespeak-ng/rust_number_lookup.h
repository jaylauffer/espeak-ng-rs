/* Native thousands-name control; serialized dictionary/state primitives.
 * SPDX-License-Identifier: GPL-3.0-or-later */
#ifndef ESPEAK_NG_RUST_NUMBER_LOOKUP_H
#define ESPEAK_NG_RUST_NUMBER_LOOKUP_H
#include <stddef.h>
typedef struct {
    void *context;
    /* Writes a terminated phoneme string into initialized 200-byte scratch. */
    int (*lookup)(void *, const char *, char *);
    int (*value)(void *, unsigned); /* local numbers, global variants, control */
    void (*missing)(void *, int);
} RustNumberLookup;
int espeak_rs_lookup_thousands(const RustNumberLookup *, int, int, int, char *, size_t, int *);
#endif
