/* Native spelling/plural policies against unchanged C references.
 * SPDX-License-Identifier: GPL-3.0-or-later */
#include "config.h"
#include "test_assert.h"
#include <stdint.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <espeak-ng/speak_lib.h>
#include "translate.h"
#include "translateword.h"
#include "dictionary.h"
#include "numbers.h"
#include "phoneme.h"
#include "common.h"
#include "readclause.h"
#include "rust_word_helpers.h"
typedef struct {
    Translator tr;
    char source[256], phonemes[200], output[200];
    unsigned seed, letters, stresses, rules;
    uint64_t trace;
    int alias;
} Fixture;
static Fixture *active;
static int native_rules;
static void Trace(unsigned n) {active->trace=(active->trace^n)*UINT64_C(1099511628211);}
static void Bytes(const char *s) {for(const unsigned char *p=(const unsigned char *)s;*p;p++)Trace(*p);Trace(0);}
static int MockLetter(Translator *tr,char *source,char *out,int control,const ALPHABET *alphabet)
{
    Trace(1);Bytes(source);Bytes(out);Trace((unsigned)control);Trace(alphabet!=NULL);
    int code;int consumed=utf8_in(&code,source);Trace((unsigned)code);
    active->letters++;
    tr->expect_verb=(int)(active->seed%7)+ (int)active->letters;
    if(active->seed%7==0 && active->letters==active->seed%3+1) strcpy(out,"\025fr");
    else strcat(out,active->letters%2?"a":"b");
    if(active->seed%11==0 && source[consumed]=='b') source[consumed]='x';
    if(active->seed&8) active->output[80]=(char)active->letters;
    return out[0]==phonSWITCH?0:consumed;
}
static void MockStress(Translator *tr,char *out,int mode,int count)
{
    Trace(2);Trace((unsigned)mode);Trace((unsigned)count);Bytes(out);
    active->stresses++;tr->expect_past=count;strcat(out,"stress");
}
static int MockRules(Translator *tr,char *source,char *out,int capacity,char *ending,int control,unsigned *flags)
{
    TEST_ASSERT(capacity==200&&ending==NULL&&control==0&&flags==NULL);
    TEST_ASSERT(source[-2]==0&&source[-1]==' '&&source[2]==0);
    if(native_rules) TEST_ASSERT(tr->rule_text_base==source-2&&tr->rule_text_length==5);
    Trace(3);Bytes(source);Bytes(out);active->rules++;tr->expect_noun=(int)active->seed;
    strcat(out,source);source[-1]='?';out[90]=(char)(active->seed>>2);
    return 7;
}
#define TranslateLetter MockLetter
#define SetSpellingStress MockStress
#define TranslateRules MockRules
#define SpeakIndividualLetters ReferenceSpell
#include "retained_word_spelling.inc"
#undef SpeakIndividualLetters
#define addPluralSuffixes ReferencePlural
#include "retained_word_plural.inc"
#undef addPluralSuffixes
#define SpeakIndividualLetters NativeSpell
#include "native_word_source_projections.inc"
#include "native_word_helper_projections.inc"
#undef SpeakIndividualLetters
static void Init(Fixture *f,unsigned trial)
{
    memset(f,0,sizeof(*f));f->seed=trial*0xabcdeu+23;f->trace=UINT64_C(1469598103934665603);
    /* Original strcpy traps when a switch copies an aliased buffer on macOS.
     * Compare only valid retained-C calls; test that native case separately. */
    f->alias=(trial&1)!=0&&f->seed%7!=0;
    memset(f->phonemes,0x5a,sizeof(f->phonemes));strcpy(f->phonemes,"pron");
    memset(f->output,0x6b,sizeof(f->output));strcpy(f->output,"owner");
    f->source[0]='!';
}
static void Activate(Fixture *f)
{
    active=f;translator=&f->tr;translator2=translator3=NULL;
    f->tr.rule_text_base=f->source;f->tr.rule_text_length=sizeof(f->source);
}
static void Compare(Fixture *old,Fixture *native,unsigned trial,int expected,int actual)
{
    old->tr.rule_text_base=native->tr.rule_text_base=NULL;
    if(expected!=actual||old->trace!=native->trace||old->letters!=native->letters||old->stresses!=native->stresses||old->rules!=native->rules||memcmp(old->source,native->source,256)||memcmp(old->output,native->output,200)||memcmp(old->phonemes,native->phonemes,200)||memcmp(&old->tr,&native->tr,sizeof(old->tr))) {
        fprintf(stderr,"word helper mismatch trial=%u cursor=%d/%d letters=%u/%u stress=%u/%u rules=%u/%u\n",trial,actual,expected,native->letters,old->letters,native->stresses,old->stresses,native->rules,old->rules);exit(1);
    }
}
static int GuardLetter(void *opaque,RustWordSource source,unsigned control,size_t *count)
{
    (void)source;(void)control;
    RustWordHelpersHost *host=opaque;
    memset(host->phonemes,'x',200);host->phonemes[0]=active->seed?phonSWITCH:'x';
    active->letters++;*count=active->seed?1:0;return 0;
}
int main(void)
{
    unsigned long letters=0,switches=0,stresses=0,rules=0;
    static const char *words[]={"", " tail", "abc tail", "ab", "aé𐐀 b", "ЖЯ ","κα ","कअ ","你好 ","🙂x ","a'b ","xyz "};
    static const int modes[]={-3,0,1,2,3,4,20};
    ALPHABET alphabet={0};
    for(unsigned trial=0;trial<200000;trial++) {
        Fixture old,native;Init(&old,trial);strcpy(old.source+1,words[trial%12]);native=old;
        int mode=modes[(trial/12)%7];const ALPHABET *a=trial&2?&alphabet:NULL;
        Activate(&old);char *expected=ReferenceSpell(&old.tr,old.source+1,old.alias?old.output:old.phonemes,mode,a,old.output);
        int old_offset=expected==NULL?-1:(int)(expected-old.source);
        Activate(&native);char *actual=NativeSpell(&native.tr,native.source+1,native.alias?native.output:native.phonemes,mode,a,native.output);
        int native_offset=actual==NULL?-1:(int)(actual-native.source);
        Compare(&old,&native,trial,old_offset,native_offset);letters+=old.letters;switches+=expected==NULL;stresses+=old.stresses;
    }
    for(unsigned trial=0;trial<200000;trial++) {
        Fixture old,native;Init(&old,trial);native=old;
        unsigned flags=trial%3==0?0:(trial&1?FLAG_HAS_PLURAL:0xffffffffu);
        unsigned char last=(unsigned char)(trial/3);
        Activate(&old);native_rules=0;ReferencePlural((int)flags,&old.tr,(char)last,old.output);
        Activate(&native);native_rules=1;
        RustWordHelpersHost host={.source={.tr=&native.tr,.output=native.output}};
        RustWordHelpers callbacks=HelpersCallbacks(&host);
        TEST_ASSERT(espeak_rs_word_plural(&callbacks,flags,last)==0);
        TEST_ASSERT(native.tr.rule_text_base==native.source&&native.tr.rule_text_length==sizeof(native.source));
        Compare(&old,&native,trial,0,0);rules+=old.rules;
    }
    Fixture alias;
    for(unsigned trial=0;;trial++) {Init(&alias,trial);if(alias.seed%7==0)break;}
    strcpy(alias.source+1,"abc ");Activate(&alias);
    TEST_ASSERT(NativeSpell(&alias.tr,alias.source+1,alias.output,3,NULL,alias.output)==NULL);
    TEST_ASSERT(strcmp(alias.output,"\025fr")==0&&alias.stresses==0&&alias.letters==alias.seed%3+1);
    RustWordSource cursor={9,123};int switched=17;RustWordHelpers absent={0};
    TEST_ASSERT(espeak_rs_word_spell(NULL,0,&cursor,&switched)==-1&&cursor.slot==9&&cursor.offset==123&&switched==17);
    TEST_ASSERT(espeak_rs_word_plural(&absent,4,'f')==-1);
    Fixture f;Init(&f,1);Activate(&f);
    RustWordHelpersHost host={.source={.tr=&f.tr,.output=f.output},.phonemes=f.phonemes};
    RustWordHelpers callbacks=HelpersCallbacks(&host);
    TEST_ASSERT(espeak_rs_word_spell(&callbacks,0,&cursor,&switched)==-1&&cursor.slot==9&&cursor.offset==123&&switched==17);
    strcpy(f.source+1,"a ");TEST_ASSERT(WordWindow(&host.source,f.source+1)==0);
    callbacks.letter=GuardLetter;
    for(unsigned progress=0;progress<2;progress++) {
        f.seed=progress;unsigned before=f.letters;
        TEST_ASSERT(espeak_rs_word_spell(&callbacks,0,&cursor,&switched)==-1&&cursor.slot==9&&cursor.offset==123&&switched==17);
        TEST_ASSERT(f.letters==before+1&&f.phonemes[0]==(progress?phonSWITCH:'x')&&strcmp(f.output,"owner")==0&&f.stresses==0);
    }
    unsigned before=f.letters;
    TEST_ASSERT(espeak_rs_word_spell(&callbacks,0,NULL,&switched)==-1&&f.letters==before);
    printf("200000 spelling and 200000 plural comparisons passed; %lu letter calls, %lu switches, %lu stresses, %lu rules; alias/scoped-context and ABI guards passed\n",letters,switches,stresses,rules);
    return 0;
}
