/* Actual native C engine primitives with real Rust compiler leases and files.
 * SPDX-License-Identifier: GPL-3.0-or-later */
#include <espeak-ng/espeak_ng.h>
#include "speech.h"
#include "rust_compiler_native.h"
#include "rust_compiler_source.h"
#include "rust_compiler_tables.h"
#include "rust_compiler_program.h"
#include "rust_compiler_output.h"
#include "test_assert.h"
#include <ctype.h>
#include <errno.h>
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
#if UINTPTR_MAX==UINT64_MAX
_Static_assert(sizeof(RustNativePhonemeCompiler)==120,"native descriptor");
#endif
typedef RustCompilerContext CompileContext;
static int samplerate;
static void *voice=(void *)1;
static FILE *retained_log;
static char directory[N_PATH_BUF],source[N_PATH_BUF],destination[N_PATH_BUF];
static unsigned events[64],used,close_count,episodes,fallback_count;
static int fail_voice,table_status,program_status,errors,file_status;
static uint64_t tables,program,output;
static char error_path[4096];
static RustNativePhonemeCompiler *guard;
static char *mutable_source;
static void LoadPhData(void *a,void *b){TEST_ASSERT(!a&&!b);}
static void *LoadVoice(const char *name,int mode){TEST_ASSERT(!*name&&mode==8);return fail_voice?NULL:voice;}
static void WavegenInit(long rate,int zero){TEST_ASSERT(rate==22050&&zero==0);samplerate=24000;}
static void WavegenSetVoice(void *v){TEST_ASSERT(v==voice);}
static void WavegenFini(void){}
static void StartPhonemeTable(CompileContext *c,const char *name){TEST_ASSERT(!strcmp(name,"base")&&c->n_phoneme_tabs==0);c->n_phoneme_tabs=1;}
static void CompilePhonemeFiles(CompileContext *c)
{
 TEST_ASSERT(c->table_owner&&c->program_owner&&c->output_owner&&c->output_mask==31);
 TEST_ASSERT(c->f_errors==retained_log&&c->f_in==1&&!strcmp(c->current_fname,"phonemes"));
 char line[32];TEST_ASSERT(espeak_rs_compiler_source_line(c->source_pool,c->f_in,(unsigned char *)line,sizeof(line))==1&&!strcmp(line,"fixture source\n"));
 output=c->output_owner;unsigned char bytes[2]={77,88};TEST_ASSERT(espeak_rs_compiler_output_write(output,RUST_OUTPUT_INDEX,bytes,2)==0);
 c->count_references=19;c->duplicate_references=7;c->error_count=errors;c->table_status=table_status;c->program_status=program_status;
}
static void EndPhonemeTable(CompileContext *c){TEST_ASSERT(c->n_phoneme_tabs==1);}
static void WritePhonemeTables(CompileContext *c){TEST_ASSERT(c->output_owner==output);}
static void error_from_status(CompileContext *c,int status,const char *name){TEST_ASSERT(status&&(status==table_status||status==program_status)&&!strcmp(name,"compiler output"));c->error_count++;fprintf(c->f_errors,"output error %d\n",status);}
static int IgnoreClose(FILE *f){TEST_ASSERT(f==retained_log);close_count++;return 0;}
static int create_file_error_context(espeak_ng_ERROR_CONTEXT *unused,int status,const char *path){(void)unused;file_status=status;snprintf(error_path,sizeof(error_path),"%s",path);return status;}
#define fclose IgnoreClose
#include "compiler_driver_primitives.inc"
#undef fclose
static int Engine(void *opaque,RustCompilerContext *c,int op,int value)
{
 TEST_ASSERT(used<64);events[used++]=(unsigned)op;
 if(op==RUST_COMPILER_RATE){
  tables=c->table_owner;program=c->program_owner;TEST_ASSERT(tables&&program);
  if(guard){
   TEST_ASSERT(espeak_rs_compile_phonemes_native(guard)==11);
   guard->engine=NULL;guard->classes=NULL;guard->version=999;mutable_source[0]='X';
  }
 }
 return PhonemePrimitive(opaque,c,op,value);
}
static int Fallback(void *opaque,const unsigned char *bytes,size_t length){TEST_ASSERT(opaque&&bytes&&length);fallback_count++;return 0;}
static void Classes(unsigned char *spaces,unsigned char *digits){for(int i=0;i<256;i++){spaces[i]=isspace(i)!=0;digits[i]=isdigit(i)!=0;}}
static void Write(const char *name,const char *bytes){FILE *f=fopen(name,"wb");TEST_ASSERT(f&&fwrite(bytes,1,strlen(bytes),f)==strlen(bytes)&&fclose(f)==0);}
static int Run(int expected,unsigned trial,int snapshot)
{
 FILE *log=tmpfile();TEST_ASSERT(log);retained_log=log;used=close_count=fallback_count=0;tables=program=output=0;file_status=0;error_path[0]=0;
 unsigned char alpha[256],spaces[256];for(int i=0;i<256;i++){alpha[i]=isalpha(i)!=0;spaces[i]=isspace(i)!=0;}
 char copied_source[N_PATH_BUF];snprintf(copied_source,sizeof(copied_source),"%s",source);
 PhonemeEffects effects={22050,log,NULL};
 RustNativePhonemeCompiler d={&effects,directory,copied_source,destination,alpha,spaces,N_PATH_BUF,0,0,123,log,Engine,PhonemeLog,PhonemeError,Fallback,Classes};
 guard=snapshot?&d:NULL;mutable_source=copied_source;
 if(trial==0){size_t width=d.width;d.width=1;TEST_ASSERT(espeak_rs_compile_phonemes_native(&d)==EINVAL&&used==0);d.width=width;TEST_ASSERT(espeak_rs_compile_phonemes_native(NULL)==EINVAL);}
 TEST_ASSERT(espeak_rs_compile_phonemes_native(&d)==expected);guard=NULL;
 if(tables){TEST_ASSERT(espeak_rs_compiler_tables_end(tables)!=0);}
 if(program){TEST_ASSERT(espeak_rs_compiler_program_end(program)!=0);}
 if(output){TEST_ASSERT(espeak_rs_compiler_output_end(output)!=0);}
 int e=42;unsigned char digits[256];Classes(spaces,digits);void *session=espeak_rs_compiler_sources_begin(spaces,digits,&e);TEST_ASSERT(session&&e==0);espeak_rs_compiler_sources_end(session);
 TEST_ASSERT(fflush(log)==0);rewind(log);char text[8192];size_t n=fread(text,1,sizeof(text)-1,log);text[n]=0;
 if(output){
  TEST_ASSERT(close_count==1&&strstr(text,"\nRefs 19,  Reused 7\n"));
  TEST_ASSERT(events[0]==RUST_COMPILER_RATE&&events[1]==RUST_COMPILER_RELOAD&&events[2]==RUST_COMPILER_VOICE&&events[3]==RUST_COMPILER_WAVE_INIT);
  TEST_ASSERT(events[4]==RUST_COMPILER_OUTPUT_OPEN&&events[5]==RUST_COMPILER_BASE_TABLE&&events[6]==RUST_COMPILER_COMPILE&&events[7]==RUST_COMPILER_END_TABLE&&events[8]==RUST_COMPILER_WRITE_TABLES);
  if(table_status||program_status){TEST_ASSERT(used==11&&events[9]==RUST_COMPILER_OUTPUT_ERROR&&events[10]==RUST_COMPILER_CLOSE_LOG);}
  else{TEST_ASSERT(used==12&&events[9]==RUST_COMPILER_RELOAD&&events[10]==RUST_COMPILER_WAVE_FINI&&events[11]==RUST_COMPILER_CLOSE_LOG&&strstr(text,"Compiled phonemes:"));}
  char name[N_PATH_BUF];snprintf(name,sizeof(name),"%s/phonindex",destination);FILE *f=fopen(name,"rb");unsigned char bytes[6];TEST_ASSERT(f&&fread(bytes,1,6,f)==6&&bytes[4]==77&&bytes[5]==88&&fgetc(f)==EOF&&fclose(f)==0);
  snprintf(name,sizeof(name),"%s/phondata",destination);f=fopen(name,"rb");unsigned char header[8];TEST_ASSERT(f&&fread(header,1,8,f)==8&&header[0]==123&&header[1]==0&&header[2]==0&&header[3]==0&&header[4]==(24000&255)&&header[5]==(24000>>8)&&header[6]==0&&header[7]==0&&fclose(f)==0);
 }
 else{TEST_ASSERT(close_count==0);}
 TEST_ASSERT(fclose(log)==0);retained_log=NULL;episodes++;(void)trial;return expected;
}
int main(void)
{
 const char *temporary=getenv("TMPDIR");
#ifdef _WIN32
 if(!temporary){temporary=getenv("TEMP");}
#endif
 if(!temporary){temporary="/tmp";}
 snprintf(directory,sizeof(directory),"%s/espeak-native-host-%ld",temporary,(long)process_id());TEST_ASSERT(mkdir_path(directory)==0);
 snprintf(source,sizeof(source),"%s/source",directory);snprintf(destination,sizeof(destination),"%s/output",directory);TEST_ASSERT(mkdir_path(source)==0&&mkdir_path(destination)==0);
 char name[N_PATH_BUF];snprintf(name,sizeof(name),"%s/phonemes",source);Write(name,"fixture source\n");snprintf(name,sizeof(name),"%s/phondata-manifest",directory);Write(name,"W 12 wave.wav\n");
 for(unsigned i=0;i<200;i++){
  errors=i%4;table_status=i%5==0?13:0;program_status=i%7==0?22:0;
  int status=table_status?table_status:(program_status?program_status:(errors?ENS_COMPILE_ERROR:0));Run(status,i,i==1);
 }
 errors=table_status=program_status=0;fail_voice=1;Run(ENS_VOICE_NOT_FOUND,0,0);fail_voice=0;
 snprintf(name,sizeof(name),"%s/phonemes",source);TEST_ASSERT(remove(name)==0);Run(ENOENT,0,0);TEST_ASSERT(file_status==ENOENT&&strstr(error_path,"phonemes"));Write(name,"fixture source\n");
 char saved[N_PATH_BUF];snprintf(saved,sizeof(saved),"%s",destination);snprintf(destination,sizeof(destination),"%s/absent",directory);Run(ENOENT,0,0);TEST_ASSERT(file_status==ENOENT&&strstr(error_path,"phondata")&&fallback_count==1);snprintf(destination,sizeof(destination),"%s",saved);
 uint64_t owners[4];for(int i=0;i<4;i++){owners[i]=espeak_rs_compiler_tables_begin();TEST_ASSERT(owners[i]);}Run(ENOMEM,0,0);TEST_ASSERT(used==0);for(int i=0;i<4;i++){TEST_ASSERT(espeak_rs_compiler_tables_end(owners[i])==0);}
 for(int i=0;i<4;i++){owners[i]=espeak_rs_compiler_program_begin();TEST_ASSERT(owners[i]);}Run(ENOMEM,0,0);TEST_ASSERT(used==0);for(int i=0;i<4;i++){TEST_ASSERT(espeak_rs_compiler_program_end(owners[i])==0);}
 /* Failed program admission must have returned its already-created table slot. */
 for(int i=0;i<4;i++){owners[i]=espeak_rs_compiler_tables_begin();TEST_ASSERT(owners[i]);}for(int i=0;i<4;i++){TEST_ASSERT(espeak_rs_compiler_tables_end(owners[i])==0);}
 Run(0,0,0);
 const char *names[]={"phondata-manifest","phondata","phonindex","phontab"};for(unsigned i=0;i<4;i++){snprintf(name,sizeof(name),"%s/%s",destination,names[i]);TEST_ASSERT(remove(name)==0);}
 TEST_ASSERT(rmdir_path(destination)==0);snprintf(name,sizeof(name),"%s/compile_prog_log",source);TEST_ASSERT(remove(name)==0);snprintf(name,sizeof(name),"%s/phonemes",source);TEST_ASSERT(remove(name)==0&&rmdir_path(source)==0);snprintf(name,sizeof(name),"%s/phondata-manifest",directory);TEST_ASSERT(remove(name)==0&&rmdir_path(directory)==0);
 printf("%u real native compiler host episodes: admission snapshots/reentry, complete primitive order, statuses, output bytes and lease release pass\n",episodes);return 0;
}
