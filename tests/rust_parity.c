/* SPDX-License-Identifier: GPL-3.0-or-later
 * Differential tests against the retained C implementation, not a second
 * rendering of the Rust algorithm. Exercises all Unicode codepoints.
 */
#include "config.h"
#include "test_assert.h"
#include <stdint.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <math.h>
#include <espeak-ng/espeak_ng.h>
#include <espeak-ng/encoding.h>
#include <ucd/ucd.h>
#include "ieee80.h"
#include "mnemonics.h"
#include "phoneme.h"

#define CLASSIFIERS(X) \
 X(ucd_isalnum) X(ucd_isalpha) X(ucd_isblank) X(ucd_iscntrl) \
 X(ucd_isdigit) X(ucd_isgraph) X(ucd_islower) X(ucd_isprint) \
 X(ucd_ispunct) X(ucd_isspace) X(ucd_isupper) X(ucd_isxdigit)
#define DECLARE_CLASSIFIER(name) extern int reference_##name(codepoint_t);
CLASSIFIERS(DECLARE_CLASSIFIER)
extern codepoint_t reference_ucd_toupper(codepoint_t);
extern codepoint_t reference_ucd_tolower(codepoint_t);
extern codepoint_t reference_ucd_totitle(codepoint_t);
extern ucd_category reference_ucd_lookup_category(codepoint_t);
extern ucd_category_group reference_ucd_lookup_category_group(codepoint_t);
extern ucd_script reference_ucd_lookup_script(codepoint_t);
extern ucd_property reference_ucd_properties(codepoint_t, ucd_category);
extern const char *reference_ucd_get_category_string(ucd_category);
extern const char *reference_ucd_get_category_group_string(ucd_category_group);
extern const char *reference_ucd_get_script_string(ucd_script);
extern espeak_ng_TEXT_DECODER *reference_create_text_decoder(void);
extern void reference_destroy_text_decoder(espeak_ng_TEXT_DECODER *);
extern espeak_ng_STATUS reference_text_decoder_decode_string(espeak_ng_TEXT_DECODER *, const char *, int, espeak_ng_ENCODING);
extern espeak_ng_STATUS reference_text_decoder_decode_string_auto(espeak_ng_TEXT_DECODER *, const char *, int, espeak_ng_ENCODING);
extern int reference_text_decoder_eof(espeak_ng_TEXT_DECODER *);
extern uint32_t reference_text_decoder_getc(espeak_ng_TEXT_DECODER *);
extern uint32_t reference_text_decoder_peekc(espeak_ng_TEXT_DECODER *);
extern const void *reference_text_decoder_get_buffer(espeak_ng_TEXT_DECODER *);
extern double reference_ieee_extended_to_double(const unsigned char *);
extern int reference_LookupMnem(const MNEM_TAB *, const char *);
extern const char *reference_LookupMnemName(const MNEM_TAB *, int);
extern phoneme_feature_t reference_phoneme_feature_from_string(const char *);
extern espeak_ng_STATUS reference_phoneme_add_feature(PHONEME_TAB *, phoneme_feature_t);

static uint32_t random_state = 0x12345678;
static uint32_t next_random(void)
{
 random_state = random_state * 1664525u + 1013904223u;
 return random_state;
}

static void unicode_point(uint32_t c)
{
 TEST_ASSERT(ucd_lookup_category(c) == reference_ucd_lookup_category(c));
 TEST_ASSERT(ucd_lookup_category_group(c) == reference_ucd_lookup_category_group(c));
 TEST_ASSERT(ucd_lookup_script(c) == reference_ucd_lookup_script(c));
 TEST_ASSERT(ucd_toupper(c) == reference_ucd_toupper(c));
 TEST_ASSERT(ucd_tolower(c) == reference_ucd_tolower(c));
 TEST_ASSERT(ucd_totitle(c) == reference_ucd_totitle(c));
#define CHECK_CLASSIFIER(name) TEST_ASSERT(name(c) == reference_##name(c));
 CLASSIFIERS(CHECK_CLASSIFIER)
 for (int cat = 0; cat <= UCD_CATEGORY_Zs; ++cat) {
  if (ucd_properties(c, (ucd_category)cat) != reference_ucd_properties(c, (ucd_category)cat))
   fprintf(stderr, "property mismatch: codepoint=%08x category=%d\n", c, cat);
  TEST_ASSERT(ucd_properties(c, (ucd_category)cat) == reference_ucd_properties(c, (ucd_category)cat));
 }
}

static void unicode_parity(void)
{
 for (uint32_t c = 0; c <= 0x10ffff; ++c) unicode_point(c);
 for (int i = 0; i < 1000; ++i) unicode_point(next_random());
 unicode_point(UINT32_MAX);
 for (int i = -1; i <= 32; ++i) {
  TEST_ASSERT(strcmp(ucd_get_category_string((ucd_category)i), reference_ucd_get_category_string((ucd_category)i)) == 0);
  TEST_ASSERT(strcmp(ucd_get_category_group_string((ucd_category_group)i), reference_ucd_get_category_group_string((ucd_category_group)i)) == 0);
 }
 for (int i = -1; i <= UCD_SCRIPT_Zzzz + 1; ++i)
  TEST_ASSERT(strcmp(ucd_get_script_string((ucd_script)i), reference_ucd_get_script_string((ucd_script)i)) == 0);
 puts("Unicode parity: all 1,114,112 codepoints, all 31 property categories, 1,001 invalid/random points");
}

static void decoder_parity(void)
{
 espeak_ng_TEXT_DECODER *c = reference_create_text_decoder();
 espeak_ng_TEXT_DECODER *r = create_text_decoder();
 TEST_ASSERT(c && r);
 for (int encoding = 1; encoding <= ESPEAKNG_ENCODING_ISO_10646_UCS_2; ++encoding) {
  for (int sample = 0; sample < 1000; ++sample) {
   unsigned char bytes[64];
   for (unsigned int j = 0; j < sizeof(bytes); ++j) bytes[j] = (unsigned char)(next_random() >> 24);
   int length = sample % 48;
   for (int auto_mode = 0; auto_mode <= 1; ++auto_mode) {
    /* C AUTO is only defined for a codepage fallback; other modes dereference NULL. */
    if (auto_mode && (encoding == 1 || encoding >= ESPEAKNG_ENCODING_UTF_8)) continue;
    if (auto_mode) {
     TEST_ASSERT(reference_text_decoder_decode_string_auto(c, (const char *)bytes, length, (espeak_ng_ENCODING)encoding) == ENS_OK);
     TEST_ASSERT(text_decoder_decode_string_auto(r, (const char *)bytes, length, (espeak_ng_ENCODING)encoding) == ENS_OK);
    } else {
     TEST_ASSERT(reference_text_decoder_decode_string(c, (const char *)bytes, length, (espeak_ng_ENCODING)encoding) == ENS_OK);
     TEST_ASSERT(text_decoder_decode_string(r, (const char *)bytes, length, (espeak_ng_ENCODING)encoding) == ENS_OK);
    }
    int steps = 0;
    while (!reference_text_decoder_eof(c)) {
     TEST_ASSERT(!text_decoder_eof(r));
     TEST_ASSERT(reference_text_decoder_get_buffer(c) == text_decoder_get_buffer(r));
     TEST_ASSERT(reference_text_decoder_peekc(c) == text_decoder_peekc(r));
     TEST_ASSERT(reference_text_decoder_getc(c) == text_decoder_getc(r));
     TEST_ASSERT(++steps <= length);
    }
    TEST_ASSERT(text_decoder_eof(r));
    TEST_ASSERT(text_decoder_getc(r) == 0); /* bounded improvement over C past EOF */
   }
  }
 }
 TEST_ASSERT(text_decoder_decode_string(r, "x", 1, (espeak_ng_ENCODING)-1) == ENS_UNKNOWN_TEXT_ENCODING);
 TEST_ASSERT(text_decoder_decode_string(r, "x", 1, (espeak_ng_ENCODING)UINT32_MAX) == ENS_UNKNOWN_TEXT_ENCODING);
 destroy_text_decoder(r);
 reference_destroy_text_decoder(c);
 puts("Decoder parity: 20 encodings x 1,000 random buffers; supported AUTO fallbacks and cursor/peek behavior");
}

static void ieee80_parity(void)
{
 for (unsigned int exp = 0; exp <= 0xffff; ++exp) {
  for (int sample = 0; sample < 3; ++sample) {
   unsigned char bytes[10];
   bytes[0] = (unsigned char)(exp >> 8);
   bytes[1] = (unsigned char)exp;
   for (int j = 2; j < 10; ++j) bytes[j] = sample == 0 ? 0 : (unsigned char)(next_random() >> 24);
   double c = reference_ieee_extended_to_double(bytes);
   double r = ieee_extended_to_double(bytes);
   TEST_ASSERT((isnan(c) && isnan(r)) || memcmp(&c, &r, sizeof(double)) == 0);
  }
 }
 puts("IEEE80 parity: 196,608 values across every exponent and sign, bit-exact except NaN payloads");
}

int main(void)
{
 TEST_ASSERT(sizeof(PHONEME_TAB) == 16);
 for (char a = 'a'; a <= 'z'; ++a) for (char b = 'a'; b <= 'z'; ++b) for (char d = 'a'; d <= 'z'; ++d) {
  char name[4] = {a, b, d, 0};
  phoneme_feature_t feature = phoneme_feature_from_string(name);
  TEST_ASSERT(feature == reference_phoneme_feature_from_string(name));
  for (int j = 0; j < 10; ++j) {
   PHONEME_TAB c = {0};
   c.phflags = next_random();
   c.type = (unsigned char)next_random();
   PHONEME_TAB r = c;
   TEST_ASSERT(phoneme_add_feature(&r, feature) == reference_phoneme_add_feature(&c, feature));
   TEST_ASSERT(memcmp(&c, &r, sizeof(c)) == 0);
  }
 }
 TEST_ASSERT(phoneme_add_feature(NULL, vwl) == reference_phoneme_add_feature(NULL, vwl));
 puts("Phoneme-feature parity: all 17,576 lowercase triples, 175,760 randomized records");
 const MNEM_TAB table[] = {{"a", 7}, {"a", 8}, {"b", 7}, {NULL, -3}};
 const char *names[] = {NULL, "a", "b", "missing"};
 for (int i = 0; i < 4; ++i) TEST_ASSERT(LookupMnem(table, names[i]) == reference_LookupMnem(table, names[i]));
 for (int i = -4; i < 10; ++i) TEST_ASSERT(strcmp(LookupMnemName(table, i), reference_LookupMnemName(table, i)) == 0);
 unicode_parity();
 decoder_parity();
 ieee80_parity();
 return 0;
}
