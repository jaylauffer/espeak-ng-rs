/* Exclusive synchronous source sessions, begun/used/ended on one thread.
 * Foreign strings and result pointers are never retained by Rust.
 * SPDX-License-Identifier: GPL-3.0-or-later */
#ifndef ESPEAK_RUST_COMPILER_SOURCE_H
#define ESPEAK_RUST_COMPILER_SOURCE_H
#include <stddef.h>
typedef struct {
    int status, line, terminator, length, value, diagnostic;
    unsigned char bytes[256];
} RustCompilerToken;
void *espeak_rs_compiler_sources_begin(const unsigned char *, const unsigned char *, int *);
void espeak_rs_compiler_sources_end(void *);
int espeak_rs_compiler_source_open(void *, const char *, size_t, int, int *);
int espeak_rs_compiler_source_eof(void *, int);
void espeak_rs_compiler_source_rewind(void *, int);
int espeak_rs_compiler_source_unget(void *, int);
int espeak_rs_compiler_source_check(void *, int, int *);
int espeak_rs_compiler_source_next(void *, int, int, int, RustCompilerToken *);
int espeak_rs_compiler_source_line(void *, int, unsigned char *, size_t);
#endif
