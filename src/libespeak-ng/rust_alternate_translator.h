/* Serialized alternate-language lifecycle over checked resource slots.
 * SPDX-License-Identifier: GPL-3.0-or-later */
#ifndef ESPEAK_NG_RUST_ALTERNATE_TRANSLATOR_H
#define ESPEAK_NG_RUST_ALTERNATE_TRANSLATOR_H
#include <stddef.h>
typedef struct {
    void *context;
    int (*present)(void *,unsigned);
    int (*cached)(void *,unsigned,size_t);
    int (*cache)(void *,unsigned,size_t,unsigned char);
    int (*lookup)(void *,const char *);
    int (*create)(void *,unsigned,const char *);
    int (*load)(void *,unsigned);
    int (*main_table)(void *);
    int (*effect)(void *,unsigned,unsigned,int);
} RustAlternateTranslator;
/* Immutable aligned table/context retained for serialized call and disjoint
 * from mutable resources/output. Name is length initialized bytes, copied
 * before callbacks (max 19, no embedded NUL); primitives copy it and retain no
 * pointer into scratch. Modes: slot 0/1 selection, 2 release both slots.
 * Effects 0 delete, 1 clear pointer, 2 table, 3 repeat reset, 4 table selection.
 * Cache operations check 20-byte bounds. Failure preserves output and completed
 * live effects; no retained C replay. Translator slots and cache/handle
 * admission remain C; resident dictionary storage is already Rust-owned. */
int espeak_rs_alternate_translator(const RustAlternateTranslator *,unsigned,const char *,size_t,int *);
#endif
