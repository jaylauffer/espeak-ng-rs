/* Actual previous/native letter host adapters over identical primitive effects.
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

static Translator local, secondary, alternate_secondary;
static voice_t alternate_voice;
static char saved_source[16]="saved source";
static Translator *fixture_secondary = &secondary;
static voice_t current_voice, *fixture_voice = &current_voice;
static uint64_t mask, trace;
static unsigned calls, rules, selects, stresses, mutations, fault;
static int language, accents, selected;
static void Trace(unsigned n) { trace=(trace^n)*UINT64_C(1099511628211); }
static void State(void) {
    Trace((unsigned)local.translator_name); Trace((unsigned)local.langopts.accents);
    Trace((unsigned)current_voice.phoneme_tab_ix); Trace((unsigned)selected);
    Trace(fixture_voice==&alternate_voice);Trace(fixture_voice?(unsigned)fixture_voice->phoneme_tab_ix:UINT32_MAX);
    Trace((unsigned)local.rule_text_length);
}
static int IsName(const char *key) {
    static const char *names[]={"_lig","_smc","_tur","_rev","_crl","_acu","_brv","_hac","_ced","_cir","_dia","_ac2","_dot","_grv","_mcn","_ogo","_rng","_stk","_tld","_bar","_rfx","_hok"};
    for(unsigned i=0;i<22;++i) if(strcmp(key,names[i])==0) return 1;
    return 0;
}
static int LookupName(Translator *tr,const char *key,char *out) {
    TEST_ASSERT(tr==&local || tr==&secondary || tr==&alternate_secondary); TEST_ASSERT(calls<32);
    State(); Trace(11); Trace(tr==&secondary);Trace(tr==&alternate_secondary);
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
    if(tr!=&local && (mutations&16)) { fixture_voice=&alternate_voice;fixture_voice->phoneme_tab_ix+=5; }
    if(fault==1 || (fault==2 && named) || (fault==3 && tr!=&local)) memset(out,31,200);
    Trace((unsigned)flags); return flags;
}
static void Rules(Translator *tr,char *key,char *out,int capacity,unsigned *flags,int control,WORD_TAB *wtab) {
    TEST_ASSERT(tr==&local && flags==NULL && wtab==NULL);
    TEST_ASSERT(capacity==20 || capacity==160);
    ++rules; State(); Trace(12); Trace((unsigned)capacity); Trace((unsigned)control);
    Trace((unsigned char)key[-2]); Trace((unsigned char)key[-1]);
    for(const unsigned char *p=(const unsigned char *)key;*p;++p) Trace(*p); Trace(0);
    TEST_ASSERT(tr->rule_text_base==key-2 && tr->rule_text_length==10);
    size_t prefix=strlen(out);
    for(size_t i=0;i<prefix;++i) Trace((unsigned char)out[i]); Trace(0);
    if(mask&UINT64_C(0x800000000)) {
        if(mask&UINT64_C(0x200000000)) { out[prefix]=(char)0x82; out[prefix+1]=0; }
    } else { out[0]=mask&UINT64_C(0x200000000) ? (char)0x82 : 0; out[1]=0; }
    if(mutations&1) local.langopts.accents^=1;
    if(mutations&8) { tr->rule_text_base=saved_source+1;tr->rule_text_length=3; }
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
    ++selects; State(); Trace(14); selected=99; current_voice.phoneme_tab_ix+=3;
    fixture_secondary=(mutations&32)?NULL:(mutations&64)?&alternate_secondary:&secondary;
    if(fault==6) fixture_voice=NULL;
    return 0;
}
static int Restore(int index) { ++selects; State(); Trace(15); Trace((unsigned)index); selected=index; return 0; }
static int Space(unsigned code) { Trace(16); Trace(code); return iswspace(code)!=0; }
#define LookupBounded HookLookup
static int HookLookup(Translator *tr,const char *key,char *out,size_t capacity) { TEST_ASSERT(capacity==200);return LookupName(tr,key,out); }
#define TranslateRules Rules
#define SetWordStress Stress
#define SetTranslator3 Setup
#define SelectPhonemeTable Restore
#define translator3 fixture_secondary
#define voice fixture_voice
#define iswspace Space
#define LookupAccentedLetterBounded PreviousAccent
#define LookupLetterBounded PreviousLetter
#include "letter_host_reference.inc"
#undef LookupAccentedLetterBounded
#undef LookupLetterBounded
#define LetterSpace NativeSpace
#define LetterStress NativeStress
#define LookupAccentedLetterBounded NativeAccent
#define LookupLetterBounded NativeLetter
#include "native_letter_host_effects.inc"
#undef LookupAccentedLetterBounded
#undef LookupLetterBounded
#undef LetterSpace
#undef LetterStress
#undef LookupBounded
#undef TranslateRules
#undef SetWordStress
#undef SetTranslator3
#undef SelectPhonemeTable
#undef translator3
#undef voice
#undef iswspace

static uint32_t random_state=0x37625123;
static unsigned Random(void) { random_state^=random_state<<13;random_state^=random_state>>17;random_state^=random_state<<5;return random_state; }
static void Reset(void)
{
    memset(&local,0,sizeof(local));memset(&secondary,0,sizeof(secondary));memset(&alternate_secondary,0,sizeof(alternate_secondary));
    memset(&current_voice,0,sizeof(current_voice));memset(&alternate_voice,0,sizeof(alternate_voice));
    local.translator_name=language;local.langopts.accents=accents;
    local.rule_text_base=saved_source;local.rule_text_length=sizeof(saved_source);
    fixture_secondary=&secondary;fixture_voice=&current_voice;current_voice.phoneme_tab_ix=11;alternate_voice.phoneme_tab_ix=29;selected=11;
    calls=rules=selects=stresses=0;trace=UINT64_C(14695981039346656037);
}
static void Compare(unsigned trial,unsigned code,int next,int control,int accent,size_t capacity)
{
    char expected[200],actual[200];memset(expected,0x97,200);memset(actual,0x97,200);expected[0]=actual[0]=44;expected[1]=actual[1]=0;
    Reset();if(accent) PreviousAccent(&local,code,expected,capacity);else PreviousLetter(&local,code,next,expected,control,capacity);
    State();uint64_t wanted_trace=trace;unsigned wanted_calls=calls,wanted_rules=rules,wanted_selects=selects,wanted_stresses=stresses;
    Translator wanted=local;voice_t wanted_voice=current_voice,wanted_alternate=alternate_voice;
    int wanted_selection=selected,wanted_voice_slot=fixture_voice==&alternate_voice;
    TEST_ASSERT(local.rule_text_base==saved_source&&local.rule_text_length==sizeof(saved_source));
    Reset();if(accent) NativeAccent(&local,code,actual,capacity);else NativeLetter(&local,code,next,actual,control,capacity);
    State();
    if(memcmp(expected,actual,200)||wanted_trace!=trace||wanted_calls!=calls||wanted_rules!=rules||wanted_selects!=selects||wanted_stresses!=stresses||memcmp(&wanted,&local,sizeof(local))||memcmp(&wanted_voice,&current_voice,sizeof(current_voice))||memcmp(&wanted_alternate,&alternate_voice,sizeof(alternate_voice))||wanted_selection!=selected||wanted_voice_slot!=(fixture_voice==&alternate_voice)) {
        fprintf(stderr,"letter host trial=%u code=%x next=%d control=%d accent=%d cap=%zu mutation=%x fault=%u trace=%llx/%llx calls=%u/%u rules=%u/%u selects=%u/%u\n",trial,code,next,control,accent,capacity,mutations,fault,(unsigned long long)wanted_trace,(unsigned long long)trace,wanted_calls,calls,wanted_rules,rules,wanted_selects,selects);TEST_ASSERT(0);
    }
    TEST_ASSERT(local.rule_text_base==saved_source&&local.rule_text_length==sizeof(saved_source));
}
int main(void)
{
    static const unsigned glyphs[]={0,9,32,0xa0,0x3000,'a','A',0xe0,0xe6,0x100,0x17e,0x17f,0x180,0x250,0x266,0x2a8,0x2a9,0x451,0x1f600,0x1f9ec,0x10ffff,0xd800,0x110000,0xffffffff};
    static const int nexts[]={-1,32,31,0,'x'};
    for(unsigned trial=0;trial<200000;trial++) {
        mask=((uint64_t)Random()<<32)|Random();language=(Random()&1)?L('e','n'):L('f','r');accents=(int)Random();mutations=Random()&127;fault=0;
        unsigned code=trial%2?glyphs[trial%(sizeof(glyphs)/sizeof(*glyphs))]:Random();
        Compare(trial,code,nexts[trial%5],(int)Random(),(int)(trial%2),1+Random()%200);
    }
    for(fault=1;fault<=5;fault++) for(unsigned trial=0;trial<100;trial++) {
        mask=((uint64_t)Random()<<32)|Random();language=L('f','r');accents=(int)Random();mutations=Random()&127;
        Compare(trial,trial%2?0xe9:'x',nexts[trial%5],(int)Random(),(int)(trial%2),trial%3?200:1);
    }
    fault=0;mask=0;language=L('f','r');mutations=0;Reset();
    char out[200];memset(out,0x97,200);
    NativeLetter(NULL,'x',32,out,0,200);TEST_ASSERT((unsigned char)out[0]==0x97&&calls==0);
    NativeAccent(&local,0xe9,out,0);TEST_ASSERT((unsigned char)out[0]==0x97&&calls==0);
    NativeLetter(&local,'x',32,out,0,201);TEST_ASSERT((unsigned char)out[0]==0x97&&calls==0);
    /* Native-only missing-voice guard: previous adapter dereferences NULL, so
     * execute no undefined C comparison. Setup/lookup effects remain live. */
    fault=6;mask=4;Reset();RustLetterGlobals globals=LetterGlobals();
    TEST_ASSERT(espeak_rs_lookup_letter_host(&local,&globals,&LetterEffects,'x',-1,0,0,out,200)==-1);
    TEST_ASSERT((unsigned char)out[0]==0x97&&calls==3&&selects==1&&fixture_voice==NULL);
    TEST_ASSERT(local.rule_text_base==saved_source&&local.rule_text_length==sizeof(saved_source));
    printf("200000 actual previous/native letter host episodes, 500 malformed episodes, early guards and native-only missing voice passed\n");
    return 0;
}
