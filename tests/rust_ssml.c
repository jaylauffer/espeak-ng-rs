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
#define ParseSsmlReference ReferenceParseSsmlReference
#include "ssml_reference.inc"
#undef ParseSsmlReference
static int WideSpace(uint32_t c) {return iswspace((wint_t)c)!=0;}
static int ByteSpace(uint32_t c) {return c<=255 && isspace((unsigned char)c)!=0;}
static unsigned seed=0x72c184abu;
static unsigned next(void){seed^=seed<<13;seed^=seed>>17;seed^=seed<<5;return seed;}
static size_t comparisons,numbers,copies,attributes,references,keys;

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
int main(void)
{
	TEST_ASSERT(setlocale(LC_CTYPE,"C")!=NULL);helpers();scans();refs();guards();
	if(setlocale(LC_CTYPE,"en_US.UTF-8")||setlocale(LC_CTYPE,"C.UTF-8")){helpers();scans();refs();guards();}
	printf("Matched %zu comparisons, %zu numbers, %zu copies, %zu attributes, %zu references and %zu key replacements\n",comparisons,numbers,copies,attributes,references,keys);
	return 0;
}
