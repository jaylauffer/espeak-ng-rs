/* Marker/URI/wide name bytes against retained C storage.
 * SPDX-License-Identifier: GPL-3.0-or-later */
#include "config.h"
#include "test_assert.h"
#include <stdint.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <wchar.h>
#include "translate.h"
#include "readclause.h"
#include "rust_data.h"
static char *reference_names;
static int reference_offset,reference_capacity;
#define namedata reference_names
#define namedata_ix reference_offset
#define n_namedata reference_capacity
#define AddNameData ReferenceAppend
#define InitNamedata ReferenceReset
#include "namedata_reference.inc"
#undef InitNamedata
#undef AddNameData
#undef n_namedata
#undef namedata_ix
#undef namedata
static unsigned random_state=0x912c64adu;
static unsigned next(void){random_state^=random_state<<13;random_state^=random_state>>17;random_state^=random_state<<5;return random_state;}
int main(void)
{
	size_t compared=0;
	for(int batch=0;batch<200;batch++) {
		InitNamedata();ReferenceReset();TEST_ASSERT(namedata==NULL && reference_names==NULL);
		for(int entry=0;entry<500;entry++) {
			char text[161];wchar_t wide[161];int length=(int)(next()%160),is_wide=next()%2;
			for(int i=0;i<length;i++){text[i]=(char)(1+next()%255);wide[i]=(wchar_t)(1+next()%0x10ffff);}
			text[length]=0;wide[length]=0;
			const char *source=is_wide?(const char *)wide:text;
			int expected=ReferenceAppend(source,is_wide),actual=AddNameData(source,is_wide);
			TEST_ASSERT(actual==expected);TEST_ASSERT(memcmp(namedata,reference_names,reference_offset)==0);compared++;
		}
	}
	InitNamedata();ReferenceReset();FreeNamedata();TEST_ASSERT(namedata==NULL);
	void *owner=espeak_rs_names_create(32);TEST_ASSERT(owner!=NULL);
	const unsigned char *view=NULL;TEST_ASSERT(espeak_rs_names_append(owner,(const unsigned char *)"abc",4,1,&view)==0);
	TEST_ASSERT(memcmp(view,"abc",4)==0);TEST_ASSERT((uintptr_t)view%8==0);
	const unsigned char *first=view;size_t reserved=espeak_rs_names_reserved(owner);TEST_ASSERT(reserved<=32);
	unsigned char too_large[32];memset(too_large,'a',sizeof(too_large));too_large[31]=0;
	TEST_ASSERT(espeak_rs_names_append(owner,too_large,32,1,&view)==-1);TEST_ASSERT(view==first);TEST_ASSERT(memcmp(view,"abc",4)==0);
	TEST_ASSERT(espeak_rs_names_append(owner,(const unsigned char *)"bad",3,1,&view)==-1);TEST_ASSERT(view==first);
	TEST_ASSERT(espeak_rs_names_append(owner,(const unsigned char *)"x\0y",4,1,&view)==-1);TEST_ASSERT(view==first);
	TEST_ASSERT(espeak_rs_names_append(owner,(const unsigned char *)"x",2,3,&view)==-1);TEST_ASSERT(view==first);
	for(int round=0;round<1000;round++) {
		espeak_rs_names_reset(owner);
		TEST_ASSERT(espeak_rs_names_append(owner,(const unsigned char *)"repeat",7,1,&view)==0);
		TEST_ASSERT(first==view);TEST_ASSERT(espeak_rs_names_reserved(owner)==reserved);
	}
	espeak_rs_names_destroy(owner);
	TEST_ASSERT(espeak_rs_names_create(7)==NULL);TEST_ASSERT(espeak_rs_names_create(128u*1024u*1024u+1)==NULL);
	InitNamedata();TEST_ASSERT(AddNameData("after shutdown",0)==0);TEST_ASSERT(strcmp(namedata,"after shutdown")==0);FreeNamedata();
	printf("Matched %zu mixed narrow/wide name offsets and complete byte prefixes; 1000 warm resets reused pointer/capacity\n",compared);
	return 0;
}
