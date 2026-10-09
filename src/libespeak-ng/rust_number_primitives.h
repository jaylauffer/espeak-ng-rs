/* Native number/spelling algorithms, bounded serialized spans.
 * SPDX-License-Identifier: GPL-3.0-or-later */
#ifndef ESPEAK_NG_RUST_NUMBER_PRIMITIVES_H
#define ESPEAK_NG_RUST_NUMBER_PRIMITIVES_H
#include <stddef.h>
#include <stdint.h>
int espeak_rs_superscript(int);
const char *espeak_rs_number_variant(int, int);
int espeak_rs_spelling(unsigned char *, size_t, size_t, int, int, int);
int espeak_rs_number_hungarian(const unsigned char *, int, int);
int espeak_rs_number_group(const unsigned char *, int);
int espeak_rs_number_roman(const unsigned char *, unsigned char, unsigned, int, int, int, int *, size_t *);
#endif
