/* Complete spectral compiler; copied admission and primitive stream effects.
 * SPDX-License-Identifier: GPL-3.0-or-later */
#ifndef ESPEAK_RUST_SPECT_COMPILE_H
#define ESPEAK_RUST_SPECT_COMPILE_H
#include <stddef.h>
#include <stdint.h>
#include "rust_spect_file.h"
typedef struct {int32_t address,count_frames;uint32_t has_address;} RustSpectResult;
typedef struct {
    void *context;
    const char *source,*path;
    void *generator;
    size_t width;
    int32_t rate,control,initial_count;
    void (*log)(void *,const unsigned char *,size_t);
    void (*bad)(void *,const char *);
    int64_t (*position)(void *);
    int (*write)(void *,const unsigned char *,size_t);
} RustSpectCompiler;
int espeak_rs_compile_spect(const RustSpectCompiler *,RustSpectResult *);
#endif
