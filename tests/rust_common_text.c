/* Common text helpers against independently extracted retained C.
 * SPDX-License-Identifier: GPL-3.0-or-later */
#include "config.h"
#include "test_assert.h"
#include <locale.h>
#include <stdint.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <wctype.h>
#include <ucd/ucd.h>
#include "common.h"
#include "rust_data.h"
#define IsAlpha ReferenceAlpha
#define IsEmoji ReferenceEmoji
#define IsRegionalIndicator ReferenceRegional
#define IsEmojiModifier ReferenceModifier
#define IsEmojiTag ReferenceTag
#define IsBracket ReferenceBracket
#define IsDigit09 ReferenceDigit09
#define IsDigit ReferenceDigit
#define IsSpace ReferenceSpace
#define isspace2 ReferenceByteSpace
#define is_str_totally_null ReferenceNull
#define StringToWord ReferenceWord
#define towlower2 ReferenceLower
static int ReferenceEmoji(unsigned int);
#include "common_text_reference.inc"
#undef IsAlpha
#undef IsEmoji
#undef IsRegionalIndicator
#undef IsEmojiModifier
#undef IsEmojiTag
#undef IsBracket
#undef IsDigit09
#undef IsDigit
#undef IsSpace
#undef isspace2
#undef is_str_totally_null
#undef StringToWord
#undef towlower2
static unsigned seed=0x3781d452u;
static unsigned next(void){seed^=seed<<13;seed^=seed>>17;seed^=seed<<5;return seed;}
static void Compare(uint32_t code)
{
 TEST_ASSERT(IsAlpha(code)==ReferenceAlpha(code));TEST_ASSERT(IsEmoji(code)==ReferenceEmoji(code));
 TEST_ASSERT(IsRegionalIndicator(code)==ReferenceRegional(code));TEST_ASSERT(IsEmojiModifier(code)==ReferenceModifier(code));
 TEST_ASSERT(IsEmojiTag(code)==ReferenceTag(code));TEST_ASSERT(IsBracket((int)code)==ReferenceBracket((int)code));
 TEST_ASSERT(IsDigit09(code)==ReferenceDigit09(code));TEST_ASSERT(IsDigit(code)==ReferenceDigit(code));
 TEST_ASSERT(IsSpace(code)==ReferenceSpace(code));TEST_ASSERT(isspace2(code)==ReferenceByteSpace(code));
 Translator translator={0};
 TEST_ASSERT(towlower2(code,&translator)==ReferenceLower(code,&translator));
 translator.langopts.dotless_i=1;TEST_ASSERT(towlower2(code,&translator)==ReferenceLower(code,&translator));
}
int main(void)
{
 size_t cases=0;
 const char *locales[]={"C","en_US.UTF-8","C.UTF-8"};
 for(size_t locale=0;locale<sizeof(locales)/sizeof(locales[0]);locale++){
  if(!setlocale(LC_CTYPE,locales[locale]))continue;
  for(uint32_t code=0;code<0x110000;code++){Compare(code);cases++;}
  Compare(0x110000);Compare(0x7fffffff);Compare(0x80000000);Compare(UINT32_MAX);
 }
 for(int i=0;i<200000;i++){
  unsigned char text[9];for(int j=0;j<8;j++)text[j]=(unsigned char)next();text[8]=0;
  text[3]&=127; // Keep the most significant byte in the defined C shift domain.
  if(i%3==0)text[next()%4]=0;
  TEST_ASSERT(StringToWord((const char *)text)==ReferenceWord((const char *)text));
  int length=1+(int)(next()%7),offset=(int)(next()%2);if(i%2==0)memset(text+offset,0,(size_t)length);
  TEST_ASSERT(is_str_totally_null((const char *)text+offset,length)==ReferenceNull((const char *)text+offset,length));
 }
 TEST_ASSERT(StringToWord(NULL)==0);TEST_ASSERT(is_str_totally_null(NULL,0)==0);
 unsigned char only_nul[1]={0};TEST_ASSERT(StringToWord((const char *)only_nul)==0);
 unsigned char only_nonzero[1]={1};TEST_ASSERT(is_str_totally_null((const char *)only_nonzero,4)==0);
 unsigned char four[4]={255,254,253,127};TEST_ASSERT(StringToWord((const char *)four)==0x7ffdfeffu);
 fprintf(stderr,"%zu exhaustive common classifications and 200000 byte-word/null cases passed\n",cases);
 return 0;
}
