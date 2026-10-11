/* Thread-confined registered output set; foreign byte loans copied before I/O.
 * Paths are initialized through NUL within 4096 bytes (all 4096 initialized
 * on unterminated-input rejection). Descriptor/result are aligned and disjoint.
 * Callback/context stay live on the owner thread through explicit end; callbacks
 * cannot retain byte loans or unwind. End completes admitted writes before
 * unregister/close and never closes borrowed stderr. Each write admits <=16 MiB.
 * Bad argument/reentry rejection leaves the result untouched; an admitted begin
 * publishes a fully initialized result even when file creation fails.
 * SPDX-License-Identifier: GPL-3.0-or-later */
#ifndef ESPEAK_RUST_COMPILER_OUTPUT_H
#define ESPEAK_RUST_COMPILER_OUTPUT_H
#include <stddef.h>
#include <stdint.h>
enum { RUST_OUTPUT_MANIFEST,RUST_OUTPUT_DATA,RUST_OUTPUT_INDEX,RUST_OUTPUT_TABLE,RUST_OUTPUT_LOG };
typedef struct {
    const char *source,*destination;size_t width;
    uint32_t version;int32_t rate;void *context;
    int (*fallback)(void *,const unsigned char *,size_t);
} RustCompilerOutput;
typedef struct {uint64_t owner;uint32_t mask;int32_t status;unsigned char filename[4096];} RustCompilerOutputResult;
int espeak_rs_compiler_output_begin(const RustCompilerOutput *,RustCompilerOutputResult *);
int espeak_rs_compiler_output_write(uint64_t,uint32_t,const unsigned char *,size_t);
int64_t espeak_rs_compiler_output_position(uint64_t,uint32_t);
int espeak_rs_compiler_output_flush(uint64_t,uint32_t);
int espeak_rs_compiler_output_end(uint64_t);
#endif
