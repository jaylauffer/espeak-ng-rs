/* Whole unchanged previous-native synthdata host, with real resident resources.
 * SPDX-License-Identifier: GPL-3.0-or-later */
#include "config.h"
#include "test_assert.h"
#include "rust_data.h"
#include "synthdata.h"
#include "speech.h"
#include <espeak-ng/speak_lib.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
extern int reference_n_tunes,reference_n_phoneme_tab,reference_phoneme_tab_number;
extern TUNE *reference_tunes;
extern PHONEME_TAB *reference_phoneme_tab[256];
extern PHONEME_TAB_LIST reference_phoneme_tab_list[150];
extern unsigned char *reference_wavefile_data;
extern int seq_len_adjust,reference_seq_len_adjust;
void reference_SelectPhonemeTable(int);
int reference_LookupPhonemeTable(const char *);
int reference_SelectPhonemeTableName(const char *);
int reference_PhonemeCode(unsigned int);
int reference_LookupPhonemeString(const char *);
size_t reference_RustPhonemeDataLength(void);
const unsigned char *reference_RustPhonemePrograms(size_t *);
void reference_InterpretPhonemeWithLength(Translator *,int,PHONEME_LIST *,PHONEME_LIST *,PHONEME_DATA *,WORD_PH_DATA *,size_t);
void reference_InterpretPhoneme2WithData(int,PHONEME_TAB *,PHONEME_DATA *);
PHONEME_TAB *reference_TonePhoneme(const PHONEME_LIST *);
frameref_t *reference_LookupSpect(PHONEME_TAB *,int,FMT_PARAMS *,int *,PHONEME_LIST *);
const unsigned char *reference_GetEnvelope(int);
/* The previous host's scalar is independent. This unchanged native primitive
 * reads the product scalar; fence that alias exactly around its call. */
int reference_FormantTransitionWithCapacity(frameref_t *frames,int *count,int a,int b,PHONEME_TAB *other,int which,size_t capacity)
{
 int saved=seq_len_adjust;seq_len_adjust=reference_seq_len_adjust;
 int added=FormantTransitionWithCapacity(frames,count,a,b,other,which,capacity);
 reference_seq_len_adjust=seq_len_adjust;seq_len_adjust=saved;return added;
}
static unsigned seed=0x3124ab79;
static unsigned next(void){seed^=seed<<13;seed^=seed>>17;seed^=seed<<5;return seed;}
static unsigned programs,isolated,spectra,tables;
static void Table(int number)
{
 /* Force both projection callbacks without reloading/releasing buffers. Each
  * view is freshly published while the same retained resources stay live. */
 reference_phoneme_tab_number=phoneme_tab_number;
 reference_SelectPhonemeTable(-1);reference_SelectPhonemeTable(number);
 SelectPhonemeTable(-1);SelectPhonemeTable(number);
 TEST_ASSERT(reference_n_phoneme_tab==n_phoneme_tab&&reference_n_tunes==n_tunes);
 TEST_ASSERT(reference_tunes==tunes&&reference_wavefile_data==wavefile_data);
 TEST_ASSERT(!memcmp(reference_phoneme_tab,phoneme_tab,sizeof(reference_phoneme_tab)));
 TEST_ASSERT(!memcmp(reference_phoneme_tab_list,phoneme_tab_list,sizeof(reference_phoneme_tab_list)));
 size_t a,b;TEST_ASSERT(reference_RustPhonemePrograms(&a)==RustPhonemePrograms(&b)&&a==b);
 TEST_ASSERT(reference_RustPhonemeDataLength()==RustPhonemeDataLength());tables++;
}
static void Spect(PHONEME_TAB *ph,PHONEME_LIST *entry,int which,FMT_PARAMS *parameters)
{
 int n=0,m=0;frameref_t saved[N_SEQ_FRAMES];unsigned char bytes[N_SEQ_FRAMES][64];
 frameref_t *old=reference_LookupSpect(ph,which,parameters,&n,entry);
 int adjust=reference_seq_len_adjust;
 if(old){memcpy(saved,old,(size_t)n*sizeof(*saved));for(int i=0;i<n;i++)memcpy(bytes[i],old[i].frame,old[i].frame->frflags&FRFLAG_KLATT?64:44);}
 frameref_t *current=LookupSpect(ph,which,parameters,&m,entry);
 TEST_ASSERT((old==NULL)==(current==NULL)&&m==n);
 if(!old)return;
 TEST_ASSERT(seq_len_adjust==adjust);
 for(int i=0;i<n;i++){TEST_ASSERT(current[i].length==saved[i].length&&current[i].frflags==saved[i].frflags);TEST_ASSERT(!memcmp(bytes[i],current[i].frame,current[i].frame->frflags&FRFLAG_KLATT?64:44));}
 spectra++;
}
int main(void)
{
 TEST_ASSERT(espeak_Initialize(AUDIO_OUTPUT_RETRIEVAL,0,NULL,0)==22050);
 TEST_ASSERT(espeak_SetVoiceByName("en")==EE_OK);
 Translator language=*translator;
 for(int table=0;table<150&&phoneme_tab_list[table].n_phonemes>0;table++){
  Table(table);
  const char *name=phoneme_tab_list[table].name;
  reference_phoneme_tab_number=phoneme_tab_number;
  TEST_ASSERT(reference_LookupPhonemeTable(name)==LookupPhonemeTable(name));
  TEST_ASSERT(reference_phoneme_tab_number==phoneme_tab_number);
  reference_phoneme_tab_number=phoneme_tab_number;
  int selected=reference_SelectPhonemeTableName(name);
  /* Both adapters share one asset pool; a cached selection does not publish.
   * Revoke the product selection before its independent name operation. */
  SelectPhonemeTable(-1);
  TEST_ASSERT(selected==SelectPhonemeTableName(name));
  TEST_ASSERT(!memcmp(reference_phoneme_tab,phoneme_tab,sizeof(reference_phoneme_tab)));
  for(int code=0;code<256;code++){
   PHONEME_TAB *ph=phoneme_tab[code];if(!ph)continue;
   TEST_ASSERT(reference_PhonemeCode(ph->mnemonic)==PhonemeCode(ph->mnemonic));
   char mnemonic[5];for(int i=0;i<4;i++)mnemonic[i]=(char)(ph->mnemonic>>(i*8));mnemonic[4]=0;
   TEST_ASSERT(reference_LookupPhonemeString(mnemonic)==LookupPhonemeString(mnemonic));
   for(unsigned trial=0;trial<4;trial++){
    PHONEME_LIST old[9]={0},current[9];WORD_PH_DATA word={0},newword;PHONEME_DATA a,b;
    for(int i=0;i<9;i++){old[i].ph=phoneme_tab[phonPAUSE];old[i].phcode=phonPAUSE;old[i].type=phPAUSE;old[i].sourceix=i==0||i>=6;old[i].stresslevel=next()%8;old[i].wordstress=next()%8;old[i].synthflags=next()&0x7f;old[i].std_length=47;old[i].phontab_addr=0xabcdef;old[i].sound_param=-17;}
    old[3].ph=ph;old[3].phcode=(unsigned char)code;old[3].type=ph->type;
    old[3].sourceix=trial&1;word.prev_vowel=old[3];newword=word;
    memcpy(current,old,sizeof(old));language.langopts.param[LOPT_REDUCE]=trial;
    Translator *tr=trial&1?&language:NULL;int control=(trial&2?0x100:0)|(trial&1);
    reference_InterpretPhonemeWithLength(tr,control,old+3,old,&a,&word,9);
    InterpretPhonemeWithLength(tr,control,current+3,current,&b,&newword,9);
    TEST_ASSERT(!memcmp(&a,&b,sizeof(a))&&!memcmp(old,current,sizeof(old))&&!memcmp(&word,&newword,sizeof(word)));programs++;
   }
   PHONEME_DATA a,b;reference_InterpretPhoneme2WithData(code,ph,&a);InterpretPhoneme2WithData(code,ph,&b);
   TEST_ASSERT(!memcmp(&a,&b,sizeof(a)));isolated++;
   PHONEME_LIST tone={0};tone.tone_ph=(unsigned char)code;
   TEST_ASSERT(reference_TonePhoneme(&tone)==TonePhoneme(&tone));tone.tone_ph_data=ph;tone.tone_ph=0;
   TEST_ASSERT(reference_TonePhoneme(&tone)==TonePhoneme(&tone));
   if(a.sound_addr[0]>0){FMT_PARAMS parameters={0};parameters.fmt_addr=a.sound_addr[0];parameters.std_length=ph->std_length*2;
    for(int which=0;which<3;which++){parameters.fmt_length=which*7;parameters.use_vowelin=which==1;parameters.transition0=25|(16<<6)|(2<<12);parameters.transition1=30;PHONEME_LIST row={0};row.synthflags=SFLAG_LENGTHEN;Spect(ph,&row,which,&parameters);}}
   if(a.pitch_env>0)TEST_ASSERT(reference_GetEnvelope(a.pitch_env)==GetEnvelope(a.pitch_env));
  }
 }
 Table(0);TEST_ASSERT(LookupPhonemeTable("__missing_table__")==-1&&reference_LookupPhonemeTable("__missing_table__")==-1);
 PHONEME_DATA data;memset(&data,0xa5,sizeof(data));InterpretPhoneme2WithData(7,NULL,&data);
 unsigned char zero[sizeof(data)]={0};TEST_ASSERT(!memcmp(&data,zero,sizeof(data)));
 PHONEME_LIST invalid[2]={0};memset(&data,0xa5,sizeof(data));InterpretPhonemeWithLength(NULL,0,invalid+1,invalid,&data,NULL,1);
 for(size_t i=0;i<sizeof(data);i++)TEST_ASSERT(((unsigned char *)&data)[i]==0xa5);
 TEST_ASSERT(LookupPhonemeString(NULL)==0&&TonePhoneme(NULL)==NULL&&RustPhonemePrograms(NULL)==NULL);
 FreePhData();TEST_ASSERT(wavefile_data==NULL&&tunes==NULL&&n_tunes==0&&n_phoneme_tab==0&&RustPhonemeDataLength()==0);
 espeak_Terminate();
 printf("whole previous-native synthdata host: %u table projections, %u clause VM calls, %u isolated programs, %u spectra; mnemonic/tone/envelope and admission/release guards match\n",tables,programs,isolated,spectra);
 return 0;
}
