/* Common copy/random/stream primitives against independently extracted C.
 * SPDX-License-Identifier: GPL-3.0-or-later */
#include "config.h"
#include "test_assert.h"
#include <limits.h>
#include <stdint.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include "common.h"
#include "rust_data.h"
#define strncpy0 ReferenceCopy
#define Read4Bytes ReferenceRead4
#define espeak_rand ReferenceRandom
#define espeak_srand ReferenceSeed
#include "common_primitives_reference.inc"
#undef strncpy0
#undef Read4Bytes
#undef espeak_rand
#undef espeak_srand
static unsigned seed=0x732ea519u;
static unsigned next(void){seed^=seed<<13;seed^=seed>>17;seed^=seed<<5;return seed;}
int main(void)
{
 for(int episode=0;episode<2000;episode++){
  long value=(long)(int32_t)next();ReferenceSeed(value);espeak_srand(value);
  for(int i=0;i<1000;i++){
   long min=(long)(int)(next()%100000)-50000;
   long max=min+(long)(next()%200000)-100000;
   if(max==min-1)max++;
   TEST_ASSERT(espeak_rand(min,max)==ReferenceRandom(min,max));
  }
 }
 ReferenceSeed(7);espeak_srand(7);
 TEST_ASSERT(espeak_rand(1,0)==0);TEST_ASSERT(espeak_rand(LONG_MIN,LONG_MAX)==0);
 TEST_ASSERT(espeak_rand(-8192,8191)==ReferenceRandom(-8192,8191));
 for(int i=0;i<200000;i++){
  unsigned char input[512],actual[512],expected[512];
  int size=1+(int)(next()%512),length=(int)(next()%512);
  for(int j=0;j<512;j++)input[j]=(unsigned char)(1+next()%255);input[length]=0;
  memset(actual,0xa5,512);memset(expected,0xa5,512);
  ReferenceCopy((char *)expected,(const char *)input,size);
  strncpy0((char *)actual,(const char *)input,size);
  TEST_ASSERT(memcmp(actual,expected,512)==0);
 }
 unsigned char guard[8];memset(guard,0xa5,8);
 TEST_ASSERT(espeak_rs_copy0(guard,(const unsigned char *)"a",0)==-1);TEST_ASSERT(guard[0]==0xa5);
 FILE *stream=tmpfile();TEST_ASSERT(stream!=NULL);
 for(int i=0;i<20000;i++){
  unsigned char bytes[4];for(int j=0;j<4;j++)bytes[j]=(unsigned char)next();bytes[3]&=127;
  rewind(stream);TEST_ASSERT(fwrite(bytes,1,4,stream)==4);rewind(stream);int expected=ReferenceRead4(stream);
  TEST_ASSERT(ftell(stream)==4);rewind(stream);TEST_ASSERT(Read4Bytes(stream)==expected);TEST_ASSERT(ftell(stream)==4);
 }
 TEST_ASSERT(Read4Bytes(stream)==-1 && feof(stream));fclose(stream);
 stream=tmpfile();TEST_ASSERT(stream!=NULL);fputc(0x12,stream);rewind(stream);
 TEST_ASSERT(Read4Bytes(stream)==(int)0xffffff12u && feof(stream));fclose(stream);
 fprintf(stderr,"2000000 random outputs, 200000 copies and 20000 stdio words match retained C; EOF/guard cases pass\n");
 return 0;
}
