/* Native word admission over serialized source/state owners.
 * SPDX-License-Identifier: GPL-3.0-or-later */
#ifndef ESPEAK_NG_RUST_WORD_ADMISSION_H
#define ESPEAK_NG_RUST_WORD_ADMISSION_H
#include "rust_translate_word.h"
typedef struct {
    void *context;
    int (*byte)(void *, RustWordSource);
    int (*write)(void *, RustWordSource, unsigned char);
    int (*value)(void *, unsigned);
    int (*alpha)(void *, unsigned);
    int (*vowel)(void *, unsigned);
    int (*probe)(void *, char *, int *);
    void (*skip)(void *, int);
} RustWordAdmission;
/* All callbacks and context required. Source zero is the original word, with
 * admitted initialized predecessor/lookahead. Scalar value fields are alphabet
 * offset, unpronounceable mode and maximum initial consonants, in that order.
 * Probe receives disjoint owned initialized 200-byte scratch and no flags;
 * primitives retain no Rust loan. Its scoped predecessor is restored even on
 * failure; other executed effects remain. Result is disjoint from all owners
 * and callbacks. -1 preserves the result slot and never replays C policy. */
int espeak_rs_word_dotted(const RustWordAdmission *, int *);
int espeak_rs_word_unpronounceable(const RustWordAdmission *, int, int *);
#endif
