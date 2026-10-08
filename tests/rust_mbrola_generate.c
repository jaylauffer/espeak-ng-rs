/* Native MBROLA generation against extracted retained C decisions/text/effects.
 * SPDX-License-Identifier: GPL-3.0-or-later */
#include "config.h"
#include "test_assert.h"
#include <stdbool.h>
#include <stdint.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <espeak-ng/speak_lib.h>
#include <espeak-ng/espeak_ng.h>
#include "phoneme.h"
#include "synthdata.h"
#include "synthesize.h"
#include "translate.h"
#include "rust_data.h"

typedef struct { int op,index,a,b,c; char text[384]; } Record;
static Record records[2][16000];static int used[2],side;
static PHONEME_LIST lists[2][N_PHONEME_LIST+1];
static PHONEME_TAB phones[N_PHONEME_LIST+1],lengthener;
static PHONEME_TAB *ref_table[N_PHONEME_TAB];
static struct { int pause_factor,wav_factor; } ref_speed;
static int ref_rate,ref_char,ref_word,ref_sentences,ref_events,ref_options;
static FILE *ref_output;
static int free_space=1000,blocked_writes,blocked_flushes,select_calls,embedded_calls;
static int partial_limit;static char admitted[1024];static size_t admitted_length;
static unsigned seed=0x61427389,case_seed;
static unsigned mix(unsigned n){n^=n>>16;n*=0x7feb352d;n^=n>>15;n*=0x846ca68b;return n^(n>>16);}
static unsigned random_u32(void){seed^=seed<<13;seed^=seed>>17;seed^=seed<<5;return seed;}
static int index_of(const PHONEME_LIST *p){return (int)(p-lists[side]);}
static Record *record(int op,int index,int a,int b,int c) {
    TEST_ASSERT(used[side]<16000);Record *r=&records[side][used[side]++];
    memset(r,0,sizeof(*r));r->op=op;r->index=index;r->a=a;r->b=b;r->c=c;return r;
}
static int RefFree(void){return free_space;}
static void RefEmbedded(int *cursor,int source) {
    record(2,0,source,*cursor,0);(*cursor)++;embedded_calls++;
    ref_speed.pause_factor=100+(source%300);ref_speed.wav_factor=128+(source%256);
}
static void RefMarker(int kind,int position,int length,int value){record(3,kind,position,length,value);}
static const char *RefName(char *out,PHONEME_TAB *ph,PHONEME_LIST *p,int ipa,int *flags) {
    TEST_ASSERT(ph==p->ph && flags==NULL);sprintf(out,"%d:%d",index_of(p),ipa);return out;
}
static void RefPhonemeMarker(int kind,int pos,int length,char *name) {
    int ix,ipa;TEST_ASSERT(kind==espeakEVENT_PHONEME && length==0 && sscanf(name,"%d:%d",&ix,&ipa)==2);
    record(4,ix,ipa,pos,0);
}
static int RefSelect(PHONEME_LIST *p,PHONEME_TAB *ph,PHONEME_TAB *previous,PHONEME_TAB *next,int *second,int *percent,int *control) {
    int ix=index_of(p);TEST_ASSERT(ph==p->ph && previous==lists[side][ix-1].ph && next==lists[side][ix+1].ph);
    unsigned h=mix(case_seed+(unsigned)ix);select_calls++;record(5,ix,0,0,0);
    *percent=10+(int)(h%81);*control=(h%7==0);*second=0;
    if(h%5==0)*second='_';else if(h%3==0)*second='z';
    return h%11==0 ? 0 : (h%4==0 ? (int)ph->mnemonic : 'a');
}
static char *RefPitch(int env,int first,int last,int split,int final) {
    static char text[50];record(6,0,split,final,env);
    if(final)snprintf(text,sizeof(text),"\t100 %d\n",last+90);
    else snprintf(text,sizeof(text)," 0 %d 80 %d 100 %d\n",first+60,last+90,env+80);
    return text;
}
static void RefInterpret(Translator *tr,int control,PHONEME_LIST *p,PHONEME_LIST *start,PHONEME_DATA *d,WORD_PH_DATA *word,size_t count) {
    TEST_ASSERT(tr==NULL && control==0 && word==NULL && start==lists[side]);
    record(8,index_of(p),p->synthflags,(int)count,0);memset(d,0,sizeof(*d));
    d->sound_addr[pd_FMT]=100+index_of(p);d->pd_param[0]=50+index_of(p);
}
static int RefSample(PHONEME_DATA *d,int length,int amp) {
    TEST_ASSERT(amp==-1);record(9,0,length,d->pd_param[0],0);
    int result=d->pd_param[0]*7+length;d->pd_param[0]++;return result;
}
static int RefSpect(PHONEME_TAB *ph,int which,FMT_PARAMS *fmt,PHONEME_LIST *p,int modulation) {
    TEST_ASSERT(ph==p->ph && which==0 && modulation==-1);
    FMT_PARAMS expected={0};expected.fmt_addr=100+index_of(p);TEST_ASSERT(memcmp(fmt,&expected,sizeof(expected))==0);
    record(10,index_of(p),fmt->fmt_addr,0,0);return fmt->fmt_addr*9;
}
static int RefPause(int length,int control){record(11,0,length,control,0);return length*(control+1)+ref_speed.pause_factor/100;}
static int RefWrite(const char *text) {
    Record *r=record(12,0,0,0,0);TEST_ASSERT(strlen(text)<sizeof(r->text));strcpy(r->text,text);
    if(blocked_writes){blocked_writes--;return 0;}
    size_t n=strlen(text);if(partial_limit && n>(size_t)partial_limit)n=(size_t)partial_limit;
    if(partial_limit){TEST_ASSERT(admitted_length+n<sizeof(admitted));memcpy(admitted+admitted_length,text,n);admitted_length+=n;}
    return (int)n;
}
static void RefQueue(int length){record(13,0,length,0,0);}
static int RefFlush(void){record(14,0,0,0,0);if(blocked_flushes){blocked_flushes--;return 0;}return 1;}

#define MbrolaTranslate ReferenceTranslate
#define MbrolaGenerate ReferenceGenerate
#define WcmdqFree RefFree
#define DoEmbedded RefEmbedded
#define DoMarker RefMarker
#define WritePhMnemonic RefName
#define DoPhonemeMarker RefPhonemeMarker
#define GetMbrName RefSelect
#define WritePitch RefPitch
#define InterpretPhonemeWithLength RefInterpret
#define DoSample3 RefSample
#define DoSpect2 RefSpect
#define PauseLength RefPause
#define write_MBR RefWrite
#define flush_MBR RefFlush
#define speed ref_speed
#define samplerate ref_rate
#define phoneme_tab ref_table
#define clause_start_char ref_char
#define clause_start_word ref_word
#define count_sentences ref_sentences
#define option_phoneme_events ref_events
#define option_phonemes ref_options
#define f_trans ref_output
#ifndef USE_RUST_CORE
#define USE_RUST_CORE 1
#endif
#define espeak_rs_queue_mbrola(ignored,length) RefQueue(length)
#include "mbrola_generation_reference.inc"
#undef MbrolaTranslate
#undef MbrolaGenerate

static int NativeEffect(void *context,RustMbrGenerateEffect *e) {
    int count=*(int *)context;PHONEME_LIST *p=&lists[side][e->index];
    switch(e->op) {
    case 0:return RefFree();
    case 1: {RustMbrGenerateSettings s={ref_speed.pause_factor,ref_speed.wav_factor,ref_rate,lengthener.std_length,ref_char,ref_word,ref_sentences,ref_events};*e->settings=s;break;}
    case 2:RefEmbedded(e->cursor,e->a);break;
    case 3:RefMarker(e->index,e->a,e->b,e->c);break;
    case 4: {char name[16];RefName(name,p->ph,p,e->a ? espeakINITIALIZE_PHONEME_IPA : 0,NULL);RefPhonemeMarker(espeakEVENT_PHONEME,e->b,0,name);break;}
    case 5:e->selection->name=RefSelect(p,p->ph,lists[side][e->index-1].ph,lists[side][e->index+1].ph,&e->selection->second,&e->selection->percent,&e->selection->control);break;
    case 6: {char *text=RefPitch(p->env,p->pitch1,p->pitch2,e->a,e->b);size_t n=strlen(text);TEST_ASSERT(n<e->capacity);memcpy(e->text,text,n);return (int)n;}
    case 7:p->synthflags=e->a;break;
    case 8:RefInterpret(NULL,0,p,lists[side],e->data,NULL,(size_t)count);break;
    case 9:return RefSample(e->data,e->a,-1);
    case 10:return RefSpect(p->ph,0,e->fmt,p,-1);
    case 11:return RefPause(e->a,e->b);
    case 12:if(e->a){size_t n=fwrite(e->text,1,e->capacity,ref_output);return n ? (int)n : -1;}return RefWrite((const char *)e->text);
    case 13:RefQueue(e->a);break;
    case 15:RefQueue(500);break;
    case 14:return RefFlush();
    }
    return 0;
}
static RustGenerateEntry entries[N_PHONEME_LIST+1];
static void copy_entries(int count) {
    for(int ix=0;ix<=count;ix++) {
        PHONEME_LIST *p=&lists[1][ix];RustGenerateEntry *e=&entries[ix];memset(e,0,sizeof(*e));
        if(p->ph){e->phoneme=*p->ph;e->present=1;}e->length=p->length;e->synthflags=p->synthflags;e->source=p->sourceix;
        e->type=p->type;e->newword=p->newword;e->prepause=p->prepause;e->env=p->env;e->pitch1=p->pitch1;e->pitch2=p->pitch2;
    }
}
static void reset_side(int which) {
    side=which;used[side]=0;ref_speed.pause_factor=256;ref_speed.wav_factor=256;
    select_calls=embedded_calls=0;blocked_writes=blocked_flushes=0;free_space=1000;
}
static int make_list(void) {
    int count=2+(int)(random_u32()%40);if(case_seed%17==0)count=1000;
    memset(lists,0,sizeof(lists));
    for(int ix=0;ix<=count;ix++) {
        PHONEME_TAB *ph=&phones[ix];memset(ph,0,sizeof(*ph));ph->type=(int)(random_u32()%10);
        ph->std_length=20+(random_u32()%180);ph->mnemonic='b';ph->code=random_u32()%10==0 ? phonEND_WORD : 40;
        PHONEME_LIST *p=&lists[0][ix];p->ph=ph;p->type=(random_u32()%7==0) ? (int)(random_u32()%10) : ph->type;
        p->length=20+random_u32()%700;p->synthflags=(random_u32()%3==0?SFLAG_LENGTHEN:0)|(random_u32()%5==0?SFLAG_EMBEDDED:0);
        p->sourceix=(uint16_t)random_u32();p->newword=random_u32()%5==0?PHLIST_START_OF_SENTENCE:(random_u32()%3==0);
        p->prepause=random_u32()%100;p->env=random_u32()%20;p->pitch1=random_u32()%100;p->pitch2=random_u32()%100;
    }
    memcpy(lists[1],lists[0],sizeof(lists[0]));return count;
}
int main(void) {
    ref_rate=22050;ref_char=800;ref_word=60;ref_sentences=12;ref_events=espeakINITIALIZE_PHONEME_IPA;
    lengthener.std_length=70;ref_table[phonLENGTHEN]=&lengthener;
    size_t effects=0,bytes=0;
    for(unsigned n=0;n<3000;n++) {
        case_seed=random_u32();int count=make_list();bool file=n%3==0;
        if(n%97==0) {
            count=2;phones[1].type=phVSTOP;lists[0][0].ph=NULL;lists[0][2].ph=NULL;
            memcpy(lists[1],lists[0],sizeof(lists[0]));
        }
        reset_side(0);ref_output=file?tmpfile():NULL;TEST_ASSERT(!file || ref_output);
        free_space=24;TEST_ASSERT(ReferenceTranslate(lists[0],count,false,ref_output)==1);
        free_space=1000;TEST_ASSERT(ReferenceTranslate(lists[0],count,true,ref_output)==0);
        long size=0;char *reference=NULL;
        if(file){size=ftell(ref_output);reference=malloc((size_t)size+1);rewind(ref_output);TEST_ASSERT(fread(reference,1,(size_t)size,ref_output)==(size_t)size);fclose(ref_output);}
        reset_side(1);copy_entries(count);ref_output=file?tmpfile():NULL;void *owner=espeak_rs_mbrola_generator_create();TEST_ASSERT(owner);
        free_space=24;TEST_ASSERT(espeak_rs_mbrola_generate(owner,entries,count+1,count,0,file,&count,NativeEffect)==1);
        free_space=1000;TEST_ASSERT(espeak_rs_mbrola_generate(owner,entries,count+1,count,1,file,&count,NativeEffect)==0);
        TEST_ASSERT(used[0]==used[1]);
        for(int i=0;i<used[0];i++) {
            if(memcmp(&records[0][i],&records[1][i],sizeof(Record))!=0){fprintf(stderr,"case %u effect %d: C %d/%d/%d/%d/%d [%s], Rust %d/%d/%d/%d/%d [%s]\n",n,i,records[0][i].op,records[0][i].index,records[0][i].a,records[0][i].b,records[0][i].c,records[0][i].text,records[1][i].op,records[1][i].index,records[1][i].a,records[1][i].b,records[1][i].c,records[1][i].text);TEST_ASSERT(false);}
        }
        for(int ix=0;ix<=count;ix++)TEST_ASSERT(lists[0][ix].synthflags==lists[1][ix].synthflags);
        if(file){TEST_ASSERT(ftell(ref_output)==size);rewind(ref_output);for(long i=0;i<size;i++)TEST_ASSERT(fgetc(ref_output)==(unsigned char)reference[i]);fclose(ref_output);free(reference);bytes+=(size_t)size;}
        effects+=(size_t)used[0];espeak_rs_mbrola_generator_destroy(owner);
    }
    printf("3000 MBROLA clauses: %zu ordered effects and %zu file bytes match C\n",effects,bytes);
    /* Deliberately reproduce the retained C's failed-admission bug separately
     * from the defined-domain oracle. A control-1 mapping advances phix before
     * its rejected write: C then resumes at phoneme 2, whereas native commits
     * the retained phoneme-1 command and correctly continues at phoneme 3. */
    make_list();int count=4;
    for(int ix=0;ix<=count;ix++) {
        phones[ix].type=phVSTOP;lists[0][ix].ph=&phones[ix];
        lists[0][ix].synthflags=SFLAG_EMBEDDED;lists[0][ix].newword=PHLIST_START_OF_SENTENCE;
    }
    memcpy(lists[1],lists[0],sizeof(lists[0]));
    for(case_seed=0;;case_seed++) {
        unsigned a=mix(case_seed+1),b=mix(case_seed+2),c=mix(case_seed+3);
        if(a%7==0 && a%11!=0 && b%7!=0 && b%11!=0 && c%11!=0)break;
    }
    reset_side(0);blocked_writes=1;
    TEST_ASSERT(ReferenceTranslate(lists[0],count,false,NULL)==1);
    TEST_ASSERT(ReferenceTranslate(lists[0],count,true,NULL)==0);
    TEST_ASSERT(select_calls==3 && embedded_calls==3);
    int old_second=-1;
    for(int i=0,seen=0;i<used[0];i++)if(records[0][i].op==5 && ++seen==2)old_second=records[0][i].index;
    TEST_ASSERT(old_second==2);
    reset_side(1);copy_entries(count);blocked_writes=1;
    void *owner=espeak_rs_mbrola_generator_create();TEST_ASSERT(owner);
    TEST_ASSERT(espeak_rs_mbrola_generate(owner,entries,count+1,count,0,0,&count,NativeEffect)==1);
    TEST_ASSERT(espeak_rs_mbrola_generate(owner,entries,count+1,count,1,0,&count,NativeEffect)==0);
    TEST_ASSERT(select_calls==2 && embedded_calls==2);
    int native_second=-1;
    for(int i=0,seen=0;i<used[1];i++)if(records[1][i].op==5 && ++seen==2)native_second=records[1][i].index;
    TEST_ASSERT(native_second==3);
    espeak_rs_mbrola_generator_destroy(owner);
    puts("Retained C resumes rejected control-1 mapping at phoneme 2; native retains it and resumes at phoneme 3");
    /* Exercise actual byte-count returns through the C ABI, including zero
     * after an accepted prefix. Compare concatenated accepted bytes against
     * a complete admission, rather than reproducing the old C truncation. */
    char expected[1024];size_t expected_length=0;
    reset_side(1);copy_entries(count);owner=espeak_rs_mbrola_generator_create();TEST_ASSERT(owner);
    TEST_ASSERT(espeak_rs_mbrola_generate(owner,entries,count+1,count,0,0,&count,NativeEffect)==0);
    for(int i=0;i<used[1];i++)if(records[1][i].op==12) {
        size_t n=strlen(records[1][i].text);TEST_ASSERT(expected_length+n<sizeof(expected));
        memcpy(expected+expected_length,records[1][i].text,n);expected_length+=n;
    }
    espeak_rs_mbrola_generator_destroy(owner);
    reset_side(1);copy_entries(count);partial_limit=2;admitted_length=0;
    owner=espeak_rs_mbrola_generator_create();TEST_ASSERT(owner);
    TEST_ASSERT(espeak_rs_mbrola_generate(owner,entries,count+1,count,0,0,&count,NativeEffect)==1);
    TEST_ASSERT(admitted_length==2 && select_calls==1 && embedded_calls==1);
    blocked_writes=1;
    TEST_ASSERT(espeak_rs_mbrola_generate(owner,entries,count+1,count,1,0,&count,NativeEffect)==1);
    TEST_ASSERT(admitted_length==2 && select_calls==1 && embedded_calls==1);
    int result=1;
    for(int retries=0;retries<512 && result==1;retries++)
        result=espeak_rs_mbrola_generate(owner,entries,count+1,count,1,0,&count,NativeEffect);
    TEST_ASSERT(result==0 && select_calls==2 && embedded_calls==2);
    TEST_ASSERT(admitted_length==expected_length && memcmp(admitted,expected,expected_length)==0);
    espeak_rs_mbrola_generator_destroy(owner);
    puts("Partial C-ABI writes preserve every command byte and issue phoneme effects once");
    return 0;
}
