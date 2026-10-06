/* Contextual lookup and alphabet parity against unchanged C routines.
 * SPDX-License-Identifier: GPL-3.0-or-later */
#include "config.h"
#include "test_assert.h"
#include <assert.h>
#include <stdbool.h>
#include <stdint.h>
#include <stdlib.h>
#include <string.h>
#include <espeak-ng/speak_lib.h>
#include "common.h"
#include "compiledict.h"
#include "dictionary.h"
#include "phoneme.h"
#include "speech.h"
#include "translate.h"
#define TransposeAlphabet reference_TransposeAlphabet
#define LookupDict2 reference_LookupDict2
#include "lookup_reference.inc"
#undef TransposeAlphabet
#undef LookupDict2
static uint32_t random_state = 0x927da132;
static uint32_t random32(void) { random_state ^= random_state << 13; random_state ^= random_state >> 17; random_state ^= random_state << 5; return random_state; }

static void compare_lookup(Translator *tr, const char *word, char *next, int end_flags, WORD_TAB *words, int word_count, unsigned int initial)
{
	unsigned int expected_flags[2] = {0x1234,initial}, actual_flags[2] = {0x1234,initial};
	char expected_phonemes[160] = "untouched", actual_phonemes[160] = "untouched";
	FILE *saved_trace = f_trans, *expected_trace = NULL, *actual_trace = NULL;
	if (option_phonemes & espeakPHONEMES_TRACE) {
		expected_trace = tmpfile(); actual_trace = tmpfile();
		TEST_ASSERT(expected_trace != NULL && actual_trace != NULL);
		f_trans = expected_trace;
	}
	dictionary_skipwords = 77;
	const char *expected = reference_LookupDict2(tr, word, next, expected_phonemes, expected_flags, end_flags, words, word_count);
	int expected_skip = dictionary_skipwords;
	if (actual_trace != NULL) f_trans = actual_trace;
	dictionary_skipwords = 77;
	const char *actual = espeak_rs_lookup_dict(tr, word, next, actual_phonemes, actual_flags, end_flags, words, word_count);
	if (expected != actual || memcmp(expected_flags, actual_flags, sizeof(expected_flags)) || strcmp(expected_phonemes,actual_phonemes) || expected_skip != dictionary_skipwords) {
		fprintf(stderr,"lookup word %s: flags %08x/%08x vs %08x/%08x; end %p vs %p; skips %d/%d; phon %s/%s\n", word,expected_flags[0],expected_flags[1],actual_flags[0],actual_flags[1],(void *)expected,(void *)actual,expected_skip,dictionary_skipwords,expected_phonemes,actual_phonemes);
		TEST_ASSERT(false);
	}
	if (expected_trace != NULL) {
		char expected_text[4096], actual_text[4096];
		rewind(expected_trace); rewind(actual_trace);
		size_t expected_length = fread(expected_text,1,sizeof(expected_text),expected_trace);
		size_t actual_length = fread(actual_text,1,sizeof(actual_text),actual_trace);
		TEST_ASSERT(expected_length < sizeof(expected_text) && actual_length < sizeof(actual_text));
		if (expected_length != actual_length || memcmp(expected_text,actual_text,expected_length) != 0)
			fprintf(stderr,"lookup word %s: trace differs\nC:    %.*s\nRust: %.*s\n",word,(int)expected_length,expected_text,(int)actual_length,actual_text);
		TEST_ASSERT(expected_length == actual_length && memcmp(expected_text,actual_text,expected_length) == 0);
		fclose(expected_trace); fclose(actual_trace);
		f_trans = saved_trace;
	}
}
static void synthetic_contexts(void)
{
	Translator *tr = translator;
	tr->transpose_min = 0;
	unsigned char bucket[512];
	char next[] = " dog ";
	WORD_TAB words[20] = {{0}};
	for (int trial = 0; trial < 20000; trial++) {
		unsigned char flag = trial < 164 ? trial : random32() % 164;
		int length = 0;
		bucket[length++] = 0; bucket[length++] = 3;
		memcpy(bucket + length, "cat",3); length += 3;
		bucket[length++] = 42; bucket[length++] = 0;
		bucket[length++] = flag;
		if (flag > 80 && flag < 100) { memcpy(bucket + length,"dog ",4); length += 4; }
		bucket[0] = length;
		bucket[length++] = 7; bucket[length++] = 3;
		memcpy(bucket + length,"cat",3); length += 3;
		bucket[length++] = 43; bucket[length++] = 0;
		bucket[length++] = 0;
		for (int ix = 0; ix < 1024; ix++) tr->dict_hashtab[ix] = (char *)bucket;
		tr->data_dictrules = (char *)bucket + length;
		tr->dict_condition = random32();
		tr->expect_verb = random32() & 1; tr->expect_verb_s = random32() & 1;
		tr->expect_past = random32() & 1; tr->expect_noun = random32() & 1;
		tr->prev_dict_flags[0] = random32();
		tr->translator_name = trial % 3 == 0 ? L('h','u') : L('e','n');
		tr->clause_end = &next[random32() % sizeof(next)];
		tr->clause_terminator = random32();
		for (int ix = 0; ix < 20; ix++) { words[ix].flags = random32(); words[ix].length = random32() & 1; }
		option_phonemes = trial < 164 ? espeakPHONEMES_TRACE : 0;
		compare_lookup(tr,"cat",next+1,random32() & 0x7fff,words,random32() % 21,random32() & FLAG_LOOKUP_SYMBOL);
	}
	/* LookupDictList can pass a pointer immediately after the terminal NUL. */
	char last_word[] = "cat";
	unsigned char last_bucket[] = {7,3,'c','a','t',42,0,0};
	for (int ix = 0; ix < 1024; ix++) tr->dict_hashtab[ix] = (char *)last_bucket;
	tr->data_dictrules = (char *)last_bucket + sizeof(last_bucket);
	tr->clause_end = last_word + sizeof(last_word);
	option_phonemes = espeakPHONEMES_TRACE;
	compare_lookup(tr,last_word,last_word+sizeof(last_word),0,NULL,0,0);
	option_phonemes = 0;
	/* The owned C translator must not retain synthetic pointers after return. */
	memset(tr->dict_hashtab,0,sizeof(tr->dict_hashtab));
	tr->data_dictrules = NULL;
	printf("Compared 20000 contextual lookups, all flag bytes and randomized grammar\n");
}
static void real_lookups(void)
{
	const char *languages[] = {"en","hu","ru","fa","ar","af","de","it"};
	const char *keys[] = {"and","for","you","phoenix","MIT","APH","ITX","mit","read","digest","alternate","such","it","IT","has","Polish","polish","ö","представляет","говорю","Россия","она","...","😀","_a","_"};
	char next[] = " as dog ";
	WORD_TAB words[20] = {{0}};
	for (unsigned int language = 0; language < sizeof(languages)/sizeof(*languages); language++) {
		TEST_ASSERT(espeak_SetVoiceByName(languages[language]) == EE_OK);
		Translator *tr = translator;
		for (int trial = 0; trial < 100; trial++) {
			option_phonemes = trial == 0 ? espeakPHONEMES_TRACE : 0;
			tr->dict_condition = random32(); tr->expect_verb = random32() & 1;
			tr->expect_verb_s = random32() & 1; tr->expect_past = random32() & 1;
			tr->expect_noun = random32() & 1; tr->prev_dict_flags[0] = random32();
			tr->clause_terminator = random32(); tr->clause_end = next + random32() % sizeof(next);
			for (int ix = 0; ix < 20; ix++) { words[ix].flags = random32(); words[ix].length = random32() & 1; }
			for (unsigned int key = 0; key < sizeof(keys)/sizeof(*keys); key++)
				compare_lookup(tr, keys[key], next+1, random32() & 0x7fff, words, 20, random32() & FLAG_LOOKUP_SYMBOL);
		}
	}
	printf("Compared 20800 real-dictionary lookups including compressed keys and symbols\n");
}
static void transpositions(void)
{
	const char *languages[] = {"en","ru","ar","fa","uk"};
	for (unsigned int language = 0; language < sizeof(languages)/sizeof(*languages); language++) {
		Translator *tr = SelectTranslator(languages[language]);
		TEST_ASSERT(tr != NULL);
		for (int trial = 0; trial < 10000; trial++) {
			char expected[161], actual[161];
			memset(expected,0xa5,sizeof(expected));
			int cursor = 0;
			for (int ix = 0, count = random32() % 32; ix < count; ix++) {
				unsigned int code = tr->transpose_min + random32() % (tr->transpose_max - tr->transpose_min + 1);
				if (trial % 7 == 0) code = 'x';
				cursor += utf8_out(code,expected+cursor);
			}
			expected[cursor] = 0;
			memcpy(actual,expected,sizeof(actual));
			int wanted = reference_TransposeAlphabet(tr,expected);
			int got = TransposeAlphabet(tr,actual);
			TEST_ASSERT(wanted == got);
			TEST_ASSERT(memcmp(expected,actual,sizeof(actual)) == 0);
		}
		DeleteTranslator(tr);
	}
	printf("Compared 50000 alphabet transpositions including maps, pairs and hash tails\n");
}
int main(void)
{
	TEST_ASSERT(espeak_Initialize(AUDIO_OUTPUT_RETRIEVAL,0,NULL,0) == 22050);
	TEST_ASSERT(espeak_SetVoiceByName("en") == EE_OK);
	transpositions();
	real_lookups();
	synthetic_contexts();
	TEST_ASSERT(espeak_Terminate() == EE_OK);
	return 0;
}
