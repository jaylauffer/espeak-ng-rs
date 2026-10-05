/* Bounded SSML helpers against independently extracted retained C.
 * SPDX-License-Identifier: GPL-3.0-or-later */
#include "config.h"
#include "test_assert.h"
#include <ctype.h>
#include <limits.h>
#include <locale.h>
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
static size_t comparisons,numbers,copies,attributes,references,keys,parameters,pops,pushes,voice_choices;
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
int main(void)
{
	TEST_ASSERT(setlocale(LC_CTYPE,"C")!=NULL);helpers();scans();refs();guards();
	if(setlocale(LC_CTYPE,"en_US.UTF-8")||setlocale(LC_CTYPE,"C.UTF-8")){helpers();scans();refs();guards();}
	stack_helpers();
	voice_stack_helpers();
	printf("Matched %zu comparisons, %zu numbers, %zu copies, %zu attributes, %zu references, %zu keys, %zu parameter selections, %zu pops, %zu pushes and %zu voice choices\n",comparisons,numbers,copies,attributes,references,keys,parameters,pops,pushes,voice_choices);
	return 0;
}
