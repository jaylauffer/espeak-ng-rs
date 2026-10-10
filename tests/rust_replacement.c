/* Complete replacement segmentation/expansion versus retained C policies.
 * SPDX-License-Identifier: GPL-3.0-or-later */
#include "config.h"
#include "test_assert.h"
#include <stdint.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <wchar.h>
#include <wctype.h>
#include <ctype.h>
#include <locale.h>
#include "translate.h"
#include "phoneme.h"
#include "common.h"
#include "readclause.h"
#include "rust_replacement.h"
typedef struct {
    Translator tr; WORD_TAB rows[3]; char source[400],text[161],output[200];
    int skip,remaining; bool any; unsigned seed,calls,null_text; uint64_t trace;
} Fixture;
static Fixture *active;
static unsigned calls,null_outputs,segmented,splits;
static int wide_output,fail_main,mutate_context;
static void Trace(unsigned n){active->trace=(active->trace^n)*UINT64_C(1099511628211);}
static void Bytes(const char *p){while(*p)Trace((unsigned char)*p++);Trace(0);}
#define dictionary_skipwords (active->skip)
#define any_stressed_words (active->any)
#define word_phonemes (active->output)
#define current_alphabet NULL
static int MockWord(Translator *tr,char *word,WORD_TAB *rows,int remaining,char *text,bool *any,ALPHABET *alphabet,char *output,size_t capacity)
{
    TEST_ASSERT(tr==&active->tr&&(rows==NULL||rows==active->rows+1)&&remaining==active->remaining&&any==&any_stressed_words&&alphabet==NULL&&output==word_phonemes&&capacity==200);
    Trace(1);Bytes(word);Trace(rows!=NULL?rows->flags:0);Trace(text!=NULL);
    TEST_ASSERT(tr->rule_text_base!=NULL);
    uintptr_t delta=(uintptr_t)word-(uintptr_t)tr->rule_text_base;
    TEST_ASSERT(delta<tr->rule_text_length);
    Trace((unsigned)delta);Trace((unsigned)tr->rule_text_length);Trace((unsigned char)word[-1]);
    active->calls++;if(text==NULL)active->null_text++;
    if(mutate_context){tr->rule_text_base=active->source;tr->rule_text_length=3;}
    word[0]=word[0]=='z'?'q':word[0];if(rows!=NULL)rows->flags^=FLAG_HYPHEN;tr->expect_noun++;
    if(active->seed&16)any_stressed_words=true;
    active->skip=0;
    if(active->calls>1 && active->seed%7==0) {
        char *end=strchr(word,' ');
        if(end && end[1]!=0 && end[1]!=' ')active->skip=1;
    }
    if(active->calls>1 && active->seed%11==0) {
        char *end=strchr(word,' ');if(end && end[1]!=' ' && end[1]!=0)end[1]='z';
    }
    unsigned mode=active->seed%6;
    if(text!=NULL && (active->calls==1||mode>=4)) {
        const char *replace=mode==0?"":mode==1?"Abc Def ":mode==2?"汉字 ":mode==3?(active->seed&1?"მომღიმარე თვალებით ":"Côte-État "):"A B ";
        strcpy(text,replace);strcpy(output,"i");return FLAG_TEXTMODE|FLAG_FOUND;
    }
    if(wide_output){memset(output,'p',199);output[199]=0;}
    else if(active->seed%13==0)output[0]=0;
    else {output[0]=(char)(30+word[0]%10);output[1]=(char)(40+active->calls%10);output[2]=0;}
    return FLAG_FOUND|(active->seed&8?FLAG_STRESS_END2:0);
}
#define TranslateWord3 MockWord
#define ShouldSplitIdeographs ReferenceSplit
#define SegmentReplacement ReferenceSegment
#include "retained_replacement_segmentation.inc"
#define TranslateWordWithBounds ReferenceExpand
#include "retained_replacement_expansion.inc"
#undef TranslateWordWithBounds
#undef ShouldSplitIdeographs
#undef SegmentReplacement
#define ShouldSplitIdeographs NativeSplit
#define TranslateWordWithBounds NativeExpand
#include "native_replacement_projections.inc"
#undef TranslateWordWithBounds
#undef ShouldSplitIdeographs
static void Init(Fixture *f,unsigned trial)
{
    memset(f,0,sizeof(*f));f->seed=trial*0xabcdu+29;f->trace=UINT64_C(1469598103934665603);
    strcpy(f->source,"   original next ");memset(f->text,0x55,161);f->text[0]=0;
    memset(f->output,0x57,200);f->output[0]=0;f->skip=f->seed%5;f->remaining=f->seed%4;
    f->tr.langopts.ideographs=f->seed&1;f->tr.langopts.alt_alphabet=f->seed&2?0x3100:0;
    f->tr.punct_within_word=f->seed&4?L"เ้":L"";
    f->rows[1].flags=f->seed&8?FLAG_FIRST_UPPER:FLAG_PHONEMES;
}
static void Activate(Fixture *f){active=f;f->tr.rule_text_base=f->source;f->tr.rule_text_length=400;}
static void Normalize(Fixture *f){f->tr.rule_text_base=NULL;}
static int FailMain(void *opaque,char *source,size_t initialized,size_t offset,char *text,unsigned *flags)
{
    int result=ReplacementMain(opaque,source,initialized,offset,text,flags);
    return fail_main?-1:result;
}
static void Guards(void)
{
    Fixture f;Init(&f,0);Activate(&f);unsigned flags=0x12345;
    RustReplacementHost h={&f.tr,f.source+3,f.text,f.rows+1,f.remaining};RustReplacement c=ReplacementCallbacks(&h);
    TEST_ASSERT(espeak_rs_replacement(NULL,1,0,&flags)==-1&&flags==0x12345);
    c.lower=NULL;TEST_ASSERT(espeak_rs_replacement(&c,1,0,&flags)==-1&&flags==0x12345);
    c=ReplacementCallbacks(&h);c.main=FailMain;fail_main=1;
    TEST_ASSERT(espeak_rs_replacement(&c,1,0,&flags)==-1&&flags==0x12345&&f.calls==1);
    TEST_ASSERT(f.text[0]!=0&&f.text[160]==0x55);fail_main=0;
    TEST_ASSERT(espeak_rs_replacement(&c,0,-1,&flags)==-1&&flags==0x12345&&f.calls==1);
    char owned[323]={0};memcpy(owned,"\0  a ",5);mutate_context=1;
    TEST_ASSERT(ReplacementMain(&h,owned,6,3,NULL,&flags)==0);
    TEST_ASSERT(f.tr.rule_text_base==f.source&&f.tr.rule_text_length==400);mutate_context=0;
    TEST_ASSERT(ReplacementMain(&h,owned,324,3,NULL,&flags)==-1);
    TEST_ASSERT(ReplacementMain(&h,owned,6,6,NULL,&flags)==-1);
    char destination[320];memset(destination,0x55,320);char invalid[161];memset(invalid,'a',161);
    TEST_ASSERT(espeak_rs_segment_replacement(&c,invalid,161,destination,320)==-1&&destination[0]==0x55);
    TEST_ASSERT(espeak_rs_segment_replacement(&c,"a",2,destination,0)==-1);
    Fixture old,native;Init(&old,2);native=old;
    Activate(&old);int expected=ReferenceExpand(&old.tr,old.source+3,NULL,old.remaining,NULL,0);
    Activate(&native);int actual=NativeExpand(&native.tr,native.source+3,NULL,native.remaining,NULL,0);
    Normalize(&old);Normalize(&native);TEST_ASSERT(expected==actual&&memcmp(&old,&native,sizeof(old))==0&&native.calls==1);
}
int main(void)
{
    setlocale(LC_CTYPE,"");
    const char *pieces[]={"Abc","-","汉","字","้","เ","É","Κ","Ж"," a ","1","🙂","_","\x80\x80" "A"};
    for(unsigned trial=0;trial<200000;trial++) {
        Fixture f;Init(&f,trial);Activate(&f);char text[161]={0},expected[400],actual[400];
        size_t used=0;unsigned value=f.seed;
        for(unsigned i=0;i<1+trial%12;i++){const char *piece=pieces[value%14];size_t n=strlen(piece);if(used+n>159)break;memcpy(text+used,piece,n);used+=n;value=value*1664525u+1013904223u;}
        memset(expected,0x55,400);memset(actual,0x55,400);int capacity=1+f.seed%320;
        ReferenceSegment(&f.tr,text,expected,capacity);
        RustReplacementHost h={.tr=&f.tr};RustReplacement c=ReplacementCallbacks(&h);
        TEST_ASSERT(espeak_rs_segment_replacement(&c,text,used+1,actual,capacity)==0);
        if(memcmp(expected,actual,400)){fprintf(stderr,"segment mismatch trial=%u text=%s cap=%d expected=%s actual=%s\n",trial,text,capacity,expected,actual);return 1;}
        unsigned current=trial%3==0?0x4e01:trial%3==1?0x3040:trial%3==2?0x3041:0;
        unsigned previous=trial%5==0?0x3400:'A';
        TEST_ASSERT(ReferenceSplit(&f.tr,current,previous)==NativeSplit(&f.tr,current,previous));
        segmented+=(unsigned)strlen(actual);splits+=NativeSplit(&f.tr,current,previous);
    }
    for(unsigned trial=0;trial<200000;trial++) {
        Fixture old,native;Init(&old,trial);native=old;int depth=trial%5;
        Activate(&old);int expected=ReferenceExpand(&old.tr,old.source+3,old.rows+1,old.remaining,old.text,depth);
        Activate(&native);int actual=NativeExpand(&native.tr,native.source+3,native.rows+1,native.remaining,native.text,depth);
        Normalize(&old);Normalize(&native);
        if(expected!=actual||memcmp(&old,&native,sizeof(old))){
            fprintf(stderr,"replacement mismatch trial=%u depth=%d return=%x/%x calls=%u/%u skip=%d/%d hash=%llu/%llu\n",trial,depth,actual,expected,native.calls,old.calls,native.skip,old.skip,(unsigned long long)native.trace,(unsigned long long)old.trace);
            const unsigned char *a=(const unsigned char *)&old,*b=(const unsigned char *)&native;
            for(size_t i=0;i<sizeof(old);i++)if(a[i]!=b[i]){fprintf(stderr,"byte %zu %u/%u\n",i,b[i],a[i]);break;}return 1;
        }
        calls+=native.calls;null_outputs+=native.null_text;
    }
    Guards();
    printf("200000 segmentation and 200000 expansion comparisons passed; %u segmented bytes, %u ideograph splits, %u word calls, %u depth-limit calls; ABI/failure/output-tail/context guards passed\n",segmented,splits,calls,null_outputs);
    return 0;
}
