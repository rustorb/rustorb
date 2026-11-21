# The rustorb C binding: librustorb_c (the OMG C Language Mapping runtime,
# in C99, on the C ABI of crates/rustorb-ffi) and its tests.
#
#   cmake -S c -B build/c -DIDL_PARSER_DIR=path/to/idl_parser
#   cmake --build build/c && ctest --test-dir build/c
#
# The build runs cargo for librustorb_ffi (static) and rustorb-idl -bc for
# the test IDLs; rustorb-idl needs idl_parser, importable or in
# IDL_PARSER_DIR. Set RUSTORB_FFI_LIBRARY to use a librustorb_ffi built
# elsewhere instead. With -DRUSTORB_BUILD_TESTS=OFF only the libraries are
# built; `cmake --install` puts the headers and the libraries under
# CMAKE_INSTALL_PREFIX.

cmake_minimum_required(VERSION 3.16)
project(rustorb_c VERSION 0.6.0 LANGUAGES C)

set(CMAKE_C_STANDARD 99)
set(CMAKE_C_STANDARD_REQUIRED ON)
set(CMAKE_C_EXTENSIONS OFF)

get_filename_component(RUSTORB_ROOT "${CMAKE_CURRENT_SOURCE_DIR}/.." ABSOLUTE)
set(IDL_PARSER_DIR "" CACHE PATH "a checkout of idl_parser, if it is not importable")
set(RUSTORB_FFI_LIBRARY "" CACHE FILEPATH "librustorb_ffi to link (default: built with cargo)")
option(RUSTORB_BUILD_TESTS "build the tests (needs idl_parser)" ON)

find_package(Threads REQUIRED)
if(RUSTORB_BUILD_TESTS)
  find_package(Python3 3.9 REQUIRED COMPONENTS Interpreter)
endif()

if(RUSTORB_FFI_LIBRARY)
  add_custom_target(rustorb_ffi_build)
else()
  find_program(CARGO cargo REQUIRED)
  set(RUSTORB_FFI_LIBRARY
      "${RUSTORB_ROOT}/target/release/${CMAKE_STATIC_LIBRARY_PREFIX}rustorb_ffi${CMAKE_STATIC_LIBRARY_SUFFIX}")
  # Always run: cargo knows whether it is up to date.
  add_custom_target(rustorb_ffi_build ALL
    COMMAND "${CARGO}" build --release --locked -p rustorb-ffi
            --manifest-path "${RUSTORB_ROOT}/Cargo.toml"
    BYPRODUCTS "${RUSTORB_FFI_LIBRARY}"
    COMMENT "Building librustorb_ffi with cargo"
    VERBATIM)
endif()

# What the static librustorb_ffi needs from the system.
set(RUSTORB_SYSTEM_LIBS ${CMAKE_DL_LIBS})
if(UNIX AND NOT APPLE)
  list(APPEND RUSTORB_SYSTEM_LIBS m)
elseif(APPLE)
  list(APPEND RUSTORB_SYSTEM_LIBS "-framework CoreFoundation" "-framework Security")
elseif(WIN32)
  list(APPEND RUSTORB_SYSTEM_LIBS ws2_32 userenv ntdll bcrypt)
endif()

if(MSVC)
  set(RUSTORB_WARNINGS /W4)
else()
  set(RUSTORB_WARNINGS -Wall -Wextra -Wpedantic -Werror)
endif()

set(RUSTORB_C_SOURCES
  src/any.c src/dii.c src/dynany.c src/env.c src/invoke.c src/managers.c src/marshal.c src/memory.c src/orb.c src/poa.c
  src/types.c src/CosNaming.c)
add_library(rustorb_c STATIC ${RUSTORB_C_SOURCES})
add_dependencies(rustorb_c rustorb_ffi_build)
target_include_directories(rustorb_c
  PUBLIC "${CMAKE_CURRENT_SOURCE_DIR}/include" "${RUSTORB_ROOT}/crates/rustorb-ffi/include"
  PRIVATE "${CMAKE_CURRENT_SOURCE_DIR}/include/rustorb")
target_compile_options(rustorb_c PRIVATE ${RUSTORB_WARNINGS})
target_link_libraries(rustorb_c PUBLIC "${RUSTORB_FFI_LIBRARY}" Threads::Threads ${RUSTORB_SYSTEM_LIBS})

include(GNUInstallDirs)
install(DIRECTORY include/rustorb DESTINATION "${CMAKE_INSTALL_INCLUDEDIR}")
install(FILES "${RUSTORB_ROOT}/crates/rustorb-ffi/include/rustorb/rustorb.h"
        DESTINATION "${CMAKE_INSTALL_INCLUDEDIR}/rustorb")
install(TARGETS rustorb_c ARCHIVE DESTINATION "${CMAKE_INSTALL_LIBDIR}")
install(FILES "${RUSTORB_FFI_LIBRARY}" DESTINATION "${CMAKE_INSTALL_LIBDIR}")

if(NOT RUSTORB_BUILD_TESTS)
  return()
endif()

# rustorb_idl_generate(<var> <idl>...): runs rustorb-idl -bc on each IDL
# file and appends the generated sources to <var>. rustorb-idl is the pinned
# one (pip install -r tools/rustorb-idl.txt), or the checkout in
# -DRUSTORB_IDL_DIR=...; -DIDL_PARSER_DIR=... puts an idl_parser checkout
# first.
set(RUSTORB_IDL_DIR "" CACHE PATH "a rustorb-idl checkout to use instead of the installed one")
set(RUSTORB_IDL_PYTHONPATH "")
set(RUSTORB_IDL_GENERATOR "")
if(RUSTORB_IDL_DIR)
  set(RUSTORB_IDL_PYTHONPATH "${RUSTORB_IDL_DIR}/python")
  file(GLOB RUSTORB_IDL_GENERATOR "${RUSTORB_IDL_DIR}/python/rustorb_idl/*.py")
endif()
if(IDL_PARSER_DIR)
  if(NOT RUSTORB_IDL_PYTHONPATH)
    set(RUSTORB_IDL_PYTHONPATH "${IDL_PARSER_DIR}")
  elseif(WIN32)
    set(RUSTORB_IDL_PYTHONPATH "${IDL_PARSER_DIR}\;${RUSTORB_IDL_PYTHONPATH}")
  else()
    set(RUSTORB_IDL_PYTHONPATH "${IDL_PARSER_DIR}:${RUSTORB_IDL_PYTHONPATH}")
  endif()
endif()
function(rustorb_idl_generate var)
  set(outdir "${CMAKE_CURRENT_BINARY_DIR}/idl")
  set(sources ${${var}})
  foreach(idl ${ARGN})
    get_filename_component(base "${idl}" NAME_WE)
    add_custom_command(
      OUTPUT "${outdir}/${base}.h" "${outdir}/${base}.c"
      COMMAND "${CMAKE_COMMAND}" -E env "PYTHONPATH=${RUSTORB_IDL_PYTHONPATH}"
              "${Python3_EXECUTABLE}" -m rustorb_idl -bc -C "${outdir}" "${idl}"
      DEPENDS "${idl}" ${RUSTORB_IDL_GENERATOR}
      COMMENT "rustorb-idl -bc ${base}.idl"
      VERBATIM)
    list(APPEND sources "${outdir}/${base}.c")
  endforeach()
  set(${var} ${sources} PARENT_SCOPE)
endfunction()

# -DRUSTORB_SANITIZE=address builds everything with AddressSanitizer, whose
# LeakSanitizer reports at exit what the runtime did not free.
set(RUSTORB_SANITIZE "" CACHE STRING "sanitizers (-fsanitize=...)")
if(RUSTORB_SANITIZE)
  target_compile_options(rustorb_c PRIVATE -fsanitize=${RUSTORB_SANITIZE} -fno-omit-frame-pointer)
  add_compile_options(-fsanitize=${RUSTORB_SANITIZE} -fno-omit-frame-pointer)
  add_link_options(-fsanitize=${RUSTORB_SANITIZE})
endif()

enable_testing()

# The interop cases of interop/impls/rustorbc against its Echo servant, in
# one process.
set(RUSTORBC "${RUSTORB_ROOT}/interop/impls/rustorbc/src")
rustorb_idl_generate(INTEROP_SOURCES "${RUSTORB_ROOT}/interop/idl/InteropTest.idl")
add_executable(test_interop tests/test_interop.c "${RUSTORBC}/cases.c" "${RUSTORBC}/echo_impl.c"
               "${RUSTORBC}/dii_adapters.c" ${INTEROP_SOURCES})
target_include_directories(test_interop PRIVATE "${CMAKE_CURRENT_BINARY_DIR}/idl" "${RUSTORBC}")
target_link_libraries(test_interop PRIVATE rustorb_c)
target_compile_options(test_interop PRIVATE ${RUSTORB_WARNINGS})
add_test(NAME interop COMMAND test_interop)
# The same through the DII and the DSI (<rustorb/dii.h>).
add_test(NAME interop_dii COMMAND test_interop dii)
add_test(NAME interop_deferred COMMAND test_interop deferred)

# The DII and the DSI against the Echo servant and a DynamicImpl.
add_executable(test_dii tests/test_dii.c "${RUSTORBC}/echo_impl.c" ${INTEROP_SOURCES})
target_include_directories(test_dii PRIVATE "${CMAKE_CURRENT_BINARY_DIR}/idl" "${RUSTORBC}")
target_link_libraries(test_dii PRIVATE rustorb_c)
target_compile_options(test_dii PRIVATE ${RUSTORB_WARNINGS})
add_test(NAME dii COMMAND test_dii)

# The C++ mapping test IDL (constants, recursion, every discriminator,
# nested types, keywords, fixed, long double, inheritance) and the any and
# TypeCode functions.
rustorb_idl_generate(MAPPING_SOURCES "${RUSTORB_ROOT}/cpp/tests/idl/CxxMapping.idl"
                     "${RUSTORB_ROOT}/interop/idl/AnyTest.idl")
add_executable(test_mapping tests/test_mapping.c ${MAPPING_SOURCES})
target_include_directories(test_mapping PRIVATE "${CMAKE_CURRENT_BINARY_DIR}/idl")
target_link_libraries(test_mapping PRIVATE rustorb_c)
target_compile_options(test_mapping PRIVATE ${RUSTORB_WARNINGS})
add_test(NAME mapping COMMAND test_mapping)

# Local interfaces (LocalTest.idl, the one of the .NET tests).
rustorb_idl_generate(LOCAL_SOURCES "${RUSTORB_ROOT}/dotnet/tests/Rustorb.Tests/idl/LocalTest.idl")
add_executable(test_local tests/test_local.c ${LOCAL_SOURCES})
target_include_directories(test_local PRIVATE "${CMAKE_CURRENT_BINARY_DIR}/idl")
target_link_libraries(test_local PRIVATE rustorb_c)
target_compile_options(test_local PRIVATE ${RUSTORB_WARNINGS})
add_test(NAME local COMMAND test_local)

# DynamicAny (<rustorb/dynany.h>) on the types of AnyTest.idl.
add_executable(test_dynany tests/test_dynany.c "${CMAKE_CURRENT_BINARY_DIR}/idl/AnyTest.c")
target_include_directories(test_dynany PRIVATE "${CMAKE_CURRENT_BINARY_DIR}/idl")
target_link_libraries(test_dynany PRIVATE rustorb_c)
target_compile_options(test_dynany PRIVATE ${RUSTORB_WARNINGS})
add_test(NAME dynany COMMAND test_dynany)
