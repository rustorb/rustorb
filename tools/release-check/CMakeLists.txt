# The release workflow builds orb.c with this against an unpacked
# rustorb-c archive on Windows (MSVC), as an application would:
#
#   cmake -S tools/release-check -B build -DRUSTORB_C_PREFIX=<archive>
#   cmake --build build --config Release

cmake_minimum_required(VERSION 3.18)
project(rustorb_c_check LANGUAGES C)

set(RUSTORB_C_PREFIX "" CACHE PATH "an unpacked rustorb-c archive")
find_library(RUSTORB_C_LIB rustorb_c PATHS "${RUSTORB_C_PREFIX}/lib" NO_DEFAULT_PATH REQUIRED)
find_library(RUSTORB_FFI_LIB rustorb_ffi PATHS "${RUSTORB_C_PREFIX}/lib" NO_DEFAULT_PATH REQUIRED)

add_executable(orb-check orb.c)
target_include_directories(orb-check PRIVATE "${RUSTORB_C_PREFIX}/include")
target_link_libraries(orb-check PRIVATE "${RUSTORB_C_LIB}" "${RUSTORB_FFI_LIB}")
if(WIN32)
  # What the static rustorb_ffi.lib needs from the system.
  target_link_libraries(orb-check PRIVATE ws2_32 userenv ntdll bcrypt)
else()
  find_package(Threads REQUIRED)
  target_link_libraries(orb-check PRIVATE Threads::Threads ${CMAKE_DL_LIBS})
  if(APPLE)
    target_link_libraries(orb-check PRIVATE "-framework CoreFoundation" "-framework Security")
  else()
    target_link_libraries(orb-check PRIVATE m)
  endif()
endif()
