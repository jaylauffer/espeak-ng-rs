/* Original C letter/diacritic tables and controllers against native Rust.
 * SPDX-License-Identifier: GPL-3.0-or-later */
#include "config.h"
#include "test_assert.h"
#include "translate.h"
#include "phoneme.h"
#include "common.h"
#include "voice.h"
#include "rust_letter_lookup.h"
#include <stdint.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <wctype.h>

static Translator local, secondary;
static Translator *fixture_secondary = &secondary;
static voice_t current_voice, *fixture_voice = &current_voice;
static uint64_t mask, trace;
static unsigned calls, rules, selects, stresses, mutations, fault;
static int language, accents, selected;
static void Trace(unsigned n) { trace=(trace^n)*UINT64_C(1099511628211); }
static void State(void) {
    Trace((unsigned)local.translator_name); Trace((unsigned)local.langopts.accents);
    Trace((unsigned)current_voice.phoneme_tab_ix); Trace((unsigned)selected);
}
static int IsName(const char *key) {
    static const char *names[]={"_lig","_smc","_tur","_rev","_crl","_acu","_brv","_hac","_ced","_cir","_dia","_ac2","_dot","_grv","_mcn","_ogo","_rng","_stk","_tld","_bar","_rfx","_hok"};
    for(unsigned i=0;i<22;++i) if(strcmp(key,names[i])==0) return 1;
    return 0;
}
static int LookupName(Translator *tr,const char *key,char *out) {
    TEST_ASSERT(tr==&local || tr==&secondary); TEST_ASSERT(calls<32);
    State(); Trace(11); Trace(tr==&secondary);
    uint32_t hash=UINT32_C(2166136261);
    int named=IsName(key);
    if(!named) Trace((unsigned char)key[-1]);
    for(const unsigned char *p=(const unsigned char *)key;*p;++p) { hash=(hash^*p)*UINT32_C(16777619); Trace(*p); }
    Trace(0);
    unsigned present=(unsigned)(mask>>(calls++%64))&1;
    unsigned length=present ? hash%3+1 : mutations&2 ? hash%3 : 0;
    static const unsigned char codes[]={50,51,0x81,21,52,0xff};
    for(unsigned i=0;i<length;++i) out[i]=(char)codes[(hash+i)%sizeof(codes)];
    out[length]=0;
    if(mutations&1) { local.langopts.accents^=1; local.translator_name=local.translator_name==L('e','n') ? L('f','r') : L('e','n'); current_voice.phoneme_tab_ix+=1; }
    if((mutations&4) && !named && key[0]) ((char *)key)[0]='x';
    int flags=present ? 2 | (mask&UINT64_C(0x100000000) ? FLAG_ACCENT_BEFORE : 0) : 0;
    if(fault==1 || (fault==2 && named) || (fault==3 && tr==&secondary)) memset(out,31,200);
    Trace((unsigned)flags); return flags;
}
static void Rules(Translator *tr,char *key,char *out,int capacity,unsigned *flags,int control,WORD_TAB *wtab) {
    TEST_ASSERT(tr==&local && flags==NULL && wtab==NULL);
    TEST_ASSERT(capacity==20 || capacity==160);
    ++rules; State(); Trace(12); Trace((unsigned)capacity); Trace((unsigned)control);
    Trace((unsigned char)key[-2]); Trace((unsigned char)key[-1]);
    for(const unsigned char *p=(const unsigned char *)key;*p;++p) Trace(*p); Trace(0);
    out[0]=mask&UINT64_C(0x200000000) ? (char)0x82 : 0; out[1]=0;
    if(mutations&1) local.langopts.accents^=1;
    if(fault==4) memset(out,31,200);
}
static void Stress(Translator *tr,char *out,unsigned *flags,int last,int control) {
    TEST_ASSERT(tr==&local && flags[0]==0 && flags[1]==0 && last==-1);
    ++stresses; State(); Trace(13); Trace((unsigned)control);
    size_t length=strlen(out); for(size_t i=0;i<length;++i) Trace((unsigned char)out[i]); Trace(0);
    if(mask&UINT64_C(0x400000000)) { out[length]=(char)(control ? 6 : 4); out[length+1]=0; }
    if(mutations&1) current_voice.phoneme_tab_ix+=7;
    if(fault==5) memset(out,31,200);
}
static int Setup(const char *name) {
    TEST_ASSERT(strcmp(name,ESPEAKNG_DEFAULT_VOICE)==0);
    ++selects; State(); Trace(14); selected=99; current_voice.phoneme_tab_ix+=3; return 0;
}
static int Restore(int index) { ++selects; State(); Trace(15); Trace((unsigned)index); selected=index; return 0; }
static int Space(unsigned code) { Trace(16); Trace(code); return iswspace(code)!=0; }
#define Lookup LookupName
#define TranslateRules Rules
#define SetWordStress Stress
#define SetTranslator3 Setup
#define SelectPhonemeTable Restore
#define translator3 fixture_secondary
#define voice fixture_voice
#define LookupLetter2 ReferenceBasic
#define LookupAccentedLetter ReferenceAccent
#define LookupLetter ReferenceLetter
#undef iswspace
#define iswspace Space
#include "letter_lookup_reference.inc"

static int LookupCallback(void *context,char *source,size_t start,unsigned use_secondary,char *out) {
    TEST_ASSERT(context==&local && start>=1 && start<=2 && use_secondary<=1);
    TEST_ASSERT(memchr(source+start,0,10-start)!=NULL);
    return LookupName(use_secondary ? &secondary : &local,source+start,out);
}
static int Named(void *context,const char *key,char *out) { TEST_ASSERT(context==&local); return LookupName(&local,key,out); }
static int Value(void *context,unsigned field) { TEST_ASSERT(context==&local && field<=1); return field ? local.langopts.accents : local.translator_name; }
static int WideSpace(void *context,unsigned code) { TEST_ASSERT(context==&local); return Space(code); }
static void RulesCallback(void *context,char *source,size_t start,size_t capacity,unsigned flags,char *out) {
    TEST_ASSERT(context==&local && start==2); TEST_ASSERT(memchr(source+start,0,10-start)!=NULL);
    Rules(&local,source+start,out,(int)capacity,NULL,(int)flags,NULL);
}
static void Select(void *context,unsigned restore) { TEST_ASSERT(context==&local); if(restore) Restore(current_voice.phoneme_tab_ix); else Setup(ESPEAKNG_DEFAULT_VOICE); }
static void StressCallback(void *context,char *out,unsigned *flags,int control) { TEST_ASSERT(context==&local); Stress(&local,out,flags,-1,control); }
static RustLetterLookup table={&local,LookupCallback,Named,Value,WideSpace,RulesCallback,Select,StressCallback};
static void Reset(void) {
    local.translator_name=language; local.langopts.accents=accents;
    current_voice.phoneme_tab_ix=11; selected=11;
    calls=rules=selects=stresses=0; trace=UINT64_C(14695981039346656037);
}
static uint32_t seed=0x37625123;
static unsigned Random(void) { seed^=seed<<13; seed^=seed>>17; seed^=seed<<5; return seed; }
static unsigned comparisons, reduced;
static void Compare(unsigned code,int next,int control,int accent) {
    char expected[200],actual[200];
    memset(expected,0x97,sizeof(expected)); expected[0]=44; expected[1]=0;
    Reset();
    if(accent) ReferenceAccent(&local,code,expected); else ReferenceLetter(&local,code,next,expected,control);
    State(); uint64_t expected_trace=trace;
    size_t used=strlen(expected)+1;
    /* Mock callbacks emit <=3 bytes and stress can append one; original
     * unbounded joins all fit 16 bytes. Only defined C extents enter parity. */
    TEST_ASSERT(used<=16);
    size_t minimum=used<16 ? 16 : used;
    size_t capacity=minimum+Random()%(201-minimum);
    memset(actual,0x97,sizeof(actual)); actual[0]=44; actual[1]=0;
    Reset(); int result=espeak_rs_lookup_letter(&table,code,next,control,accent,actual,capacity);
    State();
    if(result<0 || trace!=expected_trace || memcmp(actual,expected,used)!=0) {
        fprintf(stderr,"letter U+%x next=%d control=%d accent=%d mask=%llx mutations=%u capacity=%zu result=%d trace=%llx/%llx\n",code,next,control,accent,(unsigned long long)mask,mutations,capacity,result,(unsigned long long)trace,(unsigned long long)expected_trace);
        TEST_ASSERT(0);
    }
    TEST_ASSERT(result==(accent && expected[0]==44 ? 0 : 1));
    for(size_t i=used;i<sizeof(actual);++i) TEST_ASSERT((unsigned char)actual[i]==(i==1 ? 0 : 0x97));
    ++comparisons; if(capacity<200) ++reduced;
}
static void Guards(void) {
    char out[200],before[200]; memset(out,0x97,sizeof(out)); memcpy(before,out,sizeof(out));
    Reset(); TEST_ASSERT(espeak_rs_lookup_letter(NULL,'a',32,0,0,out,200)==-1 && calls==0);
    TEST_ASSERT(espeak_rs_lookup_letter(&table,'a',32,0,0,NULL,200)==-1 && calls==0);
    TEST_ASSERT(espeak_rs_lookup_letter(&table,'a',32,0,2,out,200)==-1 && calls==0);
    TEST_ASSERT(espeak_rs_lookup_letter(&table,'a',32,0,0,out,0)==-1 && calls==0);
    TEST_ASSERT(espeak_rs_lookup_letter(&table,'a',32,0,0,out,201)==-1 && calls==0);
    RustLetterLookup invalid=table; invalid.stress=NULL;
    TEST_ASSERT(espeak_rs_lookup_letter(&invalid,'a',32,0,0,out,200)==-1 && calls==0);
    invalid=table; invalid.context=NULL;
    TEST_ASSERT(espeak_rs_lookup_letter(&invalid,'a',32,0,0,out,200)==-1 && calls==0);
    mask=UINT64_MAX; mutations=0;
    for(fault=1;fault<=5;++fault) {
        int mode=fault==2;
        if(fault==3) { mask=4; language=L('f','r'); }
        else if(fault==4) mask=0;
        else mask=UINT64_MAX;
        Reset();
        TEST_ASSERT(espeak_rs_lookup_letter(&table,mode ? 0xe0 : 'a',fault==3 ? -1 : 32,0,mode,out,200)==-1);
        TEST_ASSERT(memcmp(out,before,sizeof(out))==0);
        if(fault==3) TEST_ASSERT(selects==2 && selected==current_voice.phoneme_tab_ix);
    }
    fault=0; mask=UINT64_MAX;
    Reset(); TEST_ASSERT(espeak_rs_lookup_letter(&table,0xe0,32,0,1,out,1)==-1);
    TEST_ASSERT(memcmp(out,before,sizeof(out))==0);
    Reset(); TEST_ASSERT(espeak_rs_lookup_letter(&table,'a',32,0,0,out,1)==-1);
    TEST_ASSERT(memcmp(out,before,sizeof(out))==0);
    Reset(); TEST_ASSERT(espeak_rs_lookup_letter(&table,0x17f,32,0,1,out,1)==0 && calls==0);
    TEST_ASSERT(memcmp(out,before,sizeof(out))==0);
}
int main(void) {
    language=L('f','r'); accents=0; mutations=0; fault=0; mask=UINT64_MAX;
    for(unsigned code=0;code<=0x10ffff;++code) Compare(code,32,0,1);
    static const unsigned boundaries[]={0,1,9,10,13,31,32,33,65,0xa0,0x1680,0x2000,0x2028,0x2029,0x202f,0x205f,0x3000,0xd7ff,0xd800,0xdfff,0xe000,0x10ffff,0x110000,0xffffffff};
    for(unsigned i=0;i<120000;++i) {
        mask=(uint64_t)Random()<<32 | Random(); mutations=Random()&7;
        accents=(int)(Random()&3); language=Random()&1 ? L('e','n') : L('f','r');
        unsigned code=i%4==0 ? boundaries[Random()%(sizeof(boundaries)/sizeof(boundaries[0]))] : i%4==1 ? 0xe0+Random()%160 : i%4==2 ? 0x250+Random()%89 : Random()%0x110000;
        int next=i%3==0 ? -1 : i%3==1 ? 32 : (int)Random();
        Compare(code,next,(int)Random(),i%5==0);
    }
    Guards();
    printf("%u letter/diacritic comparisons passed (%u reduced capacities); malformed callbacks and output guards passed\n",comparisons,reduced);
    return 0;
}
