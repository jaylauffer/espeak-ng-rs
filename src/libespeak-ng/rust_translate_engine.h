/* The translate.c entry points that run in Rust against the engine's state
 * (rust/translate_engine_compat.rs). SPDX-License-Identifier: GPL-3.0-or-later */
#ifndef ESPEAK_NG_RUST_TRANSLATE_ENGINE_H
#define ESPEAK_NG_RUST_TRANSLATE_ENGINE_H
int espeak_rs_select_alternate(unsigned, const char *);
void espeak_rs_free_alternates(void);
int espeak_rs_translate_word_engine(void *, char *, void *, char *);
void espeak_rs_init_text_engine(int);
void espeak_rs_translate_clause_engine(void *, int *, char **, int *);
#endif
