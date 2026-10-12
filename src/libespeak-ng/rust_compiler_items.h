/* Complete owned NextItem policy. SPDX-License-Identifier: GPL-3.0-or-later */
#ifndef ESPEAK_RUST_COMPILER_ITEMS_H
#define ESPEAK_RUST_COMPILER_ITEMS_H
#include "rust_compiler_program.h"
typedef struct {
 void *source; int32_t slot,kind,line,terminator;
 const unsigned char *previous; void *context;
 int32_t (*lookup)(void *,int32_t,const unsigned char *,size_t);
 void (*error)(void *,int32_t,const unsigned char *,size_t);
} RustCompilerItem;
typedef struct { RustProgramToken token;int32_t line; } RustItemResult;
/* Live source session on its creating thread; 256 initialized previous bytes;
 * synchronous callbacks cannot retain loans/unwind. Inputs copied before
 * effects; source borrow ends before effects and overlapping result writes.
 * Errors leave result unchanged; callback reentry refuses with EAGAIN. */
int32_t espeak_rs_compiler_item(const RustCompilerItem *,RustItemResult *);
/* Two writable int32_t outputs: keyword category and value; -1/-1 if unknown. */
int32_t espeak_rs_compiler_keyword(int32_t,const unsigned char *,size_t,int32_t *);
#endif
