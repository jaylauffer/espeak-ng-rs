/* Whole unchanged previous-native synthesis host, real voice/program data.
 * SPDX-License-Identifier: GPL-3.0-or-later */
#include "config.h"
#include "test_assert.h"
#include "rust_data.h"
#include "synthdata.h"
#include "wavegen.h"
#include "translate.h"
#include "intonation.h"
#include "setlengths.h"
#include <errno.h>
#include <espeak-ng/speak_lib.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
int previous_Generate(PHONEME_LIST *,int *,bool);
void previous_SynthesizeInit(void);
int previous_PauseLength(int,int);
void previous_DoMarker(int,int,int,int);
void previous_DoPhonemeMarker(int,int,int,char *);
espeak_ng_STATUS previous_DoVoiceChange(voice_t *);
const char *previous_WordToString(char *,unsigned int);
int previous_FormantTransitionWithCapacity(frameref_t *,int *,unsigned int,unsigned int,PHONEME_TAB *,int,size_t);
int previous_SpeakNextClause(int);
void previous_SetPhonemeCallback(int (*)(const char *));
void reference_SelectPhonemeTable(int);
extern int reference_phoneme_tab_number,reference_seq_len_adjust,seq_len_adjust;
/* The previous spectrum host calls its own synthesis transition so modulation
 * and deferred pauses reach the previous command state, never product state. */
int reference_FormantTransitionWithCapacity(frameref_t *s,int *n,int a,int b,PHONEME_TAB *p,int w,size_t c)
{return previous_FormantTransitionWithCapacity(s,n,(unsigned)a,(unsigned)b,p,w,c);}
typedef struct { intptr_t words[4]; unsigned char frames[2][64]; } Command;
#define MAX_COMMANDS 10000
static Command output[2][MAX_COMMANDS];
static unsigned commands[2],suspensions[2],episodes;
static PHONEME_LIST rows[N_PHONEME_LIST+1],copies[2][N_PHONEME_LIST+1];
static SPEED_FACTORS factors;
static int embedded[N_EMBEDDED_VALUES];
static void collect(unsigned side)
{
 for(int index=wcmdq_head;index!=wcmdq_tail;index=(index+1)%N_WCMDQ){
  TEST_ASSERT(commands[side]<MAX_COMMANDS);
  Command *out=&output[side][commands[side]++];memset(out,0,sizeof(*out));
  memcpy(out->words,wcmdq[index],sizeof(out->words));
  /* Writers intentionally retain unused words in reused queue slots. */
  int op=(int)(out->words[0]&255);
  if(op==5||op==14||op==15)out->words[2]=out->words[3]=0;
  if(op==10||op==12)out->words[3]=0;
  if(op==11)out->words[1]=out->words[3]=0;
  if(out->words[0]==(10+(7<<8))){
   /* Previous C copied eight bytes from a partly initialized local string.
    * Compare the specified NUL-terminated name, excluding unused stack tail. */
   unsigned char *name=(unsigned char *)&out->words[2];
   for(unsigned i=0;i<8;i++)if(!name[i]){memset(name+i,0,8-i);break;}
  }
  if(out->words[0]>=1&&out->words[0]<=4){
   for(int frame=0;frame<2;frame++){
    frame_t *p=(frame_t *)out->words[frame+2];
    if(p){size_t length=(p->frflags&1)?64:44;memcpy(out->frames[frame],p,length);out->words[frame+2]=1;}
   }
  }
  if(out->words[0]==11){ /* queued voice ownership, copied before freeing */
   TEST_ASSERT(out->words[2]);free((void *)out->words[2]);out->words[2]=0;
  }
 }
 /* Consumer admission: all queued work consumed; next batch can use the ring. */
 wcmdq_head=wcmdq_tail;
}
static void run(unsigned side,int count)
{
 memcpy(copies[side],rows,sizeof(rows));speed=factors;memcpy(embedded_value,embedded,sizeof(embedded));
 memset(&espeak_rs_wave_memory,0,sizeof(espeak_rs_wave_memory));
 memset(&espeak_rs_frame_pool,0,sizeof(espeak_rs_frame_pool));
 seq_len_adjust=reference_seq_len_adjust=0;
 if(side)previous_SynthesizeInit();else SynthesizeInit();
 commands[side]=suspensions[side]=0;
 /* First call writes the historical isolating pause, then must suspend. */
 wcmdq_head=0;wcmdq_tail=N_WCMDQ-2;
 int n=count;
 int status=side?previous_Generate(copies[side],&n,false):Generate(copies[side],&n,false);
 if(status!=1||n!=count||wcmdq_tail!=N_WCMDQ-1)fprintf(stderr,"first admission side=%u count=%d n=%d rc=%d tail=%d\n",side,count,n,status,wcmdq_tail);
 TEST_ASSERT(status==1&&n==count&&wcmdq_tail==N_WCMDQ-1);
 wcmdq_head=N_WCMDQ-2;collect(side);
 for(unsigned turn=0;;turn++){
  TEST_ASSERT(turn<1000);
  status=side?previous_Generate(copies[side],&n,true):Generate(copies[side],&n,true);
  collect(side);
  if(!status){TEST_ASSERT(n==0);break;}
  suspensions[side]++;
 }
}
static int callbacks,callback_mode;
static int callback(const char *output)
{
 TEST_ASSERT(output&&*output);callbacks++;
 if(callback_mode){TEST_ASSERT(SpeakNextClause(2)==0);espeak_SetPhonemeCallback(NULL);}
 return 0;
}
int main(void)
{
 TEST_ASSERT(espeak_Initialize(AUDIO_OUTPUT_SYNCHRONOUS,0,NULL,0)==22050);
 if(!p_decoder)p_decoder=create_text_decoder();TEST_ASSERT(p_decoder);
 static const char *voices[]={"en","ru","pl","fi","fr","de","cmn","vi","en+klatt6","en+adam","en+mike2","fr+david"};
 static const char *texts[]={
  "One short sentence.","First sentence. Second words follow!",
  "The longer sentence contains consonants, vowels, numbers 12345 and several more words to exercise suspension and resumed generation.",
  "A repeated phrase, a repeated phrase, a repeated phrase, a repeated phrase, a repeated phrase, a repeated phrase, a repeated phrase, a repeated phrase, a repeated phrase, a repeated phrase, a repeated phrase, a repeated phrase.",
  "First \001200Sword \001100Ssecond \00150Pword.",
 };
 for(unsigned trial=0;trial<120;trial++){
  TEST_ASSERT(espeak_SetVoiceByName(voices[trial%12])==EE_OK);
  /* SetVoiceByName queues a copy of the voice for the generator, which this test does not run: free it (WcmdqStop does) before the queue is reset. */
  WcmdqStop();
  SelectPhonemeTable(translator->phoneme_tab_ix);
  const char *text=texts[(trial/12)%5];
  TEST_ASSERT(text_decoder_decode_string(p_decoder,text,-1,ESPEAKNG_ENCODING_UTF_8)==ENS_OK);
  int tone;char *change;TranslateClause(translator,&tone,&change);CalcPitches(translator,tone);CalcLengths(translator);
  int table=phoneme_tab_number;
  reference_phoneme_tab_number=table;reference_SelectPhonemeTable(-1);reference_SelectPhonemeTable(table);
  SelectPhonemeTable(-1);SelectPhonemeTable(table);
  TEST_ASSERT(n_phoneme_list>2);memcpy(rows,phoneme_list,sizeof(rows));
  factors=speed;memcpy(embedded,embedded_value,sizeof(embedded));
  option_phoneme_events=trial%3;count_characters=400+trial;clause_start_char=13;clause_start_word=7;count_sentences=3;
  int count=n_phoneme_list;run(0,count);run(1,count);
  if(commands[0]!=commands[1]||suspensions[0]!=suspensions[1]||
   memcmp(output[0],output[1],commands[0]*sizeof(Command))||memcmp(copies[0],copies[1],sizeof(rows))){
   fprintf(stderr,"whole synthesis host mismatch trial=%u commands=%u/%u waits=%u/%u\n",trial,commands[0],commands[1],suspensions[0],suspensions[1]);
   for(unsigned i=0;i<commands[0]&&i<commands[1];i++)if(memcmp(&output[0][i],&output[1][i],sizeof(Command))){for(int j=0;j<4;j++)fprintf(stderr," word%d=%lx/%lx",j,(unsigned long)output[0][i].words[j],(unsigned long)output[1][i].words[j]);fprintf(stderr,"\n");break;}
   for(unsigned i=0;i<commands[0]&&i<commands[1];i++)if(memcmp(&output[0][i],&output[1][i],sizeof(Command))){fprintf(stderr,"first command mismatch %u op=%ld/%ld\n",i,(long)output[0][i].words[0],(long)output[1][i].words[0]);break;}
   TEST_ASSERT(false);
  }
  episodes++;
 }
 unsigned pauses=0;
 for(int value=0;value<1000;value++)for(int mode=0;mode<4;mode++){TEST_ASSERT(PauseLength(value,mode)==previous_PauseLength(value,mode));pauses++;}
 for(unsigned value=0;value<10000;value++){char a[5],b[5];TEST_ASSERT(WordToString(a,value*654987)==a);TEST_ASSERT(previous_WordToString(b,value*654987)==b);TEST_ASSERT(!memcmp(a,b,5));}
 TEST_ASSERT(WordToString(NULL,0)==NULL);TEST_ASSERT(DoVoiceChange(NULL)==EINVAL);
 int invalid=1002;TEST_ASSERT(Generate(rows,&invalid,false)==0&&invalid==0);TEST_ASSERT(Generate(NULL,&invalid,false)==0);
 for(unsigned side=0;side<2;side++){
  memset(&espeak_rs_wave_memory,0,sizeof(espeak_rs_wave_memory));
  char marker[8]={'p','h','o','n','e','m','e',0};
  if(side){previous_DoMarker(1,123,5,17);previous_DoPhonemeMarker(7,234,0,marker);TEST_ASSERT(previous_DoVoiceChange(voice)==ENS_OK);}
  else{DoMarker(1,123,5,17);DoPhonemeMarker(7,234,0,marker);TEST_ASSERT(DoVoiceChange(voice)==ENS_OK);}
  TEST_ASSERT(wcmdq_tail==3&&wcmdq[2][0]==11);TEST_ASSERT(!memcmp((void *)wcmdq[2][2],voice,sizeof(*voice)));
  commands[side]=0;collect(side);
 }
 TEST_ASSERT(commands[0]==commands[1]&&!memcmp(output[0],output[1],commands[0]*sizeof(Command)));
 for(unsigned side=0;side<2;side++){
  TEST_ASSERT(espeak_SetVoiceByName("en")==EE_OK);callbacks=0;callback_mode=0;
  if(side)previous_SetPhonemeCallback(callback);else espeak_SetPhonemeCallback(callback);
  TEST_ASSERT(text_decoder_decode_string(p_decoder,"Callback words.",-1,ESPEAKNG_ENCODING_UTF_8)==ENS_OK);
  TEST_ASSERT((side?previous_SpeakNextClause(0):SpeakNextClause(0))==1&&callbacks==1);
  /* The previous host's stop empties the queue without freeing voice copies. */
  if(side)WcmdqStop();
  TEST_ASSERT((side?previous_SpeakNextClause(2):SpeakNextClause(2))==0&&n_phoneme_list==0);
  if(side)previous_SetPhonemeCallback(NULL);else espeak_SetPhonemeCallback(NULL);
 }
 callbacks=0;callback_mode=1;espeak_SetPhonemeCallback(callback);
 TEST_ASSERT(text_decoder_decode_string(p_decoder,"Reentry words.",-1,ESPEAKNG_ENCODING_UTF_8)==ENS_OK);
 TEST_ASSERT(SpeakNextClause(0)==1&&callbacks==1);TEST_ASSERT(SpeakNextClause(2)==0);
 espeak_Terminate();
 printf("%u whole previous-native synthesis episodes, %u pause cases, 10000 mnemonics; real queues/frames/resumption/markers/voices/callbacks and native admission/reentry pass\n",episodes,pauses);
 return 0;
}
