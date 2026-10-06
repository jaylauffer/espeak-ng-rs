/* Punctuation announcement against independently extracted retained C.
 * SPDX-License-Identifier: GPL-3.0-or-later */
#include "config.h"
#include "test_assert.h"
#include <stdint.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include "translate.h"
#include "rust_data.h"
static unsigned trace,seed=0x8137c091u;
static int icon,period_found,name_found,source[12],source_position,source_length,pending,pending_second,counter;
static int announcement_speed[16],name_changes;
static char period_name[30],character_name[74];
static unsigned next(void){seed^=seed<<13;seed^=seed>>17;seed^=seed<<5;return seed;}
static void Trace(int kind,int value){trace=trace*33+(unsigned)kind;trace=trace*33+(unsigned)value;trace=trace*33+(unsigned)pending;trace=trace*33+(unsigned)counter;}
static int Icon(int code){Trace(1,code);return icon;}
static const char *Special(Translator *tr,const char *key,char *out,size_t capacity)
{
 (void)capacity;TEST_ASSERT(strcmp(key,"_.p")==0);Trace(2,0);
 if(name_changes){tr->langopts.param[LOPT_ANNOUNCE_PUNCT]^=2;announcement_speed[EMBED_S]=350;}
 if(!period_found)return NULL;strcpy(out,period_name);return out;
}
static const char *Name(char *out,Translator *tr,int code,bool only)
{
 TEST_ASSERT(!only);Trace(3,code);
 if(name_changes){tr->langopts.param[LOPT_ANNOUNCE_PUNCT]^=2;announcement_speed[EMBED_S]=350;}
 if(!name_found)return NULL;strcpy(out,character_name);return out;
}
static int EofSource(void){Trace(4,0);return pending==0 && source_position>=source_length;}
static int Get(void)
{
 Trace(5,0);if(pending){int result=pending;pending=0;return result;}
 counter++;return source_position<source_length?source[source_position++]:0;
}
static void Unget(int value){Trace(6,value);pending=value;}
static void UngetSecond(int value){pending_second=value;}
static void Terminate(char *out,int index,int *value){out[index]=' ';out[index+1]=0;if(value)Unget(*value);}
#define LookupSoundicon Icon
#define LookupSpecial Special
#define LookupCharName Name
#define Eof EofSource
#define GetC Get
#define UngetC Unget
#define ungot_char2 pending_second
#define embedded_value announcement_speed
#define TerminateBufWithSpaceAndZero Terminate
#define clause_type_from_codepoint espeak_rs_clause_type
#define AnnouncePunctuation ReferenceAnnounce
#include "clause_punctuation_reference.inc"
#undef LookupSoundicon
#undef LookupSpecial
#undef LookupCharName
#undef Eof
#undef GetC
#undef UngetC
#undef ungot_char2
#undef embedded_value
#undef TerminateBufWithSpaceAndZero
#undef clause_type_from_codepoint
#undef AnnouncePunctuation
static int32_t NativeName(void *owner,int32_t code,uint32_t period,unsigned char (*out)[74])
{
 char text[74]={0};const char *name=period?Special(owner,"_.p",text,sizeof(text)):Name(text,owner,code,false);
 if(name==NULL)return 1;strcpy((char *)*out,name);return 0;
}
int main(void)
{
 static const int codes[]={46,44,59,45,60,33,63,0x2026,0x61f,0x55e,0x2014,65};
 for(int i=0;i<200000;i++) {
  Translator tr={0};tr.langopts.param[LOPT_ANNOUNCE_PUNCT]=(int)(next()%4);
  int code=codes[next()%(sizeof(codes)/sizeof(codes[0]))],following=next()%2?code:88;
  int end=(int)(next()%2),offset=(int)(next()%20),actual_offset=offset,expected_offset=offset;
  icon=next()%6==0?(int)(next()%10000):-1;period_found=(int)(next()%2);name_found=next()%7!=0;name_changes=(int)(next()%2);
  int name_length=(int)(next()%45);memset(character_name,'a',name_length);character_name[name_length]=0;
  strcpy(period_name,i%2?"period":"[\002pI@ri@d]]");
  source_position=0;source_length=(int)(next()%12);pending=0;pending_second=0;counter=(int)(next()%300);
  for(int j=0;j<source_length;j++)source[j]=j==source_length-1?88:code;
  announcement_speed[EMBED_S]=175+(int)(next()%300);
  int initial_counter=counter,initial_flags=tr.langopts.param[LOPT_ANNOUNCE_PUNCT],initial_speed=announcement_speed[EMBED_S];
  char expected[512],actual[512];memset(expected,0xa5,512);memset(actual,0xa5,512);
  int actual_next=following,expected_next=following;trace=0;
  int want=ReferenceAnnounce(&tr,code,&expected_next,expected,&expected_offset,end,512);
  unsigned expected_trace=trace;int expected_position=source_position,expected_pending=pending,expected_second=pending_second,expected_counter=counter;
  int expected_flags=tr.langopts.param[LOPT_ANNOUNCE_PUNCT],expected_speed=announcement_speed[EMBED_S];
  source_position=0;pending=0;pending_second=0;counter=initial_counter;tr.langopts.param[LOPT_ANNOUNCE_PUNCT]=initial_flags;announcement_speed[EMBED_S]=initial_speed;trace=0;
  RustClausePunctuation context={.owner=&tr,.icon=Icon,.name=NativeName,.eof=EofSource,.read=Get,.unread=Unget,.unread_second=UngetSecond,
   .flags=&tr.langopts.param[LOPT_ANNOUNCE_PUNCT],.speed=&announcement_speed[EMBED_S]};
  int got=espeak_rs_clause_announce(&context,code,&actual_next,(unsigned char *)actual,512,&actual_offset,end);
  if(got!=want || trace!=expected_trace)fprintf(stderr,"punctuation mismatch i=%d code=%x clauses=%x/%x traces=%u/%u\n",i,code,got,want,trace,expected_trace);
  TEST_ASSERT(got==want && actual_next==expected_next && actual_offset==expected_offset);
  TEST_ASSERT(memcmp(actual,expected,512)==0);
  TEST_ASSERT(trace==expected_trace && source_position==expected_position && pending==expected_pending && pending_second==expected_second && counter==expected_counter);
  TEST_ASSERT(tr.langopts.param[LOPT_ANNOUNCE_PUNCT]==expected_flags && announcement_speed[EMBED_S]==expected_speed);
 }
 Translator tr={0};tr.langopts.param[LOPT_ANNOUNCE_PUNCT]=2;
 RustClausePunctuation context={.owner=&tr,.icon=Icon,.name=NativeName,.eof=EofSource,.read=Get,.unread=Unget,.unread_second=UngetSecond,
  .flags=&tr.langopts.param[LOPT_ANNOUNCE_PUNCT],.speed=&announcement_speed[EMBED_S]};
 icon=7;int following=88,offset=5;unsigned char output[12];memset(output,0xa5,12);
 TEST_ASSERT(espeak_rs_clause_announce(&context,45,&following,output,5,&offset,1)==-1);
 TEST_ASSERT(offset==5 && following==88 && pending==88);
 for(int i=0;i<12;i++)TEST_ASSERT(output[i]==0xa5);
 TEST_ASSERT(espeak_rs_clause_announce(&context,45,&following,output,12,&offset,2)==-1);
 puts("Matched 200000 punctuation output/tail/pause/stream/state/backend-order comparisons; capacity guards preserved output");
 return 0;
}
