/* The speech.c entry points that run in Rust against the engine's state
 * (rust/speech_engine_compat.rs). SPDX-License-Identifier: GPL-3.0-or-later */
#ifndef ESPEAK_NG_RUST_SPEECH_ENGINE_H
#define ESPEAK_NG_RUST_SPEECH_ENGINE_H
#include <stdio.h>
#include <wchar.h>
#include <espeak-ng/espeak_ng.h>
#include <espeak-ng/speak_lib.h>
void espeak_rs_engine_noop(void);
void espeak_rs_engine_queue_stop(void);
void espeak_rs_engine_event_clear(void);
void espeak_rs_engine_current_voice_clear(void);
void espeak_rs_engine_stack_reset(void);
void espeak_rs_engine_voice_reset(void);
long espeak_rs_engine_clock(void);
unsigned espeak_rs_driver_encoding(Translator *);
espeak_ng_STATUS espeak_rs_driver_default_voice(void);
int espeak_rs_driver_generate(void);
void espeak_rs_request_diagnostic(const char *, const char *);
void espeak_rs_initialize_path(const char *, const char *);
espeak_ng_STATUS espeak_rs_set_phoneme_events(int, int);
int espeak_rs_engine_sample_rate(void);
espeak_ng_STATUS espeak_rs_speak_key(const char *, espeak_ng_STATUS (*)(unsigned int, const void *, int));
espeak_ng_STATUS espeak_rs_speak_char(wchar_t, espeak_ng_STATUS (*)(unsigned int, const void *, int));
void espeak_rs_set_punctuation_list(const wchar_t *);
void espeak_rs_set_synth_callback(t_espeak_callback *, void (*)(t_espeak_callback *));
int espeak_rs_get_parameter(int, int);
void espeak_rs_set_phoneme_trace(int, FILE *);
const char *espeak_rs_text_to_phonemes(const void **, int, int, int *);
int espeak_rs_is_playing(int (*)(void));
const char *espeak_rs_info(const char **, const char *);
#endif
