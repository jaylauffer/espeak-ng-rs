/* Complete C assembler controllers compared with the retained Rust owner.
 * SPDX-License-Identifier: GPL-3.0-or-later */
#include <stdio.h>
#include <stdint.h>
#include <stdlib.h>
#include <string.h>
#include <stdarg.h>
#include <stdbool.h>
#include "test_assert.h"
#include "speech.h"
#include "phoneme.h"
#include "synthesize.h"
#include "rust_compiler_program.h"
enum { tSTRING=2,tNUMBER,tSIGNEDNUMBER,tPHONEMEMNEM,tOPENBRACKET,tKEYWORD,tCONDITION,tPROPERTIES };
enum { tTRANSITION=3,tWHICH_PHONEME=6,tTEST=7,k_AND=1,k_OR,k_THEN,k_NOT,kTHISSTRESS=0x800,
 kFMT=21,kWAV,kVOWELSTART,kVOWELENDING,kANDWAV };
#define MAX_PROG_BUF 2000
#define N_ITEM_STRING 256
typedef struct { unsigned short *p_then,*p_else;bool returned; } IF_STACK;
typedef struct { RustProgramToken token;int requested; } Planned;
static Planned plan[512];static unsigned planned;static uint32_t rng=0x5391287;
typedef struct {
 PHONEME_TAB row,*phoneme_out;unsigned short prog_buf[2020],*prog_out,*prog_out_max;
 IF_STACK if_stack[12];int if_level,then_count;bool after_if;
 int item_type,item_terminator,error_count;char item_string[256];
 unsigned at,lookups,loads,ungets,trace_count;uint32_t trace_events[8192];uint64_t trace;FILE *f_errors;
} CompileContext;
static uint32_t Next(void) {rng=rng*1664525u+1013904223u;return rng;}
static void Trace(CompileContext *c,uint32_t value) {TEST_ASSERT(c->trace_count<8192);c->trace_events[c->trace_count++]=value;c->trace=(c->trace^value)*1099511628211ull;}
static void error(CompileContext *c,const char *format,...)
{va_list args;va_start(args,format);vfprintf(c->f_errors,format,args);va_end(args);fputc('\n',c->f_errors);c->error_count++;}
static int NextItem(CompileContext *c,int type)
{TEST_ASSERT(c->at<planned);Planned *p=&plan[c->at++];TEST_ASSERT(type==p->requested);c->item_type=p->token.kind;c->item_terminator=p->token.terminator;memcpy(c->item_string,p->token.text,256);Trace(c,(uint32_t)type);return p->token.value;}
static void UngetItem(CompileContext *c) {TEST_ASSERT(c->at);c->at--;c->ungets++;Trace(c,0xfeed);}
static int LookupPhoneme(CompileContext *c,const char *name,int control)
{TEST_ASSERT(control==2);c->lookups++;Trace(c,0x1234);unsigned h=0;for(const unsigned char *p=(const unsigned char *)name;*p;p++){h=h*31+*p;Trace(c,*p);}return (h&63)+8;}
static int LoadDataFile(CompileContext *c,const char *name,int control,int *addr)
{c->loads++;Trace(c,(uint32_t)(0x3456+control));unsigned h=0;for(const unsigned char *p=(const unsigned char *)name;*p;p++){h=h*31+*p;Trace(c,*p);}*addr=(strcmp(name,"NULL")==0)?0:(int)(h&0xffffff);return 0;}
#include "compiler_program_reference.inc"
_Static_assert(sizeof(RustProgramToken)==268,"token layout");
_Static_assert(sizeof(void *)!=8||sizeof(RustCompilerProgram)==96,"assembler descriptor layout");
_Static_assert(sizeof(RustProgramResult)==36,"assembler result layout");
_Static_assert(RUST_PROGRAM_WORDS==MAX_PROG_BUF+20,"original initialized program capacity");
typedef struct {
 CompileContext ctx;uint64_t owner;RustProgramResult result;RustCompilerProgram *descriptor;
 int inspect,mutate,fail,events,write_calls,source_fail,bad_token;uint16_t *loan;
 int64_t position;unsigned char payload[4040],log[8];size_t payload_len,log_len;
} Effects;
static unsigned comparisons,episodes;static void Diagnostic(void *,const unsigned char *,size_t);
static void Reentry(Effects *e)
{
 e->events++;
 if(e->inspect){RustProgramResult out;memset(&out,0xa5,sizeof(out));RustCompilerProgram nested={.owner=e->owner,.context=e,.error=Diagnostic};
  TEST_ASSERT(espeak_rs_compiler_program_call(&nested,RUST_PROGRAM_ELSE,&out)==11);TEST_ASSERT(out.count==(int32_t)0xa5a5a5a5);
  TEST_ASSERT(espeak_rs_compiler_program_end(e->owner)==11);TEST_ASSERT(espeak_rs_compiler_program_begin()==0);
  TEST_ASSERT(espeak_rs_compiler_program_copy(e->owner,NULL,0)==-11);TEST_ASSERT(e->result.count==(int32_t)0xa5a5a5a5);
 }
 if(e->mutate){if(e->descriptor){memset(e->descriptor,0,sizeof(*e->descriptor));}if(e->loan){e->loan[0]=0x5a5a;}}
}
static int Read(void *opaque,int32_t type,RustProgramToken *token)
{Effects *e=opaque;if(e->source_fail){Reentry(e);return e->source_fail;}token->value=NextItem(&e->ctx,type);token->kind=e->ctx.item_type;token->terminator=e->ctx.item_terminator;memcpy(token->text,e->ctx.item_string,256);if(e->bad_token){memset(token->text,'x',256);}Reentry(e);return 0;}
static void Replay(void *opaque) {Effects *e=opaque;UngetItem(&e->ctx);Reentry(e);}
static void Diagnostic(void *opaque,const unsigned char *bytes,size_t length)
{Effects *e=opaque;TEST_ASSERT(length<320);error(&e->ctx,"%.*s",(int)length,(const char *)bytes);Reentry(e);}
static int Lookup(void *opaque,const unsigned char *bytes,size_t length)
{Effects *e=opaque;char name[256];TEST_ASSERT(length<256);memcpy(name,bytes,length);name[length]=0;int value=LookupPhoneme(&e->ctx,name,2);Reentry(e);return value;}
static int Load(void *opaque,const unsigned char *bytes,size_t length,int32_t control)
{Effects *e=opaque;char name[256];TEST_ASSERT(length<256);memcpy(name,bytes,length);name[length]=0;int value=0;LoadDataFile(&e->ctx,name,control,&value);Reentry(e);return value;}
static int64_t Position(void *opaque) {Effects *e=opaque;Reentry(e);return e->position;}
static int Write(void *opaque,int32_t log,const unsigned char *bytes,size_t length)
{Effects *e=opaque;Reentry(e);e->write_calls++;if(log){TEST_ASSERT(length==8);memcpy(e->log,bytes,length);e->log_len=length;}else{TEST_ASSERT(length<=4040);memcpy(e->payload,bytes,length);e->payload_len=length;}return e->fail;}
static int Call(Effects *e,unsigned operation,int control,int kind,const uint16_t *words,size_t count)
{RustCompilerProgram d={e->owner,e,words,count,control,kind,Read,Replay,Diagnostic,Lookup,Load,Position,Write};e->descriptor=&d;memset(&e->result,0xa5,sizeof(e->result));int status=espeak_rs_compiler_program_call(&d,operation,&e->result);e->descriptor=NULL;return status;}
static void Add(int requested,int value,int kind,int term,const char *name)
{TEST_ASSERT(planned<512);Planned *p=&plan[planned++];memset(p,0,sizeof(*p));p->requested=requested;p->token.value=value;p->token.kind=kind;p->token.terminator=term;snprintf((char *)p->token.text,256,"%s",name?name:"");}
static size_t Contents(FILE *f,unsigned char *bytes)
{TEST_ASSERT(fflush(f)==0);long n=ftell(f);TEST_ASSERT(n>=0&&n<16384);rewind(f);TEST_ASSERT(fread(bytes,1,(size_t)n,f)==(size_t)n);TEST_ASSERT(fseek(f,0,SEEK_END)==0);return (size_t)n;}
static void Check(CompileContext *c,Effects *e)
{uint16_t words[2020];int n=espeak_rs_compiler_program_copy(e->owner,words,2020);TEST_ASSERT(n==c->prog_out-c->prog_buf);TEST_ASSERT(n==e->result.count);TEST_ASSERT(memcmp(words,c->prog_buf,(size_t)n*2)==0);
 TEST_ASSERT(e->result.level==c->if_level&&e->result.then_count==c->then_count&&e->result.after_if==c->after_if&&e->result.returned==c->if_stack[c->if_level].returned);
 TEST_ASSERT(c->at==e->ctx.at&&c->trace==e->ctx.trace&&c->loads==e->ctx.loads&&c->lookups==e->ctx.lookups&&c->ungets==e->ctx.ungets&&c->error_count==e->ctx.error_count);
 TEST_ASSERT(c->trace_count==e->ctx.trace_count&&memcmp(c->trace_events,e->ctx.trace_events,c->trace_count*sizeof(uint32_t))==0);
 static unsigned char a[16384],b[16384];size_t x=Contents(c->f_errors,a),y=Contents(e->ctx.f_errors,b);TEST_ASSERT(x==y&&memcmp(a,b,x)==0);comparisons++;}
static void Start(CompileContext *c,Effects *e)
{memset(c,0,sizeof(*c));memset(e,0,sizeof(*e));c->phoneme_out=&c->row;e->ctx.phoneme_out=&e->ctx.row;c->f_errors=tmpfile();e->ctx.f_errors=tmpfile();TEST_ASSERT(c->f_errors&&e->ctx.f_errors);e->owner=espeak_rs_compiler_program_begin();TEST_ASSERT(e->owner);c->prog_out=c->prog_buf;c->prog_out_max=c->prog_buf+1999;}
static void Close(CompileContext *c,Effects *e)
{TEST_ASSERT(espeak_rs_compiler_program_end(e->owner)==0);fclose(c->f_errors);fclose(e->ctx.f_errors);}
static void Source(CompileContext *c,Effects *e) {planned=0;c->at=e->ctx.at=0;}
static void Emit(CompileContext *c,Effects *e)
{uint16_t words[4];unsigned count=1+Next()%4;for(unsigned i=0;i<count;i++){words[i]=(uint16_t)Next();*c->prog_out++=words[i];}if(c->then_count>0)c->then_count--;TEST_ASSERT(Call(e,RUST_PROGRAM_EMIT,0,0,words,count)==0);TEST_ASSERT(Call(e,RUST_PROGRAM_DECREMENT,0,0,NULL,0)==0);Check(c,e);}
static void Condition(CompileContext *c,Effects *e,int elif,unsigned trial)
{Source(c,e);unsigned count=1+Next()%5;for(unsigned i=0;i<count;i++){
 if(Next()%2){Add(tCONDITION,k_NOT,0,0,"NOT");}
 unsigned form=Next()%3;
 if(form==0){Add(tCONDITION,0xf01+(int)(Next()%3),tTEST,0,"KlattSynth");}
 else if(form==1){int which=(int)(Next()%11);Add(tCONDITION,which,tWHICH_PHONEME,0,"nextPh");Add(tOPENBRACKET,1,-1,0,"");int value=Next()%2?(int)(Next()%64):-1;Add(tPROPERTIES,value,-1,')',value<0?"xyz":"isVowel");}
 else {Add(tCONDITION,kTHISSTRESS,tTEST,0,"Stress");Add(tOPENBRACKET,1,-1,0,"");unsigned stresses=1+Next()%4;for(unsigned k=0;k<stresses;k++){Add(tNUMBER,(int)(Next()%12),-1,k+1<stresses?',':')',"");}}
 Add(tCONDITION,i+1<count?(Next()%2?k_AND:k_OR):k_THEN,0,0,"");
 }
 if(trial%47==0){Source(c,e);Add(tCONDITION,-1,-1,0,"endphoneme");}
 int value=elif==2?CompileElif(c):CompileIf(c,elif);TEST_ASSERT(Call(e,elif==2?RUST_PROGRAM_ELIF:RUST_PROGRAM_IF,elif==1,0,NULL,0)==0);TEST_ASSERT(e->result.value==value);Check(c,e);}
static void Pair(unsigned trial)
{CompileContext c;Effects e;Start(&c,&e);
 /* Complete transition grammar, including signed ranges and replay. */
 Source(&c,&e);int which=(int)(trial%3),kind=(trial%2?phSTOP:phVOWEL);c.row.type=(unsigned char)kind;
 for(int key=1;key<=13;key++){Add(tKEYWORD,key,tTRANSITION,0,"");switch(key){
 case 1:case 2:case 13:Add(tNUMBER,(int)(Next()%500),-1,0,"");break;
 case 3:Add(tNUMBER,(int)(Next()%2),-1,0,"");break;
 case 4:Add(tNUMBER,(int)(Next()%5000),-1,0,"");Add(tSIGNEDNUMBER,(int)(Next()%3000)-1500,-1,0,"");Add(tSIGNEDNUMBER,(int)(Next()%3000)-1500,-1,0,"");break;
 case 5:Add(tSIGNEDNUMBER,(int)(Next()%3000)-1500,-1,0,"");Add(tNUMBER,(int)(Next()%300),-1,0,"");break;
 case 12:Add(tNUMBER,(int)(Next()%3),-1,0,"");break;}}
 Add(tKEYWORD,4,4,0,"endphoneme");CompileVowelTransition(&c,which);TEST_ASSERT(Call(&e,RUST_PROGRAM_TRANSITION,which,kind,NULL,0)==0);Check(&c,&e);
 /* Sound parameter clamp, operand sign, copied resource path. */
 for(int key=kFMT;key<=kANDWAV;key++){
  Source(&c,&e);Add(tOPENBRACKET,trial%17==0?0:1,-1,0,"");Add(tSTRING,0,-1,trial%3?',':')',trial%11==0?"NULL":"resource/spec");
  if(trial%3){Add(key==kVOWELSTART||key==kVOWELENDING?tSIGNEDNUMBER:tNUMBER,(key==kVOWELSTART||key==kVOWELENDING)?(int)(Next()%1000)-500:(int)(Next()%1000),-1,trial%19?'\x29':'x',"");}
  CompileSound(&c,key,trial%2);TEST_ASSERT(Call(&e,RUST_PROGRAM_SOUND,key,trial%2,NULL,0)==0);Check(&c,&e);
 }
 /* Tone paths, address and narrowed pitch endpoints. */
 Source(&c,&e);Add(tOPENBRACKET,1,-1,0,"");Add(tNUMBER,(int)(Next()%512),-1,',',"");Add(tNUMBER,(int)(Next()%512),-1,',',"");Add(tSTRING,0,-1,',',trial%7==0?"NULL":"pitch/env");Add(tSTRING,0,-1,')',trial%13==0?"NULL":"amp/env");
 CompileToneSpec(&c);TEST_ASSERT(Call(&e,RUST_PROGRAM_TONE,0,0,NULL,0)==0);TEST_ASSERT(e.result.start==c.row.start_type&&e.result.end==c.row.end_type);Check(&c,&e);
 /* Nested branches, ELSE chains, returning branch and single-statement folds. */
 unsigned depth=1+Next()%5;for(unsigned i=0;i<depth;i++){Condition(&c,&e,0,trial+1);Emit(&c,&e);}
 for(unsigned i=0;i<depth;i++){
  unsigned count=Next()%4;for(unsigned k=0;k<count;k++){if(k%2){Condition(&c,&e,2,trial+1);}else{Source(&c,&e);CompileElse(&c);TEST_ASSERT(Call(&e,RUST_PROGRAM_ELSE,0,0,NULL,0)==0);Check(&c,&e);Condition(&c,&e,1,trial+1);}Emit(&c,&e);}
  if(Next()%2){c.if_stack[c.if_level].returned=true;TEST_ASSERT(Call(&e,RUST_PROGRAM_RETURNED,0,0,NULL,0)==0);}
  CompileElse(&c);TEST_ASSERT(Call(&e,RUST_PROGRAM_ELSE,0,0,NULL,0)==0);Check(&c,&e);Emit(&c,&e);CompileEndif(&c);TEST_ASSERT(Call(&e,RUST_PROGRAM_ENDIF,0,0,NULL,0)==0);Check(&c,&e);
 }
 CompileElse(&c);TEST_ASSERT(Call(&e,RUST_PROGRAM_ELSE,0,0,NULL,0)==0);CompileElif(&c);TEST_ASSERT(Call(&e,RUST_PROGRAM_ELIF,0,0,NULL,0)==0);CompileEndif(&c);TEST_ASSERT(Call(&e,RUST_PROGRAM_ENDIF,0,0,NULL,0)==0);Check(&c,&e);
 for(int which=0;which<=2;which++){CompileSwitch(&c,which);TEST_ASSERT(Call(&e,RUST_PROGRAM_SWITCH,which,0,NULL,0)==0);Check(&c,&e);}
 if(trial%47==0){Condition(&c,&e,0,trial);}
 if(c.if_level>0){error(&c,"Missing ENDIF");}if(c.prog_out>c.prog_buf&&!c.if_stack[0].returned){*c.prog_out++=INSTN_RETURN;}
 TEST_ASSERT(Call(&e,RUST_PROGRAM_FINISH,0,0,NULL,0)==0);Check(&c,&e);
 e.position=4+(int64_t)trial*140;TEST_ASSERT(Call(&e,RUST_PROGRAM_PUBLISH,1,0,NULL,0)==0);
 TEST_ASSERT(e.result.value==e.position/2);TEST_ASSERT(e.payload_len==(size_t)(c.prog_out-c.prog_buf)*2&&memcmp(e.payload,c.prog_buf,e.payload_len)==0);
 PHONEME_PROG_LOG log={(uint16_t)(e.position/2),(unsigned)(c.prog_out-c.prog_buf)};TEST_ASSERT(e.log_len==8&&memcmp(&log,e.log,8)==0);TEST_ASSERT(e.write_calls==2);Close(&c,&e);episodes++;
}
static void Boundary(void)
{CompileContext c;Effects e;Start(&c,&e);uint16_t words[256]={0};
 for(unsigned i=0;i<7;i++){TEST_ASSERT(Call(&e,RUST_PROGRAM_EMIT,0,0,words,256)==0);}TEST_ASSERT(Call(&e,RUST_PROGRAM_EMIT,0,0,words,207)==0);c.prog_out+=1999;
 TEST_ASSERT(CompileIf(&c,0)==0);TEST_ASSERT(Call(&e,RUST_PROGRAM_IF,0,0,NULL,0)==0);Check(&c,&e);
 *c.prog_out++=INSTN_RETURN;TEST_ASSERT(Call(&e,RUST_PROGRAM_FINISH,0,0,NULL,0)==0);Check(&c,&e);
 TEST_ASSERT(Call(&e,RUST_PROGRAM_PUBLISH,0,0,NULL,0)==0&&e.payload_len==4000&&memcmp(e.payload,c.prog_buf,4000)==0);Close(&c,&e);
}
static void Guards(void)
{CompileContext c;Effects e;Start(&c,&e);e.inspect=e.mutate=1;Source(&c,&e);Add(tKEYWORD,4,4,0,"endphoneme");TEST_ASSERT(Call(&e,RUST_PROGRAM_TRANSITION,1,4,NULL,0)==0);TEST_ASSERT(e.events>=2);e.inspect=e.mutate=0;
 TEST_ASSERT(Call(&e,RUST_PROGRAM_RESET,0,0,NULL,0)==0);
 e.source_fail=5;TEST_ASSERT(Call(&e,RUST_PROGRAM_TRANSITION,1,4,NULL,0)==5);e.source_fail=0;
 Source(&c,&e);Add(tKEYWORD,4,4,0,"endphoneme");e.bad_token=1;TEST_ASSERT(Call(&e,RUST_PROGRAM_TRANSITION,1,4,NULL,0)==22&&e.result.count==0);e.bad_token=0;
 TEST_ASSERT(Call(&e,RUST_PROGRAM_RESET,0,0,NULL,0)==0);
 for(unsigned depth=1;depth<=12;depth++){Source(&c,&e);Add(tCONDITION,0xf01,tTEST,0,"PreVoicing");Add(tCONDITION,k_THEN,0,0,"THEN");int status=Call(&e,RUST_PROGRAM_IF,0,0,NULL,0);TEST_ASSERT(status==(depth==12?22:0));}TEST_ASSERT(e.result.level==11);
 TEST_ASSERT(Call(&e,RUST_PROGRAM_RESET,0,0,NULL,0)==0);
 /* Source EOF is finite; C would keep asking for conditions indefinitely. */
 Source(&c,&e);Add(tCONDITION,-2,-1,0,"");TEST_ASSERT(Call(&e,RUST_PROGRAM_IF,0,0,NULL,0)==0&&e.result.value==-1);
 TEST_ASSERT(Call(&e,RUST_PROGRAM_RESET,0,0,NULL,0)==0);uint16_t words[256]={0x1234};
 for(unsigned i=0;i<7;i++){TEST_ASSERT(Call(&e,RUST_PROGRAM_EMIT,0,0,words,256)==0);}TEST_ASSERT(Call(&e,RUST_PROGRAM_EMIT,0,0,words,228)==0);
 e.inspect=e.mutate=1;e.loan=words;TEST_ASSERT(Call(&e,RUST_PROGRAM_EMIT,0,0,words,1)==22);TEST_ASSERT(words[0]==0x5a5a&&e.result.count==2020);e.loan=NULL;e.inspect=e.mutate=0;
 TEST_ASSERT(Call(&e,RUST_PROGRAM_RESET,0,0,NULL,0)==0);words[0]=0x1234;TEST_ASSERT(Call(&e,RUST_PROGRAM_EMIT,0,0,words,1)==0);e.inspect=1;e.fail=28;
 TEST_ASSERT(Call(&e,RUST_PROGRAM_PUBLISH,1,0,NULL,0)==28);TEST_ASSERT(e.write_calls==1&&e.payload_len==0);TEST_ASSERT(Call(&e,RUST_PROGRAM_PUBLISH,1,0,NULL,0)==22);TEST_ASSERT(e.write_calls==1);e.inspect=0;
 RustCompilerProgram d={.owner=e.owner,.context=&e,.error=Diagnostic};RustProgramResult out;memset(&out,0xa5,sizeof(out));
 TEST_ASSERT(espeak_rs_compiler_program_call(&d,99,&out)==22&&out.count==(int32_t)0xa5a5a5a5);
 d.length=257;d.words=words;TEST_ASSERT(espeak_rs_compiler_program_call(&d,RUST_PROGRAM_EMIT,&out)==22&&out.count==(int32_t)0xa5a5a5a5);
 TEST_ASSERT(espeak_rs_compiler_program_copy(e.owner,NULL,0)==-22);
 /* Emitted words and publication may share caller storage. */
 TEST_ASSERT(Call(&e,RUST_PROGRAM_RESET,0,0,NULL,0)==0);union {RustProgramResult out;uint16_t words[18];} alias;memset(&alias,0,sizeof(alias));alias.words[0]=0xabcd;d.words=alias.words;d.length=1;
 TEST_ASSERT(espeak_rs_compiler_program_call(&d,RUST_PROGRAM_EMIT,&alias.out)==0&&alias.out.count==1);uint16_t copied;TEST_ASSERT(espeak_rs_compiler_program_copy(e.owner,&copied,1)==1&&copied==0xabcd);
 uint64_t owners[3];for(unsigned i=0;i<3;i++){owners[i]=espeak_rs_compiler_program_begin();TEST_ASSERT(owners[i]);}TEST_ASSERT(espeak_rs_compiler_program_begin()==0);for(unsigned i=0;i<3;i++){TEST_ASSERT(espeak_rs_compiler_program_end(owners[i])==0);}
 uint64_t old=e.owner;Close(&c,&e);TEST_ASSERT(espeak_rs_compiler_program_end(old)==9);TEST_ASSERT(espeak_rs_compiler_program_call(&d,RUST_PROGRAM_ELSE,&out)==9);TEST_ASSERT(espeak_rs_compiler_program_copy(old,NULL,0)==-9);
}
int main(void)
{for(unsigned trial=0;trial<2000;trial++){Pair(trial);}Boundary();Guards();printf("%u complete C assembler episodes, %u word/state/diagnostic/primitive comparisons; native bounds, input lifetime, reentry and failed-publication guards pass\n",episodes,comparisons);return 0;}
