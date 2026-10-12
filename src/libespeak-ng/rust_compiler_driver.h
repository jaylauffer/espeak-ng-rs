/* Complete public phoneme compiler. All paths are initialized through NUL
 * within width (2..4096); optional source/destination may be NULL. On missing
 * NUL rejection all width bytes are initialized. Maps are 256 initialized bytes.
 * Descriptor/text/maps are copied before effects. Serialized engine callbacks
 * cannot retain loans or unwind. Effect paths are terminated owned 4096-byte
 * buffers; error scratch is initialized, writable 4096 bytes. Allocate failure
 * must leave no live context. Cleanup consumes an admitted context exactly once.
 * Reentry refuses with EAGAIN, overlong paths with ENAMETOOLONG before effects.
 * keep_prefix_x snapshots host scanf's incomplete-hex-prefix consumption;
 * probe with unsigned value=0, char tail=0; sscanf("0x g","%x %c",&value,&tail),
 * then pass tail=='x'. This avoids assuming identical libc scanner behavior.
 * SPDX-License-Identifier: GPL-3.0-or-later */
#ifndef ESPEAK_RUST_COMPILER_DRIVER_H
#define ESPEAK_RUST_COMPILER_DRIVER_H
#include <stddef.h>
#include <stdint.h>
enum {
 RUST_COMPILER_ALLOCATE,RUST_COMPILER_TABLES,RUST_COMPILER_PROGRAM,
 RUST_COMPILER_RATE,RUST_COMPILER_RELOAD,RUST_COMPILER_VOICE,
 RUST_COMPILER_WAVE_INIT,RUST_COMPILER_INITIALIZE,RUST_COMPILER_SOURCE_OPEN,
 RUST_COMPILER_OUTPUT_OPEN,RUST_COMPILER_BASE_TABLE,RUST_COMPILER_COMPILE,
 RUST_COMPILER_END_TABLE,RUST_COMPILER_WRITE_TABLES,RUST_COMPILER_SOURCE_CLOSE,
 RUST_COMPILER_OUTPUT_CLOSE,RUST_COMPILER_OUTPUT_ERROR,RUST_COMPILER_CLOSE_LOG,
 RUST_COMPILER_WAVE_FINI,RUST_COMPILER_CLEANUP
};
typedef struct {int32_t references,reused,errors;} RustCompilerCounts;
typedef struct {
 void *context;
 const char *home,*source,*destination;
 const unsigned char *alpha,*spaces;
 size_t width;int windows,keep_prefix_x;
 int (*effect)(void *,int,int,const char *,const char *,unsigned char *);
 void (*counts)(void *,RustCompilerCounts *);
 void (*log)(void *,const unsigned char *,size_t);
 int (*error)(void *,int,const char *);
} RustPhonemeCompiler;
int espeak_rs_compile_phonemes(const RustPhonemeCompiler *);
/* Bounded in-memory manifest admission. Inputs/maps are copied; rows may
 * overlap input. Output rows/used must be mutually disjoint and aligned.
 * Parsing/refusal never publishes partial rows. Capacity <=131072; input
 * <=16 MiB. Empty manifest returns ENS_EMPTY_PHONEME_MANIFEST with used=0.
 * Invalid admission, bounds, insufficient capacity leave output untouched. */
typedef struct {uint32_t value;unsigned char name[120];} RustManifestRow;
typedef struct {
 const unsigned char *bytes;size_t length,width;int windows,keep_prefix_x;
 const unsigned char *alpha,*spaces;
} RustManifest;
int espeak_rs_compiler_manifest_parse(const RustManifest *,RustManifestRow *,size_t,size_t *);
#endif
