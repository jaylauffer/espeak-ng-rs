/* Complete frozen C NextItem scanner/tables vs owned Rust policy.
 * SPDX-License-Identifier: GPL-3.0-or-later */
#include "config.h"
#include "test_assert.h"
#include "rust_compiler_source.h"
#include "rust_compiler_items.h"
#include "speech.h"
#include "phoneme.h"
#include "synthesize.h"
#include <ctype.h>
#include <stdarg.h>
#include <stdint.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <unistd.h>
enum { tENDFILE=1,tSTRING,tNUMBER,tSIGNEDNUMBER,tPHONEMEMNEM,tOPENBRACKET,tKEYWORD,tCONDITION,tPROPERTIES,tINTONATION };
#include "compiler_items_reference.inc"
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
    (void)ctx;(void)text;TEST_ASSERT(control==2);return 41;
}
#include "compiler_source_reference.inc"
_Static_assert(sizeof(RustCompilerToken)==280,"fixed compiler token ABI");
static uint32_t seed=0x93859273;
static uint32_t Next(void) {seed^=seed<<13;seed^=seed>>17;seed^=seed<<5;return seed;}
static unsigned long comparisons;
typedef struct {int diagnostic,line,lookups,inspect,mutate;RustCompilerItem *descriptor;RustCompilerItem admitted;RustItemResult *output;unsigned char *previous;} Effects;
static void Effect(Effects *e)
{if(e->inspect){RustItemResult out;memset(&out,0x55,sizeof(out));TEST_ASSERT(espeak_rs_compiler_item(&e->admitted,&out)==11);TEST_ASSERT(out.line==0x55555555);TEST_ASSERT(e->output->line==0x55555555);}if(e->mutate){memset(e->descriptor,0,sizeof(*e->descriptor));memset(e->previous,0,256);}}
static void Diagnostic(void *opaque,int32_t line,const unsigned char *bytes,size_t length)
{Effects *e=opaque;e->line=line;e->diagnostic=length==strlen("Expected an unsigned number")&&!memcmp(bytes,"Expected an unsigned number",length)?1:2;Effect(e);}
static int Lookup(void *opaque,int32_t line,const unsigned char *bytes,size_t length)
{Effects *e=opaque;TEST_ASSERT(length<=255);e->line=line;e->lookups++;Effect(e);return 41;}
_Static_assert(sizeof(RustItemResult)==272,"complete item result");
_Static_assert(sizeof(void *)!=8||sizeof(RustCompilerItem)==56,"complete item descriptor");
static int Compare(void *session,int slot,CompileContext *ctx,int kind)
{
 Effects e={0};unsigned char previous[256];memcpy(previous,ctx->actual_string,256);RustItemResult out;memset(&out,0x55,sizeof(out));
 RustCompilerItem d={session,slot,kind,ctx->linenum,ctx->item_terminator,previous,&e,Lookup,Diagnostic};e.descriptor=&d;e.admitted=d;e.previous=previous;e.output=&out;
 ctx->diagnostic=0;TEST_ASSERT(espeak_rs_compiler_item(&d,&out)==0);int expected=NextItem(ctx,kind);
 TEST_ASSERT(out.token.value==expected&&out.token.kind==ctx->item_type&&out.line==ctx->linenum&&out.token.terminator==ctx->item_terminator);
 TEST_ASSERT(e.diagnostic==ctx->diagnostic);if(e.lookups||e.diagnostic){TEST_ASSERT(e.line==ctx->linenum);}TEST_ASSERT(espeak_rs_compiler_source_eof(session,slot)==(feof(ctx->f_in)!=0));
 TEST_ASSERT(memcmp(out.token.text,ctx->item_string,256)==0);memcpy(ctx->actual_string,out.token.text,256);comparisons++;return expected;
}
static void Keywords(void)
{for(int table=0;table<3;table++){for(const keywtab_t *p=keyword_tabs[table];p->mnem;p++){int out[2]={0};TEST_ASSERT(espeak_rs_compiler_keyword(7+table,(const unsigned char *)p->mnem,strlen(p->mnem),out)==0);const keywtab_t *first=keyword_tabs[table];while(strcmp(first->mnem,p->mnem)){first++;}TEST_ASSERT(out[0]==first->type&&out[1]==first->data);comparisons++;}}int out[2];TEST_ASSERT(espeak_rs_compiler_keyword(6,NULL,0,out)==0&&out[0]==-1&&out[1]==-1);}
static void ItemGuards(void *session,const char *path)
{FILE *f=fopen(path,"wb");TEST_ASSERT(f);fputs("-x another",f);fclose(f);int error_code;TEST_ASSERT(espeak_rs_compiler_source_open(session,path,0,0,&error_code)==1);unsigned char previous[256]={0};Effects e={.inspect=1,.mutate=1};RustItemResult out;memset(&out,0x55,sizeof(out));RustCompilerItem d={session,1,3,1,44,previous,&e,Lookup,Diagnostic};e.descriptor=&d;e.admitted=d;e.previous=previous;e.output=&out;TEST_ASSERT(espeak_rs_compiler_item(&d,&out)==0&&e.diagnostic==1&&out.token.value==0);e.inspect=e.mutate=0;d=e.admitted;d.lookup=NULL;memset(&out,0x55,sizeof(out));TEST_ASSERT(espeak_rs_compiler_item(&d,&out)==22&&out.line==0x55555555);d=e.admitted;d.slot=0;TEST_ASSERT(espeak_rs_compiler_item(&d,&out)==0&&out.token.value==-2);
 union {RustCompilerItem d;RustItemResult out;} overlap;overlap.d=e.admitted;overlap.d.kind=7;e.descriptor=&overlap.d;e.output=&overlap.out;TEST_ASSERT(espeak_rs_compiler_item(&overlap.d,&overlap.out)==0&&overlap.out.token.value==-1&&strcmp((char *)overlap.out.token.text,"another")==0);int values[2]={55,55};TEST_ASSERT(espeak_rs_compiler_keyword(7,previous,256,values)==22&&values[0]==55&&values[1]==55);}
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
        int kinds[]={tSTRING,tNUMBER,tSIGNEDNUMBER,tOPENBRACKET,tKEYWORD,tCONDITION,tPROPERTIES,tPHONEMEMNEM};
        int kind=kinds[Next()%8];
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
    const char *edges[]={"phoneme procedure include ChangeIfStressed IF isWordEnd nextVowel len= ","","/","//","//comment","\\","a\\","( ) , len= 12 a\\(b)","  \t\n// ignored\n -12x 12x -x","\xef\xbb\xbf é \xff"};
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
    Guards(session,path);Chunks(session,path);TextChunks(session,path);ItemGuards(session,path);Keywords();
    espeak_rs_compiler_sources_end(session);
    // Closing returns the retained pool to the next serialized compiler.
    session=espeak_rs_compiler_sources_begin(spaces,digits,&error_code);TEST_ASSERT(session);
    Episode(session,path,(const unsigned char *)"again",5);
    espeak_rs_compiler_sources_end(session);espeak_rs_compiler_sources_end(NULL);
    TEST_ASSERT(unlink(path)==0);
    printf("%lu complete C NextItem/table/check/replay comparisons; input/reentry/overlap and source guards passed\n",comparisons);
    return 0;
}
