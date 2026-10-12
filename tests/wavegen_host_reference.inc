/* Complete frozen 8586e3cb native dispatcher and Sonic/fill/stop controllers.
 * SPDX-License-Identifier: GPL-3.0-or-later */
static int WavegenEffect(void *context, RustWavegenEffect *e)
{
	(void)context;
	intptr_t *q = (e->index >= 0 && e->index < N_WCMDQ) ? wcmdq[e->index] : wcmdq[0];
	switch (e->op)
	{
	case 1:
		if (e->a == 1)
			output_hooks->outputVoiced(e->b);
		else if (e->a == 2)
			output_hooks->outputSilence(e->b);
		else
			output_hooks->outputUnvoiced(e->b);
		break;
	case 2:
		MarkerEvent(q[0] >> 8, q[1], * (int *) & q[2], * ((int *) & q[2] + 1), out_ptr);
		break;
	case 3:
	{
		char* data = (char*)q[1];
		output_hooks->outputPhoSymbol(data,q[2]);
		free(data);
	}
		break;
	case 4:
		MarkerEvent(espeakEVENT_SAMPLERATE, 0, e->a, 0, out_ptr);
		break;
#if USE_LIBSONIC
	case 5:
		sonicSpeed = (double)q[1] / 1024;
		if (sonicSpeedupStream && (sonicSpeed <= 1.0)) {
			sonicFlushStream(sonicSpeedupStream);
			int length = (out_end - out_ptr);
			length = sonicReadShortFromStream(sonicSpeedupStream, (short*)out_ptr, length/2);
#ifdef ARCH_BIG
			{
				unsigned i;
				for (i = 0; i < length/2; i++) {
					unsigned short v = ((unsigned short *) out_ptr)[i];
					out_ptr[i*2] = v & 0xff;
					out_ptr[i*2+1] = v >> 8;
				}
			}
#endif
			out_ptr += length * 2;
		}
		break;
#endif
	case 6:
		return espeak_rand(e->a, e->b);
	case 7:
		free((voice_t *)e->value);
		break;
#if USE_KLATT
	case 8:
		KlattReset(1);
		break;
	case 9:
		return Wavegen_Klatt(e->a, e->b, (frame_t *)e->value, (frame_t *)e->value2, e->data, e->voice);
#endif
#if USE_MBROLA
	case 10:
		return MbrolaFill(e->a, e->b, e->c);
#endif
	}
	return 0;
}

static int SpeedUp(short *outbuf, int length_in, int length_out, int end_of_text)
{
#ifdef ARCH_BIG
	unsigned i;
#endif

	if (length_in > 0) {
		if (sonicSpeedupStream == NULL)
			sonicSpeedupStream = sonicCreateStream(22050, 1);
		if (sonicGetSpeed(sonicSpeedupStream) != sonicSpeed)
			sonicSetSpeed(sonicSpeedupStream, sonicSpeed);

#ifdef ARCH_BIG
		for (i = 0; i < length_in; i++) {
			unsigned short v = ((unsigned char*) outbuf)[i*2] | (((unsigned char *)outbuf)[i*2+1] << 8);
			((unsigned short *) outbuf)[i] = v;
		}
#endif
		sonicWriteShortToStream(sonicSpeedupStream, outbuf, length_in);
	}

	if (sonicSpeedupStream == NULL)
		return 0;

	if (end_of_text)
		sonicFlushStream(sonicSpeedupStream);

	int ret = sonicReadShortFromStream(sonicSpeedupStream, outbuf, length_out);
#ifdef ARCH_BIG
	for (i = 0; i < length_out; i++) {
		unsigned short v = ((unsigned short *) outbuf)[i];
		((unsigned char *)outbuf)[i*2] = v & 0xff;
		((unsigned char *)outbuf)[i*2+1] = v >> 8;
	}
#endif
	return ret;
}

int WavegenFill(void)
{
	int finished;
#if USE_LIBSONIC
	unsigned char *p_start;

	p_start = out_ptr;
#endif

	finished = WavegenFill2();

#if USE_LIBSONIC
	if (sonicSpeed > 1.0) {
		int length;
		int max_length;
		int length_in;

		max_length = (out_end - p_start);
		length_in = (out_ptr-p_start)/2;
		length =  2*SpeedUp((short *)p_start, length_in, max_length/2, finished);
		out_ptr = p_start + length;

		// The events recorded while this buffer was filled refer to the
		// uncompressed audio, so move them onto what libsonic produced.
		RescaleEventSamples(length_in, length/2);

		if (length >= max_length)
			finished = 0; // there may be more data to flush
	}
#endif
	return finished;
}

void WcmdqStop(void)
{
#ifndef USE_RUST_CORE
	wcmdq_head = 0;
	wcmdq_tail = 0;
#else
	espeak_rs_wcmdq_stop(&espeak_rs_wave_memory);
#endif

#if USE_LIBSONIC
	if (sonicSpeedupStream != NULL) {
		sonicDestroyStream(sonicSpeedupStream);
		sonicSpeedupStream = NULL;
	}
#endif

#if USE_MBROLA
	if (mbrola_name[0] != 0)
		MbrolaReset();
#endif
}
