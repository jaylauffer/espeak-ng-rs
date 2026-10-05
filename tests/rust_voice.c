/* Acoustic configuration against the retained C routines and voice files.
 * SPDX-License-Identifier: GPL-3.0-or-later */
#include "config.h"
#include "test_assert.h"
#include <ctype.h>
#include <dirent.h>
#include <stdint.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <sys/stat.h>
#include <wctype.h>
#include <unistd.h>
#include <strings.h>
#include <espeak-ng/speak_lib.h>
#include "voice.h"
#include "synthesize.h"
#include "speech.h"
#include "rust_data.h"
#include "synthdata.h"
#include "langopts.h"
#include "common.h"
static voice_t expected;
static voice_t *reference_voice = &expected;
static SPEED_FACTORS reference_speed;
static int reference_rates[9], reference_points[12], reference_replacements, speed_calls;
static const int formant_rate_22050[9] = {240,170,170,170,170,170,170,170,170};
static void ReferenceBreath(void) {}
static void ReferenceSpeed(int control) { TEST_ASSERT(control == 3); speed_calls++; }
#define voice reference_voice
#define speed reference_speed
#define formant_rate reference_rates
#define tone_points reference_points
#define n_replace_phonemes reference_replacements
#define InitBreath ReferenceBreath
#define SetSpeed ReferenceSpeed
#define VoiceReset ReferenceReset
#define ReadTonePoints ReferenceTonePoints
#define Read8Numbers ReferenceNumbers
#include "voice_tone_reference.inc"
#include "voice_defaults_reference.inc"
#include "voice_formant_reference.inc"
#include "voice_numbers_reference.inc"
#include "voice_strip_reference.inc"
static int ReferenceApply(const char *keyword, char *p)
{
	int value, ix, pitch1, pitch2;
	int key = LookupMnem(keyword_tab,keyword);
	switch (key) {
#include "voice_pitch_reference.inc"
#include "voice_acoustic_reference.inc"
#include "voice_fast_reference.inc"
	case V_KLATT:
#include "voice_klatt_reference.inc"
		break;
	default: return 1;
	}
	return 0;
}
#undef voice
#undef speed
#undef formant_rate
#undef tone_points
#undef n_replace_phonemes
#undef InitBreath
#undef SetSpeed
#undef VoiceReset
#undef ReadTonePoints
#undef Read8Numbers
static const MNEM_TAB genders[]={{"male",ENGENDER_MALE},{"female",ENGENDER_FEMALE},{NULL,ENGENDER_MALE}};
#define DEFAULT_LANGUAGE_PRIORITY 5
#define ReadVoiceFile ReferenceMetadata
#include "voice_metadata_reference.inc"
#undef ReadVoiceFile
#define ScoreVoice ReferenceScore
#include "voice_scoring_reference.inc"
#undef ScoreVoice
#define ExtractVoiceVariantName ReferenceVariant
#include "voice_variant_reference.inc"
#undef ExtractVoiceVariantName
static int n_voices_list;
static espeak_VOICE *voices_list[500];
#undef USE_RUST_CORE
#define SelectVoiceByName ReferenceByName
#include "voice_names_reference.inc"
#undef SelectVoiceByName
#define USE_RUST_CORE 1
static int reference_tone_flags;
static int ReferenceCheck(Translator *tr,const MNEM_TAB *table,int key) {(void)table;(void)key;TEST_ASSERT(tr!=NULL);return 0;}
static int ReferenceLookupTune(const char *name) {for(int i=0;i<n_tunes;i++)if(strcmp(tunes[i].name,name)==0)return i;return -1;}
#define ReadNumbers ReferenceOrdinals
#define Read8Numbers ReferenceNumbers
#define ProcessLanguageOptions ReferenceSeparators
#define CheckTranslator ReferenceCheck
#define LookupTune ReferenceLookupTune
#define LoadLanguageOptions ReferenceLanguageOptions
#define option_tone_flags reference_tone_flags
#include "language_ordinals_reference.inc"
#include "language_separators_reference.inc"
#include "language_options_reference.inc"
#undef ReadNumbers
#undef Read8Numbers
#undef ProcessLanguageOptions
#undef CheckTranslator
#undef LookupTune
#undef LoadLanguageOptions
#undef option_tone_flags
static int32_t language_environment(void *opaque,uint32_t kind,uint32_t key,const unsigned char *name,size_t length,int32_t number)
{
    (void)opaque;(void)key;(void)number;
    if(kind!=0)return -1;
    for(int i=0;i<n_tunes;i++)if(strlen(tunes[i].name)==length&&!memcmp(tunes[i].name,name,length))return i;
    return -1;
}
static uint32_t seed = 0x672154ab;
static uint32_t random32(void) {seed ^= seed<<13;seed ^= seed>>17;seed ^= seed<<5;return seed;}
static unsigned long comparisons, resets, files, scans, language_comparisons;
static unsigned long metadata_comparisons,score_comparisons,name_comparisons,variant_comparisons;
static void metadata_pair(FILE *file,const char *identifier)
{
    rewind(file);espeak_VOICE *expected=ReferenceMetadata(file,identifier,0);
    rewind(file);RustVoiceMetadata actual={.variants=4};char line[120];
    while(fgets(line,sizeof(line),file))TEST_ASSERT(espeak_rs_voice_metadata_line(&actual,line)<2);
    TEST_ASSERT((expected==NULL)==(actual.language_count==0));
    if(expected){
        const char *name=actual.name[0]?(const char*)actual.name:identifier;
        TEST_ASSERT(strcmp(expected->name,name)==0);TEST_ASSERT(strcmp(expected->identifier,identifier)==0);
        TEST_ASSERT(memcmp(expected->languages,actual.languages,actual.language_length+1)==0);
        TEST_ASSERT(expected->gender==espeak_rs_voice_metadata_gender(&actual));
        TEST_ASSERT(expected->age==(unsigned char)actual.age);TEST_ASSERT(expected->xx1==(unsigned char)actual.variants);
        TEST_ASSERT(expected->variant==0);free(expected);
    }
    metadata_comparisons++;
}
static void generated_selection(void)
{
    FILE *file=tmpfile();TEST_ASSERT(file!=NULL);
    for(int trial=0;trial<10000;trial++){
        rewind(file);TEST_ASSERT(ftruncate(fileno(file),0)==0);
        fprintf(file,"name Native %d //comment\nlanguage en-gb %d\ngender %s %d\nvariants %d\n",trial,(int)(random32()%301)-50,
            (const char*[]){"male","female","unknown"}[trial%3],(int)(random32()%401)-100,(int)(random32()%501)-100);
        fprintf(file,"language en %u\nlanguage de\nlanguage variants 0\n",random32()%128);
        if(trial%5==0)fprintf(file,"gender female invalid\nvariants invalid\n");
        fflush(file);metadata_pair(file,"synthetic/native");
    }
    fclose(file);
    for(int trial=0;trial<50000;trial++){
        char name[100],copy[100];
        switch(trial%6){case 0:snprintf(name,sizeof(name),"en+%u",random32()%10000);break;
            case 1:snprintf(name,sizeof(name),"en+named%u",random32()%10000);break;
            case 2:strcpy(name,"en+12junk");break;case 3:strcpy(name,"en+-3");break;case 4:strcpy(name,"en+");break;default:strcpy(name,"en");break;}
        strcpy(copy,name);int number=(int)(random32()%10000)-100,directory=trial%2;
        const char *expected=ReferenceVariant(trial%7==0?NULL:copy,number,directory);
        unsigned char actual[40];size_t base_length;
        TEST_ASSERT(espeak_rs_voice_variant(trial%7==0?NULL:name,number,directory,PATHSEP,&base_length,&actual)==0);
        TEST_ASSERT(strcmp(expected,(const char*)actual)==0);
        if(trial%7!=0){name[base_length]=0;TEST_ASSERT(strcmp(copy,name)==0);}variant_comparisons++;
    }
    const char *languages[]={"en","en-gb","en-us","en-gb-x","de","variants","x/"};
    for(int trial=0;trial<200000;trial++){
        char packed[200];unsigned priority1=1+random32()%127,priority2=1+random32()%127;
        const char *first=languages[random32()%6],*second=languages[random32()%6];
        size_t used=0;packed[used++]=priority1;strcpy(packed+used,first);used+=strlen(first)+1;
        packed[used++]=priority2;strcpy(packed+used,second);used+=strlen(second)+1;packed[used]=0;
        espeak_VOICE candidate={.name="Native",.identifier="x/Native",.languages=packed,.gender=random32()%4,.age=3+random32()%98};
        if(trial%7==0)candidate.age=0;
        const char *required=languages[random32()%7];int parts=1;for(const char *p=required;*p;p++)if(*p=='-')parts++;
        if(trial%13==0)parts=0;else if(trial%19==0)parts=-1;
        espeak_VOICE spec={.name=(const char*[]){NULL,"Native","x/Native","other"}[trial%4],.gender=random32()%4,.age=3+random32()%98};
        if(trial%5==0)spec.age=0;
        int expected=ReferenceScore(&spec,required,parts,strlen(required),&candidate);
        int actual=espeak_rs_voice_score(&spec,required,parts,strlen(required),&candidate);
        if(expected!=actual)fprintf(stderr,"Score mismatch %d: %s %s %d/%d\n",trial,required,first,expected,actual);
        TEST_ASSERT(expected==actual);score_comparisons++;
    }
    espeak_VOICE a={.name="Native",.identifier="lang/a",.languages="\005en\0"},b={.name="Other",.identifier="long/path/NATIVE",.languages="\005en\0"};
    espeak_VOICE *list[]={&a,&b,NULL};
    TEST_ASSERT(espeak_rs_voice_by_name(list,"native",PATHSEP)==&a);name_comparisons++;
    TEST_ASSERT(espeak_rs_voice_by_name(list,"lang/a",PATHSEP)==&a);name_comparisons++;
    TEST_ASSERT(espeak_rs_voice_by_name(list,"NATIVE",PATHSEP)==&a);name_comparisons++;
    a.name="unmatched";
    TEST_ASSERT(espeak_rs_voice_by_name(list,"NATIVE",PATHSEP)==&b);name_comparisons++;
    TEST_ASSERT(espeak_rs_voice_by_name(list,"a name longer than either identifier",PATHSEP)==NULL);name_comparisons++;
    for(int trial=0;trial<50000;trial++){
        /* Padded identifier allocations let the retained filename matcher
         * examine its old negative offsets without crossing allocation bounds. */
        char ids[6][160],names[6][80],requested[100];espeak_VOICE candidates[6]={0};espeak_VOICE *choices[7]={0};
        for(int i=0;i<6;i++){
            memset(ids[i],'~',sizeof(ids[i]));
            snprintf(ids[i]+80,80,"path%u/%s%u",random32()%4,(const char*[]){"Native","voice","x"}[trial%3],random32()%5);
            snprintf(names[i],80,"Name%u",random32()%5);
            candidates[i].name=names[i];candidates[i].identifier=ids[i]+80;candidates[i].languages="\005en\0";choices[i]=&candidates[i];
        }
        int pick=random32()%6;
        switch(trial%4){case 0:strcpy(requested,names[pick]);break;case 1:strcpy(requested,ids[pick]+80);break;
            case 2:strcpy(requested,strrchr(ids[pick]+80,'/')+1);break;default:strcpy(requested,"a_long_non_matching_identifier");break;}
        if(trial%5==0)for(char *p=requested;*p;p++)*p=toupper((unsigned char)*p);
        TEST_ASSERT(ReferenceByName(choices,requested)==espeak_rs_voice_by_name(choices,requested,PATHSEP));name_comparisons++;
    }
}
static void language_pair(Translator *reference,int key,char *text)
{
    RustLanguageOptions actual,expected_options;
    espeak_rust_language_capture(reference,reference_tone_flags,&actual);
    ReferenceLanguageOptions(reference,key,text);
    espeak_rust_language_capture(reference,reference_tone_flags,&expected_options);
    TEST_ASSERT(espeak_rs_language_option(&actual,key,text,NULL,language_environment)==0);
    if(memcmp(&actual,&expected_options,sizeof(actual))) {
        fprintf(stderr,"language mismatch key=%d text=%s\n",key,text);
        for(size_t byte=0;byte<sizeof(actual);byte++)if(((unsigned char *)&actual)[byte]!=((unsigned char *)&expected_options)[byte])
            fprintf(stderr,"byte %zu: %u/%u\n",byte,((unsigned char *)&expected_options)[byte],((unsigned char *)&actual)[byte]);
        TEST_ASSERT(false);
    }
    language_comparisons++;
}
static void generated_language(void)
{
    static const int keys[]={V_DICTMIN,V_DICTRULES,V_INTONATION,V_NUMBERS,V_LOWERCASE_SENTENCE,V_SPELLINGSTRESS,V_STRESSADD,V_STRESSAMP,V_STRESSLENGTH,V_STRESSOPT,V_STRESSRULE,V_TUNES,V_WORDGAP};
    Translator reference={0};
    for(int trial=0;trial<10000;trial++) {
        RustLanguageOptions options;
        for(size_t byte=0;byte<sizeof(options);byte++)((unsigned char *)&options)[byte]=random32();
        options.lowercase_sentence=trial&1;options.spelling_stress=(trial>>1)&1;
        espeak_rust_language_commit(&reference,&options);reference_tone_flags=options.tone_flags;
        for(size_t key=0;key<sizeof(keys)/sizeof(keys[0]);key++) {
            char text[180];
            if(keys[key]==V_DICTRULES||keys[key]==V_STRESSOPT||keys[key]==V_NUMBERS) {
                int max=keys[key]==V_NUMBERS?64:32;
                snprintf(text,sizeof(text),"%u %u %u %u",random32()%max,random32()%max,random32()%max,random32()%max);
            } else if(keys[key]==V_TUNES) snprintf(text,sizeof(text),"%s NULL %s",tunes[trial%n_tunes].name,tunes[(trial+1)%n_tunes].name);
            else snprintf(text,sizeof(text),"%d %d %d %d %d %d %d %d",(int)(random32()%1001)-500,(int)(random32()%1001)-500,(int)(random32()%1001)-500,120,240,360,480,600);
            if(trial%17==0 && keys[key]!=V_TUNES)strcpy(text,trial&1?"":"bad");
            language_pair(&reference,keys[key],text);
        }
        for(int parameter=0;parameter<N_LOPTS;parameter++) {
            char text[40];snprintf(text,sizeof(text),"%d",(int)random32());
            language_pair(&reference,0x100+parameter,text);
        }
    }
}
static void equal_voice(voice_t *actual, const char *keyword, const char *text)
{
	if (memcmp(&expected,actual,sizeof(expected))) {
		fprintf(stderr,"voice mismatch: %s %s\n",keyword,text);
		for (size_t byte=0;byte<sizeof(expected);byte++)
			if (((unsigned char *)&expected)[byte] != ((unsigned char *)actual)[byte])
				fprintf(stderr,"byte %zu: %u/%u\n",byte,((unsigned char *)&expected)[byte],((unsigned char *)actual)[byte]);
		TEST_ASSERT(false);
	}
}
static void reset_pair(voice_t *actual)
{
	int points[12] = {600,170,1200,135,2000,110,3000,110,-1,0};
	memcpy(reference_points,points,sizeof(points));
	ReferenceReset(1);
	int rates[9], fast;
	TEST_ASSERT(espeak_rs_voice_reset(actual,samplerate,points,rates,&fast) == 0);
	equal_voice(actual,"reset","");
	TEST_ASSERT(memcmp(points,reference_points,sizeof(points)) == 0);
	TEST_ASSERT(memcmp(rates,reference_rates,sizeof(rates)) == 0 && fast == reference_speed.fast_settings);
	resets++;
}
static void apply_pair(voice_t *actual, int *fast, const char *keyword, char *text)
{
	speed_calls=0;
	int reference = ReferenceApply(keyword,text);
	uint32_t update=99;
	int result=espeak_rs_voice_attribute(actual,keyword,text,1,fast,&update);
	TEST_ASSERT(result == reference);
	if (result == 0) {
		equal_voice(actual,keyword,text);
		TEST_ASSERT(*fast == reference_speed.fast_settings && update == (uint32_t)speed_calls);
		comparisons++;
	}
}
static void generated(void)
{
	static const char *keywords[] = {"formant","pitch","echo","flutter","roughness","clarity","tone","voicing","breath","breathw","consonants","speed","klatt","fast_test2"};
	voice_t actual;
	for (int trial=0;trial<20000;trial++) {
		for (size_t byte=0;byte<sizeof(actual);byte++) ((unsigned char *)&expected)[byte]=random32();
		actual=expected;
		int old_rate=samplerate;
		samplerate=(int[]){8000,16000,22050,44100,48000,96000}[trial%6];
		reset_pair(&actual);
		samplerate=old_rate;
		int fast=reference_speed.fast_settings;
		for (int ix=0;ix<14;ix++) {
			char text[200];
			int a=(int)(random32()%401)-100,b=(int)(random32()%301)-100,c=(int)(random32()%301)-100;
			if (ix==0) snprintf(text,sizeof(text),"%d %d %d %d %d",trial%12-2,a,b,c,(int)(random32()%3001)-1500);
			else if (ix==1) snprintf(text,sizeof(text),"%d %d",10+random32()%200,10+random32()%200);
			else if (ix==6) snprintf(text,sizeof(text),"%u %d %u %d %u %d %u %d",random32()%500,a,500+random32()%1500,b,2000+random32()%2000,c,4000+random32()%4001,a);
			else snprintf(text,sizeof(text),"%d %d %d 40 50 60 70 80 999",a,b,c);
			if (trial%19==0 && ix != 6) strcpy(text,trial%2 ? "" : " invalid");
			apply_pair(&actual,&fast,keywords[ix],text);
		}
	}
	static const char *strings[]={""," \t\n","+","-","+3 -4 5","0x12 14","12.5 16","1 2 bad 3","-2147483648 2147483647","\v+0\f-0", "123junk", "1 2 3 4 5 6 7 8 9 10"};
	for (size_t ix=0;ix<sizeof(strings)/sizeof(strings[0]);ix++) {
		int a[12],b[12];
		int expected_count=ReferenceNumbers((char *)strings[ix],a), actual_count=Read8Numbers((char *)strings[ix],b);
		if (expected_count != actual_count) fprintf(stderr,"scanner mismatch index=%zu text=%s count=%d/%d\n",ix,strings[ix],expected_count,actual_count);
		TEST_ASSERT(expected_count == actual_count);
		TEST_ASSERT(memcmp(a,b,8*sizeof(int)) == 0);
		ReferenceTonePoints((char *)strings[ix],a); ReadTonePoints((char *)strings[ix],b);
		TEST_ASSERT(memcmp(a,b,sizeof(a)) == 0); scans++;
	}
}
static void voice_files(const char *root)
{
	DIR *directory=opendir(root); TEST_ASSERT(directory != NULL);
	struct dirent *entry;
	while ((entry=readdir(directory)) != NULL) {
		if (entry->d_name[0]=='.') continue;
		char path[N_PATH_BUF]; snprintf(path,sizeof(path),"%s/%s",root,entry->d_name);
		struct stat metadata; TEST_ASSERT(stat(path,&metadata) == 0);
		if (S_ISDIR(metadata.st_mode)) {voice_files(path);continue;}
		if (!S_ISREG(metadata.st_mode)) continue;
		FILE *file=fopen(path,"r"); TEST_ASSERT(file != NULL);
        metadata_pair(file,path);rewind(file);
		voice_t actual={0}; memset(&expected,0,sizeof(expected)); reset_pair(&actual);
		int fast=reference_speed.fast_settings; char line[N_PATH_BUF];
		Translator reference={0};reference_tone_flags=0;
		while (fgets_strip(line,sizeof(line),file)) {
			char *p=line; while (*p && !isspace((unsigned char)*p)) p++;
			if (*p) *p++=0; if (line[0]) {
                int key=LookupMnem(langopts_tab,line);
                if(key)language_pair(&reference,key,p);
                else apply_pair(&actual,&fast,line,p);
            }
		}
		fclose(file);files++;
	}
	closedir(directory);
}
int main(void)
{
	TEST_ASSERT(espeak_Initialize(AUDIO_OUTPUT_SYNCHRONOUS,0,NULL,0)>0);
	generated();
	generated_language();
    generated_selection();
	char path[N_PATH_BUF]; snprintf(path,sizeof(path),"%s/lang",path_home);voice_files(path);
	snprintf(path,sizeof(path),"%s/voices",path_home);voice_files(path);
	unsigned long built_files = files;
	voice_files(ESPEAK_VOICE_SOURCE_DIR "/lang");
	voice_files(ESPEAK_VOICE_SOURCE_DIR "/voices");
	printf("Covered %lu built and %lu source voice/language files (MBROLA backend not executed)\n",built_files,files-built_files);
	printf("Compared %lu acoustic attributes, %lu defaults, %lu real voice/language files and %lu scanner cases\n",comparisons,resets,files,scans);
	printf("Compared %lu native language-option snapshots including tunes and all parameter keys\n",language_comparisons);
    printf("Compared %lu native metadata records, %lu voice scores, %lu bounded name selections and %lu variants\n",metadata_comparisons,score_comparisons,name_comparisons,variant_comparisons);
	espeak_Terminate();return 0;
}
