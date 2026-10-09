/* Native engine lifecycle versus the retained production controllers.
 * SPDX-License-Identifier: GPL-3.0-or-later */
#include "config.h"
#define USE_RUST_CORE 1
#include "test_assert.h"
#include <errno.h>
#include <locale.h>
#include <limits.h>
#include <stdint.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <time.h>
#include "rust_data.h"
#include "rust_engine_lifecycle.h"
#include "readclause.h"
#include "synthesize.h"

/* Exercise the complete controller feature set using deterministic host
 * primitives even in synchronous/audio-off production configurations. */
#undef USE_ASYNC
#undef USE_PROACTOR
#undef USE_MBROLA
#define USE_ASYNC 1
#define USE_PROACTOR 1
#define USE_MBROLA 1
#define HAVE_AUDIO_OUTPUT 1

static uint64_t trace;
static unsigned calls, allocation_failure, locale_success, locale_calls;
static unsigned destroys, native_observation;
static espeak_ng_STATUS load_result, sync_result;
static int rate, output_mode, output_rate, voice_rate, error_code;
static int defaults[15] = { 0, 175, 100, 50, 50, 0, 0, 0, 0, 0, 100, 0, 0, 0, 0 };
static int current[15], saved[15], capitals, punctuation, phonemes, phoneme_events, echo;
static Translator *translator_slot;
static espeak_ng_TEXT_DECODER *decoder_slot;
static espeak_VOICE selected_voice;
static int audio_token, translator_token, decoder_token, context_token;
static struct audio_object *audio;
static void Number(uint64_t value) { trace = (trace ^ value) * UINT64_C(1099511628211); }
static void Call(unsigned index) { ++calls; Number(index); }
static void String(const char *bytes) { if (!bytes) { Number(0); return; } Number(1); while (*bytes) Number((unsigned char)*bytes++); }
#define ACTION(name, index) static void name(void) { Call(index); }
ACTION(Config, 0) ACTION(SynthesisInit, 1) ACTION(NamesInit, 2) ACTION(QueueInit, 3)
ACTION(QueueStop, 4) ACTION(QueueTerminate, 5) ACTION(EventClear, 6) ACTION(EventTerminate, 7)
ACTION(PhonemeRelease, 8) ACTION(VoiceListRelease, 9) ACTION(CurrentVoiceRelease, 10)
ACTION(AlternateRelease, 11) ACTION(DictionaryRelease, 12) ACTION(WaveFinish, 13)
ACTION(NamesRelease, 14) ACTION(IconsRelease, 15) ACTION(MbrolaRelease, 16)
ACTION(EventsRelease, 20) ACTION(OutputRelease, 21)
static espeak_ng_STATUS Stop(void) { QueueStop(); return ENS_AUDIO_ERROR; }
static espeak_ng_STATUS ClearEvents(void) { EventClear(); return ENS_AUDIO_ERROR; }
static char *Locale(int category, const char *name)
{
    TEST_ASSERT(category == LC_CTYPE); Call(30); String(name);
    return locale_calls++ == locale_success ? (char *)name : NULL;
}
static espeak_ng_STATUS Load(int *value, espeak_ng_ERROR_CONTEXT *context)
{
    Call(31); Number(*value); *value = 16000;
    if (load_result && context) *context = (espeak_ng_ERROR_CONTEXT)&context_token;
    return load_result;
}
static void Wave(int value, int factor) { Call(32); Number(value); Number(factor); }
static espeak_VOICE *GetVoice(void) { Call(17); return &selected_voice; }
static void ClearVoice(void) { memset(GetVoice(), 0, sizeof(selected_voice)); }
static void Stack(espeak_VOICE *value, const char *name) { TEST_ASSERT(!value && name && !*name); Call(18); }
static void ResetStack(void) { Stack(NULL, ""); }
static void VoiceResetFake(int value) { TEST_ASSERT(value == 0); Call(19); }
static void ResetVoice(void) { VoiceResetFake(0); }
static espeak_ng_STATUS Parameter(int index, int value, int relative)
{
    TEST_ASSERT(index >= 0 && index < 15 && relative == 0);
    Call(33); Number(index); Number((uint32_t)value); current[index] = value;
    /* Catch a copied/borrowed saved snapshot across callbacks. */
    if (index + 1 < 15) saved[index + 1]++;
    return ENS_AUDIO_ERROR;
}
static long Clock(void) { Call(34); return 1234567; }
static void Seed(long value) { Call(35); Number(value); }
static struct audio_object *Create(const char *device, const char *app, const char *description)
{
    Call(36); String(device); String(app); String(description);
    return (struct audio_object *)&audio_token;
}
static void Close(struct audio_object *pointer)
{
    if (!pointer) return;
    TEST_ASSERT(pointer == (struct audio_object *)&audio_token);
    if (native_observation) TEST_ASSERT(espeak_rs_engine_audio() == NULL);
    Call(37);
}
static void Destroy(struct audio_object *pointer) {
    if (!pointer) return;
    TEST_ASSERT(pointer == (struct audio_object *)&audio_token);
    if (native_observation) TEST_ASSERT(espeak_rs_engine_audio() == NULL);
    Call(38); ++destroys;
}
static int Flush(struct audio_object *pointer) { Call(39); Number(pointer != NULL); return 99; }
static int ReserveOutput(size_t bytes) { Call(40); Number(bytes); return allocation_failure == 1 ? -1 : 0; }
static int ReserveEvents(int count) { Call(41); Number(count); return allocation_failure == 2 ? -1 : 0; }
static int ReferenceOutputReserve(RustOutput *owner, size_t bytes) { (void)owner; return ReserveOutput(bytes); }
static int ReferenceEventsReserve(RustEventList *owner, int count) { (void)owner; return ReserveEvents(count); }
static void ReferenceOutputRelease(RustOutput *owner) { (void)owner; OutputRelease(); }
static void ReferenceEventsRelease(RustEventList *owner) { (void)owner; EventsRelease(); }
static espeak_ng_STATUS Synchronize(void) { Call(42); return sync_result; }
static void DestroyTranslator(Translator *pointer)
{
    TEST_ASSERT(!pointer || pointer == (Translator *)&translator_token);
    if (native_observation) TEST_ASSERT(translator_slot == NULL);
    Call(43); Number(pointer != NULL);
}
static void DestroyDecoder(espeak_ng_TEXT_DECODER *pointer)
{
    TEST_ASSERT(pointer == (espeak_ng_TEXT_DECODER *)&decoder_token);
    if (native_observation) TEST_ASSERT(decoder_slot == NULL);
    Call(44);
}
static const RustEngineLifecycle callbacks = {
    .capabilities = 15,
    .actions = { Config, SynthesisInit, NamesInit, QueueInit, QueueStop, QueueTerminate, EventClear, EventTerminate,
        PhonemeRelease, VoiceListRelease, CurrentVoiceRelease, AlternateRelease, DictionaryRelease, WaveFinish,
        NamesRelease, IconsRelease, MbrolaRelease, ClearVoice, ResetStack, ResetVoice, EventsRelease, OutputRelease },
    .locale = Locale, .ctype = LC_CTYPE, .load = Load, .wave_init = Wave,
    .defaults = defaults, .current = current, .saved = saved, .capitals = &capitals, .punctuation = &punctuation,
    .phonemes = &phonemes, .phoneme_events = &phoneme_events, .echo = &echo, .parameter = Parameter,
    .clock = Clock, .seed = Seed, .create_audio = Create, .close_audio = Close, .destroy_audio = Destroy, .flush_audio = Flush,
    .output_reserve = ReserveOutput, .events_reserve = ReserveEvents, .synchronize = Synchronize,
    .translator = &translator_slot, .destroy_translator = DestroyTranslator, .decoder = &decoder_slot, .destroy_decoder = DestroyDecoder
};

/* Adapt only the controller's external state and primitive names. */
static struct { int *parameter; } reference_stack[1] = {{ current }};
static int embedded[7];
static const int min_buffer_length = 60;
#define my_mode output_mode
#define my_audio audio
#define out_samplerate output_rate
#define voice_samplerate voice_rate
#define err error_code
#define samplerate rate
#define param_stack reference_stack
#define saved_parameters saved
#define param_defaults defaults
#define option_capitals capitals
#define option_punctuation punctuation
#define option_phonemes phonemes
#define option_phoneme_events phoneme_events
#define embedded_value embedded
#define translator translator_slot
#define p_decoder decoder_slot
#define ENGINE_STORE(field, variable, value) ((variable) = (value))
#define espeak_ng_InitializeOutput ReferenceOutput
#define espeak_ng_Initialize ReferenceInitialize
#define espeak_ng_Cancel ReferenceCancel
#define espeak_ng_Synchronize ReferenceSynchronize
#define espeak_ng_Terminate ReferenceTerminate
#define setlocale Locale
#define LoadPhData Load
#define WavegenInit Wave
#define LoadConfig Config
#define espeak_GetCurrentVoice GetVoice
#define SetVoiceStack Stack
#define SynthesizeInit SynthesisInit
#define InitNamedata NamesInit
#define VoiceReset VoiceResetFake
#define SetParameter Parameter
#define time(ignore) Clock()
#define espeak_srand Seed
#define create_audio_device_object Create
#define fifo_init QueueInit
#define fifo_stop Stop
#define fifo_terminate QueueTerminate
#define event_clear_all ClearEvents
#define event_terminate EventTerminate
#define fifo_synchronize Synchronize
#define audio_object_close Close
#define audio_object_destroy Destroy
#define audio_object_flush Flush
#define espeak_rs_output_reserve ReferenceOutputReserve
#define espeak_rs_events_reserve ReferenceEventsReserve
#define espeak_rs_output_release ReferenceOutputRelease
#define espeak_rs_events_release ReferenceEventsRelease
#define FreePhData PhonemeRelease
#define FreeVoiceList VoiceListRelease
#define FreeCurrentVoice CurrentVoiceRelease
#define DeleteTranslator DestroyTranslator
#define FreeAlternateTranslators AlternateRelease
#define FreeDictionaryCache DictionaryRelease
#define destroy_text_decoder DestroyDecoder
#define WavegenFini WaveFinish
#define FreeNamedata NamesRelease
#define FreeSoundIcons IconsRelease
#define FreeMbrolaTable MbrolaRelease
#include "engine_lifecycle_reference.inc"

static void Reset(void)
{
    trace = UINT64_C(1469598103934665603); calls = destroys = locale_calls = native_observation = 0;
    memset(&selected_voice, 0xa5, sizeof(selected_voice));
    for (int i = 0; i < 15; ++i) current[i] = saved[i] = reference_stack[0].parameter[i] = i * 3;
    capitals = 4; punctuation = 2; phonemes = 99; phoneme_events = 77; echo = embedded[EMBED_T] = 17;
    translator_slot = (Translator *)&translator_token;
    decoder_slot = (espeak_ng_TEXT_DECODER *)&decoder_token;
    output_mode = 1; output_rate = 0; voice_rate = 22050; error_code = 0; audio = NULL;
}
static void Initializations(void)
{
    unsigned pairs = 0;
    for (unsigned locale = 0; locale <= 4; ++locale)
        for (unsigned failure = 0; failure < 4; ++failure) {
            locale_success = locale; load_result = failure ? (espeak_ng_STATUS)(0x100000ff + (failure << 8)) : ENS_OK;
            espeak_ng_ERROR_CONTEXT a = NULL, b = NULL;
            Reset(); espeak_ng_STATUS expected = ReferenceInitialize(&a);
            uint64_t hash = trace; unsigned count = calls;
            int expected_current[15], expected_saved[15];
            memcpy(expected_current, current, sizeof(expected_current));
            memcpy(expected_saved, saved, sizeof(saved));
            Reset();
            TEST_ASSERT(espeak_rs_engine_initialize(&callbacks, &b) == expected);
            TEST_ASSERT(a == b && trace == hash && calls == count);
            if (!failure) TEST_ASSERT(memcmp(current, expected_current, sizeof(current)) == 0);
            TEST_ASSERT(memcmp(saved, expected_saved, sizeof(saved)) == 0);
            TEST_ASSERT(phonemes == (failure ? 99 : 0) && phoneme_events == (failure ? 77 : 0));
            if (!failure) { unsigned char zero[sizeof(selected_voice)] = {0}; TEST_ASSERT(memcmp(&selected_voice, zero, sizeof(selected_voice)) == 0); }
            ++pairs;
        }
    printf("engine initialization: %u retained-C locale/load/order/state pairs\n", pairs);
}
static void Outputs(void)
{
    unsigned pairs = 0;
    for (int mode = -1; mode <= 7; ++mode)
        for (int length = -1; length <= 301; length += 7)
            for (int r = 0; r <= 2; ++r)
                for (unsigned failure = 0; failure <= 2; ++failure) {
                    allocation_failure = failure; rate = r == 0 ? 0 : r == 1 ? 22050 : 48000;
                    Reset(); espeak_ng_STATUS expected = ReferenceOutput((espeak_ng_OUTPUT_MODE)mode, length, "\xff/device");
                    uint64_t hash = trace; unsigned count = calls;
                    TEST_ASSERT(espeak_rs_engine_audio() == NULL);
                    Reset();
                    TEST_ASSERT(espeak_rs_engine_output(&callbacks, mode, length, rate, "\xff/device") == expected);
                    TEST_ASSERT(trace == hash && calls == count);
                    TEST_ASSERT(espeak_rs_engine_value(RUST_ENGINE_MODE) == mode && espeak_rs_engine_value(RUST_ENGINE_OUTPUT_RATE) == 0);
                    native_observation = 1;
                    TEST_ASSERT(espeak_rs_engine_terminate(&callbacks) == ENS_OK);
                    TEST_ASSERT(espeak_rs_engine_audio() == NULL);
                    ++pairs;
                }
    allocation_failure = 0;
    Reset();
    TEST_ASSERT(espeak_rs_engine_output(&callbacks, 1, INT_MAX, 22050, NULL) == ENOMEM);
    TEST_ASSERT(calls == 0); /* overflow rejected before either allocation */
    printf("engine output: %u retained-C mode/rate/buffer/allocation pairs\n", pairs);
}
static void CancellationAndTeardown(void)
{
    unsigned pairs = 0;
    for (int mode = 0; mode < 4; ++mode) {
        allocation_failure = 0; Reset();
        TEST_ASSERT(espeak_rs_engine_output(&callbacks, mode, 60, 22050, NULL) == ENS_OK);
        Reset(); output_mode = mode; audio = espeak_rs_engine_audio();
        TEST_ASSERT(ReferenceCancel() == ENS_OK);
        uint64_t hash = trace; unsigned count = calls; int expected[15]; memcpy(expected, current, sizeof(expected));
        Reset();
        TEST_ASSERT(espeak_rs_engine_cancel(&callbacks) == ENS_OK);
        TEST_ASSERT(trace == hash && calls == count && echo == 0 && memcmp(current, expected, sizeof(current)) == 0);
        Reset(); output_mode = mode; audio = espeak_rs_engine_audio();
        TEST_ASSERT(ReferenceTerminate() == ENS_OK); hash = trace; count = calls;
        Reset(); native_observation = 1;
        TEST_ASSERT(espeak_rs_engine_terminate(&callbacks) == ENS_OK);
        TEST_ASSERT(trace == hash && calls == count && !translator_slot && !decoder_slot && !espeak_rs_engine_audio());
        ++pairs;
    }
    /* Confirm the old mode-switch leak and the native ownership correction. */
    Reset(); ReferenceOutput(ENOUTPUT_MODE_SPEAK_AUDIO, 60, NULL); ReferenceOutput(0, 60, NULL); ReferenceTerminate();
    TEST_ASSERT(audio != NULL && destroys == 0);
    Reset(); espeak_rs_engine_output(&callbacks, 2, 60, 22050, NULL); espeak_rs_engine_output(&callbacks, 0, 60, 22050, NULL);
    native_observation = 1; espeak_rs_engine_terminate(&callbacks);
    TEST_ASSERT(destroys == 1 && !espeak_rs_engine_audio());
    espeak_rs_engine_terminate(&callbacks); TEST_ASSERT(destroys == 1);
    printf("engine cancel/teardown: %u retained-C pairs and mode-switch cleanup regression\n", pairs);
}
static void Waits(void)
{
    for (unsigned failure = 0; failure < 2; ++failure) {
        sync_result = failure ? EINVAL : ENS_OK;
        Reset(); error_code = ENS_AUDIO_ERROR;
        espeak_ng_STATUS expected = ReferenceSynchronize(); uint64_t hash = trace; unsigned count = calls;
        int error = error_code;
        Reset(); espeak_rs_engine_store(RUST_ENGINE_ERROR, ENS_AUDIO_ERROR);
        TEST_ASSERT(espeak_rs_engine_synchronize(&callbacks) == expected);
        TEST_ASSERT(trace == hash && calls == count && espeak_rs_engine_value(RUST_ENGINE_ERROR) == error);
    }
    puts("engine synchronize: retained error snapshot/reset and failed-wait behavior");
}
int main(void) { Initializations(); Outputs(); CancellationAndTeardown(); Waits(); return 0; }
