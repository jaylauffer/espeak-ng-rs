/* Complete previous-native C hosts versus Rust, using actual resident data. */
/* SPDX-License-Identifier: GPL-3.0-or-later */
#include "config.h"
#include "test_assert.h"
#include "rust_clause_postprocess.h"
#include "rust_data.h"
#include "phonemelist.h"
#include "intonation.h"
#include "setlengths.h"
#include "synthdata.h"
#include "speech.h"
#include "wavegen.h"
#include <espeak-ng/speak_lib.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <errno.h>
extern int n_ph_list2;
extern PHONEME_LIST2 ph_list2[N_PHONEME_LIST];
void reference_MakePhonemeList(Translator *,int,bool);
void reference_CalcPitches(Translator *,int);
void reference_CalcLengths(Translator *);
void reference_SetSpeed(int);
int reference_SetParameter(int,int,int);
void reference_SetLengthMods(Translator *,int);
static unsigned seed=0x971b26du;
static unsigned next(void){seed^=seed<<13;seed^=seed>>17;seed^=seed<<5;return seed;}
typedef struct {
 PHONEME_LIST rows[N_PHONEME_LIST+1]; PHONEME_LIST2 source[N_PHONEME_LIST];
 RustWaveMemory wave; SPEED_FACTORS factors; int embedded[N_EMBEDDED_VALUES]; int count,source_count;
} State;
static State base,expected;
static void save(State *s){
 memcpy(s->rows,phoneme_list,sizeof(s->rows));memcpy(s->source,ph_list2,sizeof(s->source));
 memcpy(&s->wave,&espeak_rs_wave_memory,sizeof(s->wave));s->factors=speed;
 memcpy(s->embedded,embedded_value,sizeof(s->embedded));s->count=n_phoneme_list;s->source_count=n_ph_list2;
}
static void restore(const State *s){
 memcpy(phoneme_list,s->rows,sizeof(s->rows));memcpy(ph_list2,s->source,sizeof(s->source));
 memcpy(&espeak_rs_wave_memory,&s->wave,sizeof(s->wave));speed=s->factors;
 memcpy(embedded_value,s->embedded,sizeof(s->embedded));n_phoneme_list=s->count;n_ph_list2=s->source_count;
}
static void compare(const State *s,unsigned trial,const char *stage){
 if(memcmp(phoneme_list,s->rows,sizeof(s->rows))||memcmp(ph_list2,s->source,sizeof(s->source))||
  memcmp(&espeak_rs_wave_memory,&s->wave,sizeof(s->wave))||memcmp(&speed,&s->factors,sizeof(speed))||
  memcmp(embedded_value,s->embedded,sizeof(s->embedded))||n_phoneme_list!=s->count||n_ph_list2!=s->source_count){
  fprintf(stderr,"previous-native host mismatch: trial=%u stage=%s count=%d/%d\n",trial,stage,n_phoneme_list,s->count);
  TEST_ASSERT(false);
 }
}
int main(void){
 TEST_ASSERT(espeak_Initialize(AUDIO_OUTPUT_SYNCHRONOUS,0,NULL,0)==22050);
 static const char *voices[]={"en","ru","pl","fi","fr","de","cmn","vi","en+klatt6","en+adam","en+mike2","fr+david"};
 unsigned lists=0,pitches=0,lengths=0,empty=0;
 for(unsigned trial=0;trial<1024;trial++){
  TEST_ASSERT(espeak_SetVoiceByName(voices[trial%12])==EE_OK);
  /* SetVoiceByName queues a copy of the voice for the generator, which this test does not run: free it (WcmdqStop does) before the queue is reset. */
  WcmdqStop();
  SelectPhonemeTable(translator->phoneme_tab_ix);
  unsigned char codes[256];unsigned nc=0;
  for(int i=1;i<n_phoneme_tab;i++)if(phoneme_tab[i]&&i!=phonSWITCH)codes[nc++]=(unsigned char)i;
  TEST_ASSERT(nc>0);
  memset(phoneme_list,0,sizeof(base.rows));
  for(unsigned i=0;i<N_PHONEME_LIST+1;i++){phoneme_list[i].amp=42;phoneme_list[i].std_length=71;phoneme_list[i].phontab_addr=0xabcdef;phoneme_list[i].sound_param=-13;}
  memset(ph_list2,0,sizeof(base.source));
  int count=trial%16==0?N_PHONEME_LIST:3+(int)(next()%80);
  for(int i=0;i<count;i++){
   PHONEME_LIST2 *p=&ph_list2[i];p->phcode=codes[next()%nc];p->stresslevel=(unsigned char)(next()%8);
   p->sourceix=next()%4==0?(unsigned short)(1+next()%500):0;p->wordstress=(unsigned char)(next()%8);
   p->synthflags=(unsigned short)(next()&0x7f);p->tone_ph=next()%8==0?codes[next()%nc]:0;
  }
  ph_list2[0].phcode=phonPAUSE;
  for(int i=count-2;i<count;i++){ph_list2[i].phcode=phonPAUSE;ph_list2[i].synthflags=0;ph_list2[i].stresslevel=0;}
  for(unsigned i=0;i<N_EMBEDDED_LIST;i++)embedded_list[i]=(80+next()%350)<<8|EMBED_S|0x80;
  n_ph_list2=count;n_phoneme_list=0;n_replace_phonemes=0;
  embedded_value[EMBED_S]=embedded_value[EMBED_S2]=80+(int)(next()%370);
  int pause=(int)(next()%200),clause=(int)(next()%6);bool sentence=next()%2!=0;
  save(&base);reference_MakePhonemeList(translator,pause,sentence);save(&expected);
  restore(&base);MakePhonemeList(translator,pause,sentence);compare(&expected,trial,"list");lists++;
  if(n_phoneme_list==0){empty++;continue;}
  save(&base);reference_CalcPitches(translator,clause);save(&expected);
  restore(&base);CalcPitches(translator,clause);compare(&expected,trial,"pitch");pitches++;
  save(&base);reference_SetSpeed(3);reference_CalcLengths(translator);save(&expected);
  restore(&base);SetSpeed(3);CalcLengths(translator);compare(&expected,trial,"length/speed");lengths++;
 }
 TEST_ASSERT(espeak_rs_postprocess_list(NULL,0,false)==EINVAL);
 TEST_ASSERT(espeak_rs_postprocess_pitch(NULL,0)==EINVAL);
 TEST_ASSERT(espeak_rs_postprocess_lengths(NULL)==EINVAL);
 n_phoneme_list=0;TEST_ASSERT(espeak_rs_postprocess_pitch(translator,0)==0);
 TEST_ASSERT(espeak_rs_postprocess_lengths(translator)==0);
 TUNE *saved_tunes=tunes;int saved_n_tunes=n_tunes;
 tunes=NULL;n_tunes=17;n_phoneme_list=1;memset(&phoneme_list[0],0,sizeof(phoneme_list[0]));
 save(&base);reference_CalcPitches(translator,0);save(&expected);restore(&base);
 TEST_ASSERT(espeak_rs_postprocess_pitch(translator,0)==0);compare(&expected,1024,"null-tunes count ignored");
 n_tunes=-1;TEST_ASSERT(espeak_rs_postprocess_pitch(translator,0)==0);
 tunes=saved_tunes;n_tunes=saved_n_tunes;
 n_ph_list2=N_PHONEME_LIST+1;n_phoneme_list=77;
 TEST_ASSERT(espeak_rs_postprocess_list(translator,0,false)==0&&n_phoneme_list==0);
 const unsigned char *first=translator->langopts.length_mods,*last=translator->langopts.length_mods0;
 for(int mode=0;mode<6;mode++){
  reference_SetLengthMods(translator,mode);const unsigned char *a=translator->langopts.length_mods,*b=translator->langopts.length_mods0;
  translator->langopts.length_mods=first;translator->langopts.length_mods0=last;
  SetLengthMods(translator,mode);TEST_ASSERT(a==translator->langopts.length_mods&&b==translator->langopts.length_mods0);
 }
 espeak_Terminate();
 printf("1024 complete previous-native clause host programs match: %u list, %u pitch, %u length/rate calls, %u empty/refused clauses; initialized full rows/source/queue/speed/embedded and admission/modifier guards\n",lists,pitches,lengths,empty);
 return 0;
}
