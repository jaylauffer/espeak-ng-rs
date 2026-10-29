/* Contextual lookup and alphabet parity against unchanged C routines.
 * SPDX-License-Identifier: GPL-3.0-or-later */
#include "config.h"
#include "test_assert.h"
#include <assert.h>
#include <stdbool.h>
#include <stdint.h>
#include <limits.h>
#include <stdlib.h>
#include <string.h>
#include <espeak-ng/speak_lib.h>
#include "common.h"
#include "compiledict.h"
#include "dictionary.h"
#include "phoneme.h"
#include "speech.h"
#include "translate.h"
#include "rust_data.h"
#define TransposeAlphabet reference_TransposeAlphabet
#define LookupDict2 reference_LookupDict2
#include "lookup_reference.inc"
#undef TransposeAlphabet
#undef LookupDict2
#define espeak_rs_lookup_dict previous_native_lookup
#define TransposeAlphabet previous_native_transpose
#include "lookup_host_reference.inc"
#undef TransposeAlphabet
#undef espeak_rs_lookup_dict
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
	FILE *previous_trace = actual_trace != NULL ? tmpfile() : NULL;
	if (actual_trace != NULL) { TEST_ASSERT(previous_trace != NULL); f_trans = previous_trace; }
	unsigned previous_flags[2] = {0x1234,initial};
	char previous_phonemes[160] = "untouched";
	dictionary_skipwords = 77;
	const char *previous = previous_native_lookup(tr, word, next, previous_phonemes, previous_flags, end_flags, words, word_count);
	if (previous != expected || memcmp(previous_flags,expected_flags,sizeof(previous_flags)) || strcmp(previous_phonemes,expected_phonemes) || dictionary_skipwords != expected_skip) {
		fprintf(stderr,"host lookup word %s: flags %08x/%08x vs %08x/%08x; end %p vs %p; skips %d/%d; phon %s/%s\n", word,expected_flags[0],expected_flags[1],previous_flags[0],previous_flags[1],(void *)expected,(void *)previous,expected_skip,dictionary_skipwords,expected_phonemes,previous_phonemes);
		TEST_ASSERT(false);
	}
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
		char previous_text[4096]; rewind(previous_trace);
		size_t previous_length = fread(previous_text,1,sizeof(previous_text),previous_trace);
		TEST_ASSERT(previous_length == actual_length && memcmp(previous_text,actual_text,previous_length) == 0);
		fclose(previous_trace);
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
			char expected[161], actual[161], previous[161];
			memset(expected,0xa5,sizeof(expected));
			int cursor = 0;
			for (int ix = 0, count = random32() % 32; ix < count; ix++) {
				unsigned int code = tr->transpose_min + random32() % (tr->transpose_max - tr->transpose_min + 1);
				if (trial % 7 == 0) code = 'x';
				cursor += utf8_out(code,expected+cursor);
			}
			expected[cursor] = 0;
			memcpy(actual,expected,sizeof(actual));
			memcpy(previous,expected,sizeof(previous));
			int wanted = reference_TransposeAlphabet(tr,expected);
			int old = previous_native_transpose(tr,previous);
			int got = TransposeAlphabet(tr,actual);
			TEST_ASSERT(wanted == got && old == got);
			TEST_ASSERT(memcmp(expected,actual,sizeof(actual)) == 0);
			TEST_ASSERT(memcmp(previous,actual,sizeof(actual)) == 0);
		}
		DeleteTranslator(tr);
	}
	printf("Compared 50000 alphabet transpositions including maps, pairs and hash tails\n");
}
static void NoTrace(const RustDictionaryTrace *event) { (void)event; }
static void bounded_trace(void)
{
    unsigned code=0;
    for(unsigned i=1;i<255;i++) if(i!=phonSWITCH && phoneme_tab[i]!=NULL && phoneme_tab[i]->type!=phSTRESS && (phoneme_tab[i]->mnemonic>>8)!=0) { code=i;break; }
    TEST_ASSERT(code!=0);
    /* 159 valid encoded bytes expand beyond the trace's 200-byte buffer.
     * Exercise only the bounded native adapter, never the unbounded C oracle. */
    unsigned char bucket[166]={165,3,'c','a','t'};memset(bucket+5,(int)code,159);
    Translator tr={0},*saved=translator;translator=&tr;
    for(unsigned i=0;i<1024;i++) tr.dict_hashtab[i]=(char *)bucket;
    tr.data_dictrules=(char *)bucket+sizeof(bucket);
    FILE *saved_trace=f_trans;f_trans=tmpfile();TEST_ASSERT(f_trans!=NULL);
    option_phonemes=espeakPHONEMES_TRACE;dictionary_skipwords=77;
    char next[]=" ",output[200]={0};unsigned flags[2]={0};
    TEST_ASSERT(espeak_rs_lookup_dict(&tr,"cat",next+1,output,flags,0,NULL,0)==next+1);
    TEST_ASSERT(strlen(output)==159 && dictionary_skipwords==77 && flags[0]&FLAG_FOUND);
    for(unsigned i=0;i<159;i++) TEST_ASSERT((unsigned char)output[i]==code);
    rewind(f_trans);char trace[200]={0};size_t length=fread(trace,1,sizeof(trace)-1,f_trans);
    TEST_ASSERT(length>0 && strcmp(trace,"Found: 'cat' []  \n")==0);
    fclose(f_trans);f_trans=saved_trace;translator=saved;option_phonemes=0;
    printf("159-byte pronunciation retains speech/flags; oversized diagnostic decode safely emits an empty mnemonic\n");
}
static void host_guards(void)
{
    Translator tr={0}, *saved=translator; translator=&tr;
    unsigned char bucket[]={8,3,'c','a','t',42,0,100,0};
    char next[256];memset(next,'z',sizeof(next));next[0]=' ';
    char last[]="cat"; WORD_TAB words[20]={{0}};
    for(unsigned trial=0;trial<10;trial++) {
        translator=trial==9?NULL:&tr;
        memset(tr.dict_hashtab,0,sizeof(tr.dict_hashtab));
        if(trial!=0) for(unsigned i=0;i<1024;i++) tr.dict_hashtab[i]=(char *)bucket;
        tr.data_dictrules=(char *)bucket+sizeof(bucket);
        if(trial==1) tr.data_dictrules=NULL;
        if(trial==2) tr.data_dictrules=(char *)bucket;
        if(trial==3) tr.data_dictrules=(char *)((uintptr_t)bucket-1);
        bucket[0]=trial==4?255:8; tr.dict_condition=trial==5?0:1;
        const char *tail=trial==6?last+sizeof(last):next+1;
        int count=trial==7?-1:trial==8?INT_MAX:0;
        words[0].flags=2;words[0].length=0;
        unsigned expected_flags[]={19,0x40000000},actual_flags[]={19,0x40000000};
        char expected[200],actual[200];memset(expected,0x97,200);strcpy(expected,"untouched");memcpy(actual,expected,200);
        option_phonemes=0;dictionary_skipwords=77;
        const char *wanted=previous_native_lookup(&tr,last,tail,expected,expected_flags,0,words,count);int skip=dictionary_skipwords;
        dictionary_skipwords=77;
        const char *got=espeak_rs_lookup_dict(&tr,last,tail,actual,actual_flags,0,words,count);
        TEST_ASSERT(wanted==got && memcmp(expected,actual,200)==0 && memcmp(expected_flags,actual_flags,sizeof(actual_flags))==0 && dictionary_skipwords==skip);
        if(trial==0) TEST_ASSERT(actual_flags[0]==0 && actual_flags[1]==0x40000000 && strcmp(actual,"untouched")==0);
        if(trial>=1 && trial<=4) TEST_ASSERT(actual_flags[0]==19 && strcmp(actual,"untouched")==0);
        if(trial==5) TEST_ASSERT(actual_flags[0]==19 && actual[0]==42 && actual[1]==0 && got==NULL);
    }
    translator=&tr;
    char phonetic[200]="untouched";unsigned flags[]={19,23};
    RustDictionaryGlobals globals={&tr,&dictionary_skipwords,0};RustDictionaryEffects effects={IsAlpha,NoTrace};
    dictionary_skipwords=77;
    TEST_ASSERT(espeak_rs_lookup_dict_host(NULL,last,next+1,phonetic,flags,0,NULL,0,&globals,&effects)==NULL);
    TEST_ASSERT(espeak_rs_lookup_dict_host(&tr,NULL,next+1,phonetic,flags,0,NULL,0,&globals,&effects)==NULL);
    TEST_ASSERT(espeak_rs_lookup_dict_host(&tr,last,NULL,phonetic,flags,0,NULL,0,&globals,&effects)==NULL);
    TEST_ASSERT(espeak_rs_lookup_dict_host(&tr,last,next+1,NULL,flags,0,NULL,0,&globals,&effects)==NULL);
    TEST_ASSERT(espeak_rs_lookup_dict_host(&tr,last,next+1,phonetic,NULL,0,NULL,0,&globals,&effects)==NULL);
    TEST_ASSERT(espeak_rs_lookup_dict_host(&tr,last,next+1,phonetic,flags,0,NULL,0,NULL,&effects)==NULL);
    TEST_ASSERT(espeak_rs_lookup_dict_host(&tr,last,next+1,phonetic,flags,0,NULL,0,&globals,NULL)==NULL);
    effects.alpha=NULL;
    TEST_ASSERT(espeak_rs_lookup_dict_host(&tr,last,next+1,phonetic,flags,0,NULL,0,&globals,&effects)==NULL);effects.alpha=IsAlpha;
    effects.trace=NULL;
    TEST_ASSERT(espeak_rs_lookup_dict_host(&tr,last,next+1,phonetic,flags,0,NULL,0,&globals,&effects)==NULL);effects.trace=NoTrace;
    globals.skipwords=NULL;
    TEST_ASSERT(espeak_rs_lookup_dict_host(&tr,last,next+1,phonetic,flags,0,NULL,0,&globals,&effects)==NULL);
    TEST_ASSERT(flags[0]==19 && flags[1]==23 && dictionary_skipwords==77 && strcmp(phonetic,"untouched")==0);
    int16_t pairs[128]={0};tr.transpose_min=0x430;tr.transpose_max=0x451;tr.frequent_pairs=pairs;
    char key[]="аб";char previous[sizeof(key)];memcpy(previous,key,sizeof(key));
    TEST_ASSERT(previous_native_transpose(&tr,previous)==(int)strlen(key));
    TEST_ASSERT(TransposeAlphabet(&tr,key)==(int)strlen(previous) && memcmp(key,previous,sizeof(key))==0);
    TEST_ASSERT(espeak_rs_transpose_host(NULL,key)==-1 && espeak_rs_transpose_host(&tr,NULL)==-1);
    translator=saved;
    printf("Previous native host matches 10 bounded/malformed cases; null/table/output and missing-pair-sentinel guards pass\n");
}
int main(void)
{
	size_t layout[6]; espeak_rs_lookup_word_layout(layout);
	const size_t expected_layout[] = {sizeof(WORD_TAB),offsetof(WORD_TAB,flags),offsetof(WORD_TAB,start),offsetof(WORD_TAB,pre_pause),offsetof(WORD_TAB,sourceix),offsetof(WORD_TAB,length)};
	TEST_ASSERT(memcmp(layout,expected_layout,sizeof(layout)) == 0);
	TEST_ASSERT(espeak_Initialize(AUDIO_OUTPUT_RETRIEVAL,0,NULL,0) == 22050);
	TEST_ASSERT(espeak_SetVoiceByName("en") == EE_OK);
	transpositions();
	real_lookups();
	synthetic_contexts();
	host_guards();
	bounded_trace();
	TEST_ASSERT(espeak_Terminate() == EE_OK);
	return 0;
}
