/* Complete frozen C source-file controller vs owned Rust stack/dispatch.
 * SPDX-License-Identifier: GPL-3.0-or-later */
#include "speech.h"
#include "phoneme.h"
#include "common.h"
#include "rust_compiler_files.h"
#include "test_assert.h"
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <stdarg.h>
#include <stdint.h>
enum {tSTRING=2,tKEYWORD=7,kPHONEMESTART=1,kUTF8_BOM,kPROCEDURE,kENDPHONEME,kENDPROCEDURE,kPHONEMETABLE,kINCLUDE};
typedef int CompilerInput;
typedef struct {CompilerInput file;int linenum;char fname[80];} STACK;
#define N_STACK 12
typedef struct {RustProgramToken token;int requested;} Planned;
static Planned plan[14][64];static unsigned lengths[14];
typedef struct {
 CompilerInput f_in;int linenum,count_references,duplicate_references,count_frames,n_procs;
 int program_status,refuse_item,stack_ix,n_phoneme_tabs,n_phcodes,item_type,item_terminator,error_count,compiled,closed,terminated,fail_open;
 STACK stack[12];PHONEME_TAB phoneme_tab2[32];char phsrc[N_PATH_BUF],current_fname[80],item_string[256];
 unsigned positions[14],trace_count;uint32_t trace[8192];FILE *f_errors;
} CompileContext;
static void Trace(CompileContext *c,uint32_t value){TEST_ASSERT(c->trace_count<8192);c->trace[c->trace_count++]=value;}
static void Text(CompileContext *c,const char *name){for(const unsigned char *p=(const unsigned char *)name;*p;p++){Trace(c,*p);}}
static void error(CompileContext *c,const char *format,...)
{va_list args;va_start(args,format);fprintf(c->f_errors,"%s(%d): ",c->current_fname,c->linenum-1);vfprintf(c->f_errors,format,args);fputc('\n',c->f_errors);va_end(args);c->error_count++;}
static int SourceEof(CompileContext *c){TEST_ASSERT(c->f_in>=1&&c->f_in<=14);Trace(c,0xe000+(unsigned)c->f_in);return c->positions[c->f_in-1]>=lengths[c->f_in-1];}
static void SourceClose(CompileContext *c){c->closed++;Trace(c,0xc000+(unsigned)c->f_in);}
static int NextItem(CompileContext *c,int kind)
{if(c->refuse_item){c->program_status=c->refuse_item;return -2;}unsigned index=(unsigned)c->f_in-1;Trace(c,0x1000+(unsigned)kind);c->item_type=-1;if(c->positions[index]>=lengths[index]){return -2;}Planned *p=&plan[index][c->positions[index]++];TEST_ASSERT(kind==p->requested);c->linenum++;c->item_type=p->token.kind;c->item_terminator=p->token.terminator;memcpy(c->item_string,p->token.text,256);Trace(c,(unsigned)p->token.value);Text(c,c->item_string);return p->token.value;}
#include "compiler_token_bridge.inc"
static CompilerInput SourceOpen(CompileContext *c,const char *path,size_t depth,int text)
{TEST_ASSERT(text==0);Trace(c,0x2000+(unsigned)depth);Text(c,path);const char *name=strrchr(path,'/');name=name?name+1:path;if(c->fail_open||name[0]!='f'){return 0;}int index=atoi(name+1);if(index<0||index>=14){return 0;}c->positions[index]=0;return index+1;}
static void EndPhonemeTable(CompileContext *c){Trace(c,0x3000);}
static void StartPhonemeTable(CompileContext *c,const char *name){Trace(c,0x4000);Text(c,name);c->n_phoneme_tabs++;}
static int CompilePhoneme(CompileContext *c,int phoneme)
{Trace(c,0x5000+(unsigned)phoneme);c->compiled++;c->count_references+=phoneme;c->count_frames++;if(!phoneme){c->n_procs++;}NextItem(c,tSTRING);NextItem(c,tKEYWORD);return 0;}
#undef USE_RUST_CORE
#include "compiler_files_reference.inc"
_Static_assert(sizeof(RustCompilerFrame)==88,"source frame");
_Static_assert(sizeof(void *)!=8||sizeof(RustCompilerFiles)==128,"source controller descriptor");
typedef struct {
 CompileContext ctx;RustCompilerFiles *descriptor,admitted;int32_t result;
 int inspect,mutate,fail,events;unsigned char *root;
} Effects;
static void Guard(Effects *e)
{e->events++;if(e->inspect){int32_t result=55;TEST_ASSERT(espeak_rs_compiler_files(&e->admitted,&result)==11&&result==55);TEST_ASSERT(e->result==55);}if(e->mutate){memset(e->descriptor,0,sizeof(*e->descriptor));memset(e->root,0,N_PATH_BUF);}}
static int Reset(void *opaque){Effects *e=opaque;e->ctx.linenum=1;e->ctx.count_references=e->ctx.duplicate_references=e->ctx.count_frames=e->ctx.n_procs=0;Guard(e);return 0;}
static int Eof(void *opaque){Effects *e=opaque;int value=SourceEof(&e->ctx);Guard(e);return value;}
static int Read(void *opaque,int32_t kind,RustProgramToken *token){Effects *e=opaque;int status=ProgramNext(&e->ctx,kind,token);Guard(e);return status;}
static int Frame(void *opaque,RustCompilerFrame *frame){Effects *e=opaque;frame->file=e->ctx.f_in;frame->line=e->ctx.linenum;memcpy(frame->name,e->ctx.current_fname,80);Guard(e);return 0;}
static int Project(void *opaque,const RustCompilerFrame *frame){Effects *e=opaque;e->ctx.f_in=frame->file;e->ctx.linenum=frame->line;memcpy(e->ctx.current_fname,frame->name,80);Guard(e);return 0;}
static int Open(void *opaque,const unsigned char *bytes,size_t length,size_t depth){Effects *e=opaque;char path[N_PATH_BUF];TEST_ASSERT(length<sizeof(path));memcpy(path,bytes,length);path[length]=0;int value=SourceOpen(&e->ctx,path,depth,0);Guard(e);return value;}
static int Close(void *opaque){Effects *e=opaque;SourceClose(&e->ctx);Guard(e);return 0;}
static int Tables(void *opaque){Effects *e=opaque;Guard(e);return e->ctx.n_phoneme_tabs;}
static int End(void *opaque){Effects *e=opaque;EndPhonemeTable(&e->ctx);Guard(e);return 0;}
static int Start(void *opaque,const unsigned char *bytes,size_t length){Effects *e=opaque;char name[256];TEST_ASSERT(length<sizeof(name));memcpy(name,bytes,length);name[length]=0;StartPhonemeTable(&e->ctx,name);Guard(e);return e->fail;}
static int Compile(void *opaque,int32_t phoneme){Effects *e=opaque;CompilePhoneme(&e->ctx,phoneme);Guard(e);return e->fail;}
static int Terminate(void *opaque){Effects *e=opaque;memset(&e->ctx.phoneme_tab2[e->ctx.n_phcodes+1],0,sizeof(PHONEME_TAB));e->ctx.terminated++;Guard(e);return e->fail;}
static void Diagnostic(void *opaque,const unsigned char *bytes,size_t length){Effects *e=opaque;error(&e->ctx,"%.*s",(int)length,bytes);Guard(e);}
static int Call(Effects *e)
{unsigned char root[N_PATH_BUF];memcpy(root,e->ctx.phsrc,N_PATH_BUF);RustCompilerFiles d={e,root,N_PATH_BUF,Reset,Eof,Read,Frame,Project,Open,Close,Tables,End,Start,Compile,Terminate,Diagnostic};e->descriptor=&d;e->admitted=d;e->root=root;e->result=55;int status=espeak_rs_compiler_files(&d,&e->result);e->descriptor=NULL;return status;}
static void Add(unsigned file,int kind,int value,const char *name)
{TEST_ASSERT(file<14&&lengths[file]<64);Planned *p=&plan[file][lengths[file]++];memset(p,0,sizeof(*p));p->requested=kind;p->token.value=value;p->token.kind=-1;snprintf((char *)p->token.text,256,"%s",name?name:"");}
static void Definition(unsigned file,int phoneme){Add(file,7,phoneme?1:3,phoneme?"phoneme":"procedure");Add(file,2,0,"name");Add(file,7,4,"endphoneme");}
static unsigned comparisons;
static void Pair(unsigned trial)
{CompileContext c;Effects e;memset(&c,0,sizeof(c));memset(&e,0,sizeof(e));memset(plan,0,sizeof(plan));memset(lengths,0,sizeof(lengths));c.f_in=1;c.n_phoneme_tabs=trial%17!=0;c.n_phcodes=5;c.linenum=91;c.count_references=77;c.duplicate_references=88;c.count_frames=99;c.n_procs=33;c.fail_open=trial%13==0;memset(c.phoneme_tab2,0x55,sizeof(c.phoneme_tab2));snprintf(c.phsrc,sizeof(c.phsrc),"/source_root");strncpy0(c.current_fname,"phonemes",80);memcpy(&e.ctx,&c,sizeof(c));c.f_errors=tmpfile();e.ctx.f_errors=tmpfile();TEST_ASSERT(c.f_errors&&e.ctx.f_errors);
 unsigned depth=trial%14;
 for(unsigned i=0;i<14;i++){Add(i,7,2,"BOM");if(i<depth){char child[256];snprintf(child,sizeof(child),"f%u_",i+1);unsigned start=(unsigned)strlen(child),size=trial%3==0?220:12;memset(child+start,'x',size);child[start+size]=0;Add(i,7,7,"include");Add(i,2,0,child);}
 if(trial%17!=0||trial%2){Add(i,7,6,"phonemetable");Add(i,2,0,"child");}
 Definition(i,trial%2);Add(i,7,-1,"unexpected");Definition(i,1);if(trial%7==0){Add(i,7,7,"include");Add(i,2,0,"missing");}}
 if(trial%19==0){lengths[depth]=0;}
 CompilePhonemeFiles(&c);TEST_ASSERT(Call(&e)==0);TEST_ASSERT(c.f_in==e.ctx.f_in&&c.linenum==e.ctx.linenum&&c.error_count==e.ctx.error_count);TEST_ASSERT(c.count_references==e.ctx.count_references&&c.duplicate_references==e.ctx.duplicate_references&&c.count_frames==e.ctx.count_frames&&c.n_procs==e.ctx.n_procs&&c.compiled==e.ctx.compiled&&c.closed==e.ctx.closed&&c.n_phoneme_tabs==e.ctx.n_phoneme_tabs);TEST_ASSERT(memcmp(c.current_fname,e.ctx.current_fname,80)==0&&memcmp(c.phoneme_tab2,e.ctx.phoneme_tab2,sizeof(c.phoneme_tab2))==0&&memcmp(c.positions,e.ctx.positions,sizeof(c.positions))==0);TEST_ASSERT(c.trace_count==e.ctx.trace_count&&memcmp(c.trace,e.ctx.trace,c.trace_count*sizeof(uint32_t))==0);
 TEST_ASSERT(fflush(c.f_errors)==0&&fflush(e.ctx.f_errors)==0);TEST_ASSERT(ftell(c.f_errors)==ftell(e.ctx.f_errors));rewind(c.f_errors);rewind(e.ctx.f_errors);for(;;){int a=fgetc(c.f_errors),b=fgetc(e.ctx.f_errors);TEST_ASSERT(a==b);if(a==EOF){break;}}TEST_ASSERT(e.result==(c.phoneme_tab2[c.n_phcodes+1].mnemonic==0));fclose(c.f_errors);fclose(e.ctx.f_errors);comparisons+=6;}
static void Guards(void)
{Effects e;memset(&e,0,sizeof(e));memset(lengths,0,sizeof(lengths));e.ctx.f_in=1;e.ctx.n_phoneme_tabs=1;e.ctx.n_phcodes=5;strncpy0(e.ctx.phsrc,"/source",sizeof(e.ctx.phsrc));strncpy0(e.ctx.current_fname,"phonemes",80);e.ctx.f_errors=tmpfile();TEST_ASSERT(e.ctx.f_errors);Add(0,7,7,"include");Add(0,2,0,"f1_abc");Definition(0,1);Definition(1,1);e.inspect=e.mutate=1;TEST_ASSERT(Call(&e)==0&&e.result==1&&e.ctx.compiled==2&&e.events>8);e.inspect=e.mutate=0;
 memset(lengths,0,sizeof(lengths));e.ctx.f_in=1;memset(e.ctx.positions,0,sizeof(e.ctx.positions));Definition(0,1);e.fail=5;int compiled=e.ctx.compiled;TEST_ASSERT(Call(&e)==5&&e.result==55&&e.ctx.compiled==compiled+1);e.fail=0;
 memset(lengths,0,sizeof(lengths));e.ctx.f_in=1;memset(e.ctx.positions,0,sizeof(e.ctx.positions));Definition(0,1);e.ctx.refuse_item=11;compiled=e.ctx.compiled;TEST_ASSERT(Call(&e)==11&&e.result==55&&e.ctx.positions[0]==0&&e.ctx.compiled==compiled);e.ctx.refuse_item=0;TEST_ASSERT(Call(&e)==11&&e.result==55&&e.ctx.positions[0]==0&&e.ctx.compiled==compiled);e.ctx.program_status=0;
 unsigned char root[N_PATH_BUF];memset(root,'x',sizeof(root));RustCompilerFiles d=e.admitted;d.root=root;int32_t out=55;int events=e.events;TEST_ASSERT(espeak_rs_compiler_files(&d,&out)==22&&out==55&&e.events==events);d.root=(const unsigned char *)e.ctx.phsrc;d.width=4097;TEST_ASSERT(espeak_rs_compiler_files(&d,&out)==22&&out==55);d.width=N_PATH_BUF;d.compile=NULL;TEST_ASSERT(espeak_rs_compiler_files(&d,&out)==22&&out==55);TEST_ASSERT(espeak_rs_compiler_files(NULL,&out)==22&&out==55);
 memset(lengths,0,sizeof(lengths));e.ctx.f_in=1;memset(e.ctx.positions,0,sizeof(e.ctx.positions));union {RustCompilerFiles d;int32_t result;} overlap;overlap.d=e.admitted;overlap.d.root=(const unsigned char *)e.ctx.phsrc;e.descriptor=&overlap.d;TEST_ASSERT(espeak_rs_compiler_files(&overlap.d,&overlap.result)==0&&overlap.result==1);fclose(e.ctx.f_errors);}
int main(void){for(unsigned i=0;i<2000;i++){Pair(i);}Guards();printf("2000 complete C source-file/include episodes: %u comparisons; snapshot/reentry/overlap/failure guards passed\n",comparisons);return 0;}
