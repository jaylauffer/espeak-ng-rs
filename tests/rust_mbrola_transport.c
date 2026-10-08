/* Native owner/ABI and extracted production command submission regression. */
#include <assert.h>
#include <limits.h>
#include <stdint.h>
#include <stdio.h>
#include <string.h>
#include "src/libespeak-ng/rust_mbrola_transport.h"

#if !defined(_WIN32) && !defined(_WIN64)
#include <errno.h>
#include <fcntl.h>
#include <stdlib.h>
#include <unistd.h>
static espeak_rs_mbr_transport *mbr_transport;
static int mbr_pid = 1, mbr_cmd_fd, mode, write_calls;
static unsigned char written[64];
static size_t written_length;
static int mbrola_has_errors(void) { return 0; }
static void err(const char *format, ...) { (void)format; }
static ssize_t MockWrite(int fd, const void *bytes, size_t length)
{
	write_calls++;
	if (mode == 1 || mode == 2) {
		errno = mode == 1 ? EAGAIN : EINTR;
		return -1;
	}
	if (mode == 4)
		return write(fd, bytes, length);
	if (mode == 3 && length > 2)
		length = 2;
	assert(written_length + length <= sizeof(written));
	memcpy(written + written_length, bytes, length);
	written_length += length;
	return (ssize_t)length;
}
#define write MockWrite
#include "mbrola_transport_reference.inc"
#undef write

static void submission(void)
{
	mbr_transport = espeak_rs_mbr_transport_create();
	mode = 1;
	assert(send_to_mbrola("old") == 3);
	mode = 3;
	assert(send_to_mbrola("new") == 3);
	assert(written_length == 2 && !memcmp(written, "ol", 2));
	mode = 2;
	assert(drain_mbrola_commands() == 0);
	mode = 0;
	assert(drain_mbrola_commands() == 0);
	assert(written_length == 6 && !memcmp(written, "oldnew", 6));
	assert(write_calls == 4);
	const unsigned char *front;
	assert(espeak_rs_mbr_front(mbr_transport, &front) == 0);

	/* Full admission produces zero syscalls and preserves the older bytes. */
	unsigned char *full = malloc(256 * 1024);
	assert(full);
	memset(full, 'x', 256 * 1024);
	assert(espeak_rs_mbr_queue(mbr_transport, full, 256 * 1024) == 1);
	assert(send_to_mbrola("later") == 0 && write_calls == 4);
	assert(espeak_rs_mbr_front(mbr_transport, &front) == 256 * 1024);
	assert(!memcmp(front, full, 256 * 1024));
	free(full);
	espeak_rs_mbr_clear(mbr_transport, 1);

	/* Exercise the same extracted code against actual full nonblocking pipes. */
	int descriptors[2];
	assert(pipe(descriptors) == 0);
	assert(fcntl(descriptors[1], F_SETFL, O_NONBLOCK) == 0);
	assert(fcntl(descriptors[0], F_SETFL, O_NONBLOCK) == 0);
	unsigned char buffer[4096] = {0};
	while (write(descriptors[1], buffer, sizeof(buffer)) > 0) {}
	assert(errno == EAGAIN);
	mbr_cmd_fd = descriptors[1];
	mode = 4;
	assert(send_to_mbrola("first") == 5);
	while (read(descriptors[0], buffer, sizeof(buffer)) > 0) {}
	assert(errno == EAGAIN);
	assert(send_to_mbrola("second") == 6);
	assert(read(descriptors[0], buffer, sizeof(buffer)) == 11);
	assert(!memcmp(buffer, "firstsecond", 11));
	close(descriptors[0]);
	close(descriptors[1]);
	espeak_rs_mbr_transport_destroy(mbr_transport);
}
#endif

int main(void)
{
	espeak_rs_mbr_transport *owner = espeak_rs_mbr_transport_create();
	const unsigned char *front;
	assert(espeak_rs_mbr_queue(NULL, (const unsigned char *)"x", 1) == -1);
	assert(espeak_rs_mbr_queue(owner, NULL, 1) == -1);
	assert(espeak_rs_mbr_consume(owner, 1) == -1);
	assert(espeak_rs_mbr_queue(owner, (const unsigned char *)"abc", 3) == 1);
	assert(espeak_rs_mbr_front(owner, &front) == 3 && !memcmp(front, "abc", 3));
	unsigned char error[160] = "preserved";
	const unsigned char *stream = (const unsigned char *)"first\nGot a reset signal !\nInput Flush Signal\nlast";
	for (size_t i = 0; i < strlen((const char *)stream); i++)
		assert(espeak_rs_mbr_stderr(owner, stream + i, 1, 0, error, sizeof(error)) >= 0);
	assert(!strcmp((char *)error, "first"));
	assert(espeak_rs_mbr_stderr(owner, stream, 0, 1, error, sizeof(error)) == 1);
	assert(!strcmp((char *)error, "last"));
	unsigned char tiny[1] = {42};
	assert(espeak_rs_mbr_stderr(owner, (const unsigned char *)"x\n", 2, 0, tiny, 1) == 1 && tiny[0] == 0);
	unsigned char header[44] = {0};
	memcpy(header, "RIFF", 4);
	memcpy(header + 8, "WAVEfmt ", 8);
	/* Compare every representable generated rate with an independent C decode. */
	uint32_t seed = 1;
	for (int i = 0; i < 100000; i++) {
		seed = seed * 1664525u + 1013904223u;
		uint32_t rate = (seed & INT_MAX) | 1;
		for (int j = 0; j < 4; j++) header[24 + j] = rate >> (j * 8);
		int decoded = header[24] | (header[25] << 8) | (header[26] << 16) | (header[27] << 24);
		assert(espeak_rs_mbr_sample_rate(header, sizeof(header)) == decoded);
	}
	assert(espeak_rs_mbr_sample_rate(header, 43) == -1);
	memset(header + 24, 0, 4);
	assert(espeak_rs_mbr_sample_rate(header, 44) == -1);
	header[0] = 'x';
	assert(espeak_rs_mbr_sample_rate(header, 44) == -1);
	espeak_rs_mbr_transport_destroy(owner);
#if !defined(_WIN32) && !defined(_WIN64)
	submission();
#endif
	puts("MBROLA: FIFO partial/EAGAIN/EINTR/full admission, real pipes, fragmented stderr, 100000 WAV rates");
	return 0;
}
