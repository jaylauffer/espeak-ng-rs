/* Native dispatch versus the unchanged production audio/event controllers.
 * SPDX-License-Identifier: GPL-3.0-or-later */
#include "config.h"
#include "test_assert.h"
#include "rust_engine_audio.h"
#include <errno.h>
#include <limits.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>

static int values[4], initial_mode, initial_rate, initial_output, event_kind;
static int enabled, opened, written, event_result, queued_ms, mutation, available;
static unsigned initial_count, present_callback, present_pcm;
static unsigned writes, opens, closes, admissions, callbacks_called, diagnostics;
static long samples;
static int offset, audio_tokens[2], current_audio;
static short pcm[8];
static espeak_EVENT list[8];
static RustEventList owner;
static uint64_t trace;
static void Number(uint64_t value) { trace = (trace ^ value) * UINT64_C(1099511628211); }
static void Call(unsigned op) { Number(op); for(int i=0; i<4; ++i) Number((uint32_t)values[i]); Number(owner.count); Number(current_audio); }
static int Value(unsigned field) { TEST_ASSERT(field < 4); return values[field]; }
static void Store(unsigned field, int value) { TEST_ASSERT(field < 4); values[field] = value; }
static struct audio_object *Audio(void) { return available ? (void *)&audio_tokens[current_audio] : NULL; }
static int Enabled(void) { Call(1); return enabled; }
static void Close(struct audio_object *audio) { TEST_ASSERT(audio == Audio()); Call(2); ++closes; if(mutation) { current_audio ^= 1; values[2] = 11025; } }
static int Open(struct audio_object *audio, int format, int rate, int channels)
{
    TEST_ASSERT(audio == Audio() && format == 37 && channels == 1);
    Call(3); ++opens; Number(rate);
    if(mutation) { values[0] = (values[0]+1)&3; values[2] = 16000; owner.count = 4; }
    return opened;
}
static int Write(struct audio_object *audio, const void *data, size_t bytes)
{
    TEST_ASSERT(audio == Audio() && data == pcm && bytes <= sizeof(pcm));
    Call(4); ++writes; Number(bytes); for(size_t i=0; i<bytes/2; ++i) Number(pcm[i]);
    if(mutation) { values[0] ^= 1; list[0].type = espeakEVENT_WORD; list[0].length = 0; }
    return written;
}
static void Diagnostic(unsigned op, int error) { TEST_ASSERT(op < 3); Call(5); ++diagnostics; Number(op); Number((uint32_t)error); }
static int error_argument;
static const char *ErrorString(struct audio_object *audio, int error) { TEST_ASSERT(audio == Audio()); error_argument = error; return "fixture"; }
static int Print(FILE *stream, const char *format, ...)
{
    TEST_ASSERT(stream == stderr);
    Diagnostic(strstr(format,"reopen") ? 1 : strstr(format,"write") ? 2 : 0, error_argument);
    return 0;
}
static void InitEvents(void) { Call(6); if(mutation) { values[0] = 3; owner.count = 2; } }
static int Latency(struct audio_object *audio) { TEST_ASSERT(audio == Audio()); Call(7); return queued_ms; }
static void Event(const espeak_EVENT *event)
{
    Number(event != NULL);
    if(event) { TEST_ASSERT(event >= list && event < list+8); Number(event-list); Number(event->type); Number(event->length); Number(event->sample); Number(event->id.number); }
}
static espeak_ng_STATUS Declare(const espeak_EVENT *event, int delay)
{
    Call(8); ++admissions; Event(event); Number(delay);
    if(mutation) { values[0] ^= 1; owner.count = 3; list[1].length = 0; }
    return (espeak_ng_STATUS)event_result;
}
static int Callback(short *data, int length, espeak_EVENT *event)
{
    Call(9); ++callbacks_called; Number(data != NULL); Number(length); Event(event);
    if(mutation) { owner.count = 2; values[0] = 2; }
    return -91; /* Dispatch deliberately ignores caller return value. */
}
static t_espeak_callback *callback_slot;
static const RustEngineAudio table = {
    .capabilities = 7, .format = 37, .value = Value, .store = Store, .audio = Audio,
    .enabled = Enabled, .close = Close, .open = Open, .write = Write, .diagnostic = Diagnostic,
    .event_init = InitEvents, .latency = Latency, .declare = Declare,
    .samples = &samples, .mbrola_delay = &offset, .callback = &callback_slot, .events = &owner
};
#undef USE_ASYNC
#define USE_ASYNC 1
#undef USE_PROACTOR
#define USE_PROACTOR 1
#undef USE_RUST_AUDIO
#define USE_RUST_AUDIO 1
#undef USE_MBROLA
#define USE_MBROLA 1
#define HAVE_AUDIO_OUTPUT 1
#define AUDIO_OBJECT_FORMAT_S16LE 37
#define my_mode values[0]
#define out_samplerate values[1]
#define voice_samplerate values[2]
#define err values[3]
#define RUST_ENGINE_VOICE_RATE 2
#define RUST_ENGINE_OUTPUT_RATE 1
#define RUST_ENGINE_ERROR 3
#define ENGINE_STORE(field, variable, value) Store(field, value)
#define count_samples samples
#define mbrola_delay offset
#define my_audio Audio()
#define event_list_ix owner.count
#define synth_callback callback_slot
#define fifo_is_command_enabled Enabled
#define audio_object_close Close
#define audio_object_open Open
#define audio_object_write Write
#define audio_object_strerror ErrorString
#define fprintf Print
#define event_init InitEvents
#define espeak_rs_audio_latency_ms Latency
#define espeak_rs_event_declare_wait Declare
#define dispatch_audio ReferenceDispatch
#define create_events ReferenceEvents
#define declare_event ReferenceDeclare
#include "engine_audio_reference.inc"
#undef fprintf
static void Reset(void)
{
    trace=UINT64_C(1469598103934665603); writes=opens=closes=admissions=callbacks_called=diagnostics=0;
    values[0]=initial_mode; values[1]=initial_output; values[2]=initial_rate; values[3]=67;
    current_audio=0; samples=10000; offset=123; callback_slot=present_callback ? Callback : NULL;
    owner=(RustEventList){ list, (int)initial_count, 8 };
    for(int i=0; i<8; ++i) list[i]=(espeak_EVENT){ .type=event_kind, .length=i%2, .sample=500+i*100, .id.number=initial_rate };
    for(int i=0; i<8; ++i) pcm[i]=(short)(i*317);
}
static uint64_t State(int result)
{
    Number((uint32_t)result); Call(99); Number(writes); Number(opens); Number(closes); Number(admissions); Number(callbacks_called); Number(diagnostics);
    for(int i=0; i<8; ++i) { Number(list[i].type); Number(list[i].length); Number(list[i].sample); Number(list[i].id.number); }
    return trace;
}
int main(void)
{
    unsigned cases=0, declarations=0;
    const int modes[]={-1,0,1,2,3,4}; const int rates[]={0,16000,22050}; const int outputs[]={0,16000,22050};
    const int kinds[]={-1,0,espeakEVENT_WORD,espeakEVENT_MARK,espeakEVENT_MSG_TERMINATED,espeakEVENT_SAMPLERATE};
    const int errors[]={-1,0,7};
    for(unsigned m=0; m<6; ++m) for(unsigned r=0; r<3; ++r) for(unsigned o=0; o<3; ++o)
    for(unsigned k=0; k<6; ++k) for(unsigned e=0; e<3; ++e) for(unsigned bits=0; bits<32; ++bits) {
        initial_mode=modes[m]; initial_rate=rates[r]; initial_output=outputs[o]; event_kind=kinds[k];
        enabled=bits&1; mutation=(bits>>1)&1; present_callback=(bits>>2)&1; present_pcm=(bits>>3)&1; available=(bits>>4)&1;
        opened=errors[e]; written=errors[(e+1)%3]; event_result=e ? ENS_EVENT_BUFFER_FULL : ENS_OK; queued_ms=777;
        for(initial_count=0; initial_count<=4; ++initial_count) {
            Reset(); int old=ReferenceEvents(present_pcm ? pcm : NULL, 3, list); uint64_t expected=State(old);
            Reset(); int actual=espeak_rs_audio_events(&table, present_pcm ? pcm : NULL, 3, list);
            TEST_ASSERT(State(actual)==expected); ++cases;
        }
    }
    for(event_kind=0; event_kind<=8; ++event_kind) for(int sample=-100; sample<=20000; sample+=100)
    for(int rate=0; rate<3; ++rate) for(int q=0; q<3; ++q) for(available=0; available<=1; ++available) {
        initial_rate=rates[rate]; queued_ms=q*1000-1; initial_mode=2; initial_output=0; initial_count=1; mutation=0;
        Reset(); list[0].sample=sample; unsigned old=ReferenceDeclare(list); uint64_t expected=State(old);
        Reset(); list[0].sample=sample; unsigned actual=espeak_rs_audio_declare(&table,list);
        TEST_ASSERT(State(actual)==expected); ++declarations;
    }
    /* Native guards exclude undefined C extents and arithmetic from parity. */
    Reset(); TEST_ASSERT(espeak_rs_audio_dispatch(&table,pcm,-1,list)==-1 && values[3]==EINVAL && !writes);
    Reset(); owner.count=9; TEST_ASSERT(espeak_rs_audio_events(&table,pcm,3,list)==-1 && values[3]==EINVAL && !opens && !writes);
    Reset(); owner.count=1; TEST_ASSERT(espeak_rs_audio_events(&table,pcm,3,NULL)==-1 && !writes);
    Reset(); owner.count=-1; TEST_ASSERT(espeak_rs_audio_events(&table,pcm,3,list)==-1 && !writes);
    Reset(); TEST_ASSERT(espeak_rs_audio_declare(&table,NULL)==EINVAL);
    TEST_ASSERT(espeak_rs_audio_events(NULL,pcm,3,list)==-1);
    RustEngineAudio invalid=table; invalid.declare=NULL;
    TEST_ASSERT(espeak_rs_audio_dispatch(&invalid,pcm,3,list)==-1);
    Reset(); samples=LONG_MAX; offset=INT_MAX; values[2]=1; queued_ms=INT_MAX; list[0].sample=INT_MIN; list[0].type=espeakEVENT_WORD; available=1;
    TEST_ASSERT(espeak_rs_audio_declare(&table,list)==(unsigned)event_result && admissions==1);
    printf("%u retained-C audio dispatch comparisons; %u event-delay comparisons passed\n",cases,declarations);
    return 0;
}
