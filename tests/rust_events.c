/* The native event list against independently extracted legacy C: the same
 * marker, rescaling and termination scripts must leave byte-identical event
 * lists, including the union bytes an event's kind leaves untouched.
 * SPDX-License-Identifier: GPL-3.0-or-later */
#include "config.h"
#include "test_assert.h"
#include <stdbool.h>
#include <stdint.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <espeak-ng/espeak_ng.h>
#include <espeak-ng/speak_lib.h>
#include <espeak-ng/encoding.h>
#include "phoneme.h"
#include "synthesize.h"
#include "translate.h"
#include "voice.h"
#include "rust_data.h"

/* The reference's statics. */
static espeak_EVENT *ref_events;
static int ref_ix,ref_n;
static unsigned int ref_uid;
static void *ref_user;
static long ref_count;
static int ref_rate,ref_delay;
static char names[256];
static char *ref_names=names;
static unsigned char outbuf[4096];
static unsigned char *ref_start=outbuf;
#define event_list ref_events
#define event_list_ix ref_ix
#define n_event_list ref_n
#define my_unique_identifier ref_uid
#define my_user_data ref_user
#define count_samples ref_count
#define samplerate ref_rate
#define mbrola_delay ref_delay
#define namedata ref_names
#define out_start ref_start
#define MarkerEvent RefMarkerEvent
#define RescaleEventSamples RefRescale
void RefMarkerEvent(int type, unsigned int char_position, int value, int value2, unsigned char *position);
void RefRescale(int length_pre, int length_post);
#include "events_reference.inc"
#undef event_list
#undef event_list_ix
#undef n_event_list
#undef my_unique_identifier
#undef my_user_data
#undef count_samples
#undef samplerate
#undef mbrola_delay
#undef namedata
#undef out_start
#undef MarkerEvent
#undef RescaleEventSamples

static RustEventList native;
static unsigned seed=0x243f6a88u;
static unsigned next(void){seed^=seed<<13;seed^=seed>>17;seed^=seed<<5;return seed;}
static size_t scripts,markers,rescales;

static int delay(void)
{
#if USE_MBROLA
	return ref_delay;
#else
	return 0; // the reference's own constant
#endif
}
// Both lists get the same capacity and the same random contents.
static void reserve(int capacity)
{
	TEST_ASSERT(espeak_rs_events_reserve(&native,capacity)==0);
	espeak_EVENT *grown=realloc(ref_events,sizeof(espeak_EVENT)*(size_t)capacity);
	TEST_ASSERT(grown!=NULL);
	ref_events=grown;ref_n=capacity;
	for(int i=0;i<capacity;i++) {
		unsigned char *a=(unsigned char *)&ref_events[i];
		for(size_t b=0;b<sizeof(espeak_EVENT);b++)a[b]=(unsigned char)next();
	}
	memcpy(native.events,ref_events,sizeof(espeak_EVENT)*(size_t)capacity);
	TEST_ASSERT(native.capacity==capacity);
}
static void compare(void)
{
	TEST_ASSERT(native.count==ref_ix && native.capacity==ref_n);
	if(memcmp(native.events,ref_events,sizeof(espeak_EVENT)*(size_t)ref_n)!=0) {
		for(int i=0;i<ref_n;i++)if(memcmp(&native.events[i],&ref_events[i],sizeof(espeak_EVENT))) {
			const espeak_EVENT *a=&ref_events[i],*b=&native.events[i];
			fprintf(stderr,"script %zu event %d: type %d/%d pos %d/%d len %d/%d audio %d/%d sample %d/%d\n",scripts,i,a->type,b->type,
				a->text_position,b->text_position,a->length,b->length,a->audio_position,b->audio_position,a->sample,b->sample);
			break;
		}
		TEST_ASSERT(false);
	}
}
static void script(void)
{
	static const int rates[]={22050,16000,44100,8000,1};
	reserve(2+(int)(next()%40));
	ref_ix=native.count=(int)(next()%3);
	for(int op=0,n=1+(int)(next()%60);op<n;op++) {
		ref_uid=next();ref_user=(void *)(uintptr_t)next();
		// sometimes keep the previous base, so events sit exactly at it
		if(op==0 || next()%3) {
			ref_count=next()%4==0?(long)next()*(long)(next()%5):(long)(next()%1000000);
			ref_delay=next()%2?0:(int)(next()%2000);
		}
		ref_rate=rates[next()%5];
		RustEventSettings s={ref_uid,ref_user,ref_count,delay(),ref_rate,ref_names};
		switch(next()%8) {
		default: {
			static const int types[]={espeakEVENT_WORD,espeakEVENT_SENTENCE,espeakEVENT_MARK,espeakEVENT_PLAY,espeakEVENT_END,espeakEVENT_PHONEME,espeakEVENT_SAMPLERATE};
			int type=types[next()%7],value=next()%3?(int)(next()%256):(int)next(),value2=(int)next();
			unsigned int position=next();int offset=next()%4==0?(int)(next()%2):(int)(next()%sizeof(outbuf));
			RefMarkerEvent(type,position,value,value2,ref_start+offset);
			espeak_rs_event_marker(&native,&s,type,position,value,value2,offset);
			markers++;
			break;
		}
		case 5: {
			int pre=next()%6==0?(int)(next()%3)-1:(int)(next()%2000),post=next()%4==0?pre:(int)(next()%2000);
			RefRescale(pre,post);
			espeak_rs_events_rescale(&native,pre,post,ref_count,delay(),ref_rate);
			rescales++;
			break;
		}
		case 6: {
			int ix=next()%2?ref_ix:0;
			if(ix<ref_n) {
				ref_events[ix].type=espeakEVENT_LIST_TERMINATED;ref_events[ix].unique_identifier=ref_uid;ref_events[ix].user_data=ref_user;
				espeak_rs_events_terminate(&native,ix,ref_uid,ref_user);
			}
			if(next()%2)ref_ix=native.count=0;
			break;
		}
		}
		compare();
	}
	scripts++;
}
int main(void)
{
	for(int trial=0;trial<20000;trial++)script();
	// a message's end: a cleared message terminator, then the list terminator
	espeak_rs_events_terminated_message(&native,5,(void *)8);
	TEST_ASSERT(native.events[0].type==espeakEVENT_MSG_TERMINATED && native.events[0].unique_identifier==5 && native.events[0].user_data==(void *)8);
	TEST_ASSERT(native.events[0].text_position==0 && native.events[0].sample==0 && native.events[0].id.number==0);
	TEST_ASSERT(native.events[1].type==espeakEVENT_LIST_TERMINATED && native.events[1].unique_identifier==5);
	espeak_rs_events_release(&native);
	TEST_ASSERT(native.events==NULL && native.capacity==0);
	free(ref_events);
	printf("Matched %zu scripts: %zu markers, %zu rescales\n",scripts,markers,rescales);
	return 0;
}
