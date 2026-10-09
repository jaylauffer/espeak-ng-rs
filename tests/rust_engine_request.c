/* Native request control versus the extracted production controllers.
 * SPDX-License-Identifier: GPL-3.0-or-later */
#include "config.h"
#include "test_assert.h"
#include "rust_engine_request.h"
#include <errno.h>
#include <stdarg.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>

static uint64_t trace;
static uint64_t numbers[2048];
static unsigned number_count;
static unsigned identifier, output_identifier, observed, failure, creations;
static unsigned capabilities, queue_result, synthesis_result, audio_result;
static int mode, next_mode, current[15], saved[15], skips[3], end_slot;
static bool skipping;
static char marker[50];
static void *user;
static int audio_tokens[2], user_token;
static struct audio_object *audio;
static t_espeak_command commands[2];
static unsigned deleted[2];
static void Number(uint64_t value) { TEST_ASSERT(number_count < 2048); numbers[number_count++] = value; trace = (trace ^ value) * UINT64_C(1099511628211); }
static void Call(unsigned op) { Number(op); Number(observed ? output_identifier : 0); }
static void String(const char *s) { Number(s != NULL); if (s) while (*s) Number((unsigned char)*s++); }
static void Wide(const wchar_t *s) { Number(s != NULL); if (s) while (*s) Number((uint32_t)*s++); }
static void Payload(const t_espeak_command *c)
{
    Number(c->type);
    switch(c->type) {
    case ET_TEXT:
        Number(c->u.my_text.unique_identifier); String(c->u.my_text.text);
        Number(c->u.my_text.position); Number(c->u.my_text.position_type);
        Number(c->u.my_text.end_position); Number(c->u.my_text.flags);
        Number(c->u.my_text.user_data != NULL); break;
    case ET_MARK:
        Number(c->u.my_mark.unique_identifier); String(c->u.my_mark.text); String(c->u.my_mark.index_mark);
        Number(c->u.my_mark.end_position); Number(c->u.my_mark.flags);
        Number(c->u.my_mark.user_data != NULL); break;
    case ET_KEY: String(c->u.my_key.key_name); Number(c->u.my_key.user_data != NULL); break;
    case ET_CHAR: Number((uint32_t)c->u.my_char.character); Number(c->u.my_char.user_data != NULL); break;
    case ET_PARAMETER: Number(c->u.my_param.parameter); Number((uint32_t)c->u.my_param.value); Number(c->u.my_param.relative); break;
    case ET_PUNCTUATION_LIST: Wide(c->u.my_punctuation_list); break;
    case ET_TERMINATED_MSG: Number(c->u.my_terminated_msg.unique_identifier); Number(c->u.my_terminated_msg.user_data != NULL); break;
    default: TEST_ASSERT(0);
    }
}
static int Value(unsigned field) { TEST_ASSERT(field == 0); return mode; }
static struct audio_object *Audio(void) { return audio; }
static void Init(int flags)
{
    Call(1); Number((uint32_t)flags); Number(identifier); Number(user != NULL);
    for(int i = 0; i < 15; ++i) current[i] += 31;
    for(int i = 0; i < 3; ++i) skips[i] = (flags & (1 << i)) ? 4 + i : 0;
    /* Leave skipping unchanged to catch an unintended false store. */
}
static espeak_ng_STATUS Synthesis(unsigned id, const void *text, int flags)
{
    Call(2); Number(id); String(text); Number((uint32_t)flags);
    Number(identifier); Number(user != NULL); Number(skipping); Number((uint32_t)end_slot);
    for(int i = 0; i < 3; ++i) Number((uint32_t)skips[i]);
    for(int i = 0; i < 15; ++i) Number((uint32_t)saved[i]);
    for(int i = 0; i < 50; ++i) Number((unsigned char)marker[i]);
    mode = next_mode;
    return synthesis_result;
}
static espeak_ng_STATUS Key(const char *key) { Call(3); String(key); return synthesis_result; }
static espeak_ng_STATUS Character(wchar_t value) { Call(4); Number((uint32_t)value); return synthesis_result; }
static espeak_ng_STATUS Parameter(int parameter, int value, int relative) { Call(5); Number(parameter); Number((uint32_t)value); Number(relative); return synthesis_result; }
static void Punctuation(const wchar_t *list) { Call(6); Wide(list); }
static t_espeak_command *Create(const t_espeak_command *input, size_t size)
{
    TEST_ASSERT(creations < 2); unsigned slot = creations++;
    Call(10); Payload(input); Number(size);
    if (failure & (1u << slot)) return NULL;
    commands[slot] = *input;
    if (input->type == ET_TEXT) commands[slot].u.my_text.unique_identifier = 42;
    if (input->type == ET_MARK) commands[slot].u.my_mark.unique_identifier = 42;
    return &commands[slot];
}
static t_espeak_command *CreateText(const void *text, size_t size, unsigned position, espeak_POSITION_TYPE type, unsigned end, unsigned flags, void *u)
{
    t_espeak_command c = { .type = ET_TEXT, .u.my_text = { 0, (void *)text, position, type, end, flags, u } };
    return Create(&c, size);
}
static t_espeak_command *CreateMark(const void *text, size_t size, const char *mark, unsigned end, unsigned flags, void *u)
{
    t_espeak_command c = { .type = ET_MARK, .u.my_mark = { 0, (void *)text, mark, end, flags, u } };
    return Create(&c, size);
}
static t_espeak_command *CreateTerminated(unsigned id, void *u) { t_espeak_command c = { .type = ET_TERMINATED_MSG, .u.my_terminated_msg = { id, u } }; return Create(&c, 0); }
static t_espeak_command *CreateKey(const char *name, void *u) { t_espeak_command c = { .type = ET_KEY, .u.my_key = { 0, u, name } }; return Create(&c, 0); }
static t_espeak_command *CreateCharacter(wchar_t value, void *u) { t_espeak_command c = { .type = ET_CHAR, .u.my_char = { 0, u, value } }; return Create(&c, 0); }
static t_espeak_command *CreateParameter(espeak_PARAMETER p, int value, int relative) { t_espeak_command c = { .type = ET_PARAMETER, .u.my_param = { p, value, relative } }; return Create(&c, 0); }
static t_espeak_command *CreatePunctuation(const wchar_t *list) { t_espeak_command c = { .type = ET_PUNCTUATION_LIST, .u.my_punctuation_list = list }; return Create(&c, 0); }
static int Delete(t_espeak_command *c)
{
    Call(13); Number(c != NULL);
    if (c) { unsigned slot = (unsigned)(c - commands); TEST_ASSERT(slot < 2 && !deleted[slot]); deleted[slot]++; Payload(c); }
    return c != NULL;
}
static espeak_ng_STATUS Pair(t_espeak_command *a, t_espeak_command *b)
{
    TEST_ASSERT(a && b); Call(11); Payload(a); Payload(b);
    if (!queue_result) { memset(a, 0x77, sizeof(*a)); memset(b, 0x77, sizeof(*b)); }
    /* Callbacks can observe and change the caller slot. The controller must
     * not rewrite it after transferring ownership to the queue. */
    if (observed) output_identifier = 900;
    return queue_result;
}
static espeak_ng_STATUS Single(t_espeak_command *a)
{
    Call(12); Number(a != NULL); if (a) Payload(a);
    if (!a) return EINVAL;
    if (!queue_result) memset(a, 0x77, sizeof(*a));
    return queue_result;
}
static int Flush(struct audio_object *a) { Call(20); Number(a == (void *)&audio_tokens[0]); audio = (void *)&audio_tokens[1]; return audio_result; }
static int Drain(struct audio_object *a) { Call(21); Number(a == (void *)&audio_tokens[0]); audio = (void *)&audio_tokens[1]; return audio_result; }
static const char *AudioError(struct audio_object *a, int error) { Call(22); Number(a == (void *)&audio_tokens[1]); Number(error); return "fixture error"; }
static void Diagnose(const char *operation, const char *message) { Call(23); String(operation); String(message); }
static int Print(FILE *stream, const char *format, ...)
{
    TEST_ASSERT(stream == stderr && strcmp(format, "audio %s error: %s\n") == 0);
    va_list args; va_start(args, format); const char *operation = va_arg(args, const char *); const char *message = va_arg(args, const char *);
    Diagnose(operation, message); va_end(args); return 0;
}
static void Copy(char *out, const char *in, int size) { strncpy(out, in, size); out[size-1] = 0; }
static RustEngineRequest Callbacks(void)
{
    RustEngineRequest cb = {
        .capabilities = capabilities, .value = Value, .audio = Audio,
        .init_text = Init, .synthesize = Synthesis, .key = Key, .character = Character,
        .parameter = Parameter, .punctuation = Punctuation,
        .create = Create, .single = Single, .pair = Pair, .delete_command = Delete,
        .identifier = &identifier, .user = &user, .current = current, .saved = saved,
        .skip = { &skips[0], &skips[1], &skips[2] }, .skipping = &skipping,
        .end = &end_slot, .marker = marker,
        .flush = Flush, .drain = Drain, .audio_error = AudioError, .diagnose = Diagnose
    };
    if (!(capabilities & 1)) { cb.create = NULL; cb.single = NULL; cb.pair = NULL; cb.delete_command = NULL; }
    if (!(capabilities & 2)) { cb.flush = NULL; cb.drain = NULL; cb.audio_error = NULL; cb.diagnose = NULL; }
    return cb;
}
/* Compile the original eight bodies in all four async/audio combinations.
 * Macro aliases affect only the extracted oracle, never production code. */
static struct { int *parameter; } reference_stack[1] = {{ current }};
#define my_mode mode
#define my_audio audio
#define my_unique_identifier identifier
#define my_user_data user
#define param_stack reference_stack
#define saved_parameters saved
#define N_SPEECH_PARAM 15
#define skip_characters skips[0]
#define skip_words skips[1]
#define skip_sentences skips[2]
#define skipping_text skipping
#define end_character_position end_slot
#define skip_marker marker
#define InitText Init
#define Synthesize Synthesis
#define strncpy0 Copy
#define audio_object_flush Flush
#define audio_object_drain Drain
#define audio_object_strerror AudioError
#define fprintf Print
#define sync_espeak_Key Key
#define sync_espeak_Char Character
#define sync_espeak_SetPunctuationList Punctuation
#define SetParameter Parameter
#define create_espeak_text CreateText
#define create_espeak_mark CreateMark
#define create_espeak_terminated_msg CreateTerminated
#define create_espeak_key CreateKey
#define create_espeak_char CreateCharacter
#define create_espeak_parameter CreateParameter
#define create_espeak_punctuation_list CreatePunctuation
#define delete_espeak_command Delete
#define fifo_add_commands Pair
#define fifo_add_command Single
#define sync_espeak_Synth REF(sync_espeak_Synth)
#define sync_espeak_Synth_Mark REF(sync_espeak_Synth_Mark)
#define espeak_ng_Synthesize REF(espeak_ng_Synthesize)
#define espeak_ng_SynthesizeMark REF(espeak_ng_SynthesizeMark)
#define espeak_ng_SpeakKeyName REF(espeak_ng_SpeakKeyName)
#define espeak_ng_SpeakCharacter REF(espeak_ng_SpeakCharacter)
#define espeak_ng_SetParameter REF(espeak_ng_SetParameter)
#define espeak_ng_SetPunctuationList REF(espeak_ng_SetPunctuationList)
#define DISPATCH() \
static espeak_ng_STATUS REF(Submit)(const t_espeak_command *c, size_t size, unsigned *id) { \
    switch(c->type) { \
    case ET_TEXT: { const t_espeak_text *a = &c->u.my_text; return espeak_ng_Synthesize(a->text, size, a->position, a->position_type, a->end_position, a->flags, id, a->user_data); } \
    case ET_MARK: { const t_espeak_mark *a = &c->u.my_mark; return espeak_ng_SynthesizeMark(a->text, size, a->index_mark, a->end_position, a->flags, id, a->user_data); } \
    case ET_KEY: return espeak_ng_SpeakKeyName(c->u.my_key.key_name); \
    case ET_CHAR: return espeak_ng_SpeakCharacter(c->u.my_char.character); \
    case ET_PARAMETER: return espeak_ng_SetParameter(c->u.my_param.parameter, c->u.my_param.value, c->u.my_param.relative); \
    case ET_PUNCTUATION_LIST: return espeak_ng_SetPunctuationList(c->u.my_punctuation_list); \
    default: TEST_ASSERT(0); return EINVAL; } }
#undef USE_ASYNC
#define USE_ASYNC 0
#define HAVE_AUDIO_OUTPUT 0
#define REF(name) reference_0_##name
#include "engine_request_reference.inc"
DISPATCH()
#undef REF
#undef USE_ASYNC
#define USE_ASYNC 1
#define REF(name) reference_1_##name
#include "engine_request_reference.inc"
DISPATCH()
#undef REF
#undef USE_ASYNC
#define USE_ASYNC 0
#undef HAVE_AUDIO_OUTPUT
#define HAVE_AUDIO_OUTPUT 1
#define REF(name) reference_2_##name
#include "engine_request_reference.inc"
DISPATCH()
#undef REF
#undef USE_ASYNC
#define USE_ASYNC 1
#define REF(name) reference_3_##name
#include "engine_request_reference.inc"
DISPATCH()
#undef REF
#undef fprintf
#undef sync_espeak_Synth
#undef sync_espeak_Synth_Mark
#undef espeak_ng_Synthesize
#undef espeak_ng_SynthesizeMark
#undef espeak_ng_SpeakKeyName
#undef espeak_ng_SpeakCharacter
#undef espeak_ng_SetParameter
#undef espeak_ng_SetPunctuationList
static espeak_ng_STATUS (*reference_submit[4])(const t_espeak_command *, size_t, unsigned *) = { reference_0_Submit, reference_1_Submit, reference_2_Submit, reference_3_Submit };
static espeak_ng_STATUS (*reference_text[4])(unsigned, const void *, unsigned, espeak_POSITION_TYPE, unsigned, unsigned, void *) = { reference_0_sync_espeak_Synth, reference_1_sync_espeak_Synth, reference_2_sync_espeak_Synth, reference_3_sync_espeak_Synth };
static espeak_ng_STATUS (*reference_mark[4])(unsigned, const void *, const char *, unsigned, unsigned, void *) = { reference_0_sync_espeak_Synth_Mark, reference_1_sync_espeak_Synth_Mark, reference_2_sync_espeak_Synth_Mark, reference_3_sync_espeak_Synth_Mark };
static void Reset(void)
{
    trace = UINT64_C(1469598103934665603); number_count = 0; creations = 0; identifier = 777; output_identifier = 888;
    user = &user_token; end_slot = -99; skipping = true; memset(marker, 0x55, sizeof(marker));
    memset(commands, 0, sizeof(commands)); memset(deleted, 0, sizeof(deleted)); audio = (void *)&audio_tokens[0];
    for(int i = 0; i < 15; ++i) { current[i] = i * 7; saved[i] = -i; }
    for(int i = 0; i < 3; ++i) skips[i] = i + 17;
}
static uint64_t State(espeak_ng_STATUS result)
{
    Number(result); Number(identifier); Number(output_identifier); Number(user != NULL); Number(mode);
    Number(skipping); Number((uint32_t)end_slot); Number(creations);
    for(int i = 0; i < 15; ++i) { Number((uint32_t)current[i]); Number((uint32_t)saved[i]); }
    for(int i = 0; i < 3; ++i) Number((uint32_t)skips[i]);
    for(int i = 0; i < 50; ++i) Number((unsigned char)marker[i]);
    Number(deleted[0]); Number(deleted[1]); return trace;
}
int main(void)
{
    static const int modes[] = { -1, 0, 1, 2, 3, 4 };
    static const unsigned results[] = { ENS_OK, ENS_SPEECH_STOPPED, ENS_AUDIO_ERROR };
    static const char long_mark[] = "abcdefghijklmnopqrstuvwxyzABCDEFGHIJKLMNOPQRSTUVWXYZ123456789";
    static const wchar_t punctuation[] = { '.', 0x2014, 0 };
    unsigned admissions = 0, preparations = 0;
    for(capabilities = 0; capabilities < 4; ++capabilities)
    for(unsigned m = 0; m < sizeof(modes)/sizeof(*modes); ++m)
    for(unsigned kind = 0; kind < 6; ++kind)
    for(failure = 0; failure < 4; ++failure)
    for(unsigned q = 0; q < 3; ++q)
    for(observed = 0; observed < 2; ++observed) {
        t_espeak_command c = { .type = kind };
        switch(kind) {
        case ET_TEXT: c.u.my_text = (t_espeak_text){ 999, "hello", UINT32_MAX, POS_WORD, 8, 5, &user_token }; break;
        case ET_MARK: c.u.my_mark = (t_espeak_mark){ 999, "world", long_mark, UINT32_MAX, 2, NULL }; break;
        case ET_KEY: c.u.my_key = (t_espeak_key){ 0, NULL, "escape" }; break;
        case ET_CHAR: c.u.my_char = (t_espeak_character){ 0, NULL, 0x2014 }; break;
        case ET_PARAMETER: c.u.my_param = (t_espeak_parameter){ espeakRATE, -25, 1 }; break;
        case ET_PUNCTUATION_LIST: c.u.my_punctuation_list = observed ? punctuation : NULL; break;
        }
        queue_result = q ? (q == 1 ? ENS_FIFO_BUFFER_FULL : EINVAL) : ENS_OK;
        synthesis_result = results[q]; audio_result = q ? 17 : 0; next_mode = modes[(m+1)%6];
        Reset(); mode = modes[m];
        uint64_t expected = State(reference_submit[capabilities](&c, 6, observed ? &output_identifier : NULL));
        uint64_t expected_numbers[2048]; unsigned expected_count = number_count;
        memcpy(expected_numbers, numbers, number_count * sizeof(*numbers));
        Reset(); mode = modes[m]; RustEngineRequest cb = Callbacks();
        uint64_t actual = State(espeak_rs_request_submit(&cb, &c, 6, observed ? &output_identifier : NULL));
        if (expected != actual) {
            fprintf(stderr, "request mismatch caps=%u mode=%d kind=%u fail=%u q=%u observed=%u counts=%u/%u\n", capabilities, modes[m], kind, failure, q, observed, expected_count, number_count);
            for(unsigned n = 0; n < expected_count && n < number_count; ++n) if(expected_numbers[n] != numbers[n]) { fprintf(stderr, "first difference %u: %llu/%llu\n", n, (unsigned long long)expected_numbers[n], (unsigned long long)numbers[n]); break; }
        }
        TEST_ASSERT(expected == actual); ++admissions;
    }
    observed = 0;
    for(capabilities = 0; capabilities < 4; ++capabilities)
    for(unsigned type = 0; type < 5; ++type)
    for(unsigned flags = 0; flags < 8; ++flags)
    for(unsigned pos = 0; pos < 3; ++pos)
    for(unsigned mark_kind = 0; mark_kind < 4; ++mark_kind)
    for(unsigned q = 0; q < 3; ++q) {
        unsigned position = pos == 0 ? 0 : pos == 1 ? 7 : UINT32_MAX;
        t_espeak_text text = { 123, "text", position, type, 99, flags, &user_token };
        const char *name = mark_kind == 0 ? NULL : mark_kind == 1 ? "" : mark_kind == 2 ? "short" : long_mark;
        t_espeak_mark mark = { 234, "mark", name, UINT32_MAX, flags, NULL };
        synthesis_result = results[q]; audio_result = q ? 77 : 0; next_mode = q == 2 ? 0 : 2;
        Reset(); mode = 2;
        uint64_t expected = State(reference_text[capabilities](text.unique_identifier, text.text, text.position, text.position_type, text.end_position, text.flags, text.user_data));
        Reset(); mode = 2; RustEngineRequest cb = Callbacks();
        TEST_ASSERT(expected == State(espeak_rs_request_synthesize(&cb, &text))); ++preparations;
        Reset(); mode = 2;
        expected = State(reference_mark[capabilities](mark.unique_identifier, mark.text, mark.index_mark, mark.end_position, mark.flags, mark.user_data));
        Reset(); mode = 2; cb = Callbacks();
        TEST_ASSERT(expected == State(espeak_rs_request_mark(&cb, &mark))); ++preparations;
    }
    printf("%u request admission and %u synchronous preparation comparisons passed\n", admissions, preparations);
    return 0;
}
