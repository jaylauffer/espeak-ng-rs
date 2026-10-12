/* Copied resource admission, registered retained owners and primitive effects.
 * SPDX-License-Identifier: GPL-3.0-or-later */
#ifndef ESPEAK_RUST_COMPILER_RESOURCE_H
#define ESPEAK_RUST_COMPILER_RESOURCE_H
#include <stddef.h>
#include <stdint.h>
#include "rust_spect_file.h"
#include "rust_compiler_context.h"
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
/* Native context must be a live Rust owner on its creating thread. Descriptor,
 * selected strings (through NUL within 4096, all 4096 on refusal), address,
 * generator and current phoneme view are initialized. Address is disjoint from
 * context/generator; inputs may overlap address and copy before publication.
 * FILE is borrowed/exclusive and never closed here. Locale/engine access stays
 * serialized; diagnostic primitive uses the context only between Rust scalar
 * accesses, cannot invalidate views/leases, retain byte loans or unwind.
 * Source/resource/end reentry refuses with EAGAIN during native resource effects.
 * Context end consumes the retained resource lease; no independent release.
 */
typedef struct {
    RustCompilerContext *context;const char *name;void *generator,*log;
    size_t width;int32_t rate,control;
    void (*error)(void *,int32_t,int32_t,int32_t,const unsigned char *,size_t);
} RustNativeCompilerResource;
int espeak_rs_compiler_resource_native(const RustNativeCompilerResource *,int32_t *);
#endif
