/* The voices.c entry points that run in Rust against the engine's state:
 * rust/voices_engine_compat.rs and the `engine` modules of
 * voice_native_compat.rs and voice_control_compat.rs.
 * SPDX-License-Identifier: GPL-3.0-or-later */
#ifndef ESPEAK_NG_RUST_VOICES_ENGINE_H
#define ESPEAK_NG_RUST_VOICES_ENGINE_H
#include <stddef.h>
#include <stdint.h>
#include <espeak-ng/speak_lib.h>
int64_t espeak_rs_voice_length(void *, const unsigned char *, size_t);
void espeak_rs_free_voice_list(void);
const espeak_VOICE **espeak_rs_list_voices_engine(const espeak_VOICE *);
const char *espeak_rs_select_voice_engine(const espeak_VOICE *, int *);
void espeak_rs_free_current_voice(void);
espeak_VOICE *espeak_rs_current_voice(void);
void espeak_rs_read_numbers_engine(const char *, int *, int, const void *, int);
void *espeak_rs_load_voice_engine(const char *, int, uint32_t,
    uint32_t (*)(void *, const RustMbrolaRequest *, int32_t *), void (*)(void *, uint32_t));
int espeak_rs_voice_control_engine(uint32_t, const char *, const espeak_VOICE *, int, void **);
uint32_t espeak_rs_voice_mbrola(void *, const RustMbrolaRequest *, int32_t *);
void espeak_rs_voice_backend_error(void *, uint32_t);
#endif
