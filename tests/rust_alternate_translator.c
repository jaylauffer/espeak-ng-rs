/* Actual alternate-resource projections versus unchanged lifecycle bodies.
 * SPDX-License-Identifier: GPL-3.0-or-later */
#include "config.h"
#include "test_assert.h"
#include <stdint.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include "translate.h"
#include "voice.h"
#include "rust_alternate_translator.h"
typedef struct {
    Translator objects[2];Translator *slots[2];char names[2][20];int alive[2];
    voice_t main;unsigned seed,lookups,creates,loads,deletes,empty_deletes,selections;
    int selected;uint64_t trace;
} Fixture;
static Fixture *active;static int fail_allocation;
static void Event(unsigned v){active->trace=(active->trace^v)*UINT64_C(1099511628211);}
static int Object(Translator *p){for(int i=0;i<2;i++)if(p==&active->objects[i])return i;return -1;}
static void Observe(void)
{
    for(int s=0;s<2;s++){
        Event((unsigned)(Object(active->slots[s])+1));
        for(int i=0;i<20;i++)Event((unsigned char)active->names[s][i]);
    }
}
static int MockLookup(const char *name)
{
    active->lookups++;Event(10);Observe();
    for(size_t i=0;name[i];i++)Event((unsigned char)name[i]);
    return active->seed%7==0?-3:(int)(active->seed%37);
}
static Translator *MockCreate(const char *name)
{
    active->creates++;Event(11);Observe();
    if(fail_allocation)return NULL;
    for(int i=0;i<2;i++)if(!active->alive[i]){
        Translator *p=&active->objects[i];memset(p,0,sizeof(*p));active->alive[i]=1;
        strcpy(p->dictionary_name,name);p->phonemes_repeat[0]=65;Event((unsigned)i);return p;
    }
    TEST_ASSERT(0);return NULL;
}
static void MockDelete(Translator *p)
{
    active->deletes++;Event(12);Observe();int i=Object(p);Event((unsigned)(i+1));
    if(!p){active->empty_deletes++;return;}
    TEST_ASSERT(i>=0&&active->alive[i]);active->alive[i]=0;
    p->phoneme_tab_ix=-88;
}
static int MockLoad(Translator *p,const char *name,int flags)
{
    active->loads++;Event(13);Observe();TEST_ASSERT(flags==0&&name==p->dictionary_name);
    int i=Object(p);TEST_ASSERT(i>=0&&active->alive[i]);Event((unsigned)i);
    active->main.phoneme_tab_ix=(int)(active->seed%127);
    for(int s=0;s<2;s++)if(active->slots[s]==p&&active->seed%17==0)active->names[s][1]='!';
    return active->seed%5==0?1:0;
}
static int MockSelect(int table)
{
    active->selections++;Event(14);Observe();active->selected=table;Event((unsigned)table);return table;
}
#define translator2 (active->slots[0])
#define translator3 (active->slots[1])
#define translator2_language (active->names[0])
#define translator3_language (active->names[1])
#define voice (&active->main)
#define DeleteTranslator MockDelete
#define SelectTranslator MockCreate
#define LoadDictionary MockLoad
#define SelectPhonemeTable MockSelect
#define SelectPhonemeTableName MockLookup
#define SetTranslator2 Reference2
#define SetTranslator3 Reference3
#define FreeAlternateTranslators ReferenceFree
#include "retained_alternate_release.inc"
#include "retained_alternate_selection.inc"
#undef SetTranslator2
#undef SetTranslator3
#undef FreeAlternateTranslators
#define SetTranslator2 Native2
#define SetTranslator3 Native3
#define FreeAlternateTranslators NativeFree
#include "native_alternate_projections.inc"
typedef struct {Fixture state;int slots[2];} Snapshot;
static void Capture(Snapshot *out)
{
    memset(out,0,sizeof(*out));out->state=*active;
    for(int i=0;i<2;i++){out->slots[i]=Object(active->slots[i]);out->state.slots[i]=NULL;}
}
static void Start(Fixture *f,unsigned seed)
{
    memset(f,0,sizeof(*f));f->seed=seed;f->trace=UINT64_C(14695981039346656037);f->main.phoneme_tab_ix=19;
    for(int i=0;i<2;i++){
        memset(f->names[i],'z',20);memcpy(f->names[i],i?"ru\0":"en\0",3);
        if((seed>>(i+3))&1){f->slots[i]=&f->objects[i];f->alive[i]=1;f->objects[i].phonemes_repeat[0]=41;
            f->objects[i].phoneme_tab_ix=7;}
    }
}
static void Guards(Fixture *f)
{
    Start(f,1);active=f;unsigned context=0;RustAlternateTranslator h=AlternateCallbacks(&context);
    int out=99;Snapshot before,after;Capture(&before);
    TEST_ASSERT(espeak_rs_alternate_translator(NULL,0,"en",2,&out)==-1&&out==99);
    TEST_ASSERT(espeak_rs_alternate_translator(&h,0,"en",2,NULL)==-1);
    TEST_ASSERT(espeak_rs_alternate_translator(&h,0,NULL,2,&out)==-1&&out==99);
    TEST_ASSERT(espeak_rs_alternate_translator(&h,0,"en",SIZE_MAX,&out)==-1&&out==99);
    TEST_ASSERT(espeak_rs_alternate_translator(&h,0,"e\0n",3,&out)==-1&&out==99);
    TEST_ASSERT(espeak_rs_alternate_translator(&h,9,NULL,0,&out)==-1&&out==99);
    h.create=NULL;TEST_ASSERT(espeak_rs_alternate_translator(&h,2,NULL,0,&out)==-1&&out==99);
    TEST_ASSERT(Native2(NULL)==-1);TEST_ASSERT(Native3("01234567890123456789")==-1);
    Capture(&after);TEST_ASSERT(memcmp(&before,&after,sizeof(before))==0);
    TEST_ASSERT(AlternateCached(&context,2,0)==-1);TEST_ASSERT(AlternateCached(&context,0,20)==-1);
    TEST_ASSERT(AlternateWrite(&context,1,20,0)==-1);TEST_ASSERT(AlternateEffect(&context,2,0,0)==-1);
    TEST_ASSERT(AlternateEffect(&context,0,9,0)==-1);TEST_ASSERT(AlternateCreate(&context,2,"en")==-1);
    TEST_ASSERT(AlternateLoad(&context,2)==-1);
    Start(f,9);active=f;h=AlternateCallbacks(&context);fail_allocation=1;
    TEST_ASSERT(espeak_rs_alternate_translator(&h,0,"fr",2,&out)==-1&&out==99);
    TEST_ASSERT(f->slots[0]==NULL&&f->deletes==1&&f->creates==1&&f->loads==0);
    TEST_ASSERT(memcmp(f->names[0],"en\0",3)==0);fail_allocation=0;
}
int main(void)
{
    Fixture *reference=malloc(sizeof(*reference)),*native=malloc(sizeof(*native));
    Snapshot *a=malloc(sizeof(*a)),*b=malloc(sizeof(*b));TEST_ASSERT(reference&&native&&a&&b);
    static const char *names[]={"en","ru","fr","de","","cmn","0123456789012345678","xx"};
    uint64_t lookups=0,creates=0,loads=0,deletes=0,empty=0,selections=0;
    unsigned state=UINT32_C(0xa7f35e19);
    for(unsigned episode=0;episode<200000;episode++){
        state=state*1664525u+1013904223u;Start(reference,state);Start(native,state);
        for(unsigned step=0;step<12;step++){
            state=state*1664525u+1013904223u;unsigned op=(state>>13)%3;const char *name=names[(state>>21)%8];
            active=reference;active->seed=state;int expected=0;
            if(op==0)expected=Reference2(name);else if(op==1)expected=Reference3(name);else ReferenceFree();Capture(a);
            active=native;active->seed=state;int actual=0;
            if(op==0)actual=Native2(name);else if(op==1)actual=Native3(name);else NativeFree();Capture(b);
            if(expected!=actual||memcmp(a,b,sizeof(*a))!=0){fprintf(stderr,"alternate mismatch episode %u step %u op %u name %s\n",episode,step,op,name);return 1;}
        }
        lookups+=native->lookups;creates+=native->creates;loads+=native->loads;deletes+=native->deletes;
        empty+=native->empty_deletes;selections+=native->selections;
    }
    TEST_ASSERT(lookups>100000&&creates>100000&&loads>100000&&deletes>100000&&empty>100000&&selections>10000);
    Guards(native);
    printf("200000 episodes / 2400000 lifecycle operations: %llu lookups, %llu creates, %llu loads, %llu deletes (%llu empty), %llu recovery selections; guards passed\n",
        (unsigned long long)lookups,(unsigned long long)creates,(unsigned long long)loads,(unsigned long long)deletes,(unsigned long long)empty,(unsigned long long)selections);
    free(reference);free(native);free(a);free(b);return 0;
}
