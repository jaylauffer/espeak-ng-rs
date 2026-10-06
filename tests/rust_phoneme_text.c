/* Phoneme mnemonic text/wrappers against independently extracted retained C.
 * SPDX-License-Identifier: GPL-3.0-or-later */
#include "config.h"
#include "test_assert.h"
#include <ctype.h>
#include <limits.h>
#include <stdint.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include "translate.h"
#include "rust_data.h"
static PHONEME_TAB storage[256],*records[256];
static unsigned trace,seed=0x741398abu;
static void Stress(Translator *tr,char *phonemes,unsigned int *flags,int tonic,int control)
{
 TEST_ASSERT(tonic==-1 && control==0);trace=trace*33+(unsigned)tr->translator_name;
 if(phonemes[0])phonemes[0]=65;
 flags[0]^=1;flags[1]^=2;
}
#define phoneme_tab records
#define DecodePhonemes ReferenceDecode
#define SetWordStress Stress
#define DecodeWithPhonemeMode ReferenceWrapper
#include "phoneme_text_reference.inc"
#undef phoneme_tab
#undef DecodePhonemes
#undef SetWordStress
#undef DecodeWithPhonemeMode
static unsigned next(void){seed^=seed<<13;seed^=seed>>17;seed^=seed<<5;return seed;}
static int Alpha(uint32_t code){return isalpha((char)code)!=0;}
int main(void)
{
 size_t decoded_cases=0,wrapper_cases=0;
 for(int i=0;i<200000;i++) {
  for(int j=0;j<256;j++) {
   storage[j]=(PHONEME_TAB){.mnemonic=next(),.code=(unsigned char)j,.type=(unsigned char)(next()%4),.std_length=(unsigned char)(next()%8),.program=(unsigned short)(next()%2)};
   if(j%3==0)storage[j].mnemonic&=0x00ffffff;
   records[j]=next()%7?&storage[j]:NULL;
  }
  char input[101]={0};int length=(int)(next()%100);
  for(int j=0;j<length;j++)input[j]=next()%11==0?(char)255:(char)(1+next()%127);
  unsigned char expected[512],actual[512];memset(expected,0xa5,512);memset(actual,0xa5,512);
  ReferenceDecode(input,(char *)expected);
  int got=espeak_rs_decode_phonemes((unsigned char *)input,(size_t)length+1,(const PHONEME_TAB *const *)records,Alpha,CHAR_MIN<0,actual,512);
  TEST_ASSERT(got==(int)strlen((char *)expected));TEST_ASSERT(memcmp(actual,expected,512)==0);decoded_cases++;
  memset(actual,0xa5,512);
  TEST_ASSERT(espeak_rs_decode_phonemes_legacy(input,(const PHONEME_TAB *const *)records,Alpha,CHAR_MIN<0,actual)==got);
  TEST_ASSERT(memcmp(actual,expected,512)==0);
  int capacity=got+(int)(next()%2);memset(actual,0xa5,512);
  int bounded=espeak_rs_decode_phonemes((unsigned char *)input,(size_t)length+1,(const PHONEME_TAB *const *)records,Alpha,CHAR_MIN<0,actual,(size_t)capacity);
  if(capacity>=3 && capacity>got) {TEST_ASSERT(bounded==got && memcmp(actual,expected,512)==0);}
  else {TEST_ASSERT(bounded==-1);for(int j=0;j<512;j++)TEST_ASSERT(actual[j]==0xa5);}
  Translator primary={0},secondary={0};primary.translator_name=next();secondary.translator_name=0x656e;
  char original[9]={0},phonemes[9]={0};length=(int)(next()%8);for(int j=0;j<length;j++)original[j]=(char)(1+next()%127);
  memcpy(phonemes,original,sizeof(phonemes));unsigned int flags[2]={next(),next()},initial_flags[2];memcpy(initial_flags,flags,sizeof(flags));
  int alternate=(int)(next()%2);trace=0;memset(expected,0xa5,512);memset(actual,0xa5,512);
  ReferenceWrapper((char *)expected,phonemes,&primary,alternate?&secondary:NULL,flags,74);
  unsigned expected_trace=trace;unsigned int expected_flags[2];memcpy(expected_flags,flags,sizeof(flags));
  memcpy(phonemes,original,sizeof(phonemes));memcpy(flags,initial_flags,sizeof(flags));trace=0;
  Stress(alternate?&secondary:&primary,phonemes,flags,-1,0);
  char text[55];TEST_ASSERT(espeak_rs_decode_phonemes((unsigned char *)phonemes,strlen(phonemes)+1,(const PHONEME_TAB *const *)records,Alpha,CHAR_MIN<0,(unsigned char *)text,55)>=0);
  int wrapped=espeak_rs_clause_phoneme_wrapper(text,alternate?ESPEAKNG_DEFAULT_VOICE:NULL,(unsigned)primary.translator_name,actual,74);
  TEST_ASSERT(wrapped==(int)strlen((char *)expected) && memcmp(actual,expected,512)==0);
  TEST_ASSERT(trace==expected_trace && memcmp(flags,expected_flags,sizeof(flags))==0);wrapper_cases++;
 }
 unsigned char out[80];memset(out,0xa5,80);
 TEST_ASSERT(espeak_rs_clause_phoneme_wrapper("text",NULL,0,out,8)==-1);
 for(int i=0;i<80;i++)TEST_ASSERT(out[i]==0xa5);
 char unterminated[2]={65,66};TEST_ASSERT(espeak_rs_decode_phonemes((unsigned char *)unterminated,2,(const PHONEME_TAB *const *)records,Alpha,CHAR_MIN<0,out,80)==-1);
 if(CHAR_MIN<0) {
  records[21]=&storage[21];storage[21].type=2;
  unsigned char invalid[]={21,128,0};TEST_ASSERT(espeak_rs_decode_phonemes(invalid,3,(const PHONEME_TAB *const *)records,Alpha,1,out,80)==-1);
 }
 for(int i=0;i<80;i++)TEST_ASSERT(out[i]==0xa5);
 printf("Matched %zu phoneme text output/tail comparisons and %zu clause wrapper/stress-order comparisons; capacity/domain guards preserved output\n",decoded_cases,wrapper_cases);
 return 0;
}
