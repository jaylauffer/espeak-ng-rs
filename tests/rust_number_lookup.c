/* Retained thousands-name controller with trace/state/byte parity.
 * SPDX-License-Identifier: GPL-3.0-or-later */
#include "config.h"
#include "test_assert.h"
#include "translate.h"
#include "rust_number_lookup.h"
#include <limits.h>
#include <stdint.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
static Translator local, global;
static Translator *fixture_translator = &global;
static int control, missing;
static unsigned mask, calls, mutation, variant, omit;
static unsigned long_mode;
static uint64_t trace;
static void Number(uint64_t value) { trace = (trace ^ value) * UINT64_C(1099511628211); }
static int LookupName(void *context, const char *key, char *output)
{
    TEST_ASSERT(context == &local && calls < 16);
    if(long_mode) {
        ++calls;
        if(long_mode==1) { memset(output,31,200); return 1; }
        unsigned length=strcmp(key,"_0of")==0 ? (long_mode==3 ? 12 : 11) : strncmp(key,"_0M",3)==0 ? 159 : 0;
        memset(output,31,length); output[length]=0; return length ? 2 : 0;
    }
    Number((unsigned)missing); Number((unsigned)control); Number((unsigned)global.langopts.numbers2);
    uint32_t hash=UINT32_C(2166136261);
    for(const unsigned char *p=(const unsigned char *)key; *p; ++p) { Number(*p); hash=(hash^*p)*UINT32_C(16777619); }
    Number(0);
    unsigned admitted=(mask >> calls++) & 1;
    unsigned length=admitted ? hash % 11 + 1 : 0;
    if(mutation & 2) length=hash % 7; /* Defined zero flags with nonempty phonemes. */
    for(unsigned i=0;i<length;++i) output[i]=(char)(32+(hash+i)%80);
    output[length]=0;
    if(mutation & 1) { control ^= 1; global.langopts.numbers2 = ((global.langopts.numbers2+64)&0x1c0); local.langopts.numbers ^= NUM_OMIT_1_THOUSAND; }
    int found=admitted ? ((hash&1) ? -7 : 2) : 0;
    Number((uint32_t)found);
    return found;
}
static int Value(void *context, unsigned field)
{
    TEST_ASSERT(context == &local && field < 3);
    return field == 0 ? local.langopts.numbers : field == 1 ? global.langopts.numbers2 : control;
}
static void Missing(void *context, int value) { TEST_ASSERT(context == &local); missing=value; }
static RustNumberLookup table={&local,LookupName,Value,Missing};
#define translator fixture_translator
#define number_control control
#define speak_missing_thousands missing
#define Lookup LookupName
#define LookupThousands ReferenceThousands
#include "number_lookup_reference.inc"
static void Reset(int number_control_value)
{
    local.langopts.numbers=omit ? NUM_OMIT_1_THOUSAND : 0;
    local.langopts.numbers2=0x140; /* Local/global translator options differ. */
    global.langopts.numbers2=variant*64;
    control=number_control_value; missing=-9; calls=0; trace=UINT64_C(14695981039346656037);
}
static unsigned comparisons;
static void Compare(int value, int plex, int exact, int control_value)
{
    char old[200], actual[200]; memset(old,0x97,sizeof(old)); memset(actual,0x97,sizeof(actual));
    Reset(control_value);
    int expected=ReferenceThousands(&local,value,plex,exact,old,sizeof(old));
    int expected_missing=missing, expected_control=control, expected_options=global.langopts.numbers2, expected_numbers=local.langopts.numbers;
    unsigned expected_calls=calls; uint64_t expected_trace=trace;
    Reset(control_value); int result=919;
    TEST_ASSERT(espeak_rs_lookup_thousands(&table,value,plex,exact,actual,sizeof(actual),&result)==0);
    TEST_ASSERT(result==expected && missing==expected_missing && control==expected_control);
    TEST_ASSERT(global.langopts.numbers2==expected_options && local.langopts.numbers==expected_numbers);
    TEST_ASSERT(calls==expected_calls && trace==expected_trace && memcmp(old,actual,sizeof(old))==0);
    ++comparisons;
}
static uint32_t seed=0x39822418;
static unsigned Random(void) { seed^=seed<<13; seed^=seed>>17; seed^=seed<<5; return seed; }
int main(void)
{
    static const int values[]={-100,-1,0,1,2,3,4,10,11,19,20,21,24,99,100,120,121,1000,INT_MAX};
    for(unsigned v=0;v<sizeof(values)/sizeof(values[0]);++v)
    for(int plex=0;plex<=7;++plex) for(int exact=0;exact<4;++exact)
    for(int ctl=0;ctl<2;++ctl) for(variant=0;variant<8;++variant)
    for(unsigned outcome=0;outcome<16;++outcome) {
        mask=outcome ? 1u << (outcome-1) : 0; mutation=0; omit=outcome&1;
        Compare(values[v],plex,exact,ctl);
    }
    for(unsigned trial=0;trial<200000;++trial) {
        mask=Random()&0xffff; mutation=Random()%4; variant=Random()%8; omit=Random()&1;
        Compare((int)(Random()%1000001)-1000,(int)(Random()%14)-3,Random()%8,Random()%4);
    }
    /* Adapter failures never borrow or write an uninitialized foreign tail. */
    char output[200], saved[200]; memset(output,0x97,sizeof(output)); memcpy(saved,output,sizeof(output));
    int found=919; Reset(0); mask=1; mutation=0; variant=0; omit=0;
    TEST_ASSERT(espeak_rs_lookup_thousands(&table,1,1,0,output,1,&found)==-1 && found==919 && memcmp(output,saved,sizeof(output))==0);
    TEST_ASSERT(espeak_rs_lookup_thousands(NULL,1,1,0,output,200,&found)==-1);
    TEST_ASSERT(espeak_rs_lookup_thousands(&table,1,1,0,NULL,200,&found)==-1);
    TEST_ASSERT(espeak_rs_lookup_thousands(&table,1,1,0,output,201,&found)==-1);
    TEST_ASSERT(espeak_rs_lookup_thousands(&table,1,1,0,output,200,NULL)==-1);
    RustNumberLookup invalid=table; invalid.lookup=NULL;
    TEST_ASSERT(espeak_rs_lookup_thousands(&invalid,1,1,0,output,200,&found)==-1 && found==919 && memcmp(output,saved,sizeof(output))==0);
    long_mode=1; Reset(0);
    TEST_ASSERT(espeak_rs_lookup_thousands(&table,1,1,0,output,200,&found)==-1 && found==919 && memcmp(output,saved,sizeof(output))==0);
    long_mode=3; Reset(0);
    TEST_ASSERT(espeak_rs_lookup_thousands(&table,20,1,0,output,200,&found)==-1 && found==919 && memcmp(output,saved,sizeof(output))==0);
    long_mode=2;
    for(size_t capacity=50;capacity<=170;capacity+=55) {
        Reset(0);
        TEST_ASSERT(espeak_rs_lookup_thousands(&table,20,1,0,output,capacity,&found)==-1 && found==919 && memcmp(output,saved,sizeof(output))==0);
    }
    char expected[200]; memset(expected,0x97,sizeof(expected)); Reset(0);
    int result=ReferenceThousands(&local,20,1,0,expected,sizeof(expected)); Reset(0);
    TEST_ASSERT(espeak_rs_lookup_thousands(&table,20,1,0,output,171,&found)==0 && found==result && memcmp(output,expected,sizeof(output))==0);
    printf("%u thousands lookup/state/byte comparisons passed\n",comparisons);
    return 0;
}
