/* Native number pronunciation control; serialized primitive projections.
 * SPDX-License-Identifier: GPL-3.0-or-later */
#ifndef ESPEAK_NG_RUST_NUMBER_DIGITS_H
#define ESPEAK_NG_RUST_NUMBER_DIGITS_H
#include <stddef.h>
typedef struct {
    void *context;
    int (*lookup)(void *, const char *, char *); /* initialized 200-byte output */
    int (*value)(void *, unsigned); /* numbers, global variants, control, numbers2, digit count, language */
    void (*missing)(void *, int);
    const char *(*text)(void *, unsigned); /* initialized prefix: digits 50, ordinal/alternate 12 */
    int (*phoneme_type)(void *, unsigned char); /* checked code, missing slot -1 */
} RustNumberDigits;
/* Tables/scalar/byte outputs are live and disjoint; failures preserve outputs. */
int espeak_rs_lookup_num2(const RustNumberDigits *, int, int, int, char *, size_t, int *);
int espeak_rs_lookup_num3(const RustNumberDigits *, int, int, int, int, char *, size_t);
#endif
