/*
 * Copyright (C) 2005 to 2014 by Jonathan Duddington
 * email: jonsd@users.sourceforge.net
 * Copyright (C) 2013-2017 Reece H. Dunn
 *
 * This program is free software; you can redistribute it and/or modify
 * it under the terms of the GNU General Public License as published by
 * the Free Software Foundation; either version 3 of the License, or
 * (at your option) any later version.
 *
 * This program is distributed in the hope that it will be useful,
 * but WITHOUT ANY WARRANTY; without even the implied warranty of
 * MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the
 * GNU General Public License for more details.
 *
 * You should have received a copy of the GNU General Public License
 * along with this program; if not, see: <http://www.gnu.org/licenses/>.
 */

static int LoadWavefile(CompileContext *ctx, FILE *f, const char *fname)
{
	int displ;
	unsigned char c1;
	int sample;
	int sample2;
	float x;
	int max = 0;
	int length;
	int sr1, sr2;
	int scale_factor = 0;

	fseek(f, 24, SEEK_SET);
	sr1 = Read4Bytes(f);
	sr2 = Read4Bytes(f);
	fseek(f, 40, SEEK_SET);

	if ((sr1 != samplerate) || (sr2 != sr1*2)) {
		if (sr1 != samplerate)
			error(ctx, "Can't resample (%d to %d): %s", sr1, samplerate, fname);
		else
			error(ctx, "WAV file is not mono: %s", fname);
		return 0;
	}

	displ = ftell(ctx->f_phdata);

	// data contains:  4 bytes of length (n_samples * 2), followed by 2-byte samples (lsb byte first)
	length = Read4Bytes(f);

	while (true) {
		int c;

		if ((c = fgetc(f)) == EOF)
			break;
		c1 = (unsigned char)c;

		if ((c = fgetc(f)) == EOF)
			break;

		sample = CalculateSample((unsigned char) c, c1);

		if (sample > max)
			max = sample;
		else if (sample < -max)
			max = -sample;
	}

	scale_factor = (max / 127) + 1;

	#define MIN_FACTOR   -1 // was 6, disable use of 16 bit samples
	if (scale_factor > MIN_FACTOR) {
		length = length/2 + (scale_factor << 16);
	}

	Write4Bytes(ctx->f_phdata, length);
	fseek(f, 44, SEEK_SET);

	while (!feof(f)) {
		c1 = fgetc(f);
		unsigned char c3 = fgetc(f);

		sample = CalculateSample(c3, c1);

		if (feof(f)) break;

		if (scale_factor <= MIN_FACTOR) {
			fputc(sample & 0xff, ctx->f_phdata);
			fputc(sample >> 8, ctx->f_phdata);
		} else {
			x = ((float)sample / scale_factor) + 0.5;
			sample2 = (int)x;
			if (sample2 > 127)
				sample2 = 127;
			if (sample2 < -128)
				sample2 = -128;
			fputc(sample2, ctx->f_phdata);
		}
	}

	length = ftell(ctx->f_phdata);
	while ((length & 3) != 0) {
		// pad to a multiple of 4 bytes
		fputc(0, ctx->f_phdata);
		length++;
	}

	return displ | 0x800000; // set bit 23 to indicate a wave file rather than a spectrum
}

static espeak_ng_STATUS LoadEnvelope(CompileContext *ctx, FILE *f, int *displ)
{
	char buf[128];

	if (displ)
		*displ = ftell(ctx->f_phdata);

	if (fseek(f, 12, SEEK_SET) == -1)
		return errno;

	if (fread(buf, 128, 1, f) != 128)
		return errno;
	fwrite(buf, 128, 1, ctx->f_phdata);

	return ENS_OK;
}

// Generate a hash code from the specified string
static int Hash8(const char *string)
{
	int c;
	int chars = 0;
	int hash = 0;

	while ((c = *string++) != 0) {
		c = tolower(c) - 'a';
		hash = hash * 8 + c;
		hash = (hash & 0x1ff) ^ (hash >> 8); // exclusive or
		chars++;
	}

	return (hash+chars) & 0xff;
}

static int LoadEnvelope2(CompileContext *ctx, FILE *f)
{
	int displ;
	int n_points;
	char line_buf[128];
	float env_x[20];
	float env_y[20];
	int env_lin[20];
	unsigned char env[ENV_LEN];

	n_points = 0;
	if (fgets(line_buf, sizeof(line_buf), f) != NULL) { ; // skip first line, then loop
		while (!feof(f)) {
			if (fgets(line_buf, sizeof(line_buf), f) == NULL)
				break;

			env_lin[n_points] = 0;
			int n = sscanf(line_buf, "%f %f %d", &env_x[n_points], &env_y[n_points], &env_lin[n_points]);
			if (n >= 2) {
				env_x[n_points] *= (float)1.28; // convert range 0-100 to 0-128
				n_points++;
			}
		}
	}

	if (n_points > 0) {
		env_x[n_points] = env_x[n_points-1];
	env_y[n_points] = env_y[n_points-1];

		int ix = 0;
	int ix2 = 0;
		for (int x = 0; x < ENV_LEN; x++) {
		if (n_points > 3 && x > env_x[ix+3])
			ix++;
		if (n_points > 2 && x >= env_x[ix2+1])
			ix2++;

		int y;
		if (env_lin[ix2] > 0) {
			y = (env_y[ix2] + (env_y[ix2+1] - env_y[ix2]) * ((float)x - env_x[ix2]) / (env_x[ix2+1] - env_x[ix2])) * 2.55;
		} else if (n_points > 3)
			y = (int)(polint(&env_x[ix], &env_y[ix], 4, x) * 255 / 100); // convert to range 0-255
		else
			y = (int)(polint(&env_x[ix], &env_y[ix], 3, x) * 255 / 100);
		if (y < 0) y = 0;
		if (y > 255) y = 255;
		env[x] = y;
	}
	}

	displ = ftell(ctx->f_phdata);
	fwrite(env, 1, ENV_LEN, ctx->f_phdata);

	return displ;
}

static espeak_ng_STATUS LoadDataFile(CompileContext *ctx, const char *path, int control, int *addr)
{
	// load spectrum sequence or sample data from a file.
	// return index into spect or sample data area. bit 23=1 if a sample

	int hash;
	REF_HASH_TAB *p, *p2;

	if (strcmp(path, "NULL") == 0)
		return ENS_OK;
	if (strcmp(path, "DFT") == 0) {
		*addr = 1;
		return ENS_OK;
	}

	ctx->count_references++;

	hash = Hash8(path);
	p = ctx->ref_hash_tab[hash];
	while (p != NULL) {
		if (strcmp(path, p->string) == 0) {
			ctx->duplicate_references++;
			*addr = p->value; // already loaded this data
			break;
		}
		p = (REF_HASH_TAB *)p->link;
	}

	if (*addr == 0) {
		char buf[N_PATH_BUF];
		snprintf(buf, sizeof(buf), "%s/%s", ctx->phsrc, path);

		FILE *f;
		if ((f = fopen(buf, "rb")) == NULL) {
			snprintf(buf, sizeof(buf), "%s/%s.wav", ctx->phsrc, path);
			if ((f = fopen(buf, "rb")) == NULL) {
				error(ctx, "Can't read file: %s", path);
				return errno;
			}
		}

		int id = Read4Bytes(f);
		rewind(f);

		espeak_ng_STATUS status = ENS_OK;
		int type_code = ' ';
		if (id == 0x43455053) {
			status = LoadSpect(ctx, path, control, addr);
			type_code = 'S';
		} else if (id == 0x46464952) {
			*addr = LoadWavefile(ctx, f, path);
			type_code = 'W';
		} else if (id == 0x43544950) {
			status = LoadEnvelope(ctx, f, addr);
			type_code = 'E';
		} else if (id == 0x45564E45) {
			*addr = LoadEnvelope2(ctx, f);
			type_code = 'E';
		} else {
			error(ctx, "File not SPEC or RIFF: %s", path);
			*addr = -1;
			status = ENS_UNSUPPORTED_PHON_FORMAT;
		}
		fclose(f);

		if (status != ENS_OK)
			return status;

		if (*addr > 0)
			fprintf(ctx->f_phcontents, "%c  0x%.5x  %s\n", type_code, *addr & 0x7fffff, path);
	}

	// add this item to the hash table
	if (*addr > 0) {
		p = ctx->ref_hash_tab[hash];
		p2 = (REF_HASH_TAB *)malloc(sizeof(REF_HASH_TAB)+strlen(path)+1);
		if (p2 == NULL)
			return ENOMEM;
		p2->value = *addr;
		p2->ph_mnemonic = ctx->phoneme_out->mnemonic; // phoneme which uses this file
		p2->ph_table = ctx->n_phoneme_tabs-1;
		strcpy(p2->string, path);
		p2->link = (char *)p;
		ctx->ref_hash_tab[hash] = p2;
	}

	return ENS_OK;
}
static int CalculateSample(unsigned char c3, int c1) {
	int c2 = c3 << 24;
	c2 = c2 >> 16; // sign extend

	return (c1 & 0xff) + c2;
}
