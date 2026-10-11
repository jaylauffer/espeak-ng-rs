/* Synchronous intonation compiler; descriptor strings/maps are copied first.
 * SPDX-License-Identifier: GPL-3.0-or-later */
#ifndef ESPEAK_RUST_INTONATION_COMPILE_H
#define ESPEAK_RUST_INTONATION_COMPILE_H
#include <stddef.h>
typedef struct {
    void *context;
    const char *source, *destination;
    const unsigned char *spaces, *digits;
    size_t width;
    int windows;
    void (*log)(void *,const unsigned char *,size_t);
    void (*close)(void *);
    void (*reload)(void *);
    int (*error)(void *,int,const char *);
} RustIntonationCompiler;
int espeak_rs_compile_intonation(const RustIntonationCompiler *);
#endif
