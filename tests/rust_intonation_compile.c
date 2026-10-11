/* Frozen full C intonation driver vs owned native Rust compilation.
 * SPDX-License-Identifier: GPL-3.0-or-later */
#include "config.h"
#include "test_assert.h"
#include "common.h"
#include "synthesize.h"
#include "mnemonics.h"
#include "speech.h"
#include "rust_intonation_compile.h"
#include <espeak-ng/espeak_ng.h>
#include <ctype.h>
#include <errno.h>
#include <stdbool.h>
#include <stdarg.h>
#include <stdint.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <sys/stat.h>
#include <unistd.h>
enum {tENDFILE=1,tSTRING,tNUMBER,tSIGNEDNUMBER,tPHONEMEMNEM,tOPENBRACKET,tKEYWORD,tCONDITION,tPROPERTIES,tINTONATION};
enum {kTUNE=1,kENDTUNE,kTUNE_PREHEAD,kTUNE_ONSET,kTUNE_HEAD,kTUNE_HEADENV,kTUNE_HEADEXTEND,kTUNE_HEADLAST,kTUNE_NUCLEUS0,kTUNE_NUCLEUS1,kTUNE_SPLIT};
typedef struct {const char *mnem;int type,data;} keywtab_t;
#include "intonation_keywords_reference.inc"
static const keywtab_t empty_table[]={{NULL,0,0}};
static const keywtab_t *keyword_tabs[]={empty_table,empty_table,empty_table,k_intonation};
typedef struct {
    FILE *f_in,*f_errors;
    int f_in_displ,f_in_linenum,linenum,error_count,item_type,item_terminator;
    char current_fname[80],item_string[256];
} CompileContext;
static void error(CompileContext *ctx,const char *format,...)
{
    va_list args;va_start(args,format);fprintf(ctx->f_errors,"%s(%d): ",ctx->current_fname,ctx->linenum-1);
    vfprintf(ctx->f_errors,format,args);fputc('\n',ctx->f_errors);ctx->error_count++;va_end(args);
}
static int LookupPhoneme(CompileContext *ctx,const char *text,int control) {(void)ctx;(void)text;(void)control;return -1;}
#include "compiler_source_reference.inc"
typedef struct {
    FILE *log;
    int closed,reloaded,error_calls,status,reentry,mutated;
    char path[4096];
    char *source,*destination;
    RustIntonationCompiler *compiler;
} Effects;
static Effects *reference;
static void clean_context(CompileContext *ctx) {free(ctx);}
static int ReferenceClose(FILE *f) {if(f==reference->log)reference->closed++;return fclose(f);}
static int ReferenceReload(void *a,void *b) {(void)a;(void)b;reference->reloaded++;return 0;}
static int ReferenceError(espeak_ng_ERROR_CONTEXT *context,int status,const char *path)
{
    Effects *e=(Effects *)context;e->error_calls++;e->status=status;strncpy(e->path,path,sizeof(e->path)-1);return status;
}
#define fclose ReferenceClose
#define LoadPhData ReferenceReload
#define create_file_error_context ReferenceError
#define espeak_ng_CompileIntonationPath ReferenceCompile
#include "intonation_compile_reference.inc"
#undef espeak_ng_CompileIntonationPath
#undef create_file_error_context
#undef LoadPhData
#undef fclose
static void Log(void *opaque,const unsigned char *bytes,size_t length)
{
    Effects *e=opaque;TEST_ASSERT(!e->closed);TEST_ASSERT(fwrite(bytes,1,length,e->log)==length);
    if(e->reentry&&!e->mutated) {
        e->mutated=1;
        TEST_ASSERT(espeak_rs_compile_intonation(e->compiler)==ENS_COMPILE_ERROR);
        strcpy(e->source,"invalid-after-admission");strcpy(e->destination,"invalid-after-admission");
    }
}
static void Close(void *opaque) {Effects *e=opaque;TEST_ASSERT(!e->closed);e->closed++;fclose(e->log);}
static void Reload(void *opaque) {Effects *e=opaque;e->reloaded++;}
static int FileError(void *opaque,int status,const char *path) {return ReferenceError((espeak_ng_ERROR_CONTEXT *)opaque,status,path);}
_Static_assert(sizeof(TUNE)==68,"compiled tune bytes");
_Static_assert(sizeof(RustIntonationCompiler)==(sizeof(void *)==8?88:44),"intonation descriptor ABI");
static unsigned char spaces[256],digits[256];
static unsigned cases;
static uint32_t seed=0x61267238;
static uint32_t Next(void) {seed^=seed<<13;seed^=seed>>17;seed^=seed<<5;return seed;}
static size_t Read(const char *path,unsigned char *bytes,size_t capacity)
{
    FILE *f=fopen(path,"rb");TEST_ASSERT(f);size_t n=fread(bytes,1,capacity,f);TEST_ASSERT(feof(f));fclose(f);return n;
}
static void Pair(const char *root,const char *text,int failure,int reentry)
{
    char source[512],dest[512],input[512],output[512],logs[2][512];
    snprintf(source,sizeof(source),"%s/data",root);snprintf(dest,sizeof(dest),"%s",root);
    snprintf(input,sizeof(input),"%s/phsource/intonation",root);snprintf(output,sizeof(output),"%s/intonations",root);
    snprintf(logs[0],sizeof(logs[0]),"%s/c.log",root);snprintf(logs[1],sizeof(logs[1]),"%s/rust.log",root);
    if(failure==1) {if(unlink(input)!=0)TEST_ASSERT(errno==ENOENT);} else {FILE *f=fopen(input,"wb");TEST_ASSERT(f);TEST_ASSERT(fwrite(text,1,strlen(text),f)==strlen(text));fclose(f);}
    if(failure==2)snprintf(dest,sizeof(dest),"%s/missing",root);
    unsigned char expected[6801],actual[6801],expected_log[65536],actual_log[65536];
    int status[2];Effects effects[2]={{0},{0}};size_t expected_size=0;
    for(int lane=0;lane<2;lane++) {
        effects[lane].log=fopen(logs[lane],"wb");TEST_ASSERT(effects[lane].log);
        if(lane==0) {
            reference=&effects[0];
            status[0]=ReferenceCompile(source,dest,effects[0].log,(espeak_ng_ERROR_CONTEXT *)&effects[0]);
            if(!effects[0].closed)fclose(effects[0].log);
            if(!failure)expected_size=Read(output,expected,sizeof(expected));
        } else {
            RustIntonationCompiler d={&effects[1],source,dest,spaces,digits,N_PATH_BUF,0,Log,Close,Reload,FileError};
            effects[1].source=source;effects[1].destination=dest;effects[1].compiler=&d;effects[1].reentry=reentry;
            status[1]=espeak_rs_compile_intonation(&d);
            if(!effects[1].closed)fclose(effects[1].log);
            if(!failure) {size_t n=Read(output,actual,sizeof(actual));TEST_ASSERT(n==expected_size);TEST_ASSERT(memcmp(expected,actual,n)==0);}
        }
    }
    TEST_ASSERT(status[0]==status[1]);TEST_ASSERT(effects[0].closed==effects[1].closed);
    TEST_ASSERT(effects[0].reloaded==effects[1].reloaded);TEST_ASSERT(effects[0].error_calls==effects[1].error_calls);
    TEST_ASSERT(strcmp(effects[0].path,effects[1].path)==0);
    size_t a=Read(logs[0],expected_log,sizeof(expected_log)),b=Read(logs[1],actual_log,sizeof(actual_log));
    TEST_ASSERT(a==b);if(memcmp(expected_log,actual_log,a)) {fwrite(expected_log,1,a,stderr);fputs("--- Rust ---\n",stderr);fwrite(actual_log,1,b,stderr);abort();}
    if(reentry)TEST_ASSERT(effects[1].mutated);
    cases++;
}
static void Guards(void)
{
    RustIntonationCompiler d={NULL,"x","y",spaces,digits,N_PATH_BUF,0,Log,Close,Reload,FileError};
    TEST_ASSERT(espeak_rs_compile_intonation(NULL)==ENS_COMPILE_ERROR);
    d.width=1;TEST_ASSERT(espeak_rs_compile_intonation(&d)==ENS_COMPILE_ERROR);d.width=4097;TEST_ASSERT(espeak_rs_compile_intonation(&d)==ENS_COMPILE_ERROR);d.width=N_PATH_BUF;
    d.log=NULL;TEST_ASSERT(espeak_rs_compile_intonation(&d)==ENS_COMPILE_ERROR);d.log=Log;
    d.digits=NULL;TEST_ASSERT(espeak_rs_compile_intonation(&d)==ENS_COMPILE_ERROR);d.digits=digits;
    d.source=NULL;TEST_ASSERT(espeak_rs_compile_intonation(&d)==ENS_COMPILE_ERROR);
    char unterminated[4096];memset(unterminated,'a',sizeof(unterminated));d.source=unterminated;TEST_ASSERT(espeak_rs_compile_intonation(&d)==ENS_COMPILE_ERROR);
}
int main(void)
{
    for(int i=0;i<256;i++) {spaces[i]=isspace(i)!=0;digits[i]=isdigit(i)!=0;}
    char root[256],directory[512];snprintf(root,sizeof(root),"intonation-compiler-%ld.tmp",(long)getpid());TEST_ASSERT(mkdir(root,0700)==0);
    snprintf(directory,sizeof(directory),"%s/phsource",root);TEST_ASSERT(mkdir(directory,0700)==0);
    snprintf(directory,sizeof(directory),"%s/data",root);TEST_ASSERT(mkdir(directory,0700)==0);
    const char *basic="tune s1\nendtune\ntune c1\nendtune\ntune q1\nendtune\ntune e1\nendtune\n";
    Pair(root,basic,0,1);
    const char *edges[]={"","// comment\n","endtune\n","tune s1\nheadenv missing 7\nendtune\n", "tune s1\nnucleus missing\nsplit missing\nnucleus0 missing\nendtune\n","tune s1\nsplit rise 1 2 3 4 unknown\nendtune\n", "tune s1\nprehead -12 x\nonset 90 -x -4\nheadextend - 20\nendtune\n", "tune s1\nonset 9 2 3\nheadlast 4 5 6\nendtune\ntune c1\nhead 4 90 20 -10 -3\nendtune\n", "  tune extra\nendtune\n", "tune s1\nendtune\ntune s1\nendtune\n","tune abcdefghijklmnop\nendtune\n"};
    for(unsigned i=0;i<sizeof(edges)/sizeof(edges[0]);i++)Pair(root,edges[i],0,0);
    char bounded[12000];size_t bounded_length=snprintf(bounded,sizeof(bounded),"%s",basic);
    for(unsigned i=0;i<110;i++)bounded_length+=snprintf(bounded+bounded_length,sizeof(bounded)-bounded_length,"tune n%u\nendtune\n",i);
    Pair(root,bounded,0,0);
    for(unsigned i=0;i<500;i++) {
        char text[12000];size_t n=snprintf(text,sizeof(text),"%s",basic);
        for(unsigned t=0;t<10;t++) {
            const char *names[]={"s1","c1","q1","e1","added","longname11x","another"};
            const char *envs[]={"fall","rise","fall-rise","fall-rise2","rise-fall","fall-rise3","fall-rise4","fall2","rise2","rise-fall-rise","bad"};
            const char *name=names[Next()%7];n+=snprintf(text+n,sizeof(text)-n,"tune %s\nprehead %u %u\nheadenv %s %u\nhead %u %u %u %d %d\n",name,Next()%512,Next()%512,envs[Next()%11],Next()%512,Next()%8,Next()%512,Next()%512,(int)(Next()%511)-255,(int)(Next()%511)-255);
            if(Next()%2)n+=snprintf(text+n,sizeof(text)-n,"onset %u %d %d\n",Next()%512,(int)(Next()%511)-255,(int)(Next()%511)-255);
            if(Next()%2)n+=snprintf(text+n,sizeof(text)-n,"headlast %u %d %d\n",Next()%512,(int)(Next()%511)-255,(int)(Next()%511)-255);
            n+=snprintf(text+n,sizeof(text)-n,"headextend %d %d %d\nnucleus0 %s %u %u\nnucleus %s %u %u %u %u\n",(int)(Next()%511)-255,(int)(Next()%511)-255,(int)(Next()%511)-255,envs[Next()%11],Next()%512,Next()%512,envs[Next()%11],Next()%512,Next()%512,Next()%512,Next()%512);
            if(Next()%2)n+=snprintf(text+n,sizeof(text)-n,"split %s %u %u %u %u %s\n",envs[Next()%11],Next()%512,Next()%512,Next()%512,Next()%512,names[Next()%7]);
            n+=snprintf(text+n,sizeof(text)-n,"endtune\n");TEST_ASSERT(n<sizeof(text));
        }
        Pair(root,text,0,0);
    }
    Pair(root,basic,1,0);Pair(root,basic,2,0);Guards();
    const char *files[]={"c.log","rust.log","intonations","phsource/intonation"};
    for(unsigned i=0;i<4;i++){snprintf(directory,sizeof(directory),"%s/%s",root,files[i]);TEST_ASSERT(unlink(directory)==0);}
    snprintf(directory,sizeof(directory),"%s/phsource",root);TEST_ASSERT(rmdir(directory)==0);
    snprintf(directory,sizeof(directory),"%s/data",root);TEST_ASSERT(rmdir(directory)==0);TEST_ASSERT(rmdir(root)==0);
    printf("%u full C/Rust intonation output/status/log/lifecycle comparisons; descriptor and reentry guards passed\n",cases);return 0;
}
