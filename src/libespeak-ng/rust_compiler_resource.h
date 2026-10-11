/* Copied resource admission, registered retained owners and primitive effects.
 * SPDX-License-Identifier: GPL-3.0-or-later */
#ifndef ESPEAK_RUST_COMPILER_RESOURCE_H
#define ESPEAK_RUST_COMPILER_RESOURCE_H
#include <stddef.h>
#include <stdint.h>
#include "rust_spect_file.h"
typedef struct {int32_t address,references,duplicates,frames;} RustResourceProgress;
typedef struct {
    uint64_t owner;void *context;
    const char *source,*name;void *generator;
    const int32_t *lower;const unsigned char *space;size_t width;
    int32_t rate,control,mnemonic,table,decimal;RustResourceProgress initial;
    void (*log)(void *,const unsigned char *,size_t);
    void (*error)(void *,int32_t,int32_t,int32_t,const unsigned char *,size_t);
    int64_t (*position)(void *);
    int (*write)(void *,int32_t,const unsigned char *,size_t);
} RustCompilerResource;
uint64_t espeak_rs_compiler_resources_begin(void);
int espeak_rs_compiler_resources_end(uint64_t);
int espeak_rs_compiler_resource_load(const RustCompilerResource *,RustResourceProgress *);
#endif
