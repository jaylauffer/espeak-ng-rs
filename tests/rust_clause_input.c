/* Clause input and legacy UTF8 against independently extracted retained C.
 * SPDX-License-Identifier: GPL-3.0-or-later */
#include "config.h"
#include "test_assert.h"
#include <limits.h>
#include <stdint.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <wchar.h>
#include <ucd/ucd.h>
#include <espeak-ng/espeak_ng.h>
#include <espeak-ng/encoding.h>
#include "translate.h"
#include "rust_data.h"
static uint64_t override_properties;
static int override_enabled;
static ucd_property Properties(codepoint_t c,ucd_category cat)
{
 return override_enabled?override_properties:ucd_properties(c,cat);
}
static wchar_t source[100];
static int source_position,source_length,reference_pending,reference_count;
static espeak_ng_TEXT_DECODER *reference_decoder;
static int SourceEof(espeak_ng_TEXT_DECODER *unused){(void)unused;return source_position>=source_length;}
static codepoint_t SourceRead(espeak_ng_TEXT_DECODER *unused){(void)unused;return source_position<source_length?(uint32_t)source[source_position++]:0;}
#define ucd_properties Properties
#define clause_type_from_codepoint ReferenceType
#define Eof ReferenceEof
#define GetC ReferenceGet
#define ungot_char reference_pending
#define count_characters reference_count
#define p_decoder reference_decoder
#define text_decoder_eof SourceEof
#define text_decoder_getc SourceRead
#define WordToString2 ReferenceWord
#define utf8_in2 ReferenceUtf8
#include "clause_input_reference.inc"
#undef ucd_properties
#undef clause_type_from_codepoint
#undef Eof
#undef GetC
#undef ungot_char
#undef count_characters
#undef p_decoder
#undef text_decoder_eof
#undef text_decoder_getc
#undef WordToString2
#undef utf8_in2
static unsigned seed=0x714309abu;
static unsigned next(void){seed^=seed<<13;seed^=seed>>17;seed^=seed<<5;return seed;}
int main(void)
{
 size_t utf8_cases=0,cursor_cases=0;
 for(uint32_t c=0;c<0x110000;c++)TEST_ASSERT(espeak_rs_clause_type(c)==ReferenceType(c));
 override_enabled=1;
 for(uint64_t bits=0;bits<4096;bits++) {
  override_properties=(bits<<52)|next();
  TEST_ASSERT(espeak_rs_clause_properties(override_properties)==ReferenceType(0));
 }
 override_enabled=0;
 static const unsigned short table[]={0xad,1,0x640,1,0x200c,45,7,0,7,1,0,0};
 Translator tr={0};tr.chars_ignore=table;
 for(int i=0;i<200000;i++) {
  char actual[5],expected[5];memset(actual,0xa5,5);memset(expected,0xa5,5);unsigned word=next();
  if(i%5==0)word &= (unsigned[]){0xffffff00,0xffff00ff,0xff00ffff,0x00ffffff,0}[next()%5];
  TEST_ASSERT(ReferenceWord(expected,word)==expected);espeak_rs_clause_word((unsigned char *)actual,word);TEST_ASSERT(memcmp(actual,expected,5)==0);
  uint32_t code=next()%0x110000;TEST_ASSERT(espeak_rs_clause_roman(code)==IsRomanU(code));
  int a=(int)(next()%5)-2,b=(int)next(),c1=(int[]){91,93,65,-1}[next()%4],c2=(int[]){91,93,65,-1}[next()%4];
  TEST_ASSERT(espeak_rs_clause_phoneme_mode(a,b,c1,c2)==CheckPhonemeMode(a,b,c1,c2));
  int old=(int[]){0xad,0x640,0x200c,7,0x200d,-1,(int)code}[next()%7],fresh=old;
  int ignore=IgnoreOrReplaceChar(&tr,&old);
  TEST_ASSERT(espeak_rs_clause_replace(table,sizeof(table)/sizeof(table[0]),&fresh)==ignore && fresh==old);
 }
 for(int i=0;i<500000;i++) {
  unsigned char text[40]={0};text[0]='a';
  for(int j=1;j<39;j++)text[j]=(unsigned char)(1+next()%255);
  text[1+next()%38]=0;
  int position=(int)(next()%40),backwards=(int)(next()%2);
  /* The last NUL and first ASCII byte bound both directional scans; the
   * initialized trailing padding permits every possible three-byte tail. */
  if(position>35)position=35;
  int expected=-1,actual=-1;
  int want=ReferenceUtf8(&expected,(const char *)text+position,backwards);
  int got=espeak_rs_utf8_in2(&actual,text+position,backwards);
  TEST_ASSERT(got==want && actual==expected);utf8_cases++;
 }
 espeak_ng_TEXT_DECODER *decoder=create_text_decoder();TEST_ASSERT(decoder!=NULL);
 for(int episode=0;episode<2000;episode++) {
  source_position=0;source_length=(int)(next()%100);
  for(int i=0;i<source_length;i++)source[i]=(wchar_t)(next()%0x110000);
  TEST_ASSERT(text_decoder_decode_wstring(decoder,source,source_length)==ENS_OK);
  int pending=reference_pending=(int)(next()%4),count=reference_count=-1;
  for(int step=0;step<120;step++) {
   TEST_ASSERT(espeak_rs_clause_eof(pending,decoder)==ReferenceEof());
   int expected=ReferenceGet(),got=espeak_rs_clause_getc(&pending,&count,decoder);
   TEST_ASSERT(got==expected && pending==reference_pending && count==reference_count);
   if(next()%5==0)pending=reference_pending=(int)next();
   cursor_cases++;
  }
 }
 TEST_ASSERT(text_decoder_decode_wstring(decoder,source,1)==ENS_OK);
 int pending=0,count=INT_MAX;
 const void *before=text_decoder_get_buffer(decoder);
 TEST_ASSERT(espeak_rs_clause_getc(&pending,&count,decoder)==0 && pending==0 && count==INT_MAX);
 TEST_ASSERT(text_decoder_get_buffer(decoder)==before);
 pending=88;TEST_ASSERT(espeak_rs_clause_getc(&pending,&count,decoder)==88 && pending==0 && count==INT_MAX);
 TEST_ASSERT(text_decoder_get_buffer(decoder)==before);
 unsigned short missing[]={5,6};int code=5;
 TEST_ASSERT(espeak_rs_clause_replace(missing,2,&code)==-1 && code==5);
 destroy_text_decoder(decoder);
 printf("Matched 1114112 codepoint classes, 4096 property combinations, 200000 preprocessing cases, %zu UTF8 scans and %zu cursor transitions\n",utf8_cases,cursor_cases);
 return 0;
}
