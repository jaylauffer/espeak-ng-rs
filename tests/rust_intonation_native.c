/* Previous native C public host vs Rust-owned host, with real compiler files.
 * SPDX-License-Identifier: GPL-3.0-or-later */
#include "config.h"
#include "test_assert.h"
#include "common.h"
#include "speech.h"
#include "rust_intonation_compile.h"
#include <espeak-ng/espeak_ng.h>
#include <ctype.h>
#include <errno.h>
#include <fcntl.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <sys/stat.h>
#include <unistd.h>

static char home[4096];
static FILE *fallback;
typedef struct {
    FILE *log;
    int fd, reloads, errors, status, reentry, nested;
    char path[4096];
    RustNativeIntonationCompiler *descriptor;
} Effects;
static Effects *active;
static unsigned cases;
static int ReloadPrimitive(void *a, void *b)
{
    (void)a; (void)b;
    TEST_ASSERT(fcntl(active->fd,F_GETFD)!=-1);
    active->reloads++;
    if(active->reentry) {
        TEST_ASSERT(espeak_rs_compile_intonation_native(active->descriptor)==ENS_COMPILE_ERROR);
        TEST_ASSERT(fcntl(active->fd,F_GETFD)!=-1);
        active->nested++;
        active->descriptor->source=NULL;
        active->descriptor->home=NULL;
        active->descriptor->reload=NULL;
    }
    return 0;
}
static int ErrorPrimitive(void *context, int status, const char *path)
{
    Effects *e=context;
    TEST_ASSERT(fcntl(e->fd,F_GETFD)==-1&&errno==EBADF);
    e->errors++;e->status=status;
    TEST_ASSERT(strlen(path)<sizeof(e->path));strcpy(e->path,path);
    return status;
}
#define path_home home
#pragma push_macro("stderr")
#undef stderr
#define stderr fallback
#define LoadPhData ReloadPrimitive
#define create_file_error_context ErrorPrimitive
#define IntonationLog PreviousLog
#define IntonationClose PreviousClose
#define IntonationReload PreviousReload
#define IntonationError PreviousError
#define espeak_ng_CompileIntonationPath PreviousCompile
#include "intonation_native_reference.inc"
#undef espeak_ng_CompileIntonationPath
#undef IntonationLog
#undef IntonationClose
#undef IntonationReload
#undef IntonationError
#include "intonation_native_primitives.inc"
static int NativeCompile(const char *source_path,const char *destination_path,
                         FILE *log,espeak_ng_ERROR_CONTEXT *context)
{
#include "intonation_native_admission.inc"
}
#undef create_file_error_context
#undef LoadPhData
#undef stderr
#pragma pop_macro("stderr")
#undef path_home

_Static_assert(sizeof(RustNativeIntonationCompiler)==(sizeof(void *)==8?72:36),"native ABI");
static size_t Read(const char *path,unsigned char *bytes,size_t capacity)
{
    FILE *f=fopen(path,"rb");TEST_ASSERT(f);
    size_t n=fread(bytes,1,capacity,f);TEST_ASSERT(feof(f));fclose(f);return n;
}
static void Pair(const char *root,const char *text,int defaults,int failure,int direct)
{
    char source[512],dest[512],input[512],output[512],logs[2][512];
    snprintf(home,sizeof(home),"%s/data",root);
    snprintf(source,sizeof(source),"%s/data",root);snprintf(dest,sizeof(dest),"%s",root);
    snprintf(input,sizeof(input),"%s/phsource/intonation",root);
    snprintf(output,sizeof(output),"%s/intonations",(defaults&2)?home:dest);
    snprintf(logs[0],sizeof(logs[0]),"%s/previous.log",root);
    snprintf(logs[1],sizeof(logs[1]),"%s/native.log",root);
    if(unlink(input)!=0) {TEST_ASSERT(errno==ENOENT);}
    if(!failure||failure==2) {
        FILE *f=fopen(input,"wb");TEST_ASSERT(f);
        TEST_ASSERT(fwrite(text,1,strlen(text),f)==strlen(text));fclose(f);
    }
    if(failure==2) {
        if(unlink(output)!=0) {TEST_ASSERT(errno==ENOENT);}
        TEST_ASSERT(mkdir(output,0700)==0);
    }
    Effects e[2]={{0},{0}};int status[2],closed[2];
    unsigned char expected[6801],actual[6801],log_expected[65536],log_actual[65536];
    size_t expected_length=0;
    for(int lane=0;lane<2;lane++) {
        e[lane].log=fopen(logs[lane],"wb");TEST_ASSERT(e[lane].log);
        e[lane].fd=fileno(e[lane].log);fallback=e[lane].log;active=&e[lane];
        const char *src=(defaults&1)?NULL:source,*dst=(defaults&2)?NULL:dest;
        FILE *log=(defaults&4)?NULL:e[lane].log;
        if(!lane) {
            status[lane]=PreviousCompile(src,dst,log,(espeak_ng_ERROR_CONTEXT *)&e[lane]);
        } else if(!direct) {
            status[lane]=NativeCompile(src,dst,log,(espeak_ng_ERROR_CONTEXT *)&e[lane]);
        } else {
            RustNativeIntonationCompiler d={src,dst,home,log,fallback,&e[lane],N_PATH_BUF,IntonationReload,IntonationError};
            e[lane].descriptor=&d;e[lane].reentry=1;
            status[lane]=espeak_rs_compile_intonation_native(&d);
            TEST_ASSERT(e[lane].nested==1);
        }
        closed[lane]=fcntl(e[lane].fd,F_GETFD)==-1;
        if(closed[lane]) {TEST_ASSERT(errno==EBADF);} else {fclose(e[lane].log);}
        if(!failure) {
            if(!lane) {expected_length=Read(output,expected,sizeof(expected));}
            else {size_t n=Read(output,actual,sizeof(actual));TEST_ASSERT(n==expected_length);TEST_ASSERT(memcmp(expected,actual,n)==0);}
        }
    }
    TEST_ASSERT(status[0]==status[1]);TEST_ASSERT(closed[0]==closed[1]);
    TEST_ASSERT(closed[1]==(failure!=0));
    TEST_ASSERT(e[0].reloads==e[1].reloads&&e[0].errors==e[1].errors&&e[0].status==e[1].status);
    TEST_ASSERT(strcmp(e[0].path,e[1].path)==0);
    size_t a=Read(logs[0],log_expected,sizeof(log_expected)),b=Read(logs[1],log_actual,sizeof(log_actual));
    TEST_ASSERT(a==b&&memcmp(log_expected,log_actual,a)==0);
    if(failure==2) {TEST_ASSERT(rmdir(output)==0);}
    cases++;
}
static void Guards(const char *root)
{
    char logpath[512],bad[4096];snprintf(logpath,sizeof(logpath),"%s/guards.log",root);
    FILE *f=fopen(logpath,"wb");TEST_ASSERT(f);int fd=fileno(f);
    RustNativeIntonationCompiler good={"x","y",NULL,f,NULL,NULL,N_PATH_BUF,IntonationReload,IntonationError};
    TEST_ASSERT(espeak_rs_compile_intonation_native(NULL)==ENS_COMPILE_ERROR);
    for(int i=0;i<8;i++) {
        RustNativeIntonationCompiler d=good;
        switch(i) {
        case 0:d.width=1;break;
        case 1:d.width=4097;break;
        case 2:d.source=NULL;break;
        case 3:d.destination=NULL;break;
        case 4:d.log=NULL;break;
        case 5:d.reload=NULL;break;
        case 6:d.error=NULL;break;
        default:memset(bad,'a',sizeof(bad));d.source=bad;break;
        }
        TEST_ASSERT(espeak_rs_compile_intonation_native(&d)==ENS_COMPILE_ERROR);
        TEST_ASSERT(fcntl(fd,F_GETFD)!=-1&&ftell(f)==0);
    }
    fclose(f);TEST_ASSERT(unlink(logpath)==0);
}
int main(void)
{
    char root[256],path[512],text[2048];snprintf(root,sizeof(root),"intonation-native-%ld.tmp",(long)getpid());
    TEST_ASSERT(mkdir(root,0700)==0);
    snprintf(path,sizeof(path),"%s/data",root);TEST_ASSERT(mkdir(path,0700)==0);
    snprintf(path,sizeof(path),"%s/phsource",root);TEST_ASSERT(mkdir(path,0700)==0);
    const char *basic="tune s1\nendtune\ntune c1\nendtune\ntune q1\nendtune\ntune e1\nendtune\n";
    for(int defaults=0;defaults<8;defaults++) {
        Pair(root,basic,defaults,0,0);
        Pair(root,basic,defaults,1,0);
        Pair(root,basic,defaults,2,0);
        for(unsigned i=0;i<16;i++) {
            snprintf(text,sizeof(text),"%stune s1\nprehead %u %u\nheadenv %s %u\nhead 4 %u %u -10 -3\nendtune\n",basic,i*17,i*29,(i%3)?"fall":"missing",i*19,i*21,i*23);
            Pair(root,text,defaults,0,0);
        }
        Pair(root,basic,defaults,0,1);
    }
    Guards(root);
    const char *files[]={"previous.log","native.log","intonations","data/intonations","phsource/intonation"};
    for(unsigned i=0;i<5;i++) {snprintf(path,sizeof(path),"%s/%s",root,files[i]);TEST_ASSERT(unlink(path)==0);}
    snprintf(path,sizeof(path),"%s/data",root);TEST_ASSERT(rmdir(path)==0);
    snprintf(path,sizeof(path),"%s/phsource",root);TEST_ASSERT(rmdir(path)==0);TEST_ASSERT(rmdir(root)==0);
    printf("%u complete previous-C/native intonation host file/status/log/lifecycle pairs; defaults, native admission and reentry guards pass\n",cases);
    return 0;
}
