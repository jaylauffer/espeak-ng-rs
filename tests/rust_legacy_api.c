/* Native legacy control and diagnostics versus the original C branches.
 * SPDX-License-Identifier: GPL-3.0-or-later */
#include "config.h"
#include "test_assert.h"
#include <errno.h>
#include <limits.h>
#include <stdint.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <espeak-ng/espeak_ng.h>
#include <espeak-ng/speak_lib.h>
#include <espeak-ng/encoding.h>
#include "common.h"
#include "compiledict.h"
#include "synthesize.h"
#include "translate.h"
#include "rust_legacy_api.h"

#undef USE_RUST_CORE
#define create_file_error_context ReferenceFile
#define create_version_mismatch_error_context ReferenceVersion
#define espeak_ng_ClearErrorContext ReferenceClear
#define espeak_ng_GetStatusCodeMessage ReferenceMessage
#define espeak_ng_PrintStatusCodeMessage ReferencePrint
#include "error_reference.inc"
#undef create_file_error_context
#undef create_version_mismatch_error_context
#undef espeak_ng_ClearErrorContext
#undef espeak_ng_GetStatusCodeMessage
#undef espeak_ng_PrintStatusCodeMessage

/* Both controllers receive this recorder. No real engine/global state is
 * initialized by the control oracle; public production APIs have own tests. */
static char trace[32];
static size_t trace_size;
static espeak_ng_STATUS result_code;
static int event_flags, mode_seen, length_seen, compile_flags;
static int context_token;
static const char *path_seen, *dictionary_seen;
static FILE *errors_seen, *log_seen;
static void Record(char code) { TEST_ASSERT(trace_size + 1 < sizeof(trace)); trace[trace_size++] = code; trace[trace_size] = 0; }
static void Path(const char *path) { Record('p'); path_seen = path; }
static espeak_ng_STATUS Initialize(espeak_ng_ERROR_CONTEXT *context)
{
    Record('i'); TEST_ASSERT(*context == NULL);
    if (result_code) *context = (espeak_ng_ERROR_CONTEXT)&context_token;
    return result_code;
}
static espeak_ng_STATUS Output(espeak_ng_OUTPUT_MODE mode, int length, const char *device)
{
    Record('o'); TEST_ASSERT(device == NULL); mode_seen = mode; length_seen = length;
    return ENS_AUDIO_ERROR; /* The original legacy API ignores this failure. */
}
static int Rate(void) { Record('r'); return 22050; }
static void Print(espeak_ng_STATUS status, FILE *stream, espeak_ng_ERROR_CONTEXT context)
{
    Record('d'); TEST_ASSERT(status == result_code);
    TEST_ASSERT(context == (espeak_ng_ERROR_CONTEXT)&context_token); errors_seen = stream;
}
static void Clear(espeak_ng_ERROR_CONTEXT *context)
{
    Record('c'); TEST_ASSERT(*context == (espeak_ng_ERROR_CONTEXT)&context_token); *context = NULL;
}
static espeak_ng_STATUS Compile(const char *path, const char *dictionary, FILE *log, int flags, espeak_ng_ERROR_CONTEXT *context)
{
    Record('b'); TEST_ASSERT(*context == NULL); path_seen = path; dictionary_seen = dictionary;
    log_seen = log; compile_flags = flags;
    if (result_code) *context = (espeak_ng_ERROR_CONTEXT)&context_token;
    return result_code;
}
static void Exit(int code) { TEST_ASSERT(code == 1); Record('x'); }
static const RustLegacyApi callbacks = { Path, Initialize, Output, Rate, Print, Clear, Compile, Exit };

#define espeak_ng_InitializePath Path
#define espeak_ng_Initialize Initialize
#define espeak_ng_InitializeOutput Output
#define espeak_ng_GetSampleRate Rate
#define espeak_ng_PrintStatusCodeMessage Print
#define espeak_ng_ClearErrorContext Clear
#define espeak_ng_CompileDictionary Compile
#define option_phoneme_events event_flags
#define exit Exit
#define status_to_espeak_error ReferenceLegacyStatus
#define espeak_Initialize ReferenceInitialize
#define espeak_Synth ReferenceSynth
#define espeak_Synth_Mark ReferenceSynthMark
#define espeak_Key ReferenceKey
#define espeak_Char ReferenceChar
#define espeak_SetParameter ReferenceParameter
#define espeak_SetPunctuationList ReferencePunctuation
#define espeak_SetVoiceByName ReferenceVoiceName
#define espeak_SetVoiceByFile ReferenceVoiceFile
#define espeak_SetVoiceByProperties ReferenceVoiceProperties
#define espeak_Cancel ReferenceCancel
#define espeak_Synchronize ReferenceSynchronize
#define espeak_Terminate ReferenceTerminate
#define espeak_CompileDictionary ReferenceCompile
#include "espeak_api_reference.inc"
#undef espeak_ng_InitializePath
#undef espeak_ng_Initialize
#undef espeak_ng_InitializeOutput
#undef espeak_ng_GetSampleRate
#undef espeak_ng_PrintStatusCodeMessage
#undef espeak_ng_ClearErrorContext
#undef espeak_ng_CompileDictionary
#undef option_phoneme_events
#undef exit

static void Reset(espeak_ng_STATUS status)
{
    trace_size = 0; trace[0] = 0; result_code = status;
    event_flags = -1; mode_seen = -1; length_seen = 0; compile_flags = 0;
    path_seen = dictionary_seen = NULL; errors_seen = log_seen = NULL;
}
static void Controllers(void)
{
    const char *path = "\xff/voice/path";
    const espeak_ng_STATUS codes[] = { ENS_OK, ENS_COMPILE_ERROR, ENOENT, ENS_SPEECH_STOPPED };
    unsigned pairs = 0;
    for (size_t s = 0; s < sizeof(codes)/sizeof(*codes); ++s)
        for (int output = -2; output <= 5; ++output)
            for (int bits = 0; bits < 8; ++bits) {
                int options = bits | espeakINITIALIZE_DONT_EXIT;
                char expected[32];
                Reset(codes[s]);
                int rate = ReferenceInitialize((espeak_AUDIO_OUTPUT)output, -123, path, options);
                strcpy(expected, trace);
                int flags = event_flags, mode = mode_seen, length = length_seen;
                TEST_ASSERT(path_seen == path);
                Reset(codes[s]);
                TEST_ASSERT(espeak_rs_legacy_initialize(&callbacks, stderr, &event_flags, output, -123, path, options) == rate);
                TEST_ASSERT(strcmp(trace, expected) == 0);
                TEST_ASSERT(event_flags == flags && mode_seen == mode && length_seen == length);
                TEST_ASSERT(path_seen == path && (!codes[s] || errors_seen == stderr));
                ++pairs;
            }
    /* Success with DONT_EXIT absent, then failure with a deliberately returning
     * test exit callback. The real callback is non-returning exit(1). */
    Reset(ENS_OK);
    TEST_ASSERT(espeak_rs_legacy_initialize(&callbacks, stderr, &event_flags, 0, 10, path, 3) == 22050);
    TEST_ASSERT(strcmp(trace, "pior") == 0 && event_flags == 3);
    Reset(ENS_COMPILE_ERROR);
    TEST_ASSERT(espeak_rs_legacy_initialize(&callbacks, stderr, &event_flags, 0, 10, path, 0) == -1);
    TEST_ASSERT(strcmp(trace, "pidcx") == 0 && event_flags == -1);
    for (size_t s = 0; s < sizeof(codes)/sizeof(*codes); ++s)
        for (int flags = -2; flags <= 3; ++flags) {
            char expected[32];
            Reset(codes[s]); ReferenceCompile(path, stdout, flags); strcpy(expected, trace);
            Reset(codes[s]); espeak_rs_legacy_compile(&callbacks, stderr, path, dictionary_name, stdout, flags);
            TEST_ASSERT(strcmp(trace, expected) == 0);
            TEST_ASSERT(path_seen == path && dictionary_seen == dictionary_name && log_seen == stdout && compile_flags == flags);
            TEST_ASSERT(!codes[s] || errors_seen == stderr);
            ++pairs;
        }
    printf("legacy control: %u retained-C initialization/compiler pairs\n", pairs);
}
static void ComparePrint(espeak_ng_STATUS status, espeak_ng_ERROR_CONTEXT native, espeak_ng_ERROR_CONTEXT reference)
{
    FILE *a = tmpfile(), *b = tmpfile(); TEST_ASSERT(a && b);
    espeak_rs_error_print(status, a, native, NULL); /* Rejected table does no I/O. */
    espeak_ng_PrintStatusCodeMessage(status, a, native);
    ReferencePrint(status, b, reference);
    rewind(a); rewind(b);
    int x, y;
    do { x = fgetc(a); y = fgetc(b); TEST_ASSERT(x == y); } while (x != EOF);
    TEST_ASSERT(!ferror(a) && !ferror(b)); fclose(a); fclose(b);
}
static void Contexts(void)
{
    espeak_ng_ERROR_CONTEXT a = NULL, b = NULL;
    char name[] = "path'\xff/voice";
    TEST_ASSERT(espeak_rs_error_file(NULL, ENOENT, (const char *)(uintptr_t)1) == ENOENT);
    TEST_ASSERT(ReferenceFile(NULL, ENOENT, (const char *)(uintptr_t)1) == ENOENT);
    for (int i = 0; i < 300; ++i) {
        void *old = a;
        espeak_ng_STATUS status;
        if (i & 1) {
            status = espeak_rs_error_file(&a, ENS_COMPILE_ERROR, name);
            TEST_ASSERT(ReferenceFile(&b, ENS_COMPILE_ERROR, name) == status);
        } else {
            status = espeak_rs_error_version(&a, name, INT_MIN + i, -1);
            TEST_ASSERT(ReferenceVersion(&b, name, INT_MIN + i, -1) == status);
        }
        TEST_ASSERT(!old || old == a);
        TEST_ASSERT(a->type == b->type && a->version == b->version && a->expected_version == b->expected_version);
        TEST_ASSERT(strcmp(a->name, b->name) == 0);
        name[0] = 'X'; /* Names are owned independently of input mutation. */
        ComparePrint(status, a, b);
        name[0] = 'p';
    }
    /* Native copies before replacing the old name; the C alias case is UB. */
    TEST_ASSERT(espeak_rs_error_file(&a, ENOENT, a->name) == ENOENT);
    TEST_ASSERT(strcmp(a->name, name) == 0 && a->type == ERROR_CONTEXT_FILE);
    TEST_ASSERT(espeak_rs_error_file(&a, ENOENT, NULL) == EINVAL);
    TEST_ASSERT(strcmp(a->name, name) == 0);
    espeak_rs_error_clear(&a); ReferenceClear(&b); TEST_ASSERT(!a && !b);
    espeak_rs_error_clear(&a); espeak_rs_error_clear(NULL);
    espeak_ng_ERROR_CONTEXT_ unknown = { (espeak_ng_CONTEXT_TYPE)9, name, 0, 0 };
    ComparePrint(ENS_COMPILE_ERROR, &unknown, &unknown);
    ComparePrint(ENS_COMPILE_ERROR, NULL, NULL);
    ComparePrint(ENOENT, NULL, NULL);
    puts("error contexts: 300 owned replacements and exact diagnostic bytes");
}
static void Messages(void)
{
    unsigned messages = 0;
    for (unsigned i = 0; i < 300; ++i) {
        espeak_ng_STATUS status = i < 256 ? (espeak_ng_STATUS)i : (espeak_ng_STATUS)(0x100000ff + ((i - 256) << 8));
        for (size_t size = 0; size <= 80; ++size) {
            unsigned char a[96], b[96]; memset(a, 0xa5, sizeof(a)); memset(b, 0xa5, sizeof(b));
            espeak_ng_GetStatusCodeMessage(status, (char *)a, size);
            ReferenceMessage(status, (char *)b, size);
            TEST_ASSERT(memcmp(a, b, sizeof(a)) == 0);
            ++messages;
        }
        TEST_ASSERT(espeak_rs_legacy_status(status) == ReferenceLegacyStatus(status));
    }
    uint32_t state = 7;
    for (unsigned i = 0; i < 100000; ++i) {
        state = state * 1664525u + 1013904223u;
        TEST_ASSERT(espeak_rs_legacy_status((espeak_ng_STATUS)state) == ReferenceLegacyStatus((espeak_ng_STATUS)state));
        /* Non-errno messages have portable, exact unsigned hex output. */
        espeak_ng_STATUS status = (espeak_ng_STATUS)(state | 0x10000000u);
        unsigned char a[96], b[96]; memset(a, 0xa5, sizeof(a)); memset(b, 0xa5, sizeof(b));
        espeak_ng_GetStatusCodeMessage(status, (char *)a, i % sizeof(a));
        ReferenceMessage(status, (char *)b, i % sizeof(b));
        TEST_ASSERT(memcmp(a, b, sizeof(a)) == 0);
        ++messages;
    }
    printf("status messages: %u retained-C byte comparisons including output tails\n", messages);
}
static int locked, reentered, fragments;
static void Lock(FILE *stream) { TEST_ASSERT(stream == stdout && !locked); locked = 1; }
static void Write(FILE *stream, const unsigned char *bytes, size_t size)
{
    TEST_ASSERT(stream == stdout && locked && bytes && size);
    ++fragments;
    if (!reentered) {
        reentered = 1;
        espeak_ng_ERROR_CONTEXT context = NULL;
        TEST_ASSERT(espeak_rs_error_file(&context, ENOENT, "callback") == ENOENT);
        espeak_rs_error_clear(&context);
    }
}
static void Unlock(FILE *stream) { TEST_ASSERT(stream == stdout && locked); locked = 0; }
static void Errno(espeak_ng_STATUS status, char *buffer, size_t size) { strerror_r(status, buffer, size); }
int main(void)
{
    Controllers(); Messages(); Contexts();
    RustStatusIo io = { Lock, Write, Unlock, Errno };
    espeak_rs_error_print(ENS_COMPILE_ERROR, stdout, NULL, &io);
    TEST_ASSERT(!locked && reentered && fragments == 3);
    puts("serialized diagnostic fragments permit independent context reentry");
    return 0;
}
