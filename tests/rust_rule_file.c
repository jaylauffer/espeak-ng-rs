/* Actual retained-C rule-file compilation and current production bridges.
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
#include <unistd.h>
#include <espeak-ng/speak_lib.h>
#define LEN_GROUP_NAME 12
#define N_RULES 3000
typedef struct {char name[13];void *start;size_t length;int group3_ix,index;} RGROUP;
typedef struct {
    FILE *f_log;int linenum,error_count,debug_flag;
    char letterGroupsDefined[95];
    char rule_cond[80],rule_pre[80],rule_post[80],rule_match[80],rule_phonemes[80];
    char group_name[13];int group3_ix;
} CompileContext;
#define compile_rule ReferenceCompile
#include "rule_compile_reference.inc"
#include "rule_file_reference.inc"
#undef compile_rule
#include "native_rule_compile.inc"
#include "native_rule_file.inc"
static unsigned compared;
static uint32_t random_state=0x7811eb98;
static uint32_t Next(void) {random_state^=random_state<<13;random_state^=random_state>>17;random_state^=random_state<<5;return random_state;}
static void CompareFiles(FILE *a,FILE *b,const char *source,const char *kind)
{
    fflush(a);fflush(b);long x=ftell(a),y=ftell(b);
    if(x!=y) {fprintf(stderr,"%s length mismatch C=%ld Rust=%ld\n%s",kind,x,y,source);abort();}
    rewind(a);rewind(b);
    for(long i=0;i<x;i++)if(fgetc(a)!=fgetc(b)) {fprintf(stderr,"%s mismatch byte %ld\n%s",kind,i,source);abort();}
}
static void Compare(const char *source,int offset,int prefix,int debug,int letters)
{
    char path[]="/tmp/espeak-rule-file-XXXXXX";int fd=mkstemp(path);TEST_ASSERT(fd>=0);
    FILE *file=fdopen(fd,"wb");TEST_ASSERT(file);size_t length=strlen(source);TEST_ASSERT(fwrite(source,1,length,file)==length);fclose(file);
    FILE *input=fopen(path,"r"),*out_a=tmpfile(),*out_b=tmpfile();TEST_ASSERT(input&&out_a&&out_b);
    CompileContext a={0},b={0};a.f_log=tmpfile();b.f_log=tmpfile();TEST_ASSERT(a.f_log&&b.f_log);
    a.debug_flag=b.debug_flag=debug;memset(a.letterGroupsDefined,letters,95);memset(b.letterGroupsDefined,letters,95);
    a.linenum=b.linenum=72;strcpy(a.group_name,"old");strcpy(b.group_name,"old");
    for(int i=0;i<prefix;i++) {fputc(0x55,out_a);fputc(0x55,out_b);}
    translator->letter_bits_offset=offset;
    TEST_ASSERT(compile_dictrules(&a,input,out_a)==ENS_OK);
    TEST_ASSERT(compile_dictrules_native(&b,path,out_b)==ENS_OK);
    if(a.error_count!=b.error_count) {fprintf(stderr,"error count mismatch C=%d Rust=%d\n%s",a.error_count,b.error_count,source);abort();}
    TEST_ASSERT(a.linenum==b.linenum&&a.group3_ix==b.group3_ix);
    TEST_ASSERT(strcmp(a.group_name,b.group_name)==0);
    TEST_ASSERT(memcmp(a.letterGroupsDefined,b.letterGroupsDefined,95)==0);
    CompareFiles(out_a,out_b,source,"bytecode");CompareFiles(a.f_log,b.f_log,source,"diagnostics");
    fclose(input);fclose(out_a);fclose(out_b);fclose(a.f_log);fclose(b.f_log);unlink(path);compared++;
}
int main(void)
{
    TEST_ASSERT(espeak_Initialize(AUDIO_OUTPUT_SYNCHRONOUS,0,NULL,0)>0);TEST_ASSERT(espeak_SetVoiceByName("en")==EE_OK);
    int saved_offset=translator->letter_bits_offset;
    static const char *names[]={"a","b","9","0x100","0x101","0x61","ab","abc","é","α","😊","0xz","0x0X61"};
    static const char *rules[]={"$group a\n","$group b\n","?1 _A)$group(B a\n","?!2 $group(Sep2 b\n","$group a b\n","\t$group a\n","$group // comment\n","\n"};
    char source[16000];
    for(unsigned episode=0;episode<1200;episode++) {
        size_t used=0;
        if(episode%2)used+=snprintf(source+used,sizeof(source)-used,".L01 a abc a_b e\n.L00 x z\n");
        for(unsigned group=0;group<Next()%8+1;group++) {
            used+=snprintf(source+used,sizeof(source)-used,".group %s\n",names[Next()%13]);
            for(unsigned rule=0;rule<Next()%10+1;rule++)used+=snprintf(source+used,sizeof(source)-used,"%s",rules[Next()%8]);
            if(Next()%3==0)used+=snprintf(source+used,sizeof(source)-used,".replace\na bc\nxyz\n\n");
        }
        Compare(source,episode%3==0?'a':0,episode%4,episode%2,episode%7==0);
    }
    const char *edges[]={"","ignored\n",".replace\n a x y\n b \n",".Lx1 a\n.L99 b\n.L01 a\n.L01 b\n",".group a\na a\n.L01 xyz a\n$group b\n",".group a\na a\n.group a\na b\n.group a\na a\n",".group abc\n$group a\n.group a\na a\n.group a\na b\n",".group α\nα a\n.group α\nα b\n",".group a\na a\r\n\r\n",".group 0x100\n$group a\n.group 0x101\n$group b\n",".group a\na a\n.what\n$group b\n"};
    for(unsigned i=0;i<sizeof(edges)/sizeof(*edges);i++)for(int offset=0;offset<2;offset++)Compare(edges[i],offset?'a':0,i%4,1,0);
    for(int offset=0;offset<2;offset++) {
        Compare(".L01 j// voiced\n",offset?'a':0,0,0,0);
        Compare(".L01 abc defghi xyz\n.L02 z",offset?'a':0,1,0,0);
    }
    for(unsigned width=1;width<490;width+=7) {
        strcpy(source,".L01 ");memset(source+5,'a',width);strcpy(source+5+width,"\n.group a\na a\n");Compare(source,0,width%4,0,0);
    }
    translator->letter_bits_offset=saved_offset;espeak_Terminate();
    printf("%u complete actual C rule-file bytecode/state/diagnostic comparisons passed\n",compared);return 0;
}
