/* Roman pronunciation controller with scalar source/state and owned numeric text.
 * SPDX-License-Identifier: GPL-3.0-or-later */
#ifndef ESPEAK_NG_RUST_NUMBER_ROMAN_H
#define ESPEAK_NG_RUST_NUMBER_ROMAN_H
#include "rust_number_frontend.h"
typedef struct {
    const RustNumberFrontend *frontend;
    int (*range)(void *, unsigned);
    const unsigned char *(*suffix)(void *);
    void (*word)(void *, unsigned);
    void (*clear)(void *);
    int (*list)(void *, char *, char *, unsigned *);
    unsigned (*translate)(void *, char *, size_t, size_t);
} RustNumberRoman;
/* All callbacks required; frontend contracts apply. Suffix admits initialized
 * prefix through NUL or 160 bytes. Owned synthetic source passed to list and
 * translate has three predecessors, initialized prefix including final NUL,
 * and no retained pointer/loan after callback. Translation scopes/restores
 * its source/rule context. Output capacity 1..200, disjoint initialized storage.
 * -1 preserves output; applied source/state/dictionary effects remain. 0/1
 * publishes the final prefix even on post-recognition decline, as legacy C. */
int espeak_rs_translate_roman(const RustNumberRoman *, size_t, int, char *, size_t);
#endif
