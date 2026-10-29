/* Complete native compiler callback host. SPDX-License-Identifier: GPL-3.0-or-later */
#ifndef ESPEAK_RUST_COMPILER_HOST_H
#define ESPEAK_RUST_COMPILER_HOST_H
#include "rust_compiler_context.h"
#include "rust_compiler_tables.h"
struct RustWavegen;
typedef struct {
 RustCompilerContext *context;size_t width;FILE *resource_log;
 int (*rate)(void);
 struct RustWavegen *(*generator)(void);
 void (*message)(int,char *,size_t);
 void (*file_error)(int);
} RustNativeCompilerHost;
/* Serialized engine, live native owner on creating thread; never a copied or
 * caller-allocated context. Context and table row views are quiescent until
 * return. Descriptor copied before effects, name terminates within 256 bytes
 * for START/LOOKUP/FIND/IMPORT (all bytes initialized on rejection). Descriptor
 * and names may overlap quiescent views; publication follows input snapshots.
 * All primitives are synchronous, cannot unwind, retain loans or access the
 * host/context/row views. Generator remains live for each resource load; borrowed
 * resource_log is not closed. Status message fills its initialized bounded loan.
 * File error sets platform errno. Width 2..4096, matching the native root width.
 * Public host/end reentry and wrong-thread calls refuse with EAGAIN; internal
 * source/resource/diagnostic scopes nest and keep the native owner alive.
 * Files and tables preserve sticky native status and historical row/count
 * publication. Existing generic controllers and bounded proactor pools remain.
 */
int espeak_rs_compiler_host_tables(const RustNativeCompilerHost *,uint32_t,const char *,int);
int espeak_rs_compiler_host_files(const RustNativeCompilerHost *);
/* espeak_ng_CompilePhonemeDataPath and espeak_ng_CompileIntonationPath with the
 * engine's primitives (null paths: the data directory; null log: stderr). */
int espeak_rs_compile_phoneme_data(long,const char *,const char *,FILE *,void *);
int espeak_rs_compile_intonation_data(const char *,const char *,FILE *,void *);
/* Disjoint writable 256-byte maps; serialized engine/locale, no retained loans. */
void espeak_rs_compiler_host_classes(unsigned char *,unsigned char *);
#endif
