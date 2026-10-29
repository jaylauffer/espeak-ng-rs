/* Frozen test fixture: the C host the native compilers received before
 * their primitives moved to Rust (rust/compiler_engine_compat.rs), copied from
 * src/libespeak-ng/compiledata.c as the test generator extracted it.
 * SPDX-License-Identifier: GPL-3.0-or-later */
/* Begin native intonation admission projection. */
	RustNativeIntonationCompiler compiler = {
		source_path, destination_path, path_home, log, stderr, context, N_PATH_BUF,
		IntonationReload, IntonationError
	};
	return espeak_rs_compile_intonation_native(&compiler);
