/* Letter/symbol and accent pronunciation via serialized dictionary primitives.
 * SPDX-License-Identifier: GPL-3.0-or-later */
#ifndef ESPEAK_NG_RUST_LETTER_LOOKUP_H
#define ESPEAK_NG_RUST_LETTER_LOOKUP_H
#include <stddef.h>
typedef struct {
    void *context;
    int (*lookup)(void *, char *, size_t, unsigned, char *);
    int (*named)(void *, const char *, char *);
    int (*value)(void *, unsigned);
    int (*space)(void *, unsigned);
    void (*rules)(void *, char *, size_t, size_t, unsigned, char *);
    void (*select)(void *, unsigned);
    void (*stress)(void *, char *, unsigned *, int);
} RustLetterLookup;
/* All fields required, immutable table and live serialized context disjoint from
 * output. Source is owned initialized 10-byte storage; callbacks retain no source
 * or scratch pointer and rule translation scopes/restores its source context.
 * Lookup/named/rules/stress scratch is initialized 200 bytes; rule scratch carries
 * the dictionary pronunciation prefix and supports append/no-op; stress flags two
 * initialized unsigneds. Capacity is actual output extent 1..200. Accent mode
 * preserves output when no pronunciation is assembled. -1 preserves output,
 * with already executed dictionary/source/state effects retained, never replayed.
 * 0 leaves accent output unchanged; 1 publishes only the terminated prefix. */
int espeak_rs_lookup_letter(const RustLetterLookup *, unsigned, int, int, int, char *, size_t);
#endif
