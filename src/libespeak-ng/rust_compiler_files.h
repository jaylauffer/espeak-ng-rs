/* Owned source-file controller. SPDX-License-Identifier: GPL-3.0-or-later */
#ifndef ESPEAK_RUST_COMPILER_FILES_H
#define ESPEAK_RUST_COMPILER_FILES_H
#include "rust_compiler_program.h"
typedef struct { int32_t file,line;unsigned char name[80]; } RustCompilerFrame;
typedef struct {
 void *context; const unsigned char *root;size_t width;
 int32_t (*reset)(void *),(*eof)(void *);
 int32_t (*next)(void *,int32_t,RustProgramToken *);
 int32_t (*frame)(void *,RustCompilerFrame *);
 int32_t (*project)(void *,const RustCompilerFrame *);
 int32_t (*open)(void *,const unsigned char *,size_t,size_t);
 int32_t (*close)(void *),(*tables)(void *),(*end)(void *);
 int32_t (*start)(void *,const unsigned char *,size_t);
 int32_t (*compile)(void *,int32_t);
 int32_t (*terminate)(void *);
 void (*error)(void *,const unsigned char *,size_t);
} RustCompilerFiles;
/* Initialized root extent width<=4096 and descriptor copied before effects;
 * live synchronous callbacks cannot retain loans/unwind. Result supports
 * overlap and is written after effects: 1 complete, 0 missing-table early exit.
 * Admission/effect errors leave result untouched; reentry refuses EAGAIN.
 * open returns positive file token, 0 missing, or negative errno. */
int32_t espeak_rs_compiler_files(const RustCompilerFiles *,int32_t *);
#endif
