# Native Rust replacements; the legacy C build remains the reference oracle.
find_program(ESPEAK_CARGO_EXECUTABLE cargo REQUIRED)
set(ESPEAK_RUST_TARGET "" CACHE STRING "Rust target triple (required for cross compilation)")
if(CMAKE_CROSSCOMPILING AND NOT ESPEAK_RUST_TARGET)
  message(FATAL_ERROR "USE_RUST_CORE cross builds require -DESPEAK_RUST_TARGET=<triple>")
endif()

set(_rust_target_args)
set(_rust_target_dir "${CMAKE_BINARY_DIR}/rust-target")
set(_rust_profile_dir "${_rust_target_dir}")
if(ESPEAK_RUST_TARGET)
  list(APPEND _rust_target_args --target "${ESPEAK_RUST_TARGET}")
  set(_rust_profile_dir "${_rust_profile_dir}/${ESPEAK_RUST_TARGET}")
endif()
if(WIN32 AND NOT MINGW)
  set(_rust_library "${_rust_profile_dir}/release/espeak_ng_rs.lib")
else()
  set(_rust_library "${_rust_profile_dir}/release/libespeak_ng_rs.a")
endif()
file(GLOB _rust_sources CONFIGURE_DEPENDS "${CMAKE_SOURCE_DIR}/rust/*.rs")
add_custom_command(
  OUTPUT "${_rust_library}"
  COMMAND "${ESPEAK_CARGO_EXECUTABLE}" build --locked --release --features c-abi
    --manifest-path "${CMAKE_SOURCE_DIR}/Cargo.toml"
    --target-dir "${_rust_target_dir}" ${_rust_target_args}
  DEPENDS ${_rust_sources} "${CMAKE_SOURCE_DIR}/Cargo.toml" "${CMAKE_SOURCE_DIR}/Cargo.lock"
  COMMENT "Building native Rust text and Unicode core"
  VERBATIM
)
add_custom_target(espeak-rust-build DEPENDS "${_rust_library}")
add_library(espeak-rust-core STATIC IMPORTED GLOBAL)
set_target_properties(espeak-rust-core PROPERTIES IMPORTED_LOCATION "${_rust_library}")
add_dependencies(espeak-rust-core espeak-rust-build)
if(WIN32)
  set_property(TARGET espeak-rust-core PROPERTY INTERFACE_LINK_LIBRARIES
    "advapi32;bcrypt;kernel32;ntdll;userenv;ws2_32")
else()
  set_property(TARGET espeak-rust-core PROPERTY INTERFACE_LINK_LIBRARIES
    "${CMAKE_DL_LIBS};Threads::Threads;m")
endif()
