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

static const keywtab_t k_intonation[] = {
	{ "tune",       0, kTUNE },
	{ "endtune",    0, kENDTUNE },
	{ "prehead",    0, kTUNE_PREHEAD },
	{ "onset",      0, kTUNE_ONSET },
	{ "head",       0, kTUNE_HEAD },
	{ "headenv",    0, kTUNE_HEADENV },
	{ "headextend", 0, kTUNE_HEADEXTEND },
	{ "headlast",   0, kTUNE_HEADLAST },
	{ "nucleus0",   0, kTUNE_NUCLEUS0 },
	{ "nucleus",    0, kTUNE_NUCLEUS1 },
	{ "split",      0, kTUNE_SPLIT },

	{ NULL, 0, -1 }
};
