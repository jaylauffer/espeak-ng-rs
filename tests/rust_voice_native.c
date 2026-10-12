/* Complete preceding native LoadVoice/host vs Rust over real source files.
 * SPDX-License-Identifier: GPL-3.0-or-later */
#include "config.h"
#include "common.h"
#include "speech.h"
#include "voice.h"
#include "translate.h"
#include "rust_data.h"
#include "test_assert.h"
#include <errno.h>
#include <pthread.h>
#include <stdarg.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <sys/stat.h>
#ifdef _WIN32
#include <direct.h>
#include <process.h>
#define MKDIR(p) _mkdir(p)
#define PID _getpid
#else
#include <unistd.h>
#define MKDIR(p) mkdir(p,0700)
#define PID getpid
#endif
/* The oracle selects its full native backend host; fake effects never spawn
 * backend processes. Production feature configurations run separately. */
#undef USE_MBROLA
#define USE_MBROLA 1
#undef USE_KLATT
#define USE_KLATT 1
_Static_assert(sizeof(RustVoiceNative)==(sizeof(void *)==8?168:84),"native descriptor ABI");
_Static_assert(sizeof(RustVoiceEnvironment)==(sizeof(void *)==8?48:24),"environment ABI");
_Static_assert(sizeof(voice_t)==1344,"all acoustic fields packed without padding");
static char fixture_root[N_PATH_BUF],original[N_PATH_BUF],dictionary[40];
static voice_t states[3];
static Translator translators[2];
static espeak_VOICE selected;
static RustCurrentVoice *current;
static struct {int fast_settings;} fixture_speed;
static unsigned options,pairs,guards,native_guard,bad_environment;
static int native_lane;
static FILE *stream;
static unsigned char trace[256*1024],expected[256*1024];static size_t used;
static RustVoiceNative *live_descriptor;
static const char *fresh_original;
static void Add(const void *bytes,size_t n)
{TEST_ASSERT(used+n<sizeof(trace));memcpy(trace+used,bytes,n);used+=n;}
static void Text(const char *s)
{unsigned n=s?(unsigned)strlen(s)+1:0;Add(&n,sizeof(n));if(n)Add(s,n);}
static void Observe(unsigned kind,const char *s,int argument)
{
 Add(&kind,sizeof(kind));Text(s);Add(&argument,sizeof(argument));
 int index=(int)(voice-states),has_translator=translator!=NULL;Add(&index,sizeof(index));Add(&has_translator,sizeof(has_translator));
 Add(states,sizeof(states));Add(&fixture_speed,sizeof(fixture_speed));
 unsigned char values[]={selected.gender,selected.age,selected.variant};Add(values,sizeof(values));
 int present=current!=NULL;Add(&present,sizeof(present));if(current)Add(current,sizeof(*current));
}
static int64_t Length(void *context,const unsigned char *path,size_t n)
{(void)context;(void)n;return GetFileLength((const char *)path);}
static void *Nested(void *opaque)
{
 void *out=(void *)0x55;TEST_ASSERT(espeak_rs_voice_native_load(opaque,"en",0,&out)==EAGAIN&&out==(void *)0x55);
 TEST_ASSERT(espeak_ng_SetVoiceByName("en")==EAGAIN);return NULL;
}
static void ResetVoice(int tone)
{
 Observe(1,NULL,tone);memset(voice,0,sizeof(*voice));strcpy(voice->language_name,"reset");voice->samplerate=22050;fixture_speed.fast_settings=71;
 if(options&16)strcpy(original,"Reset changed original");
 if(native_guard) {
  void *out=(void *)0x55;RustCurrentVoice *saved=current;
  TEST_ASSERT(espeak_rs_voice_native_load(live_descriptor,"en",0,&out)==EAGAIN&&out==(void *)0x55);
  TEST_ASSERT(espeak_rs_voice_native_clear(&current)==EAGAIN&&current==saved);
  TEST_ASSERT(espeak_ng_SetVoiceByName("en")==EAGAIN);
  pthread_t thread;TEST_ASSERT(pthread_create(&thread,NULL,Nested,live_descriptor)==0&&pthread_join(thread,NULL)==0);
  live_descriptor->table=NULL;live_descriptor->dictionary=NULL;live_descriptor->root=NULL;
  native_guard=0;
 }
}
static void Delete(Translator *p)
{TEST_ASSERT(p==translator);Observe(2,NULL,0);}
static Translator *TranslatorFor(const char *name)
{Observe(3,name,0);return &translators[options%2];}
static int Table(const char *name)
{
 Observe(4,name,0);
 if((options&32)&&fresh_original)strcpy((char *)fresh_original,"Table changed original");
 return options&1?-1:3;
}
static void Options(Translator *p,int key,char *value)
{TEST_ASSERT(p==translator);Observe(5,value,key);voice->roughness+=9;fixture_speed.fast_settings+=11;}
static void Speed(int mode)
{Observe(6,NULL,mode);voice->flutter+=31;fixture_speed.fast_settings+=2;}
static void Replacement(char *value)
{Observe(7,value,0);voice->voicing+=17;}
static void Dictionary(Translator *p,const char *name,int flags)
{
 TEST_ASSERT(p==translator);Observe(8,name,flags);strcpy(dictionary,options&2?"":"ready");
 if(options&16){strcpy((char *)current->name,"Dictionary changed name");fixture_speed.fast_settings+=23;}
}
static espeak_ng_STATUS Backend(const char *name,const char *table,int *rate)
{
 Observe(9,name,*rate);Text(table);if(options&8){voice=&states[2];fixture_speed.fast_settings+=43;}
 *rate+=7;return options&4?ENS_VOICE_NOT_FOUND:ENS_OK;
}
static void BackendError(espeak_ng_STATUS status,FILE *output,espeak_ng_ERROR_CONTEXT *context)
{TEST_ASSERT(output==stream&&!context);Observe(10,NULL,status);fprintf(output,"backend=%x\n",status);}
static void Environment(void *context,RustVoiceEnvironment *e)
{
 (void)context;e->voice=bad_environment==1?NULL:voice;e->translator=translator;e->fast=&fixture_speed.fast_settings;
 e->translator_index=bad_environment==2?NULL:translator?&translator->phoneme_tab_ix:NULL;
 e->selected=&selected;e->dictionary_ready=dictionary[0]!=0;
}
static void Bind(void *context,const RustCurrentVoice *owner)
{(void)context;voice=&states[1];selected.identifier=(const char *)owner->identifier;selected.name=(const char *)owner->name;selected.languages=(const char *)owner->languages;}
static void NativeReset(void *context,int tone){(void)context;ResetVoice(tone);}
static void NativeDelete(void *context,int clear){(void)context;Delete(translator);if(clear)translator=NULL;}
static void NativeTranslator(void *context,const char *name){(void)context;translator=TranslatorFor(name);}
static int NativeTable(void *context,const char *name){(void)context;return Table(name);}
static void NativeOptions(void *context,uint32_t key,const char *value){(void)context;Options(translator,key,(char *)value);}
static void NativeSpeed(void *context){(void)context;Speed(3);}
static void NativeReplacement(void *context,const char *value){(void)context;Replacement((char *)value);}
static void NativeDictionary(void *context,const char *name,uint32_t flags){(void)context;Dictionary(translator,name,flags);}
static uint32_t NativeBackend(void *context,const RustMbrolaRequest *request,int32_t *rate){(void)context;return Backend((const char *)request->voice,(const char *)request->table,rate);}
static void NativeError(void *context,uint32_t status){(void)context;BackendError(status,stream,NULL);}
static void Diagnostic(void *context,const char *format,const char *key){(void)context;fprintf(stream,format,key);}
static RustVoiceNative Descriptor(void)
{
 RustVoiceNative d={NULL,fixture_root,"en",&current,PATHSEP,N_PATH_BUF,3,Environment,Length,Bind,NativeReset,NativeDelete,NativeTranslator,NativeTable,NativeOptions,NativeSpeed,NativeReplacement,NativeDictionary,NativeBackend,NativeError,Diagnostic};return d;
}
#define voicedata states[1]
#define speed fixture_speed
#define current_voice_selected selected
#define rust_current_voice current
#define dictionary_name dictionary
#define path_home fixture_root
#define RustVoiceLength Length
#define VoiceReset ResetVoice
#define DeleteTranslator Delete
#define SelectTranslator TranslatorFor
#define SelectPhonemeTableName Table
#define LoadLanguageOptions Options
#define SetSpeed Speed
#define PhonemeReplacement Replacement
#define LoadDictionary Dictionary
#define LoadMbrolaTable Backend
#define espeak_ng_PrintStatusCodeMessage BackendError
#pragma push_macro("stderr")
#undef stderr
#define stderr stream
#define LoadVoice PreviousLoad
#include "voice_native_reference.inc"
#undef voicedata
#undef speed
#undef current_voice_selected
#undef rust_current_voice
#undef dictionary_name
#undef path_home
#undef RustVoiceLength
#undef VoiceReset
#undef DeleteTranslator
#undef SelectTranslator
#undef SelectPhonemeTableName
#undef LoadLanguageOptions
#undef SetSpeed
#undef PhonemeReplacement
#undef LoadDictionary
#undef LoadMbrolaTable
#undef espeak_ng_PrintStatusCodeMessage
#undef stderr
#pragma pop_macro("stderr")
#undef LoadVoice
typedef struct {
 voice_t voices[3];RustCurrentVoice metadata;int present,index,translator_index,has_translator,fast;
 unsigned char gender,age,variant;char dictionary[40];
} Result;
#if defined(__APPLE__)
typedef struct {char bytes[32768];size_t length;unsigned calls;} MutationLog;
static int MutationWrite(void *cookie,const char *bytes,int length)
{
 MutationLog *log=cookie;TEST_ASSERT(length>=0&&log->length+(size_t)length<sizeof(log->bytes));memcpy(log->bytes+log->length,bytes,length);log->length+=length;log->calls++;
 voice=&states[2];voice->roughness+=19;fixture_speed.fast_settings+=7;selected.age=90+log->calls;
 if(current)strcpy((char *)current->name,"Stream changed name");strcpy(original,"Stream changed original");
 if(native_lane){RustCurrentVoice *saved=current;TEST_ASSERT(espeak_rs_voice_native_clear(&current)==EAGAIN&&current==saved);TEST_ASSERT(espeak_ng_SetVoiceByName("en")==EAGAIN);}
 return length;
}
#endif
static void Init(unsigned config,const char *name)
{
 options=config;used=0;native_guard=bad_environment=0;current=(config&128)?NULL:espeak_rs_current_voice_create();
 if(current){strcpy((char *)current->identifier,"en");strcpy((char *)current->name,"Previous Name");memcpy(current->languages,"\5en\0\0",5);}
 memset(states,0,sizeof(states));strcpy(states[0].language_name,"old");voice=&states[0];
 memset(translators,0,sizeof(translators));translator=(config&64)?NULL:&translators[0];fixture_speed.fast_settings=37;strcpy(dictionary,"prior");
 memset(&selected,0,sizeof(selected));if(current){selected.name=(const char *)current->name;selected.identifier=(const char *)current->identifier;selected.languages=(const char *)current->languages;}selected.gender=config%3;selected.age=config%101;selected.variant=config%13;
 snprintf(original,sizeof(original),"%s",name);fresh_original=original;stream=tmpfile();TEST_ASSERT(stream);setvbuf(stream,NULL,_IONBF,0);
}
static void Capture(Result *r)
{
 memset(r,0,sizeof(*r));memcpy(r->voices,states,sizeof(states));r->present=current!=NULL;if(current)r->metadata=*current;
 r->index=(int)(voice-states);r->has_translator=translator!=NULL;r->translator_index=translator?translator->phoneme_tab_ix:0;r->fast=fixture_speed.fast_settings;
 r->gender=selected.gender;r->age=selected.age;r->variant=selected.variant;memcpy(r->dictionary,dictionary,40);
}
static void Pair(const char *name,unsigned control,unsigned config,unsigned alias)
{
 Result results[2];int accepted[2];unsigned log_calls[2]={0,0};size_t trace_size=0,log_size=0;char expected_log[32768],actual_log[32768];
 for(int lane=0;lane<2;lane++) {
  Init(config,name);native_lane=lane;RustVoiceNative d=Descriptor();const char *input=alias==1?(current?(const char *)current->identifier:NULL):alias==2?(current?(const char *)current->name:NULL):alias==3?NULL:original;fresh_original=input;
#if defined(__APPLE__)
  MutationLog log={0};if(config&256){fclose(stream);stream=funopen(&log,NULL,MutationWrite,NULL,NULL);TEST_ASSERT(stream);setvbuf(stream,NULL,_IONBF,0);}
#endif
  void *out=(void *)0x55;
  if(lane){int status=espeak_rs_voice_native_load(&d,input,control,&out);accepted[lane]=status==0;TEST_ASSERT(status==0||status==EINVAL||status==ENS_VOICE_NOT_FOUND);if(status){TEST_ASSERT(out==(void *)0x55);}else{TEST_ASSERT(out==voice);}}
  else {out=PreviousLoad(input,control);accepted[lane]=out!=NULL;if(out)TEST_ASSERT(out==voice);}
  Capture(&results[lane]);fflush(stream);size_t n;
#if defined(__APPLE__)
  if(config&256){n=log.length;memcpy(lane?actual_log:expected_log,log.bytes,n);log_calls[lane]=log.calls;}else
#endif
  {rewind(stream);n=fread(lane?actual_log:expected_log,1,sizeof(expected_log),stream);TEST_ASSERT(!ferror(stream));}
  if(!lane){trace_size=used;memcpy(expected,trace,used);log_size=n;}
  else if(trace_size!=used||memcmp(expected,trace,used)||log_size!=n||memcmp(expected_log,actual_log,n)||log_calls[0]!=log_calls[1]||accepted[0]!=accepted[1]||memcmp(&results[0],&results[1],sizeof(Result))) {
   size_t td=0,rd=0;while(td<trace_size&&td<used&&expected[td]==trace[td])td++;while(rd<sizeof(Result)&&((unsigned char*)&results[0])[rd]==((unsigned char*)&results[1])[rd])rd++;
   fprintf(stderr,"first trace difference=%zu first result difference=%zu of %zu metadata offset=%zu\n",td,rd,sizeof(Result),offsetof(Result,metadata));
   fprintf(stderr,"unbuffered stream calls=%u/%u\n",log_calls[0],log_calls[1]);
   fprintf(stderr,"native voice mismatch name=%s flags=%u config=%u alias=%u old/new accepted=%d/%d trace=%zu/%zu log=%zu/%zu\n",name,control,config,alias,accepted[0],accepted[1],trace_size,used,log_size,n);abort();
  }
  fclose(stream);TEST_ASSERT(espeak_rs_voice_native_clear(&current)==0&&current==NULL);
 }
 pairs++;
}
static void File(const char *directory,const char *name,const char *text)
{char path[N_PATH_BUF];snprintf(path,sizeof(path),"%s%c%s%c%s",fixture_root,PATHSEP,directory,PATHSEP,name);FILE *f=fopen(path,"wb");TEST_ASSERT(f);TEST_ASSERT(fwrite(text,1,strlen(text),f)==strlen(text)&&fclose(f)==0);}
static void Guards(void)
{
 Init(0,"en");RustVoiceNative d=Descriptor(),bad=d;void *out=(void *)0x55;
 TEST_ASSERT(espeak_rs_voice_native_load(NULL,original,0,&out)==EINVAL&&out==(void *)0x55);
 TEST_ASSERT(espeak_rs_voice_native_load(&d,original,0,NULL)==EINVAL);guards++;
 bad.separator=0;TEST_ASSERT(espeak_rs_voice_native_load(&bad,original,0,&out)==EINVAL&&used==0);guards++;
 bad=d;bad.width=1;TEST_ASSERT(espeak_rs_voice_native_load(&bad,original,0,&out)==EINVAL&&used==0);guards++;
 bad=d;bad.dictionary=NULL;TEST_ASSERT(espeak_rs_voice_native_load(&bad,original,0,&out)==EINVAL&&used==0);guards++;
 bad=d;bad.mbrola=NULL;TEST_ASSERT(espeak_rs_voice_native_load(&bad,original,0,&out)==EINVAL&&used==0);guards++;
 bad=d;bad.current=NULL;TEST_ASSERT(espeak_rs_voice_native_load(&bad,original,0,&out)==EINVAL&&used==0);guards++;
 live_descriptor=&d;native_guard=1;TEST_ASSERT(espeak_rs_voice_native_load(&d,original,0,&out)==0&&out==voice);guards++;
 fclose(stream);TEST_ASSERT(espeak_rs_voice_native_clear(&current)==0);
 for(int invalid=1;invalid<=2;invalid++) {
  Init(0,"en");d=Descriptor();bad_environment=invalid;out=(void *)0x55;TEST_ASSERT(espeak_rs_voice_native_load(&d,original,0,&out)==EINVAL&&out==(void *)0x55);guards++;
  fclose(stream);TEST_ASSERT(espeak_rs_voice_native_clear(&current)==0);
 }
 TEST_ASSERT(espeak_rs_voice_native_clear(NULL)==EINVAL);guards++;
}
int main(void)
{
 const char *temp=getenv("TMPDIR");if(!temp)temp=getenv("TMP");if(!temp)temp="/tmp";
 snprintf(fixture_root,sizeof(fixture_root),"%s%cvoice-native-%ld",temp,PATHSEP,(long)PID());TEST_ASSERT(MKDIR(fixture_root)==0);
 char path[N_PATH_BUF];snprintf(path,sizeof(path),"%s%cvoices",fixture_root,PATHSEP);TEST_ASSERT(MKDIR(path)==0);snprintf(path,sizeof(path),"%s%clang",fixture_root,PATHSEP);TEST_ASSERT(MKDIR(path)==0);
 snprintf(path,sizeof(path),"%s%cvoices%c!v",fixture_root,PATHSEP,PATHSEP);TEST_ASSERT(MKDIR(path)==0);
 File("voices","en","name Native One\nlanguage en 5\ngender female 31\npitch 90 120\nfast 400\nformant 2 111 90 110 4\nreplace 1 a b\nmbrola en1 en1_phtrans 16000\ndictionary en\nbad_attr value\npitch 99999999999999999\n");
 File("lang","fr","name Native French\nlanguage fr 7\nphonemes fr\nroughness 3\nbreath 1 2 3\ntone 100 50 800 100\n");
 File("voices","!v/m2","pitch 70 115\nflutter 5\nname Tone Name\n");
 const char *names[]={"en","fr","missing","!v/m2","","EN","en","fr","missing","!v/m2","en"};
 unsigned controls[]={0,1,2,3,4,5,6,7,8,10,16,18,24,26};
 for(unsigned trial=0;trial<2048;trial++) {
  unsigned control=controls[(trial/11)%14];const char *name=names[trial%11];char absolute[N_PATH_BUF];
  if(control&16){snprintf(absolute,sizeof(absolute),"%s%cvoices%c%s",fixture_root,PATHSEP,PATHSEP,name);name=absolute;}
  /* Table mutation is a fresh preparation/finalization effect; input aliases
   * stay within their actual writable40-byte metadata budget. */
  Pair(name,control,(trial*37)%128,(trial/154)%4);
  if(trial<1024)Pair(name,control,((trial*37)%128)|128,0);
 }
 Guards();
#if defined(__APPLE__)
 for(unsigned config=0;config<128;config++)Pair("en",0,config|256,0);
#endif
 const char *files[]={"voices/en","lang/fr","voices/!v/m2"};for(unsigned i=0;i<3;i++){snprintf(path,sizeof(path),"%s/%s",fixture_root,files[i]);TEST_ASSERT(remove(path)==0);}
 snprintf(path,sizeof(path),"%s/voices/!v",fixture_root);TEST_ASSERT(remove(path)==0);snprintf(path,sizeof(path),"%s/voices",fixture_root);TEST_ASSERT(remove(path)==0);snprintf(path,sizeof(path),"%s/lang",fixture_root);TEST_ASSERT(remove(path)==0);TEST_ASSERT(remove(fixture_root)==0);
 printf("%u complete previous-native-C/Rust voice admission/host pairs match real-file status, every primitive argument/order, full acoustic/current/scalar state, fresh aliases and diagnostics; %u native guard groups\n",pairs,guards);return 0;
}
