/* Rust-owned native compiler storage and ordered lease teardown.
 * SPDX-License-Identifier: GPL-3.0-or-later */
#ifndef ESPEAK_RUST_COMPILER_CONTEXT_H
#define ESPEAK_RUST_COMPILER_CONTEXT_H
#include <stddef.h>
#include <stdint.h>
#include <stdio.h>
#include "phoneme.h"
typedef struct {
 PHONEME_TAB *phoneme_out;
 uint64_t table_owner;int table_status;
 PHONEME_TAB *phoneme_tab2;
 PHONEME_TAB definition_row;int definition_phoneme;
 uint64_t program_owner;int program_status,n_phoneme_tabs,n_phcodes;
 uint64_t output_owner;uint32_t output_mask;
 FILE *f_errors;int f_in;void *source_pool;
 int linenum,count_references,duplicate_references,count_frames,error_count;
 char current_fname[80];uint64_t resource_owner;
 int item_type,item_terminator;char item_string[256],phsrc[4096];
} RustCompilerContext;
typedef struct {
 RustCompilerContext *context;const char *path;size_t width,depth;int text;
 void (*classes)(unsigned char *,unsigned char *);
} RustCompilerSourceOpen;
/* Contexts begin/use/end on one thread; whole engine access is serialized.
 * C accesses this initialized writable view only between context/source/resource
 * calls, except a resource diagnostic primitive may use its admitted fields.
 * Never allocate/free/copy the context or independently destroy its leases.
 * Primitive output close may consume its lease and clear output_owner. End drains
 * program/table/output/source/resource leases in that order and consumes once;
 * no view survives end. f_errors is borrowed and is not closed by context end.
 * Inputs terminate within width (2..4096), full width initialized on rejection.
 * Descriptor/path copy before effects/I/O. Output may overlap those inputs but
 * is disjoint from context/classes storage. Classes fills 256 initialized bytes
 * in each scratch map synchronously, without retention, unwind or context access.
 * Reentry during source/resource/end and wrong-thread calls refuse with EAGAIN;
 * malformed admission with EINVAL. Source depths are 0..12 (root plus includes).
 * Path/descriptor scratch is never retained across callbacks or completions.
 */
RustCompilerContext *espeak_rs_compiler_context_begin(const char *,size_t,int *);
int espeak_rs_compiler_context_end(RustCompilerContext *);
int espeak_rs_compiler_context_source(const RustCompilerSourceOpen *,int *);
#endif
