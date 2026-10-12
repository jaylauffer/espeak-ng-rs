/* Original compiler byte records and diagnostics vs frozen generic adapters.
 * SPDX-License-Identifier: GPL-3.0-or-later */
#include "config.h"
#include "test_assert.h"
#include "common.h"
#include "dictionary.h"
#include "translate.h"
#include "rust_data.h"
#include <stdbool.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <espeak-ng/speak_lib.h>

typedef struct {
    FILE *f_log;
    int linenum, error_count, debug_flag;
    char letterGroupsDefined[95];
    char rule_cond[80],rule_pre[80],rule_post[80],rule_match[80],rule_phonemes[80];
    char group_name[13];
} CompileContext;

static size_t allocation_length;
static void *RecordMalloc(size_t size) {allocation_length=size;return malloc(size);}
#define malloc RecordMalloc
#define compile_rule ReferenceCompile
#include "rule_compile_reference.inc"
#undef compile_rule
#include "native_rule_compile.inc"
#undef malloc

static unsigned compared, guards;
static uint32_t random_state=0x811cc27;
static uint32_t Next(void) {random_state^=random_state<<13;random_state^=random_state>>17;random_state^=random_state<<5;return random_state;}
static void Compare(const char *source, const char *group, int debug, int line, int letters)
{
    char input[500];strcpy(input,source);
    CompileContext a={0},b={0};
    a.f_log=tmpfile();b.f_log=tmpfile();TEST_ASSERT(a.f_log && b.f_log);
    a.linenum=b.linenum=line;a.debug_flag=b.debug_flag=debug;
    memset(a.letterGroupsDefined,letters,95);memset(b.letterGroupsDefined,letters,95);
    strcpy(a.group_name,group);strcpy(b.group_name,group);
    allocation_length=0;char *expected=ReferenceCompile(&a,input);size_t expected_length=allocation_length;
    allocation_length=0;char *actual=compile_rule(&b,input);size_t actual_length=allocation_length;
    if ((expected==NULL)!=(actual==NULL) || expected_length!=actual_length || (expected && memcmp(expected,actual,expected_length))) {
        fprintf(stderr,"rule byte mismatch: [%s], group [%s], expected %zu actual %zu\n",source,group,expected_length,actual_length);abort();
    }
    TEST_ASSERT(a.error_count==b.error_count);
    fflush(a.f_log);fflush(b.f_log);long size_a=ftell(a.f_log),size_b=ftell(b.f_log);
    TEST_ASSERT(size_a==size_b && size_a<8192);
    char log_a[8192],log_b[8192];rewind(a.f_log);rewind(b.f_log);
    TEST_ASSERT(fread(log_a,1,size_a,a.f_log)==(size_t)size_a);TEST_ASSERT(fread(log_b,1,size_b,b.f_log)==(size_t)size_b);
    if(memcmp(log_a,log_b,size_a)) {fprintf(stderr,"diagnostic mismatch: [%s]\nC: %.*sRust: %.*s",source,(int)size_a,log_a,(int)size_b,log_b);abort();}
    free(expected);free(actual);fclose(a.f_log);fclose(b.f_log);compared++;
}
static void Guard(const char *input)
{
    CompileContext ctx={0};ctx.f_log=tmpfile();TEST_ASSERT(ctx.f_log);
    char *result=compile_rule(&ctx,(char *)input);
    if(result!=NULL || ctx.error_count!=1) {fprintf(stderr,"guard [%s]: result %p errors %d\n",input,(void *)result,ctx.error_count);rewind(ctx.f_log);int c;while((c=fgetc(ctx.f_log))!=EOF)fputc(c,stderr);abort();}
    fclose(ctx.f_log);guards++;
}
int main(void)
{
    TEST_ASSERT(espeak_Initialize(AUDIO_OUTPUT_SYNCHRONOUS,0,NULL,0)>0);
    TEST_ASSERT(espeak_SetVoiceByName("en")==EE_OK);
    static const char *tokens[]={"_","A","B","C","H","F","G","Y","D","K","N","V","Z","+","<","@","&","%","#","!","T","W","X","J","L01","L94","L00","L95","Lx1","L1x","$unpr","$noprefix","$list","$w_alt1","$w_alt6","$w_alt","$p_alt1","$p_alt6","$p_alt","$wrong","Seivdfqtbam123","P2","0x414243","0x4142z","\\101","\\377","\\_","\\A","abc","","\\"};
    static const char *matches[]={"a","abc","$group","9","१२","Z","?","é"};
    static const char *groups[]={"","a","abc","9","9x","z","é"};
    static const char *conditions[]={"","?1 ","?!2 ","?0 ","?255 ","?-3 ","?12tail ","?!0 ","?+3 ","?999 ","?x "};
    static const char *phonemes[]={"a","a b","","?","a||b","_^_FR"};
    char input[500];
    for(unsigned i=0;i<24000;i++) {
        const char *pre=tokens[Next()%(sizeof(tokens)/sizeof(*tokens))];
        const char *post=tokens[Next()%(sizeof(tokens)/sizeof(*tokens))];
        snprintf(input,sizeof(input),"%s%s)%s(%s %s%s",conditions[Next()%11],pre,matches[Next()%8],post,phonemes[Next()%6],i%3==0?"\r\n":"");
        Compare(input,groups[Next()%7],Next()%2,Next()%100000,Next()%2);
    }
    const char *syntax[]={"","  ","a","a a","(A a","a( A a","?1 a a","a?b a","a a?b","a\tb\nafter","$group a","_A)a(B a","0x610062 a","\\000 a","a(\\000 a","a(Sei a"};
    for(unsigned i=0;i<sizeof(syntax)/sizeof(*syntax);i++)Compare(syntax[i],"a",1,255,1);
    for(unsigned byte=1;byte<256;byte++) {
        /* Literal byte coverage never subjects negative bytes to C ctype. */
        snprintf(input,sizeof(input),"\\%03o)a(\\%03o a",byte,byte);
        Compare(input,"a",0,1,1);
    }
    for(unsigned width=1;width<=79;width++) {
        memset(input,'a',width);strcpy(input+width," a");Compare(input,"a",1,1,1);
    }
    memset(input,'a',80);strcpy(input+80," a");Guard(input);
    Guard("AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA)a a");
    Guard("a(L a");Guard("a(S99999999999999999999 a");
    Guard("?999999999999999999999 a a");
    /* Short output refusal preserves bytes, while callbacks still execute. */
    CompileContext ctx={0};ctx.f_log=tmpfile();TEST_ASSERT(ctx.f_log);
    uint8_t spaces[256],output[512];for(int i=0;i<256;i++)spaces[i]=isspace2((char)i)!=0;
    RustRuleCompiler descriptor={ctx.group_name,(uint8_t *)ctx.letterGroupsDefined,spaces,1,0,(char)0xff<0,&ctx,rule_encode,rule_digit,rule_report};
    memset(output,0x55,sizeof(output));TEST_ASSERT(espeak_rs_compile_rule(&descriptor,"a ☃",output,1)==0);
    TEST_ASSERT(ctx.error_count==2);for(unsigned i=0;i<sizeof(output);i++)TEST_ASSERT(output[i]==0x55);
    char unterminated[500];memset(unterminated,'a',sizeof(unterminated));TEST_ASSERT(espeak_rs_compile_rule(&descriptor,unterminated,output,sizeof(output))==0);
    TEST_ASSERT(espeak_rs_compile_rule(NULL,"a a",output,sizeof(output))==0);
    fclose(ctx.f_log);espeak_Terminate();
    printf("%u actual C rule records and exact diagnostics; %u malformed/overflow guards passed\n",compared,guards);
    return 0;
}
