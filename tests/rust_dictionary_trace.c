/* Frozen actual C formatting vs sized native Rust trace rendering.
 * SPDX-License-Identifier: GPL-3.0-or-later */
#include "config.h"
#include "test_assert.h"
#include "common.h"
#include "translate.h"
#include "mnemonics.h"
#include "rust_data.h"
#include <stdbool.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <limits.h>
#define print_dictionary_flags ReferenceFlags
#define DecodeRule ReferenceRule
#include "dictionary_trace_reference.inc"
#undef print_dictionary_flags
#undef DecodeRule
_Static_assert(sizeof(RustMatchHost)==(sizeof(void *)==8?56:32),"native matcher trace extent layout");
static uint32_t seed=0x75173831;
static uint32_t Next(void) {seed^=seed<<13;seed^=seed>>17;seed^=seed<<5;return seed;}
static void Token(unsigned char *rule,size_t *length)
{
    unsigned char rb;
    switch(Next()%8) {
    case 0:rule[(*length)++]=RULE_DOLLAR;rule[(*length)++]=Next()%64;break;
    case 1:rule[(*length)++]=RULE_ENDING;for(int i=0;i<3;i++)rule[(*length)++]=Next();break;
    case 2:rule[(*length)++]=RULE_LETTERGP;rule[(*length)++]='A'+Next()%7;break;
    case 3:rule[(*length)++]=RULE_LETTERGP2;rule[(*length)++]=Next();break;
    case 4:rb=10+Next()%22;if(rb==14||rb==17||rb==18||rb==28)rb=19;rule[(*length)++]=rb;break;
    default:rule[(*length)++]=32+Next()%224;break;
    }
}
static void Guards(void)
{
    unsigned char output[80],rule[]={RULE_LINENUM,2,1,RULE_CONDITION,1,RULE_PHONEMES};
    memset(output,0x55,sizeof(output));
    TEST_ASSERT(espeak_rs_decode_rule("a",1,rule,sizeof(rule),0,CHAR_MIN<0,sizeof(char *),output,sizeof(output))==10);
    TEST_ASSERT(strcmp((char *)output,"    1:\t?1 ")==0);
    for(size_t i=11;i<sizeof(output);i++)TEST_ASSERT(output[i]==0x55);
    const unsigned char malformed[][4]={{RULE_DOLLAR},{RULE_ENDING,1,1},{RULE_LETTERGP,'H'},{RULE_LETTERGP2},{RULE_LINENUM,1},{'a'}};
    const size_t lengths[]={1,3,2,1,2,1};
    for(size_t i=0;i<6;i++) {
        memset(output,0x55,sizeof(output));
        TEST_ASSERT(espeak_rs_decode_rule("a",1,malformed[i],lengths[i],0,1,8,output,sizeof(output))==-1);
        for(size_t j=0;j<sizeof(output);j++)TEST_ASSERT(output[j]==0x55);
    }
    TEST_ASSERT(espeak_rs_decode_rule(NULL,1,rule,sizeof(rule),0,1,8,output,sizeof(output))==-1);
    TEST_ASSERT(espeak_rs_decode_rule("a",200,rule,sizeof(rule),0,1,8,output,sizeof(output))==-1);
    TEST_ASSERT(espeak_rs_decode_rule("a",1,rule,0,0,1,8,output,sizeof(output))==-1);
    TEST_ASSERT(espeak_rs_dictionary_flags(NULL,output,sizeof(output))==-1);
}
int main(void)
{
    for(unsigned i=0;i<200000;i++) {
        unsigned flags[2]={Next(),Next()};char expected[512];unsigned char actual[512];
        size_t capacity=6+Next()%507;
        memset(expected,0x55,sizeof(expected));memset(actual,0x55,sizeof(actual));
        ReferenceFlags(flags,expected,(int)capacity);
        TEST_ASSERT(espeak_rs_dictionary_flags(flags,actual,capacity)==(int)strlen(expected));
        TEST_ASSERT(memcmp(expected,actual,sizeof(expected))==0);
    }
    for(unsigned i=0;i<100000;i++) {
        unsigned char encoded[512];size_t length=0;unsigned mode=sizeof(char *)>=8?i%3:0;int control=Next();
        if(mode==1) {encoded[length++]=RULE_LINENUM;encoded[length++]=1+Next()%255;encoded[length++]=1+Next()%255;}
        if(mode==2) {encoded[length++]=RULE_CONDITION;encoded[length++]=Next();}
        if(mode==0 && Next()%2) {
            encoded[length++]=Next()%2?RULE_PRE:RULE_PRE_ATSTART;
            for(unsigned token=0;token<4;token++)Token(encoded,&length);
            encoded[length++]=RULE_POST;
        }
        for(unsigned token=0;token<8;token++)Token(encoded,&length);
        encoded[length++]=Next()%2?RULE_PHONEMES:0;
        const char *groups[]={"","a","abc","é","αβ",""};
        const char *group=groups[Next()%6];char expected[80];unsigned char actual[80];
        memset(expected,0x55,sizeof(expected));memset(actual,0x55,sizeof(actual));
        ReferenceRule(group,strlen(group),(char *)encoded,control,expected);
        int result=espeak_rs_decode_rule(group,strlen(group),encoded,length,control,CHAR_MIN<0,sizeof(char *),actual,sizeof(actual));
        if(result!=(int)strlen(expected)||memcmp(expected,actual,sizeof(expected))) {
            fprintf(stderr,"rule trace mismatch case=%u mode=%u length=%zu result=%d C=[%s] Rust=[%s]\n",i,mode,length,result,expected,actual);abort();
        }
    }
    Guards();
    puts("200000 actual C flag traces and 100000 bounded C rule traces; unsafe-prefix regression and ten admission guards passed");
    return 0;
}
