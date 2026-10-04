/* Differential rule execution against unchanged C routines.
 * SPDX-License-Identifier: GPL-3.0-or-later */
#include "config.h"
#include "test_assert.h"
#include <assert.h>
#include <stdbool.h>
#include <stdint.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <wchar.h>
#include <wctype.h>
#include <unistd.h>
#include <espeak-ng/speak_lib.h>
#include "common.h"
#include "compiledict.h"
#include "dictionary.h"
#include "phoneme.h"
#include "speech.h"
#include "translate.h"
#include "rust_data.h"
static void DollarRule(char **, char *, int, int, char *, Translator *, int, int *, int *);
static int LookupFlags(Translator *tr, const char *word, unsigned int flags[2])
{
	char phonemes[160], *input = (char *)word;
	flags[0] = flags[1] = 0;
	LookupDictList(tr,&input,phonemes,flags,0,NULL,0);
	return flags[0];
}
#include "matcher_reference.inc"
static uint32_t seed = 0x318ce735;
static uint32_t random32(void) { seed ^= seed << 13; seed ^= seed >> 17; seed ^= seed << 5; return seed; }
static unsigned long comparisons;
static FILE *reference_trace, *native_trace;
static void compare(Translator *tr, char *text, int offset, int group_length, char *rules, int word_flags, int dict_flags)
{
	char *expected_word = text+offset, *actual_word = text+offset;
	MatchRecord expected = {.points = 19, .phonemes = "untouched", .end_type = 13, .del_fwd = text};
	MatchRecord actual = expected;
	Translator snapshot = *tr;
	FILE *saved_trace = f_trans;
	if (option_phonemes & espeakPHONEMES_TRACE) {
		fflush(reference_trace); fflush(native_trace);
		TEST_ASSERT(ftruncate(fileno(reference_trace),0) == 0 && ftruncate(fileno(native_trace),0) == 0);
		rewind(reference_trace); rewind(native_trace);
		f_trans = reference_trace;
	}
	MatchRule(tr,&expected_word,text,group_length,rules,&expected,word_flags,dict_flags);
	*tr = snapshot;
	if (option_phonemes & espeakPHONEMES_TRACE) f_trans = native_trace;
	espeak_rs_match_rule(tr,&actual_word,text,group_length,rules,&actual,word_flags,dict_flags,strlen(text)+2);
	if (expected_word != actual_word || expected.points != actual.points || expected.end_type != actual.end_type || expected.del_fwd != actual.del_fwd || strcmp(expected.phonemes,actual.phonemes)) {
		fprintf(stderr,"lang=%x text=%s offset=%d group=%d flags=%x dict=%x rule=%td\n",tr->translator_name,text,offset,group_length,word_flags,dict_flags,rules ? rules-tr->data_dictlist : -1);
		fprintf(stderr,"points %d/%d, advance %td/%td ending %x/%x delete %td/%td phon=%s/%s\n",expected.points,actual.points,expected_word-text,actual_word-text,expected.end_type,actual.end_type,expected.del_fwd ? expected.del_fwd-text : -1,actual.del_fwd ? actual.del_fwd-text : -1,expected.phonemes,actual.phonemes);
		if (rules) { for (int ix=0; ix<20; ix++) fprintf(stderr,"%02x ",(unsigned char)rules[ix]); fprintf(stderr,"\n"); }
		TEST_ASSERT(false);
	}
	if (option_phonemes & espeakPHONEMES_TRACE) {
		char expected_text[16384], actual_text[16384];
		rewind(reference_trace); rewind(native_trace);
		size_t expected_length = fread(expected_text,1,sizeof(expected_text),reference_trace);
		size_t actual_length = fread(actual_text,1,sizeof(actual_text),native_trace);
		TEST_ASSERT(expected_length < sizeof(expected_text) && actual_length < sizeof(actual_text));
		if (expected_length != actual_length || memcmp(expected_text,actual_text,expected_length)) {
			fprintf(stderr,"trace mismatch text=%s group=%d rule=%td\n",text,group_length,rules-tr->data_dictlist);
			TEST_ASSERT(false);
		}
		f_trans = saved_trace;
	}
	comparisons++;
}
static void real_rules(void)
{
	const char *languages[] = {"en","el","de","ru","hu","tr","ar","fa","si","ko","cmn"};
	const char *words[] = {"hello","world","read","cat","testing","station","3:45","p.m.","can't","abc123","xyzyxx","eagles","stressed","suffixes","polish","skies","precondition","καλημέρα","ψυχοφθόρα","Россия","говорю","представляет","සිංහල","مرحبا","فارسی","한국어","中文","a-b","öäü","...","😀"};
	for (unsigned int language=0; language < sizeof(languages)/sizeof(*languages); language++) {
		TEST_ASSERT(espeak_SetVoiceByName(languages[language]) == EE_OK);
		Translator *tr = translator;
		for (unsigned int ix=0; ix < sizeof(words)/sizeof(*words); ix++) {
			char buffer[256] = {0,' '};
			snprintf(buffer+2,sizeof(buffer)-2,"%s ",words[ix]);
			char *text = buffer+2;
			tr->rule_text_base = buffer;
			tr->rule_text_length = strlen(text)+3;
			for (int trial=0; trial<12; trial++) {
				option_phonemes = trial == 0 ? espeakPHONEMES_TRACE : 0;
				tr->dict_condition = random32(); tr->expect_verb = random32() & 1;
				tr->word_vowel_count = random32() % 5; tr->word_stressed_count = random32() % 3;
				int flags = trial == 0 ? 0 : random32();
				int dict_flags = random32();
				for (int p=0; text[p] && text[p]!=' '; p++) {
					unsigned char c = text[p];
					/* A rule group starts at a character boundary. Artificial
					 * continuation-byte starts can make legacy PRE scans run
					 * before the allocation; exercise those only in Rust. */
					if ((c & 0xc0) == 0x80) continue;
					compare(tr,text,p,1,tr->groups1[c],flags,dict_flags);
					compare(tr,text,p,0,tr->groups1[0],flags,dict_flags);
					if ((c & 0xc0) != 0x80 && tr->letter_bits_offset > 0) {
						int code, width = utf8_in(&code,text+p);
						int group = code-tr->letter_bits_offset;
						if (group >= 0 && group < 128) compare(tr,text,p,width,tr->groups3[group],flags,dict_flags);
					}
					for (int pair=0; pair < tr->n_groups2; pair++)
						if (tr->groups2_name[pair] == (c | ((unsigned char)text[p+1]<<8)))
							compare(tr,text,p,2,tr->groups2[pair],flags,dict_flags);
				}
			}
			tr->rule_text_base = NULL;
			tr->rule_text_length = 0;
		}
		option_phonemes = 0;
	}
}
static void synthetic_rules(void)
{
	TEST_ASSERT(espeak_SetVoiceByName("en") == EE_OK);
	Translator *tr = translator;
	char *rules = tr->groups1['a'];
	TEST_ASSERT(rules != NULL && tr->data_dict_size-(rules-tr->data_dictlist) >= 64);
	char saved[64]; memcpy(saved,rules,sizeof(saved));
	const unsigned char opcodes[] = {10,11,12,13,14,15,16,17,18,19,21,23,24,25,26,28,29,32,60,'a','.','-'};
	const char *words[] = {"aeeabc","church","hello","aeiou","bcd","1.23","xyzyx","cat","-a-","öä","१a२"};
	for (unsigned int op=0; op<sizeof(opcodes); op++) for (int mode=0; mode<3; mode++) for (int trial=0; trial<20; trial++) {
		unsigned char stream[64]; memset(stream,7,sizeof(stream));
		int length=0;
		stream[length++] = 5; stream[length++] = 1+(trial%63);
		if (mode != 0) stream[length++] = mode;
		stream[length++] = opcodes[op];
		if (opcodes[op] == 17 || opcodes[op] == 18) stream[length++] = 'A'+(trial%8);
		if (opcodes[op] == 28) { unsigned char commands[] = {1,2,3,0x11,0x21,0x31}; stream[length++] = commands[trial%6]; }
		if (opcodes[op] == 14) { stream[length++] = 0x81; stream[length++] = 0x82; stream[length++] = 0x83; }
		if (opcodes[op] == 23) stream[length++] = 'a';
		stream[length++] = 3; stream[length++] = 42; stream[length++] = 0;
		stream[length++] = 3; stream[length++] = 43; stream[length++] = 0;
		memcpy(rules,stream,sizeof(stream));
		tr->dict_condition = random32(); tr->expect_verb = random32() & 1;
		tr->word_vowel_count = random32()%5; tr->word_stressed_count = random32()%3;
		tr->langopts.tone_numbers = random32() & 1;
		option_phonemes = trial == 0 ? espeakPHONEMES_TRACE : 0;
		for (unsigned int word=0; word<sizeof(words)/sizeof(*words); word++) {
			char buffer[256] = {0,' '}; snprintf(buffer+2,sizeof(buffer)-2,"%s ",words[word]);
			for (int p=0; buffer[2+p] && buffer[2+p]!=' '; p++) {
				if ((buffer[2+p] & 0xc0) == 0x80) continue;
				compare(tr,buffer+2,p,1,rules,random32(),random32());
			}
		}
	}
	memcpy(rules,saved,sizeof(saved));
	option_phonemes = 0;
}
int main(void)
{
	TEST_ASSERT(espeak_Initialize(AUDIO_OUTPUT_RETRIEVAL,0,NULL,0) == 22050);
	reference_trace = tmpfile(); native_trace = tmpfile();
	TEST_ASSERT(reference_trace != NULL && native_trace != NULL);
	real_rules();
	printf("Compared %lu real rule executions across 11 languages\n",comparisons);
	comparisons = 0; synthetic_rules();
	printf("Compared %lu synthetic executions covering every contextual opcode\n",comparisons);
	fclose(reference_trace); fclose(native_trace);
	TEST_ASSERT(espeak_Terminate() == EE_OK);
	return 0;
}
