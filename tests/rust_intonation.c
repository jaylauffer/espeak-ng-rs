/* Native clause intonation against independently extracted retained C.
 * SPDX-License-Identifier: GPL-3.0-or-later */
#include "config.h"
#include "test_assert.h"
#include <stdbool.h>
#include <stdint.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <espeak-ng/espeak_ng.h>
#include <espeak-ng/speak_lib.h>
#include <espeak-ng/encoding.h>
#include "intonation.h"
#include "phoneme.h"
#include "synthdata.h"
#include "synthesize.h"
#include "translate.h"
#include "rust_data.h"
static PHONEME_LIST reference_list[N_PHONEME_LIST+1];
static int reference_n;
static int ReferenceCode(unsigned int mnemonic)
{
	for(int ix=0;ix<n_phoneme_tab;ix++)if(phoneme_tab[ix] && phoneme_tab[ix]->mnemonic==mnemonic)return phoneme_tab[ix]->code;
	return 0;
}
static PHONEME_TAB *ReferenceTone(const PHONEME_LIST *p){return p->tone_ph_data?p->tone_ph_data:phoneme_tab[p->tone_ph];}
#define CalcPitches ReferenceCalcPitches
#define phoneme_list reference_list
#define n_phoneme_list reference_n
#define PhonemeCode ReferenceCode
#define TonePhoneme ReferenceTone
#include "intonation_reference.inc"
#undef CalcPitches
#undef phoneme_list
#undef n_phoneme_list
#undef PhonemeCode
#undef TonePhoneme
static unsigned seed=0x51c3e7a9u;
static unsigned next(void){seed^=seed<<13;seed^=seed>>17;seed^=seed<<5;return seed;}
static size_t compared,tone_runs,emphasized,clause_splits,rejected;
static unsigned char codes[N_PHONEME_TAB];static int n_codes;
static int random_stress(void)
{
	static const unsigned char weights[]={0,1,1,2,2,3,4,4,4,5,6,7};
	return weights[next()%sizeof(weights)];
}
static int random_list(int n)
{
	memset(reference_list,0,sizeof(reference_list));
	for(int ix=0;ix<n;ix++) {
		PHONEME_LIST *p=&reference_list[ix];int code=codes[next()%n_codes];unsigned kind=next()%20;
		if(kind==0 && phoneme_tab[phonPAUSE_CLAUSE])code=phonPAUSE_CLAUSE;
		else if(kind==1 && phoneme_tab[phonPAUSE])code=phonPAUSE;
		p->ph=phoneme_tab[code];p->phcode=code;p->type=next()%16?p->ph->type:next()%10;
		bool syllable=p->type==phVOWEL?next()%10!=0:next()%10==0;
		p->synthflags=(unsigned short)((next()&~SFLAG_SYLLABLE&0xffff)|(syllable?SFLAG_SYLLABLE:0));
		p->stresslevel=random_stress();
		p->tone_ph=next()%5<3?0:codes[next()%n_codes];
		p->tone_ph_data=next()%10<3?phoneme_tab[codes[next()%n_codes]]:NULL;
		p->newword=next()%4==0?(unsigned char)(next()%16):0;
		p->env=next();p->pitch1=next();p->pitch2=next();p->length=next();p->amp=next();p->sourceix=next();
	}
	int syllables=0;
	for(int ix=0;ix+1<n;ix++)if(reference_list[ix].synthflags&SFLAG_SYLLABLE)syllables++;
	return syllables;
}
static void compare(Translator *tr,int n,int clause_type)
{
	int syllables=random_list(n);
	for(int ix=0;ix+1<n;ix++)if((reference_list[ix].synthflags&SFLAG_SYLLABLE) && reference_list[ix].stresslevel==6)emphasized++;
	for(int ix=0;ix+1<n;ix++)if(reference_list[ix].ph->code==phonPAUSE_CLAUSE)clause_splits++;
	memcpy(phoneme_list,reference_list,sizeof(reference_list));
	reference_n=n_phoneme_list=n;
	ReferenceCalcPitches(tr,clause_type);CalcPitches(tr,clause_type);
	if(memcmp(phoneme_list,reference_list,sizeof(PHONEME_LIST)*(size_t)n)) {
		for(int ix=0;ix<n;ix++)if(memcmp(&phoneme_list[ix],&reference_list[ix],sizeof(PHONEME_LIST)))
			fprintf(stderr,"entry %d/%d lang=%x tone=%d group=%d clause=%d: stress %d/%d tone %d/%d env %d/%d pitch %d,%d/%d,%d\n",
				ix,n,tr->translator_name,tr->langopts.tone_language,tr->langopts.intonation_group,clause_type,
				phoneme_list[ix].stresslevel,reference_list[ix].stresslevel,phoneme_list[ix].tone_ph,reference_list[ix].tone_ph,
				phoneme_list[ix].env,reference_list[ix].env,phoneme_list[ix].pitch1,phoneme_list[ix].pitch2,reference_list[ix].pitch1,reference_list[ix].pitch2);
		TEST_ASSERT(false);
	}
	compared++;if(tr->langopts.tone_language==1 && syllables)tone_runs++;
}
static void rejections(Translator *tr)
{
	// Inputs whose C reads are out of bounds leave the list unchanged.
	random_list(12);
	for(int ix=0;ix<11;ix++)reference_list[ix].synthflags|=SFLAG_SYLLABLE;
	LANGUAGE_OPTIONS saved=tr->langopts;
	tr->langopts.tone_language=0;tr->langopts.intonation_group=0;tr->langopts.tunes[0]=(unsigned char)n_tunes;
	for(int trial=0;trial<3;trial++) {
		// invalid tune, then stress past the drop tables, then clause type
		if(trial==1)tr->langopts.tunes[0]=0,reference_list[3].stresslevel=8;
		if(trial==2)reference_list[3].stresslevel=4;
		memcpy(phoneme_list,reference_list,sizeof(reference_list));n_phoneme_list=12;
		CalcPitches(tr,trial==2?6:0);
		TEST_ASSERT(memcmp(phoneme_list,reference_list,sizeof(PHONEME_LIST)*12)==0);rejected++;
	}
	tr->langopts.tone_language=1;tr->translator_name=L('z','h');reference_list[2].tone_ph=0;
	memcpy(phoneme_list,reference_list,sizeof(reference_list));n_phoneme_list=12;
	PHONEME_TAB *saved_pause=phoneme_tab[phonPAUSE];phoneme_tab[phonPAUSE]=NULL;
	CalcPitches(tr,0);phoneme_tab[phonPAUSE]=saved_pause;
	TEST_ASSERT(memcmp(phoneme_list,reference_list,sizeof(PHONEME_LIST)*12)==0);rejected++;
	tr->langopts=saved;
}
int main(void)
{
	_Static_assert(sizeof(RustPitchEntry)==14 && sizeof(RustPitchSettings)==72,"intonation snapshot layout");
	TEST_ASSERT(espeak_Initialize(AUDIO_OUTPUT_RETRIEVAL,0,NULL,0)==22050);
	TEST_ASSERT(n_tunes>0);
	static const char *languages[]={"en","de","fr","es","ru","sv","nl","hi","ja","vi","cmn","hak","yue","zh","sw","tr"};
	static const int tone_names[]={L('v','i'),L3('h','a','k'),L('z','h'),L3('c','m','n'),L('e','n')};
	for(int table=0;table<N_PHONEME_TABS && phoneme_tab_list[table].n_phonemes>0;table++) {
		SelectPhonemeTable(table);n_codes=0;
		for(int code=0;code<n_phoneme_tab;code++)if(phoneme_tab[code])codes[n_codes++]=code;
		// Tone rules dereference the default, pause and lookup-miss records.
		bool tones=phoneme_tab[0] && phoneme_tab[phonPAUSE] && phoneme_tab[phonDEFAULTTONE];
		if(n_codes==0)continue;
		for(size_t language=0;language<sizeof(languages)/sizeof(*languages);language++) {
			Translator *tr=SelectTranslator(languages[language]);TEST_ASSERT(tr);
			LANGUAGE_OPTIONS saved=tr->langopts;int saved_name=tr->translator_name;
			for(int trial=0;trial<60;trial++) {
				tr->langopts=saved;tr->translator_name=saved_name;
				unsigned variant=next()%8;
				if(variant>=5) {
					tr->langopts.intonation_group=next()%10;
					for(int i=0;i<6;i++)tr->langopts.tunes[i]=next()%n_tunes;
					for(int g=0;g<INTONATION_TYPES;g++)for(int i=0;i<PUNCT_INTONATIONS;i++)tr->punct_to_tone[g][i]=next()%13;
				}
				if(variant==7)tr->langopts.tone_language=!tr->langopts.tone_language;
				// With a fixed-table group, the remainder after an emphasis split
				// takes langopts.tunes as a fixed-table index (13 entries).
				if(tr->langopts.intonation_group!=0)for(int i=0;i<6;i++)tr->langopts.tunes[i]%=13;
				if(tr->langopts.tone_language==1 && next()%2)tr->translator_name=tone_names[next()%5];
				if(tr->langopts.tone_language==1 && !tones)tr->langopts.tone_language=0;
				option_tone_flags=next()%3==0?OPTION_EMPHASIZE_PENULTIMATE:0;
				int n=next()%10==0?(int)(next()%(N_PHONEME_LIST+1))+1:(int)(next()%60);
				compare(tr,n,next()%6);
			}
			rejections(tr);
			tr->langopts=saved;tr->translator_name=saved_name;
			DeleteTranslator(tr);
		}
	}
	option_tone_flags=0;
	TEST_ASSERT(espeak_Terminate()==EE_OK);
	printf("Matched %zu clauses (%zu tone-language, %zu emphasized syllables, %zu clause pauses); %zu rejected inputs unchanged\n",
		compared,tone_runs,emphasized,clause_splits,rejected);
	return 0;
}
