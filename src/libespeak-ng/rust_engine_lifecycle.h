/* Native engine lifecycle over serialized platform/engine primitives.
 * SPDX-License-Identifier: GPL-3.0-or-later */
#ifndef ESPEAK_NG_RUST_ENGINE_LIFECYCLE_H
#define ESPEAK_NG_RUST_ENGINE_LIFECYCLE_H
#include <stdint.h>
#include <stddef.h>
#include <espeak-ng/espeak_ng.h>
#include <espeak-ng/encoding.h>
#include "translate.h"
struct audio_object;
enum { RUST_ENGINE_MODE, RUST_ENGINE_OUTPUT_RATE, RUST_ENGINE_VOICE_RATE, RUST_ENGINE_ERROR };
typedef struct {
    uint32_t capabilities;
    void (*actions[22])(void);
    char *(*locale)(int, const char *);
    int ctype;
    espeak_ng_STATUS (*load)(int *, espeak_ng_ERROR_CONTEXT *);
    void (*wave_init)(int, int);
    const int *defaults;
    int *current, *saved, *capitals, *punctuation, *phonemes, *phoneme_events, *echo;
    espeak_ng_STATUS (*parameter)(int, int, int);
    long (*clock)(void);
    void (*seed)(long);
    struct audio_object *(*create_audio)(const char *, const char *, const char *);
    void (*close_audio)(struct audio_object *);
    void (*destroy_audio)(struct audio_object *);
    int (*flush_audio)(struct audio_object *);
    int (*output_reserve)(size_t);
    int (*events_reserve)(int);
    espeak_ng_STATUS (*synchronize)(void);
    Translator **translator;
    void (*destroy_translator)(Translator *);
    espeak_ng_TEXT_DECODER **decoder;
    void (*destroy_decoder)(espeak_ng_TEXT_DECODER *);
} RustEngineLifecycle;
int32_t espeak_rs_engine_value(uint32_t);
void espeak_rs_engine_store(uint32_t, int32_t);
struct audio_object *espeak_rs_engine_audio(void);
espeak_ng_STATUS espeak_rs_engine_initialize(const RustEngineLifecycle *, espeak_ng_ERROR_CONTEXT *);
espeak_ng_STATUS espeak_rs_engine_output(const RustEngineLifecycle *, int, int, int, const char *);
espeak_ng_STATUS espeak_rs_engine_cancel(const RustEngineLifecycle *);
espeak_ng_STATUS espeak_rs_engine_synchronize(const RustEngineLifecycle *);
espeak_ng_STATUS espeak_rs_engine_terminate(const RustEngineLifecycle *);
#endif
