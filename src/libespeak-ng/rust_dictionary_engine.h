/* The dictionary.c entry points that run in Rust against the engine's state:
 * rust/phoneme_render_compat.rs, list_host_compat.rs,
 * translate_rules_host_compat.rs and dictionary_engine_compat.rs (each one's
 * `engine` module). SPDX-License-Identifier: GPL-3.0-or-later */
#ifndef ESPEAK_NG_RUST_DICTIONARY_ENGINE_H
#define ESPEAK_NG_RUST_DICTIONARY_ENGINE_H
#include <stddef.h>
const char *espeak_rs_encode_phonemes_engine(const char *, unsigned char *, size_t, int *);
void espeak_rs_decode_phonemes_engine(const char *, unsigned char *);
void espeak_rs_decode_phonemes_bounded_engine(const unsigned char *, unsigned char *, size_t);
char *espeak_rs_write_phoneme_engine(unsigned char *, size_t, const void *, const void *, int,
    unsigned, int *);
const char *espeak_rs_translated_phonemes_engine(int);
int espeak_rs_lookup_list_engine(void *, char **, char *, unsigned *, int, void *, int, size_t);
int espeak_rs_lookup_symbol_engine(void *, const char *, char *, size_t);
int espeak_rs_lookup_flags_engine(void *, const char *, unsigned *);
int espeak_rs_translate_rules_engine(void *, char *, char *, int, char *, int, unsigned *);
int espeak_rs_load_dictionary(void *, const char *, int);
void espeak_rs_free_dictionary_cache(void);
int espeak_rs_translator_is_letter(const void *, int, int);
int espeak_rs_remove_ending_engine(void *, char *, int, char *);
#endif
