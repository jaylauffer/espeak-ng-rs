/* Retained dot-ordinal decisions: source, callback order and live state parity.
 * SPDX-License-Identifier: GPL-3.0-or-later */
#include "config.h"
#include "test_assert.h"
#include "translate.h"
#include "common.h"
#include "rust_number_ordinal.h"
#include <stdint.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <wctype.h>

static Translator local;
static WORD_TAB words[2];
static unsigned char source[40], initial[40];
static unsigned next_result, initial_previous, translated_previous, initial_flags, translated_flags;
static unsigned calls, classes;
static uint64_t trace;
static size_t length;
static void Trace(unsigned value) { trace = (trace ^ value) * UINT64_C(1099511628211); }
static int Alpha(unsigned code) { ++classes; Trace(code); Trace(0); return IsAlpha(code); }
static int Digit(unsigned code) { ++classes; Trace(code); Trace(1); return iswdigit(code) != 0; }
static int Month(Translator *tr, char *word, char *output, WORD_TAB *wtab)
{
    TEST_ASSERT(tr == &local && output == NULL && wtab == NULL);
    ++calls; Trace((unsigned)((unsigned char *)word - source)); Trace(source[3]);
    local.prev_dict_flags[0] = translated_previous;
    words[0].flags = translated_flags;
    return (int)next_result;
}
#define CheckDotOrdinal ReferenceDot
#define IsAlpha Alpha
#undef iswdigit
#define iswdigit Digit
#define TranslateWord Month
#include "number_ordinal_reference.inc"
#undef CheckDotOrdinal
#undef IsAlpha
#undef iswdigit
#undef TranslateWord

static unsigned char Byte(void *context, ptrdiff_t offset)
{
    TEST_ASSERT(context == &local);
    return offset >= -2 && (offset < 0 || (size_t)offset < length) ? source[offset + 2] : 0;
}
static void Space(void *context, size_t offset)
{
    TEST_ASSERT(context == &local && offset < length); source[offset + 2] = ' ';
}
static unsigned Value(void *context, unsigned field)
{
    TEST_ASSERT(context == &local && field < 5);
    switch(field) {
    case 0: return local.langopts.numbers;
    case 1: return local.translator_name;
    case 2: return words[0].flags;
    case 3: return words[1].flags;
    default: return local.prev_dict_flags[0];
    }
}
static int Classify(void *context, unsigned code, unsigned kind)
{
    TEST_ASSERT(context == &local && kind < 2); return kind ? Digit(code) : Alpha(code);
}
static unsigned Translate(void *context, size_t offset)
{
    TEST_ASSERT(context == &local && offset < length);
    return (unsigned)Month(&local, (char *)source + 2 + offset, NULL, NULL);
}
static RustNumberOrdinal table = { &local, Byte, Space, Value, Classify, Translate };
static uint32_t seed = 0x57429018;
static unsigned Random(void) { seed ^= seed << 13; seed ^= seed >> 17; seed ^= seed << 5; return seed; }
static void Reset(void)
{
    memcpy(source, initial, sizeof(source));
    words[0].flags = initial_flags;
    local.prev_dict_flags[0] = initial_previous;
    calls = classes = 0; trace = UINT64_C(14695981039346656037);
}
static unsigned comparisons;
static void Compare(int roman, int remaining)
{
    Reset();
    int expected = ReferenceDot(&local, (char *)source + 2, (char *)source + 3, words, remaining, roman);
    unsigned char expected_source[40]; memcpy(expected_source, source, sizeof(source));
    unsigned expected_flags = words[0].flags, expected_previous = local.prev_dict_flags[0];
    unsigned expected_calls = calls, expected_classes = classes; uint64_t expected_trace = trace;
    Reset();
    unsigned saved_next = words[1].flags;
    if(remaining <= 1) words[1].flags = 0;
    int actual = espeak_rs_number_dot(&table, length, 1, roman);
    words[1].flags = saved_next;
    if(actual != expected || memcmp(source, expected_source, sizeof(source)) != 0
        || words[0].flags != expected_flags || local.prev_dict_flags[0] != expected_previous
        || calls != expected_calls || classes != expected_classes || trace != expected_trace) {
        fprintf(stderr, "dot mismatch expected=%d actual=%d roman=%d remaining=%d flags=%x next=%x previous=%x translated=%x result=%x calls=%u/%u classes=%u/%u\n",
            expected, actual, roman, remaining, initial_flags, saved_next, initial_previous, translated_previous, next_result, calls, expected_calls, classes, expected_classes);
        exit(1);
    }
    ++comparisons;
}
int main(void)
{
    static const char *following[] = { "month", "Month", "\xc3\xa9", "\xe6\x97\xa5", "1", "", "-", "\x80\x81month", "\xe2" };
    for(unsigned trial = 0; trial < 160000; ++trial) {
        memset(initial, 0, sizeof(initial)); initial[0] = Random() & 1 ? '-' : ' ';
        initial[1] = ' '; initial[2] = '2';
        initial[3] = Random() % 3 == 0 ? ' ' : '.';
        initial[4] = Random() % 9 == 0 ? 0 : ' ';
        strcpy((char *)initial + 5, following[trial % 9]);
        length = strlen((char *)initial + 2) + 1;
        initial_flags = (Random() & 1 ? FLAG_HAS_DOT : 0) | (Random() & 1 ? FLAG_COMMA_AFTER : 0);
        words[1].flags = (Random() & 1 ? FLAG_FIRST_UPPER : 0) | (Random() % 4 == 0 ? FLAG_NOSPACE : 0);
        local.langopts.numbers = Random() % 8 == 0 ? 0 : NUM_ORDINAL_DOT;
        local.translator_name = Random() & 1 ? L('h','u') : L('e','n');
        initial_previous = (Random() & 1 ? FLAG_ALT_TRANS : 0) | (Random() & 1 ? FLAG_ALT3_TRANS : 0);
        translated_previous = (Random() & 1 ? FLAG_ALT_TRANS : 0) | (Random() & 1 ? FLAG_ALT3_TRANS : 0);
        translated_flags = Random() & 1 ? FLAG_COMMA_AFTER : 0;
        next_result = (Random() & 1 ? FLAG_ALT_TRANS : 0) | (Random() & 1 ? FLAG_ALT3_TRANS : 0);
        Compare(trial & 1, trial % 3);
    }
    Reset(); unsigned char saved[40]; memcpy(saved, source, sizeof(source));
    TEST_ASSERT(espeak_rs_number_dot(NULL, length, 1, 0) == -1);
    TEST_ASSERT(espeak_rs_number_dot(&table, 0, 0, 0) == -1);
    TEST_ASSERT(espeak_rs_number_dot(&table, 801, 1, 0) == -1);
    TEST_ASSERT(espeak_rs_number_dot(&table, length, length, 0) == -1);
    RustNumberOrdinal invalid = table; invalid.translate = NULL;
    TEST_ASSERT(espeak_rs_number_dot(&invalid, length, 1, 0) == -1);
    invalid = table; invalid.context = NULL;
    TEST_ASSERT(espeak_rs_number_dot(&invalid, length, 1, 0) == -1);
    TEST_ASSERT(calls == 0 && classes == 0 && memcmp(saved, source, sizeof(source)) == 0);
    /* A terminator at the last initialized byte: later source is nonzero but
     * inaccessible. The adapter supplies virtual NUL for decoder lookahead. */
    memset(source, 0x80, sizeof(source)); source[2] = '2'; source[3] = '.'; source[4] = 0;
    length = 3; local.langopts.numbers = NUM_ORDINAL_DOT; words[1].flags = 0;
    TEST_ASSERT(espeak_rs_number_dot(&table, length, 1, 0) == 0 && source[3] == '.');
    printf("%u dot ordinal source/callback/live-state comparisons passed\n", comparisons);
    return 0;
}
