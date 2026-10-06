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
static int WideSpace(uint32_t c) {return iswspace((wint_t)c)!=0;}
static int ByteSpace(uint32_t c) {return c<=255 && isspace((unsigned char)c)!=0;}
static unsigned seed=0x72c184abu;
static unsigned next(void){seed^=seed<<13;seed^=seed>>17;seed^=seed<<5;return seed;}
static size_t comparisons,numbers,copies,attributes,references,keys,parameters,pops,pushes,voice_choices,float_values,prosody_values,prosody_parameters,voice_frames,voice_changes;
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
int main(void)
{
	TEST_ASSERT(setlocale(LC_CTYPE,"C")!=NULL);helpers();scans();refs();guards();
	if(setlocale(LC_CTYPE,"en_US.UTF-8")||setlocale(LC_CTYPE,"C.UTF-8")){helpers();scans();refs();guards();}
	stack_helpers();
	voice_stack_helpers();
	prosody_helpers();
	numeric_locale_helpers();
	voice_attribute_helpers();
	printf("Matched %zu comparisons, %zu numbers, %zu copies, %zu attributes, %zu references, %zu keys, %zu parameter selections, %zu pops, %zu pushes, %zu voice choices, %zu binary64 parses, %zu prosody values, %zu prosody parameters, %zu voice-frame dispatches and %zu identifier changes\n",comparisons,numbers,copies,attributes,references,keys,parameters,pops,pushes,voice_choices,float_values,prosody_values,prosody_parameters,voice_frames,voice_changes);
	return 0;
}
