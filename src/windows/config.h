// The Visual C++ C Runtime deprecates several standard library functions in
// preference for _s variants that are specific to Visual C++. This removes
// those deprecation warnings.
#define _CRT_SECURE_NO_WARNINGS

// The Visual C++ C Runtime deprecates standard POSIX APIs that conflict with
// reserved ISO C names (like strdup) in favour of non-portable conforming
// variants that start with an '_'. This removes those deprecation warnings.
#define _CRT_NONSTDC_NO_DEPRECATE

// This is created by autoconf on POSIX-compatibe build environments.
#define PACKAGE_VERSION "1.52.0"

// CMake's generated config.h defines every USE_* switch as 0 or 1. These
// projects define the enabled ones (USE_KLATT=1 and so on); the others must
// still be 0, because the Rust-core C sources use them as values, not only
// in #if. Neither asynchronous synthesis (event.c, fifo.c) nor libsonic is
// built here.
#ifndef USE_ASYNC
#define USE_ASYNC 0
#endif
#ifndef USE_LIBSONIC
#define USE_LIBSONIC 0
#endif
