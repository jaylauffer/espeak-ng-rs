/* Independent retained number controllers: byte/trace/state parity.
 * SPDX-License-Identifier: GPL-3.0-or-later */
#include "config.h"
#include "test_assert.h"
#include "translate.h"
#include "phoneme.h"
#include "rust_number_digits.h"
#include <limits.h>
#include <stdint.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>

static Translator local, global;
static Translator *fixture_translator = &global;
static int fixture_control, missing, digit_count;
static char cache[50], ordinal_text[12], alternate[12];
static char *cache_pointer=cache;
static PHONEME_TAB types[256], *table_types[256];
static unsigned calls, mutation, cached, suffix, malayalam, fault;
static int options, options2, variants, initial_control;
static uint64_t mask, trace;
static void Number(uint64_t value) { trace=(trace^value)*UINT64_C(1099511628211); }
static void State(void) {
    Number((uint32_t)local.langopts.numbers); Number((uint32_t)local.langopts.numbers2);
    Number((uint32_t)global.langopts.numbers2); Number((uint32_t)fixture_control); Number((uint32_t)missing);
    Number((uint32_t)digit_count); Number((uint32_t)local.translator_name);
    for(unsigned i=0;i<4;++i) { Number((unsigned char)cache[i]); Number((unsigned char)ordinal_text[i]); Number((unsigned char)alternate[i]); }
}
static int LookupName(void *context, const char *key, char *output)
{
    TEST_ASSERT(context==&local && calls<64); State();
    uint32_t hash=UINT32_C(2166136261);
    for(const unsigned char *p=(const unsigned char *)key;*p;++p) { hash=(hash^*p)*UINT32_C(16777619); Number(*p); } Number(0);
    if(fault==1) { ++calls; memset(output,31,200); return 1; }
    if(fault==2) {
        ++calls;
        if(strcmp(key,"_2X")==0 || strcmp(key,"_1")==0) {
            output[0]=6; output[1]=strcmp(key,"_1")==0 ? (char)200 : 50; output[2]=0; return 1;
        }
        output[0]=0; return 0;
    }
    unsigned present=(unsigned)((mask >> calls++)&1);
    unsigned length=present ? hash%4+1 : (mutation&2) ? hash%3 : 0;
    static const unsigned char codes[]={6,50,51,52,10,15,21,61,98};
    for(unsigned i=0;i<length;++i)output[i]=(char)codes[(hash+i)%(sizeof(codes)/sizeof(codes[0]))]; output[length]=0;
    if(mutation&1) {
        fixture_control ^= 1;
        local.langopts.numbers ^= NUM_SWAP_TENS|NUM_SINGLE_VOWEL|NUM_SINGLE_STRESS_L|NUM_OMIT_1_THOUSAND;
        local.langopts.numbers2 ^= NUM2_MULTIPLE_ORDINAL|NUM2_ORDINAL_NO_AND|NUM2_MYRIADS;
        global.langopts.numbers2=(global.langopts.numbers2+64)&0x1c0;
        digit_count=(digit_count+1)%3;
        cache[0]=cache[0] ? 0 : 6; cache[1]=50; cache[2]=0;
        ordinal_text[0]=ordinal_text[0] ? 0 : 52; ordinal_text[1]=6; ordinal_text[2]=0;
        alternate[0]=alternate[0] ? 0 : 51; alternate[1]=5; alternate[2]=0;
        local.translator_name=local.translator_name==L('m','l') ? L('e','n') : L('m','l');
    }
    int found=present ? ((hash&1) ? -7 : 2) : 0; Number((uint32_t)found); return found;
}
static int Value(void *context, unsigned field) {
    TEST_ASSERT(context==&local && field<6);
    switch(field) { case 0:return local.langopts.numbers; case 1:return global.langopts.numbers2; case 2:return fixture_control; case 3:return local.langopts.numbers2; case 4:return digit_count; default:return local.translator_name; }
}
static void Missing(void *context,int value) { TEST_ASSERT(context==&local); missing=value; }
static const char *Text(void *context,unsigned kind) { TEST_ASSERT(context==&local && kind<3); return kind==0 ? cache_pointer : kind==1 ? ordinal_text : alternate; }
static int Type(void *context,unsigned char code) { TEST_ASSERT(context==&local); return table_types[code] ? table_types[code]->type : -1; }
static RustNumberDigits table={&local,LookupName,Value,Missing,Text,Type};
#define translator fixture_translator
#define number_control fixture_control
#define speak_missing_thousands missing
#define n_digit_lookup digit_count
#define digit_lookup cache_pointer
#define ph_ordinal2 ordinal_text
#define ph_ordinal2x alternate
#define phoneme_tab table_types
#define Lookup LookupName
#define LookupThousands ReferenceThousands
#define LookupNum2 ReferenceTwo
#define LookupNum3 ReferenceThree
#include "number_digits_reference.inc"
static void Reset(void) {
    local.langopts.numbers=options; local.langopts.numbers2=options2; global.langopts.numbers2=variants;
    local.translator_name=malayalam ? L('m','l') : L('e','n');
    fixture_control=initial_control; digit_count=cached%3; missing=-9; calls=0; trace=UINT64_C(14695981039346656037);
    memset(cache,0,sizeof(cache)); memset(ordinal_text,0,sizeof(ordinal_text)); memset(alternate,0,sizeof(alternate));
    if(cached>=3) { cache[0]=6; cache[1]=50; cache[2]=6; cache[3]=51; }
    if(suffix&1) { ordinal_text[0]=52; ordinal_text[1]=6; }
    if(suffix&2) { alternate[0]=51; alternate[1]=5; }
}
static unsigned two_count, three_count;
static void Compare(int value,int plex,int control,unsigned three,unsigned suppress) {
    char old[200],actual[200]; memset(old,0x97,sizeof(old)); memset(actual,0x97,sizeof(actual)); Reset();
    int expected=three ? ReferenceThree(&local,value,old,suppress!=0,plex,control,sizeof(old)) : ReferenceTwo(&local,value,plex,control,old,sizeof(old));
    State(); uint64_t expected_trace=trace; unsigned expected_calls=calls;
    Reset(); int found=919;
    int status=three ? espeak_rs_lookup_num3(&table,value,plex,control,suppress,actual,sizeof(actual)) : espeak_rs_lookup_num2(&table,value,plex,control,actual,sizeof(actual),&found);
    State();
    if(status!=0 || (three ? expected!=0 : found!=expected) || trace!=expected_trace || calls!=expected_calls || memcmp(old,actual,sizeof(old))!=0) {
        fprintf(stderr,"number mismatch value=%d plex=%d control=%x three=%u mask=%llx options=%x options2=%x mutation=%u cache=%u suffix=%u status=%d calls=%u/%u\n",value,plex,control,three,(unsigned long long)mask,options,options2,mutation,cached,suffix,status,calls,expected_calls); exit(1);
    }
    if(three) ++three_count; else ++two_count;
}
static uint32_t seed=0x37798432;
static unsigned Random(void) { seed^=seed<<13; seed^=seed>>17; seed^=seed<<5; return seed; }
int main(void) {
    for(unsigned i=0;i<256;++i) { types[i].type=i==6 ? phSTRESS : i%3==2 ? phVOWEL : 4; table_types[i]=&types[i]; }
    static const int flags[]={NUM_SWAP_TENS,NUM_AND_UNITS,NUM_SINGLE_VOWEL,NUM_VIGESIMAL,NUM_SINGLE_STRESS,NUM_SINGLE_STRESS_L,NUM_HUNDRED_AND,NUM_SINGLE_AND,NUM_1900,NUM_OMIT_1_HUNDRED,NUM_AND_HUNDRED,NUM_THOUSAND_AND,NUM_ZERO_HUNDRED,NUM_HUNDRED_AND_DIGIT,NUM_OMIT_1_THOUSAND};
    static const int flags2[]={NUM2_ORDINAL_NO_AND,NUM2_MULTIPLE_ORDINAL,NUM2_NO_TEEN_ORDINALS,NUM2_ORDINAL_AND_THOUSANDS,NUM2_ORDINAL_DROP_VOWEL,NUM2_ZERO_TENS,NUM2_OMIT_1_HUNDRED_ONLY,NUM2_MYRIADS,NUM2_SWAP_THOUSANDS,0x1e};
    for(unsigned feature=0;feature<26;++feature) for(int value=0;value<100;++value)
    for(unsigned ctl=0;ctl<64;++ctl) {
        options=feature<15 ? flags[feature] : 0; options2=feature>=15 && feature<25 ? flags2[feature-15] : 0;
        variants=(feature%8)*64; mutation=0; cached=ctl%6; suffix=ctl%4; malayalam=feature&1; initial_control=ctl&1;
        mask=ctl&4 ? UINT64_MAX : ctl&8 ? UINT64_C(0xaaaaaaaaaaaaaaaa) : 0;
        Compare(value,feature%5,(int)ctl|((ctl&16)?0x200:0),0,0);
    }
    static const int values[]={0,1,9,10,11,19,20,21,99,100,101,110,199,200,999,1000,1001,1100,1900,1984,1999,2000,9999,10000,19999,99999,1000000,INT_MAX};
    for(unsigned feature=0;feature<26;++feature) for(unsigned v=0;v<sizeof(values)/sizeof(values[0]);++v)
    for(unsigned ctl=0;ctl<128;++ctl) {
        options=feature<15 ? flags[feature] : 0; options2=feature>=15 && feature<25 ? flags2[feature-15] : 0;
        variants=(feature%8)*64; mutation=0; cached=ctl%6; suffix=ctl%4; malayalam=feature&1; initial_control=ctl&1;
        mask=ctl&4 ? UINT64_MAX : ctl&8 ? UINT64_C(0xaaaaaaaaaaaaaaaa) : 0;
        Compare(values[v],feature%5,(int)(ctl&3)|((ctl&16)?0x20:0)|((ctl&32)?0x100:0)|((ctl&64)?0x400:0),1,ctl&2);
    }
    for(unsigned trial=0;trial<400000;++trial) {
        options=0; options2=0;
        for(unsigned f=0;f<15;++f) if(Random()&1) options|=flags[f];
        for(unsigned f=0;f<10;++f) if(Random()&1) options2|=flags2[f];
        mask=((uint64_t)Random()<<32)|Random(); mutation=Random()%4; cached=Random()%6; suffix=Random()%4;
        variants=(Random()%8)*64; malayalam=Random()&1; initial_control=Random()%4;
        unsigned three=trial&1; int value=three ? (int)(Random()%100000) : (int)(Random()%100);
        Compare(value,Random()%8,Random()%0x800,three,Random()&1);
    }
    char output[200],saved[200]; memset(output,0x97,sizeof(output)); memcpy(saved,output,sizeof(output));
    int found=919; mutation=0; cached=0; suffix=0; options=0; options2=0; mask=UINT64_MAX; Reset();
    TEST_ASSERT(espeak_rs_lookup_num2(&table,21,0,2,output,1,&found)==-1 && found==919 && memcmp(output,saved,sizeof(output))==0);
    TEST_ASSERT(espeak_rs_lookup_num3(&table,100,0,0,0,output,1)==-1 && memcmp(output,saved,sizeof(output))==0);
    TEST_ASSERT(espeak_rs_lookup_num2(NULL,1,0,2,output,200,&found)==-1);
    TEST_ASSERT(espeak_rs_lookup_num2(&table,1,0,2,NULL,200,&found)==-1);
    TEST_ASSERT(espeak_rs_lookup_num2(&table,1,0,2,output,201,&found)==-1);
    TEST_ASSERT(espeak_rs_lookup_num2(&table,-1,0,2,output,200,&found)==-1);
    TEST_ASSERT(espeak_rs_lookup_num3(&table,1,-1,0,0,output,200)==-1);
    RustNumberDigits invalid=table; invalid.phoneme_type=NULL;
    TEST_ASSERT(espeak_rs_lookup_num2(&invalid,1,0,2,output,200,&found)==-1 && found==919 && memcmp(output,saved,sizeof(output))==0);
    TEST_ASSERT(espeak_rs_lookup_num3(NULL,1,0,0,0,output,200)==-1);
    TEST_ASSERT(espeak_rs_lookup_num3(&table,1,0,0,0,NULL,200)==-1);
    TEST_ASSERT(espeak_rs_lookup_num3(&table,1,0,0,0,output,0)==-1);
    cached=5; Reset(); memset(cache,31,sizeof(cache));
    TEST_ASSERT(espeak_rs_lookup_num2(&table,21,0,2,output,200,&found)==-1 && found==919 && memcmp(output,saved,sizeof(output))==0);
    cached=0; Reset(); memset(ordinal_text,31,sizeof(ordinal_text));
    TEST_ASSERT(espeak_rs_lookup_num2(&table,21,0,1,output,200,&found)==-1 && found==919 && memcmp(output,saved,sizeof(output))==0);
    fault=1; Reset();
    TEST_ASSERT(espeak_rs_lookup_num2(&table,21,0,2,output,200,&found)==-1 && found==919 && memcmp(output,saved,sizeof(output))==0);
    TEST_ASSERT(espeak_rs_lookup_num3(&table,100,0,0,0,output,200)==-1 && memcmp(output,saved,sizeof(output))==0);
    fault=2; options=NUM_SINGLE_VOWEL; Reset(); table_types[200]=NULL;
    TEST_ASSERT(espeak_rs_lookup_num2(&table,21,0,2,output,200,&found)==-1 && found==919 && memcmp(output,saved,sizeof(output))==0);
    table_types[200]=&types[200]; Reset();
    TEST_ASSERT(espeak_rs_lookup_num2(&table,21,0,2,output,200,&found)==0 && found==0);
    TEST_ASSERT((unsigned char)output[0]==6 && (unsigned char)output[1]==6 && (unsigned char)output[2]==200 && output[3]==0 && memcmp(output+4,saved+4,sizeof(output)-4)==0);
    printf("%u tens/units and %u hundreds/thousands trace/state/byte comparisons passed\n",two_count,three_count);
    return 0;
}
