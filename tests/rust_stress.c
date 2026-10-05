/* Native stress planning against independent retained C.
 * SPDX-License-Identifier: GPL-3.0-or-later */
#include "config.h"
#include "test_assert.h"
#include <assert.h>
#include <stdbool.h>
#include <stdint.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <espeak-ng/speak_lib.h>
#include "common.h"
#include "dictionary.h"
#include "speech.h"
#include "synthesize.h"
#include "synthdata.h"
#include "translate.h"
#include "rust_data.h"
#define GetVowelStress ReferenceGetVowelStress
#define SetWordStress ReferenceSetWordStress
#include "stress_reference.inc"
#undef GetVowelStress
#undef SetWordStress
static unsigned seed=0xa2914c3u;
static unsigned next(void){seed^=seed<<13;seed^=seed>>17;seed^=seed<<5;return seed;}
static size_t extracted,assigned;
static void compare_extract(Translator *tr,const unsigned char *word,int request,int control)
{
	unsigned char actual[200],expected[200];signed char stress[100],wanted[100];
	memset(actual,0x5a,200);memcpy(actual,word,strlen((const char *)word)+1);memcpy(expected,actual,200);
	memset(stress,0x5a,100);memcpy(wanted,stress,100);
	int count=0,expected_count=0,primary=request,expected_primary=request;
	int maximum=ReferenceGetVowelStress(tr,expected,wanted,&expected_count,&expected_primary,control);
	int result=GetVowelStress(tr,actual,stress,&count,&primary,control);
	if(result!=maximum || count!=expected_count || primary!=expected_primary || memcmp(actual,expected,200) || memcmp(stress,wanted,100))
		fprintf(stderr,"Extract mismatch rule=%d flags=%x request=%d control=%d maximum=%d/%d count=%d/%d primary=%d/%d\n",tr->langopts.stress_rule,tr->langopts.stress_flags,request,control,result,maximum,count,expected_count,primary,expected_primary);
	TEST_ASSERT(result==maximum && count==expected_count && primary==expected_primary);
	TEST_ASSERT(memcmp(actual,expected,200)==0);TEST_ASSERT(memcmp(stress,wanted,100)==0);extracted++;
}
static void compare_assign(Translator *tr,const unsigned char *word,unsigned dflags,int has_dictionary,int tonic,int control)
{
	char actual[200],expected[200];memset(actual,0x5a,200);memcpy(actual,word,strlen((const char*)word)+1);memcpy(expected,actual,200);
	Translator reference=*tr;tr->prev_last_stress=reference.prev_last_stress=(int)(next()%7)-1;
	ReferenceSetWordStress(&reference,expected,has_dictionary?&dflags:NULL,tonic,control);
	SetWordStress(tr,actual,has_dictionary?&dflags:NULL,tonic,control);
	if(memcmp(actual,expected,200) || tr->prev_last_stress!=reference.prev_last_stress) {
		fprintf(stderr,"Assign mismatch language=%x rule=%d flags=%x dflags=%x dict=%d tonic=%d control=%d previous=%d/%d\n",tr->translator_name,tr->langopts.stress_rule,tr->langopts.stress_flags,dflags,has_dictionary,tonic,control,tr->prev_last_stress,reference.prev_last_stress);
		fprintf(stderr,"input:");for(const unsigned char *p=word;*p;p++)fprintf(stderr," %u",*p);
		fprintf(stderr,"\nactual:");for(unsigned char *p=(unsigned char*)actual;*p;p++)fprintf(stderr," %u",*p);
		fprintf(stderr,"\nexpected:");for(unsigned char *p=(unsigned char*)expected;*p;p++)fprintf(stderr," %u",*p);fprintf(stderr,"\n");
	}
	TEST_ASSERT(memcmp(actual,expected,200)==0);TEST_ASSERT(tr->prev_last_stress==reference.prev_last_stress);assigned++;
}
static void synthetic(void)
{
	PHONEME_TAB records[256]={0};
	for(int i=0;i<256;i++){records[i].code=i;records[i].type=phVIRTUAL;phoneme_tab[i]=records+i;}
	records[0].type=phPAUSE;records[1].type=phSTRESS;records[1].program=1;
	for(int level=0;level<7;level++){int code=(unsigned char)stress_phonemes[level];records[code].type=phSTRESS;records[code].std_length=level;}
	records[8].type=phSTRESS;
	records[10].type=records[11].type=records[23].type=phPAUSE;
	records[13].type=phVOWEL;
	for(int i=40;i<46;i++){records[i].type=phVOWEL;records[i].mnemonic='a'+i-40;}
	records[41].phflags=phUNSTRESSED;records[42].phflags=phLONG;records[43].phflags=phNONSYLLABIC;records[44].phflags=phLONG|phUNSTRESSED;
	records[50].type=phNASAL;records[50].mnemonic='n';records[51].type=phSTOP;records[51].mnemonic='t';
	records[52].type=phFRICATIVE;records[52].mnemonic='s';records[53].type=phNASAL;records[53].phflags=phLONG;
	phoneme_tab[250]=NULL;n_phoneme_tab=256;
	Translator *tr=SelectTranslator("en");TEST_ASSERT(tr);
	const int rules[]={0,1,2,3,4,5,6,7,8,9,12,13,15};
	const unsigned flags[]={0,S_NO_DIM,S_FINAL_DIM,S_FINAL_DIM_ONLY,S_FINAL_NO_2,S_NO_AUTO_2,S_2_TO_HEAVY,S_FIRST_PRIMARY,S_FINAL_VOWEL_UNSTRESSED,S_FINAL_SPANISH,S_2_SYL_2,S_INITIAL_2,0x8000,S_MID_DIM,S_PRIORITY_STRESS,S_FINAL_LONG,0xffffff};
	const unsigned char examples[][200]={
		{40,0},{40,50,40,51,40,0},{6,40,7,40,41,50,20,0},
		{40,40,8,0},{3,40,42,12,50,50,40,0},{1,15,40,12,52,0},
		{41,50,20,40,53,42,0},{40,50,52,0},{40,40,50,0},
		{40,43,40,0},{250,40,255,0},{5,40,5,40,0},{40,12,40,12,40,12,0},
	};
	for(size_t rule=0;rule<sizeof(rules)/sizeof(*rules);rule++)for(size_t flag=0;flag<sizeof(flags)/sizeof(*flags);flag++) {
		tr->langopts.stress_rule=rules[rule];tr->langopts.stress_flags=flags[flag];
		tr->langopts.unstressed_wd1=1;tr->langopts.unstressed_wd2=3;
		for(size_t word=0;word<sizeof(examples)/sizeof(*examples);word++)for(int option=0;option<32;option++) {
			tr->translator_name=(unsigned[]){L('e','s'),L('c','a'),L('a','n'),L('i','a')}[option%4];
			tr->langopts.vowel_pause=(option%4)*16;tr->langopts.param[LOPT_IT_LENGTHEN]=(option%4==3)?17:option%4;
			compare_extract(tr,examples[word],option%9,option&1);
			compare_assign(tr,examples[word],option%16,option&1,(option%8)-1,option%4);
		}
	}
	for(int trial=0;trial<40000;trial++) {
		unsigned char word[200]={0};size_t length=0;unsigned syllables=next()%35+1;
		for(unsigned syllable=0;syllable<syllables && length<170;syllable++) {
			if(next()&1)word[length++]=(unsigned char)stress_phonemes[next()%7];
			word[length++]=40+(next()%6);
			if(next()%3==0)word[length++]=12;
			if(next()%3==0){word[length++]=50+(next()%4);if(next()%4==0)word[length++]=20;}
		}
		word[length]=0;
		tr->langopts.stress_rule=rules[next()%(sizeof(rules)/sizeof(*rules))];tr->langopts.stress_flags=next()&0xffffff;
		tr->langopts.vowel_pause=(next()%4)*16;tr->langopts.param[LOPT_IT_LENGTHEN]=next()&31;
		tr->langopts.unstressed_wd1=next()%7;tr->langopts.unstressed_wd2=next()%7;
		compare_extract(tr,word,next()%10,next()%2);compare_assign(tr,word,next()%16,next()%2,(int)(next()%8)-1,next()%4);
	}
	unsigned char long_word[200]={0};for(int i=0;i<198;i++)long_word[i]=(i&1)?50:40;
	for(size_t rule=0;rule<sizeof(rules)/sizeof(*rules);rule++)for(int control=0;control<4;control++) {
		tr->langopts.stress_rule=rules[rule];tr->langopts.stress_flags=0;
		compare_extract(tr,long_word,0,control&1);compare_assign(tr,long_word,0,1,-1,control);
	}
	unsigned char word[200]={40,0},saved[200];memcpy(saved,word,200);
	RustWordStress settings={0};int32_t previous=77;
	TEST_ASSERT(espeak_rs_word_stress(word,2,(const PHONEME_TAB *const *)phoneme_tab,256,&settings,NULL,7,0,&previous)!=0);
	TEST_ASSERT(previous==77 && memcmp(word,saved,200)==0);
	memset(word,40,200);memcpy(saved,word,200);
	TEST_ASSERT(espeak_rs_word_stress(word,200,(const PHONEME_TAB *const *)phoneme_tab,256,&settings,NULL,-1,0,&previous)!=0);
	TEST_ASSERT(previous==77 && memcmp(word,saved,200)==0);
	signed char stresses[100],saved_stresses[100];memset(stresses,0x5a,100);memcpy(saved_stresses,stresses,100);
	int32_t count=77,primary=77,maximum=77;
	TEST_ASSERT(espeak_rs_vowel_stress(word,200,(const PHONEME_TAB *const *)phoneme_tab,0,0,stresses,&count,&primary,&maximum)!=0);
	TEST_ASSERT(count==77 && primary==77 && maximum==77 && memcmp(stresses,saved_stresses,100)==0 && memcmp(word,saved,200)==0);
	DeleteTranslator(tr);
}
static void actual_tables(void)
{
	TEST_ASSERT(espeak_Initialize(AUDIO_OUTPUT_RETRIEVAL,0,NULL,0)==22050);
	const char *languages[]={"en","es","ca","an","ia","ru","hi","tr","fi","kl","ml","eu","de","fr","hu","hy","vi"};
	for(int table=0;table<N_PHONEME_TABS && phoneme_tab_list[table].n_phonemes>0;table++) {
		SelectPhonemeTable(table);unsigned char vowels[256],consonants[256];int nv=0,nc=0;
		for(int code=1;code<n_phoneme_tab;code++)if(phoneme_tab[code]) {
			if(phoneme_tab[code]->type==phVOWEL && !(phoneme_tab[code]->phflags&phNONSYLLABIC))vowels[nv++]=code;
			else if(phoneme_tab[code]->type>=phLIQUID && phoneme_tab[code]->type<=phNASAL)consonants[nc++]=code;
		}
		if(nv==0)continue;
		for(size_t language=0;language<sizeof(languages)/sizeof(*languages);language++) {
			Translator *tr=SelectTranslator(languages[language]);TEST_ASSERT(tr);
			for(int trial=0;trial<80;trial++) {
				unsigned char word[200]={0};size_t length=0;unsigned syllables=next()%16+1;
				for(unsigned syllable=0;syllable<syllables;syllable++) {
					if(next()&1)word[length++]=(unsigned char)stress_phonemes[next()%7];
					word[length++]=vowels[next()%nv];if(nc&&next()%3==0)word[length++]=consonants[next()%nc];
				}
				compare_extract(tr,word,next()%10,next()%2);compare_assign(tr,word,next()%16,next()%2,(int)(next()%8)-1,next()%4);
			}
			DeleteTranslator(tr);
		}
	}
	TEST_ASSERT(espeak_Terminate()==EE_OK);
}
int main(void)
{
	_Static_assert(sizeof(RustWordStress)==32,"stress settings layout");
	actual_tables();synthetic();
	printf("Matched %zu vowel extractions and %zu word-stress assignments\n",extracted,assigned);
	return 0;
}
