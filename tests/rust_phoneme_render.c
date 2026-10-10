/* Actual retained renderer and production effect bridge over identical leaves.
 * SPDX-License-Identifier: GPL-3.0-or-later */
#include "config.h"
#include "test_assert.h"
#include "dictionary.h"
#include "rust_data.h"
#include "common.h"
#include <stdint.h>
#include <stddef.h>
#include <stdlib.h>
#include <stdio.h>
#include <string.h>
#include <wctype.h>
static PHONEME_LIST nodes[N_PHONEME_LIST+1];
static PHONEME_TAB records[8],*tables[256];
static PHONEME_TAB_LIST names[N_PHONEME_TABS];
static int node_count;
static unsigned seed,calls,mode,fail,reenter;
static uint64_t trace;
static const char *NativeClause(int);
static void Trace(unsigned value){trace=(trace^value)*UINT64_C(1099511628211);}
static void Ipa(PHONEME_LIST *node,int code,PHONEME_DATA *data) {
    Trace(node?(unsigned)(node-nodes)+1:0);Trace((unsigned)code);Trace((unsigned)node_count);++calls;
    memset(data,0,sizeof(*data));
    static const char *ipa[]={"","a","\7ts"," ","\3","\xc9\x99","t\xca\x83","\4t\xca\x83"};
    strcpy(data->ipa_string,ipa[(seed+calls)%8]);
    if(fail==1)memset(data->ipa_string,'a',sizeof(data->ipa_string));
    if(node && (mode&1)) {
        node->synthflags=(unsigned short)((seed+calls)&12);node->stresslevel=(unsigned char)((seed+calls)%8);
        node->newword=(unsigned char)((seed+calls)%16);node->type=(seed&2)?phVOWEL:phFRICATIVE;
        if(mode&2)node->ph=&records[6];
        if(mode&4)node->tone_ph_data=(seed&4)?&records[7]:NULL;
        tables[phonLENGTHEN]=&records[seed&1?4:5];
        if(calls==1)node_count+=seed&8?1:-1;
    }
    if(reenter && calls==1)TEST_ASSERT(strcmp(NativeClause(2),"")==0);
}
static void Interpret2(int code,PHONEME_DATA *data){Ipa(NULL,code,data);}
static void InterpretList(Translator *tr,int control,PHONEME_LIST *node,PHONEME_LIST *start,
    PHONEME_DATA *data,WORD_PH_DATA *worddata,size_t count) {
    TEST_ASSERT(tr==NULL && control==0 && start==nodes && worddata==NULL && count==(size_t)node_count);
    Ipa(node,node->ph->code,data);
}
static PHONEME_TAB *Tone(const PHONEME_LIST *node) {
    return node->tone_ph_data ? node->tone_ph_data : tables[node->tone_ph];
}
static int Alpha(unsigned code){Trace(code);return iswalpha(code)!=0;}
#define phoneme_list nodes
#define n_phoneme_list node_count
#define phoneme_tab tables
#define phoneme_tab_list names
#define InterpretPhoneme2 Interpret2
#define InterpretPhonemeWithLength InterpretList
#define TonePhoneme Tone
#undef iswalpha
#define iswalpha Alpha
#define WritePhMnemonic RefWrite
#define WritePhMnemonicWithStress RefStress
#define GetTranslatedPhonemeString RefClause
#include "phoneme_render_reference.inc"
#undef WritePhMnemonic
#undef WritePhMnemonicWithStress
#undef GetTranslatedPhonemeString
#define WritePhMnemonic NativeWrite
#define WritePhMnemonicWithStress NativeStress
#define WritePhMnemonicBounded NativeBounded
#define WritePhMnemonicWithStressBounded NativeStressBounded
#define GetTranslatedPhonemeString NativeClause
#include "native_phoneme_render_effects.inc"
#undef iswalpha
static void Reset(unsigned value) {
    seed=value;calls=0;trace=UINT64_C(1469598103934665603);fail=reenter=0;
    memset(nodes,0,sizeof(nodes));memset(records,0,sizeof(records));memset(names,0,sizeof(names));
    static const char *mnemonics[]={"ts","a#","_:","A12/",":","=","s","T"};
    for(unsigned i=0;i<8;i++) {
        memcpy(&records[i].mnemonic,mnemonics[i],strlen(mnemonics[i]));records[i].code=40+i;records[i].type=phFRICATIVE;
    }
    records[1].type=phVOWEL;records[6].code=seed&4?phonSWITCH:46;
    for(unsigned i=0;i<256;i++)tables[i]=&records[i%8];
    tables[phonLENGTHEN]=&records[4];tables[phonSYLLABIC]=&records[5];
    for(unsigned i=0;i<N_PHONEME_TABS;i++)snprintf(names[i].name,32,"v%u",i);
    node_count=8+(int)(seed%10);
    for(unsigned i=0;i<N_PHONEME_LIST+1;i++) {
        nodes[i].ph=&records[i%6];nodes[i].phcode=nodes[i].ph->code;
        nodes[i].synthflags=(unsigned short)((seed+i)&12);nodes[i].stresslevel=(unsigned char)((seed+i)%8);
        nodes[i].newword=(unsigned char)((seed+i)%16);nodes[i].tone_ph=(unsigned char)((seed+i)%6);
        nodes[i].tone_ph_data=(seed&1)?&records[7]:NULL;nodes[i].type=(seed&2)?phVOWEL:phFRICATIVE;
    }
}
int main(void) {
    TEST_ASSERT(espeak_rs_phoneme_render_layout(0)==sizeof(PHONEME_LIST));
    TEST_ASSERT(espeak_rs_phoneme_render_layout(1)==offsetof(PHONEME_LIST,ph));
    TEST_ASSERT(espeak_rs_phoneme_render_layout(2)==offsetof(PHONEME_LIST,tone_ph_data));
    TEST_ASSERT(espeak_rs_phoneme_render_layout(3)==offsetof(PHONEME_LIST,newword));
    TEST_ASSERT(espeak_rs_phoneme_render_layout(4)==sizeof(PHONEME_TAB_LIST));
    TEST_ASSERT(espeak_rs_phoneme_render_layout(5)==offsetof(PHONEME_TAB_LIST,phoneme_tab_ptr));
    unsigned writers=0,clauses=0;
    for(unsigned episode=0;episode<20000;episode++) {
        mode=episode%8;
        for(unsigned kind=0;kind<8;kind++) {
            unsigned value=episode*31+kind;
            char expected[64],actual[64];int expected_flags=0x7788,actual_flags=0x7788;
            memset(expected,0x55,64);memset(actual,0x55,64);
            Reset(value);PHONEME_LIST *node=kind&4?NULL:&nodes[2];PHONEME_TAB *ph=&records[kind%6];
            if(kind%4==0)ph=&records[6];if(kind%5==0)ph=&records[7];
            if(ph->code==phonSWITCH && node==NULL)node=&nodes[2];
            char *end=(kind&2)&&node?RefStress(expected,ph,node,kind&1,&expected_flags):RefWrite(expected,ph,node,kind&1,&expected_flags);
            ptrdiff_t length=end-expected;uint64_t expected_trace=trace;unsigned expected_calls=calls;
            PHONEME_LIST saved[20];memcpy(saved,nodes,sizeof(saved));int count=node_count;
            Reset(value);node=kind&4?NULL:&nodes[2];ph=&records[kind%6];
            if(kind%4==0)ph=&records[6];if(kind%5==0)ph=&records[7];if(ph->code==phonSWITCH && node==NULL)node=&nodes[2];
            end=(kind&2)&&node?NativeStress(actual,ph,node,kind&1,&actual_flags):NativeWrite(actual,ph,node,kind&1,&actual_flags);
            TEST_ASSERT(end && end-actual==length && expected_flags==actual_flags && memcmp(expected,actual,64)==0);
            TEST_ASSERT(trace==expected_trace && calls==expected_calls && count==node_count && memcmp(saved,nodes,sizeof(saved))==0);
            ++writers;
        }
        static const int modes[]={0,2,32<<8,(0x361<<8)|0x80|2,('_'<<8)|2,(0x300<<8)|2,0x80|2,-256};
        int option=modes[episode%8];unsigned value=episode*19+3;
        Reset(value);char expected[2048];strcpy(expected,RefClause(option));
        uint64_t expected_trace=trace;unsigned expected_calls=calls;int count=node_count;
        PHONEME_LIST saved[20];memcpy(saved,nodes,sizeof(saved));
        Reset(value);const char *actual=NativeClause(option);
        TEST_ASSERT(strcmp(expected,actual)==0 && trace==expected_trace && calls==expected_calls);
        TEST_ASSERT(count==node_count && memcmp(saved,nodes,sizeof(saved))==0);++clauses;
    }
    mode=0;
    for(unsigned byte=1;byte<256;byte++)for(unsigned vowel=0;vowel<2;vowel++)for(unsigned ipa=0;ipa<2;ipa++) {
        char expected[64],actual[64];int ef=0x7788,af=0x7788;
        memset(expected,0x55,64);memset(actual,0x55,64);
        Reset(7);records[0].mnemonic=byte;records[0].type=vowel?phVOWEL:phFRICATIVE;
        char *end=RefWrite(expected,&records[0],NULL,ipa,&ef);ptrdiff_t length=end-expected;
        Reset(7);records[0].mnemonic=byte;records[0].type=vowel?phVOWEL:phFRICATIVE;
        end=NativeBounded(actual,(size_t)length+1,&records[0],NULL,ipa,&af);
        TEST_ASSERT(end && end-actual==length && ef==af && memcmp(expected,actual,64)==0);++writers;
    }
    Reset(1);records[0].code=phonEND_WORD;
    char stress_end[64],native_end[64];memset(stress_end,0x55,64);memset(native_end,0x55,64);
    char *end=RefStress(stress_end,&records[0],&nodes[2],1,NULL);
    char *next=NativeStress(native_end,&records[0],&nodes[2],1,NULL);
    TEST_ASSERT(next && next-native_end==end-stress_end && memcmp(stress_end,native_end,64)==0);
    for(int count=0;count<4;count++){node_count=count;TEST_ASSERT(strcmp(RefClause(0),NativeClause(0))==0);}
    Reset(1);char out[64];memset(out,0x55,64);int flags=0x7788;
    TEST_ASSERT(NativeStressBounded(out,1,&records[0],&nodes[2],0,&flags)==NULL);
    TEST_ASSERT(flags==0x7788 && out[0]==0x55);
    fail=1;TEST_ASSERT(NativeBounded(out,64,&records[0],&nodes[2],1,&flags)==NULL);
    TEST_ASSERT(flags==0x7788 && out[0]==0x55);
    fail=0;records[0].mnemonic=0xff636261;TEST_ASSERT(NativeBounded(out,64,&records[0],NULL,0,&flags)==NULL);
    TEST_ASSERT(out[0]==0x55 && flags==0x7788);
    Reset(1);records[0].code=phonSWITCH;nodes[1].ph=&records[0];nodes[1].tone_ph=1;nodes[1].newword=0;
    node_count=4;memset(names[1].name,'a',31);names[1].name[31]=0;
    TEST_ASSERT(strlen(NativeClause(0))==33); /* Original30-byte clause scratch overflows. */
    memset(names[1].name,'a',32);TEST_ASSERT(strcmp(NativeClause(0),"")==0);
    Reset(1);mode=0;reenter=1;TEST_ASSERT(NativeClause(2)!=NULL && calls>0);
    node_count=1002;TEST_ASSERT(strcmp(NativeClause(0),"")==0);
    Reset(1);PHONEME_LIST partial; /* Only fields read by the writer are initialized. */
    partial.synthflags=0;
    TEST_ASSERT(NativeStressBounded(out,64,&records[0],&partial,0,NULL)!=NULL && strcmp(out,"ts")==0);
    records[0].code=phonSWITCH;partial.tone_ph=1;
    TEST_ASSERT(NativeBounded(out,64,&records[0],&partial,0,NULL)!=NULL && strcmp(out,"(v1)")==0);
    Reset(1);node_count=N_PHONEME_LIST+1;mode=0;
    TEST_ASSERT(NativeClause(0)!=NULL && strlen(NativeClause(0))>1000);
    Reset(1);mode=0;node_count=4;nodes[1].ph=&records[0];nodes[1].synthflags=0;
    nodes[1].newword=nodes[1].tone_ph=0;records[0].mnemonic=0x64636261;
    TEST_ASSERT(strcmp(RefClause(0),"abcd")==0 && strcmp(NativeClause(0),"abcd")==0);
    records[0].mnemonic=0x00808074;
    const char *ref_tail=RefClause(0),*native_tail=NativeClause(0);
    TEST_ASSERT(memcmp(ref_tail,native_tail,5)==0 && native_tail[0]=='t' && native_tail[1]==0 && native_tail[2]=='c' && native_tail[3]==0);
    free(phon_out_buf);
    printf("%u actual C/native writer cases and %u clause cases passed; native malformed/long-name/reentry guards pass\n",writers,clauses);
    return 0;
}
