/* Complete frozen C public controllers vs Rust, with stateful engine effects.
 * SPDX-License-Identifier: GPL-3.0-or-later */
#include "config.h"
#include "common.h"
#include "speech.h"
#include "voice.h"
#include "rust_data.h"
#include "test_assert.h"
#include <ctype.h>
#include <errno.h>
#include <pthread.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
_Static_assert(sizeof(RustVoiceControl)==(sizeof(void *)==8?88:44),"control ABI");
static voice_t states[3],*fixture_voice;
static int n_voices_list;
static espeak_VOICE candidate,*voices_list[]={&candidate,NULL};
static unsigned char trace[16384],expected[16384];static size_t trace_length;
static unsigned pairs,guards;
static unsigned options,loads,changes,stacks;
static char chosen[4096],property_id[4096],input[128],prop_name[128],prop_language[4096];
static espeak_VOICE properties;
static int native_guard,bad_map,bad_current,bad_result;
static RustVoiceControl *live;
static void Event(unsigned kind,const void *bytes,size_t n)
{TEST_ASSERT(n<=4096&&trace_length+n+3<sizeof(trace));trace[trace_length++]=kind;trace[trace_length++]=n;trace[trace_length++]=n>>8;memcpy(trace+trace_length,bytes,n);trace_length+=n;}
static void Text(unsigned kind,const char *text)
{if(text)Event(kind,text,strlen(text)+1);else Event(kind,"",0);}
static void StackFields(const espeak_VOICE *s)
{
 char name[40]={0};if(s->name)strncpy0(name,s->name,40);Event(20,name,40);
 Text(21,s->languages);unsigned char values[]={s->age,s->gender,s->variant};Event(22,values,3);
}
static void *NestedThread(void *opaque)
{TEST_ASSERT(espeak_rs_voice_control_call(opaque,RUST_VOICE_NAME,"en",NULL,0,NULL)==EAGAIN);return NULL;}
static voice_t *FakeLoad(const char *name,int control)
{
 Event(1,&control,sizeof(control));Text(2,name);loads++;
 if(native_guard&&loads==1) {
  TEST_ASSERT(espeak_rs_voice_control_call(live,RUST_VOICE_VARIANT,"en",NULL,0,(void **)&candidate.spare)==EAGAIN);
  pthread_t thread;TEST_ASSERT(pthread_create(&thread,NULL,NestedThread,live)==0&&pthread_join(thread,NULL)==0);
  live->load=NULL;live->stack=NULL;live->separator=0;
 }
 if(options&16) {strcpy(input,"Changed Original Input");strcpy(prop_name,"Changed Property Name");strcpy(prop_language,"fr");properties.age=91;properties.gender=2;properties.variant=11;}
 if(control==2) {fixture_voice=&states[2];return options&4?NULL:fixture_voice;}
 fixture_voice=&states[1];
 if(control==1||control==0x10) {n_voices_list=(options&8)?0:1;return options&1?NULL:fixture_voice;}
 return options&2?NULL:fixture_voice;
}
static const espeak_VOICE **FakeCatalog(espeak_VOICE *spec)
{TEST_ASSERT(!spec);Event(3,"",0);n_voices_list=1;return (const espeak_VOICE **)voices_list;}
static espeak_VOICE *FakeByName(espeak_VOICE **list,const char *name)
{TEST_ASSERT(list==voices_list);Text(4,name);candidate.identifier=chosen;return options&32?NULL:&candidate;}
static const char *FakeSelect(espeak_VOICE *spec,int *found)
{
 Text(5,spec->name);Text(6,spec->languages);Text(7,spec->identifier);unsigned char values[]={spec->age,spec->gender,spec->variant};Event(8,values,3);
 *found=(options&64)==0;
 if(options&16){strcpy(prop_name,"Selection Changed Name");properties.age=82;}
 return bad_result==1?NULL:property_id;
}
static void FakeChange(voice_t *v)
{TEST_ASSERT(v==fixture_voice);Text(9,v->language_name);changes++;if(options&16){strcpy(input,"Changed After Voice Change");strcpy(prop_name,"Changed After Voice Change");properties.age=97;}}
static void FakeStack(espeak_VOICE *spec,const char *variant)
{StackFields(spec);Text(23,variant);stacks++;}
#define voice fixture_voice
#define LoadVoice FakeLoad
#define espeak_ListVoices FakeCatalog
#define SelectVoiceByName FakeByName
#define SelectVoice FakeSelect
#define DoVoiceChange FakeChange
#define SetVoiceStack FakeStack
#define LoadVoiceVariant PreviousVariant
#define espeak_ng_SetVoiceByFile PreviousFile
#define espeak_ng_SetVoiceByName PreviousName
#define espeak_ng_SetVoiceByProperties PreviousProperties
#include "voice_control_reference.inc"
#undef voice
#undef LoadVoice
#undef espeak_ListVoices
#undef SelectVoiceByName
#undef SelectVoice
#undef DoVoiceChange
#undef SetVoiceStack
#undef LoadVoiceVariant
#undef espeak_ng_SetVoiceByFile
#undef espeak_ng_SetVoiceByName
#undef espeak_ng_SetVoiceByProperties
static void Classes(void *r,unsigned char *map) {(void)r;for(unsigned i=0;i<256;i++)map[i]=tolower(i);if(bad_map)map[0]=1;}
static void *Load(void *r,const char *name,int control) {(void)r;return FakeLoad(name,control);}
static int Empty(void *r) {(void)r;return n_voices_list==0;}
static void Catalog(void *r) {(void)r;FakeCatalog(NULL);}
static const char *ByName(void *r,const char *name) {(void)r;espeak_VOICE *v=FakeByName(voices_list,name);return v?v->identifier:NULL;}
static const char *Select(void *r,const espeak_VOICE *spec,int *found) {(void)r;return FakeSelect((espeak_VOICE *)spec,found);}
static void Change(void *r) {(void)r;FakeChange(fixture_voice);}
static void *Current(void *r) {(void)r;if(bad_current==1)return NULL;if(bad_current==2)memset(fixture_voice->language_name,'x',20);return fixture_voice;}
static void Stack(void *r,const espeak_VOICE *s,const char *suffix) {(void)r;FakeStack((espeak_VOICE *)s,suffix);}
static RustVoiceControl Descriptor(void)
{RustVoiceControl d={NULL,PATHSEP,Classes,Load,Empty,Catalog,ByName,Select,Change,Current,Stack};return d;}
static void Reset(const char *request,unsigned config)
{
 options=config;loads=changes=stacks=0;trace_length=0;n_voices_list=config%2;native_guard=bad_map=bad_current=bad_result=0;
 memset(states,0,sizeof(states));strcpy(states[0].language_name,"old");strcpy(states[1].language_name,"en");strcpy(states[2].language_name,"tone");fixture_voice=&states[0];
 strcpy(input,request);strcpy(prop_name,"Original Selector");strcpy(prop_language,"en");strcpy(chosen,"Mixed/Fallback/ID");strcpy(property_id,config%2?"en+3":"en+MALE");
 memset(&properties,0,sizeof(properties));properties.name=prop_name;properties.languages=prop_language;properties.identifier="Original/Identifier";properties.gender=config%3;properties.age=config%101;properties.variant=config%21;
}
static void Pair(const char *request,unsigned mode,unsigned config,int number)
{
 int code[2],index[2];unsigned observed[2][4];size_t size=0;
 for(int lane=0;lane<2;lane++) {
  Reset(request,config);RustVoiceControl d=Descriptor();void *result=(void *)0x55;
  if(lane)code[lane]=espeak_rs_voice_control_call(&d,mode,input,&properties,number,&result);
  else if(mode==RUST_VOICE_NAME)code[lane]=PreviousName(input);
  else if(mode==RUST_VOICE_FILE)code[lane]=PreviousFile(input);
  else if(mode==RUST_VOICE_PROPERTIES)code[lane]=PreviousProperties(&properties);
  else {result=PreviousVariant(input,number);code[lane]=result?ENS_OK:ENS_VOICE_NOT_FOUND;}
  index[lane]=mode==3?(result==&states[1]?1:result==&states[2]?2:0):(int)(fixture_voice-states);
  observed[lane][0]=loads;observed[lane][1]=changes;observed[lane][2]=stacks;observed[lane][3]=n_voices_list;
  if(!lane){size=trace_length;memcpy(expected,trace,size);}else if(size!=trace_length||memcmp(expected,trace,size)) {fprintf(stderr,"voice-control trace mismatch request=%s mode=%u config=%u old=%zu new=%zu\n",request,mode,config,size,trace_length);abort();}
 }
 TEST_ASSERT(code[0]==code[1]&&index[0]==index[1]&&memcmp(observed[0],observed[1],sizeof(observed[0]))==0);pairs++;
}
static void Guards(void)
{
 Reset("EN+1",0);RustVoiceControl d=Descriptor(),invalid=d;void *out=(void *)0x55;
 TEST_ASSERT(espeak_rs_voice_control_call(NULL,0,input,NULL,0,NULL)==EINVAL);
 TEST_ASSERT(espeak_rs_voice_control_call(&d,4,input,NULL,0,NULL)==EINVAL);
 TEST_ASSERT(espeak_rs_voice_control_call(&d,0,NULL,NULL,0,NULL)==EINVAL);guards++;
 invalid.separator=0;TEST_ASSERT(espeak_rs_voice_control_call(&invalid,0,input,NULL,0,NULL)==EINVAL);
 invalid=d;invalid.load=NULL;TEST_ASSERT(espeak_rs_voice_control_call(&invalid,3,input,NULL,0,&out)==EINVAL&&out==(void *)0x55);guards++;
 TEST_ASSERT(espeak_rs_voice_control_call(&d,2,NULL,NULL,0,NULL)==EINVAL);
 TEST_ASSERT(espeak_rs_voice_control_call(&d,3,input,NULL,0,NULL)==EINVAL&&loads==0);guards++;
 /* Mode-selected primitives: variant does not need catalogue/stack/classes. */
 invalid=d;invalid.classes=NULL;invalid.empty=NULL;invalid.catalog=NULL;invalid.by_name=NULL;invalid.select=NULL;invalid.change=NULL;invalid.current=NULL;invalid.stack=NULL;
 TEST_ASSERT(espeak_rs_voice_control_call(&invalid,3,input,NULL,0,&out)==0&&out==&states[2]);guards++;
 Reset("EN+1",0);d=Descriptor();native_guard=1;live=&d;TEST_ASSERT(espeak_rs_voice_control_call(&d,0,input,NULL,0,NULL)==0&&loads==2&&changes==1&&stacks==1);guards++;
 Reset("en",0);d=Descriptor();bad_map=1;TEST_ASSERT(espeak_rs_voice_control_call(&d,0,input,NULL,0,NULL)==EINVAL&&loads==0);guards++;
 for(int bad=1;bad<=2;bad++) {Reset("en",0);d=Descriptor();bad_current=bad;TEST_ASSERT(espeak_rs_voice_control_call(&d,0,input,NULL,0,NULL)==EINVAL&&changes==1&&stacks==0);guards++;}
 Reset("en",64);d=Descriptor();char unterminated[4096];memset(unterminated,'x',sizeof(unterminated));properties.name=unterminated;TEST_ASSERT(espeak_rs_voice_control_call(&d,2,NULL,&properties,0,NULL)==ENS_VOICE_NOT_FOUND&&loads==0&&trace_length>4096);guards++;
 Reset("en",0);d=Descriptor();memset(prop_language,'x',4096);TEST_ASSERT(espeak_rs_voice_control_call(&d,2,NULL,&properties,0,NULL)==EINVAL&&changes==1&&stacks==0);guards++;
 Reset("en",64);d=Descriptor();memset(prop_language,'x',24);prop_language[24]=0;TEST_ASSERT(espeak_rs_voice_control_call(&d,2,NULL,&properties,0,NULL)==ENS_VOICE_NOT_FOUND&&loads==0);guards++;
 Reset("en",0);d=Descriptor();memset(prop_language,'x',24);prop_language[24]=0;TEST_ASSERT(espeak_rs_voice_control_call(&d,2,NULL,&properties,0,NULL)==EINVAL&&changes==1&&stacks==0);guards++;
 Reset("en",0);d=Descriptor();bad_result=1;TEST_ASSERT(espeak_rs_voice_control_call(&d,2,NULL,&properties,0,NULL)==EINVAL&&loads==0);guards++;
 Reset("en",0);d=Descriptor();memset(property_id,'x',4096);TEST_ASSERT(espeak_rs_voice_control_call(&d,2,NULL,&properties,0,NULL)==EINVAL&&loads==0);guards++;
 /* Pointer publication may overlap the request after its prefix snapshot. */
 Reset("en",0);d=Descriptor();union {void *p;char bytes[60];} overlap;memset(&overlap,0,sizeof(overlap));strcpy(overlap.bytes,"EN+1");TEST_ASSERT(espeak_rs_voice_control_call(&d,3,overlap.bytes,NULL,0,&overlap.p)==0&&overlap.p==&states[2]);guards++;
 Reset("en+1",0);d=Descriptor();d.separator='\\';TEST_ASSERT(espeak_rs_voice_control_call(&d,3,input,NULL,0,&out)==0&&out==&states[2]);guards++;
 /* Original name is a bounded prefix: a full initialized nonterminated tail
  * is valid and intentionally truncates, rather than forcing a path scan. */
 Reset("en",0);d=Descriptor();memset(input,'A',sizeof(input));TEST_ASSERT(espeak_rs_voice_control_call(&d,3,input,NULL,0,&out)==0&&out==&states[1]);guards++;
}
int main(void)
{
 const char *names[]={"EN","EN+1","EN+11","EN+MALE","/Tmp/Mixed/Voice+Name","EN+0","EN+9junk","EN+-3","EN+","+1","","EN+99999999999","UPPERCASEVISIBLEVOICEWITHALONGPATH/ANDMANYCHARS+namedvariant","EN+ name"};
 for(unsigned trial=0;trial<6000;trial++)for(unsigned mode=0;mode<4;mode++)Pair(names[trial%14],mode,trial%128,(int)(trial%24)-3);
 Guards();printf("%u complete previous-C/Rust public voice controller pairs match status, loaded/current pointer, catalogue fallback, all effect arguments/order, fresh aliased stack fields and ignored load failures; %u snapshot/reentry/thread/bounds/map/overlap guards\n",pairs,guards);return 0;
}
