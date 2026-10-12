/* Previous native C callback wiring vs complete Rust native host, using actual
 * owned contexts, source sessions, table/program/resource leases and output I/O.
 * Shared generic algorithms are independently covered by their full-C fixtures.
 * SPDX-License-Identifier: GPL-3.0-or-later */
#include "speech.h"
#include "synthesize.h"
#include "rust_compiler_host.h"
#include "rust_compiler_diagnostic.h"
#include "rust_compiler_source.h"
#include "rust_compiler_items.h"
#include "rust_compiler_program.h"
#include "rust_compiler_definition.h"
#include "rust_compiler_files.h"
#include "rust_compiler_resource.h"
#include "rust_compiler_output.h"
#include "test_assert.h"
#include <ctype.h>
#include <errno.h>
#include <pthread.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <sys/stat.h>
#ifdef _WIN32
#include <direct.h>
#include <process.h>
#define mkdir_path(p) _mkdir(p)
#define rmdir_path(p) _rmdir(p)
#define process_id _getpid
#else
#include <unistd.h>
#define mkdir_path(p) mkdir(p,0700)
#define rmdir_path(p) rmdir(p)
#define process_id getpid
#endif
typedef RustCompilerContext CompileContext;
typedef int CompilerInput;
enum {tSTRING=2};
static FILE *host_log;
#pragma push_macro("stderr")
#undef stderr
#define stderr host_log
#include "compiler_host_reference.inc"
#undef stderr
#pragma pop_macro("stderr")
_Static_assert(sizeof(RustNativeCompilerHost)==(sizeof(void *)==8?56:28),"host ABI");
static int Rate(void) {return samplerate;}
static void FileError(int value) {errno=value;}
static int Fallback(void *context,const unsigned char *bytes,size_t length)
{(void)context;(void)bytes;(void)length;return 0;}
static RustNativeCompilerHost Descriptor(CompileContext *c)
{RustNativeCompilerHost d={c,N_PATH_BUF,host_log,Rate,RustWavegenState,CompilerStatusMessage,FileError};return d;}
static const char *outputs[]={"phondata-manifest","phondata","phonindex","phontab","compile_prog_log","errors","resources"};
static char root[256],source[512];
static unsigned pairs,guard_callbacks;
typedef struct {
 int table_status,program_status,tables,codes,line,references,duplicates,frames,errors,kind,terminator,definition;
 unsigned char filename[80],item[256];PHONEME_TAB row,current[256];
} State;
static void Snapshot(CompileContext *c,State *s)
{
 memset(s,0,sizeof(*s));s->table_status=c->table_status;s->program_status=c->program_status;
 s->tables=c->n_phoneme_tabs;s->codes=c->n_phcodes;s->line=c->linenum;
 s->references=c->count_references;s->duplicates=c->duplicate_references;s->frames=c->count_frames;
 s->errors=c->error_count;s->kind=c->item_type;s->terminator=c->item_terminator;s->definition=c->definition_phoneme;
 memcpy(s->filename,c->current_fname,80);memcpy(s->item,c->item_string,256);s->row=c->definition_row;
 if(c->phoneme_tab2)memcpy(s->current,c->phoneme_tab2,sizeof(s->current));
}
static void Write(const char *name,const unsigned char *bytes,size_t length)
{char path[1024];snprintf(path,sizeof(path),"%s/%s",source,name);FILE *f=fopen(path,"wb");TEST_ASSERT(f&&fwrite(bytes,1,length,f)==length&&fclose(f)==0);}
static void Text(const char *name,const char *text) {Write(name,(const unsigned char *)text,strlen(text));}
static size_t Bytes(const char *path,unsigned char *bytes,size_t capacity)
{FILE *f=fopen(path,"rb");TEST_ASSERT(f);size_t n=fread(bytes,1,capacity,f);TEST_ASSERT(feof(f)&&fclose(f)==0);return n;}
static void Pair(const char *text,unsigned trial)
{
 static unsigned char expected[7][200000],actual[200000];size_t sizes[7];State states[2],before[2];
 Text("phonemes",text);
 /* Serial complete output sets respect the unchanged five-buffer writer pool. */
 for(int lane=0;lane<2;lane++) {
  char directory[512],path[1024];snprintf(directory,sizeof(directory),"%s/lane%d",root,lane);
  snprintf(path,sizeof(path),"%s/errors",directory);FILE *errors=fopen(path,"wb");TEST_ASSERT(errors);
  snprintf(path,sizeof(path),"%s/resources",directory);host_log=fopen(path,"wb");TEST_ASSERT(host_log);
  int status=-1;CompileContext *c=espeak_rs_compiler_context_begin(source,N_PATH_BUF,&status);TEST_ASSERT(c&&status==0);
  c->table_owner=espeak_rs_compiler_tables_begin();c->program_owner=espeak_rs_compiler_program_begin();TEST_ASSERT(c->table_owner&&c->program_owner);
  RustCompilerOutput output={source,directory,N_PATH_BUF,123,22050,NULL,Fallback};RustCompilerOutputResult opened={0};
  TEST_ASSERT(espeak_rs_compiler_output_begin(&output,&opened)==0&&opened.owner);
  c->output_owner=opened.owner;c->output_mask=opened.mask;c->f_errors=errors;strcpy(c->current_fname,"phonemes");
  c->count_references=77;c->duplicate_references=88;c->count_frames=99;c->linenum=37;
  snprintf(path,sizeof(path),"%s/phonemes",source);RustCompilerSourceOpen input={c,path,N_PATH_BUF,0,0,SourceClasses};
  c->f_in=espeak_rs_compiler_context_source(&input,&status);TEST_ASSERT(c->f_in&&status==0);
  RustNativeCompilerHost d=Descriptor(c);
  if(lane) {TEST_ASSERT(espeak_rs_compiler_host_tables(&d,RUST_TABLE_START,"base",0)==0);espeak_rs_compiler_host_files(&d);}
  else {StartPhonemeTable(c,"base");CompilePhonemeFiles(c);}
  /* Inspect before packing too: errors must not change status/view precedence. */
  Snapshot(c,&before[lane]);if(lane){espeak_rs_compiler_host_tables(&d,RUST_TABLE_FINISH,NULL,0);espeak_rs_compiler_host_tables(&d,RUST_TABLE_PACK,NULL,0);}
  else {EndPhonemeTable(c);WritePhonemeTables(c);}
  Snapshot(c,&states[lane]);
  if(lane)TEST_ASSERT(memcmp(&before[0],&before[1],sizeof(State))==0);
  uint64_t tables=c->table_owner,program=c->program_owner,out=c->output_owner,resource=c->resource_owner;
  TEST_ASSERT(espeak_rs_compiler_context_end(c)==0);TEST_ASSERT(espeak_rs_compiler_tables_end(tables)!=0&&espeak_rs_compiler_program_end(program)!=0&&espeak_rs_compiler_output_end(out)!=0);
  if(resource)TEST_ASSERT(espeak_rs_compiler_resources_end(resource)!=0);
  TEST_ASSERT(fclose(errors)==0&&fclose(host_log)==0);
  for(unsigned f=0;f<7;f++) {
   /* The log is produced in source; other payloads in the output directory. */
   snprintf(path,sizeof(path),"%s/%s",f==4?source:directory,outputs[f]);
   if(!lane)sizes[f]=Bytes(path,expected[f],sizeof(expected[f]));
   else {size_t n=Bytes(path,actual,sizeof(actual));if(n!=sizes[f]||memcmp(expected[f],actual,n)!=0){fprintf(stderr,"host trial %u output %s differs\n",trial,outputs[f]);TEST_ASSERT(0);}}
  }
 }
 TEST_ASSERT(memcmp(&states[0],&states[1],sizeof(State))==0);pairs++;
}
static RustNativeCompilerHost *mutable_host;
static CompileContext *guard_context;
static void GuardedMessage(int status,char *bytes,size_t length)
{
 RustNativeCompilerHost copy=*mutable_host;
 TEST_ASSERT(espeak_rs_compiler_host_files(&copy)==EAGAIN);
 TEST_ASSERT(espeak_rs_compiler_host_tables(&copy,RUST_TABLE_PACK,NULL,0)==EAGAIN);
 TEST_ASSERT(espeak_rs_compiler_context_end(guard_context)==EAGAIN);
 mutable_host->context=NULL;mutable_host->message=NULL;mutable_host->rate=NULL;mutable_host->generator=NULL;
 espeak_ng_GetStatusCodeMessage(status,bytes,length);guard_callbacks++;
}
static void *WrongThread(void *opaque)
{RustNativeCompilerHost *d=opaque;TEST_ASSERT(espeak_rs_compiler_host_files(d)==EAGAIN);TEST_ASSERT(espeak_rs_compiler_host_tables(d,RUST_TABLE_FINISH,NULL,0)==EAGAIN);return NULL;}
static void Guards(void)
{
 int status=42;CompileContext *c=espeak_rs_compiler_context_begin(source,N_PATH_BUF,&status);TEST_ASSERT(c&&status==0);
 FILE *errors=tmpfile();TEST_ASSERT(errors);c->f_errors=errors;host_log=errors;guard_context=c;
 RustNativeCompilerHost d=Descriptor(c),bad=d;
 TEST_ASSERT(espeak_rs_compiler_host_files(NULL)==EINVAL);bad.width=1;TEST_ASSERT(espeak_rs_compiler_host_files(&bad)==EINVAL);
 bad=d;bad.message=NULL;TEST_ASSERT(espeak_rs_compiler_host_tables(&bad,RUST_TABLE_PACK,NULL,0)==EINVAL);
 TEST_ASSERT(espeak_rs_compiler_host_tables(&d,7,NULL,0)==EINVAL&&c->program_status==0&&c->table_status==0);
 pthread_t thread;TEST_ASSERT(pthread_create(&thread,NULL,WrongThread,&d)==0&&pthread_join(thread,NULL)==0);
 /* Stale table admission takes the guarded status path; descriptor mutation
  * cannot revoke its copied configuration or consume the active context. */
 d.message=GuardedMessage;mutable_host=&d;
 TEST_ASSERT(espeak_rs_compiler_host_tables(&d,RUST_TABLE_PACK,NULL,0)==EINVAL);
 TEST_ASSERT(guard_callbacks==1&&c->table_status==EINVAL&&c->error_count==1);
 d=Descriptor(c);c->table_owner=espeak_rs_compiler_tables_begin();TEST_ASSERT(c->table_owner);
 strcpy(c->item_string,"base");TEST_ASSERT(espeak_rs_compiler_host_tables(&d,RUST_TABLE_START,c->item_string,0)==0);
 TEST_ASSERT(c->n_phoneme_tabs==1&&c->table_status==EINVAL); // Historical sticky first error remains.
 char path[1024];snprintf(path,sizeof(path),"%s/phonemes",source);
 RustCompilerSourceOpen input={c,path,N_PATH_BUF,0,0,SourceClasses};
 c->f_in=espeak_rs_compiler_context_source(&input,&status);TEST_ASSERT(c->f_in&&status==0);
 c->program_status=5;TEST_ASSERT(espeak_rs_compiler_host_files(&d)==5&&c->program_status==5);
 TEST_ASSERT(espeak_rs_compiler_context_end(c)==0&&fclose(errors)==0);
 unsigned char space[256],digit[256];espeak_rs_compiler_host_classes(space,digit);
 for(int i=0;i<256;i++)TEST_ASSERT(space[i]==(isspace(i)!=0)&&digit[i]==(isdigit(i)!=0));
}
int main(void)
{
 TEST_ASSERT(espeak_Initialize(AUDIO_OUTPUT_RETRIEVAL,0,NULL,0)==22050&&espeak_SetVoiceByName("en")==EE_OK);
 snprintf(root,sizeof(root),"compiler-host-%ld.tmp",(long)process_id());TEST_ASSERT(mkdir_path(root)==0);
 snprintf(source,sizeof(source),"%s/source",root);TEST_ASSERT(mkdir_path(source)==0);
 char path[1024];for(int lane=0;lane<2;lane++){snprintf(path,sizeof(path),"%s/lane%d",root,lane);TEST_ASSERT(mkdir_path(path)==0);}
 Text("shared","phoneme b\nvcd alv\nstp\nlength 80\nendphoneme\n");Text("empty","");
 Text("nested","include shared\ninclude empty\n");
 unsigned char wave[108]={0};memcpy(wave,"RIFF",4);for(int j=0;j<4;j++){wave[24+j]=22050>>(j*8);wave[28+j]=44100>>(j*8);wave[40+j]=64>>(j*8);}for(int j=44;j<108;j++)wave[j]=(unsigned char)(j*17);
 Write("sound",wave,sizeof(wave));Text("envelope","ENVELOPE\nignored\n0 20 1\n50 80 1\n100 0 1\n");
 static const char *cases[]={
  "", "phoneme a\nvwl\nlength 140\nendphoneme\n", "include nested\nphoneme a\nvwl\nendphoneme\n",
  "include missing\nphoneme a\nvwl\nendphoneme\n", "include shared\ninclude shared\n",
  "procedure p\nContinue\nendprocedure\nphoneme a\nvwl\nCALL p\nendphoneme\n",
  "procedure p\nContinue\nendprocedure\nprocedure p\nRETURN\nendprocedure\n",
  "phoneme b\nvcd alv\nstp\nContinue\nendphoneme\nphoneme a\nCALL b\nendphoneme\n",
  "phoneme a\nvwl\nendphoneme\nphonemetable child base\nphoneme z\nimport_phoneme base/a\nendphoneme\n",
  "phonemetable child missing\nphoneme a\nvwl\nendphoneme\n", "phoneme z\nimport_phoneme missing/a\nendphoneme\n",
  "phoneme a\nvwl\nIF NOT nextVowel(isVowel) OR PreVoicing THEN\nContinue\nELIF thisPh(isWordEnd) THEN\nLengthAdd -9\nELSE\nPauseBefore 12\nENDIF\nendphoneme\n",
  "phoneme a\nvwl\nIF thisPh(isVowel) THEN\nIF nextPh(a) THEN\nContinue\nELSE\nLengthAdd 9\nENDIF\nELSE\nPauseAfter 5\nENDIF\nendphoneme\n",
  "phoneme a\nvcd alv\nstp\nVowelin len=42 rms=30 f1=1 f2=1500 -200 300 f3=-250 80 brk rate glstop lenadd f4 gpaus colr=2 amp=15\nVowelout len=36 rms=16\nendphoneme\n",
  "phoneme a\nvwl\nELSE\nELIF\nENDIF\nendphoneme\n", "phoneme a\nvwl\nIF thisPh(isVowel) THEN\nContinue\nendphoneme\n",
  "phoneme a\nvwl\nTone(120,80,envelope,envelope)\nWAV(sound,100)\nWAV(sound,100)\nendphoneme\n",
  "phoneme a\nvwl\nFMT(NULL,20)\nWAV(DFT,30)\nTone(120,80,NULL,NULL)\nendphoneme\n",
  "phoneme a\nvwl\nWAV(missing,100)\nendphoneme\n", "phoneme a\nvwl\nunknownfeature\nendphoneme\n",
  "phoneme a\nvwl\nlength -12\nendphoneme\n", "phoneme a\nvwl\nlength 12abc\nendphoneme\n",
  "phoneme a\nvwl\nlength 2147483648\nendphoneme\n", "phoneme a\nvwl\nlengthmod 10\nendphoneme\n",
  "phoneme a\nvwl\nipa α|β\nendphoneme\n", "phoneme a\nvwl\nipa U+ZZZA\nendphoneme\n",
  "phoneme a\ndelete_phoneme\nendphoneme\n", "phoneme a\nvwl\ninclude shared\n",
  "phoneme a\nvwl\n", "unexpected\nphoneme a\nendphoneme\n", "\xef\xbb\xbf\nphoneme a\nvwl\nendphoneme\n"
 };
 char text[32000];for(unsigned i=0;i<124;i++){
  snprintf(text,sizeof(text),"%s%s",i%4==0?"// copied source comment\n":"",cases[i%(sizeof(cases)/sizeof(cases[0]))]);Pair(text,i);
 }
 /* Shared bounded-controller refusals must retain identical native publication. */
 strcpy(text,"phoneme a\nvwl\n");for(unsigned i=0;i<2500;i++)strcat(text,"Continue\n");strcat(text,"endphoneme\n");Pair(text,124);
 text[0]=0;for(unsigned i=0;i<51;i++){char p[128];snprintf(p,sizeof(p),"procedure p%u\nContinue\nendprocedure\n",i);strcat(text,p);}Pair(text,125);
 text[0]=0;for(unsigned i=0;i<150;i++){char p[128];snprintf(p,sizeof(p),"phonemetable t%03u _\n",i);strcat(text,p);}Pair(text,126);
 for(unsigned i=0;i<14;i++){char name[32],child[128];snprintf(name,sizeof(name),"depth%u",i);snprintf(child,sizeof(child),i<13?"include depth%u\n":"phoneme x\nvwl\nendphoneme\n",i+1);Text(name,child);}Pair("include depth0\n",127);
 Guards();
 for(unsigned i=0;i<14;i++){snprintf(path,sizeof(path),"%s/depth%u",source,i);TEST_ASSERT(unlink(path)==0);}
 const char *inputs[]={"shared","empty","nested","sound","envelope","phonemes","compile_prog_log"};
 for(unsigned i=0;i<sizeof(inputs)/sizeof(inputs[0]);i++){snprintf(path,sizeof(path),"%s/%s",source,inputs[i]);TEST_ASSERT(unlink(path)==0);}
 for(int lane=0;lane<2;lane++){for(unsigned i=0;i<7;i++)if(i!=4){snprintf(path,sizeof(path),"%s/lane%d/%s",root,lane,outputs[i]);TEST_ASSERT(unlink(path)==0);}snprintf(path,sizeof(path),"%s/lane%d",root,lane);TEST_ASSERT(rmdir_path(path)==0);}
 TEST_ASSERT(rmdir_path(source)==0&&rmdir_path(root)==0&&espeak_Terminate()==EE_OK);
 printf("%u previous-C/Rust complete native host pairs; all five outputs, diagnostics, resource logs, rows/tokens/statuses and lease teardown match; %u guarded callback, thread/admission/overlap/sticky guards pass\n",pairs,guard_callbacks);return 0;
}
