/* Retained Roman/main/child chain: pronunciation/source/live-state parity.
 * SPDX-License-Identifier: GPL-3.0-or-later */
#include "config.h"
#include "test_assert.h"
#include "translate.h"
#include "common.h"
#include "phoneme.h"
#include "rust_number_roman.h"
#include <errno.h>
#include <limits.h>
#include <stdint.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <wctype.h>
#include <ctype.h>

static Translator local, global;
static Translator *fixture_translator = &global;
static WORD_TAB words[8];
static unsigned initial_words[8];
static unsigned char source[803], initial[803];
static char ordinal_text[12], alternate[12], initial_alternate[12];
static PHONEME_TAB types[256], *table_types[256];
static int fixture_control, missing, digit_count, sayas, skips;
static char *cache_pointer;
static unsigned calls, lists, classes, translations, mutation, fault;
static int options, options2, variants, language, previous, initial_missing, initial_sayas;
static int decimal_sep, thousands_sep, remaining;
static int roman_min, roman_max;
static const unsigned char *roman_suffix;
static uint64_t mask, trace;
static size_t length, initial_extent;
static char *reference_output;
static void Trace(unsigned value) { trace=(trace^value)*UINT64_C(1099511628211); }
static void State(void) {
    Trace((unsigned)local.langopts.numbers); Trace((unsigned)local.langopts.numbers2);
    Trace((unsigned)global.langopts.numbers2); Trace((unsigned)local.translator_name);
    Trace(local.prev_dict_flags[1]); Trace((unsigned)missing); Trace((unsigned)skips); Trace(local.prev_dict_flags[0]);
    for(unsigned i=0;i<12;++i) { Trace((unsigned char)ordinal_text[i]); Trace((unsigned char)alternate[i]); }
    for(unsigned i=0;i<8;++i) Trace(words[i].flags);
}
static int LookupName(void *context, const char *key, char *output) {
    TEST_ASSERT(context==&local && calls<512); State(); Trace(11);
    uint32_t hash=UINT32_C(2166136261);
    for(const unsigned char *p=(const unsigned char *)key;*p;++p) { hash=(hash^*p)*UINT32_C(16777619); Trace(*p); } Trace(0);
    unsigned present=(unsigned)((mask >> (calls++%64))&1);
    if(strlen(key)>2 && key[strlen(key)-1]=='n') present=present && (hash%7==0);
    if(fault==1) { memset(output,31,200); return 1; }
    unsigned size=present ? hash%3+1 : mutation&2 ? hash%3 : 0;
    static const char codes[]={6,50,51,10,15,21};
    for(unsigned i=0;i<size;++i) output[i]=codes[(hash+i)%sizeof(codes)]; output[size]=0;
    if(mutation&1) {
        local.langopts.numbers ^= NUM_SINGLE_STRESS|NUM_AND_UNITS|NUM_OMIT_1_THOUSAND;
        local.langopts.numbers2 ^= NUM2_FRACTION_FEMININE|NUM2_SWAP_THOUSANDS;
        global.langopts.numbers2 = (global.langopts.numbers2+64)&0x1c0;
        words[0].flags ^= FLAG_ORDINAL;
    }
    if(mutation&4) local.langopts.numbers ^= NUM_ROMAN_AFTER;
    Trace(present); return present ? 2 : 0;
}
static int ListName(Translator *tr, char **cursor, char *output, unsigned *flags, int suffix, WORD_TAB *wtab, int count) {
    TEST_ASSERT(tr==&local && wtab==words && count==remaining && suffix==FLAG_SUFX);
    ++lists; State(); Trace(12); for(const unsigned char *p=(const unsigned char *)*cursor;*p;++p) Trace(*p); Trace(0); Trace(flags[0]); Trace(flags[1]);
    unsigned present=(unsigned)((mask>>(lists%64))&1);
    if(fault==2) { memset(output,31,200); return 1; }
    if(present) { output[0]=6; output[1]=52; output[2]=0; flags[0] ^= 0x40; flags[1] |= 0x800; skips=3; }
    else output[0]=0;
    Trace(present); return present;
}
static int Month(Translator *tr, char *word, char *output, WORD_TAB *wtab) {
    TEST_ASSERT(tr==&local && output==NULL && wtab==NULL);
    ++translations; State(); Trace(13); for(const unsigned char *p=(const unsigned char *)word;*p;++p) Trace(*p); Trace(0);
    local.prev_dict_flags[0] ^= FLAG_ALT_TRANS|FLAG_ALT3_TRANS;
    words[0].flags ^= FLAG_COMMA_AFTER;
    return (int)(mask&1 ? FLAG_ALT_TRANS : FLAG_ALT3_TRANS);
}
static int Class(unsigned c, unsigned kind) {
    ++classes; Trace(14); Trace(c); Trace(kind);
    switch(kind) { case 0:return IsAlpha(c); case 1:return iswdigit(c)!=0; case 2:return iswalpha(c)!=0; default:return isspace((unsigned char)c)!=0; }
}
static int Alpha(unsigned c) { return Class(c,0); }
static int Digit(unsigned c) { return Class(c,1); }
static int WideAlpha(unsigned c) { return Class(c,2); }
static int SpaceByte(int c) { return Class((unsigned char)c,3); }
static void ObserveInitial(char *output,int size) {
    initial_extent=(size_t)(output-reference_output)+(size_t)size;
    TEST_ASSERT(initial_extent<200);
}
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
#define LookupDictList ListName
#define TranslateWord Month
#define CheckDotOrdinal ReferenceDot
#define CheckThousandsGroup ReferenceGroup
#define hu_number_e ReferenceHungarian
#define TranslateNumber_1 ReferenceMain
#define TranslateNumber ReferenceTranslate
#define option_sayas sayas
#define dictionary_skipwords skips
#define IsAlpha Alpha
#undef iswdigit
#define iswdigit Digit
#undef iswalpha
#define iswalpha WideAlpha
#undef isspace
#define isspace SpaceByte
#include "number_main_reference.inc"
#define RecognizeRoman ReferenceRecognize
#define TranslateRoman ReferencePronounce
#include "number_roman_reference.inc"

static unsigned char Byte(void *context, ptrdiff_t offset) {
    TEST_ASSERT(context==&local);
    return offset>=-3 && (offset<0 || (size_t)offset<length) ? source[offset+3] : 0;
}
static void Write(void *context,size_t offset,unsigned char value) { TEST_ASSERT(context==&local && offset<length); source[offset+3]=value; }
static int Value(void *context,unsigned field) {
    TEST_ASSERT(context==&local && field<10);
    switch(field) { case 0:return local.langopts.numbers; case 1:return global.langopts.numbers2; case 2:return local.langopts.numbers2; case 3:return local.translator_name; case 4:return missing; case 5:return sayas; case 6:return local.langopts.decimal_sep; case 7:return local.langopts.thousands_sep; case 8:return (int)local.prev_dict_flags[0]; default:return (char)0x80<0; }
}
static unsigned Word(void *context,size_t index) { TEST_ASSERT(context==&local && index<8); return words[index].flags; }
static int List(void *context,ptrdiff_t offset,char *out,unsigned *flags) {
    TEST_ASSERT(context==&local && offset>=-3 && (offset<0 || (size_t)offset<length));
    char *cursor=(char *)source+3+offset; return ListName(&local,&cursor,out,flags,FLAG_SUFX,words,remaining);
}
static const char *Text(void *context,unsigned kind) {
    TEST_ASSERT(context==&local && kind<3); return kind==0 ? ordinal_text : kind==1 ? alternate : local.langopts.ordinal_indicator;
}
static int StoreText(void *context,unsigned kind,const char *text,size_t size) {
    TEST_ASSERT(context==&local && kind<2 && size>0 && size<=12 && text[size-1]==0); memcpy(kind==0 ? ordinal_text : alternate,text,size); return 0;
}
static int Classify(void *context,unsigned c,unsigned kind) { TEST_ASSERT(context==&local && kind<4); return Class(c,kind); }
static unsigned Translate(void *context,size_t offset) { TEST_ASSERT(context==&local && offset<length); return (unsigned)Month(&local,(char *)source+3+offset,NULL,NULL); }
static void Missing(void *context,int value) { TEST_ASSERT(context==&local); missing=value; }
static void Skip(void *context,int value) { TEST_ASSERT(context==&local); skips=value; }
static int Type(void *context,unsigned char code) { TEST_ASSERT(context==&local); return table_types[code] ? table_types[code]->type : -1; }
static RustNumberFrontend frontend={&local,Byte,Write,Value,Word,LookupName,List,Text,StoreText,Classify,Translate,Missing,Skip,Type};

static int Range(void *context,unsigned maximum) { TEST_ASSERT(context==&local); return maximum ? local.langopts.max_roman : local.langopts.min_roman; }
static const unsigned char *Suffix(void *context) { TEST_ASSERT(context==&local); return local.langopts.roman_suffix; }
static void StoreWord(void *context,unsigned flags) { TEST_ASSERT(context==&local); words[0].flags=flags; }
static void Clear(void *context) { TEST_ASSERT(context==&local); local.prev_dict_flags[0]=local.prev_dict_flags[1]=0; }
static int SyntheticList(void *context,char *cursor,char *out,unsigned *flags) {
    TEST_ASSERT(context==&local); return ListName(&local,&cursor,out,flags,FLAG_SUFX,words,remaining);
}
static unsigned SyntheticTranslate(void *context,char *base,size_t initialized,size_t index) {
    TEST_ASSERT(context==&local && index>=3 && index<initialized && initialized<=160 && base[initialized-1]==0);
    return (unsigned)Month(&local,base+index,NULL,NULL);
}
static RustNumberRoman table={&frontend,Range,Suffix,StoreWord,Clear,SyntheticList,SyntheticTranslate};
static void Reset(void) {
    memcpy(source,initial,sizeof(source));
    local.langopts.numbers=options; local.langopts.numbers2=options2; global.langopts.numbers2=variants;
    local.langopts.decimal_sep=decimal_sep; local.langopts.thousands_sep=thousands_sep;
    local.translator_name=language; local.prev_dict_flags[0]=(unsigned)previous; local.prev_dict_flags[1]=919;
    local.langopts.min_roman=roman_min; local.langopts.max_roman=roman_max; local.langopts.roman_suffix=roman_suffix;
    local.langopts.ordinal_indicator=mask&2 ? (mask&4 ? "e" : "an-indicator-longer-than-any-admitted-number-suffix") : NULL;
    for(unsigned i=0;i<8;++i) words[i].flags=initial_words[i];
    memset(ordinal_text,0,sizeof(ordinal_text)); memcpy(alternate,initial_alternate,sizeof(alternate));
    missing=initial_missing; sayas=initial_sayas; skips=-9; calls=lists=classes=translations=0; initial_extent=0;
    trace=UINT64_C(14695981039346656037);
}
static uint32_t seed=0x69048231;
static unsigned Random(void) { seed^=seed<<13; seed^=seed>>17; seed^=seed<<5; return seed; }
static unsigned comparisons, reduced_capacities, excluded_joins;
static void Compare(void) {
    char expected[200],actual[200];
    Reset(); memset(expected,0x97,sizeof(expected)); reference_output=expected;
    ReferencePronounce(&local,(char *)source+3,expected,expected+200,words,remaining);
    size_t minimum=strlen(expected)+1;
    if(initial_extent>0 && minimum<initial_extent+4) minimum=initial_extent+4;
    if(minimum<4) minimum=4;
    TEST_ASSERT(minimum<=200);
    size_t capacity=minimum+Random()%(201-minimum);
    Reset(); memset(expected,0x97,sizeof(expected)); reference_output=expected;
    int result=ReferencePronounce(&local,(char *)source+3,expected,expected+capacity,words,remaining);
    size_t used=strlen(expected)+1;
    /* Only admit the original unbounded prefix/initial/final joins when their
     * observed extents fit the advertised capacity. No C decision is repaired. */
    if(used>capacity || (initial_extent>0 && initial_extent+4>capacity)) { ++excluded_joins; return; }
    State(); uint64_t expected_trace=trace; unsigned expected_calls=calls,expected_lists=lists,expected_classes=classes,expected_translations=translations;
    unsigned char expected_source[803]; memcpy(expected_source,source,sizeof(source));
    Reset(); memset(actual,0x97,sizeof(actual));
    int status=espeak_rs_translate_roman(&table,length,remaining,actual,capacity);
    State();
    if(status!=result || trace!=expected_trace || calls!=expected_calls || lists!=expected_lists || classes!=expected_classes || translations!=expected_translations
        || memcmp(expected_source,source,sizeof(source))!=0 || memcmp(expected,actual,used)!=0) {
        fprintf(stderr,"Roman mismatch input=%s options=%x options2=%x range=%d/%d remaining=%d mask=%llx mutation=%u cap=%zu status=%d/%d calls=%u/%u lists=%u/%u classes=%u/%u translations=%u/%u\n",
            initial+3,options,options2,roman_min,roman_max,remaining,(unsigned long long)mask,mutation,capacity,status,result,calls,expected_calls,lists,expected_lists,classes,expected_classes,translations,expected_translations);
        for(size_t i=0;i<used;++i) fprintf(stderr,"%02x/%02x ",(unsigned char)actual[i],(unsigned char)expected[i]); fprintf(stderr,"\ntrace %llx/%llx\n",(unsigned long long)trace,(unsigned long long)expected_trace); exit(1);
    }
    for(size_t i=used;i<sizeof(actual);++i) TEST_ASSERT((unsigned char)actual[i]==0x97);
    ++comparisons; if(capacity<200) ++reduced_capacities;
}
int main(void) {
    for(unsigned i=0;i<256;++i) { types[i].type=i==6 ? phSTRESS : i%3==2 ? phVOWEL : 4; table_types[i]=&types[i]; }
    static const char *inputs[]={"xx ","iv ","iii ","i ","i. next ","i . next ","x ","mm ","xl en ","xx el ","xx ezt ","xx att ","xx ent ","xx month ","xx 2 ","xx -en ","xx. ","xx. next ","xx . next ","xx \xc3\xa9 ","xx \x80\x81month ","xx \xe2 ","mcmxcix ","mmmdcccxxxiii ","invalid ","iiii ","xxxx ","ixix ","vl ","ixcm ","","xx\0"};
    static const unsigned char *suffixes[]={ (const unsigned char *)"", (const unsigned char *)"o",(const unsigned char *)"st",(const unsigned char *)"e",(const unsigned char *)"-e",(const unsigned char *)"\xc2\xaa",(const unsigned char *)"\xc2\xba" };
    static const int number_flags[]={NUM_ALLOW_SPACE,NUM_ORDINAL_DOT,NUM_NOPAUSE,NUM_AND_UNITS,NUM_SINGLE_STRESS,NUM_SINGLE_VOWEL,NUM_OMIT_1_HUNDRED,NUM_1900,NUM_OMIT_1_THOUSAND,NUM_ROMAN_ORDINAL,NUM_ROMAN_AFTER,NUM_ROMAN_CAPITALS};
    static const int second_flags[]={NUM2_MYRIADS,NUM2_SWAP_THOUSANDS,NUM2_FRACTION_FEMININE,NUM2_PERCENT_BEFORE,NUM2_MULTIPLE_ORDINAL,NUM2_ORDINAL_AND_THOUSANDS,NUM2_ZERO_TENS};
    for(unsigned trial=0;trial<240000;++trial) {
        memset(initial,0,sizeof(initial)); memcpy(initial,"   ",3); strcpy((char *)initial+3,inputs[trial%(sizeof(inputs)/sizeof(inputs[0]))]);
        if(trial%37==0) initial[1]='2';
        length=sizeof(initial)-3;
        options=1|(int)((trial%8)<<13); options2=0;
        for(unsigned f=0;f<sizeof(number_flags)/sizeof(number_flags[0]);++f) if(Random()&1) options|=number_flags[f];
        for(unsigned f=0;f<sizeof(second_flags)/sizeof(second_flags[0]);++f) if(Random()&1) options2|=second_flags[f];
        for(unsigned i=0;i<8;++i) initial_words[i]=(Random()&1 ? FLAG_HYPHEN_AFTER : 0)|(Random()&1 ? FLAG_ORDINAL : 0)|(Random()%8==0 ? FLAG_MULTIPLE_SPACES : 0)|(Random()&1 ? FLAG_HAS_DOT : 0)|(Random()&1 ? FLAG_COMMA_AFTER : 0)|(Random()%16==0 ? FLAG_INDIVIDUAL_DIGITS : 0)|(Random()%5==0 ? FLAG_NOSPACE : 0)|(Random()&1 ? FLAG_FIRST_UPPER : 0)|(Random()&1 ? FLAG_ALL_UPPER : 0);
        remaining=Random()%8; variants=(Random()%8)*64; language=Random()%3==0 ? L('h','u') : Random()%4==0 ? L('m','l') : L('e','n');
        previous=(Random()&1 ? FLAG_ALT_TRANS : 0)|(Random()&1 ? FLAG_ALT3_TRANS : 0);
        decimal_sep='.'; thousands_sep=Random()%3==0 ? ' ' : ',';
        initial_missing=Random()%4; initial_sayas=Random()%31==0 ? SAYAS_DIGITS1 : 0;
        mask=((uint64_t)Random()<<32)|Random(); mutation=Random()%8;
        roman_min=trial%7==0 ? 10 : 1; roman_max=trial%11==0 ? 99 : 10000;
        roman_suffix=suffixes[trial%7];
        memset(initial_alternate,0,sizeof(initial_alternate)); if(Random()&1) { initial_alternate[0]=51; initial_alternate[1]=5; }
        Compare();
    }
    Reset(); char output[200],saved[200]; memset(output,0x97,sizeof(output)); memcpy(saved,output,sizeof(output));
    TEST_ASSERT(espeak_rs_translate_roman(NULL,length,remaining,output,200)==-1);
    TEST_ASSERT(espeak_rs_translate_roman(&table,0,remaining,output,200)==-1);
    TEST_ASSERT(espeak_rs_translate_roman(&table,801,remaining,output,200)==-1);
    TEST_ASSERT(espeak_rs_translate_roman(&table,length,301,output,200)==-1);
    TEST_ASSERT(espeak_rs_translate_roman(&table,length,-1,output,200)==-1);
    TEST_ASSERT(espeak_rs_translate_roman(&table,length,remaining,output,0)==-1);
    TEST_ASSERT(espeak_rs_translate_roman(&table,length,remaining,output,201)==-1);
    TEST_ASSERT(espeak_rs_translate_roman(&table,length,remaining,NULL,200)==-1);
    RustNumberRoman invalid=table; invalid.list=NULL;
    TEST_ASSERT(espeak_rs_translate_roman(&invalid,length,remaining,output,200)==-1);
    invalid=table; invalid.frontend=NULL;
    TEST_ASSERT(espeak_rs_translate_roman(&invalid,length,remaining,output,200)==-1);
    RustNumberFrontend bad_front=frontend; bad_front.context=NULL; invalid=table; invalid.frontend=&bad_front;
    TEST_ASSERT(espeak_rs_translate_roman(&invalid,length,remaining,output,200)==-1);
    TEST_ASSERT(memcmp(output,saved,sizeof(output))==0 && calls==0);
    memcpy(initial,"   xx ",7); length=800; options=1; options2=0; remaining=1; initial_sayas=0; initial_words[0]=0;
    roman_min=1; roman_max=10000; roman_suffix=(const unsigned char *)"";
    fault=1; Reset();
    TEST_ASSERT(espeak_rs_translate_roman(&table,length,remaining,output,200)==-1 && memcmp(output,saved,sizeof(output))==0);
    fault=2; mask=UINT64_MAX; mutation=0; Reset();
    TEST_ASSERT(espeak_rs_translate_roman(&table,length,remaining,output,200)==-1 && memcmp(output,saved,sizeof(output))==0);
    fault=0; mask=0; mutation=0; Reset(); options=1|NUM_ROMAN_AFTER; local.langopts.numbers=options;
    TEST_ASSERT(espeak_rs_translate_roman(&table,length,remaining,output,1)==-1 && memcmp(output,saved,sizeof(output))==0);
    unsigned char long_suffix[160]; memset(long_suffix,'a',sizeof(long_suffix)); roman_suffix=long_suffix; Reset();
    TEST_ASSERT(espeak_rs_translate_roman(&table,length,remaining,output,200)==-1 && memcmp(output,saved,sizeof(output))==0);
    printf("%u Roman/main/child source/trace/state/pronunciation comparisons (%u reduced-capacity cases; %u undefined joins excluded) passed\n",comparisons,reduced_capacities,excluded_joins);
    return 0;
}
