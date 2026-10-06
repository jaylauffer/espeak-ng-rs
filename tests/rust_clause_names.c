/* Character/special lookup against independently extracted retained C.
 * SPDX-License-Identifier: GPL-3.0-or-later */
#include "config.h"
#include "test_assert.h"
#include <stdint.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include "common.h"
#include "translate.h"
#include "rust_data.h"
static Translator primary,secondary,*alternate=&secondary;
static voice_t original_voice,*active_voice=&original_voice;
static unsigned trace,seed=0x3917ca85u;
static int query_index,query_found[4],query_code[4],rule_code,rule_language,active_table;
static unsigned char *observed_output;
static unsigned next(void){seed^=seed<<13;seed^=seed>>17;seed^=seed<<5;return seed;}
static void Trace(int kind,const char *text,Translator *tr,unsigned int *flags)
{
 trace=trace*33+(unsigned)kind;trace=trace*33+(unsigned)tr->translator_name;
 trace=trace*33+(unsigned)active_table;trace=trace*33+observed_output[0];
 if(flags){trace=trace*33+flags[0];trace=trace*33+flags[1];}
 if(text)for(const unsigned char *p=(const unsigned char *)text;*p;p++)trace=trace*33+*p;
 trace=trace*33+7;
}
static int Dictionary(Translator *tr,char **word,char *phonemes,unsigned int *flags,int ending,WORD_TAB *wtab,int remaining)
{
 TEST_ASSERT(ending==0 && wtab==NULL && remaining==0 && query_index<4);
 Trace(tr==&secondary?2:1,*word,tr,flags);
 phonemes[0]=(char)query_code[query_index];phonemes[1]=0;
 flags[0]=flags[0]*3+1;flags[1]=flags[1]*3+2;
 return query_found[query_index++];
}
static void Rules(Translator *tr,char *word,char *phonemes,int capacity,void *unused,int control,void *wtab)
{
 TEST_ASSERT(capacity==60 && unused==NULL && control==0 && wtab==NULL && word[-1]==' ');
 Trace(3,word,tr,NULL);phonemes[0]=(char)rule_code;phonemes[1]=0;
 if(rule_language)tr->translator_name=0x656e;
}
static int Setup(const char *language)
{
 TEST_ASSERT(strcmp(language,ESPEAKNG_DEFAULT_VOICE)==0);Trace(4,language,&primary,NULL);active_table=29;return 29;
}
static int Restore(int table){Trace(5,NULL,&primary,NULL);active_table=table;return table;}
static void Format(char *output,char *phonemes,Translator *tr,Translator *tr2,unsigned int *flags,size_t capacity)
{
 TEST_ASSERT(capacity>=74 || capacity==30);
 Trace(tr2?7:6,phonemes,tr2?tr2:tr,flags);
 flags[0]^=8;flags[1]^=16;
 strcpy(output,tr2?"fallback-name":"native-name");
}
#define LookupDictList Dictionary
#define TranslateRules Rules
#define SetTranslator2 Setup
#define SelectPhonemeTable Restore
#define DecodeWithPhonemeMode Format
#define translator2 alternate
#define voice active_voice
#define LookupSpecial ReferenceSpecial
#define LookupCharName ReferenceName
#include "clause_names_reference.inc"
#undef LookupDictList
#undef TranslateRules
#undef SetTranslator2
#undef SelectPhonemeTable
#undef DecodeWithPhonemeMode
#undef translator2
#undef voice
#undef LookupSpecial
#undef LookupCharName
static int32_t Query(void *owner,RustCharacterCommand *command)
{
 Translator *tr=owner;char phonemes[200]={0};memcpy(phonemes,command->data.phonemes,60);
 char *word=(char *)command->data.word+command->data.start;
 switch(command->kind) {
 case 1:command->found=Dictionary(command->secondary?&secondary:tr,&word,phonemes,command->data.flags,0,NULL,0);break;
 case 2:Rules(tr,word,phonemes,60,NULL,0,NULL);break;
 case 3:Setup(ESPEAKNG_DEFAULT_VOICE);return 0;
 case 4:Format((char *)command->text,phonemes,tr,command->secondary?&secondary:NULL,command->data.flags,74);break;
 case 5:Restore(original_voice.phoneme_tab_ix);return 0;
 default:return 2;
 }
 memcpy(command->data.phonemes,phonemes,strlen(phonemes)+1);return 0;
}
int main(void)
{
 size_t character_cases=0,special_cases=0;
 for(int i=0;i<200000;i++) {
  primary.translator_name=(int[]){0x656e,0x6672,0x6869}[next()%3];secondary.translator_name=0x656e;
  int original_language=primary.translator_name;original_voice.phoneme_tab_ix=(int)(next()%20);
  for(int j=0;j<4;j++){query_found[j]=(int)(next()%2);query_code[j]=(int[]){0,21,12,6}[next()%4];}
  rule_code=(int[]){0,21,12,6}[next()%4];rule_language=(int)(next()%2);
  int code=(int)(next()%0x120000),only=(int)(next()%2);
  unsigned char expected[74],actual[74];memset(expected,0xa5,74);memset(actual,0xa5,74);
  observed_output=expected;trace=0;query_index=0;active_table=original_voice.phoneme_tab_ix;
  TEST_ASSERT(ReferenceName((char *)expected,&primary,code,only)==(char *)expected);
  unsigned expected_trace=trace;int end_table=active_table,end_language=primary.translator_name,end_queries=query_index;
  primary.translator_name=original_language;observed_output=actual;trace=0;query_index=0;active_table=original_voice.phoneme_tab_ix;
  RustCharacterContext context={.owner=&primary,.query=Query,.language=&primary.translator_name};
  int got=espeak_rs_clause_character_name(&context,code,only,actual,74);
  if(got!=(int)strlen((char *)expected) || trace!=expected_trace)
   fprintf(stderr,"character mismatch i=%d code=%x only=%d traces=%u/%u result=%d\n",i,code,only,trace,expected_trace,got);
  TEST_ASSERT(got==(int)strlen((char *)expected) && memcmp(actual,expected,74)==0);
  TEST_ASSERT(trace==expected_trace && active_table==end_table && primary.translator_name==end_language && query_index==end_queries);character_cases++;
  memset(expected,0xa5,74);memset(actual,0xa5,74);primary.translator_name=original_language;
  observed_output=expected;trace=0;query_index=0;active_table=original_voice.phoneme_tab_ix;
  const char *name=i%2?"_cap":"_.p";const char *result=ReferenceSpecial(&primary,name,(char *)expected,74);
  expected_trace=trace;end_queries=query_index;
  observed_output=actual;trace=0;query_index=0;active_table=original_voice.phoneme_tab_ix;
  got=espeak_rs_clause_special(&context,name,actual,74);
  TEST_ASSERT(got==(result?(int)strlen(result):-1));TEST_ASSERT(memcmp(actual,expected,74)==0);
  TEST_ASSERT(trace==expected_trace && query_index==end_queries);special_cases++;
 }
 unsigned char output[74];memset(output,0xa5,74);query_found[0]=1;query_code[0]=12;query_index=0;observed_output=output;trace=0;
 RustCharacterContext context={.owner=&primary,.query=Query,.language=&primary.translator_name};
 TEST_ASSERT(espeak_rs_clause_special(&context,"_cap",output,4)==-2);
 for(int i=0;i<74;i++)TEST_ASSERT(output[i]==0xa5);
 TEST_ASSERT(espeak_rs_clause_character_name(&context,65,2,output,74)==-2);
 for(int i=0;i<74;i++)TEST_ASSERT(output[i]==0xa5);
 printf("Matched %zu character-name and %zu special-name output/tail/backend-order/fallback-table comparisons\n",character_cases,special_cases);
 return 0;
}
