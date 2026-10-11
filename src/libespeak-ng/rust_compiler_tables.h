/* Registered retained tables with separate quiescent compatibility row views.
 * Source/target/descriptor are copied before callbacks; results publish after.
 * Only current rows may be written between calls; all views are quiescent
 * during calls/callbacks, older found rows are read-only, and end expires them.
 * Initialized names terminate within 256 bytes (all 256 initialized on reject).
 * Descriptors/results are aligned and disjoint; callback context remains live,
 * callbacks cannot retain byte loans or unwind. SPDX-License-Identifier: GPL-3.0-or-later */
#ifndef ESPEAK_RUST_COMPILER_TABLES_H
#define ESPEAK_RUST_COMPILER_TABLES_H
#include <stddef.h>
#include <stdint.h>
#include "phoneme.h"
enum {RUST_TABLE_START,RUST_TABLE_LOOKUP,RUST_TABLE_FIND,RUST_TABLE_IMPORT,RUST_TABLE_FINISH,RUST_TABLE_PACK,RUST_TABLE_TERMINATE};
typedef struct {
    uint64_t owner;void *context;const char *name;PHONEME_TAB *target;int32_t line,control;
    int (*base)(void *,unsigned char *,size_t);
    void (*error)(void *,int32_t,int32_t,const unsigned char *,size_t);
    int (*write)(void *,const unsigned char *,size_t);
} RustCompilerTables;
typedef struct {PHONEME_TAB *current,*found;int32_t tables,count,value,status;} RustCompilerTableResult;
uint64_t espeak_rs_compiler_tables_begin(void);
int espeak_rs_compiler_tables_end(uint64_t);
int espeak_rs_compiler_tables_call(const RustCompilerTables *,uint32_t,RustCompilerTableResult *);
#endif
