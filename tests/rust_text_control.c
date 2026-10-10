/* Connected text-control policies versus unchanged C and actual projections.
 * SPDX-License-Identifier: GPL-3.0-or-later */
#include "config.h"
#include "test_assert.h"
#include <stdint.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <ctype.h>
#include <locale.h>
#include <wchar.h>
#include <wctype.h>
#include "translate.h"
#include "common.h"
#include "phoneme.h"
#include "synthesize.h"
#include "synthdata.h"
#include "readclause.h"
#include "rust_text_control.h"
extern char word_phonemes[N_WORD_PHONEMES];
typedef struct {
    Translator current,main;WORD_TAB rows[4];int v[22],flags,selected;
    char clause[840],word[840],ph[200],marker[N_MARKER_LENGTH];unsigned char p[200],types[256];
    unsigned commands[250],seed,calls,lookups,selections,initializers;uint64_t trace;
} Fixture;
static Fixture *active;static PHONEME_TAB phonemes[256];static int mutate_context;
static void Event(unsigned x){active->trace=(active->trace^x)*UINT64_C(1099511628211);}
#define embedded_ix (active->v[0])
#define embedded_read (active->v[1])
#define option_sayas (active->v[2])
#define option_sayas2 (active->v[3])
#define count_sayas_digits (active->v[4])
#define word_emphasis (active->v[5])
#define option_emphasis (active->v[6])
#define pre_pause (active->v[7])
#define dictionary_skipwords (active->v[10])
#define count_sentences (active->v[12])
#define count_words (active->v[13])
#define end_character_position (active->v[14])
#define skip_sentences (active->v[15])
#define skip_words (active->v[16])
#define skip_characters (active->v[17])
#define skipping_text (active->v[18])
#define new_sentence (active->v[19])
#define embedded_flag (active->v[20])
#define source (active->clause)
#define skip_marker (active->marker)
#define embedded_list (active->commands)
#define translator (&active->main)
#define translator2 NULL
#define translator3 NULL
static int MockTranslate(Translator *tr,char *word,WORD_TAB *rows,int remaining,char *out,int depth)
{
    TEST_ASSERT(out==NULL&&depth==0&&tr->rule_text_base==active->word&&tr->rule_text_length==840);
    TEST_ASSERT(rows>=active->rows&&rows<active->rows+4&&remaining>=1&&remaining<=4-(rows-active->rows));
    Event(tr==translator?1:2);Event((unsigned)(word-active->word));Event((unsigned)(rows-active->rows));Event((unsigned)remaining);
    for(size_t i=0;i<30&&word[i];i++)Event((unsigned char)word[i]);
    unsigned lane=active->calls++;active->v[7]++;rows->flags|=0x20;
    if(word[0]=='q')word[0]='r';
    unsigned flags=lane==0?(active->seed%5==0?FLAG_WAS_UNPRONOUNCABLE:active->seed%3==0?FLAG_ALT_TRANS:0):lane==1?(active->seed%4==0?0:0x42):0x102;
    unsigned n=1+active->seed%7;for(unsigned i=0;i<n;i++)word_phonemes[i]=(char)(1+(active->seed+i)%7);
    if(lane==0&&active->seed%7==0)word_phonemes[0]=phonSWITCH;word_phonemes[n]=0;
    if(lane==1){phonemes[3].type=phVOWEL;active->types[3]=phVOWEL;}
    if(mutate_context){tr->rule_text_base="changed";tr->rule_text_length=8;}
    return (int)flags;
}
static int MockLookup(const char *name){active->lookups++;Event(31);unsigned n=0;for(size_t i=0;name[i];i++){Event((unsigned char)name[i]);n++;}return active->seed%3?1+(int)(n*13):0;}
static int MockSelect(int table){active->selections++;Event((unsigned)table);active->selected=table;word_phonemes[3]=71;return table;}
static void MockInit2(void){active->initializers++;Event(41);count_words=73;}
static void MockNamed(void){active->initializers++;Event(42);word_emphasis=62;}
#define TranslateWordWithBounds MockTranslate
#define LookupPhonemeTable MockLookup
#define SelectPhonemeTable MockSelect
#define InitText2 MockInit2
#define InitNamedata MockNamed
#define CountSyllables ReferenceCount
#define Word_EmbeddedCmd ReferenceWordCommands
#include "retained_word_command_policy.inc"
#undef CountSyllables
#undef Word_EmbeddedCmd
#define EmbeddedCommand ReferenceEmbedded
#include "retained_embedded_parse_policy.inc"
#undef EmbeddedCommand
#define CombineFlag ReferenceCombine
#define CountSyllables ReferenceCount
#define SwitchLanguage ReferenceSwitch
#define InitText ReferenceInit
#include "retained_text_control_tail.inc"
#undef CombineFlag
#undef CountSyllables
#undef SwitchLanguage
#undef InitText
#define Word_EmbeddedCmd NativeWordCommands
#define EmbeddedCommand NativeEmbedded
#define CombineFlag NativeCombine
#define SwitchLanguage NativeSwitch
#define InitText NativeInit
#include "native_text_control_projections.inc"
#undef Word_EmbeddedCmd
#undef EmbeddedCommand
#undef CombineFlag
#undef SwitchLanguage
#undef InitText
static void Activate(Fixture *f){active=f;memcpy(word_phonemes,f->ph,200);for(unsigned i=0;i<256;i++){phonemes[i].type=f->types[i];phoneme_tab[i]=phonemes+i;}}
static void Capture(Fixture *f){memcpy(f->ph,word_phonemes,200);f->current.rule_text_base=NULL;f->main.rule_text_base=NULL;}
static void Init(Fixture *f,unsigned trial)
{
    memset(f,0,sizeof(*f));f->seed=trial*1664525u+1013904223u;
    for(unsigned i=0;i<22;i++)f->v[i]=7+(int)(f->seed%20);f->v[0]=trial%251;f->v[1]=trial%249;
    f->v[9]=1+trial%4;f->flags=0x4123;f->v[10]=3;
    f->current.rule_text_base=f->word;f->main.rule_text_base=f->word;
    f->current.rule_text_length=f->main.rule_text_length=840;
    f->current.langopts.param[LOPT_COMBINE_WORDS]=(int)(trial%8)+(trial%3==0?0x100:0)+(trial%5==0?0x200:0);
    for(unsigned i=0;i<4;i++){f->rows[i].flags=1+(trial%7==0?FLAG_LAST_WORD:0);f->rows[i].start=i*3;}
    memset(f->ph,0x55,200);f->ph[0]=1;f->ph[1]=2;f->ph[2]=0;
    f->p[0]=1;f->p[1]=2;f->p[2]=3;f->p[3]=0;memset(f->marker,0x55,sizeof(f->marker));
    for(unsigned i=0;i<256;i++)f->types[i]=i%3==0?phVOWEL:phSTOP;
    for(unsigned i=0;i<250;i++)f->commands[i]=(unsigned)(i%19==0?EMBED_B:i%3==0?EMBED_Y:i%3==1?EMBED_F:EMBED_B)+(i%19==0?0xfffffe00u:((i%17)<<8))+(i%5==0?0x80:0);
    strcpy(f->word,trial%5==0?"ab 9 ":trial%7==0?"ab qd ":"ab cd ");
    const char *tokens[]={"+3F","-12Y","P","p","?","","77B","+V","-0A","123456P","Y","2F"};
    strcpy(f->clause+3,tokens[trial%12]);
}
static int RefuseVowel(void *opaque,unsigned char code,int *out){(void)opaque;(void)code;(void)out;return -1;}
static void Guards(Fixture *f)
{
    Init(f,1);Activate(f);RustTextHost h={.tr=&f->current,.word=f->word,.rows=f->rows,.remaining=4,.pronunciation=word_phonemes,.count=(unsigned char *)word_phonemes,.flags=&f->flags};
    ControlWordBounds(&h);RustTextControl c=ControlCallbacks(&h);RustTextResult out={99,77};
    TEST_ASSERT(espeak_rs_text_control(NULL,2,0,&out)==-1&&out.value==99&&out.index==77);
    c.translate=NULL;TEST_ASSERT(espeak_rs_text_control(&c,2,0,&out)==-1&&out.value==99&&out.index==77);
    c=ControlCallbacks(&h);TEST_ASSERT(espeak_rs_text_control(&c,0,SIZE_MAX,&out)==-1&&out.value==99&&out.index==77);
    strcpy(source,"2147483648Y");TEST_ASSERT(espeak_rs_text_control(&c,0,0,&out)==-1&&embedded_ix==1&&out.value==99);
    f->current.langopts.param[LOPT_COMBINE_WORDS]=1;f->seed=1;c.vowel=RefuseVowel;
    TEST_ASSERT(espeak_rs_text_control(&c,2,0,&out)==-1&&f->calls==2&&f->flags==0x42&&f->word[2]=='-'&&out.value==99&&out.index==77);
    mutate_context=1;unsigned flags=0;TEST_ASSERT(ControlTranslate(&h,1,0,0,4,&flags)==0&&f->main.rule_text_base==f->word&&f->main.rule_text_length==840);mutate_context=0;
    TEST_ASSERT(ControlTranslate(&h,1,SIZE_MAX,0,4,&flags)==-1&&ControlTranslate(&h,1,0,4,1,&flags)==-1);
    h.base=NULL;h.length=0;TEST_ASSERT(espeak_rs_text_control(&c,2,0,&out)==-1);
}
int main(void)
{
    setlocale(LC_CTYPE,"");Fixture *a=malloc(sizeof(*a)),*b=malloc(sizeof(*b));TEST_ASSERT(a&&b);
    unsigned accepted=0,word_commands=0,translations=0,switches=0,resets=0;unsigned nuls=0;
    for(unsigned trial=0;trial<200000;trial++){
        for(unsigned mode=0;mode<6;mode++){
            Init(a,trial);Init(b,trial);unsigned old_index=3,new_index=3;int expected=0,actual=0;
            if(mode==3){strcpy(a->word,"_^_EN ");strcpy(b->word,"_^_EN ");}
            Activate(a);switch(mode){case 0:expected=ReferenceEmbedded(&old_index);break;case 1:ReferenceWordCommands();break;
            case 2:ReferenceCombine(&a->current,a->rows,a->v[9],a->word,&a->flags,trial%2?a->p:(unsigned char *)word_phonemes,word_phonemes);break;
            case 3:ReferenceSwitch(a->word,word_phonemes);break;case 4:ReferenceInit(trial%2?espeakKEEP_NAMEDATA:0);break;case 5:expected=ReferenceCount(a->p);break;}
            Capture(a);Activate(b);switch(mode){case 0:actual=NativeEmbedded(&new_index);break;case 1:NativeWordCommands();break;
            case 2:NativeCombine(&b->current,b->rows,b->v[9],b->word,&b->flags,trial%2?b->p:(unsigned char *)word_phonemes,word_phonemes);break;
            case 3:NativeSwitch(b->word,word_phonemes);break;case 4:NativeInit(trial%2?espeakKEEP_NAMEDATA:0);break;
            case 5:{RustTextHost h={.count=b->p};RustTextControl c=ControlCallbacks(&h);RustTextResult out;TEST_ASSERT(espeak_rs_text_control(&c,5,0,&out)==0);actual=(int)out.value;break;}}
            Capture(b);
            if(expected!=actual||old_index!=new_index||memcmp(a,b,sizeof(*a))){fprintf(stderr,"text control mismatch trial=%u mode=%u return=%d/%d index=%u/%u calls=%u/%u flags=%x/%x\n",trial,mode,actual,expected,new_index,old_index,b->calls,a->calls,b->flags,a->flags);
                const unsigned char *x=(const unsigned char *)a,*y=(const unsigned char *)b;for(size_t i=0;i<sizeof(*a);i++)if(x[i]!=y[i]){fprintf(stderr,"byte %zu %u/%u\n",i,y[i],x[i]);break;}return 1;}
            if(mode==0){accepted+=(unsigned)actual;nuls+=actual&&trial%12==5;}if(mode==1)word_commands+=(unsigned)b->v[1]-trial%249;
            if(mode==2)translations+=b->calls;if(mode==3)switches+=b->selections;if(mode==4)resets+=b->initializers;
        }
    }
    TEST_ASSERT(accepted>1000&&translations>1000&&switches>1000&&nuls>1000);Guards(a);free(a);free(b);
    printf("1200000 retained-C text-control episodes: %u embedded admissions (%u NUL), %u word commands, %u translations, %u table selections, %u reset primitives; ABI/bounds/effect/context guards pass\n",accepted,nuls,word_commands,translations,switches,resets);
    return 0;
}
