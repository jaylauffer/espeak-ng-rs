/* All async command kinds and disposal states against retained C.
 * SPDX-License-Identifier: GPL-3.0-or-later */
#include "config.h"
#include "test_assert.h"
#include <assert.h>
#include <stdint.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <wchar.h>
#include "espeak_command.h"
#include "rust_async_command.h"

static uint64_t recorded;
static unsigned calls;
static t_espeak_command *active;
static void number(uint64_t value) { recorded = (recorded ^ value) * UINT64_C(1099511628211); }
static void string(const char *value)
{
	number(value != NULL);
	if (value)
		for (; *value; ++value)
			number((unsigned char)*value);
}
static void wide(const wchar_t *value)
{
	for (; *value; ++value)
		number((uint32_t)*value);
}
static void called(unsigned kind)
{
	TEST_ASSERT(active->state == CS_PROCESSED);
	++calls;
	number(kind);
	/* A host callback may allocate/dispose an independent command. */
	t_espeak_command *nested = create_espeak_parameter(espeakRATE, 121, 0);
	TEST_ASSERT(nested && delete_espeak_command(nested) == 1);
}
static espeak_ng_STATUS record_synth(unsigned id, const void *text, unsigned position,
                                    espeak_POSITION_TYPE type, unsigned end, unsigned flags, void *user)
{
	called(ET_TEXT); number(id); string(text); number(position); number(type);
	number(end); number(flags); number((uintptr_t)user); return ENS_OK;
}
static espeak_ng_STATUS record_mark(unsigned id, const void *text, const char *mark,
                                   unsigned end, unsigned flags, void *user)
{
	called(ET_MARK); number(id); string(text); string(mark); number(end);
	number(flags); number((uintptr_t)user); return ENS_OK;
}
static espeak_ng_STATUS record_key(const char *name) { called(ET_KEY); string(name); return ENS_OK; }
static espeak_ng_STATUS record_char(wchar_t value) { called(ET_CHAR); number((uint32_t)value); return ENS_OK; }
static espeak_ng_STATUS record_parameter(int parameter, int value, int relative)
{
	called(ET_PARAMETER); number(parameter); number((uint32_t)value); number(relative); return ENS_OK;
}
static void record_punctuation(const wchar_t *list) { called(ET_PUNCTUATION_LIST); wide(list); }
static espeak_ERROR record_voice_name(const char *name) { called(ET_VOICE_NAME); string(name); return EE_OK; }
static espeak_ERROR record_voice(espeak_VOICE *voice)
{
	called(ET_VOICE_SPEC); string(voice->name); string(voice->languages); string(voice->identifier);
	number(voice->gender); number(voice->age); number(voice->variant); number(voice->xx1);
	number((uint32_t)voice->score); number((uintptr_t)voice->spare); return EE_OK;
}
static int record_terminated(unsigned id, void *user)
{
	called(ET_TERMINATED_MSG); number(id); number((uintptr_t)user); return 0;
}
static const RustAsyncCommandCallbacks callbacks = {
	record_synth, record_mark, record_key, record_char, record_parameter,
	record_punctuation, record_voice_name, record_voice, record_terminated
};

#define create_espeak_text reference_text
#define create_espeak_mark reference_mark
#define create_espeak_key reference_key
#define create_espeak_char reference_char
#define create_espeak_parameter reference_parameter
#define create_espeak_punctuation_list reference_punctuation
#define create_espeak_voice_name reference_voice_name
#define create_espeak_voice_spec reference_voice
#define create_espeak_terminated_msg reference_terminated
#define process_espeak_command reference_process
#define delete_espeak_command reference_delete
#define sync_espeak_Synth record_synth
#define sync_espeak_Synth_Mark record_mark
#define sync_espeak_Key record_key
#define sync_espeak_Char record_char
#define SetParameter record_parameter
#define sync_espeak_SetPunctuationList record_punctuation
#define espeak_SetVoiceByName record_voice_name
#define espeak_SetVoiceByProperties record_voice
#define sync_espeak_terminated_msg record_terminated
#include "async_command_reference.inc"
#undef create_espeak_text
#undef create_espeak_mark
#undef create_espeak_key
#undef create_espeak_char
#undef create_espeak_parameter
#undef create_espeak_punctuation_list
#undef create_espeak_voice_name
#undef create_espeak_voice_spec
#undef create_espeak_terminated_msg
#undef process_espeak_command
#undef delete_espeak_command
#undef sync_espeak_Synth
#undef sync_espeak_Synth_Mark
#undef sync_espeak_Key
#undef sync_espeak_Char
#undef SetParameter
#undef sync_espeak_SetPunctuationList
#undef espeak_SetVoiceByName
#undef espeak_SetVoiceByProperties
#undef sync_espeak_terminated_msg

static uint64_t payload(t_espeak_command *command)
{
	recorded = UINT64_C(1469598103934665603);
	number(command->type); number(command->state);
	switch (command->type) {
	case ET_TEXT: {
		t_espeak_text *p = &command->u.my_text;
		number(p->unique_identifier); string(p->text); number(p->position); number(p->position_type);
		number(p->end_position); number(p->flags); number((uintptr_t)p->user_data); break;
	}
	case ET_MARK: {
		t_espeak_mark *p = &command->u.my_mark;
		number(p->unique_identifier); string(p->text); string(p->index_mark);
		number(p->end_position); number(p->flags); number((uintptr_t)p->user_data); break;
	}
	case ET_KEY:
		number(command->u.my_key.unique_identifier); number((uintptr_t)command->u.my_key.user_data);
		string(command->u.my_key.key_name); break;
	case ET_CHAR:
		number(command->u.my_char.unique_identifier); number((uintptr_t)command->u.my_char.user_data);
		number((uint32_t)command->u.my_char.character); break;
	case ET_PARAMETER:
		number(command->u.my_param.parameter); number((uint32_t)command->u.my_param.value);
		number(command->u.my_param.relative); break;
	case ET_PUNCTUATION_LIST: wide(command->u.my_punctuation_list); break;
	case ET_VOICE_NAME: string(command->u.my_voice_name); break;
	case ET_VOICE_SPEC: {
		espeak_VOICE *p = &command->u.my_voice_spec;
		string(p->name); string(p->languages); string(p->identifier); number(p->gender); number(p->age);
		number(p->variant); number(p->xx1); number((uint32_t)p->score); number((uintptr_t)p->spare); break;
	}
	case ET_TERMINATED_MSG:
		number(command->u.my_terminated_msg.unique_identifier);
		number((uintptr_t)command->u.my_terminated_msg.user_data); break;
	}
	return recorded;
}
static void pair(t_espeak_command *native, t_espeak_command *oracle, unsigned state, int process)
{
	TEST_ASSERT(native && oracle);
	TEST_ASSERT(payload(native) == payload(oracle));
	native->state = oracle->state = state;
	active = oracle; recorded = 0; calls = 0;
	if (process)
		reference_process(oracle);
	TEST_ASSERT(reference_delete(oracle) == 1);
	uint64_t expected = recorded;
	unsigned expected_calls = calls;
	active = native; recorded = 0; calls = 0;
	if (process)
		espeak_rs_async_command_process(native, &callbacks);
	TEST_ASSERT(espeak_rs_async_command_delete(native, &callbacks) == 1);
	TEST_ASSERT(recorded == expected && calls == expected_calls);
	active = NULL;
}
int main(void)
{
	TEST_ASSERT(delete_espeak_command(NULL) == 0);
	process_espeak_command(NULL);
	TEST_ASSERT(!create_espeak_text(NULL, 10, 0, POS_CHARACTER, 0, 0, NULL));
	TEST_ASSERT(!create_espeak_text("text", 0, 0, POS_CHARACTER, 0, 0, NULL));
	TEST_ASSERT(!create_espeak_mark("text", 5, NULL, 0, 0, NULL));
	/* Invalid required mark must reject before borrowing the text at all. */
	TEST_ASSERT(!create_espeak_mark((void *)(uintptr_t)1, 10, NULL, 0, 0, NULL));
	TEST_ASSERT(!create_espeak_key(NULL, NULL) && !create_espeak_punctuation_list(NULL));
	TEST_ASSERT(!create_espeak_voice_name(NULL) && !create_espeak_voice_spec(NULL));
	TEST_ASSERT(!create_espeak_text("x", SIZE_MAX, 0, POS_CHARACTER, 0, 0, NULL));
	t_espeak_command invalid = { .type = 42 };
	TEST_ASSERT(!espeak_rs_async_command_create(&invalid, 0));
	unsigned pairs = 0;
	for (unsigned episode = 0; episode < 6000; ++episode) {
		char text[80], mark[40], name[40], languages[20], identifier[40];
		snprintf(text, sizeof(text), "text episode %u \xc3\xa9", episode);
		snprintf(mark, sizeof(mark), "mark-%u", episode);
		snprintf(name, sizeof(name), "voice-%u", episode);
		snprintf(languages, sizeof(languages), "\005en-%u", episode % 9);
		snprintf(identifier, sizeof(identifier), "lang/%u", episode);
		wchar_t punctuation[] = { '.', 0x2603, (wchar_t)(episode + 1), 0 };
		void *user = (void *)(uintptr_t)(episode + 1);
		unsigned state = episode % 3;
		int process = (episode / 3) % 2;
		unsigned flags = episode ^ 0x7312;
		espeak_VOICE voice = { episode & 1 ? name : NULL, episode & 2 ? languages : NULL,
		                       episode & 4 ? identifier : NULL, episode % 3, episode % 101,
		                       episode % 8, episode % 255, -(int)episode, user };
		t_espeak_command *n[9], *r[9];
		n[0] = create_espeak_text(text, strlen(text) + 1, episode, POS_CHARACTER, episode + 9, flags, user);
		r[0] = reference_text(text, strlen(text) + 1, episode, POS_CHARACTER, episode + 9, flags, user);
		n[1] = create_espeak_mark(text, strlen(text) + 1, mark, episode, flags, user);
		r[1] = reference_mark(text, strlen(text) + 1, mark, episode, flags, user);
		n[2] = create_espeak_key(name, user); r[2] = reference_key(name, user);
		n[3] = create_espeak_char((wchar_t)episode, user); r[3] = reference_char((wchar_t)episode, user);
		n[4] = create_espeak_parameter(espeakPITCH, -(int)episode, episode & 1);
		r[4] = reference_parameter(espeakPITCH, -(int)episode, episode & 1);
		n[5] = create_espeak_punctuation_list(punctuation); r[5] = reference_punctuation(punctuation);
		n[6] = create_espeak_voice_name(name); r[6] = reference_voice_name(name);
		n[7] = create_espeak_voice_spec(&voice); r[7] = reference_voice(&voice);
		n[8] = create_espeak_terminated_msg(episode, user); r[8] = reference_terminated(episode, user);
		/* Caller buffers expire/change before queue processing. */
		memset(text, '!', sizeof(text)); memset(mark, '!', sizeof(mark)); memset(name, '!', sizeof(name));
		memset(languages, '!', sizeof(languages)); memset(identifier, '!', sizeof(identifier));
		memset(punctuation, 0, sizeof(punctuation)); memset(&voice, 0, sizeof(voice));
		for (unsigned kind = 0; kind < 9; ++kind) { pair(n[kind], r[kind], state, process); ++pairs; }
	}
	/* Native bounded copies initialize termination beyond a supplied byte extent. */
	char raw[] = { 'a', 'b' };
	t_espeak_command *text = create_espeak_text(raw, sizeof(raw), 0, POS_CHARACTER, 0, espeakCHARS_UTF8, NULL);
	TEST_ASSERT(text && memcmp(text->u.my_text.text, "ab\0", 3) == 0);
	TEST_ASSERT(delete_espeak_command(text) == 1);
	wchar_t raw_wide[] = { 0x2603, '!' };
	text = create_espeak_mark(raw_wide, sizeof(raw_wide), "mark", 0, espeakCHARS_WCHAR, NULL);
	TEST_ASSERT(text && ((wchar_t *)text->u.my_mark.text)[2] == 0);
	TEST_ASSERT(delete_espeak_command(text) == 1);
	fprintf(stderr, "%u owned command pairs match retained C: nine kinds, all states, process/discard, caller expiry and reentry; bounded terminators pass\n", pairs);
	return 0;
}
