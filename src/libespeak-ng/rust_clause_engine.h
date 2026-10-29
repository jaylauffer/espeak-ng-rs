/* The readclause.c and ssml.c entry points that run in Rust against the
 * engine's state: rust/readclause_engine_compat.rs and ssml_compat.rs (its
 * `host` module). SPDX-License-Identifier: GPL-3.0-or-later */
#ifndef ESPEAK_NG_RUST_CLAUSE_ENGINE_H
#define ESPEAK_NG_RUST_CLAUSE_ENGINE_H
#include <stdbool.h>
#include <stddef.h>
#include <wchar.h>
int espeak_rs_clause_reader_eof(void);
int espeak_rs_read_clause_engine(void *, char *, short *, int *, int, int *, char *, unsigned);
void espeak_rs_init_text(void);
void espeak_rs_set_voice_stack(const void *, const char *);
int espeak_rs_add_name_data(const char *, int);
void espeak_rs_free_name_data(void);
void espeak_rs_init_name_data(void);
int espeak_rs_ssml_tag_engine(wchar_t *, char *, int *, int, const char *, bool *, char *,
    const void *, const char *, bool *, bool *, int *, int *, void *, int *, int *, int *,
    unsigned);
void espeak_rs_set_uri_callback(int (*)(int, const char *, const char *));
int espeak_rs_ssml_reference_engine(char *, int *, int *);
#endif
