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
#include "rust_compiler_definition.h"
#include "common.h"
#include <ctype.h>
#include <assert.h>
enum { tSTRING=2,tNUMBER,tSIGNEDNUMBER,tPHONEMEMNEM,tOPENBRACKET,tKEYWORD,tCONDITION,tPROPERTIES };
enum { tPHONEME_TYPE=1,tPHONEME_FLAG,tTRANSITION,tSTATEMENT,tINSTRN1,tWHICH_PHONEME,tTEST,
 k_AND=1,k_OR,k_THEN,k_NOT,kTHISSTRESS=0x800 };
enum {
	kPHONEMESTART = 1,
	kUTF8_BOM,
	kPROCEDURE,
	kENDPHONEME,
	kENDPROCEDURE,
	kPHONEMETABLE,
	kINCLUDE,
	kIMPORT_PH,

	kSTARTTYPE,
	kENDTYPE,
	kSTRESSTYPE,
	kVOICINGSWITCH,

	kIF,
	kELSE,
	kELIF,
	kENDIF,
	kCALLPH,

	kSWITCH_PREVVOWEL,
	kSWITCH_NEXTVOWEL,
	kENDSWITCH,

	kFMT,
	kWAV,
	kVOWELSTART,
	kVOWELENDING,
	kANDWAV,

	kVOWELIN,
	kVOWELOUT,
	kTONESPEC,

	kRETURN,
	kCONTINUE,
};

#define MAX_PROG_BUF 2000
#define N_ITEM_STRING 256
typedef struct { unsigned short *p_then,*p_else;bool returned; } IF_STACK;
typedef struct { RustProgramToken token;int requested; } Planned;
static Planned plan[512];static unsigned planned;static uint32_t rng=0x5391287;
typedef struct {
 PHONEME_TAB row,*phoneme_out,phoneme_tab2[256],source;
 int n_procs,proc_addr[50],phoneme_flags;char proc_names[257][50];FILE *f_phindex,*f_prog_log;unsigned short prog_buf[2020],*prog_out,*prog_out_max;
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
{c->lookups++;Trace(c,(unsigned)(0x1234+control));unsigned h=0;for(const unsigned char *p=(const unsigned char *)name;*p;p++){h=h*31+*p;Trace(c,*p);}int index=(int)(h&63)+8;if(control==1){c->phoneme_tab2[index].mnemonic=StringToWord(name);}return index;}
static int LoadDataFile(CompileContext *c,const char *name,int control,int *addr)
{c->loads++;Trace(c,(uint32_t)(0x3456+control));unsigned h=0;for(const unsigned char *p=(const unsigned char *)name;*p;p++){h=h*31+*p;Trace(c,*p);}*addr=(strcmp(name,"NULL")==0)?0:(int)(h&0xffffff);return 0;}

#define N_PROCS 50
#define LENGTH_MOD_LIMIT 10
static int SourceEof(CompileContext *c) {Trace(c,0xffee);return c->at>=planned;}
static PHONEME_TAB *FindPhoneme(CompileContext *c,const char *name)
{Trace(c,0xcafe);for(const unsigned char *p=(const unsigned char *)name;*p;p++){Trace(c,*p);}if(!strcmp(name,"missing")){return NULL;}if(!strcmp(name,"self")){return c->phoneme_out;}return &c->source;}
static void ImportRow(CompileContext *c,const char *name,PHONEME_TAB *out)
{PHONEME_TAB *found=FindPhoneme(c,name);if(!found){error(c,"Cannot find phoneme '%s' to import.",name);return;}if(out->phflags||out->type!=phINVALID||out->start_type||out->end_type||out->std_length||out->length_mod){error(c,"Phoneme import will override set properties.");}unsigned mnemonic=out->mnemonic;unsigned char code=out->code;*out=*found;out->mnemonic=mnemonic;out->code=code;if(out->type!=phVOWEL){out->end_type=0;}}
static void ImportPhoneme(CompileContext *c) {NextItem(c,tSTRING);ImportRow(c,c->item_string,c->phoneme_out);}
static void error_from_status(CompileContext *c,espeak_ng_STATUS status,const char *name) {error(c,"feature %u: %s",(unsigned)status,name);}
#undef USE_RUST_CORE
#include "compiler_program_reference.inc"
#include "compiler_definition_reference.inc"
_Static_assert(sizeof(void *)!=8||sizeof(RustCompilerDefinition)==160,"definition descriptor");
_Static_assert(sizeof(RustDefinitionResult)==28,"definition result");
typedef struct {CompileContext ctx;uint64_t owner;RustDefinitionResult result;RustCompilerDefinition *descriptor,admitted;unsigned char *alnum;int phoneme,inspect,mutate,fail,events,writes;} Effects;
static void Guard(Effects *e)
{e->events++;if(e->inspect){RustDefinitionResult result;memset(&result,0xa5,sizeof(result));TEST_ASSERT(espeak_rs_compiler_definition(&e->admitted,e->phoneme,&result)==11);TEST_ASSERT(result.status==(int32_t)0xa5a5a5a5);TEST_ASSERT(espeak_rs_compiler_program_end(e->owner)==11);TEST_ASSERT(espeak_rs_compiler_program_begin()==0);TEST_ASSERT(espeak_rs_compiler_program_copy(e->owner,NULL,0)==-11);TEST_ASSERT(e->result.status==(int32_t)0xa5a5a5a5);}if(e->mutate){memset(e->descriptor,0,sizeof(*e->descriptor));memset(e->alnum,0,256);}}
static int Read(void *opaque,int32_t type,RustProgramToken *token)
{Effects *e=opaque;token->value=NextItem(&e->ctx,type);token->kind=e->ctx.item_type;token->terminator=e->ctx.item_terminator;memcpy(token->text,e->ctx.item_string,256);Guard(e);return 0;}
static void Replay(void *opaque){Effects *e=opaque;UngetItem(&e->ctx);Guard(e);}
static void Diagnostic(void *opaque,const unsigned char *bytes,size_t length){Effects *e=opaque;error(&e->ctx,"%.*s",(int)length,bytes);Guard(e);}
static void Name(char name[256],const unsigned char *bytes,size_t length){TEST_ASSERT(length<256);memcpy(name,bytes,length);name[length]=0;}
static int LookupMode(void *opaque,const unsigned char *bytes,size_t length,int32_t mode){Effects *e=opaque;char name[256];Name(name,bytes,length);int value=LookupPhoneme(&e->ctx,name,mode);Guard(e);return value;}
static int Lookup(void *opaque,const unsigned char *bytes,size_t length){return LookupMode(opaque,bytes,length,2);}
static int Load(void *opaque,const unsigned char *bytes,size_t length,int32_t mode){Effects *e=opaque;char name[256];Name(name,bytes,length);int address=0;LoadDataFile(&e->ctx,name,mode,&address);Guard(e);return address;}
static int64_t Position(void *opaque){Effects *e=opaque;Guard(e);TEST_ASSERT(fflush(e->ctx.f_phindex)==0);return ftell(e->ctx.f_phindex);}
static int Write(void *opaque,int32_t log,const unsigned char *bytes,size_t length){Effects *e=opaque;Guard(e);e->writes++;if(e->fail){return e->fail;}return fwrite(bytes,1,length,log?e->ctx.f_prog_log:e->ctx.f_phindex)==length?0:5;}
static int Eof(void *opaque){Effects *e=opaque;int value=SourceEof(&e->ctx);Guard(e);return value;}
static int Row(void *opaque,int32_t index,PHONEME_TAB *row){Effects *e=opaque;TEST_ASSERT(index>=0&&index<256);*row=e->ctx.phoneme_tab2[index];Guard(e);return 0;}
static int Project(void *opaque,const PHONEME_TAB *row){Effects *e=opaque;e->ctx.row=*row;e->ctx.phoneme_out=&e->ctx.row;if(e->phoneme){e->ctx.phoneme_tab2[row->code]=*row;}Guard(e);return 0;}
static int Find(void *opaque,const unsigned char *bytes,size_t length,PHONEME_TAB *row){Effects *e=opaque;char name[256];Name(name,bytes,length);PHONEME_TAB *found=FindPhoneme(&e->ctx,name);if(found){*row=*found;}Guard(e);return found?1:0;}
static int Import(void *opaque,const unsigned char *bytes,size_t length,PHONEME_TAB *row){Effects *e=opaque;char name[256];Name(name,bytes,length);ImportRow(&e->ctx,name,row);Guard(e);return 0;}
static void Unknown(void *opaque,const unsigned char *bytes,size_t length){Effects *e=opaque;char name[256];Name(name,bytes,length);error_from_status(&e->ctx,ENS_UNKNOWN_PHONEME_FEATURE,name);Guard(e);}
static int Call(Effects *e,int phoneme)
{unsigned char alnum[256];for(int i=0;i<256;i++){alnum[i]=isalnum((unsigned char)i)!=0;}RustCompilerProgram p={e->owner,e,NULL,0,1,0,Read,Replay,Diagnostic,Lookup,Load,Position,Write};RustCompilerDefinition d={p,alnum,Eof,LookupMode,Row,Project,Find,Import,Unknown};e->descriptor=&d;e->admitted=d;e->alnum=alnum;e->phoneme=phoneme;memset(&e->result,0xa5,sizeof(e->result));int status=espeak_rs_compiler_definition(&d,phoneme,&e->result);e->descriptor=NULL;return status;}
static void Add(int requested,int value,int kind,int term,const char *name)
{TEST_ASSERT(planned<512);Planned *p=&plan[planned++];memset(p,0,sizeof(*p));p->requested=requested;p->token.value=value;p->token.kind=kind;p->token.terminator=term;snprintf((char *)p->token.text,256,"%s",name?name:"");}
static void Keyword(int value,int kind,const char *name){Add(tKEYWORD,value,kind,0,name);}
static void Start(CompileContext *c,Effects *e)
{memset(c,0,sizeof(*c));memset(e,0,sizeof(*e));c->phoneme_out=&c->row;e->ctx.phoneme_out=&e->ctx.row;c->source=(PHONEME_TAB){.mnemonic=1,.phflags=phVOICED|phARTICULATION,.program=513,.type=phSTOP,.start_type=15,.end_type=20,.std_length=65,.length_mod=4};e->ctx.source=c->source;c->f_errors=tmpfile();e->ctx.f_errors=tmpfile();c->f_phindex=tmpfile();e->ctx.f_phindex=tmpfile();c->f_prog_log=tmpfile();e->ctx.f_prog_log=tmpfile();TEST_ASSERT(c->f_errors&&e->ctx.f_errors&&c->f_phindex&&e->ctx.f_phindex&&c->f_prog_log&&e->ctx.f_prog_log);e->owner=espeak_rs_compiler_program_begin();TEST_ASSERT(e->owner);}
static void Close(CompileContext *c,Effects *e){TEST_ASSERT(espeak_rs_compiler_program_end(e->owner)==0);fclose(c->f_errors);fclose(e->ctx.f_errors);fclose(c->f_phindex);fclose(e->ctx.f_phindex);fclose(c->f_prog_log);fclose(e->ctx.f_prog_log);}
static unsigned comparisons;
static void Files(FILE *a,FILE *b){TEST_ASSERT(fflush(a)==0&&fflush(b)==0);TEST_ASSERT(ftell(a)==ftell(b));rewind(a);rewind(b);for(;;){unsigned char x[4096],y[4096];size_t n=fread(x,1,sizeof(x),a),m=fread(y,1,sizeof(y),b);TEST_ASSERT(n==m&&memcmp(x,y,n)==0);if(n==0){break;}}TEST_ASSERT(fseek(a,0,SEEK_END)==0&&fseek(b,0,SEEK_END)==0);comparisons++;}
static void Source(CompileContext *c,Effects *e,const char *name){planned=0;c->at=e->ctx.at=0;Add(tSTRING,0,-1,0,name);}
static void Pair(CompileContext *c,Effects *e,int phoneme)
{TEST_ASSERT(CompilePhoneme(c,phoneme)==0);TEST_ASSERT(Call(e,phoneme)==0);TEST_ASSERT(c->at==e->ctx.at&&c->trace_count==e->ctx.trace_count);TEST_ASSERT(memcmp(c->trace_events,e->ctx.trace_events,c->trace_count*sizeof(uint32_t))==0);TEST_ASSERT(c->error_count==e->ctx.error_count&&c->n_procs==e->result.procedures);TEST_ASSERT(memcmp(c->phoneme_tab2,e->ctx.phoneme_tab2,sizeof(c->phoneme_tab2))==0);uint16_t words[2020];int n=espeak_rs_compiler_program_copy(e->owner,words,2020);if(n!=c->prog_out-c->prog_buf||memcmp(words,c->prog_buf,(size_t)n*2)){fprintf(stderr,"word mismatch comparison=%u count %d/%td IPA=%s\n",comparisons,n,c->prog_out-c->prog_buf,(char *)plan[0].token.text);for(int j=0;j<n;j++){if(words[j]!=c->prog_buf[j]){fprintf(stderr,"at %d Rust %04x C %04x\n",j,words[j],c->prog_buf[j]);}}}TEST_ASSERT(n==c->prog_out-c->prog_buf&&memcmp(words,c->prog_buf,(size_t)n*2)==0);if(phoneme){TEST_ASSERT(memcmp(&e->result.row,c->phoneme_out,sizeof(PHONEME_TAB))==0);}Files(c->f_errors,e->ctx.f_errors);Files(c->f_phindex,e->ctx.f_phindex);Files(c->f_prog_log,e->ctx.f_prog_log);comparisons+=7;}
static void Definitions(unsigned trial)
{CompileContext c;Effects e;Start(&c,&e);Source(&c,&e,"proc");Keyword(kCONTINUE,tSTATEMENT,"Continue");Keyword(kENDPROCEDURE,tSTATEMENT,"endprocedure");Pair(&c,&e,0);
 Source(&c,&e,"a");
 if(trial%3==0){Keyword(-1,-1,"vwl");Keyword(-1,-1,"alv");}else if(trial%3==1){Keyword(kCALLPH,tSTATEMENT,"CALL");Add(tSTRING,0,-1,0,"source");}else{Keyword(phFRICATIVE,tPHONEME_TYPE,"fricative");Keyword(phLIQUID,tPHONEME_TYPE,"liquid");Keyword(phVOICED,tPHONEME_FLAG,"voiced");}
 Keyword(kCALLPH,tSTATEMENT,"CALL");Add(tSTRING,0,-1,0,"proc");Keyword(kCALLPH,tSTATEMENT,"CALL");Add(tSTRING,0,-1,0,"missing");
 Keyword(i_SET_LENGTH,tINSTRN1,"length");Add(tNUMBER,(int)(Next()%700),-1,0,"");Keyword(i_LENGTH_MOD,tINSTRN1,"lengthmod");Add(tNUMBER,(int)(Next()%10),-1,0,"");
 Keyword(kSTARTTYPE,tSTATEMENT,"starttype");Add(tPHONEMEMNEM,-1,-1,0,"s");Keyword(kENDTYPE,tSTATEMENT,"endtype");Add(tPHONEMEMNEM,9,-1,0,"");Keyword(kVOICINGSWITCH,tSTATEMENT,"voicingswitch");Add(tPHONEMEMNEM,7,-1,0,"");
 for(int key=1;key<=0x14;key++){if(key>6&&key<0x10){continue;}Keyword(key,tINSTRN1,"");Add(tOPENBRACKET,1,-1,0,"");Add(tPHONEMEMNEM,31,-1,')',"");}
 for(int key=7;key<=8;key++){Keyword(key,tINSTRN1,"");Add(tNUMBER,(int)(Next()%400),-1,0,"");}
 Keyword(i_ADD_LENGTH,tINSTRN1,"LengthAdd");Add(tSIGNEDNUMBER,(int)(Next()%600)-300,-1,0,"");
 static const char *ipa[]={"NULL","a|b","a|bc|d","|a","U+03B1","U+ZZZA","U+12xy","U+0x!a","U+-12a","U+ 12a","U+0000","αβʃ","abcdefghij|","U+FFFD"};
 Keyword(i_IPA_NAME,tINSTRN1,"ipa");Add(tSTRING,0,-1,0,ipa[trial%(sizeof(ipa)/sizeof(*ipa))]);
 Keyword(kIF,tSTATEMENT,"IF");Add(tCONDITION,0xf01,tTEST,0,"PreVoicing");Add(tCONDITION,k_THEN,0,0,"THEN");Keyword(i_SET_LENGTH,tINSTRN1,"length");Add(tNUMBER,80,-1,0,"");Keyword(kELSE,tSTATEMENT,"ELSE");Keyword(kRETURN,tSTATEMENT,"RETURN");Keyword(kENDIF,tSTATEMENT,"ENDIF");
 Keyword(kVOWELIN,tSTATEMENT,"Vowelin");Keyword(4,tTRANSITION,"f2");Add(tNUMBER,1500,-1,0,"");Add(tSIGNEDNUMBER,-200,-1,0,"");Add(tSIGNEDNUMBER,300,-1,0,"");
 Keyword(kENDPHONEME,tSTATEMENT,"endphoneme");Pair(&c,&e,1);
 Source(&c,&e,"z");Keyword(kIMPORT_PH,tSTATEMENT,"import_phoneme");Add(tSTRING,0,-1,0,"source");Keyword(kSTRESSTYPE,tSTATEMENT,"stress_type");Add(tNUMBER,6,-1,0,"");Keyword(kENDPHONEME,tSTATEMENT,"endphoneme");Pair(&c,&e,1);
 Source(&c,&e,"z");Keyword(phDELETED,tPHONEME_TYPE,"delete_phoneme");Keyword(kENDPHONEME,tSTATEMENT,"endphoneme");Pair(&c,&e,1);Close(&c,&e);}
static void Registry(void)
{CompileContext c;Effects e;Start(&c,&e);TEST_ASSERT(fseek(c.f_phindex,131074,SEEK_SET)==0&&fseek(e.ctx.f_phindex,131074,SEEK_SET)==0);for(unsigned i=0;i<51;i++){char name[40];snprintf(name,sizeof(name),"p%u",i);Source(&c,&e,name);Keyword(kCONTINUE,tSTATEMENT,"Continue");Keyword(kENDPROCEDURE,tSTATEMENT,"endprocedure");if(i<50){Pair(&c,&e,0);}else{TEST_ASSERT(CompilePhoneme(&c,0)==0&&Call(&e,0)==0);TEST_ASSERT(e.result.compiled==0&&e.result.procedures==50);Files(c.f_errors,e.ctx.f_errors);}}
 Source(&c,&e,"a");Keyword(kCALLPH,tSTATEMENT,"CALL");Add(tSTRING,0,-1,0,"p0");Keyword(phVOWEL,tPHONEME_TYPE,"vowel");Keyword(kENDPHONEME,tSTATEMENT,"endphoneme");Pair(&c,&e,1);Close(&c,&e);}
static void Edges(void)
{CompileContext c;Effects e;Start(&c,&e);
 Source(&c,&e,"empty");Keyword(kENDPROCEDURE,tSTATEMENT,"endprocedure");Pair(&c,&e,0);TEST_ASSERT(e.result.procedures==0);
 for(int ending=0;ending<4;ending++){Source(&c,&e,"a");Keyword(-1,-1,"bogus_feature");Keyword(kSTARTTYPE,tSTATEMENT,"starttype");Add(tPHONEMEMNEM,12,-1,0,"");if(ending==1){Keyword(-2,-1,"");}if(ending>=2){Keyword(ending==2?kINCLUDE:kPHONEMETABLE,tSTATEMENT,ending==2?"include":"phonemetable");}Pair(&c,&e,1);}
 Source(&c,&e,"a");Keyword(kCALLPH,tSTATEMENT,"CALL");Add(tSTRING,0,-1,0,"self");Keyword(phVOWEL,tPHONEME_TYPE,"vowel");Keyword(kCONTINUE,tSTATEMENT,"Continue");Keyword(kSTRESSTYPE,tSTATEMENT,"stress_type");Add(tNUMBER,258,-1,0,"");Keyword(kENDPHONEME,tSTATEMENT,"endphoneme");Pair(&c,&e,1);
 Source(&c,&e,"a");Keyword(phVOWEL,tPHONEME_TYPE,"vowel");Keyword(phSTOP,tPHONEME_TYPE,"stop");Keyword(kIMPORT_PH,tSTATEMENT,"import_phoneme");Add(tSTRING,0,-1,0,"source");Keyword(kIMPORT_PH,tSTATEMENT,"import_phoneme");Add(tSTRING,0,-1,0,"missing");Keyword(kENDPHONEME,tSTATEMENT,"endphoneme");Pair(&c,&e,1);
 for(int key=kFMT;key<=kANDWAV;key++){Source(&c,&e,"a");Keyword(phVOWEL,tPHONEME_TYPE,"vowel");Keyword(key,tSTATEMENT,"");Add(tOPENBRACKET,1,-1,0,"");Add(tSTRING,0,-1,')',"NULL");Keyword(kENDPHONEME,tSTATEMENT,"endphoneme");Pair(&c,&e,1);}
 Source(&c,&e,"a");Keyword(phVOWEL,tPHONEME_TYPE,"vowel");Keyword(kTONESPEC,tSTATEMENT,"Tone");Add(tOPENBRACKET,1,-1,0,"");Add(tNUMBER,120,-1,',',"");Add(tNUMBER,80,-1,',',"");Add(tSTRING,0,-1,',',"NULL");Add(tSTRING,0,-1,')',"NULL");Keyword(kSWITCH_PREVVOWEL,tSTATEMENT,"");Keyword(kSWITCH_NEXTVOWEL,tSTATEMENT,"");Keyword(kENDPHONEME,tSTATEMENT,"endphoneme");Pair(&c,&e,1);Close(&c,&e);}
static void Guards(void)
{CompileContext c;Effects e;Start(&c,&e);Source(&c,&e,"a");Keyword(phVOWEL,tPHONEME_TYPE,"vowel");Keyword(i_IPA_NAME,tINSTRN1,"ipa");Add(tSTRING,0,-1,0,"U+03B1");Keyword(kENDPHONEME,tSTATEMENT,"endphoneme");e.inspect=e.mutate=1;TEST_ASSERT(Call(&e,1)==0&&e.result.compiled==1&&e.events>6);e.inspect=e.mutate=0;
 char name[256];memset(name,'x',255);name[255]=0;Source(&c,&e,name);Keyword(kCONTINUE,tSTATEMENT,"Continue");Keyword(kENDPROCEDURE,tSTATEMENT,"endprocedure");TEST_ASSERT(Call(&e,0)==0&&e.result.procedures==1);Source(&c,&e,"a");Keyword(kCALLPH,tSTATEMENT,"CALL");Add(tSTRING,0,-1,0,name);Keyword(kENDPHONEME,tSTATEMENT,"endphoneme");TEST_ASSERT(Call(&e,1)==0&&e.result.row.type==0);
 Source(&c,&e,"a");Keyword(i_LENGTH_MOD,tINSTRN1,"lengthmod");Add(tNUMBER,10,-1,0,"");TEST_ASSERT(Call(&e,1)==22&&e.result.compiled==0);
 Source(&c,&e,"failed");Keyword(kCONTINUE,tSTATEMENT,"Continue");Keyword(kENDPROCEDURE,tSTATEMENT,"endprocedure");e.fail=5;int writes=e.writes;TEST_ASSERT(Call(&e,0)==5&&e.result.procedures==1&&e.writes==writes+1);e.fail=0;RustProgramResult result;RustCompilerProgram p={.owner=e.owner,.context=&e,.error=Diagnostic,.position=Position,.write=Write};TEST_ASSERT(espeak_rs_compiler_program_call(&p,RUST_PROGRAM_PUBLISH,&result)==22);uint64_t stale=e.owner;Close(&c,&e);p.owner=stale;unsigned char alnum[256]={0};RustCompilerDefinition d=e.admitted;d.program=p;d.program.next=Read;d.program.unget=Replay;d.program.lookup=Lookup;d.program.load=Load;d.alnum=alnum;RustDefinitionResult out;memset(&out,0xa5,sizeof(out));TEST_ASSERT(espeak_rs_compiler_definition(NULL,1,&out)==22&&out.status==(int32_t)0xa5a5a5a5);
 uint64_t leases[4];for(int i=0;i<4;i++){leases[i]=espeak_rs_compiler_program_begin();TEST_ASSERT(leases[i]);}TEST_ASSERT(espeak_rs_compiler_program_begin()==0);for(int i=0;i<4;i++){TEST_ASSERT(espeak_rs_compiler_program_end(leases[i])==0);} TEST_ASSERT(espeak_rs_compiler_definition(&d,1,&out)==9&&out.status==(int32_t)0xa5a5a5a5);
}
int main(void){for(unsigned i=0;i<1000;i++){Definitions(i);}Registry();Edges();Guards();printf("1000 full C/Rust definition episodes and registry bounds: %u comparisons; native callback/name/output guards passed\n",comparisons);return 0;}
