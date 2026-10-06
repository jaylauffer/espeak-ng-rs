/* Bounded SSML helpers against independently extracted retained C.
 * SPDX-License-Identifier: GPL-3.0-or-later */
#include "config.h"
#include "test_assert.h"
#include <ctype.h>
#include <limits.h>
#include <locale.h>
#include <math.h>
#include <stdint.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <wchar.h>
#include <wctype.h>
#include <ucd/ucd.h>
#include "common.h"
#include "mnemonics.h"
#include "ssml.h"
#include "translate.h"
#include "rust_data.h"
#include "speech.h"
static espeak_VOICE *ReferenceSelectName(espeak_VOICE **voices,const char *name);
static const char *ReferenceSelect(espeak_VOICE *choice,int *found);
#define SelectVoiceByName ReferenceSelectName
#define SelectVoice ReferenceSelect
static int reference_punctuation, reference_capitals;
#define option_punctuation reference_punctuation
#define option_capitals reference_capitals
#define ParseSsmlReference ReferenceParseSsmlReference
#include "ssml_reference.inc"
#undef ParseSsmlReference
#undef option_punctuation
#undef option_capitals
#undef SelectVoiceByName
#undef SelectVoice
#include "ssml_tag_reference.inc"
static Translator reference_translator;
static Translator *directive_translator=&reference_translator;
#define translator directive_translator
#define option_punctuation reference_punctuation
#define option_capitals reference_capitals
#include "ssml_directive_reference.inc"
#undef translator
#undef option_punctuation
#undef option_capitals
static int WideSpace(uint32_t c) {return iswspace((wint_t)c)!=0;}
#include "ssml_text_reference.inc"
static SPEED_FACTORS reference_speed;
static int next_clause_pause,next_pause,rate_calls;
static unsigned rate_output_hash;
static char *rate_output;
static int *rate_offset;
static espeak_ERROR ReferenceRate(espeak_PARAMETER parameter,int value,int relative)
{
	TEST_ASSERT(parameter==espeakRATE && relative==0);(void)value;rate_calls++;
	for(int i=0;i<=*rate_offset;i++)rate_output_hash=rate_output_hash*33+(unsigned char)rate_output[i];
	reference_speed.clause_pause_factor=next_clause_pause;reference_speed.pause_factor=next_pause;return EE_OK;
}
#define speed reference_speed
#define espeak_SetParameter ReferenceRate
#include "ssml_break_reference.inc"
#undef espeak_SetParameter
#undef speed
static int voice_call_count,voice_call_tags[3],voice_call_counts[3],voice_call_flags[3];
static int ReferenceVoiceCall(wchar_t *pw,int tag_type,SSML_STACK *sp,SSML_STACK *frames,int count,char *current,espeak_VOICE *base,char *variant)
{
	(void)pw;(void)sp;(void)frames;(void)current;(void)base;(void)variant;
	TEST_ASSERT(voice_call_count<3);int i=voice_call_count++;voice_call_tags[i]=tag_type;voice_call_counts[i]=count;return voice_call_flags[i];
}
#define GetVoiceAttributes ReferenceVoiceCall
#include "ssml_voice_directive_reference.inc"
#undef GetVoiceAttributes
static char resource_names[512],resource_skip[50];
static int resource_index,resource_uri_result;
static unsigned resource_trace;
static void ResourceTrace(const char *value)
{
	for(const unsigned char *p=(const unsigned char *)value;*p;p++)resource_trace=resource_trace*33+*p;
	resource_trace=resource_trace*33+7;
}
static int ResourceAppend(const char *name,int wide)
{
	TEST_ASSERT(wide==0);resource_trace=resource_trace*33+1;ResourceTrace(name);
	if(resource_index>=0)strcpy(resource_names+resource_index,name);
	return resource_index;
}
static int ResourceLoad(const char *name)
{
	resource_trace=resource_trace*33+2;ResourceTrace(name);return resource_index;
}
static int ResourceUri(int kind,const char *uri,const char *base)
{
	TEST_ASSERT(kind==1);resource_trace=resource_trace*33+3;ResourceTrace(uri);
	resource_trace=resource_trace*33+(base==NULL?0:1);if(base)ResourceTrace(base);return resource_uri_result;
}
static int (*resource_callback)(int,const char *,const char *);
#define namedata resource_names
#define skip_marker resource_skip
#define AddNameData ResourceAppend
#define LoadSoundFile2 ResourceLoad
#define uri_callback resource_callback
#include "ssml_resource_reference.inc"
#undef uri_callback
#undef LoadSoundFile2
#undef AddNameData
#undef skip_marker
#undef namedata
static int ByteSpace(uint32_t c) {return c<=255 && isspace((unsigned char)c)!=0;}
static unsigned seed=0x72c184abu;
static unsigned next(void){seed^=seed<<13;seed^=seed>>17;seed^=seed<<5;return seed;}
static size_t comparisons,numbers,copies,attributes,references,keys,parameters,pops,pushes,voice_choices,float_values,prosody_values,prosody_parameters,voice_frames,voice_changes,tags,directives,text_directives,text_capacity_rejections;
static size_t breaks,voice_directives;
static size_t resource_requests,resource_directives;
static RustSsmlVoiceChoice captured_choice;
static const char *selected_voice;
static unsigned resolution_order;
static espeak_VOICE *ReferenceSelectName(espeak_VOICE **voices,const char *name)
{
	(void)voices;
	for(const unsigned char *p=(const unsigned char *)name;*p;p++)resolution_order=resolution_order*33+*p;
	resolution_order=resolution_order*33+7;
	static espeak_VOICE first={.identifier="gmw/en"},second={.identifier="roa/fr"};
	if(strcmp(name,"known-en")==0)return &first;
	if(strcmp(name,"known-fr")==0)return &second;
	return NULL;
}
static const char *ReferenceSelect(espeak_VOICE *choice,int *found)
{
	memset(&captured_choice,0,sizeof(captured_choice));
	strcpy((char *)captured_choice.name,choice->name);
	strcpy((char *)captured_choice.identifier,choice->identifier);
	strcpy((char *)captured_choice.language,choice->languages);
	captured_choice.gender=choice->gender;captured_choice.age=choice->age;captured_choice.variant=choice->variant;
	*found=selected_voice!=NULL;
	return selected_voice;
}
static int32_t ResolveName(const unsigned char (*name)[40],unsigned char (*identifier)[40])
{
	espeak_VOICE *voice=ReferenceSelectName(NULL,(const char *)*name);
	if(voice==NULL)return 1;
	strcpy((char *)*identifier,voice->identifier);return 0;
}

static void helpers(void)
{
	static const MNEM_TAB table[]={{"male",1},{"male",7},{"female",2},{"",3},{NULL,-1}};
	static const wchar_t *words[]={L"male\"",L"male'",L"male",L"female\"tail",L"\"",L"male'ignored",L"maleSuffix'",L"unknown\"",NULL};
	for(int i=0;i<100000;i++) {
		const wchar_t *word=words[next()%(sizeof(words)/sizeof(words[0]))];
		size_t n=word?wcslen(word)+1:0;
		for(int j=0;table[j].mnem;j++) {
			TEST_ASSERT(espeak_rs_ssml_compare(word,n,table[j].mnem)==attrcmp(word,table[j].mnem));comparisons++;
		}
		TEST_ASSERT(espeak_rs_ssml_lookup(word,n,table)==attrlookup(word,table));
		wchar_t number[80]={0};int value=next()%2000000;int kind=next()%3;int fallback=(int)(next()%401)-200;
		const wchar_t *suffixes[]={L"s'",L"S\"",L"ms'",L"x",L"",L".5s"};
		swprintf(number,80,L"%d%ls",value,suffixes[next()%6]);
		if(i%13==0)number[0]='-';
		if(i%17==0)number[0]=' ';
		const wchar_t *source=i%31==0?NULL:number;
		TEST_ASSERT(espeak_rs_ssml_number(source,source?wcslen(source)+1:0,fallback,kind)==attrnumber(source,fallback,kind));numbers++;
		wchar_t copy[130]={0};wchar_t quote=i%3==0?'"':i%3==1?'\'':' ';
		copy[0]=quote;
		static const uint32_t codes[]={65,32,47,92,39,34,0x7f,0x80,0x7ff,0x800,0xd800,0xffff,0x10000,0x10ffff,0x110000};
		int units=next()%120;
		for(int j=1;j<=units;j++)copy[j]=(wchar_t)codes[next()%(quote==' '?6:sizeof(codes)/sizeof(codes[0]))];
		unsigned char actual[520],expected[520];memset(actual,0xa5,sizeof(actual));memset(expected,0xa5,sizeof(expected));
		int capacity=1+next()%sizeof(actual);
		int want=attrcopy_utf8((char *)expected,copy+1,capacity);
		int got=espeak_rs_ssml_copy(copy+1,wcslen(copy+1)+1,(uint32_t)quote,ByteSpace,actual,capacity);
		TEST_ASSERT(got==want);TEST_ASSERT(memcmp(actual,expected,sizeof(actual))==0);copies++;
	}
}
static void scans(void)
{
	static const wchar_t *forms[]={L" %ls='%d' tail='x'",L" %ls = \"%d\" /",L" %ls=%d /",L" %lsSuffix=%d /",L" x='a' %ls /",L" %ls ",L" %ls=",L" %ls='",L" x=' %ls=%d'",L" %ls='/quoted'"};
	static const char *names[]={"name","xml:lang","age","variant","n","missing"};
	for(int i=0;i<100000;i++) {
		wchar_t text[256]={0},name[30]={0};const char *key=names[next()%6];
		for(size_t j=0;j<strlen(key);j++)name[j]=(unsigned char)key[j];
		swprintf(text,256,forms[next()%10],name,(int)(next()%1000));
		const char *wanted=names[next()%6];
		const wchar_t *expect=GetSsmlAttribute(text+1,wanted);
		size_t offset=12345;
		int result=espeak_rs_ssml_attribute(text,256,1,wanted,WideSpace,&offset);
		if(expect==NULL && result!=1)fprintf(stderr,"attribute mismatch '%ls' name '%s' status %d offset %zu\n",text,wanted,result,offset);
		if(expect==NULL){TEST_ASSERT(result==1);}
		else {
			TEST_ASSERT(result==0);
			if(offset==SIZE_MAX){TEST_ASSERT(expect[0]==0);}
			else {TEST_ASSERT(expect==text+offset);}
		}
		attributes++;
	}
}
static void refs(void)
{
	static const char *fixed[]={"","gt","lt","amp","quot","nbsp","apos","unknown","GT","#","#x","#junk","#xjunk","#  ","#+","#-","#x0x","#x0xz","#x-0X","#x0Xf","#Xff","#-2147483648","#2147483647","#xffffffff","#x-ffffffff"};
	for(int i=0;i<100000;i++) {
		char input[100];int value=(int)(next()%2000000000u);if(i%2)value=-value;
		if(i%3==0)snprintf(input,sizeof(input),"# \t%+dtrail",value);
		else if(i%3==1)snprintf(input,sizeof(input),"#x +%x!",next());
		else snprintf(input,sizeof(input),"%s",fixed[next()%(sizeof(fixed)/sizeof(fixed[0]))]);
		int a=(int)(next()%1000),b=i%2?0:7,ea=a,eb=b;
		int expected=ReferenceParseSsmlReference(input,&ea,&eb);
		int actual=espeak_rs_ssml_reference(input,&a,&b,ByteSpace);
		if(actual!=expected||a!=ea||b!=eb)fprintf(stderr,"reference mismatch '%s': %d %d %d / %d %d %d\n",input,actual,a,b,expected,ea,eb);
		TEST_ASSERT(actual==expected);TEST_ASSERT(a==ea);TEST_ASSERT(b==eb);
		int ca=123,cb=0,da=ca,db=cb;
		TEST_ASSERT(ParseSsmlReference(input,&ca,&cb)==ReferenceParseSsmlReference(input,&da,&db));
		TEST_ASSERT(ca==da&&cb==db);references++;
		const char *names[]={"space ","tab ","underscore ","double-quote ","space","Space ","tab x",""};
		unsigned char out[80],old[80];memset(out,0xa5,80);memset(old,0xa5,80);
		int index=next()%20;strcpy((char *)out+index,names[next()%8]);memcpy(old,out,80);
		int ax=777,ex=777;
		int code=espeak_rs_ssml_key(out+index,index,&ax);
		TEST_ASSERT(code==ReplaceKeyName((char *)old,index,&ex));
		TEST_ASSERT(ax==ex);TEST_ASSERT(memcmp(out,old,80)==0);keys++;
	}
}
static void guards(void)
{
	wchar_t wide[]={65,65};unsigned char out[20];memset(out,0xa5,20);
	TEST_ASSERT(espeak_rs_ssml_copy(wide,2,34,ByteSpace,out,20)==-1);
	for(int i=0;i<20;i++)TEST_ASSERT(out[i]==0xa5);
	TEST_ASSERT(espeak_rs_ssml_copy(wide,2,34,ByteSpace,out,0)==-1);
	TEST_ASSERT(espeak_rs_ssml_number(L"2147483648",11,77,0)==77);
	TEST_ASSERT(espeak_rs_ssml_number(L"2147484s",9,77,1)==77);
	size_t offset=777;TEST_ASSERT(espeak_rs_ssml_attribute(wide,2,0,"a",WideSpace,&offset)==2);TEST_ASSERT(offset==777);
	int a=77,b=88;TEST_ASSERT(espeak_rs_ssml_reference("#2147483648",&a,&b,ByteSpace)==-1);TEST_ASSERT(a==77&&b==88);
}
static void stack_helpers(void)
{
	TEST_ASSERT(N_SPEECH_PARAM==15);TEST_ASSERT(N_PARAM_STACK==20);
	TEST_ASSERT(sizeof(PARAM_STACK)==64);TEST_ASSERT(sizeof(RustSsmlParameters)==160);
	for(int trial=0;trial<100000;trial++) {
		PARAM_STACK actual[20],expected[20];
		int count=next()%21;
		for(int i=0;i<20;i++) {
			actual[i].type=next()%17;
			for(int j=0;j<15;j++) {
				static const int values[]={-1,0,1,2,100,500,INT_MAX,INT_MIN,-17};
				actual[i].parameter[j]=values[next()%9];
			}
		}
		memcpy(expected,actual,sizeof(actual));
		int tag=(int)(next()%50)-3;
		int ac=count,ec=count;
		if(count<20) {
			PARAM_STACK *frame=PushParamStack(tag,&ec,expected);
			int index=espeak_rs_ssml_push(actual,&ac,tag);
			TEST_ASSERT(index==frame-expected);TEST_ASSERT(ac==ec);TEST_ASSERT(memcmp(actual,expected,sizeof(actual))==0);pushes++;
		} else {
			TEST_ASSERT(espeak_rs_ssml_push(actual,&ac,tag)==-1);
			TEST_ASSERT(ac==count);TEST_ASSERT(memcmp(actual,expected,sizeof(actual))==0);
		}
		// Use the same initialized snapshot for independent process/pop plans.
		int current[15],reference[15];
		for(int i=0;i<15;i++)current[i]=(int)(next()%700)-20;
		for(int pop=0;pop<=1;pop++) {
			memcpy(reference,current,sizeof(current));
			int punctuation=(int)(next()%8)-3,capitals=(int)(next()%50)-3;
			reference_punctuation=punctuation;reference_capitals=capitals;
			unsigned char out[200],old[200];memset(out,0xa5,sizeof(out));memset(old,0xa5,sizeof(old));
			int offset=next()%20,old_offset=offset,new_count=ac;
			if(pop)PopParamStack(tag,(char *)old,&old_offset,&new_count,actual,reference,sizeof(old));
			else ProcessParamStack((char *)old,&old_offset,ac,actual,reference,sizeof(old));
			RustSsmlParameters effect;
			TEST_ASSERT(espeak_rs_ssml_parameters(actual,ac,(const int32_t (*)[15])current,punctuation,capitals,pop,tag,sizeof(out)-offset,&effect)==0);
			TEST_ASSERT(effect.changed<=1);TEST_ASSERT(effect.length<80);
			if(effect.changed)memcpy(out+offset,effect.commands,effect.length+1);
			TEST_ASSERT(offset+(int)effect.length==old_offset);TEST_ASSERT(memcmp(out,old,sizeof(out))==0);
			TEST_ASSERT(memcmp(effect.values,reference,sizeof(reference))==0);
			TEST_ASSERT(effect.punctuation==reference_punctuation);TEST_ASSERT(effect.capitals==reference_capitals);
			TEST_ASSERT(effect.count==(unsigned)new_count);
			if(effect.changed) {
				RustSsmlParameters rejected,before;memset(&rejected,0xa5,sizeof(rejected));before=rejected;
				TEST_ASSERT(espeak_rs_ssml_parameters(actual,ac,(const int32_t (*)[15])current,punctuation,capitals,pop,tag,effect.length,&rejected)==1);
				TEST_ASSERT(memcmp(&rejected,&before,sizeof(rejected))==0);
			}
			if(pop)pops++;else parameters++;
		}
	}
	PARAM_STACK frames[20]={0};int current[15]={0};RustSsmlParameters result,before;memset(&result,0xa5,sizeof(result));before=result;
	TEST_ASSERT(espeak_rs_ssml_parameters(frames,-1,(const int32_t (*)[15])current,7,8,0,0,100,&result)==1);
	TEST_ASSERT(espeak_rs_ssml_parameters(frames,21,(const int32_t (*)[15])current,7,8,0,0,100,&result)==1);
	TEST_ASSERT(espeak_rs_ssml_parameters(frames,1,(const int32_t (*)[15])current,7,8,2,0,100,&result)==1);
	TEST_ASSERT(memcmp(&result,&before,sizeof(result))==0);
	int count=-1;TEST_ASSERT(espeak_rs_ssml_push(frames,&count,3)==-1);TEST_ASSERT(count==-1);
}
static void voice_stack_helpers(void)
{
	TEST_ASSERT(sizeof(SSML_STACK)==76);TEST_ASSERT(sizeof(RustSsmlVoiceChoice)==132);
	unsigned char previous[40]={0};
	static const char *names[]={"","known-en","known-fr","unknown","missing","known-en "};
	static const char *languages[]={"","en","fr","en-gb","de","en-US"};
	static const char *selected[]={"en","fr+f1","123456789012345678901234567890123456789",NULL,"long-identifier-for-tests"};
	for(int trial=0;trial<100000;trial++) {
		SSML_STACK frames[20]={0};int count=1+next()%20;
		for(int i=0;i<count;i++) {
			strcpy(frames[i].voice_name,names[next()%6]);strcpy(frames[i].language,languages[next()%6]);
			frames[i].tag_type=next()%16;
			frames[i].voice_gender=(int)(next()%261)-2;
			frames[i].voice_age=(int)(next()%521)-3;
			frames[i].voice_variant_number=(int)(next()%261)-2;
		}
		const char *base_languages=trial%3==0?"\0":trial%3==1?"\x05""en-gb\0\x08""en\0\x09""de\0\0":"\x05""fr\0\x08""en\0\0";
		espeak_VOICE base={.name="base",.identifier="base",.languages=base_languages,.gender=(unsigned char)(next()%3)};
		char variant[40]={0};
		if(trial%3==1)strcpy(variant,"m2");
		if(trial%3==2)memset(variant,'v',39);
		selected_voice=selected[next()%5];
		resolution_order=0;
		const char *expected=VoiceFromStack(frames,count,&base,variant);
		char expected_id[80];strcpy(expected_id,expected);
		unsigned order=resolution_order;resolution_order=0;
		RustSsmlVoiceChoice actual;
		TEST_ASSERT(espeak_rs_ssml_voice_choice(frames,count,&base,&previous,ResolveName,&actual)==0);
		TEST_ASSERT(resolution_order==order);
		TEST_ASSERT(strcmp((char *)actual.name,(char *)captured_choice.name)==0);
		TEST_ASSERT(strcmp((char *)actual.identifier,(char *)captured_choice.identifier)==0);
		TEST_ASSERT(strcmp((char *)actual.language,(char *)captured_choice.language)==0);
		TEST_ASSERT(actual.gender==captured_choice.gender);TEST_ASSERT(actual.age==captured_choice.age);TEST_ASSERT(actual.variant==captured_choice.variant);
		memcpy(previous,actual.identifier,sizeof(previous));
		unsigned char native_id[40];memset(native_id,0xa5,sizeof(native_id));
		int changed=selected_voice?espeak_rs_ssml_base_variant(selected_voice,actual.gender,base.gender,variant,&native_id):0;
		const char *actual_id=selected_voice==NULL?"default":changed==1?(char *)native_id:selected_voice;
		TEST_ASSERT(strcmp(actual_id,expected_id)==0);
		if(changed==0)for(int i=0;i<40;i++){TEST_ASSERT(native_id[i]==0xa5);}
		voice_choices++;
	}
	SSML_STACK frame={0};strcpy(frame.voice_name,"known-en");
	espeak_VOICE base={.name="base",.identifier="base",.languages="\x05""en\0\0"};
	RustSsmlVoiceChoice output,before;memset(&output,0xa5,sizeof(output));before=output;
	resolution_order=0;
	TEST_ASSERT(espeak_rs_ssml_voice_choice(&frame,0,&base,&previous,ResolveName,&output)==1);
	TEST_ASSERT(espeak_rs_ssml_voice_choice(&frame,21,&base,&previous,ResolveName,&output)==1);
	memset(frame.voice_name,'x',40);
	TEST_ASSERT(espeak_rs_ssml_voice_choice(&frame,1,&base,&previous,ResolveName,&output)==1);
	TEST_ASSERT(resolution_order==0);TEST_ASSERT(memcmp(&output,&before,sizeof(output))==0);
}
static uint32_t DecimalPoint(void)
{
	wchar_t point='.';mbstate_t state={0};const char *text=localeconv()->decimal_point;
	size_t size=mbrtowc(&point,text,strlen(text),&state);
	return size==(size_t)-1||size==(size_t)-2||size==0?'.':(uint32_t)point;
}
static void compare_float(const wchar_t *input)
{
	wchar_t *end;double expected=wcstod(input,&end),actual=777.0;
	size_t tail=777;
	int result=espeak_rs_ssml_float(input,wcslen(input)+1,DecimalPoint(),WideSpace,&actual,&tail);
	if(end==input) {
		TEST_ASSERT(result==1);TEST_ASSERT(actual==777.0);TEST_ASSERT(tail==777);
	} else {
		TEST_ASSERT(result==0);TEST_ASSERT(tail==(size_t)(end-input));
		uint64_t a,e;memcpy(&a,&actual,8);memcpy(&e,&expected,8);
		if(a!=e&&!isnan(expected))fprintf(stderr,"float mismatch '%ls': %.17g %016llx / %.17g %016llx\n",input,actual,(unsigned long long)a,expected,(unsigned long long)e);
		TEST_ASSERT(a==e||isnan(expected));
	}
	float_values++;
}
static int int_value_defined(double value)
{
	return isfinite(value)&&trunc(value)>=INT_MIN&&trunc(value)<=INT_MAX;
}
static int prosody_defined(int type,const wchar_t *text)
{
	while(iswspace(*text))text++;
	int sign=0;if(*text=='+'){text++;sign=1;}if(*text=='-'){text++;sign=-1;}
	wchar_t *tail;double value=wcstod(text,&tail);
	if(tail==text)return 1;
	if(!isfinite(value))return 0;
	if(*tail=='%')return int_value_defined(sign?100+sign*value:value);
	if(tail[0]=='s'&&tail[1]=='t')return int_value_defined(pow(2.0,(value*sign)/12)*100);
	if(type==espeakRATE) {
		double product=(sign?sign:1)*value*100;
		return int_value_defined(product)&&(!sign||int_value_defined(100+trunc(product)));
	}
	return int_value_defined(value);
}
static void prosody_helpers(void)
{
	const wchar_t *signs[]={L"",L"+",L"-",L"+-",L"--",L"++",L"+ "};
	const wchar_t *tails[]={L"%",L"st",L"",L"ST",L"ms",L"x"};
	const wchar_t *fixed[]={L"default'",L"x-low'",L"slow'",L"silent'",L"medium\"",L"x-loud'",L"x-fast'",L"bad",L"",L".5",L"0x",L"0xz",L"0x.p3",L"0x1p+",L"1e+",L"1e",L".e2",L"nan",L"infinity",L"-inf",L"NAN(test)",L"0x1.00000000000008p0",L"0x1.00000000000018p0",L"0x1p-1075",L"0x1.8p-1075"};
	for(int trial=0;trial<200000;trial++) {
		wchar_t input[513]={0};
		const wchar_t *sign=signs[next()%7],*tail=tails[next()%6];
		if(trial%4==0)swprintf(input,513,L" \t%ls%d.%06d%ls'",sign,(int)(next()%61),(int)(next()%1000000),tail);
		else if(trial%4==1)swprintf(input,513,L"%ls0x%x.%08xp%d%ls'",sign,next()%8,next(),(int)(next()%12)-10,tail);
		else if(trial%4==2)swprintf(input,513,L"%ls%.17g%ls'",sign,(double)(next()%60000)/997.0,tail);
		else wcscpy(input,fixed[next()%(sizeof(fixed)/sizeof(fixed[0]))]);
		compare_float(input);
		int type=1+next()%4;
		if(!prosody_defined(type,input))continue;
		int value=777,kind=attr_prosody_value(type,input,&value);
		RustSsmlProsody effect={77,88};
		TEST_ASSERT(espeak_rs_ssml_prosody(type,input,wcslen(input)+1,DecimalPoint(),WideSpace,&effect)==0);
		if(effect.kind!=kind||effect.value!=value)fprintf(stderr,"prosody mismatch '%ls' param %d: %d %d / %d %d\n",input,type,effect.kind,effect.value,kind,value);
		TEST_ASSERT(effect.kind==kind);TEST_ASSERT(effect.value==value);prosody_values++;
		PARAM_STACK base={0},output={0};int current[15]={0};
		base.parameter[type]=(int)(next()%501)-100;current[type]=(int)(next()%501)-100;output.parameter[type]=77;
		// Exclude retained C signed-product/addition overflow from the oracle.
		int64_t relative=(int64_t)current[type]*value;
		int64_t absolute=(int64_t)current[type]+(int64_t)value*kind;
		if((kind==2&&(relative<INT_MIN||relative>INT_MAX))||((kind==1||kind==-1)&&(absolute<INT_MIN||absolute>INT_MAX)))continue;
		SetProsodyParameter(type,input,&output,&base,current);
		int32_t actual=777;
		TEST_ASSERT(espeak_rs_ssml_prosody_parameter(type,input,wcslen(input)+1,base.parameter[type],current[type],DecimalPoint(),WideSpace,&actual)==0);
		TEST_ASSERT(actual==output.parameter[type]);prosody_parameters++;
	}
	// Arbitrary hexadecimal mantissas and powers exercise correct rounding,
	// sticky bits, normal/subnormal transitions, overflow and signed zero.
	for(int trial=0;trial<200000;trial++) {
		char text[513];size_t length=0;if(trial%2)text[length++]='-';text[length++]='0';text[length++]='x';
		int digits=1+next()%120,point=next()%(digits+1);
		for(int i=0;i<digits;i++){if(i==point)text[length++]='.';text[length++]="0123456789abcdef"[next()%16];}
		if(point==digits)text[length++]='.';
		snprintf(text+length,sizeof(text)-length,"p%+dtrail'",(int)(next()%4401)-2200);
		wchar_t input[513]={0};for(size_t i=0;i<strlen(text);i++)input[i]=(unsigned char)text[i];
		compare_float(input);
	}
	RustSsmlProsody out={77,88};
	TEST_ASSERT(espeak_rs_ssml_prosody(3,L"nan",4,DecimalPoint(),WideSpace,&out)==1);TEST_ASSERT(out.kind==77&&out.value==88);
	TEST_ASSERT(espeak_rs_ssml_prosody(3,L"2147483648",11,DecimalPoint(),WideSpace,&out)==1);TEST_ASSERT(out.kind==77&&out.value==88);
	int32_t scalar=77;
	TEST_ASSERT(espeak_rs_ssml_prosody_parameter(3,L"high'",6,INT_MAX,50,DecimalPoint(),WideSpace,&scalar)==1);TEST_ASSERT(scalar==77);
	TEST_ASSERT(espeak_rs_ssml_prosody_parameter(0,L"high'",6,100,50,DecimalPoint(),WideSpace,&scalar)==1);TEST_ASSERT(scalar==77);
	wchar_t oversized[514]={0};double number=777;size_t tail=777;
	TEST_ASSERT(espeak_rs_ssml_float(oversized,514,DecimalPoint(),WideSpace,&number,&tail)==2);TEST_ASSERT(number==777&&tail==777);
	wchar_t unterminated[2]={49,50};
	TEST_ASSERT(espeak_rs_ssml_float(unterminated,2,DecimalPoint(),WideSpace,&number,&tail)==2);TEST_ASSERT(number==777&&tail==777);
}
static void numeric_locale_helpers(void)
{
	const char *locale=setlocale(LC_NUMERIC,"de_DE.UTF-8");
	if(locale==NULL)locale=setlocale(LC_NUMERIC,"fr_FR.UTF-8");
	if(locale==NULL)return;
	printf("Checked LC_NUMERIC=%s decimal U+%04x\n",locale,(unsigned)DecimalPoint());
	for(int trial=0;trial<20000;trial++) {
		wchar_t input[100]={0};swprintf(input,100,L"+%.17g%%'",(double)(next()%30000)/99.0);
		compare_float(input);
		int type=1+next()%4,value=77;
		int kind=attr_prosody_value(type,input,&value);
		RustSsmlProsody effect={77,88};
		TEST_ASSERT(espeak_rs_ssml_prosody(type,input,wcslen(input)+1,DecimalPoint(),WideSpace,&effect)==0);
		TEST_ASSERT(effect.kind==kind&&effect.value==value);prosody_values++;
		PARAM_STACK base={0},output={0};int current[15]={0};base.parameter[type]=100;current[type]=100;
		SetProsodyParameter(type,input,&output,&base,current);int32_t actual=77;
		TEST_ASSERT(espeak_rs_ssml_prosody_parameter(type,input,wcslen(input)+1,100,100,DecimalPoint(),WideSpace,&actual)==0);
		TEST_ASSERT(actual==output.parameter[type]);prosody_parameters++;
	}
	TEST_ASSERT(setlocale(LC_NUMERIC,"C")!=NULL);
}
static void voice_attribute_helpers(void)
{
	TEST_ASSERT(sizeof(RustSsmlVoiceFrame)==88);
	unsigned char previous[40];memcpy(previous,captured_choice.identifier,40);
	const wchar_t *forms[]={L" name='%ls' xml:lang='%ls' gender='%ls' age='%d' variant='%d'",L" name=\"%ls\" xml:lang=%ls gender=%ls age=%d variant=%d /",L" name='%ls'",L" xml:lang='%2$ls'",L" ",L" age='%4$d'",L" xml:lang=''",L" xml:lang='/' name='/test'"};
	const wchar_t *names[]={L"",L"known-en",L"known-fr",L"unknown",L"missing"};
	const wchar_t *languages[]={L"",L"en",L"fr",L"en-gb",L"de"};
	const wchar_t *genders[]={L"male",L"female",L"neutral",L"unknown",L""};
	for(int trial=0;trial<100000;trial++) {
		SSML_STACK actual[20]={0},old[20];int count=1+next()%19;
		for(int i=0;i<20;i++) {
			actual[i].tag_type=next()%16;actual[i].voice_variant_number=next()%7;actual[i].voice_age=next()%80;actual[i].voice_gender=next()%4;
			strcpy(actual[i].voice_name,trial%3?"known-en":"unknown");strcpy(actual[i].language,trial%3?"en":"fr");
		}
		memcpy(old,actual,sizeof(old));
		wchar_t text[256]={0};
		swprintf(text,256,forms[next()%8],names[next()%5],languages[next()%5],genders[next()%5],(int)(next()%140),(int)(next()%12));
		int kind=trial%3==0?SSML_VOICE:trial%3==1?SSML_SENTENCE:SSML_SPEAK;
		if(trial%4==0)kind+=SSML_CLOSE;
		char variant[40]="m2";
		espeak_VOICE base={.name="base",.identifier="base",.languages="\x05""en-gb\0\x08""en\0\0",.gender=1};
		selected_voice=trial%5==0?NULL:trial%5==1?"en":trial%5==2?"fr+f1":"test";
		char expected[40],current[40];memset(current,0xa5,40);strcpy(current,trial%2?"en":"default");memcpy(expected,current,40);
		resolution_order=0;
		int expected_flag=GetVoiceAttributes(text+1,kind,old+count-1,old,count,expected,&base,variant);
		unsigned order=resolution_order;resolution_order=0;
		RustSsmlVoiceFrame change;
		TEST_ASSERT(espeak_rs_ssml_voice_frame(text,wcslen(text)+1,1,kind,count,WideSpace,ByteSpace,&change)==0);
		if(change.action==2) {
			TEST_ASSERT(change.index==(unsigned)count);TEST_ASSERT(change.count==(unsigned)count+1);
			actual[change.index]=change.frame;
			TEST_ASSERT(change.frame.tag_type==old[count].tag_type);
			TEST_ASSERT(change.frame.voice_variant_number==old[count].voice_variant_number);
			TEST_ASSERT(change.frame.voice_gender==old[count].voice_gender);TEST_ASSERT(change.frame.voice_age==old[count].voice_age);
			TEST_ASSERT(strcmp(change.frame.voice_name,old[count].voice_name)==0);TEST_ASSERT(strcmp(change.frame.language,old[count].language)==0);
		}
		int flag=0;
		if(change.action!=0) {
			RustSsmlVoiceChoice choice;
			TEST_ASSERT(espeak_rs_ssml_voice_choice(actual,change.count,&base,&previous,ResolveName,&choice)==0);
			TEST_ASSERT(resolution_order==order);
			TEST_ASSERT(strcmp((char *)choice.name,(char *)captured_choice.name)==0);
			TEST_ASSERT(strcmp((char *)choice.identifier,(char *)captured_choice.identifier)==0);
			TEST_ASSERT(strcmp((char *)choice.language,(char *)captured_choice.language)==0);
			TEST_ASSERT(choice.gender==captured_choice.gender&&choice.age==captured_choice.age&&choice.variant==captured_choice.variant);
			memcpy(previous,choice.identifier,40);
			unsigned char id[40];int changed=selected_voice?espeak_rs_ssml_base_variant(selected_voice,choice.gender,base.gender,variant,&id):0;
			const char *selected=selected_voice==NULL?"default":changed==1?(char *)id:selected_voice;
			flag=espeak_rs_ssml_voice_changed((unsigned char *)current,selected)==1?CLAUSE_TYPE_VOICE_CHANGE:0;
		} else {TEST_ASSERT(order==0);}
		TEST_ASSERT(flag==expected_flag);TEST_ASSERT(memcmp(current,expected,40)==0);voice_frames++;
		unsigned char destination[40];memset(destination,0xa5,40);strcpy((char *)destination,trial%2?"en":"fr");
		unsigned char snapshot[40];memcpy(snapshot,destination,40);
		const char *selected=trial%2?"en":"de";
		int changed=espeak_rs_ssml_voice_changed(destination,selected);
		TEST_ASSERT(changed==(strcmp((char *)snapshot,selected)!=0));
		if(changed){strcpy((char *)snapshot,selected);}
		TEST_ASSERT(memcmp(destination,snapshot,40)==0);voice_changes++;
	}
	RustSsmlVoiceFrame change,before;memset(&change,0xa5,sizeof(change));before=change;
	TEST_ASSERT(espeak_rs_ssml_voice_frame(L" name='x'",10,1,SSML_VOICE,20,WideSpace,ByteSpace,&change)==1);
	TEST_ASSERT(memcmp(&change,&before,sizeof(change))==0);
	unsigned char current[40]={0};strcpy((char *)current,"en");unsigned char old[40];memcpy(old,current,40);
	TEST_ASSERT(espeak_rs_ssml_voice_changed(current,"1234567890123456789012345678901234567890")==-1);TEST_ASSERT(memcmp(current,old,40)==0);
	TEST_ASSERT(espeak_rs_ssml_voice_changed(current,(char *)current)==0);TEST_ASSERT(memcmp(current,old,40)==0);
}
static int ByteLower(uint32_t c){return tolower((unsigned char)c);}
static void tag_helpers(void)
{
	TEST_ASSERT(sizeof(RustSsmlTag)==24);
	for(int trial=0;trial<200000;trial++) {
		wchar_t input[501]={0},reference[501];
		const char *name=trial%3?ssmltags[next()%32].mnem:"unknown-long-name-for-attribute-boundary";
		int close=next()%2,self=next()%2;size_t index=0;
		if(close)input[index++]='/';
		for(size_t j=0;j<strlen(name);j++)input[index++]=trial%4?toupper((unsigned char)name[j]):(unsigned char)name[j];
		const wchar_t *tails[]={L"",L" ",L" name='test'",L"\tXML:LANG='en'",L"   ",L"xxxxxxxxxxxxxxxxxxxxxxxxxxxxxxx",L" / ",L"\n"};
		wcscpy(input+index,tails[next()%8]);index=wcslen(input);
		if(self){input[index++]='/';input[index]=0;}
		if(trial%29==0)input[0]=(wchar_t)(0x10000+(unsigned char)input[0]);
		if(trial%43==0){input[0]=(wchar_t)0x10000;}
		memcpy(reference,input,sizeof(input));
		char old[20],actual[20];memset(old,0xa5,20);memset(actual,0xa5,20);int offset=0;
		RustSsmlTag expected={0},parsed;
		TEST_ASSERT(ReferenceTag(reference,old,&offset,&expected)==1);
		TEST_ASSERT(espeak_rs_ssml_tag(input,wcslen(input)+1,CHAR_MIN<0,WideSpace,ByteLower,&parsed)==0);
		TEST_ASSERT(memcmp(&parsed,&expected,sizeof(parsed))==0);
		if(parsed.slash_index!=UINT32_MAX)input[parsed.slash_index]=' ';
		if(parsed.separator)actual[0]=' ';
		TEST_ASSERT(memcmp(input,reference,sizeof(input))==0);TEST_ASSERT(memcmp(actual,old,20)==0);tags++;
	}
	RustSsmlTag parsed,before;memset(&parsed,0xa5,sizeof(parsed));before=parsed;
	TEST_ASSERT(espeak_rs_ssml_tag(L"",1,CHAR_MIN<0,WideSpace,ByteLower,&parsed)==1);
	TEST_ASSERT(espeak_rs_ssml_tag(L"b",2,2,WideSpace,ByteLower,&parsed)==1);
	wchar_t missing[2]={65,66};TEST_ASSERT(espeak_rs_ssml_tag(missing,2,CHAR_MIN<0,WideSpace,ByteLower,&parsed)==1);
	TEST_ASSERT(memcmp(&parsed,&before,sizeof(parsed))==0);
}
static void directive_helpers(void)
{
	const wchar_t *prosody[]={L" ",L" rate='fast'",L" volume='loud' pitch='+12st' range='50%'",L" pitch='-10' rate='+0.25'",L" rate='0x1.8p0' volume='x-soft'",L" rate='slow' pitch='high' range='x-high'"};
	const wchar_t *style[]={L" ",L" field='punctuation' mode='all'",L" field='punctuation' mode='none'",L" field='capital_letters' mode='pitch'",L" field='capital_letters' mode='no'",L" field='unknown' mode='invalid'"};
	const wchar_t *emphasis[]={L" ",L" level='none'",L" level='reduced'",L" level='moderate'",L" level='strong'",L" level='x-strong'"};
	for(int trial=0;trial<200000;trial++) {
		int kind=trial%3==0?SSML_PROSODY:trial%3==1?SSML_STYLE:SSML_EMPHASIS;
		const wchar_t *text=kind==SSML_PROSODY?prosody[next()%6]:kind==SSML_STYLE?style[next()%6]:emphasis[next()%6];
		wchar_t input[200]={0};wcscpy(input,text);
		PARAM_STACK actual[20],old[20];
		for(int i=0;i<20;i++){actual[i].type=next()%15;for(int j=0;j<15;j++)actual[i].parameter[j]=(int)(next()%351)-50;}
		memcpy(old,actual,sizeof(old));
		int count=1+next()%19,old_count=count,current[15],reference[15];
		for(int i=0;i<15;i++)current[i]=(int)(next()%351)-50;
		memcpy(reference,current,sizeof(current));
		int tone=(int)(next()%4)-1;reference_translator.langopts.tone_language=tone;
		int punctuation=(int)(next()%5),capitals=(int)(next()%10);reference_punctuation=punctuation;reference_capitals=capitals;
		char out[200],expected[200];memset(out,0xa5,200);memset(expected,0xa5,200);int offset=0;
		ReferenceDirective(kind,input+1,expected,&offset,sizeof(expected),&old_count,old,reference);
		PARAM_STACK frame;
		TEST_ASSERT(espeak_rs_ssml_directive(kind,input,wcslen(input)+1,1,(const int32_t (*)[15])actual[0].parameter,(const int32_t (*)[15])current,tone,DecimalPoint(),WideSpace,&frame)==0);
		int index=espeak_rs_ssml_push(actual,&count,kind);TEST_ASSERT(index>=0);actual[index]=frame;
		RustSsmlParameters effect;
		TEST_ASSERT(espeak_rs_ssml_parameters(actual,count,(const int32_t (*)[15])current,punctuation,capitals,0,0,sizeof(out),&effect)==0);
		if(effect.changed)memcpy(out,effect.commands,effect.length+1);
		TEST_ASSERT(count==old_count);TEST_ASSERT(memcmp(actual,old,sizeof(actual))==0);
		TEST_ASSERT(memcmp(effect.values,reference,sizeof(reference))==0);
		TEST_ASSERT(effect.punctuation==reference_punctuation);TEST_ASSERT(effect.capitals==reference_capitals);
		TEST_ASSERT(effect.length==(unsigned)offset);TEST_ASSERT(memcmp(out,expected,sizeof(out))==0);directives++;
	}
	PARAM_STACK output,before;memset(&output,0xa5,sizeof(output));before=output;int values[15]={0};
	TEST_ASSERT(espeak_rs_ssml_directive(12,L" level='unknown'",17,1,(const int32_t (*)[15])values,(const int32_t (*)[15])values,0,46,WideSpace,&output)==1);
	TEST_ASSERT(memcmp(&output,&before,sizeof(output))==0);
}
static void text_helpers(void)
{
	static const int kinds[]={SSML_PHONEME,SSML_SAYAS,SSML_SAYAS+SSML_CLOSE,SSML_SUB,SSML_IGNORE_TEXT,SSML_SUB+SSML_CLOSE,SSML_IGNORE_TEXT+SSML_CLOSE};
	static const wchar_t *phoneme[]={L" alphabet='espeak' ph='abcdef'",L" alphabet='espeak' ph='\\\\ab\\'cd'",L" alphabet='unknown' ph='a'",L" alphabet='espeak'",L" alphabet='espeak' ph=''",L" alphabet=espeak ph=abc/",L" alphabet='espeak' ph='αβ界😀'"};
	static const wchar_t *sayas[]={L" interpret-as='characters'",L" interpret-as='tts:char'",L" interpret-as='tts:key'",L" interpret-as='tts:digits' detail='0'",L" interpret-as='tts:digits' detail='2'",L" interpret-as='tts:digits' detail='2147483500'",L" interpret-as='telephone'",L" interpret-as='unknown'",L" format='glyphs'",L" interpret-as='tts:key' format='glyphs'",L" "};
	static const wchar_t *alias[]={L" alias='abc'",L" alias='αβ界😀'",L" alias=''",L" alias='/Alice Bob'",L" alias=abc/",L" unknown='ignored'"};
	static const char *keynames[]={"space ","tab ","underscore ","double-quote ","space", "arbitrary ", "space \0ignored"};
	for(int round=0;round<400000;round++) {
		int kind=kinds[next()%(sizeof(kinds)/sizeof(*kinds))];
		const wchar_t *source=kind==SSML_PHONEME?phoneme[next()%(sizeof(phoneme)/sizeof(*phoneme))]:kind==SSML_SAYAS?sayas[next()%(sizeof(sayas)/sizeof(*sayas))]:kind==SSML_SUB?alias[next()%(sizeof(alias)/sizeof(*alias))]:L" ";
		wchar_t xml[501];wcscpy(xml,source);
		char out[256],expected[256];memset(out,0xa5,sizeof(out));
		int offset=(int)(next()%30),mode=(int)(next()%300)-30,start=(int)(next()%30)-10;
		for(int i=0;i<offset;i++)out[i]=(char)('a'+next()%26);
		if(kind==SSML_SAYAS+SSML_CLOSE && next()%2) {
			mode=SAYAS_KEY;start=offset;
			const char *name=keynames[next()%(sizeof(keynames)/sizeof(*keynames))];
			int length=(int)strlen(name);memcpy(out+offset,name,length);offset+=length;
			if(next()%10==0 && length>=3)out[start+1]=0;
		} else if(mode==SAYAS_KEY) mode=0;
		memcpy(expected,out,sizeof(out));
		bool ignore=next()%2;int old_offset=offset,old_mode=mode,old_start=start;bool old_ignore=ignore;
		// Defined C output extents: 4 bytes minimum for phoneme wrappers, 16
		// for longest say-as command, 2 for close; alias copy retains reserve4.
		int remaining=kind==SSML_SAYAS?16:kind==SSML_PHONEME?4:kind==SSML_SAYAS+SSML_CLOSE?2:kind==SSML_SUB?1:0;
		remaining+=(int)(next()%90);int capacity=offset+remaining;
		char original[256];memcpy(original,out,sizeof(original));
		ReferenceText(kind,xml+1,expected,&old_offset,capacity,&old_mode,&old_start,&old_ignore);
		RustSsmlTextState state={offset,mode,start,ignore};
		int status=espeak_rs_ssml_text(kind,xml,wcslen(xml)+1,1,(unsigned char *)out,capacity,&state,WideSpace,ByteSpace);
		if(old_offset>capacity) {
			// Retained C's multibyte copy can consume the wrapper's final byte
			// beyond the declared capacity. The physical oracle array is larger;
			// native dispatch rejects the contract violation before any writes.
			TEST_ASSERT(kind==SSML_PHONEME);TEST_ASSERT(status==1);
			TEST_ASSERT(state.offset==offset && state.mode==mode && state.start==start && state.ignore==(unsigned)ignore);
			TEST_ASSERT(memcmp(out,original,sizeof(out))==0);
			text_capacity_rejections++;continue;
		}
		if(status)fprintf(stderr,"text rejection round=%d kind=%d offset=%d capacity=%d mode=%d start=%d old_offset=%d xml=%ls\n",round,kind,offset,capacity,mode,start,old_offset,xml);
		TEST_ASSERT(status==0);
		TEST_ASSERT(state.offset==old_offset);TEST_ASSERT(state.mode==old_mode);TEST_ASSERT(state.start==old_start);TEST_ASSERT(state.ignore==(unsigned)old_ignore);
		TEST_ASSERT(memcmp(out,expected,sizeof(out))==0);text_directives++;
	}
	char out[32],before[32];memset(out,0xa5,sizeof(out));memcpy(out,"space ",6);memcpy(before,out,sizeof(out));
	RustSsmlTextState state={6,SAYAS_KEY,0,0},saved=state;
	TEST_ASSERT(espeak_rs_ssml_text(SSML_SAYAS+SSML_CLOSE,L" ",2,1,(unsigned char *)out,6,&state,WideSpace,ByteSpace)==1);
	TEST_ASSERT(memcmp(out,before,sizeof(out))==0);TEST_ASSERT(memcmp(&state,&saved,sizeof(state))==0);
	state.offset=0;state.mode=0;state.start=-1; saved=state;
	const wchar_t *overflow=L" interpret-as='tts:digits' detail='2147483647'";
	TEST_ASSERT(espeak_rs_ssml_text(SSML_SAYAS,overflow,wcslen(overflow)+1,1,(unsigned char *)out,32,&state,WideSpace,ByteSpace)==1);
	TEST_ASSERT(memcmp(out,before,sizeof(out))==0);TEST_ASSERT(memcmp(&state,&saved,sizeof(state))==0);
	state.ignore=2;saved=state;
	TEST_ASSERT(espeak_rs_ssml_text(SSML_IGNORE_TEXT,L" ",2,1,(unsigned char *)out,32,&state,WideSpace,ByteSpace)==1);
	TEST_ASSERT(memcmp(out,before,sizeof(out))==0);TEST_ASSERT(memcmp(&state,&saved,sizeof(state))==0);
}
static void clause_helpers(void)
{
	TEST_ASSERT(sizeof(RustSsmlBreak)==28);TEST_ASSERT(sizeof(RustSsmlVoiceClause)==28);
	const wchar_t *strengths[]={L"none",L"x-weak",L"weak",L"medium",L"strong",L"x-strong",L"unknown",L""};
	for(int sonic=0;sonic<=1;sonic++)for(int round=0;round<100000;round++) {
		wchar_t xml[501];
		unsigned form=next()%7;int duration=(int)(next()%100001);
		if(form==0)swprintf(xml,501,L" strength='%ls' time='%dms'",strengths[next()%8],duration);
		else if(form==1)swprintf(xml,501,L" strength='%ls' time='%ds'",strengths[next()%8],duration/1000);
		else if(form==2)swprintf(xml,501,L" time='%dms'",duration);
		else if(form==3)swprintf(xml,501,L" strength='%ls'",strengths[next()%8]);
		else if(form==4)wcscpy(xml,L" strength=strong time='unknown'");
		else if(form==5)wcscpy(xml,L" strength='' time=''");
		else wcscpy(xml,L" ");
		int parameters[15]={0};parameters[espeakRATE]=(int)(next()%2020)-20;parameters[espeakSSML_BREAK_MUL]=(int)(next()%601)-200;
		char out[128],expected[128];memset(out,0xa5,sizeof(out));int offset=(int)(next()%30);
		for(int i=0;i<offset;i++)out[i]=(char)('a'+next()%26);memcpy(expected,out,sizeof(out));int old_offset=offset;
		next_clause_pause=1+(int)(next()%2560);next_pause=1+(int)(next()%2560);
		rate_calls=0;rate_output_hash=0;rate_output=expected;rate_offset=&old_offset;
		int result=sonic?ReferenceBreak1(xml+1,expected,&old_offset,parameters):ReferenceBreak0(xml+1,expected,&old_offset,parameters);
		int calls=rate_calls;unsigned hash=rate_output_hash;
		rate_calls=0;rate_output_hash=0;rate_output=out;rate_offset=&offset;
		RustSsmlBreak request;
		TEST_ASSERT(espeak_rs_ssml_pause(xml,wcslen(xml)+1,1,parameters[espeakRATE],parameters[espeakSSML_BREAK_MUL],WideSpace,&request)==0);
		if(request.length){memcpy(out+offset,request.command,request.length+1);offset+=(int)request.length;}
		if(request.timed)ReferenceRate(espeakRATE,request.rate,0);
		int actual=777;
		TEST_ASSERT(espeak_rs_ssml_pause_finish(&request,reference_speed.clause_pause_factor,reference_speed.pause_factor,sonic,&actual)==0);
		TEST_ASSERT(actual==result && offset==old_offset);TEST_ASSERT(rate_calls==calls && rate_output_hash==hash);
		TEST_ASSERT(memcmp(out,expected,sizeof(out))==0);breaks++;
	}
	const int kinds[]={SSML_SPEAK,SSML_VOICE,SSML_SPEAK+SSML_CLOSE,SSML_VOICE+SSML_CLOSE,HTML_BREAK,HTML_BREAK+SSML_CLOSE,SSML_SENTENCE,SSML_PARAGRAPH,SSML_SENTENCE+SSML_CLOSE,SSML_PARAGRAPH+SSML_CLOSE};
	wchar_t no_attributes[2]={32,0};
	for(int round=0;round<200000;round++) {
		SSML_STACK frames[20];memset(frames,0xa5,sizeof(frames));int count=1+(int)(next()%20),old_count=count;
		for(int i=0;i<count;i++)frames[i].tag_type=(int)(next()%17);
		int kind=kinds[next()%(sizeof(kinds)/sizeof(*kinds))];
		for(int i=0;i<3;i++)voice_call_flags[i]=next()%2?CLAUSE_TYPE_VOICE_CHANGE:0;
		voice_call_count=0;int expected=ReferenceVoiceDirective(kind,no_attributes+1,frames,&old_count);
		RustSsmlVoiceClause request;TEST_ASSERT(espeak_rs_ssml_voice_clause(kind,frames,count,&request)==0);
		TEST_ASSERT(request.count==(unsigned)old_count);TEST_ASSERT(request.length==(unsigned)voice_call_count);
		int flags=0;for(unsigned i=0;i<request.length;i++) {
			TEST_ASSERT(request.tags[i]==voice_call_tags[i]);TEST_ASSERT(request.count==(unsigned)voice_call_counts[i]);flags|=voice_call_flags[i];
		}
		int actual=777;TEST_ASSERT(espeak_rs_ssml_voice_clause_finish(&request,flags,&actual)==0);TEST_ASSERT(actual==expected);voice_directives++;
	}
	RustSsmlBreak effect,before;memset(&effect,0xa5,sizeof(effect));before=effect;
	const wchar_t *overflow=L" time='2147483647s'";
	TEST_ASSERT(espeak_rs_ssml_pause(overflow,wcslen(overflow)+1,1,100,100,WideSpace,&effect)==1);TEST_ASSERT(memcmp(&effect,&before,sizeof(effect))==0);
	TEST_ASSERT(espeak_rs_ssml_pause(L" time='1'",10,1,100,100,WideSpace,&effect)==0);
	int value=777;TEST_ASSERT(espeak_rs_ssml_pause_finish(&effect,0,100,0,&value)==1);TEST_ASSERT(value==777);
	TEST_ASSERT(espeak_rs_ssml_pause_finish(&effect,100,100,2,&value)==1);TEST_ASSERT(value==777);
	RustSsmlVoiceClause voice,before_voice;memset(&voice,0xa5,sizeof(voice));before_voice=voice;
	SSML_STACK frame={0};TEST_ASSERT(espeak_rs_ssml_voice_clause(SSML_VOICE,&frame,0,&voice)==1);TEST_ASSERT(memcmp(&voice,&before_voice,sizeof(voice))==0);
}
static void NativeResourceStack(int kind,int pop,PARAM_STACK *frames,int *count,int *current,char *output,int *offset)
{
	RustSsmlParameters effect;
	TEST_ASSERT(espeak_rs_ssml_parameters(frames,*count,(const int32_t (*)[15])current,reference_punctuation,reference_capitals,pop,kind,512-*offset,&effect)==0);
	if(effect.changed)memcpy(output+*offset,effect.commands,effect.length+1);
	*offset+=(int)effect.length;memcpy(current,effect.values,sizeof(effect.values));
	reference_punctuation=effect.punctuation;reference_capitals=effect.capitals;if(pop)*count=(int)effect.count;
}
static void NativeResourceSignal(unsigned kind,int index,char *out,int *offset,PARAM_STACK *frame)
{
	RustSsmlSignal effect;TEST_ASSERT(espeak_rs_ssml_signal(kind,index,&effect)==0);
	if(effect.length){TEST_ASSERT(*offset+(int)effect.length<512);memcpy(out+*offset,effect.bytes,effect.length+1);*offset+=(int)effect.length;}
	if(frame&&effect.silence)frame->parameter[espeakSILENCE]=1;
}
static void resource_helpers(void)
{
	TEST_ASSERT(sizeof(RustSsmlResource)==168 && sizeof(RustSsmlFile)==260 && sizeof(RustSsmlSignal)==24 && sizeof(RustSsmlAudio)==16);
	const wchar_t *forms[]={L" name='marker' src='tone.wav' xml:base='base'",L" name='' src='' xml:base=''",L" name='/absolute' src='/tone.wav'",L" name=unquoted/ src=tone.wav/",L" unknown='ignored'",L" name='αβ界😀' src='αβ界😀'",L" name='a\\'b' src='c\\'d'"};
	const char *bases[]={NULL,"","root","root/","https://example.invalid/assets"};
	for(int round=0;round<200000;round++) {
		wchar_t xml[501];wcscpy(xml,forms[next()%7]);
		if(round%8==0){xml[0]=32;wcscpy(xml+1,L"name='");for(int i=7;i<450;i++)xml[i]=(wchar_t)(i%2?0x3b1:0x1f600);xml[450]=39;xml[451]=0;}
		int kind=round%3==0?SSML_SPEAK:round%3==1?SSML_MARK:SSML_AUDIO;
		RustSsmlResource request;TEST_ASSERT(espeak_rs_ssml_resource(kind,xml,wcslen(xml)+1,1,WideSpace,ByteSpace,&request)==0);
		const wchar_t *attribute=GetSsmlAttribute(xml+1,kind==SSML_SPEAK?"xml:base":kind==SSML_MARK?"name":"src");
		char expected[160];memset(expected,0xa5,sizeof(expected));attrcopy_utf8(expected,attribute,sizeof(expected));
		TEST_ASSERT(request.kind==kind && request.present==(unsigned)(attribute!=NULL));TEST_ASSERT(strcmp((char *)request.name,expected)==0);resource_requests++;
		if(kind==SSML_AUDIO && attribute!=NULL) {
			const char *base=bases[next()%5];char path[256];if(base&&expected[0]!='/')snprintf(path,sizeof(path),"%s/%s",base,expected);else strcpy(path,expected);
			RustSsmlFile result;TEST_ASSERT(espeak_rs_ssml_file(&request,base,&result)==0);TEST_ASSERT(result.length==strlen(path));TEST_ASSERT(strcmp((char *)result.bytes,path)==0);
		}
		if(kind==SSML_MARK) {
			const char *skip=round%2&&strlen(expected)<50?expected:"other";uint32_t action=77;
			TEST_ASSERT(espeak_rs_ssml_marker(&request,skip,&action)==0);TEST_ASSERT(action==(attribute==NULL?0:expected[0]&&strcmp(expected,skip)==0?1:2));
		}
	}
	for(int round=0;round<100000;round++) {
		int kind=round%3==0?SSML_MARK:round%3==1?SSML_AUDIO:SSML_AUDIO+SSML_CLOSE;
		wchar_t xml[501];wcscpy(xml,forms[next()%7]);const char *base=bases[next()%5];bool self_closing=next()%2;
		strcpy(resource_skip,next()%2?"marker":"other");char original_skip[50];memcpy(original_skip,resource_skip,50);
		resource_callback=next()%2?ResourceUri:NULL;resource_index=next()%7==0?-1:(int)(next()%100);resource_uri_result=next()%2;
		PARAM_STACK frames[20],expected_frames[20];int count=1+(int)(next()%19),expected_count=count;
		for(int i=0;i<20;i++){frames[i].type=i==0?0:(int)(next()%16);for(int j=0;j<15;j++)frames[i].parameter[j]=(int)(next()%351)-50;}
		memcpy(expected_frames,frames,sizeof(frames));int current[15],expected_current[15];for(int i=0;i<15;i++)current[i]=(int)(next()%351)-50;memcpy(expected_current,current,sizeof(current));
		char output[512],expected[512];memset(output,0xa5,512);int offset=(int)(next()%20),expected_offset=offset;for(int i=0;i<offset;i++)output[i]='a';memcpy(expected,output,512);
		bool audio=next()%2,clear=next()%2,expected_audio=audio,expected_clear=clear;
		int punctuation=next()%4,capitals=next()%20;reference_punctuation=punctuation;reference_capitals=capitals;resource_trace=0;
		int result=ReferenceResource(kind,xml+1,expected,&expected_offset,512,base,self_closing,&expected_audio,&expected_clear,&expected_count,expected_frames,expected_current);
		unsigned trace=resource_trace;int end_punctuation=reference_punctuation,end_capitals=reference_capitals;char end_skip[50];memcpy(end_skip,resource_skip,50);
		resource_trace=0;reference_punctuation=punctuation;reference_capitals=capitals;memcpy(resource_skip,original_skip,50);int actual=0;
		if(kind==SSML_MARK) {
			RustSsmlResource request;uint32_t action;
			TEST_ASSERT(espeak_rs_ssml_resource(kind,xml,wcslen(xml)+1,1,WideSpace,ByteSpace,&request)==0);
			TEST_ASSERT(espeak_rs_ssml_marker(&request,resource_skip,&action)==0);
			if(action==1){clear=true;resource_skip[0]=0;actual=CLAUSE_NONE;}
			else if(action==2)NativeResourceSignal(1,ResourceAppend((char *)request.name,0),output,&offset,NULL);
		} else {
			RustSsmlAudio effect;TEST_ASSERT(espeak_rs_ssml_audio(kind,self_closing,&effect)==0);
			if(effect.push) {
				int index=espeak_rs_ssml_push(frames,&count,kind);TEST_ASSERT(index>=0);PARAM_STACK *frame=frames+index;
				RustSsmlResource request;TEST_ASSERT(espeak_rs_ssml_resource(kind,xml,wcslen(xml)+1,1,WideSpace,ByteSpace,&request)==0);
				if(request.present) {
					if(resource_callback==NULL){RustSsmlFile path;TEST_ASSERT(espeak_rs_ssml_file(&request,base,&path)==0);NativeResourceSignal(2,ResourceLoad((char *)path.bytes),output,&offset,frame);}
					else {int index=ResourceAppend((char *)request.name,0);if(index>=0&&ResourceUri(1,(char *)request.name,base)==0)NativeResourceSignal(3,index,output,&offset,frame);}
				}
				NativeResourceStack(kind,0,frames,&count,current,output,&offset);
			}
			if(effect.pop)NativeResourceStack(kind,1,frames,&count,current,output,&offset);
			if(effect.text!=2)audio=effect.text!=0;actual=effect.terminator;
		}
		TEST_ASSERT(actual==result && count==expected_count && offset==expected_offset && audio==expected_audio && clear==expected_clear);
		TEST_ASSERT(resource_trace==trace && reference_punctuation==end_punctuation && reference_capitals==end_capitals);TEST_ASSERT(memcmp(resource_skip,end_skip,50)==0);
		TEST_ASSERT(memcmp(frames,expected_frames,sizeof(frames))==0);TEST_ASSERT(memcmp(current,expected_current,sizeof(current))==0);TEST_ASSERT(memcmp(output,expected,512)==0);resource_directives++;
	}
	RustSsmlResource request,before;memset(&request,0xa5,sizeof(request));before=request;
	wchar_t missing[2]={32,65};TEST_ASSERT(espeak_rs_ssml_resource(SSML_MARK,missing,2,1,WideSpace,ByteSpace,&request)==1);TEST_ASSERT(memcmp(&request,&before,sizeof(request))==0);
	TEST_ASSERT(espeak_rs_ssml_resource(SSML_AUDIO,L" src='x'",9,1,WideSpace,ByteSpace,&request)==0);
	RustSsmlFile path,saved;memset(&path,0xa5,sizeof(path));saved=path;char base[256];memset(base,'a',255);base[255]=0;
	TEST_ASSERT(espeak_rs_ssml_file(&request,base,&path)==1);TEST_ASSERT(memcmp(&path,&saved,sizeof(path))==0);
	RustSsmlAudio effect,saved_audio;memset(&effect,0xa5,sizeof(effect));saved_audio=effect;
	TEST_ASSERT(espeak_rs_ssml_audio(SSML_AUDIO,2,&effect)==1);TEST_ASSERT(memcmp(&effect,&saved_audio,sizeof(effect))==0);
}
int main(void)
{
	TEST_ASSERT(setlocale(LC_CTYPE,"C")!=NULL);helpers();scans();refs();guards();
	if(setlocale(LC_CTYPE,"en_US.UTF-8")||setlocale(LC_CTYPE,"C.UTF-8")){helpers();scans();refs();guards();}
	stack_helpers();
	voice_stack_helpers();
	prosody_helpers();
	numeric_locale_helpers();
	voice_attribute_helpers();
	tag_helpers();directive_helpers();text_helpers();clause_helpers();resource_helpers();
	printf("Matched %zu comparisons, %zu numbers, %zu copies, %zu attributes, %zu references, %zu keys, %zu parameter selections, %zu pops, %zu pushes, %zu voice choices, %zu binary64 parses, %zu prosody values, %zu prosody parameters, %zu voice-frame dispatches, %zu identifier changes, %zu tags and %zu directives\n",comparisons,numbers,copies,attributes,references,keys,parameters,pops,pushes,voice_choices,float_values,prosody_values,prosody_parameters,voice_frames,voice_changes,tags,directives);
	printf("Matched %zu SSML text directive output/state/tail comparisons; rejected %zu legacy wrapper capacity overruns\n",text_directives,text_capacity_rejections);
	printf("Matched %zu break timing/command/rate-order comparisons and %zu clause/voice transitions\n",breaks,voice_directives);
	printf("Matched %zu resource requests and %zu full marker/audio command/state/backend-order comparisons\n",resource_requests,resource_directives);
	return 0;
}
