/* Complete frozen C table controllers vs retained Rust table owner.
 * SPDX-License-Identifier: GPL-3.0-or-later */
#include <stdio.h>
#include <stdlib.h>
#include <stdint.h>
#include <string.h>
#include <stdarg.h>
#include "test_assert.h"
#include "speech.h"
#include "common.h"
#include "mnemonics.h"
#include "synthesize.h"
#include "rust_compiler_tables.h"
enum {tSTRING=2};
typedef struct {
    PHONEME_TAB *phoneme_out,*phoneme_tab2;
    int n_phcodes_list[N_PHONEME_TABS];PHONEME_TAB_LIST phoneme_tab_list2[N_PHONEME_TABS];
    int n_phoneme_tabs,n_phcodes,linenum,error_count,next_calls;char item_string[256],next[256];
    FILE *f_errors,*f_phtab;
} CompileContext;
static void error(CompileContext *ctx,const char *format,...)
{va_list args;va_start(args,format);vfprintf(ctx->f_errors,format,args);fputc('\n',ctx->f_errors);va_end(args);ctx->error_count++;}
static int NextItem(CompileContext *ctx,int type)
{TEST_ASSERT(type==tSTRING);ctx->next_calls++;memcpy(ctx->item_string,ctx->next,256);return 0;}
static void CaptureFree(void *pointer) {(void)pointer;} /* retain rows for post-pack state comparison */
#pragma push_macro("USE_RUST_CORE")
#undef USE_RUST_CORE
#define free CaptureFree
#include "compiler_tables_reference.inc"
#undef free
#pragma pop_macro("USE_RUST_CORE")
_Static_assert(sizeof(PHONEME_TAB)==16,"phoneme record layout");
_Static_assert(sizeof(void *)!=8||sizeof(RustCompilerTables)==64,"table descriptor ABI");
_Static_assert(sizeof(void *)!=8||sizeof(RustCompilerTableResult)==32,"table result ABI");
typedef struct {
    CompileContext ctx;uint64_t owner;RustCompilerTableResult view;RustCompilerTables *descriptor;
    RustCompilerTableResult *publication;char *loan;PHONEME_TAB *target;
    int inspect,mutate,fail_write,events;
} Effects;
static unsigned comparisons,episodes,writes;static uint32_t rng=0x4381b70d;
static uintptr_t view_address;
static void Diagnostic(void *,int32_t,int32_t,const unsigned char *,size_t);
static uint32_t Next(void) {rng=rng*1664525u+1013904223u;return rng;}
static unsigned char a[1024*1024],b[1024*1024];
static void Reentry(Effects *e)
{
    e->events++;
    if(e->inspect){RustCompilerTableResult out;memset(&out,0xa5,sizeof(out));
        RustCompilerTables nested={.owner=e->owner,.context=e,.error=Diagnostic};
        TEST_ASSERT(espeak_rs_compiler_tables_call(&nested,RUST_TABLE_FINISH,&out)==11);
        TEST_ASSERT(out.tables==(int32_t)0xa5a5a5a5);TEST_ASSERT(espeak_rs_compiler_tables_end(e->owner)==11);
        TEST_ASSERT(espeak_rs_compiler_tables_begin()==0);TEST_ASSERT(e->publication->tables==(int32_t)0xa5a5a5a5);
    }
    if(e->mutate){if(e->loan){memset(e->loan,'X',strlen(e->loan));}if(e->target){memset(e->target,0x5a,sizeof(*e->target));}if(e->descriptor){memset(e->descriptor,0,sizeof(*e->descriptor));}}
}
static int Base(void *opaque,unsigned char *out,size_t width)
{Effects *e=opaque;TEST_ASSERT(width==256);NextItem(&e->ctx,tSTRING);memcpy(out,e->ctx.item_string,256);Reentry(e);return 0;}
static void Diagnostic(void *opaque,int32_t code,int32_t value,const unsigned char *bytes,size_t length)
{
    Effects *e=opaque;CompileContext *ctx=&e->ctx;char name[256];TEST_ASSERT(length<256);memcpy(name,bytes,length);name[length]=0;
    Reentry(e);
    switch(code){
    case 1:error(ctx,"Bad phoneme name '%s'",name);break;
    case 2:error(ctx,"compile: unknown phoneme table: '%s'",name);break;
    case 3:error(ctx,"Phoneme reference not found: '%s'",name);break;
    case 4:error(ctx,"Phoneme [%s] not declared, referenced at line %d",name,value);ctx->error_count++;break;
    case 5:error(ctx,"Too many phonemetables");break;
    case 6:error(ctx,"Out of memory");break;
    case 7:error(ctx,"Can't find base phonemetable '%s'",name);break;
    case 8:error(ctx,"Cannot find phoneme '%s' to import.",name);break;
    case 9:error(ctx,"Phoneme import will override set properties.");break;
    default:TEST_ASSERT(0);
    }
}
static int Write(void *opaque,const unsigned char *bytes,size_t length)
{Effects *e=opaque;Reentry(e);TEST_ASSERT(fwrite(bytes,1,length,e->ctx.f_phtab)==length);writes++;return e->fail_write;}
static int Call(Effects *e,unsigned operation,const char *name,int control,PHONEME_TAB *target)
{
    RustCompilerTables d={e->owner,e,name,target,e->ctx.linenum,control,Base,Diagnostic,Write};
    e->descriptor=&d;e->publication=&e->view;memset(&e->view,0xa5,sizeof(e->view));
    int status=espeak_rs_compiler_tables_call(&d,operation,&e->view);
    if(status==0){e->ctx.phoneme_tab2=e->view.current;e->ctx.n_phoneme_tabs=e->view.tables;e->ctx.n_phcodes=e->view.count;}
    e->descriptor=NULL;return status;
}
static size_t Contents(FILE *file,unsigned char *out)
{TEST_ASSERT(fflush(file)==0);TEST_ASSERT(fseek(file,0,SEEK_END)==0);long n=ftell(file);TEST_ASSERT(n>=0&&n<1024*1024);rewind(file);TEST_ASSERT(fread(out,1,(size_t)n,file)==(size_t)n);TEST_ASSERT(fseek(file,0,SEEK_END)==0);return (size_t)n;}
static void Check(CompileContext *c,Effects *e)
{
    TEST_ASSERT(c->n_phoneme_tabs==e->ctx.n_phoneme_tabs&&c->n_phcodes==e->ctx.n_phcodes);
    TEST_ASSERT(c->error_count==e->ctx.error_count&&c->next_calls==e->ctx.next_calls);
    if(c->n_phoneme_tabs){TEST_ASSERT(memcmp(c->phoneme_tab2,e->ctx.phoneme_tab2,256*sizeof(PHONEME_TAB))==0);}
    size_t x=Contents(c->f_errors,a),y=Contents(e->ctx.f_errors,b);TEST_ASSERT(x==y&&memcmp(a,b,x)==0);comparisons++;
}
static void Init(CompileContext *c,Effects *e)
{
    memset(c,0,sizeof(*c));memset(e,0,sizeof(*e));c->f_errors=tmpfile();c->f_phtab=tmpfile();e->ctx.f_errors=tmpfile();e->ctx.f_phtab=tmpfile();
    TEST_ASSERT(c->f_errors&&c->f_phtab&&e->ctx.f_errors&&e->ctx.f_phtab);e->owner=espeak_rs_compiler_tables_begin();TEST_ASSERT(e->owner);
}
static void Close(CompileContext *c,Effects *e)
{for(int i=0;i<c->n_phoneme_tabs;i++){free(c->phoneme_tab_list2[i].phoneme_tab_ptr);}TEST_ASSERT(espeak_rs_compiler_tables_end(e->owner)==0);TEST_ASSERT(espeak_rs_compiler_tables_end(e->owner)==9);fclose(c->f_errors);fclose(c->f_phtab);fclose(e->ctx.f_errors);fclose(e->ctx.f_phtab);}
static void Pair(unsigned trial,unsigned tables)
{
    CompileContext c;Effects e;Init(&c,&e);char names[149][64],name[256];
    for(unsigned t=0;t<tables;t++){
        if(t){EndPhonemeTable(&c);TEST_ASSERT(Call(&e,RUST_TABLE_FINISH,NULL,0,NULL)==0);Check(&c,&e);}
        snprintf(names[t],sizeof(names[t]),"table-%03u%s",t,(trial%7==0)?"-abcdefghijklmnopqrstuvwxyz123456789":"");
        const char *base="_";if(t&&trial%5!=0){base=c.phoneme_tab_list2[Next()%t].name;}if(t&&trial%13==0)base="missing";
        snprintf(c.next,sizeof(c.next),"%s",base);memcpy(e.ctx.next,c.next,256);
        StartPhonemeTable(&c,names[t]);TEST_ASSERT(Call(&e,RUST_TABLE_START,names[t],0,NULL)==0);Check(&c,&e);
        if(t==0){if(view_address){TEST_ASSERT((uintptr_t)e.view.current==view_address);}else{view_address=(uintptr_t)e.view.current;}}
        unsigned lookups=(trial==0?270:12+Next()%45);
        for(unsigned k=0;k<lookups;k++){
            snprintf(name,sizeof(name),"%03x",Next()%512);if(k%23==0)strcpy(name,"NULL");if(k%29==1)name[0]=0;if(k%31==2)strcpy(name,"bad-name");
            c.linenum=e.ctx.linenum=(int)Next();int control=(int)(Next()%4)-1;
            int value=LookupPhoneme(&c,name,control);TEST_ASSERT(Call(&e,RUST_TABLE_LOOKUP,name,control,NULL)==0);TEST_ASSERT(e.view.value==value);Check(&c,&e);
            if(value>=0&&k%3!=0){PHONEME_TAB row=c.phoneme_tab2[value];row.code=(unsigned char)value;row.type=(unsigned char)(Next()%16);row.phflags=Next();row.std_length=(unsigned char)Next();row.end_type=(unsigned char)Next();row.program=(unsigned short)Next();c.phoneme_tab2[value]=e.ctx.phoneme_tab2[value]=row;}
            if(k%9==0){
                snprintf(name,sizeof(name),"%s/%s",c.phoneme_tab_list2[Next()%(t+1)].name,k%18==0?"":"absent");
                PHONEME_TAB *ph=FindPhoneme(&c,name);TEST_ASSERT(Call(&e,RUST_TABLE_FIND,name,0,NULL)==0);TEST_ASSERT((ph!=NULL)==(e.view.found!=NULL));if(ph){TEST_ASSERT(memcmp(ph,e.view.found,sizeof(*ph))==0);}Check(&c,&e);
                PHONEME_TAB left={.mnemonic=0x44332211,.code=42,.type=(k%18?phINVALID:phVOWEL)},right=left;c.phoneme_out=&left;
                snprintf(c.next,sizeof(c.next),"%s",name);NextItem(&c,tSTRING); /* import's token primitive counted separately below */
                c.next_calls--;memcpy(e.ctx.next,c.next,256);ImportPhoneme(&c);NextItem(&e.ctx,tSTRING);
                TEST_ASSERT(Call(&e,RUST_TABLE_IMPORT,e.ctx.item_string,0,&right)==0);TEST_ASSERT(memcmp(&left,&right,sizeof(left))==0);Check(&c,&e);
            }
        }
    }
    if(tables==149){StartPhonemeTable(&c,"extra");TEST_ASSERT(Call(&e,RUST_TABLE_START,"extra",0,NULL)==0);Check(&c,&e);}
    EndPhonemeTable(&c);TEST_ASSERT(Call(&e,RUST_TABLE_FINISH,NULL,0,NULL)==0);Check(&c,&e);
    WritePhonemeTables(&c);TEST_ASSERT(Call(&e,RUST_TABLE_PACK,NULL,0,NULL)==0);Check(&c,&e);
    size_t x=Contents(c.f_phtab,a),y=Contents(e.ctx.f_phtab,b);TEST_ASSERT(x==y&&memcmp(a,b,x)==0);
    Close(&c,&e);episodes++;
}
static void Guards(void)
{
    CompileContext c;Effects e;Init(&c,&e);TEST_ASSERT(Call(&e,RUST_TABLE_START,"base",0,NULL)==0);
    e.inspect=1;e.mutate=1;char loan[256]="bad-name";e.loan=loan;TEST_ASSERT(Call(&e,RUST_TABLE_LOOKUP,loan,1,NULL)==0);TEST_ASSERT(loan[0]=='X');TEST_ASSERT(e.ctx.phoneme_tab2[e.view.value].mnemonic==StringToWord("bad-"));
    e.inspect=0;e.mutate=0;e.loan=NULL;TEST_ASSERT(Call(&e,RUST_TABLE_FINISH,NULL,0,NULL)==0);
    strcpy(e.ctx.next,"base");e.inspect=1;e.mutate=1;strcpy(loan,"child");e.loan=loan;TEST_ASSERT(Call(&e,RUST_TABLE_START,loan,0,NULL)==0);TEST_ASSERT(loan[0]=='X');
    e.inspect=0;e.mutate=0;e.loan=NULL;PHONEME_TAB target={.mnemonic=0x11223344,.code=42,.type=phVOWEL};e.target=&target;strcpy(loan,"base/NULL");e.loan=loan;e.inspect=1;e.mutate=1;
    TEST_ASSERT(Call(&e,RUST_TABLE_IMPORT,loan,0,&target)==0);TEST_ASSERT(target.mnemonic==0x11223344&&target.code==42);TEST_ASSERT(loan[0]=='X');
    e.inspect=0;e.mutate=0;e.loan=NULL;e.target=NULL;
    char wide[256];memset(wide,'w',255);wide[255]=0;int errors=e.ctx.error_count;
    TEST_ASSERT(Call(&e,RUST_TABLE_FIND,wide,0,NULL)==0&&e.view.found==NULL);TEST_ASSERT(e.ctx.error_count==errors+1); /* old C buf[200] is unsafe here */
    for(unsigned i=0;i<270;i++){char name[16];snprintf(name,sizeof(name),"G%03u",i);TEST_ASSERT(Call(&e,RUST_TABLE_LOOKUP,name,1,NULL)==0);if(e.view.value==-1)break;}
    TEST_ASSERT(e.ctx.n_phcodes==255);e.ctx.phoneme_tab2[255].mnemonic=0xaabbccdd;
    TEST_ASSERT(Call(&e,RUST_TABLE_TERMINATE,NULL,0,NULL)==0);TEST_ASSERT(e.ctx.phoneme_tab2[255].mnemonic==0xaabbccdd);
    TEST_ASSERT(Call(&e,RUST_TABLE_FINISH,NULL,0,NULL)==0);e.inspect=1;e.fail_write=28;
    TEST_ASSERT(Call(&e,RUST_TABLE_PACK,NULL,0,NULL)==28);e.inspect=0;TEST_ASSERT(Call(&e,RUST_TABLE_PACK,NULL,0,NULL)==22);
    size_t packed=Contents(e.ctx.f_phtab,a);TEST_ASSERT(packed>609&&memcmp(a+604,"child",5)==0);
    RustCompilerTables d={e.owner,&e,NULL,NULL,0,0,Base,Diagnostic,Write};RustCompilerTableResult out;memset(&out,0xa5,sizeof(out));
    TEST_ASSERT(espeak_rs_compiler_tables_call(&d,7,&out)==22&&out.tables==(int32_t)0xa5a5a5a5);
    char raw[256];memset(raw,'X',sizeof(raw));d.name=raw;TEST_ASSERT(espeak_rs_compiler_tables_call(&d,RUST_TABLE_LOOKUP,&out)==22);d.name="x";d.target=NULL;
    TEST_ASSERT(espeak_rs_compiler_tables_call(&d,RUST_TABLE_IMPORT,&out)==22);d.error=NULL;TEST_ASSERT(espeak_rs_compiler_tables_call(&d,RUST_TABLE_FINISH,&out)==22);
    TEST_ASSERT(espeak_rs_compiler_tables_call(NULL,0,&out)==22);TEST_ASSERT(espeak_rs_compiler_tables_call(&d,0,NULL)==22);
    uint64_t owners[3];for(unsigned i=0;i<3;i++){owners[i]=espeak_rs_compiler_tables_begin();TEST_ASSERT(owners[i]);}TEST_ASSERT(espeak_rs_compiler_tables_begin()==0);
    for(unsigned i=0;i<3;i++){TEST_ASSERT(espeak_rs_compiler_tables_end(owners[i])==0);}Close(&c,&e);
    uint64_t owner=espeak_rs_compiler_tables_begin();TEST_ASSERT(owner&&owner!=e.owner);d.owner=e.owner;d.error=Diagnostic;TEST_ASSERT(espeak_rs_compiler_tables_call(&d,RUST_TABLE_FINISH,&out)==9);TEST_ASSERT(espeak_rs_compiler_tables_end(owner)==0);
}
int main(void)
{
    for(unsigned i=0;i<1200;i++){Pair(i,1+i%7);}Pair(1201,149);Guards();
    printf("%u complete frozen C table episodes, %u state/diagnostic/record comparisons, %u packed writes; native loan/reentry/stale/admission/cleanup guards pass\n",episodes,comparisons,writes);return 0;
}
