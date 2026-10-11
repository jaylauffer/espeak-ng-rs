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

static espeak_ng_STATUS LoadSpect(CompileContext *ctx, const char *path, int control, int *addr)
{
	SpectSeq *spectseq;
	int peak;
	int frame;
	int n_frames;
	int ix;
	int x, x2;
	int rms;
	float total;
	float pkheight;
	int marker1_set = 0;
	int frame_vowelbreak = 0;
	int klatt_flag = 0;
	SpectFrame *fr;
	frame_t *fr_out;
	char filename[N_PATH_BUF];

	SPECT_SEQ seq_out;
	SPECT_SEQK seqk_out;

	// create SpectSeq and import data
	spectseq = SpectSeqCreate();
	if (spectseq == NULL)
		return ENOMEM;

	snprintf(filename, sizeof(filename), "%s/%s", ctx->phsrc, path);
	espeak_ng_STATUS status = LoadSpectSeq(spectseq, filename);
	if (status != ENS_OK) {
		error(ctx, "Bad vowel file: '%s'", path);
		SpectSeqDestroy(spectseq);
		return status;
	}

	// do we need additional klatt data ?
	for (frame = 0; frame < spectseq->numframes; frame++) {
		for (ix = 5; ix < N_KLATTP2; ix++) {
			if (spectseq->frames[frame]->klatt_param[ix] != 0)
				klatt_flag = FRFLAG_KLATT;
		}
	}

	*addr = ftell(ctx->f_phdata);

	seq_out.n_frames = 0;
	seq_out.sqflags = 0;
	seq_out.length_total = 0;

	total = 0;
	for (frame = 0; frame < spectseq->numframes; frame++) {
		if (spectseq->frames[frame]->keyframe) {
			if (seq_out.n_frames == 1)
				frame_vowelbreak = frame;
			if (spectseq->frames[frame]->markers & 0x2) {
				// marker 1 is set
				marker1_set = 1;
			}

			seq_out.n_frames++;
			if (frame > 0)
				total += spectseq->frames[frame-1]->length;
		}
	}
	seq_out.length_total = (int)total;

	if ((control & 1) && (marker1_set == 0)) {
		// This is a vowel, but no Vowel Break marker is set
		// set a marker flag for the second frame of a vowel
		spectseq->frames[frame_vowelbreak]->markers |= FRFLAG_VOWEL_CENTRE;
	}

	n_frames = 0;
	for (frame = 0; frame < spectseq->numframes; frame++) {
		fr = spectseq->frames[frame];

		if (fr->keyframe) {
			if (klatt_flag)
				fr_out = &seqk_out.frame[n_frames];
			else
				fr_out = (frame_t *)&seq_out.frame[n_frames];

			x = (int)(fr->length + 0.5); // round to nearest mS
			if (x > 255) x = 255;
			fr_out->length = x;

			fr_out->frflags = fr->markers | klatt_flag;

			rms = (int)GetFrameRms(fr, spectseq->amplitude);
			if (rms > 255) rms = 255;
			fr_out->rms = rms;

			if (n_frames == (seq_out.n_frames-1))
				fr_out->length = 0; // give last frame zero length

			// write: peak data
			ctx->count_frames++;
			for (peak = 0; peak < 8; peak++) {
				if (peak < 7)
					fr_out->ffreq[peak] = fr->peaks[peak].pkfreq;

				pkheight = spectseq->amplitude * fr->amp_adjust * fr->peaks[peak].pkheight;
				pkheight = pkheight/640000;
				if (pkheight > 255) pkheight = 255;
				fr_out->fheight[peak] = (int)pkheight;

				if (peak < 6) {
					x =  fr->peaks[peak].pkwidth/4;
					if (x > 255) x = 255;
					fr_out->fwidth[peak] = x;

					if (peak < 3) {
						x2 =  fr->peaks[peak].pkright/4;
						if (x2 > 255) x2 = 255;
						fr_out->fright[peak] = x2;
					}
				}

				if (peak < 4) {
					x = fr->peaks[peak].klt_bw / 2;
					if (x > 255) x = 255;
					fr_out->bw[peak] = x;
				}
			}

			for (ix = 0; ix < 5; ix++) {
				fr_out->klattp[ix] = fr->klatt_param[ix];

				fr_out->klattp[KLATT_FNZ] = fr->klatt_param[KLATT_FNZ] / 2;
			}

			if (klatt_flag) {
				// additional klatt parameters
				for (ix = 0; ix < 5; ix++)
					fr_out->klattp2[ix] = fr->klatt_param[ix+5];

				for (peak = 0; peak < 7; peak++) {
					fr_out->klatt_ap[peak] = fr->peaks[peak].klt_ap;

					x = fr->peaks[peak].klt_bp / 2;
					if (x > 255) x = 255;
					fr_out->klatt_bp[peak] = x;
				}
				fr_out->spare = 0;
			}

			if (fr_out->bw[1] == 0) {
				fr_out->bw[0] = 89 / 2;
				fr_out->bw[1] = 90 / 2;
				fr_out->bw[2] = 140 / 2;
				fr_out->bw[3] = 260 / 2;
			}

			n_frames++;
		}
	}

	if (klatt_flag) {
		seqk_out.n_frames = seq_out.n_frames;
		seqk_out.sqflags = seq_out.sqflags;
		seqk_out.length_total = seq_out.length_total;

		ix = (char *)(&seqk_out.frame[seqk_out.n_frames]) - (char *)(&seqk_out);
		fwrite(&seqk_out, ix, 1, ctx->f_phdata);
		while (ix & 3)
		{
			// round up to multiple of 4 bytes
			fputc(0, ctx->f_phdata);
			ix++;
		}
	} else {
		ix = (char *)(&seq_out.frame[seq_out.n_frames]) - (char *)(&seq_out);
		fwrite(&seq_out, ix, 1, ctx->f_phdata);
		while (ix & 3)
		{
			// round up to multiple of 4 bytes
			fputc(0, ctx->f_phdata);
			ix++;
		}
	}

	SpectSeqDestroy(spectseq);
	return ENS_OK;
}
