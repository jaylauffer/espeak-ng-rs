/* Frozen native C host wiring vs owned Rust host with real sources/writer.
 * Algorithms share Rust controllers; independent full-C fixtures remain.
 * SPDX-License-Identifier: GPL-3.0-or-later */
#include "config.h"
#include "test_assert.h"
#include "common.h"
#include "speech.h"
#include "dictionary.h"
#include "translate.h"
#include "synthesize.h"
#include "error.h"
#include "rust_data.h"
#include <espeak-ng/speak_lib.h>
#include <ctype.h>
#include <errno.h>
#include <pthread.h>
#include <stdbool.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <sys/stat.h>
#include <unistd.h>
#include <wctype.h>
_Static_assert(sizeof(RustDictionaryContext)==(sizeof(void *)==8?352:340),"context ABI");
_Static_assert(sizeof(RustNativeDictionaryHost)==(sizeof(void *)==8?136:72),"host ABI");
_Static_assert(sizeof(RustDictionaryEnvironment)==524,"environment ABI");
typedef struct {
 FILE *f_log;char word_phonemes[200];
 int linenum,error_count;bool text_mode;int debug_flag,error_need_dictionary;
 char letterGroupsDefined[95],group_name[13];int group3_ix;
} CompileContext;
static FILE *host_error;
static unsigned char events[262144],expected_events[262144];
static size_t event_length;
static unsigned pairs,guards,stream_callbacks;
static void Event(unsigned kind,const void *bytes,size_t length)
{
 TEST_ASSERT(length<=4096&&event_length+length+3<sizeof(events));
 events[event_length++]=kind;events[event_length++]=length;events[event_length++]=length>>8;
 memcpy(events+event_length,bytes,length);event_length+=length;
}
static const char *EncodePrimitive(const char *input,char *output,size_t width,int *bad)
{Event(1,input,strlen(input)+1);Event(2,&width,sizeof(width));return EncodePhonemesBounded(input,output,width,bad);}
static int TranslatePrimitive(Translator *tr,char *input,WORD_TAB *rows,char *output)
{Event(3,input,strlen(input)+1);return TranslateWord(tr,input,rows,output);}
static int UpperPrimitive(wint_t code) {Event(4,&code,sizeof(code));return iswupper(code);}
static int LowerPrimitive(unsigned int code,Translator *tr) {Event(5,&code,sizeof(code));return towlower2(code,tr);}
static int TransposePrimitive(Translator *tr,char *word) {Event(6,word,strlen(word)+1);return TransposeAlphabet(tr,word);}
static int DigitPrimitive(unsigned int code) {Event(7,&code,sizeof(code));return IsDigit(code);}
static int ReloadPrimitive(Translator *tr,const char *name,int no_error)
{(void)tr;(void)no_error;Event(8,name,strlen(name)+1);return 0;}
static int ErrorPrimitive(espeak_ng_ERROR_CONTEXT *error,int status,const char *path)
{(void)error;Event(9,&status,sizeof(status));Event(10,path,strlen(path)+1);return status;}
#define EncodePhonemesBounded EncodePrimitive
#define TranslateWord TranslatePrimitive
#define iswupper UpperPrimitive
#define towlower2 LowerPrimitive
#define TransposeAlphabet TransposePrimitive
#define IsDigit DigitPrimitive
#define LoadDictionary ReloadPrimitive
#define create_file_error_context ErrorPrimitive
#pragma push_macro("stderr")
#undef stderr
#define stderr host_error
#define espeak_ng_CompileDictionary PreviousPublicEntry
#include "dictionary_host_adapters.inc"
#undef espeak_ng_CompileDictionary
#undef stderr
#pragma pop_macro("stderr")
#undef EncodePhonemesBounded
#undef TranslateWord
#undef iswupper
#undef towlower2
#undef TransposeAlphabet
#undef IsDigit
#undef LoadDictionary
#undef create_file_error_context
static int Encode(void *r,const char *input,char *output,size_t width,int *bad)
{(void)r;return EncodePrimitive(input,output,width,bad)!=NULL;}
static void Translate(char *input) {TranslatePrimitive(translator,input,NULL,NULL);}
static int Case(void *r,uint32_t code,uint32_t *lower)
{(void)r;if(!UpperPrimitive(code))return 0;*lower=LowerPrimitive(code,translator);return 1;}
static int Transpose(void *r,char *word)
{(void)r;return translator->transpose_min>0?TransposePrimitive(translator,word):(int)strlen(word);}
static int Digit(uint32_t code) {return DigitPrimitive(code);}
static void Reload(void *r,const char *name) {(void)r;ReloadPrimitive(translator,name,0);}
static int FileError(void *r,int status,const char *path) {return ErrorPrimitive(r,status,path);}
static void Environment(int rules,RustDictionaryEnvironment *e)
{
 if(rules) {
  for(unsigned i=0;i<256;i++)e->spaces[i]=isspace2((char)i)!=0;
  e->letter_offset=translator->letter_bits_offset;
 } else {
  for(unsigned i=0;i<256;i++)e->alnums[i]=isalnum(i)!=0;
  e->language_text=translator->langopts.textmode;e->listx=translator->langopts.listx;
 }
}
static char root[256],source[512],home[512];
static RustNativeDictionaryHost Descriptor(RustDictionaryContext *c)
{
 RustNativeDictionaryHost d={c,source,"en",home,"en",N_PATH_BUF,'/',0,(char)0xff<0,NULL,
  Environment,Encode,Translate,Case,Transpose,Digit,Reload,FileError};return d;
}
static void Write(const char *name,const char *bytes)
{char path[1024];snprintf(path,sizeof(path),"%s%s",source,name);FILE *f=fopen(path,"wb");TEST_ASSERT(f&&fwrite(bytes,1,strlen(bytes),f)==strlen(bytes)&&fclose(f)==0);}
static void Remove(const char *name)
{char path[1024];snprintf(path,sizeof(path),"%s%s",source,name);TEST_ASSERT(unlink(path)==0||errno==ENOENT);}
static size_t Bytes(FILE *f,unsigned char *bytes,size_t capacity)
{TEST_ASSERT(fflush(f)==0);rewind(f);size_t n=fread(bytes,1,capacity,f);TEST_ASSERT(feof(f));return n;}
typedef struct {int line,count,text,need,group3;unsigned char word[200],letters[95],group[13];} State;
static State OldState(CompileContext *c)
{State s={0};s.line=c->linenum;s.count=c->error_count;s.text=c->text_mode;s.need=c->error_need_dictionary;s.group3=c->group3_ix;memcpy(s.word,c->word_phonemes,200);memcpy(s.letters,c->letterGroupsDefined,95);memcpy(s.group,c->group_name,13);return s;}
static State NewState(RustDictionaryContext *c)
{State s={0};s.line=c->line;s.count=c->count;s.text=c->text_mode;s.need=c->need_dictionary;s.group3=c->group3;memcpy(s.word,c->word_phonemes,200);memcpy(s.letters,c->letters,95);memcpy(s.group,c->group,13);return s;}
static int Previous(CompileContext *c,const RustNativeDictionaryHost *d)
{
 DictionaryIo io={c,NULL};RustDictionaryCompiler driver={&io,d->source,d->name,d->home,d->table,d->width,d->separator,
  dictionary_lists,dictionary_rules,dictionary_log,dictionary_reload,dictionary_errors,dictionary_error};
 return espeak_rs_compile_dictionary(&driver);
}
static void Pair(const char *list,const char *rules,unsigned trial)
{
 static unsigned char expected[3][262144],actual[262144];size_t sizes[3],event_size=0;int status[2];State states[2];
 Remove("en_rules");Remove("en_rules.txt");Remove("en_list");Remove("en_listx");Remove("en_roots");Remove("en_emoji");Remove("en_extra");
 if(rules)Write(trial%2?"en_rules":"en_rules.txt",rules);
 if(list) {Write("en_list",list);if(trial%3==0)Write("en_listx","extra a\n");if(trial%5==0)Write("en_roots","root a\n");}
 for(int lane=0;lane<2;lane++) {
  TEST_ASSERT(espeak_SetVoiceByName(trial%4==3?"tr":"en")==EE_OK);
  translator->langopts.textmode=trial%2;translator->langopts.listx=trial%3;
  translator->letter_bits_offset=trial%4==0?0x100:0;
  FILE *log=tmpfile();host_error=tmpfile();TEST_ASSERT(log&&host_error);event_length=0;
  int error=-1;RustDictionaryContext *c=espeak_rs_dictionary_context_begin(log,host_error,trial,&error);TEST_ASSERT(c&&error==0&&c->debug==(int)(trial&1));
  c->line=19;c->count=trial%7==0?2:0;c->text_mode=trial%2;c->need_dictionary=trial%4;c->group3=37;
  memset(c->letters,trial%2,95);memcpy(c->group,"old",4);
  if(trial%3==0)memcpy(c->word_phonemes,"a",2);
  RustNativeDictionaryHost d=Descriptor(c);CompileContext old={0};old.f_log=log;old.linenum=c->line;old.error_count=c->count;old.text_mode=c->text_mode;old.debug_flag=c->debug;old.error_need_dictionary=c->need_dictionary;old.group3_ix=c->group3;
  memcpy(old.letterGroupsDefined,c->letters,95);memcpy(old.group_name,c->group,13);memcpy(old.word_phonemes,c->word_phonemes,200);
  status[lane]=lane?espeak_rs_dictionary_host_compile(&d):Previous(&old,&d);states[lane]=lane?NewState(c):OldState(&old);
  FILE *output=NULL;char path[1024];snprintf(path,sizeof(path),"%s/en_dict",home);output=fopen(path,"rb");
  FILE *files[]={log,host_error,output};
  for(unsigned k=0;k<3;k++) {
   size_t n=files[k]?Bytes(files[k],actual,sizeof(actual)):0;
   if(!lane) {sizes[k]=n;memcpy(expected[k],actual,n);} else TEST_ASSERT(sizes[k]==n&&memcmp(expected[k],actual,n)==0);
  }
  if(!lane) {event_size=event_length;memcpy(expected_events,events,event_size);}
  else TEST_ASSERT(event_size==event_length&&memcmp(expected_events,events,event_size)==0);
  TEST_ASSERT(espeak_rs_dictionary_context_end(c)==0);fclose(log);fclose(host_error);if(output)fclose(output);unlink(path);
 }
 TEST_ASSERT(status[0]==status[1]&&memcmp(&states[0],&states[1],sizeof(State))==0);pairs++;
}
static void *WrongThread(void *opaque)
{RustNativeDictionaryHost *d=opaque;TEST_ASSERT(espeak_rs_dictionary_host_compile(d)==EAGAIN&&espeak_rs_dictionary_context_end(d->context)==EAGAIN);return NULL;}
static RustNativeDictionaryHost *live;
static RustDictionaryContext *other;
static unsigned char captured[262144];static size_t captured_length;
#if defined(__APPLE__)
static int StreamWrite(void *cookie,const char *bytes,int length)
{
 (void)cookie;TEST_ASSERT(captured_length+(size_t)length<sizeof(captured));memcpy(captured+captured_length,bytes,length);captured_length+=length;
 if(!stream_callbacks++) {
  TEST_ASSERT(espeak_rs_dictionary_context_end(live->context)==EAGAIN);
  TEST_ASSERT(espeak_rs_dictionary_host_compile(live)==EAGAIN);
  RustNativeDictionaryHost secondary=*live;secondary.context=other;TEST_ASSERT(espeak_rs_dictionary_host_compile(&secondary)==EAGAIN);
  /* Both descriptor and all string inputs were already snapshotted. Native
   * counters/debug/settings are intentionally read fresh after FILE effects. */
  memset((char *)live->source,'x',32);((char *)live->source)[32]=0;
  memset((char *)live->home,'y',32);((char *)live->home)[32]=0;
  memset((char *)live->name,'z',4);((char *)live->name)[4]=0;
  live->context->count=5;live->context->line=71;live->context->debug=1;
  translator->langopts.listx=0;translator->letter_bits_offset=0x100;
  live->environment=NULL;live->encode=NULL;
 }
 return length;
}
#endif
static void Guards(void)
{
 int error=99;TEST_ASSERT(!espeak_rs_dictionary_context_begin(NULL,stderr,0,&error)&&error==EINVAL);guards++;
 TEST_ASSERT(!espeak_rs_dictionary_context_begin(stderr,stderr,0,NULL));TEST_ASSERT(espeak_rs_dictionary_context_end(NULL)==0);guards++;
 FILE *log=tmpfile(),*errors=tmpfile();TEST_ASSERT(log&&errors);
 RustDictionaryContext *c=espeak_rs_dictionary_context_begin(log,errors,7,&error);TEST_ASSERT(c&&error==0&&c->debug==1);
 RustNativeDictionaryHost d=Descriptor(c),invalid=d;State before=NewState(c);
 TEST_ASSERT(espeak_rs_dictionary_host_compile(NULL)==EINVAL);invalid.context=NULL;TEST_ASSERT(espeak_rs_dictionary_host_compile(&invalid)==EINVAL);guards++;
 invalid=d;invalid.width=1;TEST_ASSERT(espeak_rs_dictionary_host_compile(&invalid)==EINVAL);invalid.width=4097;TEST_ASSERT(espeak_rs_dictionary_host_compile(&invalid)==EINVAL);guards++;
 invalid=d;invalid.environment=NULL;TEST_ASSERT(espeak_rs_dictionary_host_compile(&invalid)==EINVAL);invalid=d;invalid.encode=NULL;TEST_ASSERT(espeak_rs_dictionary_host_compile(&invalid)==EINVAL);guards++;
 char input[4096];memset(input,'x',sizeof(input));invalid=d;invalid.source=input;event_length=0;TEST_ASSERT(espeak_rs_dictionary_host_compile(&invalid)==ENS_COMPILE_ERROR&&event_length==0);State after=NewState(c);TEST_ASSERT(memcmp(&before,&after,sizeof(State))==0);guards++;
 pthread_t thread;TEST_ASSERT(pthread_create(&thread,NULL,WrongThread,&d)==0&&pthread_join(thread,NULL)==0);guards++;
 /* Real missing source reports file errno and retains context for another call. */
 invalid=d;invalid.name="absent";TEST_ASSERT(espeak_rs_dictionary_host_compile(&invalid)==ENOENT);guards++;
 Write("en_rules",".group a\na a\n");Remove("en_rules.txt");Write("en_list","word a\n");
 invalid=d;invalid.home="/missing/espeak-dictionary-host-root";TEST_ASSERT(espeak_rs_dictionary_host_compile(&invalid)==ENOENT);guards++;
 /* Descriptor/name overlap the owner's quiescent word storage: snapshot first. */
 memcpy(c->word_phonemes,"en",3);d.name=c->word_phonemes;TEST_ASSERT(espeak_rs_dictionary_host_compile(&d)==0);guards++;
 TEST_ASSERT(espeak_rs_dictionary_context_end(c)==0);TEST_ASSERT(fputc('x',log)!=EOF&&fputc('x',errors)!=EOF);fclose(log);fclose(errors);guards++;
#if defined(__APPLE__)
 Write("en_rules",".group \304\200\n\304\200 a\n");Write("en_listx","extra b\n");
 translator->langopts.listx=1;translator->letter_bits_offset=0;
 FILE *stream=funopen(NULL,NULL,StreamWrite,NULL,NULL);errors=tmpfile();TEST_ASSERT(stream&&errors&&setvbuf(stream,NULL,_IONBF,0)==0);
 c=espeak_rs_dictionary_context_begin(stream,errors,0,&error);other=espeak_rs_dictionary_context_begin(stream,errors,0,&error);TEST_ASSERT(c&&other);
 char saved_source[4096],saved_home[4096],name[4096]="en";strcpy(saved_source,source);strcpy(saved_home,home);
 d=Descriptor(c);d.source=saved_source;d.home=saved_home;d.name=name;live=&d;captured_length=0;stream_callbacks=0;
 int status=espeak_rs_dictionary_host_compile(&d);
 TEST_ASSERT(status==ENS_COMPILE_ERROR&&c->count==5&&c->line==3&&c->group3==1&&stream_callbacks>0);
 captured[captured_length]=0;char *listx=strstr((char *)captured,"en_listx'\n"),*list=strstr((char *)captured,"en_list'\n");TEST_ASSERT(listx&&list&&listx<list);
 TEST_ASSERT(espeak_rs_dictionary_context_end(c)==0&&espeak_rs_dictionary_context_end(other)==0);TEST_ASSERT(fputc('x',stream)!=EOF&&fputc('x',errors)!=EOF);fclose(stream);fclose(errors);guards++;
#endif
}
int main(void)
{
 TEST_ASSERT(espeak_Initialize(AUDIO_OUTPUT_SYNCHRONOUS,0,NULL,0)>0);TEST_ASSERT(espeak_SetVoiceByName("en")==EE_OK);
 snprintf(root,sizeof(root),"/tmp/espeak-dictionary-host-%ld",(long)getpid());TEST_ASSERT(mkdir(root,0700)==0);snprintf(source,sizeof(source),"%s/source/",root);snprintf(home,sizeof(home),"%s/home",root);TEST_ASSERT(mkdir(source,0700)==0&&mkdir(home,0700)==0);
 const char *lists[]={"word a\nCAPS b\nMixed a\n","word word $text\n","word a $unknown\nU+41 a\n","word a\nword b\n","(one two) a\n_foo a\n","été a\nÄÖ b\nİ a\n","word ?\nnear a||b\n",NULL};
 const char *rules[]={".L01 a e\n.group a\na a\na(L01 b\n",".group 9\n9 a\n२ a\n",".group a\n?1 _A)a(B a\n?!2 a(P2 b\n",".group abc\nabc a\n.group ab\nab b\n",".group a\na($p_alt6 a\na(Sep2 a\n",".L00 x z\n.group α\nα a\n",".group a\na ?\na a",NULL};
 for(unsigned trial=0;trial<128;trial++)Pair(lists[trial%8],rules[(trial/8)%8],trial);
 Guards();Remove("en_rules");Remove("en_rules.txt");Remove("en_list");Remove("en_listx");Remove("en_roots");Remove("en_emoji");Remove("en_extra");char path[1024];snprintf(path,sizeof(path),"%s/en_dict",home);unlink(path);TEST_ASSERT(rmdir(source)==0&&rmdir(home)==0&&rmdir(root)==0);espeak_Terminate();
 printf("%u old-C/Rust native dictionary host pairs match payload, status, both logs, primitive order/arguments and complete state; %u lifetime/thread/malformed/overlap/errno guards, %u actual macOS stream callbacks\n",pairs,guards,stream_callbacks);return 0;
}
