/* Acoustic configuration against the retained C routines and voice files.
 * SPDX-License-Identifier: GPL-3.0-or-later */
#include "config.h"
#include "test_assert.h"
#include <ctype.h>
#include <dirent.h>
#include <errno.h>
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
#define N_VOICES_LIST 500
#define VoiceNameSorter ReferenceNameSorter
#define VoiceScoreSorter ReferenceScoreSorter
#include "voice_ordering_reference.inc"
#undef VoiceNameSorter
#undef VoiceScoreSorter
static int StorageAdmission(const char *,int,int);
#define GetVoices StorageDiscovery
#define AddToVoicesList StorageAdmission
#define ReadVoiceFile ReferenceMetadata
#include "voice_discovery_reference.inc"
#include "voice_admission_reference.inc"
#undef GetVoices
#undef AddToVoicesList
#undef ReadVoiceFile
#define ScoreVoice ReferenceScore
#define VoiceScoreSorter ReferenceScoreSorter
#define SetVoiceScores ReferenceRanks
#include "voice_ranking_reference.inc"
#undef ScoreVoice
#undef VoiceScoreSorter
#undef SetVoiceScores
#include "voice_variants_table_reference.inc"
#define SetVoiceScores ReferenceRanks
#define SelectVoiceByName ReferenceByName
#define ExtractVoiceVariantName ReferenceVariant
#define SelectVoice ReferenceSelection
#include "voice_selection_reference.inc"
#undef SetVoiceScores
#undef SelectVoiceByName
#undef ExtractVoiceVariantName
#undef SelectVoice
static Translator setup_translator;
static voice_t setup_voice;
static espeak_VOICE setup_selected;
static unsigned setup_language_calls,setup_table_calls;
static Translator *SetupTranslator(const char *name){TEST_ASSERT(name[0]!=0);setup_language_calls++;return &setup_translator;}
static int SetupTable(const char *name){(void)name;setup_table_calls++;return 0;}
static void SetupReplacement(char *text){(void)text;}
static void setup_reference(RustVoiceSetup *s,const char *keyword,char *value)
{
    int tone_only=s->tone_only,langix=s->language_length;bool language_set=s->language_set!=0,phonemes_set=s->phonemes_set!=0;
    char language_name[40];const char *language_type;Translator *translator=NULL;
    char *p=value;
#define translator_name (*(char(*)[40])s->translator)
#define new_dictionary (*(char(*)[40])s->dictionary)
#define phonemes_name (*(char(*)[40])s->phonemes)
#define voice_name (*(char(*)[40])s->name)
#define voice_languages (*(char(*)[100])s->languages)
    memcpy(setup_voice.language_name,s->language,sizeof(s->language));setup_selected.gender=s->gender;setup_selected.age=s->age;
    setup_language_calls=setup_table_calls=0;
#define voice (&setup_voice)
#define current_voice_selected setup_selected
#define SelectTranslator SetupTranslator
#define SelectPhonemeTableName SetupTable
    switch(LookupMnem(keyword_tab,keyword)){
#include "voice_setup_reference.inc"
#define PhonemeReplacement SetupReplacement
#undef USE_RUST_CORE
#include "voice_replacement_setup_reference.inc"
#define USE_RUST_CORE 1
#undef PhonemeReplacement
    default:break;
    }
#undef voice
#undef current_voice_selected
#undef SelectTranslator
#undef SelectPhonemeTableName
#undef translator_name
#undef new_dictionary
#undef phonemes_name
#undef voice_name
#undef voice_languages
    (void)translator;s->language_length=langix;s->languages[langix]=0;s->language_set=language_set;s->phonemes_set=phonemes_set;
    memcpy(s->language,setup_voice.language_name,sizeof(s->language));s->gender=setup_selected.gender;s->age=setup_selected.age;
}
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
static unsigned long catalog_comparisons,ranking_comparisons;
static unsigned long setup_comparisons;
static unsigned long mnemonic_comparisons,replacement_comparisons,mbrola_comparisons;
static unsigned long storage_comparisons;
static unsigned long list_comparisons;
static unsigned long request_comparisons,fallback_comparisons,identifier_comparisons;
static unsigned long directive_comparisons;
static RustMbrolaRequest backend_mbrola(char *);
static void reset_pair(voice_t *);
static RustVoiceAction DirectiveReference(voice_t *snapshot,RustVoiceSetup *setup,int *fast,int features,const char *keyword,char *value)
{
    RustVoiceAction result={0};int language=LookupMnem(langopts_tab,keyword);
    if(language){result.action=1;result.argument=language;return result;}
    int key=LookupMnem(keyword_tab,keyword);
    expected=*snapshot;reference_speed.fast_settings=*fast;speed_calls=0;
    if(!(key==V_KLATT&&!(features&1))&&ReferenceApply(keyword,value)==0){
        *snapshot=expected;*fast=reference_speed.fast_settings;result.action=2;result.argument=speed_calls;return result;
    }
    if(key==V_LANGUAGE||key==V_NAME||key==V_GENDER||key==V_DICTIONARY||key==V_PHONEMES||key==V_REPLACE||key==V_MAINTAINER||key==V_STATUS){
        setup_reference(setup,keyword,value);result.action=key==V_REPLACE?4:3;
        result.argument=setup_language_calls?1:setup_table_calls?2:0;return result;
    }
    if(key==V_MBROLA){result.action=(features&2)?5:6;if(features&2)result.backend=backend_mbrola(value);return result;}
    if(key==V_KLATT)result.action=7;
    return result;
}
static void generated_directives(void)
{
    static const char *keys[]={"language","name","gender","dictionary","phonemes","maintainer","status","replace","pitch","formant","tone","speed","klatt","mbrola","stressLength","numbers","unrecognized","fast_test2","breath"};
    static const char *values[]={"en-gb 2","Native name","female 30","custom","en","Maintainer","mature","1 a b","100 140","2 90 80 120","100 100 300 100 8000 100","110","1 2 3 4 5 60 7 8","en1 table 22050","160 180","2 3 33","ignored","450","10 20 30"};
    for(int trial=0;trial<20000;trial++){
        voice_t actual={0};expected=actual;reset_pair(&actual);int fast=reference_speed.fast_settings;
        RustVoiceSetup setup={.tone_only=trial%2};strcpy((char*)setup.translator,"en");strcpy((char*)setup.dictionary,"en");
        for(size_t i=0;i<sizeof(keys)/sizeof(keys[0]);i++){
            voice_t reference=actual;RustVoiceSetup metadata=setup;int expected_fast=fast;
            RustVoiceAction expected_action=DirectiveReference(&reference,&metadata,&expected_fast,trial%4,keys[i],(char*)values[i]),action={0};
            TEST_ASSERT(espeak_rs_voice_directive(&actual,&setup,&fast,trial%4,keys[i],values[i],&action)==0);
            TEST_ASSERT(action.action==expected_action.action&&action.argument==expected_action.argument);
            TEST_ASSERT(memcmp(&actual,&reference,sizeof(actual))==0&&fast==expected_fast);
            TEST_ASSERT(strcmp((char*)setup.translator,(char*)metadata.translator)==0&&strcmp((char*)setup.dictionary,(char*)metadata.dictionary)==0);
            TEST_ASSERT(strcmp((char*)setup.phonemes,(char*)metadata.phonemes)==0&&strcmp((char*)setup.name,(char*)metadata.name)==0);
            TEST_ASSERT(strcmp((char*)setup.language,(char*)metadata.language)==0&&setup.language_length==metadata.language_length);
            TEST_ASSERT(memcmp(setup.languages,metadata.languages,setup.language_length+1)==0);
            TEST_ASSERT(setup.language_set==metadata.language_set&&setup.phonemes_set==metadata.phonemes_set&&setup.gender==metadata.gender&&setup.age==metadata.age);
            TEST_ASSERT(memcmp(&action.backend,&expected_action.backend,sizeof(action.backend))==0);directive_comparisons++;
        }
    }
}
static int request_probe_count,request_probe_result;
static char request_probe_paths[2][4096];
static int RequestProbe(const char *path){TEST_ASSERT(request_probe_count<2);strcpy(request_probe_paths[request_probe_count++],path);return request_probe_result;}
static int64_t request_probe(void *opaque,const unsigned char *path,size_t length){(void)opaque;TEST_ASSERT(path[length]==0);return RequestProbe((char*)path);}
static voice_t *RequestPathsReference(const char *vname,int control,const char *root,RustVoiceRequest *out)
{
    char voicename[40]={0},buf[N_PATH_BUF]={0};
#define path_home root
#define GetFileLength RequestProbe
#include "voice_request_paths_reference.inc"
#undef path_home
#undef GetFileLength
    strcpy((char*)out->path,buf);strcpy((char*)out->name,voicename);out->control=control;return &expected;
}
static int fallback_table_found,fallback_table_calls;
static int FallbackTable(const char *name){(void)name;fallback_table_calls++;return fallback_table_found?0:-1;}
static voice_t *FallbackReference(RustVoiceRequest *request,int opened,char result[40])
{
    const char *language_type;char voicename[40];memcpy(voicename,request->name,40);
    int control=request->control;FILE *f_voice=opened?(FILE*)(uintptr_t)1:NULL;
#define SelectPhonemeTableName FallbackTable
#include "voice_fallback_reference.inc"
#undef SelectPhonemeTableName
    strcpy(result,language_type);return &expected;
}
static void IdentifierReference(char voice_identifier[40],const char *vname,int tone_only)
{
    if(!tone_only){strncpy0(voice_identifier,vname,40);return;}
    char *p;
    // Preserve sizeof from the original static 40-byte array, not the pointer
    // parameter used by this standalone independent driver.
#define voice_identifier (*(char(*)[40])voice_identifier)
#include "voice_identifier_reference.inc"
#undef voice_identifier
}
static void generated_requests(void)
{
    for(int trial=0;trial<20000;trial++){
        char root[4200],name[120];int root_length=trial%3?trial%40:trial%4200;
        memset(root,'a',root_length);root[root_length]=0;
        int name_length=trial%120;memset(name,'b',name_length);name[name_length]=0;
        int control=random32()%64;request_probe_result=(int[]){1,0,-EISDIR,-ENOENT,123}[trial%5];
        RustVoiceRequest reference={0},actual={0};request_probe_count=0;
        int expected_status=RequestPathsReference(name,control,root,&reference)==NULL?1:0;
        char expected_paths[2][4096];memcpy(expected_paths,request_probe_paths,sizeof(expected_paths));int probes=request_probe_count;
        request_probe_count=0;
        TEST_ASSERT(espeak_rs_voice_request(root,name,control,PATHSEP,N_PATH_BUF,NULL,request_probe,&actual)==expected_status);
        TEST_ASSERT(request_probe_count==probes);
        for(int i=0;i<probes;i++)TEST_ASSERT(strcmp(request_probe_paths[i],expected_paths[i])==0);
        if(!expected_status){TEST_ASSERT(strcmp((char*)reference.path,(char*)actual.path)==0);TEST_ASSERT(strcmp((char*)reference.name,(char*)actual.name)==0);TEST_ASSERT(reference.control==actual.control);}
        request_comparisons++;
        if(!expected_status)for(int opened=0;opened<2;opened++)for(int found=0;found<2;found++){
            char expected_name[40]={0};unsigned char actual_name[40]={0};fallback_table_calls=0;fallback_table_found=found;
            int status=FallbackReference(&reference,opened,expected_name)==NULL?1:0;
            TEST_ASSERT(espeak_rs_voice_fallback(&actual,opened,found,ESPEAKNG_DEFAULT_VOICE,&actual_name)==status);
            TEST_ASSERT(fallback_table_calls==(!opened&&!(control&3)?1:0));
            if(!status)TEST_ASSERT(strcmp(expected_name,(char*)actual_name)==0);fallback_comparisons++;
        }
        char expected_id[40]={0};unsigned char actual_id[40]={0};char variant[80];
        strncpy0(expected_id,name,40);memcpy(actual_id,expected_id,40);
        snprintf(variant,sizeof(variant),"!v/%.*s",trial%60,"abcdefghijklmnopqrstuvwxyzabcdefghijklmnopqrstuvwxyzabcdefgh");
        IdentifierReference(expected_id,variant,1);
        TEST_ASSERT(espeak_rs_voice_identifier(&actual_id,variant,1)==0);TEST_ASSERT(strcmp(expected_id,(char*)actual_id)==0);identifier_comparisons++;
        IdentifierReference(expected_id,name,0);TEST_ASSERT(espeak_rs_voice_identifier(&actual_id,name,0)==0);TEST_ASSERT(memcmp(expected_id,actual_id,40)==0);identifier_comparisons++;
        // An aliased request snapshots the old bytes before output mutation.
        TEST_ASSERT(espeak_rs_voice_identifier(&actual_id,(char*)actual_id,0)==0);TEST_ASSERT(memcmp(expected_id,actual_id,40)==0);identifier_comparisons++;
    }
    RustVoiceRequest unchanged={.control=0x12345678},snapshot=unchanged;request_probe_count=0;
    TEST_ASSERT(espeak_rs_voice_request("root",NULL,0,PATHSEP,N_PATH_BUF,NULL,request_probe,&unchanged)==1);
    TEST_ASSERT(memcmp(&unchanged,&snapshot,sizeof(snapshot))==0&&request_probe_count==0);
    TEST_ASSERT(espeak_rs_voice_request("root","filename",16,PATHSEP,4,NULL,request_probe,&unchanged)==2);
    TEST_ASSERT(memcmp(&unchanged,&snapshot,sizeof(snapshot))==0&&request_probe_count==0);
    unsigned char id[40]="en+m1",before[40];memcpy(before,id,40);
    TEST_ASSERT(espeak_rs_voice_identifier(&id,"m2",1)==2);TEST_ASSERT(memcmp(id,before,40)==0);
}
static uint32_t catalog_directory(void *,const unsigned char *,size_t);
static void storage_pair(const char *root)
{
    n_voices_list=0;memset(voices_list,0,sizeof(voices_list));
    for(int language=0;language<2;language++){
        char path[N_PATH_BUF];snprintf(path,sizeof(path),"%s/%s",root,language?"lang":"voices");StorageDiscovery(path,strlen(path)+1,language);
    }
    voices_list[n_voices_list]=NULL;
    qsort(voices_list,n_voices_list,sizeof(*voices_list),ReferenceNameSorter);
    espeak_VOICE *native[500]={0};int count=-1;
    void *owner=espeak_rs_voice_catalog_create(root,native,500,&count,NULL,NULL);TEST_ASSERT(owner!=NULL);
    TEST_ASSERT(count==n_voices_list);TEST_ASSERT(native[count]==NULL);
    TEST_ASSERT(espeak_rs_voice_order(native,count)==0);
    for(int i=0;i<count;i++){
        espeak_VOICE *a=voices_list[i],*b=native[i];
        TEST_ASSERT(strcmp(a->identifier,b->identifier)==0);TEST_ASSERT(strcmp(a->name,b->name)==0);
        const char *p=a->languages;while(*p){p++;p+=strlen(p)+1;}size_t size=p-a->languages+1;
        TEST_ASSERT(memcmp(a->languages,b->languages,size)==0);
        TEST_ASSERT(a->gender==b->gender&&a->age==b->age&&a->xx1==b->xx1&&b->score==0);
        a->score=b->score=123;storage_comparisons++;
    }
    espeak_VOICE **result=espeak_rs_voice_catalog_list(owner,NULL,PATHSEP,NULL,catalog_directory);TEST_ASSERT(result!=NULL);
    espeak_VOICE **address=result;int position=0;
    for(int i=0;i<count;i++){
        espeak_VOICE *record=voices_list[i];
        if(record->languages[0]&&strcmp(record->languages+1,"variant")&&memcmp(record->identifier,"mb/",3)){
            TEST_ASSERT(result[position]==native[i]);position++;
        }
    }
    TEST_ASSERT(result[position]==NULL);list_comparisons++;
    const char *languages[]={"en","de","all","variants","mbrola","mb","en-gb","ru","ps","unknown"};
    for(int repeat=0;repeat<25;repeat++)for(size_t query=0;query<sizeof(languages)/sizeof(languages[0]);query++){
        espeak_VOICE selector={.languages=languages[query],.gender=repeat%3,.age=repeat%2?65:0};
        espeak_VOICE *expected[500]={0};int selected=ReferenceRanks(&selector,expected,1);
        result=espeak_rs_voice_catalog_list(owner,&selector,PATHSEP,NULL,catalog_directory);TEST_ASSERT(result==address);
        unsigned char matched[500]={0};
        for(int i=0;i<selected;i++){
            TEST_ASSERT(result[i]!=NULL);
            TEST_ASSERT(result[i]->score==expected[i]->score&&strcmp(result[i]->name,expected[i]->name)==0);
            // C qsort does not specify order when name and score both tie.
            // Require identical record membership within each indistinguishable
            // group, and exact preference order between distinct groups.
            int match=-1;
            for(int j=0;j<selected;j++)if(!matched[j]&&result[i]->score==expected[j]->score&&!strcmp(result[i]->name,expected[j]->name)&&!strcmp(result[i]->identifier,expected[j]->identifier)){match=j;break;}
            TEST_ASSERT(match>=0);matched[match]=1;
        }
        TEST_ASSERT(result[selected]==NULL);
        for(int i=0;i<count;i++)TEST_ASSERT(native[i]->score==voices_list[i]->score);
        list_comparisons++;
    }
    TEST_ASSERT(espeak_rs_voice_catalog_workspace(owner)!=NULL);
    espeak_rs_voice_catalog_destroy(owner);
    for(int i=0;i<n_voices_list;i++)free(voices_list[i]);n_voices_list=0;memset(voices_list,0,sizeof(voices_list));
}
static void generated_storage(void)
{
    storage_pair(path_home);storage_pair(ESPEAK_VOICE_SOURCE_DIR);
    char temporary[]="/tmp/espeak-catalogue-XXXXXX";TEST_ASSERT(mkdtemp(temporary)!=NULL);
    char path[N_PATH_BUF];snprintf(path,sizeof(path),"%s/voices",temporary);TEST_ASSERT(mkdir(path,0700)==0);
    for(int i=0;i<510;i++){
        snprintf(path,sizeof(path),"%s/voices/voice%d",temporary,i);FILE *f=fopen(path,"w");TEST_ASSERT(f!=NULL);fprintf(f,"name voice%d\nlanguage en\n",i);
        if(i==0)for(int j=0;j<1000;j++)fprintf(f,"# a deliberately long configuration file with comments across the read boundaries\n");
        fclose(f);
    }
    storage_pair(temporary);
    for(int i=0;i<510;i++){snprintf(path,sizeof(path),"%s/voices/voice%d",temporary,i);TEST_ASSERT(unlink(path)==0);}
    snprintf(path,sizeof(path),"%s/voices",temporary);TEST_ASSERT(rmdir(path)==0);TEST_ASSERT(rmdir(temporary)==0);
    espeak_VOICE *unchanged[500]={0};unchanged[0]=(espeak_VOICE *)(uintptr_t)1;int count=777;
    TEST_ASSERT(espeak_rs_voice_catalog_create(NULL,unchanged,500,&count,NULL,NULL)==NULL);TEST_ASSERT(count==777&&unchanged[0]==(espeak_VOICE *)(uintptr_t)1);
    TEST_ASSERT(espeak_rs_voice_catalog_create(path_home,unchanged,498,&count,NULL,NULL)==NULL);TEST_ASSERT(count==777&&unchanged[0]==(espeak_VOICE *)(uintptr_t)1);
}
static PHONEME_TAB *backend_table[256];
static int backend_table_count;
#define phoneme_tab backend_table
#define n_phoneme_tab backend_table_count
#define PhonemeCode BackendCode
#define LookupPhonemeString BackendLookup
#include "voice_mnemonic_reference.inc"
#undef phoneme_tab
#undef n_phoneme_tab
#undef PhonemeCode
#undef LookupPhonemeString
static REPLACE_PHONEMES backend_replacements[60];
static int backend_replacement_count;
#define replace_phonemes backend_replacements
#define n_replace_phonemes backend_replacement_count
#define LookupPhonemeString BackendLookup
#define PhonemeReplacement BackendReplacement
#include "voice_replacement_reference.inc"
#undef replace_phonemes
#undef n_replace_phonemes
#undef LookupPhonemeString
#undef PhonemeReplacement
static RustMbrolaRequest backend_mbrola(char *p)
{
    char name1[40],name2[80];
#include "voice_mbrola_reference.inc"
    RustMbrolaRequest result={.sample_rate=srate};strcpy((char*)result.voice,name1);strcpy((char*)result.table,name2);return result;
}
static void backend_pair(const char *keyword,char *value)
{
    if(!strcmp(keyword,"replace")){
        REPLACE_PHONEMES actual[60];memcpy(actual,backend_replacements,sizeof(actual));int count=backend_replacement_count;
        BackendReplacement(value);
        TEST_ASSERT(espeak_rs_voice_replacement(value,(const PHONEME_TAB *const *)backend_table,backend_table_count,actual,&count)==0);
        TEST_ASSERT(count==backend_replacement_count);TEST_ASSERT(memcmp(actual,backend_replacements,sizeof(actual))==0);replacement_comparisons++;
    }else if(!strcmp(keyword,"mbrola")){
        RustMbrolaRequest expected=backend_mbrola(value),actual={0};
        TEST_ASSERT(espeak_rs_mbrola_request(value,&actual)==0);
        TEST_ASSERT(strcmp((char*)actual.voice,(char*)expected.voice)==0);TEST_ASSERT(strcmp((char*)actual.table,(char*)expected.table)==0);
        TEST_ASSERT(actual.sample_rate==expected.sample_rate);mbrola_comparisons++;
    }
}
static void generated_backends(void)
{
    PHONEME_TAB records[256];
    for(int trial=0;trial<20000;trial++){
        backend_table_count=random32()%257;
        for(int i=0;i<256;i++){
            records[i]=(PHONEME_TAB){.mnemonic=random32(),.code=(unsigned char)random32()};
            if(i>0&&i%5==0)records[i].mnemonic=records[i-1].mnemonic;
            backend_table[i]=random32()%4?&records[i]:NULL;
        }
        uint32_t word=random32();if(trial%2&&backend_table_count)word=records[random32()%backend_table_count].mnemonic;
        TEST_ASSERT(BackendCode(word)==espeak_rs_phoneme_code((const PHONEME_TAB *const *)backend_table,backend_table_count,word));mnemonic_comparisons++;
        char name[12];for(int i=0;i<11;i++)name[i]=(char)(1+random32()%255);name[trial%12]=0;
        TEST_ASSERT(BackendLookup(name)==espeak_rs_phoneme_code((const PHONEME_TAB *const *)backend_table,backend_table_count,espeak_rs_phoneme_mnemonic(name)));mnemonic_comparisons++;
    }
    records[0]=(PHONEME_TAB){.mnemonic='a',.code=10};records[1]=(PHONEME_TAB){.mnemonic='b',.code=11};
    records[2]=(PHONEME_TAB){.mnemonic=0x4c4c554e,.code=12}; // literal NULL can be a defined phoneme
    backend_table[0]=&records[0];backend_table[1]=&records[1];backend_table[2]=&records[2];backend_table_count=3;
    const char *cases[]={""," ","+","0","1 a b","2a b","-1 a","257 b NULL","3 missing b","0 a unknown","0 a b ignored","0x12 a b","1 a\0ignored"};
    for(int trial=0;trial<20000;trial++){
        memset(backend_replacements,0,sizeof(backend_replacements));backend_replacement_count=trial%61;
        for(size_t i=0;i<sizeof(cases)/sizeof(cases[0]);i++)backend_pair("replace",(char*)cases[i]);
        char input[180];snprintf(input,sizeof(input),"en1 en1_phtrans %d%s",(int)random32(),trial%3?"suffix":"");backend_pair("mbrola",input);
    }
    backend_pair("mbrola","en1");backend_pair("mbrola","en1 table invalid");
    REPLACE_PHONEMES unchanged[60]={0},snapshot[60]={0};int used=0;
    TEST_ASSERT(espeak_rs_voice_replacement("999999999999 a b",(const PHONEME_TAB *const *)backend_table,3,unchanged,&used)==2);
    TEST_ASSERT(espeak_rs_voice_replacement("0 overlongtoken b",(const PHONEME_TAB *const *)backend_table,3,unchanged,&used)==2);
    TEST_ASSERT(used==0&&memcmp(unchanged,snapshot,sizeof(snapshot))==0);
    used=61;TEST_ASSERT(espeak_rs_voice_replacement("0 a b",(const PHONEME_TAB *const *)backend_table,3,unchanged,&used)==2);
    TEST_ASSERT(used==61&&memcmp(unchanged,snapshot,sizeof(snapshot))==0);
    RustMbrolaRequest original={.sample_rate=123},invalid=original;
    TEST_ASSERT(espeak_rs_mbrola_request("",&invalid)==2);TEST_ASSERT(memcmp(&invalid,&original,sizeof(original))==0);
    TEST_ASSERT(espeak_rs_mbrola_request("en1 table 999999999999",&invalid)==2);TEST_ASSERT(memcmp(&invalid,&original,sizeof(original))==0);
    for(int table=0;table<N_PHONEME_TABS&&phoneme_tab_list[table].name[0];table++){
        SelectPhonemeTable(table);memcpy(backend_table,phoneme_tab,sizeof(backend_table));backend_table_count=n_phoneme_tab;
        for(int i=0;i<256;i++){
            uint32_t word=phoneme_tab[i]?phoneme_tab[i]->mnemonic:random32();
            TEST_ASSERT(BackendCode(word)==espeak_rs_phoneme_code((const PHONEME_TAB *const *)backend_table,backend_table_count,word));mnemonic_comparisons++;
            if(phoneme_tab[i]){
                char name[6]={0};for(int j=0;j<4;j++)name[j]=(char)(word>>(j*8));
                TEST_ASSERT(BackendLookup(name)==LookupPhonemeString(name));mnemonic_comparisons++;
            }
        }
    }
    TEST_ASSERT(espeak_SetVoiceByName("en")==EE_OK);
    memcpy(backend_table,phoneme_tab,sizeof(backend_table));backend_table_count=n_phoneme_tab;
    backend_replacement_count=0;memset(backend_replacements,0,sizeof(backend_replacements));
}
static void setup_pair(RustVoiceSetup *state,const char *key,char *value)
{
    if(strcmp(key,"language")&&strcmp(key,"name")&&strcmp(key,"gender")&&strcmp(key,"dictionary")&&strcmp(key,"phonemes")&&strcmp(key,"maintainer")&&strcmp(key,"status")&&strcmp(key,"replace"))return;
    RustVoiceSetup expected=*state;setup_reference(&expected,key,value);
    uint32_t effect=99;TEST_ASSERT(espeak_rs_voice_setup_attribute(state,key,value,&effect)==0);
    TEST_ASSERT(effect==(setup_language_calls?1:setup_table_calls?2:0));
    if(strcmp((char*)expected.translator,(char*)state->translator))fprintf(stderr,"Setup mismatch key=%s value=%s translator=%s/%s\n",key,value,expected.translator,state->translator);
    TEST_ASSERT(strcmp((char*)expected.translator,(char*)state->translator)==0);TEST_ASSERT(strcmp((char*)expected.dictionary,(char*)state->dictionary)==0);
    TEST_ASSERT(strcmp((char*)expected.phonemes,(char*)state->phonemes)==0);TEST_ASSERT(strcmp((char*)expected.name,(char*)state->name)==0);
    TEST_ASSERT(strcmp((char*)expected.language,(char*)state->language)==0);
    if(expected.language_length!=state->language_length)fprintf(stderr,"Setup language bound mismatch key=%s value=%s tone=%u used=%u/%u set=%u/%u\n",key,value,state->tone_only,expected.language_length,state->language_length,expected.language_set,state->language_set);
    TEST_ASSERT(expected.language_length==state->language_length);
    TEST_ASSERT(memcmp(expected.languages,state->languages,state->language_length+1)==0);
    TEST_ASSERT(expected.phonemes_set==state->phonemes_set);
    TEST_ASSERT(expected.language_set==state->language_set&&expected.gender==state->gender&&expected.age==state->age);setup_comparisons++;
}
static void generated_setup(void)
{
    for(int trial=0;trial<20000;trial++){
        RustVoiceSetup state={.tone_only=trial%2,.gender=random32()%4,.age=random32()%256};strcpy((char*)state.translator,"en");strcpy((char*)state.dictionary,"en");
        char text[120];snprintf(text,sizeof(text),"%s %d",(const char*[]){"en-gb","--en-gb","de","variant"}[trial%4],(int)(random32()%401)-100);
        setup_pair(&state,"language",text);
        strcpy(text,"1 a b");setup_pair(&state,"replace",text);
        snprintf(text,sizeof(text),"%s %d",trial%3==0?"female":"male",(int)(random32()%401)-100);setup_pair(&state,"gender",text);
        strcpy(text,"custom");setup_pair(&state,"dictionary",text);setup_pair(&state,"phonemes",text);
        strcpy(text,"2 a b");setup_pair(&state,"replace",text);
        strcpy(text,"Name with spaces");setup_pair(&state,"name",text);
        strcpy(text,"de invalid");setup_pair(&state,"language",text);
        strcpy(text,"female invalid");setup_pair(&state,"gender",text);
        strcpy(text,"");setup_pair(&state,"dictionary",text);setup_pair(&state,"phonemes",text);
    }
}
static uint32_t catalog_directory(void *opaque,const unsigned char *name,size_t length)
{
    (void)opaque;char path[N_PATH_BUF];snprintf(path,sizeof(path),"%s/voices/%.*s",path_home,(int)length,(const char*)name);
    return GetFileLength(path)==-EISDIR;
}
static void generated_catalog(void)
{
    void *workspace=espeak_rs_voice_workspace_create(499);TEST_ASSERT(workspace!=NULL);
    for(int trial=0;trial<20000;trial++){
        espeak_VOICE expected[24]={0},actual[24];char names[24][80],identifiers[24][160],languages[24][100];
        espeak_VOICE *roster[25]={0},*ranked[500]={0},*expected_ranked[500]={0};
        n_voices_list=1+random32()%24;
        for(int i=0;i<n_voices_list;i++){
            snprintf(names[i],80,i==0?"English":"Voice%03d",i);
            memset(identifiers[i],'~',sizeof(identifiers[i]));
            if(i==0)strcpy(identifiers[i]+80,"en");else snprintf(identifiers[i]+80,80,"%s/v%03d",i%5==0?"mb":"lang",i);
            const char *language=(const char*[]){"en","en-gb","de","fr","variants"}[i==0?0:random32()%5];
            languages[i][0]=1+random32()%127;strcpy(languages[i]+1,language);languages[i][strlen(language)+2]=0;
            expected[i]=(espeak_VOICE){.name=names[i],.identifier=identifiers[i]+80,.languages=languages[i],.gender=random32()%4,
                .age=3+random32()%98,.xx1=random32()%20,.score=random32()%1000};
            if(trial%7==0)expected[i].xx1=0;
            if(trial%5==0)expected[i].age=0;
            actual[i]=expected[i];voices_list[i]=&expected[i];roster[i]=&actual[i];
        }
        voices_list[n_voices_list]=NULL;
        qsort(voices_list,n_voices_list,sizeof(espeak_VOICE*),ReferenceNameSorter);
        TEST_ASSERT(espeak_rs_voice_order(roster,n_voices_list)==0);
        for(int i=0;i<n_voices_list;i++)TEST_ASSERT(strcmp(voices_list[i]->identifier,roster[i]->identifier)==0);
        espeak_VOICE spec={.languages=(const char*[]){"en","en-gb","DE","all","variants","missing","mbrola",NULL}[trial%8],
            .name=(const char*[]){NULL,"English","Voice003","English+f3","missing"}[trial%5],.gender=random32()%4,
            .age=3+random32()%98,.variant=random32()%256,.identifier=trial%9==0?"mb/":NULL};
        if(trial%13==0){spec.age=0;spec.gender=0;spec.variant=0;spec.languages=NULL;}
        if(trial%7==0)spec.age=0;
        unsigned include=trial%2;
        unsigned char normalized[80];int32_t parts;
        TEST_ASSERT(espeak_rs_voice_filter(spec.languages,include,PATHSEP,&normalized,&parts)==0);
        unsigned directory=include&&parts==1?catalog_directory(NULL,normalized,strlen((char*)normalized)):0;
        int count=ReferenceRanks(&spec,expected_ranked,include);
        int got=espeak_rs_voice_rank(workspace,&spec,roster,ranked,500,include,directory,PATHSEP);
        TEST_ASSERT(count==got);
        for(int i=0;i<count;i++){TEST_ASSERT(strcmp(expected_ranked[i]->identifier,ranked[i]->identifier)==0);TEST_ASSERT(expected_ranked[i]->score==ranked[i]->score);}
        ranking_comparisons++;
        int found;const char *chosen=ReferenceSelection(&spec,&found);
        RustVoiceSelection result;int status=espeak_rs_voice_select(workspace,&spec,roster,PATHSEP,NULL,catalog_directory,&result);
        TEST_ASSERT(status<2);TEST_ASSERT((chosen==NULL)==(status==1));TEST_ASSERT(found==(int)result.found);
        if(chosen){char path[100];if(result.suffix[0])snprintf(path,sizeof(path),"%s+%s",roster[result.index]->identifier,result.suffix);
            else snprintf(path,sizeof(path),"%s",roster[result.index]->identifier);
            if(strcmp(chosen,path)!=0)fprintf(stderr,"Selection mismatch trial=%d chosen=%s got=%s lang=%s gender=%u age=%u variant=%u\n",trial,chosen,path,spec.languages?spec.languages:"NULL",spec.gender,spec.age,spec.variant);
            TEST_ASSERT(strcmp(chosen,path)==0);}
        for(int i=0;i<n_voices_list;i++)TEST_ASSERT(expected[i].score==actual[i].score);
        catalog_comparisons++;
    }
    n_voices_list=0;memset(voices_list,0,sizeof(voices_list));espeak_rs_voice_workspace_destroy(workspace);
}
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
static unsigned long stream_comparisons,stream_files;
static void stream_pair(FILE *file,const char *path,size_t width)
{
    void *reader=espeak_rs_voice_file_open(path,width);TEST_ASSERT(reader!=NULL);
    const char *key=NULL,*value=NULL,*first=NULL;
    char line[4096];
    while(fgets_strip(line,width,file)){
        char *p=line;while(*p&&!isspace((unsigned char)*p))p++;
        if(*p)*p++=0;
        if(!line[0])continue;
        TEST_ASSERT(espeak_rs_voice_file_next(reader,&key,&value)==0);
        TEST_ASSERT(strcmp(key,line)==0&&strcmp(value,p)==0);
        if(first==NULL)first=key;else TEST_ASSERT(key==first);
        stream_comparisons++;
    }
    const char *old_key=key,*old_value=value;
    TEST_ASSERT(espeak_rs_voice_file_next(reader,&key,&value)==1);
    TEST_ASSERT(key==old_key&&value==old_value);
    espeak_rs_voice_file_close(reader);stream_files++;
}
static void generated_streams(void)
{
    char path[]="/tmp/espeak-rust-stream-XXXXXX";
    int descriptor=mkstemp(path);TEST_ASSERT(descriptor>=0);
    FILE *file=fdopen(descriptor,"w+b");TEST_ASSERT(file!=NULL);
    const unsigned char values[]="abcxyz123 \t\v\f\r\n/#\0\xc3\xa9";
    uint32_t state=83;
    for(unsigned i=0;i<100000;i++){
        state=state*1664525+1013904223;
        TEST_ASSERT(fputc(values[(state>>16)%(sizeof(values)-1)],file)!=EOF);
    }
    TEST_ASSERT(fwrite("\nname final",1,11,file)==11);TEST_ASSERT(fflush(file)==0);
    size_t widths[]={2,5,120,260,1024,4096};
    for(size_t i=0;i<sizeof(widths)/sizeof(widths[0]);i++){rewind(file);stream_pair(file,path,widths[i]);}
    TEST_ASSERT(espeak_rs_voice_file_open(path,1)==NULL);
    TEST_ASSERT(espeak_rs_voice_file_open(path,4097)==NULL);
    fclose(file);TEST_ASSERT(unlink(path)==0);
    TEST_ASSERT(espeak_rs_voice_file_open(path,4096)==NULL);
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
        stream_pair(file,path,N_PATH_BUF);rewind(file);
        metadata_pair(file,path);rewind(file);
		voice_t actual={0}; memset(&expected,0,sizeof(expected)); reset_pair(&actual);
		int fast=reference_speed.fast_settings; char line[N_PATH_BUF];
		Translator reference={0};reference_tone_flags=0;
        RustVoiceSetup setup={0};strcpy((char*)setup.translator,"en");strcpy((char*)setup.dictionary,"en");
        SelectPhonemeTableName("en");backend_replacement_count=0;memset(backend_replacements,0,sizeof(backend_replacements));
		while (fgets_strip(line,sizeof(line),file)) {
			char *p=line; while (*p && !isspace((unsigned char)*p)) p++;
			if (*p) *p++=0; if (line[0]) {
                unsigned language_set=setup.language_set,phonemes_set=setup.phonemes_set;
                setup_pair(&setup,line,p);
                if((setup.language_set!=language_set)||(setup.phonemes_set!=phonemes_set))SelectPhonemeTableName((char*)setup.phonemes);
                memcpy(backend_table,phoneme_tab,sizeof(backend_table));backend_table_count=n_phoneme_tab;
                backend_pair(line,p);
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
    generated_catalog();
    generated_setup();
    generated_backends();
    generated_storage();
    generated_requests();
    generated_directives();
    generated_streams();
	char path[N_PATH_BUF]; snprintf(path,sizeof(path),"%s/lang",path_home);voice_files(path);
	snprintf(path,sizeof(path),"%s/voices",path_home);voice_files(path);
	unsigned long built_files = files;
	voice_files(ESPEAK_VOICE_SOURCE_DIR "/lang");
	voice_files(ESPEAK_VOICE_SOURCE_DIR "/voices");
	printf("Covered %lu built and %lu source voice/language files (MBROLA backend not executed)\n",built_files,files-built_files);
	printf("Compared %lu acoustic attributes, %lu defaults, %lu real voice/language files and %lu scanner cases\n",comparisons,resets,files,scans);
	printf("Compared %lu native language-option snapshots including tunes and all parameter keys\n",language_comparisons);
    printf("Compared %lu native metadata records, %lu voice scores, %lu bounded name selections and %lu variants\n",metadata_comparisons,score_comparisons,name_comparisons,variant_comparisons);
    printf("Compared %lu full native catalogue selections and %lu candidate rankings\n",catalog_comparisons,ranking_comparisons);
    printf("Compared %lu ordered native active-voice setup snapshots\n",setup_comparisons);
    printf("Compared %lu phoneme mnemonic lookups, %lu replacement snapshots and %lu MBROLA requests\n",mnemonic_comparisons,replacement_comparisons,mbrola_comparisons);
    printf("Compared %lu natively owned catalogue records including capacity/discovery boundaries\n",storage_comparisons);
    printf("Compared %lu native owned catalogue lists with persistent scores and result storage\n",list_comparisons);
    printf("Compared %lu native voice request paths, %lu fallbacks and %lu current identifiers\n",request_comparisons,fallback_comparisons,identifier_comparisons);
    printf("Compared %lu native ordered directive actions and snapshot effects\n",directive_comparisons);
    printf("Compared %lu native streamed directives across %lu files/widths with reused storage\n",stream_comparisons,stream_files);
	espeak_Terminate();return 0;
}
