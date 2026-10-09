/* Native synthesis driver versus the extracted production controller.
 * SPDX-License-Identifier: GPL-3.0-or-later */
#include "config.h"
#define USE_RUST_CORE 1
#include "test_assert.h"
#include "rust_engine_driver.h"
#include <errno.h>
#include <limits.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>

static uint64_t trace;
static unsigned calls, fills, deliveries, stops, terminations, generations;
static unsigned event_count, bytes_written, mutation, run_policy;
static int mode, initial_mode, delivered, end_delivered, generation, queued, next_clause;
static int options[3], has_callback, allocation_failure, buffers;
static espeak_ng_STATUS voice_result, decode_result;
static unsigned identifier;
static void *user;
static long samples;
static unsigned char pcm[512];
static espeak_EVENT events[6];
static RustOutput output;
static RustEventList event_owner;
static Translator voice_owner, *translator_slot;
static int decoder_token, user_tokens[2];
static espeak_ng_TEXT_DECODER *decoder_slot;
static t_espeak_callback *callback_slot;
static void Number(uint64_t value) { trace = (trace ^ value) * UINT64_C(1099511628211); }
static void Call(unsigned value) { ++calls; Number(value); Number(samples); Number(identifier); Number(user == &user_tokens[1]); }
static void String(const char *value) { Number(value != NULL); if (value) while (*value) Number((unsigned char)*value++); }
static int Value(unsigned field) { TEST_ASSERT(field == 0); return mode; }
static unsigned Encoding(Translator *value) { TEST_ASSERT(value); return value->encoding; }
static espeak_ng_STATUS Voice(const char *name) { Call(1); String(name); if (!voice_result) translator_slot = &voice_owner; return voice_result; }
static espeak_ng_STATUS DefaultVoice(void) { return Voice("fixture-voice"); }
static espeak_ng_TEXT_DECODER *CreateDecoder(void) { Call(2); return allocation_failure ? NULL : (void *)&decoder_token; }
static espeak_ng_STATUS Decode(espeak_ng_TEXT_DECODER *decoder, const void *text, espeak_ng_ENCODING encoding, int flags)
{
    Call(3); Number(decoder != NULL); String(text); Number(encoding); Number((uint32_t)flags);
    for(int i = 0; i < 3; ++i) Number(options[i]);
    return decoder ? decode_result : EINVAL;
}
static void Begin(RustOutput *value) { TEST_ASSERT(value == &output); Call(4); output.start = output.ptr = output.buffer; output.end = output.buffer + output.size; }
static int Fill(void)
{
    Call(5); Number(event_owner.count); ++fills;
    for(unsigned i = 0; i < bytes_written; ++i) pcm[i] = (unsigned char)(i + fills);
    output.ptr = pcm + bytes_written;
    /* The legacy pass counts relative to the owned buffer, not a modified
     * start cursor; this also catches a copied snapshot before fill. */
    output.start = pcm + mutation;
    event_owner.count = event_count;
    for(unsigned i = 0; i < event_count; ++i) {
        events[i] = (espeak_EVENT){ .type = espeakEVENT_WORD, .unique_identifier = identifier, .sample = (int)i, .user_data = user };
    }
    if(mutation) { mode = (mode+1)&3; identifier += 11; user = &user_tokens[1]; }
    return -77; /* Existing controller deliberately ignores this result. */
}
static void Terminate(RustEventList *owner, int index, unsigned id, void *u)
{
    TEST_ASSERT(owner == &event_owner && index >= 0 && index < 6);
    Call(6); ++terminations; Number(index); Number(id); Number(u == &user_tokens[1]);
    events[index] = (espeak_EVENT){ .type = espeakEVENT_LIST_TERMINATED, .unique_identifier = id, .user_data = u };
}
static int Delivery(unsigned op, short *data, int length, espeak_EVENT *list)
{
    Call(op); ++deliveries; Number(data != NULL); Number(length); Number(list != NULL);
    if(list) { Number(list[0].type); Number(list[0].unique_identifier); Number(list[0].user_data == &user_tokens[1]); }
    if(data) { TEST_ASSERT((void *)data == pcm && length >= 0 && (unsigned)length <= bytes_written/2); for(int i = 0; i < length*2; ++i) Number(pcm[i]); }
    if(mutation) { identifier += 7; user = &user_tokens[1]; }
    return data ? delivered : end_delivered;
}
static int Callback(short *data, int length, espeak_EVENT *list) { return Delivery(7, data, length, list); }
static int Alternate(short *data, int length, espeak_EVENT *list) { return Delivery(8, data, length, list); }
static int Play(short *data, int length, espeak_EVENT *list) { return Delivery(9, data, length, list); }
static int Dispatch(short *data, int length, espeak_EVENT *list) { TEST_ASSERT(!data && !length && !list); return Delivery(10, data, length, list); }
static int Generation(void) { Call(11); ++generations; if(mutation) { mode ^= 2; identifier += 13; } return generation; }
static int Queue(void) { Call(12); return queued; }
static int Clause(int control)
{
    Call(13); Number(control); if(control == 2) ++stops;
    if(control == 1 && mutation) { mode ^= 2; callback_slot = has_callback ? Alternate : NULL; identifier += 19; }
    return control == 1 ? next_clause : 0;
}
static int Run(int (*step)(void *), void *context)
{
    Call(14); Number(run_policy);
    if(run_policy == 2) return -1;
    for(unsigned i = 0; i < (run_policy == 1 ? 4u : 1u); ++i) {
        int done = step(context);
        if(run_policy == 3) return -1;
        if(done) return 1;
    }
    return -1; // Simulated runner refusal, never spin a non-finishing pass.
}
static int ReferenceGenerate(PHONEME_LIST *list, int *count, int resume) { TEST_ASSERT(list && count && resume == 1); return Generation(); }
static const RustEngineDriver callbacks = {
    .output = &output, .events = &event_owner, .samples = &samples,
    .options = { &options[0], &options[1], &options[2] },
    .translator = &translator_slot, .decoder = &decoder_slot,
    .encoding = Encoding, .voice = DefaultVoice, .create_decoder = CreateDecoder,
    .decode = Decode, .begin = Begin, .fill = Fill, .terminate_events = Terminate,
    .identifier = &identifier, .user = &user, .value = Value, .callback = &callback_slot,
    .play = Play, .dispatch = Dispatch, .generate = Generation, .queued = Queue,
    .clause = Clause, .run = Run
};
static PHONEME_LIST reference_list[1];
static int reference_count;
#undef USE_PROACTOR
#define USE_PROACTOR 1
#undef ESPEAKNG_DEFAULT_VOICE
#define ESPEAKNG_DEFAULT_VOICE "fixture-voice"
#define outbuf output.buffer
#undef out_ptr
#define out_ptr output.ptr
#define event_list event_owner.events
#define event_list_ix event_owner.count
#define count_samples samples
#define my_mode mode
#define my_unique_identifier identifier
#define my_user_data user
#define synth_callback callback_slot
#define option_ssml options[0]
#define option_phoneme_input options[1]
#define option_endpause options[2]
#define translator translator_slot
#define p_decoder decoder_slot
#define espeak_rs_output output
#define espeak_rs_events event_owner
#define espeak_rs_output_begin Begin
#define espeak_rs_events_terminate Terminate
#define WavegenFill Fill
#define create_events Play
#define dispatch_audio Dispatch
#define Generate ReferenceGenerate
#define phoneme_list reference_list
#define n_phoneme_list reference_count
#define WcmdqUsed Queue
#define SpeakNextClause Clause
#define espeak_ng_SetVoiceByName Voice
#define create_text_decoder CreateDecoder
#define text_decoder_decode_string_multibyte Decode
#define espeak_rs_synthesis_run Run
#define Synthesize ReferenceSynthesize
#define SynthesizeStep ReferenceStep
#include "engine_driver_reference.inc"
static void Reset(unsigned resources)
{
    trace = UINT64_C(1469598103934665603); calls = fills = deliveries = stops = terminations = generations = 0;
    samples = 41; identifier = 777; user = &user_tokens[0]; mode = initial_mode;
    for(int i = 0; i < 3; ++i) options[i] = -1;
    memset(pcm, 0x55, sizeof(pcm));
    for(int i = 0; i < 6; ++i) events[i] = (espeak_EVENT){ .type = espeakEVENT_WORD, .unique_identifier = 456, .sample = -1, .user_data = &user_tokens[0] };
    output = (RustOutput){ pcm, pcm, pcm+256, (buffers & 1) ? pcm : NULL, 256 };
    event_owner = (RustEventList){ (buffers & 2) ? events : NULL, 4, 6 };
    voice_owner.encoding = ESPEAKNG_ENCODING_ISO_8859_5;
    translator_slot = (resources & 1) ? &voice_owner : NULL;
    decoder_slot = (resources & 2) ? (void *)&decoder_token : NULL;
    callback_slot = has_callback ? Callback : NULL;
}
static uint64_t State(unsigned status, int done)
{
    Number(status); Number(done); Number(samples); Number(identifier); Number(user == &user_tokens[1]);
    Number(mode); Number(fills); Number(deliveries); Number(stops); Number(terminations); Number(generations);
    for(int i = 0; i < 3; ++i) Number(options[i]);
    Number(translator_slot != NULL); Number(decoder_slot != NULL); Number(event_owner.count);
    Number(callback_slot == Alternate);
    for(unsigned i = 0; i < sizeof(pcm); ++i) Number(pcm[i]);
    for(int i = 0; i < 6; ++i) { Number(events[i].type); Number(events[i].unique_identifier); Number(events[i].sample); Number(events[i].user_data == &user_tokens[1]); }
    return trace;
}
static void Passes(void)
{
    unsigned comparisons = 0; buffers = 3;
    for(initial_mode = 0; initial_mode < 4; ++initial_mode)
    for(has_callback = 0; has_callback < 2; ++has_callback)
    for(event_count = 0; event_count < 4; ++event_count)
    for(delivered = -1; delivered < 2; ++delivered)
    for(end_delivered = -1; end_delivered < 2; ++end_delivered)
    for(generation = -1; generation < 2; ++generation)
    for(queued = 0; queued < 2; ++queued)
    for(next_clause = -1; next_clause < 2; ++next_clause)
    for(mutation = 0; mutation < 2; ++mutation) {
        bytes_written = (comparisons * 13) % 64;
        Reset(3); SynthesisState state = { 123, ENS_VOICE_NOT_FOUND };
        int done = ReferenceStep(&state); uint64_t expected = State(state.status, done);
        Reset(3); espeak_ng_STATUS status = ENS_VOICE_NOT_FOUND;
        done = espeak_rs_driver_step(&callbacks, 123, &status);
        if(expected != State(status, done)) { fprintf(stderr, "step mismatch mode=%d callback=%d events=%u delivery=%d end=%d gen=%d queue=%d next=%d mutate=%u\n", initial_mode, has_callback, event_count, delivered, end_delivered, generation, queued, next_clause, mutation); TEST_ASSERT(0); }
        ++comparisons;
    }
    printf("%u retained-C synthesis pass comparisons passed\n", comparisons);
}
static void Starts(void)
{
    unsigned comparisons = 0; bytes_written = 9; event_count = 2; generation = 0; queued = 0; next_clause = 0;
    static const int flags[] = { 0, espeakSSML, espeakPHONEMES, espeakENDPAUSE, -1, 0x1234 };
    for(buffers = 0; buffers < 4; ++buffers)
    for(unsigned resources = 0; resources < 4; ++resources)
    for(unsigned failure = 0; failure < 3; ++failure)
    for(allocation_failure = 0; allocation_failure < 2; ++allocation_failure)
    for(run_policy = 0; run_policy < 4; ++run_policy)
    for(unsigned f = 0; f < sizeof(flags)/sizeof(*flags); ++f)
    for(mutation = 0; mutation < 2; ++mutation) {
        initial_mode = comparisons % 4; has_callback = (comparisons/4) % 2;
        delivered = (comparisons/8) % 3 - 1; end_delivered = (comparisons/24) % 3 - 1;
        voice_result = failure == 1 ? ENS_VOICE_NOT_FOUND : ENS_OK;
        decode_result = failure == 2 ? ENS_UNKNOWN_TEXT_ENCODING : ENS_OK;
        Reset(resources);
        uint64_t expected = State(ReferenceSynthesize(123, "input", flags[f]), 0);
        Reset(resources);
        if(expected != State(espeak_rs_driver_synthesize(&callbacks, 123, "input", flags[f]), 0)) { fprintf(stderr, "start mismatch buffers=%d resources=%u fail=%u alloc=%d run=%u flags=%d mutate=%u\n", buffers, resources, failure, allocation_failure, run_policy, flags[f], mutation); TEST_ASSERT(0); }
        ++comparisons;
    }
    printf("%u retained-C synthesis startup comparisons passed\n", comparisons);
}
static void Rejections(void)
{
    buffers = 3; initial_mode = 0; mutation = 0; has_callback = 1; bytes_written = 8;
    Reset(3); samples = LONG_MAX - 2; espeak_ng_STATUS status = ENS_OK;
    TEST_ASSERT(espeak_rs_driver_step(&callbacks, 123, &status) == 1 && status == EINVAL);
    TEST_ASSERT(samples == LONG_MAX - 2 && stops == 1 && !deliveries && !terminations && !generations);
    Reset(3); bytes_written = 257;
    TEST_ASSERT(espeak_rs_driver_step(&callbacks, 123, &status) == 1 && status == EINVAL);
    TEST_ASSERT(samples == 41 && stops == 1 && !deliveries && !terminations && !generations);
    RustEngineDriver invalid = callbacks; invalid.run = NULL;
    TEST_ASSERT(espeak_rs_driver_synthesize(&invalid, 123, "input", 0) == EINVAL);
    TEST_ASSERT(espeak_rs_driver_step(&callbacks, 123, NULL) == -1);
    puts("native cursor/counter overflow and runner admission checks passed");
}
int main(void) { Passes(); Starts(); Rejections(); return 0; }
