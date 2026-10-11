/* Actual frozen C scanner vs owned Rust source/lexer state.
 * SPDX-License-Identifier: GPL-3.0-or-later */
#include "config.h"
#include "test_assert.h"
#include "rust_compiler_source.h"
#include <ctype.h>
#include <stdarg.h>
#include <stdint.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <unistd.h>
enum { tENDFILE=1,tSTRING,tNUMBER,tSIGNEDNUMBER,tPHONEMEMNEM,tOPENBRACKET,tKEYWORD,tCONDITION,tPROPERTIES,tINTONATION };
typedef struct { const char *mnem; int type,data; } keywtab_t;
static const keywtab_t empty_table[]={{NULL,0,0}};
static const keywtab_t *keyword_tabs[]={empty_table,empty_table,empty_table,empty_table};
typedef struct {
    FILE *f_in;
    int f_in_displ,f_in_linenum,linenum,item_type,item_terminator,diagnostic;
    char item_string[256],actual_string[256];
} CompileContext;
static void error(CompileContext *ctx,const char *format,...)
{
    ctx->diagnostic=strcmp(format,"Expected an unsigned number")==0?1:2;
}
static int LookupPhoneme(CompileContext *ctx,const char *text,int control)
{
    (void)ctx;(void)text;(void)control;return -1;
}
#include "compiler_source_reference.inc"
_Static_assert(sizeof(RustCompilerToken)==280,"fixed compiler token ABI");
static uint32_t seed=0x93859273;
static uint32_t Next(void) {seed^=seed<<13;seed^=seed>>17;seed^=seed<<5;return seed;}
static unsigned long comparisons;
static int Compare(void *session,int slot,CompileContext *ctx,int kind)
{
    RustCompilerToken token;
    ctx->diagnostic=0;
    TEST_ASSERT(espeak_rs_compiler_source_next(session,slot,kind,ctx->linenum,&token));
    int expected=NextItem(ctx,kind);
    int actual=token.status;
    if(token.status==0)actual=(kind==tNUMBER||kind==tSIGNEDNUMBER)?token.value:(kind==tSTRING?0:-1);
    TEST_ASSERT(actual==expected);
    TEST_ASSERT(token.line==ctx->linenum);
    TEST_ASSERT(token.diagnostic==ctx->diagnostic);
    TEST_ASSERT(espeak_rs_compiler_source_eof(session,slot)==(feof(ctx->f_in)!=0));
    if(token.length>=0) {
        TEST_ASSERT(token.terminator==ctx->item_terminator);
        memcpy(ctx->actual_string,token.bytes,token.length+1);
    } else TEST_ASSERT(expected==-2||expected==-1||expected==1);
    TEST_ASSERT(memcmp(ctx->actual_string,ctx->item_string,256)==0);
    comparisons++;
    return expected;
}
static void Episode(void *session,const char *path,const unsigned char *bytes,size_t length)
{
    FILE *f=fopen(path,"wb");TEST_ASSERT(f!=NULL);
    TEST_ASSERT(fwrite(bytes,1,length,f)==length);fclose(f);
    int error_code=-1;
    int slot=espeak_rs_compiler_source_open(session,path,0,0,&error_code);
    TEST_ASSERT(slot==1&&error_code==0);
    CompileContext ctx={0};ctx.f_in=fopen(path,"rb");TEST_ASSERT(ctx.f_in!=NULL);
    ctx.linenum=1;ctx.item_terminator=0x55;memset(ctx.item_string,0x55,256);memset(ctx.actual_string,0x55,256);
    int numeric=1;
    // C's isdigit(*char) is undefined for negative bytes other than EOF.
    for(size_t i=0;i<length;i++)if(bytes[i]>=128)numeric=0;
    for(unsigned i=0;i<length*4+20&&!feof(ctx.f_in);i++) {
        if(Next()%7==0) {
            int line=ctx.linenum;
            TEST_ASSERT(espeak_rs_compiler_source_check(session,slot,&line)==CheckNextChar(&ctx));
            TEST_ASSERT(line==ctx.linenum);
            comparisons++;
        }
        int kinds[]={tSTRING,tNUMBER,tSIGNEDNUMBER,tOPENBRACKET,tKEYWORD};
        int kind=kinds[Next()%5];
        if(!numeric&&(kind==tNUMBER||kind==tSIGNEDNUMBER))kind=tSTRING;
        Compare(session,slot,&ctx,kind);
        if(Next()%5==0) {
            int line=espeak_rs_compiler_source_unget(session,slot);
            UngetItem(&ctx);TEST_ASSERT(line==ctx.linenum);
            Compare(session,slot,&ctx,kind);
        }
    }
    TEST_ASSERT(feof(ctx.f_in));
    Compare(session,slot,&ctx,tSTRING);
    espeak_rs_compiler_source_rewind(session,slot);rewind(ctx.f_in);ctx.linenum=1;
    for(unsigned i=0;i<length*4+20&&!feof(ctx.f_in);i++)Compare(session,slot,&ctx,tSTRING);
    TEST_ASSERT(feof(ctx.f_in));
    fclose(ctx.f_in);
}
static void Guards(void *session,const char *path)
{
    int error_code=-1;
    TEST_ASSERT(espeak_rs_compiler_source_open(session,path,13,0,&error_code)==0&&error_code!=0);
    TEST_ASSERT(espeak_rs_compiler_source_open(NULL,path,0,0,&error_code)==0);
    TEST_ASSERT(espeak_rs_compiler_source_eof(session,0));
    TEST_ASSERT(espeak_rs_compiler_source_eof(session,99));
    RustCompilerToken token;
    memset(&token,0x55,sizeof(token));RustCompilerToken old=token;
    TEST_ASSERT(!espeak_rs_compiler_source_next(session,99,tSTRING,1,&token));
    TEST_ASSERT(memcmp(&token,&old,sizeof(token))==0);
    TEST_ASSERT(!espeak_rs_compiler_source_next(session,1,tSTRING,1,NULL));
    TEST_ASSERT(!espeak_rs_compiler_source_next(session,1,11,1,&token));
    TEST_ASSERT(memcmp(&token,&old,sizeof(token))==0);
    TEST_ASSERT(!espeak_rs_compiler_source_open(session,"compiler-source-file-does-not-exist",1,0,&error_code));
    TEST_ASSERT(error_code==2&&espeak_rs_compiler_source_eof(session,2));
    unsigned char output[16];memset(output,0x55,sizeof(output));
    TEST_ASSERT(!espeak_rs_compiler_source_line(session,1,output,1));
    TEST_ASSERT(!espeak_rs_compiler_source_line(session,1,output,4097));
    for(unsigned i=0;i<sizeof(output);i++)TEST_ASSERT(output[i]==0x55);
}
static void Chunks(void *session,const char *path)
{
    for(unsigned width=2;width<=256;width++) {
        FILE *f=fopen(path,"rb");TEST_ASSERT(f);
        espeak_rs_compiler_source_rewind(session,1);
        char actual[256],expected[256];memset(actual,0x55,256);memset(expected,0x55,256);
        int result;
        do {
            result=fgets(expected,width,f)!=NULL;
            TEST_ASSERT(espeak_rs_compiler_source_line(session,1,(unsigned char *)actual,width)==result);
            TEST_ASSERT(memcmp(actual,expected,256)==0);
            TEST_ASSERT(espeak_rs_compiler_source_eof(session,1)==(feof(f)!=0));
            comparisons++;
        } while(result);
        fclose(f);
    }
}
static void TextChunks(void *session,const char *path)
{
    const unsigned char text[]="tune a\r\nhead\r 2\r\n\x1atune hidden";
    FILE *f=fopen(path,"wb");TEST_ASSERT(f);TEST_ASSERT(fwrite(text,1,sizeof(text)-1,f)==sizeof(text)-1);fclose(f);
    int error_code;
    TEST_ASSERT(espeak_rs_compiler_source_open(session,path,0,1,&error_code)==1);
    unsigned char output[32];memset(output,0x55,sizeof(output));
    TEST_ASSERT(espeak_rs_compiler_source_line(session,1,output,sizeof(output)));
    TEST_ASSERT(strcmp((char *)output,"tune a\n")==0);
    for(unsigned i=8;i<sizeof(output);i++)TEST_ASSERT(output[i]==0x55);
    TEST_ASSERT(espeak_rs_compiler_source_line(session,1,output,sizeof(output)));
    TEST_ASSERT(strcmp((char *)output,"head\r 2\n")==0);
    memset(output,0x55,sizeof(output));
    TEST_ASSERT(!espeak_rs_compiler_source_line(session,1,output,sizeof(output)));
    for(unsigned i=0;i<sizeof(output);i++)TEST_ASSERT(output[i]==0x55);
    espeak_rs_compiler_source_rewind(session,1);
    TEST_ASSERT(!espeak_rs_compiler_source_eof(session,1));
}
int main(void)
{
    unsigned char spaces[256],digits[256];
    for(int i=0;i<256;i++) {spaces[i]=isspace(i)!=0;digits[i]=isdigit(i)!=0;}
    int error_code;
    void *session=espeak_rs_compiler_sources_begin(spaces,digits,&error_code);TEST_ASSERT(session&&error_code==0);
    TEST_ASSERT(!espeak_rs_compiler_sources_begin(spaces,digits,&error_code)&&error_code==11);
    char path[256];snprintf(path,sizeof(path),"compiler-source-%ld.tmp",(long)getpid());
    const char *edges[]={"","/","//","//comment","\\","a\\","( ) , len= 12 a\\(b)","  \t\n// ignored\n -12x 12x -x","\xef\xbb\xbf é \xff"};
    for(unsigned i=0;i<sizeof(edges)/sizeof(edges[0]);i++)Episode(session,path,(const unsigned char *)edges[i],strlen(edges[i]));
    for(unsigned episode=0;episode<1024;episode++) {
        unsigned char bytes[2048];size_t length=0;
        for(unsigned i=0;i<200;i++) {
            const char *parts[]={"a ","12x ","-42,","\n","\t","(",")","len= ","// comment\n","a\\(b ","/x ","\v","\f","\r","0 "," ","-x ","a\\,b "};
            const char *part=parts[Next()%(sizeof(parts)/sizeof(parts[0]))];size_t n=strlen(part);
            if(length+n>=sizeof(bytes))break;
            memcpy(bytes+length,part,n);length+=n;
        }
        Episode(session,path,bytes,length);
    }
    const unsigned char embedded_nul[]={'a',0,'b',' ','1','2',0,'9',' ','x'};
    Episode(session,path,embedded_nul,sizeof(embedded_nul));
    Guards(session,path);Chunks(session,path);TextChunks(session,path);
    espeak_rs_compiler_sources_end(session);
    // Closing returns the retained pool to the next serialized compiler.
    session=espeak_rs_compiler_sources_begin(spaces,digits,&error_code);TEST_ASSERT(session);
    Episode(session,path,(const unsigned char *)"again",5);
    espeak_rs_compiler_sources_end(session);espeak_rs_compiler_sources_end(NULL);
    TEST_ASSERT(unlink(path)==0);
    printf("%lu actual C token/check/replay comparisons; source guards and Windows text chunks passed\n",comparisons);
    return 0;
}
