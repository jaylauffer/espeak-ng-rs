/* Native dotted/unpronounceable policies against unchanged C references.
 * SPDX-License-Identifier: GPL-3.0-or-later */
#include "config.h"
#include "test_assert.h"
#include <ctype.h>
#include <limits.h>
#include <stdint.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <wctype.h>
#include <espeak-ng/speak_lib.h>
#include "translate.h"
#include "translateword.h"
#include "dictionary.h"
#include "numbers.h"
#include "phoneme.h"
#include "common.h"
#include "readclause.h"
#include "rust_word_admission.h"
typedef struct {
    Translator tr; char source[520], output[200];
    uint64_t trace; unsigned seed, vowels, probes;
    int skip;
} Fixture;
static Fixture *active;
static void Trace(unsigned value) {active->trace=(active->trace^value)*UINT64_C(1099511628211);}
static void Bytes(const char *text) {for(const unsigned char *p=(const unsigned char *)text;*p;p++)Trace(*p);Trace(0);}
static int MockVowel(Translator *tr,int code)
{
    Trace(1);Trace((unsigned)code);active->vowels++;
    if(active->seed%13==0) tr->langopts.param[LOPT_UNPRONOUNCABLE]=2;
    if(active->seed%17==0) tr->langopts.max_initial_consonants=0;
    return strchr("aeiou",code)!=NULL&&code!=0;
}
static int MockRules(Translator *tr,char *source,char *out,int capacity,char *ending,int word_flags,unsigned *flags)
{
    TEST_ASSERT(capacity==200&&ending==NULL&&flags==NULL&&word_flags==(int)FLAG_UNPRON_TEST);
    TEST_ASSERT(source[-1]==' ');Trace(2);Bytes(source);active->probes++;
    strcpy(out,"discarded");strcpy(active->output,"probe-owner");
    if(active->seed&1) source[0]='X';source[-1]='?';
    tr->expect_verb=(int)(active->seed%4);
    return active->seed%3==0?0:active->seed%3==1?SUFX_UNPRON:7;
}
static int ReferenceUnpronounceable2(Translator *,char *);
#define IsVowel MockVowel
#define TranslateRules MockRules
#define CheckDottedAbbrev ReferenceDotted
#define Unpronouncable ReferenceUnpronounceable
#define Unpronouncable2 ReferenceUnpronounceable2
#include "retained_dotted_abbreviation.inc"
#include "retained_unpronounceable_admission.inc"
#undef CheckDottedAbbrev
#undef Unpronouncable
#undef Unpronouncable2
#define CheckDottedAbbrev NativeDotted
#include "native_word_source_projections.inc"
#include "native_word_admission_projection.inc"
#undef CheckDottedAbbrev
static void Activate(Fixture *f)
{
    active=f;translator=&f->tr;translator2=translator3=NULL;
    f->tr.rule_text_base=f->source;f->tr.rule_text_length=sizeof(f->source);
    dictionary_skipwords=f->skip;
}
static void Init(Fixture *f,unsigned trial)
{
    memset(f,0,sizeof(*f));f->seed=trial*0xabcdefu+17;f->trace=UINT64_C(1469598103934665603);
    memcpy(f->source,"  !",3);memset(f->output,0x5a,200);strcpy(f->output,"owner");f->skip=9;
}
static void Compare(Fixture *old,Fixture *native,unsigned trial,int actual,int expected)
{
    old->tr.rule_text_base=native->tr.rule_text_base=NULL;
    if(actual!=expected||old->trace!=native->trace||old->skip!=native->skip||memcmp(old->source,native->source,520)||memcmp(old->output,native->output,200)||memcmp(&old->tr,&native->tr,sizeof(old->tr))) {
        fprintf(stderr,"admission mismatch trial=%u result=%d/%d trace=%llu/%llu vowels=%u/%u probes=%u/%u\n",trial,actual,expected,(unsigned long long)native->trace,(unsigned long long)old->trace,native->vowels,old->vowels,native->probes,old->probes);exit(1);
    }
}
int main(void)
{
    unsigned long vowel_calls=0,probe_calls=0,compactions=0,refusals=0;
    static const unsigned letters[]={'a','b','h','x',0xe9,0x416,0x915,0x4e2d,0x10450};
    for(unsigned trial=0;trial<200000;trial++) {
        Fixture old,native;Init(&old,trial);char *p=old.source+3;
        unsigned count=trial%101;
        for(unsigned i=0;i<count;i++) {
            int n=utf8_out((int)letters[(trial+i)%9],p);p+=n;
            memcpy(p," . ",3);p+=3;
        }
        if(trial%4==0) {memcpy(p,"a ",2);p+=2;}
        else if(trial%4==1&&count>0) {p[-1]='\'';*p++='s';*p++=' ';}
        else if(trial%4==2) {*p++='1';*p++=' ';}
        *p=0;native=old;
        Activate(&old);int expected=ReferenceDotted(old.source+3);old.skip=dictionary_skipwords;
        Activate(&native);int actual=NativeDotted(native.source+3);native.skip=dictionary_skipwords;
        Compare(&old,&native,trial,actual,expected);
        compactions+=expected>1;refusals+=count>80&&expected==0;
    }
    static const char *words[]={"", " ", "'cat ","bcdf ","abc ","bca ","bcdfa ","b'ca ","tv' ","l'ami ","a1 ","b1 ","?abc ","hmm ","xyza ","école ","ЖЩ ","कथा ","你们 ","abc"};
    static const int modes[]={0,1,2,3,'h','b','x'};
    static const int offsets[]={0,0x400,0x900,0x2800};
    for(unsigned trial=0;trial<200000;trial++) {
        Fixture old,native;Init(&old,trial);strcpy(old.source+3,words[trial%20]);
        old.tr.letter_bits_offset=offsets[(trial/20)%4];old.tr.langopts.param[LOPT_UNPRONOUNCABLE]=modes[(trial/80)%7];
        old.tr.langopts.max_initial_consonants=(int)((trial/560)%7)-1;
        native=old;int position=(int)(trial%4)-1;
        Activate(&old);int expected=ReferenceUnpronounceable(&old.tr,old.source+3,position);
        Activate(&native);RustWordHost host={.tr=&native.tr};TEST_ASSERT(WordWindow(&host,native.source+3)==0);
        RustWordAdmission table=AdmissionCallbacks(&host);int actual=123;int status=espeak_rs_word_unpronounceable(&table,position,&actual);
        TEST_ASSERT(status==0);Compare(&old,&native,trial,actual,expected);vowel_calls+=old.vowels;probe_calls+=old.probes;
    }
    RustWordAdmission empty={0};int result=123;
    TEST_ASSERT(espeak_rs_word_dotted(NULL,&result)==-1&&result==123);
    TEST_ASSERT(espeak_rs_word_unpronounceable(&empty,0,&result)==-1&&result==123);
    Fixture f;Init(&f,17);Activate(&f);RustWordHost host={.tr=&f.tr};
    RustWordAdmission table=AdmissionCallbacks(&host);
    TEST_ASSERT(espeak_rs_word_dotted(&table,&result)==-1&&result==123);
    printf("200000 dotted and 200000 unpronounceable comparisons passed; %lu compactions, %lu capacity refusals, %lu vowel calls, %lu rule probes; ABI guards passed\n",compactions,refusals,vowel_calls,probe_calls);
    return 0;
}
