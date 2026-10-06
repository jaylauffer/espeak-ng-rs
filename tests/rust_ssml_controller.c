/* Owned SSML controller against the independently extracted retained C.
 * SPDX-License-Identifier: GPL-3.0-or-later */
#include "config.h"
#include "test_assert.h"
#include <ctype.h>
#include <limits.h>
#include <locale.h>
#include <math.h>
#include <stdint.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <wchar.h>
#include <wctype.h>
#include <ucd/ucd.h>
#include "common.h"
#include "mnemonics.h"
#include "ssml.h"
#include "translate.h"
#include "rust_data.h"
#include "speech.h"
typedef struct {
 PARAM_STACK parameters[20]; SSML_STACK voices[20]; int current[15];
 int parameter_count,voice_count,punctuation,capitals,mode,start,offset;
 bool audio,ignore,clear; char current_voice[40],skip[50],output[512];
 unsigned char previous[40];
} State;
static State state;
static Translator tr;
static Translator *owner_translator=&tr;
static SPEED_FACTORS factors;
static unsigned trace,seed=0x9e71381bu;
static int resource_index,uri_result,mutate;
static char names[512];
static const char *selected;
static RustSsmlVoiceChoice choice;
static int (*callback)(int,const char *,const char *);
static unsigned next(void){seed^=seed<<13;seed^=seed>>17;seed^=seed<<5;return seed;}
static void Trace(int kind,const char *text)
{
 trace=trace*33+(unsigned)kind;
 if(text)for(const unsigned char *p=(const unsigned char *)text;*p;p++)trace=trace*33+*p;
 trace=trace*33+7;
 trace=trace*33+state.parameter_count;trace=trace*33+state.voice_count;
 trace=trace*33+state.punctuation;trace=trace*33+state.capitals;
 trace=trace*33+state.audio;trace=trace*33+state.ignore;trace=trace*33+state.clear;
 for(int i=0;i<15;i++){trace=trace*33+(unsigned)state.current[i];trace=trace*33+(unsigned)state.parameters[0].parameter[i];}
 for(int i=0;i<state.offset;i++)trace=trace*33+(unsigned char)state.output[i];
}
static espeak_VOICE *SelectName(espeak_VOICE **unused,const char *name)
{
 (void)unused;Trace(1,name);
 static espeak_VOICE en={.identifier="gmw/en"},fr={.identifier="roa/fr"};
 if(strcmp(name,"known-en")==0)return &en;
 if(strcmp(name,"known-fr")==0)return &fr;
 return NULL;
}
static const char *Select(espeak_VOICE *input,int *found)
{
 Trace(2,input->name);Trace(3,input->identifier);Trace(4,input->languages);
 memset(&choice,0,sizeof(choice));strcpy((char *)choice.name,input->name);
 strcpy((char *)choice.identifier,input->identifier);strcpy((char *)choice.language,input->languages);
 choice.gender=input->gender;choice.age=input->age;choice.variant=input->variant;
 trace=trace*33+choice.gender;trace=trace*33+choice.age;trace=trace*33+choice.variant;
 *found=selected!=NULL;return selected;
}
static int Append(const char *name,int wide)
{
 TEST_ASSERT(wide==0);Trace(5,name);if(resource_index>=0)strcpy(names+resource_index,name);return resource_index;
}
static int Load(const char *name){Trace(6,name);return resource_index;}
static int Uri(int kind,const char *name,const char *base)
{
 TEST_ASSERT(kind==1);Trace(7,name);Trace(base?8:9,base);
 if(mutate){state.parameters[0].parameter[espeakRATE]++;state.capitals=2;}
 return uri_result;
}
static espeak_ERROR SetRate(espeak_PARAMETER parameter,int value,int relative)
{
 TEST_ASSERT(parameter==espeakRATE && relative==0);Trace(10,NULL);trace=trace*33+(unsigned)value;
 state.parameters[0].parameter[espeakRATE]=value;
 factors.clause_pause_factor=128;factors.pause_factor=160;return EE_OK;
}
#define translator owner_translator
#define speed factors
#define option_punctuation state.punctuation
#define option_capitals state.capitals
#define skip_marker state.skip
#define namedata names
#define AddNameData Append
#define LoadSoundFile2 Load
#define uri_callback callback
#define espeak_SetParameter SetRate
#define SelectVoiceByName SelectName
#define SelectVoice Select
#define ProcessSsmlTag ReferenceController
#define ParseSsmlReference ReferenceEntity
#include "ssml_reference.inc"
#define param_stack state.parameters
#include "ssml_controller_reference.inc"
#undef translator
#undef speed
#undef option_punctuation
#undef option_capitals
#undef param_stack
#undef skip_marker
#undef namedata
#undef AddNameData
#undef LoadSoundFile2
#undef uri_callback
#undef espeak_SetParameter
#undef SelectVoiceByName
#undef SelectVoice
#undef ProcessSsmlTag
#undef ParseSsmlReference
static int WideSpace(uint32_t c){return iswspace((wint_t)c)!=0;}
static int ByteSpace(uint32_t c){return c<=255 && isspace((unsigned char)c)!=0;}
static int Lower(uint32_t c){return c<=255?tolower((unsigned char)c):0;}
static int32_t Resolve(const unsigned char (*name)[40],unsigned char (*id)[40])
{
 espeak_VOICE *v=SelectName(NULL,(const char *)*name);if(v==NULL)return 1;strcpy((char *)*id,v->identifier);return 0;
}
static int32_t SelectNative(const RustSsmlVoiceChoice *input,unsigned char (*id)[40])
{
 espeak_VOICE v={.name=(const char *)input->name,.identifier=(const char *)input->identifier,.languages=(const char *)input->language,
  .gender=(unsigned char)input->gender,.age=(unsigned char)input->age,.variant=(unsigned char)input->variant};
 int found;const char *text=Select(&v,&found);if(text==NULL)return 1;strcpy((char *)*id,text);return 0;
}
static void RateNative(int32_t value,RustSsmlRate *result)
{
 SetRate(espeakRATE,value,0);result->clause_pause=factors.clause_pause_factor;result->pause=factors.pause_factor;
}
static void Compare(const State *expected,unsigned expected_trace,const RustSsmlVoiceChoice *expected_choice,size_t iteration,const wchar_t *xml)
{
 if(state.offset!=expected->offset || memcmp(state.output,expected->output,512)!=0 || trace!=expected_trace)
  fprintf(stderr,"controller mismatch at %zu tag=%ls offsets=%d/%d traces=%u/%u\n",iteration,xml,state.offset,expected->offset,trace,expected_trace);
 TEST_ASSERT(state.offset==expected->offset);TEST_ASSERT(memcmp(state.output,expected->output,512)==0);TEST_ASSERT(trace==expected_trace);
 TEST_ASSERT(state.parameter_count==expected->parameter_count && state.voice_count==expected->voice_count);
 TEST_ASSERT(state.punctuation==expected->punctuation && state.capitals==expected->capitals);
 TEST_ASSERT(state.mode==expected->mode && state.start==expected->start);
 TEST_ASSERT(state.audio==expected->audio && state.ignore==expected->ignore && state.clear==expected->clear);
 TEST_ASSERT(memcmp(state.parameters,expected->parameters,sizeof(state.parameters))==0);
 TEST_ASSERT(memcmp(state.current,expected->current,sizeof(state.current))==0);
 TEST_ASSERT(memcmp(state.current_voice,expected->current_voice,40)==0 && memcmp(state.skip,expected->skip,50)==0);
 TEST_ASSERT(memcmp(&choice,expected_choice,sizeof(choice))==0);
 /* Legacy copies leave unused voice-name/language tails undefined. Compare
  * initialized fields and terminated prefixes, including installed spare slots. */
 for(int i=0;i<20;i++) {
  const SSML_STACK *a=&state.voices[i],*b=&expected->voices[i];
  TEST_ASSERT(a->tag_type==b->tag_type && a->voice_variant_number==b->voice_variant_number);
  TEST_ASSERT(a->voice_gender==b->voice_gender && a->voice_age==b->voice_age);
  TEST_ASSERT(strcmp(a->voice_name,b->voice_name)==0 && strcmp(a->language,b->language)==0);
 }
}
int main(void)
{
 TEST_ASSERT(setlocale(LC_ALL,"C")!=NULL);
 static const wchar_t *tags[]={
  L"prosody rate='125%' pitch='+2st' volume='soft'",L"/prosody",L"style field='punctuation' mode='all'",L"/style",
  L"emphasis level='strong'",L"/emphasis",L"say-as interpret-as='tts:key'",L"/say-as",L"say-as interpret-as='digits' detail='3'",
  L"sub alias='alias'",L"/sub",L"ignore-text",L"/ignore-text",L"phoneme alphabet='espeak' ph='h@loU'",
  L"mark name='await'",L"mark name='other'",L"mark name=''",L"mark",L"audio src='sample.wav'",L"audio src='/sample.wav'/",L"audio/",L"/audio",
  L"break time='25ms' strength='weak'",L"break time='0.5s'",L"break strength='none'",L"break",L"br",L"/br",
  L"voice name='known-en'",L"voice name='known-fr' age='28' gender='female' variant='3'",L"voice name='missing'",L"/voice",
  L"s xml:lang='en-gb'",L"s",L"/s",L"p xml:lang='fr'",L"p",L"/p",L"speak xml:lang='en' xml:base='base'",L"/speak",
  L"voice/",L"b",L"/b",L"unknown x='1'",L"sub alias='caf\u00e9'",L"phoneme alphabet='espeak' ph='\u03bb'"
 };
 espeak_VOICE base={.name="base",.identifier="gmw/en",.languages="\005en\000\006en-gb\000",.gender=1};
 char variant[40]={0};size_t count=0;
 for(int episode=0;episode<20000;episode++) {
  memset(&state,0,sizeof(state));memset(state.output,0xa5,sizeof(state.output));
  state.parameter_count=1+(int)(next()%19);state.voice_count=1+(int)(next()%19);
  for(int i=0;i<20;i++)for(int j=0;j<15;j++)state.parameters[i].parameter[j]=-1;
  for(int j=0;j<15;j++)state.current[j]=state.parameters[0].parameter[j]=0;
  state.current[1]=state.parameters[0].parameter[1]=175+(int)(next()%500);
  state.current[2]=state.parameters[0].parameter[2]=100;state.current[3]=state.parameters[0].parameter[3]=50;
  state.current[4]=state.parameters[0].parameter[4]=50;state.current[5]=state.parameters[0].parameter[5]=1;
  state.current[10]=state.parameters[0].parameter[10]=100;
  for(int i=1;i<20;i++)state.parameters[i].type=(int[]){3,10,12,11}[next()%4];
  strcpy(state.voices[0].language,"en");
  for(int i=1;i<20;i++)state.voices[i].tag_type=(int[]){1,2,6,7}[next()%4];
  strcpy(state.current_voice,"gmw/en");strcpy(state.skip,"await");state.output[0]=0;
  selected="gmw/en";VoiceFromStack(state.voices,1,&base,variant); /* clear legacy sticky identifier */
  memset(&choice,0,sizeof(choice));strcpy(variant,next()%2?"m2":"");
  for(int step=0;step<12;step++,count++) {
   if(state.offset>300){state.offset=0;state.mode=0;state.start=0;memset(state.output,0xa5,512);state.output[0]=0;}
   const wchar_t *tag=tags[next()%(sizeof(tags)/sizeof(tags[0]))];
   wchar_t xml[501]={0},expected_xml[501]={0};wcscpy(xml,tag);wcscpy(expected_xml,tag);
   State initial=state;RustSsmlVoiceChoice initial_choice=choice;
   selected=(const char *[]) {"gmw/en","roa/fr",NULL,"gmw/en+m1"}[next()%4];
   resource_index=next()%5==0?-1:(int)(next()%100);uri_result=(int)(next()%2);mutate=(int)(next()%2);
   callback=next()%2?Uri:NULL;const char *xmlbase=(const char *[]){NULL,"","/base"}[next()%3];
   tr.langopts.tone_language=(int)(next()%2);factors.clause_pause_factor=128;factors.pause_factor=160;
   trace=0;int want=ReferenceController(expected_xml,state.output,&state.offset,512,xmlbase,&state.audio,state.current_voice,&base,variant,
    &state.ignore,&state.clear,&state.mode,&state.start,state.voices,&state.voice_count,&state.parameter_count,state.current);
   State expected=state;unsigned expected_trace=trace;RustSsmlVoiceChoice expected_choice=choice;
   state=initial;choice=initial_choice;trace=0;factors.clause_pause_factor=128;factors.pause_factor=160;
   RustSsmlContext context={.parameters=state.parameters,.parameter_count=&state.parameter_count,.current=&state.current,
    .voices=state.voices,.voice_count=&state.voice_count,.current_voice=(unsigned char *)state.current_voice,.previous_identifier=&state.previous,
    .skip=(unsigned char *)state.skip,.punctuation=&state.punctuation,.capitals=&state.capitals,.audio=&state.audio,.ignore=&state.ignore,.clear_skipping=&state.clear,
    .sayas_mode=&state.mode,.sayas_start=&state.start,.base_voice=&base,.variant=variant,.xmlbase=xmlbase,
    .wide_space=WideSpace,.byte_space=ByteSpace,.lower=Lower,.append=Append,.load=Load,.uri=callback,.rate=RateNative,.resolve=Resolve,.select=SelectNative,
    .signed_bytes=CHAR_MIN<0,.decimal='.',.tone=tr.langopts.tone_language,.sonic=USE_LIBSONIC!=0};
   int got=espeak_rs_ssml_process(&context,xml,wcslen(xml)+1,(unsigned char *)state.output,512,&state.offset);
   TEST_ASSERT(got==want);TEST_ASSERT(memcmp(xml,expected_xml,sizeof(xml))==0);Compare(&expected,expected_trace,&expected_choice,count,tag);
  }
 }
 printf("Matched %zu combined SSML controller output/state/XML/callback-order transitions\n",count);
 return 0;
}
