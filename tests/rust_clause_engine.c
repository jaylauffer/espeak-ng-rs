/* Main clause controller against independently extracted retained C.
 * SPDX-License-Identifier: GPL-3.0-or-later */
#include "config.h"
#include "test_assert.h"
#include <ctype.h>
#include <locale.h>
#include <stdint.h>
#include <setjmp.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <wchar.h>
#include <wctype.h>
#include "common.h"
#include "translate.h"
#include "rust_data.h"
static RustClauseState live;
static Translator translator_fixture;
static uint32_t source[512];
static int source_index,source_length;
static wchar_t punctuation_list[60];
static espeak_VOICE fixture_base;
static unsigned trace,seed=0x9531e627u;
static unsigned visits;
static int active_episode,active_segment,native_phase;
static int last_current,last_next,last_index,last_line;
static jmp_buf reference_guard;
static unsigned next_random(void){seed^=seed<<13;seed^=seed>>17;seed^=seed<<5;return seed;}
static void ToTranslator(void)
{
 translator_fixture.phonemes_repeat_count=live.repeat_count;
 translator_fixture.clause_upper_count=live.upper_count;translator_fixture.clause_lower_count=live.lower_count;
 translator_fixture.translator_name=live.language;translator_fixture.langopts.numbers=live.numbers;
 translator_fixture.langopts.lowercase_sentence=live.lowercase_sentence!=0;
 for(int i=0;i<60;i++){punctuation_list[i]=(wchar_t)live.punctuation_list[i];if(!punctuation_list[i])break;}
}
static void FromTranslator(void)
{
 live.repeat_count=translator_fixture.phonemes_repeat_count;
 live.upper_count=translator_fixture.clause_upper_count;live.lower_count=translator_fixture.clause_lower_count;
 live.language=translator_fixture.translator_name;live.numbers=translator_fixture.langopts.numbers;
 live.lowercase_sentence=translator_fixture.langopts.lowercase_sentence;
 for(int i=0;i<60;i++){live.punctuation_list[i]=(uint32_t)punctuation_list[i];if(!punctuation_list[i])break;}
}
static void Trace(unsigned kind,int code)
{
 FromTranslator();trace=trace*33+kind;trace=trace*33+(unsigned)code;
 trace=trace*33+(unsigned)live.pending;trace=trace*33+(unsigned)live.pending_second;
 trace=trace*33+(unsigned)live.count;trace=trace*33+(unsigned)live.index_top;
 trace=trace*33+(unsigned)live.tone;trace=trace*33+(unsigned)live.upper_count;
 trace=trace*33+(unsigned)live.lower_count;trace=trace*33+(unsigned)live.sayas_mode;
 trace=trace*33+(unsigned)live.ignore;trace=trace*33+(unsigned)live.parameters[0];
}
static int SourceEof(void){
 if(++visits>100000){
  if(!native_phase)longjmp(reference_guard,1);
  fprintf(stderr,"native clause loop guard episode=%d segment=%d source=%d/%d c1=%x c2=%x ix=%d line=%d\n",active_episode,active_segment,source_index,source_length,last_current,last_next,last_index,last_line);exit(1);
 }
 return source_index>=source_length;
}
static uint32_t SourceRead(void){return source_index<source_length?source[source_index++]:0;}
static uint32_t SourcePeek(void){return source_index<source_length?source[source_index]:0;}
static int InputEof(void){return live.pending==0 && SourceEof();}
static int FixtureEof(int current,int following,int index,int line){last_current=current;last_next=following;last_index=index;last_line=line;return InputEof();}
static int InputRead(void){if(live.pending){int code=live.pending;live.pending=0;return code;}live.count++;return (int)SourceRead();}
static void InputUnread(int code){live.pending=code;}
static void Terminate(char *out,int index,int *value){out[index]=' ';out[index+1]=0;if(value)InputUnread(*value);}
static bool Replace(Translator *tr,int *code){(void)tr;if(*code==0x2060)return true;if(*code==0xa0)*code=' ';return false;}
static const char *Capital(Translator *tr,const char *key,char *text,size_t capacity)
{
 (void)tr;TEST_ASSERT(strcmp(key,"_cap")==0 && capacity==30);Trace(4,0);strcpy(text,"[\002kaep]]");return text;
}
static const char *Character(char *text,Translator *tr,int code,bool only)
{
 (void)tr;TEST_ASSERT(only);Trace(3,code);strcpy(text,"[\002ellipsis]]");return text;
}
static int Announcement(Translator *tr,int code,int *following,char *out,int *index,int end,int capacity)
{
 (void)tr;(void)capacity;Trace(2,code);
 if(end && (*index & 1)){live.pending_second=code;Terminate(out,0,following);*index=0;return -1;}
 const char *text=" [\002punct]]";strcpy(out+*index,text);*index+=(int)strlen(text);
 return end?CLAUSE_SHORTFALL:-1;
}
static int Tag(wchar_t *xml,char *out,int *index,int capacity,const char *xmlbase,int *audio,char *current,
 espeak_VOICE *base,char *variant,int *ignore,int *clear,int *sayas,int *start,SSML_STACK *stack,int *stack_count,int *param_count,int *parameters)
{
 (void)capacity;(void)xmlbase;(void)base;(void)variant;(void)clear;(void)start;(void)stack;(void)stack_count;(void)param_count;
 Trace(1,(int)xml[0]);out[(*index)++]=' ';
 if(xml[0]=='x')*ignore=1;
 if(xml[0]=='/' && xml[1]=='x')*ignore=0;
 if(xml[0]=='a')*audio=1;
 if(xml[0]=='/' && xml[1]=='a')*audio=0;
 if(xml[0]=='c')*sayas=0x14;
 if(xml[0]=='/' && xml[1]=='c')*sayas=0;
 if(xml[0]=='s')parameters[0]=1;
 if(xml[0]=='/' && xml[1]=='s')parameters[0]=0;
 if(xml[0]=='v'){strcpy(current,"fr");return CLAUSE_VOICE;}
 if(xml[0]=='b')return CLAUSE_NONE;
 return 0;
}
static int ByteSpace(uint32_t code){return isspace((unsigned char)code)!=0;}
static int ParseReference(const char *text,int *first,int *second){return espeak_rs_ssml_reference(text,first,second,ByteSpace);}
static int PhonemeMode(int enabled,int mode,int current,int following){return espeak_rs_clause_phoneme_mode(enabled,mode,current,following);}
static int Roman(unsigned code){return espeak_rs_clause_roman(code);}
static void Remove(char *text){int code;memset(text,' ',utf8_in(&code,text));}
static int FixturePeek(void *ignored){(void)ignored;return (int)SourcePeek();}
#define N_XML_BUF 500
#define MAKE_MEM_UNDEFINED(...) ((void)0)
#define ungot_string ((char *)live.replay)
#define ungot_string_ix live.replay_index
#define ungot_char live.pending
#define ungot_char2 live.pending_second
#define clear_skipping_text live.clear_skipping
#define skipping_text live.skipping
#define count_characters live.count
#define end_character_position live.end_position
#define skip_characters live.skip_characters
#define clause_start_char live.clause_start
#define option_ssml live.ssml
#define option_phoneme_input live.phoneme_input
#define option_linelength live.line_length
#define option_capitals live.capitals
#define option_punctuation live.punctuation
#define option_punctlist punctuation_list
#define ignore_text live.ignore
#define audio_text live.audio
#define sayas_mode live.sayas_mode
#define sayas_start live.sayas_start
#define speech_parameters live.parameters
#define base_voice fixture_base
#define current_voice_id ((char *)live.current_voice)
#define base_voice_variant_name ""
#define ssml_stack NULL
#define n_ssml_stack fixture_stack_count
#define n_param_stack fixture_parameter_count
#define xmlbase NULL
#define p_decoder NULL
#define text_decoder_peekc FixturePeek
#define Eof() FixtureEof(c1,c2,ix,__LINE__)
#define GetC InputRead
#define UngetC InputUnread
#define ParseSsmlReference ParseReference
#define ProcessSsmlTag Tag
#define IgnoreOrReplaceChar Replace
#define LookupSpecial Capital
#define LookupCharName Character
#define AnnouncePunctuation Announcement
#define CheckPhonemeMode PhonemeMode
#define IsRomanU Roman
#define RemoveChar Remove
#define TerminateBufWithSpaceAndZero Terminate
#define ReadClause ReferenceReadClause
static int fixture_stack_count,fixture_parameter_count;
#include "main_clause_reference.inc"
#undef ReadClause
#undef ungot_string
#undef ungot_string_ix
#undef ungot_char
#undef ungot_char2
#undef clear_skipping_text
#undef skipping_text
#undef count_characters
#undef end_character_position
#undef skip_characters
#undef clause_start_char
#undef option_ssml
#undef option_phoneme_input
#undef option_linelength
#undef option_capitals
#undef option_punctuation
#undef option_punctlist
#undef ignore_text
#undef audio_text
#undef sayas_mode
#undef sayas_start
#undef speech_parameters
#undef base_voice
#undef current_voice_id
#undef base_voice_variant_name
#undef ssml_stack
#undef n_ssml_stack
#undef n_param_stack
#undef xmlbase
#undef p_decoder
#undef text_decoder_peekc
#undef Eof
#undef GetC
#undef UngetC
#undef ParseSsmlReference
#undef ProcessSsmlTag
#undef IgnoreOrReplaceChar
#undef LookupSpecial
#undef LookupCharName
#undef AnnouncePunctuation
#undef CheckPhonemeMode
#undef IsRomanU
#undef RemoveChar
#undef TerminateBufWithSpaceAndZero
static int32_t NativeEof(void *owner){(void)owner;return SourceEof();}
static uint32_t NativeRead(void *owner){(void)owner;return SourceRead();}
static uint32_t NativePeek(void *owner){(void)owner;return SourcePeek();}
static int32_t Classify(int32_t code,uint32_t kind)
{
 switch(kind){case 0:return iswspace(code)!=0;case 1:return iswalnum(code)!=0;case 2:return iswalpha(code)!=0;
 case 3:return iswupper(code)!=0;case 4:return iswlower(code)!=0;case 5:return iswdigit(code)!=0;
 case 6:return iswpunct(code)!=0;case 7:return IsAlpha(code)!=0;case 8:return IsBracket(code)!=0;case 9:return ByteSpace((uint32_t)code);default:return 0;}
}
static int32_t NativeReplace(void *owner,int32_t *code){(void)owner;return Replace(&translator_fixture,code);}
static int32_t NativeEffect(void *owner,RustClauseState *state,RustClauseCommand *command,unsigned char *output,size_t capacity)
{
 (void)owner;live=*state;ToTranslator();
 switch(command->kind){
 case 1:{wchar_t xml[501];for(int i=0;i<501;i++){xml[i]=(wchar_t)command->xml[i];if(!xml[i])break;}
 command->clause=Tag(xml,(char *)output,&command->index,(int)capacity,NULL,&live.audio,(char *)live.current_voice,&fixture_base,NULL,&live.ignore,&live.clear_skipping,&live.sayas_mode,&live.sayas_start,NULL,NULL,NULL,live.parameters);break;}
 case 2:command->clause=Announcement(&translator_fixture,command->code,&command->next,(char *)output,&command->index,command->end,(int)capacity);break;
 case 3:Character((char *)command->text,&translator_fixture,command->code,command->end!=0);break;
 case 4:command->found=Capital(&translator_fixture,"_cap",(char *)command->text,30)!=NULL;break;
 default:return 2;}
 FromTranslator();*state=live;return 0;
}
static void Initialize(void)
{
 memset(&live,0,sizeof(live));live.replay_index=-1;live.signed_bytes=(char)-1<0;live.wide16=sizeof(wchar_t)==2;
 live.ssml=(int)(next_random()%2);live.phoneme_input=(int)(next_random()%2);live.capitals=(int)(next_random()%3);
 live.punctuation=(int)(next_random()%3);live.punctuation_list[0]='!';live.punctuation_list[1]='.';
 live.language=next_random()%2?0x6875:0x656e;live.numbers=next_random()%2?NUM_ORDINAL_DOT:0;
 live.lowercase_sentence=(int)(next_random()%2);live.line_length=(int[]){-1,0,3,10}[next_random()%4];
 live.count=(int)(next_random()%100);live.clause_start=live.count;live.index_top=7;
 live.skip_characters=next_random()%20==0?live.count+3:0;live.end_position=next_random()%20==0?live.count+5:0;
 live.clear_skipping=(int)(next_random()%2);live.skipping=(int)(next_random()%2);
 live.upper_count=19;live.lower_count=23;live.repeat_count=7;live.sayas_mode=next_random()%15==0?0x24:0;
 strcpy((char *)live.base_identifier,"en");strcpy((char *)live.current_voice,"en");live.has_base_identifier=1;
 fixture_base.identifier="en";ToTranslator();
}
int main(void)
{
 setlocale(LC_ALL,"C");
 const uint32_t codes[]={32,32,10,9,65,66,97,98,115,49,50,73,86,46,44,33,63,39,40,41,45,91,93,38,35,59,60,62,47,1,86,66,0xa0,0x2026,0x2060,0x55e,0xf0b,0xd4d,0xdca,0x200d,0x1f600};
 const char *texts[]={"One. <i> Next", "One. <i> next", "a &bad;B", "a &amp; B", "a &#x100;B", "A<x>ignored</x>B", "A<c>x &nbsp; y</c>B", "A<v>voice", "A<s>x</s>B", "a... b?" "?!  C", "december 2., szerda", "u.s.a.'s test", "I. item", "a\001B0 b", "a\001B!. b", "a\001Vfr next", "a\n\n\n\nB", "a [[x!?]] B", "a.&no;C", "a &abcdefghijklmnopqrstuv; B", "a<a>alt</a>B"};
 size_t clauses=0,guarded_loops=0;
 for(int episode=0;episode<100000;episode++){
  Initialize();source_length=0;
  int capacity=512;
  if(episode%4==0){
   /* Buffer boundaries use defined terminating ASCII text, independent of the
    * legacy unbounded literal-name writes and four-byte boundary overrun. */
   static const char boundary_codes[]="Ab1c D2e ";
   capacity=96+(int)(next_random()%417);live.punctuation=0;live.capitals=0;live.ssml=0;live.sayas_mode=0;
   int length=64+(int)(next_random()%337);for(int i=0;i<length;i++)source[source_length++]=(unsigned char)boundary_codes[next_random()%(sizeof(boundary_codes)-1)];
  }
  else if(episode%3==0){const char *text=texts[next_random()%(sizeof(texts)/sizeof(texts[0]))];while(*text)source[source_length++]=(unsigned char)*text++;}
  else {int length=1+(int)(next_random()%65);for(int i=0;i<length;i++)source[source_length++]=codes[next_random()%(sizeof(codes)/sizeof(codes[0]))];}
  source[source_length++]=0;source_index=0;
  RustClauseState native=live;
  for(int segment=0;segment<120;segment++){
   RustClauseState initial=live;
   int initial_source=source_index;
   char expected[512],actual[512];short expected_indexes[512],actual_indexes[512];
   memset(expected,0xa5,512);memset(actual,0xa5,512);memset(expected_indexes,0x5a,sizeof(expected_indexes));memcpy(actual_indexes,expected_indexes,sizeof(expected_indexes));
   trace=0;visits=0;active_episode=episode;active_segment=segment;native_phase=0;
   if(setjmp(reference_guard)){
    live=initial;ToTranslator();source_index=initial_source;trace=0;visits=0;native_phase=1;
    RustClauseContext context={NULL,NativeEof,NativeRead,NativePeek,Classify,NativeReplace,NativeEffect};
    TEST_ASSERT(espeak_rs_read_clause(&context,&native,(unsigned char *)actual,(size_t)capacity,actual_indexes,512)==-2);
    guarded_loops++;break;
   }
   int want=ReferenceReadClause(&translator_fixture,expected,expected_indexes,&live.index_top,capacity,&live.tone,(char *)live.voice_change);
   FromTranslator();RustClauseState wanted=live;unsigned wanted_trace=trace;int wanted_source=source_index;
   live=initial;ToTranslator();source_index=initial_source;trace=0;visits=0;native_phase=1;
   RustClauseContext context={NULL,NativeEof,NativeRead,NativePeek,Classify,NativeReplace,NativeEffect};
   int got=espeak_rs_read_clause(&context,&native,(unsigned char *)actual,(size_t)capacity,actual_indexes,512);
   if(got!=want || memcmp(actual,expected,512) || memcmp(actual_indexes,expected_indexes,sizeof(expected_indexes)) || trace!=wanted_trace || source_index!=wanted_source || native.count!=wanted.count || native.pending!=wanted.pending || native.pending_second!=wanted.pending_second)
    fprintf(stderr,"clause mismatch episode=%d segment=%d clauses=%x/%x source=%d/%d count=%d/%d pending=%x/%x replay=%d/%d traces=%u/%u\n",episode,segment,got,want,source_index,wanted_source,native.count,wanted.count,native.pending,wanted.pending,native.replay_index,wanted.replay_index,trace,wanted_trace);
   TEST_ASSERT(got==want);TEST_ASSERT(memcmp(actual,expected,512)==0);TEST_ASSERT(memcmp(actual_indexes,expected_indexes,sizeof(expected_indexes))==0);
   TEST_ASSERT(trace==wanted_trace && source_index==wanted_source);
   TEST_ASSERT(native.count==wanted.count && native.pending==wanted.pending && native.pending_second==wanted.pending_second);
   TEST_ASSERT(native.upper_count==wanted.upper_count && native.lower_count==wanted.lower_count && native.repeat_count==wanted.repeat_count);
   TEST_ASSERT(native.tone==wanted.tone && native.index_top==wanted.index_top && native.punctuation==wanted.punctuation);
   TEST_ASSERT(native.clear_skipping==wanted.clear_skipping && native.skipping==wanted.skipping && native.skip_characters==wanted.skip_characters);
   TEST_ASSERT(native.ignore==wanted.ignore && native.audio==wanted.audio && native.sayas_mode==wanted.sayas_mode);
   TEST_ASSERT(memcmp(native.parameters,wanted.parameters,sizeof(native.parameters))==0);
   TEST_ASSERT(strcmp((char *)native.voice_change,(char *)wanted.voice_change)==0 && strcmp((char *)native.current_voice,(char *)wanted.current_voice)==0);
   TEST_ASSERT(native.replay_index==wanted.replay_index);
   clauses++;live=wanted;ToTranslator();
   if(want==CLAUSE_EOF)break;
   TEST_ASSERT(segment<119);
  }
 }
 RustClauseState state={0};state.replay_index=-1;state.signed_bytes=1;
 unsigned char output[8];short indexes[8];memset(output,0xa5,8);memset(indexes,0x5a,sizeof(indexes));
 RustClauseContext context={NULL,NativeEof,NativeRead,NativePeek,Classify,NativeReplace,NativeEffect};
 source[0]='a';source[1]=0;source_index=0;source_length=2;
 TEST_ASSERT(espeak_rs_read_clause(&context,&state,output,0,indexes,8)==-2);
 TEST_ASSERT(output[0]==0xa5 && indexes[0]==0x5a5a);
 fprintf(stderr,"%zu main clause comparisons passed across 100000 episodes; %zu retained-C loops guarded\n",clauses,guarded_loops);
 return 0;
}
