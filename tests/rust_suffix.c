/* Native spelling repairs and UTF8 output against independent retained C.
 * SPDX-License-Identifier: GPL-3.0-or-later */
#include "config.h"
#include "test_assert.h"
#include <limits.h>
#include <stdint.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <wchar.h>
#include "common.h"
#include "dictionary.h"
#include "speech.h"
#include "synthesize.h"
#include "translate.h"
#include "rust_data.h"
#define utf8_out ReferenceUTF8
#include "utf8_reference.inc"
static int ReferenceLetter(Translator *,int,int);
static int ReferenceVowel(Translator *tr,int code) { return ReferenceLetter(tr,code,LETTERGP_VOWEL2); }
#define IsLetter ReferenceLetter
#define IsVowel ReferenceVowel
#define RemoveEnding ReferenceEnding
#include "suffix_reference.inc"
#undef IsLetter
#undef IsVowel
#undef RemoveEnding
#undef utf8_out
static unsigned sequence=0x746f2u;
static unsigned next(void){sequence=sequence*1664525u+1013904223u;return sequence;}
static void compare(Translator *actual,const char *word,unsigned ending,int prefix)
{
	unsigned char data[2048],expected[2048];
	char original[160],reference_copy[160];
	memset(data,' ',sizeof(data));data[sizeof(data)-1]=0;
	size_t length=strlen(word);TEST_ASSERT(length<sizeof(data)-8);
	memcpy(data+3,word,length);
	if(prefix) { data[0]='x';data[1]='a';data[2]='i'; }
	memcpy(expected,data,sizeof(data));
	memset(original,0x5a,sizeof(original));memcpy(reference_copy,original,sizeof(original));
	Translator reference=*actual;
	actual->expect_verb=reference.expect_verb=(int)(next()%4)-1;
	actual->rule_text_base=(char*)data;actual->rule_text_length=sizeof(data);
	reference.rule_text_base=(char*)expected;reference.rule_text_length=sizeof(expected);
	FILE *saved_trace=f_trans,*reference_trace=NULL,*actual_trace=NULL;
	if(option_phonemes&espeakPHONEMES_TRACE) {
		reference_trace=tmpfile();actual_trace=tmpfile();TEST_ASSERT(reference_trace&&actual_trace);f_trans=reference_trace;
	}
	int wanted=ReferenceEnding(&reference,(char*)expected+3,ending,reference_copy);
	if(actual_trace) f_trans=actual_trace;
	int found=RemoveEnding(actual,(char*)data+3,ending,original);
	if(actual_trace) {
		char actual_text[100]={0},expected_text[100]={0};rewind(actual_trace);rewind(reference_trace);
		TEST_ASSERT(fread(actual_text,1,99,actual_trace)==fread(expected_text,1,99,reference_trace));
		TEST_ASSERT(memcmp(actual_text,expected_text,100)==0);
		fclose(actual_trace);fclose(reference_trace);f_trans=saved_trace;
	}
	if(found!=wanted||memcmp(data,expected,sizeof(data))!=0)
		fprintf(stderr,"Suffix mismatch language=%x word=%s end=%x prefix=%d\n",actual->translator_name,word,ending,prefix);
	TEST_ASSERT(found==wanted);
	TEST_ASSERT(memcmp(data,expected,sizeof(data))==0);
	TEST_ASSERT(memcmp(original,reference_copy,sizeof(original))==0);
	TEST_ASSERT(actual->expect_verb==reference.expect_verb);
	actual->rule_text_base=NULL;actual->rule_text_length=0;
}
int main(void)
{
	_Static_assert(sizeof(RustSuffixContext)==20,"suffix context layout");
	_Static_assert(sizeof(RustSuffixEffects)==16,"suffix effects layout");
	size_t encoded=0,cases=0;
	for(unsigned code=0;code<=0x110010;code++) {
		char actual[8],expected[8];memset(actual,0x5a,8);memset(expected,0x5a,8);
		TEST_ASSERT(utf8_out(code,actual)==ReferenceUTF8(code,expected));
		TEST_ASSERT(memcmp(actual,expected,8)==0);encoded++;
	}
	const char *words[]={"making","tries","cafEs","ioning","sponging","ranging","larging","cathes","c'","word's","paden","mannen","lopen","caf\xc3\xa9s","\xce\xb1\xce\xb2\xce\xb3","\xe4\xb8\x96\xe7\x95\x8c","a","","abc\xf0\x9f\x99\x82"};
	const char *languages[]={"en","nl","fr","de","bg","ru","hu","hr","el"};
	for(size_t language=0;language<sizeof(languages)/sizeof(*languages);language++) {
		Translator *tr=SelectTranslator(languages[language]);TEST_ASSERT(tr!=NULL);
		for(size_t word=0;word<sizeof(words)/sizeof(*words);word++) {
			int characters=0;for(const unsigned char *p=(const unsigned char*)words[word];*p;p++)if((*p&0xc0)!=0x80)characters++;
			for(unsigned count=0;count<=(unsigned)characters;count++) for(unsigned flags=0;flags<16;flags++) {
				unsigned ending=count | ((flags&1)?SUFX_E:0) | ((flags&2)?SUFX_I:0) | ((flags&4)?SUFX_V:0) | ((flags&8)?SUFX_D:0);
				compare(tr,words[word],ending,0);cases++;
				compare(tr,words[word],ending,1);cases++;
			}
		}
		for(int iteration=0;iteration<4000;iteration++) {
			char word[160];size_t length=0;int characters=(next()%30)+1;
			for(int character=0;character<characters;character++) {
				unsigned code=next()%5?('a'+next()%26):0x3b1+next()%20;
				length+=ReferenceUTF8(code,word+length);
			}
			word[length]=0;
			unsigned ending=next()%(characters+1);
			ending|=next()&0xfff0;ending&=~0x30u;
			if(iteration%4==0) tr->langopts.suffix_add_e=0x1f642;
			else if(iteration%4==1) tr->langopts.suffix_add_e=-1;
			else if(iteration%4==2) tr->langopts.suffix_add_e=0;
			else tr->langopts.suffix_add_e='e';
			compare(tr,word,ending,iteration&1);cases++;
		}
		char longest[160];memset(longest,'c',159);longest[159]=0;
		tr->langopts.suffix_add_e=0x1f642;
		for(unsigned count=0;count<64;count++) {
			compare(tr,longest,count|SUFX_E|SUFX_I|SUFX_V,0);cases++;
		}
		char chain[801];for(int index=0;index<800;index++)chain[index]=index&1?'\'':'s';chain[800]=0;
		for(unsigned count=1;count<64;count++) {compare(tr,chain,count|SUFX_E|SUFX_I|SUFX_V,0);cases++;}
		option_phonemes=espeakPHONEMES_TRACE;
		for(unsigned count=0;count<4;count++){compare(tr,"making",count|SUFX_E,0);cases++;}
		option_phonemes=0;
		DeleteTranslator(tr);
	}
	/* Guard malformed spans/counts without calling C's unbounded backscan. */
	Translator *tr=SelectTranslator("en");TEST_ASSERT(tr!=NULL);
	char malformed[16]="stem ",previous[16],copy[160],old_copy[160];
	memcpy(previous,malformed,16);memset(copy,0x5a,160);memcpy(old_copy,copy,160);
	TEST_ASSERT(RemoveEnding(tr,malformed,63,copy)==0);
	TEST_ASSERT(memcmp(malformed,previous,16)==0&&memcmp(copy,old_copy,160)==0);
	DeleteTranslator(tr);
	printf("Compared %zu suffix repairs and %zu UTF8 encodings with retained C\n",cases,encoded);
	return 0;
}
