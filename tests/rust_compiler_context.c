/* Native Rust compiler storage/leases and source admission on real files.
 * SPDX-License-Identifier: GPL-3.0-or-later */
#include "speech.h"
#include "rust_compiler_context.h"
#include "rust_compiler_source.h"
#include "rust_compiler_program.h"
#include "rust_compiler_tables.h"
#include "rust_compiler_resource.h"
#include "rust_compiler_output.h"
#include "test_assert.h"
#include <ctype.h>
#include <errno.h>
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
_Static_assert(sizeof(RustCompilerContext)==4592,"context");
_Static_assert(offsetof(RustCompilerContext,source_pool)==112,"source");
_Static_assert(offsetof(RustCompilerContext,resource_owner)==224,"resources");
_Static_assert(offsetof(RustCompilerContext,phsrc)==496,"path");
_Static_assert(sizeof(RustCompilerSourceOpen)==48,"source descriptor");
#endif
typedef RustCompilerContext CompileContext;
typedef int CompilerInput;
#define clean_context OracleCleanup
#define SourceOpen OracleSourceOpen
#include "compiler_context_reference.inc"
#undef clean_context
#undef SourceOpen
static char directory[N_PATH_BUF],source[N_PATH_BUF];static unsigned maps;
static RustCompilerSourceOpen *guard;static int inspect;static char *mutated_path;
static void Classes(unsigned char *spaces,unsigned char *digits)
{
 maps++;for(unsigned i=0;i<256;i++){spaces[i]=isspace(i)!=0;digits[i]=isdigit(i)!=0;}
 if(inspect){int error=42;TEST_ASSERT(espeak_rs_compiler_context_source(guard,&error)==0&&error==EAGAIN);TEST_ASSERT(espeak_rs_compiler_context_end(guard->context)==EAGAIN);guard->classes=NULL;mutated_path[0]='X';}
}
static int Fallback(void *opaque,const unsigned char *bytes,size_t length)
{(void)opaque;(void)bytes;(void)length;return 0;}
static int Open(int native,RustCompilerSourceOpen *d,int *error)
{if(native){return espeak_rs_compiler_context_source(d,error);}int input=OracleSourceOpen(d->context,d->path,d->depth,d->text);*error=input?0:errno;return input;}
static void Episode(unsigned trial,int native)
{
 int error=42;RustCompilerContext *c;
 if(native){c=espeak_rs_compiler_context_begin(directory,N_PATH_BUF,&error);}else{c=calloc(1,sizeof(*c));if(c){memcpy(c->phsrc,directory,strlen(directory)+1);error=0;}}
 TEST_ASSERT(c&&error==0&&strcmp(c->phsrc,directory)==0);
 TEST_ASSERT(c->phoneme_out==NULL&&c->phoneme_tab2==NULL&&c->source_pool==NULL&&c->f_errors==NULL&&c->item_string[0]==0&&c->current_fname[0]==0&&c->table_owner==0&&c->program_owner==0&&c->output_owner==0&&c->resource_owner==0);
 FILE *log=tmpfile();TEST_ASSERT(log);c->f_errors=log;c->linenum=trial;c->definition_row.mnemonic=trial+1;
 c->table_owner=espeak_rs_compiler_tables_begin();c->program_owner=espeak_rs_compiler_program_begin();c->resource_owner=espeak_rs_compiler_resources_begin();TEST_ASSERT(c->table_owner&&c->program_owner&&c->resource_owner);
 char path[N_PATH_BUF];snprintf(path,sizeof(path),"%s",source);RustCompilerSourceOpen d={c,path,N_PATH_BUF,0,0,Classes};guard=&d;mutated_path=path;inspect=native&&trial==0;unsigned count=maps;
 TEST_ASSERT(Open(native,&d,&error)==1&&error==0&&c->source_pool);inspect=0;d.classes=Classes;d.path=source;
 TEST_ASSERT(maps==count+(unsigned)native&&c->linenum==(int)trial&&c->definition_row.mnemonic==trial+1);
 void *session=c->source_pool;for(unsigned i=0;i<13;i++){d.depth=i;TEST_ASSERT(Open(native,&d,&error)==(int)i+1&&c->source_pool==session&&maps==count+(unsigned)native);}
 char line[64];TEST_ASSERT(espeak_rs_compiler_source_line(c->source_pool,1,(unsigned char *)line,sizeof(line))==1&&strcmp(line,"alpha 12\n")==0);
 if(native){
  TEST_ASSERT(espeak_rs_compiler_context_source(NULL,&error)==0&&error==EINVAL);
  TEST_ASSERT(espeak_rs_compiler_context_source(&d,NULL)==0);
  d.depth=13;TEST_ASSERT(espeak_rs_compiler_context_source(&d,&error)==0&&error==EINVAL&&c->source_pool==session&&maps==count+1);d.depth=0;
  d.width=1;TEST_ASSERT(espeak_rs_compiler_context_source(&d,&error)==0&&error==EINVAL);d.width=N_PATH_BUF;
  char unterminated[N_PATH_BUF];memset(unterminated,'x',sizeof(unterminated));d.path=unterminated;TEST_ASSERT(espeak_rs_compiler_context_source(&d,&error)==0&&error==ENAMETOOLONG);
  union {RustCompilerSourceOpen d;int error;} overlap;overlap.d=d;overlap.d.path=source;TEST_ASSERT(espeak_rs_compiler_context_source(&overlap.d,&overlap.error)==1&&overlap.error==0);
 }
 d.path="/missing/native/compiler/source";TEST_ASSERT(Open(native,&d,&error)==0&&error==ENOENT&&c->source_pool==session&&maps==count+(unsigned)native);
 if(native){
  RustCompilerContext *blocked=espeak_rs_compiler_context_begin(directory,N_PATH_BUF,&error);TEST_ASSERT(blocked);
  RustCompilerSourceOpen pending={blocked,source,N_PATH_BUF,0,0,Classes};
  TEST_ASSERT(espeak_rs_compiler_context_source(&pending,&error)==0&&error==EAGAIN&&blocked->source_pool==NULL&&c->source_pool==session);
  TEST_ASSERT(espeak_rs_compiler_context_end(blocked)==0);
 }
 RustCompilerOutput output={directory,directory,N_PATH_BUF,123,22050,c,Fallback};RustCompilerOutputResult opened={0};TEST_ASSERT(espeak_rs_compiler_output_begin(&output,&opened)==0);c->output_owner=opened.owner;c->output_mask=opened.mask;
 unsigned char payload[2]={(unsigned char)trial,77};TEST_ASSERT(espeak_rs_compiler_output_write(c->output_owner,RUST_OUTPUT_INDEX,payload,2)==0);
 uint64_t tables=c->table_owner,program=c->program_owner,resources=c->resource_owner,out=c->output_owner;
 if(native){TEST_ASSERT(espeak_rs_compiler_context_end(c)==0);}else{OracleCleanup(c);}
 TEST_ASSERT(espeak_rs_compiler_tables_end(tables)!=0&&espeak_rs_compiler_program_end(program)!=0&&espeak_rs_compiler_resources_end(resources)!=0&&espeak_rs_compiler_output_end(out)!=0);
 TEST_ASSERT(fwrite("still borrowed",1,14,log)==14&&fclose(log)==0);
 char name[N_PATH_BUF];snprintf(name,sizeof(name),"%s/phonindex",directory);FILE *f=fopen(name,"rb");unsigned char bytes[6];TEST_ASSERT(f&&fread(bytes,1,6,f)==6&&bytes[4]==payload[0]&&bytes[5]==77&&fgetc(f)==EOF&&fclose(f)==0);
 unsigned char spaces[256],digits[256];Classes(spaces,digits);void *released=espeak_rs_compiler_sources_begin(spaces,digits,&error);TEST_ASSERT(released&&error==0);espeak_rs_compiler_sources_end(released);
}
int main(void)
{
 const char *temporary=getenv("TMPDIR");
#ifdef _WIN32
 if(!temporary){temporary=getenv("TEMP");}
#endif
 if(!temporary){temporary="/tmp";}
 snprintf(directory,sizeof(directory),"%s/espeak-compiler-context-%ld",temporary,(long)process_id());TEST_ASSERT(mkdir_path(directory)==0);snprintf(source,sizeof(source),"%s/source",directory);
 FILE *f=fopen(source,"wb");TEST_ASSERT(f&&fwrite("alpha 12\n",1,9,f)==9&&fclose(f)==0);
 for(unsigned i=0;i<100;i++){Episode(i,0);Episode(i,1);}
 int failed_error=42;RustCompilerContext *failed=espeak_rs_compiler_context_begin(directory,N_PATH_BUF,&failed_error);TEST_ASSERT(failed);
 RustCompilerSourceOpen retry={failed,"/missing/native/compiler/source",N_PATH_BUF,0,0,Classes};unsigned before=maps;
 TEST_ASSERT(espeak_rs_compiler_context_source(&retry,&failed_error)==0&&failed_error==ENOENT&&failed->source_pool!=NULL&&maps==before+1);
 void *retained=failed->source_pool;retry.path=source;TEST_ASSERT(espeak_rs_compiler_context_source(&retry,&failed_error)==1&&failed_error==0&&failed->source_pool==retained&&maps==before+1);
 TEST_ASSERT(espeak_rs_compiler_context_end(failed)==0);
 int error=42;TEST_ASSERT(espeak_rs_compiler_context_begin(NULL,N_PATH_BUF,&error)==NULL&&error==EINVAL);TEST_ASSERT(espeak_rs_compiler_context_end(NULL)==0);
 for(unsigned i=0;i<6;i++){char path[N_PATH_BUF];snprintf(path,sizeof(path),"%s/%s",directory,(const char *[]){"source","phondata","phonindex","phontab","phondata-manifest","compile_prog_log"}[i]);remove(path);}TEST_ASSERT(rmdir_path(directory)==0);
 puts("100 frozen-C/native context/source lifecycle pairs, all five lease releases, quiescent fields, exact pending-output completion and snapshot/reentry/overlap/bounds guards passed");return 0;
}
