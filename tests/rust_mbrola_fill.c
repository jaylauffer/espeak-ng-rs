/* Owned PCM cursor versus retained C reads, accounting and exact sample bytes.
 * SPDX-License-Identifier: GPL-3.0-or-later */
#include "config.h"
#include "test_assert.h"
#include <stdbool.h>
#include <stdint.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include "rust_data.h"

static _Alignas(2) unsigned char output[2][256];
static unsigned char *ref_pointer,*ref_end;
static int ref_rate,side,calls[2],requested[2][1024],positions[2];
static int limit,available,forced_result;
static int Reader(short *target,int samples) {
    TEST_ASSERT(calls[side]<1024 && samples>=0);
    requested[side][calls[side]++]=samples;
    if(forced_result)return forced_result;
    int n=samples;if(n>limit)n=limit;
    int remaining=available-positions[side];if(n>remaining)n=remaining;
    for(int i=0;i<n;i++) {
        int16_t sample=(int16_t)((positions[side]+i)*19571-32768);
        unsigned char *bytes=(unsigned char *)target+i*2;
        bytes[0]=(unsigned char)sample;bytes[1]=(unsigned char)((uint16_t)sample>>8);
    }
    positions[side]+=n;return n;
}
#define MbrolaFill ReferenceFill
#define read_MBR Reader
#define out_ptr ref_pointer
#define out_end ref_end
#define samplerate ref_rate
#include "mbrola_fill_reference.inc"
#undef MbrolaFill

static int PendingRead(void *context,unsigned char *target,int samples) {
    TEST_ASSERT(context==&side);
    if(forced_result)return forced_result;
    return Reader((short *)target,samples);
}
static unsigned seed=0x7ad10459;
static unsigned next(void){seed^=seed<<13;seed^=seed>>17;seed^=seed<<5;return seed;}
int main(void) {
    size_t comparisons=0,samples=0;
    for(int trial=0;trial<12000;trial++) {
        void *owner=espeak_rs_mbrola_fill_create();TEST_ASSERT(owner);
        ref_rate=8000+(int)(next()%40001);int duration=1+(int)(next()%20);
        int amplitude=(int)(next()%201)-100;limit=1+(int)(next()%100);
        available=(int)(next()%1000);forced_result=0;
        memset(calls,0,sizeof(calls));memset(positions,0,sizeof(positions));
        int resume=0;
        for(int attempt=0;attempt<1024;attempt++) {
            size_t capacity=(1+next()%128)*2;
            memset(output,0x5a,sizeof(output));side=0;
            ref_pointer=output[0];ref_end=output[0]+capacity;
            int c=ReferenceFill(duration,resume,amplitude);size_t cbytes=(size_t)(ref_pointer-output[0]);
            side=1;size_t written=12345;
            int native=espeak_rs_mbrola_fill(owner,output[1],capacity,&written,ref_rate,duration,resume,amplitude,&side,ReadMbrolaPcm);
            TEST_ASSERT(native>=0 && native<=3 && native!=2);
            TEST_ASSERT(c==(native==1) && written==cbytes);
            TEST_ASSERT(memcmp(output[0],output[1],sizeof(output[0]))==0);
            TEST_ASSERT(calls[0]==calls[1] && positions[0]==positions[1]);
            for(int i=0;i<calls[0];i++)TEST_ASSERT(requested[0][i]==requested[1][i]);
            comparisons++;samples+=written/2;
            if(!c)break;
            resume=1;TEST_ASSERT(attempt<1023);
        }
        espeak_rs_mbrola_fill_destroy(owner);
    }
    printf("12000 MBROLA output entries: %zu calls and %zu scaled samples match C\n",comparisons,samples);
    /* Zero-capacity retained C issues a zero-sized read and ends an entry.
     * Native state retains it without I/O; subsequent room permits a resume. */
    ref_rate=1000;available=10;limit=100;forced_result=0;
    memset(calls,0,sizeof(calls));memset(positions,0,sizeof(positions));side=0;
    ref_pointer=ref_end=output[0];TEST_ASSERT(ReferenceFill(10,false,40)==0 && calls[0]==1);
    void *owner=espeak_rs_mbrola_fill_create();TEST_ASSERT(owner);size_t written=12345;side=1;
    TEST_ASSERT(espeak_rs_mbrola_fill(owner,output[1],0,&written,1000,10,0,40,&side,ReadMbrolaPcm)==1);
    TEST_ASSERT(written==0 && calls[1]==0);
    TEST_ASSERT(espeak_rs_mbrola_fill(owner,output[1],20,&written,1000,10,1,40,&side,ReadMbrolaPcm)==0);
    TEST_ASSERT(written==20 && calls[1]==1 && positions[1]==10);
    /* Pending is distinct from end, and from arbitrary DLL errors. */
    forced_result=-2;
    TEST_ASSERT(ReadMbrolaPcm(NULL,output[1],10)==-1);
    TEST_ASSERT(espeak_rs_mbrola_fill(owner,output[1],20,&written,1000,10,0,40,&side,PendingRead)==2);
    TEST_ASSERT(written==0);
    forced_result=0;positions[1]=0;available=10;
    TEST_ASSERT(espeak_rs_mbrola_fill(owner,output[1],20,&written,1000,10,1,40,&side,PendingRead)==0);
    TEST_ASSERT(written==20);
    forced_result=-2;written=12345;
    TEST_ASSERT(espeak_rs_mbrola_fill(owner,output[1],20,&written,1000,10,0,40,&side,ReadMbrolaPcm)==-1);
    TEST_ASSERT(written==12345);
    forced_result=0;positions[1]=0;available=0;
    TEST_ASSERT(espeak_rs_mbrola_fill(owner,output[1],20,&written,1000,10,0,40,&side,ReadMbrolaPcm)==3);
    TEST_ASSERT(written==0);
    TEST_ASSERT(espeak_rs_mbrola_fill(owner,output[1],20,&written,1000,10,1,40,&side,ReadMbrolaPcm)==3);
    espeak_rs_mbrola_fill_destroy(owner);
    puts("Empty output preserves the cursor; pending reads retain it; DLL errors are terminal");
    return 0;
}
