if(USE_RUST_CORE)
  add_library(ucd INTERFACE)
  target_include_directories(ucd INTERFACE src/include)
  target_link_libraries(ucd INTERFACE espeak-rust-core)
else()
  add_library(ucd STATIC
    src/case.c
    src/categories.c
    src/ctype.c
    src/proplist.c
    src/scripts.c
    src/tostring.c
  )
  # enable PIC in order to embed libucd.a in a .so
  set_property(TARGET ucd PROPERTY POSITION_INDEPENDENT_CODE ON)

  target_include_directories(ucd PUBLIC src/include)
endif()
