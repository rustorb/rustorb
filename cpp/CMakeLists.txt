# The rustorb C++ binding: the headers in include/ (the IDL to C++11 mapping
# runtime, header only) on the C ABI of crates/rustorb-ffi, and its tests.
#
#   cmake -S cpp -B build/cpp -DIDL_PARSER_DIR=path/to/idl_parser
#   cmake --build build/cpp && ctest --test-dir build/cpp
#
# The build runs cargo for librustorb_ffi (static) and rustorb-idl -bc++11
# for the test IDLs; rustorb-idl needs idl_parser, importable or in
# IDL_PARSER_DIR. Set RUSTORB_FFI_LIBRARY to use a librustorb_ffi built
# elsewhere instead.
#
# Installing (docs/release.md): with -DRUSTORB_BUILD_TESTS=OFF only
# librustorb_ffi is built, and `cmake --install` puts the headers, the
# library and a CMake package (find_package(rustorb), the rustorb::rustorb
# target and rustorb_idl_generate()) under CMAKE_INSTALL_PREFIX.

cmake_minimum_required(VERSION 3.16)
project(rustorb_cpp VERSION 0.1.1 LANGUAGES CXX)

set(CMAKE_CXX_STANDARD 17)
set(CMAKE_CXX_STANDARD_REQUIRED ON)
set(CMAKE_CXX_EXTENSIONS OFF)

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

add_library(rustorb INTERFACE)
add_dependencies(rustorb rustorb_ffi_build)
target_include_directories(rustorb INTERFACE
  "${CMAKE_CURRENT_SOURCE_DIR}/include"
  "${RUSTORB_ROOT}/crates/rustorb-ffi/include")
# What the static librustorb_ffi needs from the system (also written into
# the installed package).
set(RUSTORB_SYSTEM_LIBS ${CMAKE_DL_LIBS})
if(UNIX AND NOT APPLE)
  list(APPEND RUSTORB_SYSTEM_LIBS m)
elseif(APPLE)
  list(APPEND RUSTORB_SYSTEM_LIBS "-framework CoreFoundation" "-framework Security")
elseif(WIN32)
  list(APPEND RUSTORB_SYSTEM_LIBS ws2_32 userenv ntdll bcrypt)
endif()
target_link_libraries(rustorb INTERFACE "${RUSTORB_FFI_LIBRARY}" Threads::Threads ${RUSTORB_SYSTEM_LIBS})

include(GNUInstallDirs)
include(CMakePackageConfigHelpers)
get_filename_component(RUSTORB_FFI_LIBRARY_NAME "${RUSTORB_FFI_LIBRARY}" NAME)
install(DIRECTORY include/rustorb DESTINATION "${CMAKE_INSTALL_INCLUDEDIR}")
install(FILES "${RUSTORB_ROOT}/crates/rustorb-ffi/include/rustorb/rustorb.h"
        DESTINATION "${CMAKE_INSTALL_INCLUDEDIR}/rustorb")
install(FILES "${RUSTORB_FFI_LIBRARY}" DESTINATION "${CMAKE_INSTALL_LIBDIR}")
set(RUSTORB_CMAKE_DIR "${CMAKE_INSTALL_LIBDIR}/cmake/rustorb")
configure_package_config_file(cmake/rustorbConfig.cmake.in
  "${CMAKE_CURRENT_BINARY_DIR}/rustorbConfig.cmake"
  INSTALL_DESTINATION "${RUSTORB_CMAKE_DIR}"
  PATH_VARS CMAKE_INSTALL_INCLUDEDIR CMAKE_INSTALL_LIBDIR)
# 0.x: a minor version may break the API.
write_basic_package_version_file("${CMAKE_CURRENT_BINARY_DIR}/rustorbConfigVersion.cmake"
  COMPATIBILITY SameMinorVersion)
install(FILES "${CMAKE_CURRENT_BINARY_DIR}/rustorbConfig.cmake"
              "${CMAKE_CURRENT_BINARY_DIR}/rustorbConfigVersion.cmake"
        DESTINATION "${RUSTORB_CMAKE_DIR}")

if(NOT RUSTORB_BUILD_TESTS)
  return()
endif()

# rustorb_idl_generate(<var> <idl>...): runs rustorb-idl -bc++11 on each IDL
# file and appends the generated sources to <var>.
set(RUSTORB_IDL_PYTHONPATH "${RUSTORB_ROOT}/crates/rustorb-codegen/python")
if(IDL_PARSER_DIR)
  if(WIN32)
    set(RUSTORB_IDL_PYTHONPATH "${IDL_PARSER_DIR}\;${RUSTORB_IDL_PYTHONPATH}")
  else()
    set(RUSTORB_IDL_PYTHONPATH "${IDL_PARSER_DIR}:${RUSTORB_IDL_PYTHONPATH}")
  endif()
endif()
file(GLOB RUSTORB_IDL_GENERATOR "${RUSTORB_ROOT}/crates/rustorb-codegen/python/rustorb_idl/*.py")
function(rustorb_idl_generate var)
  set(outdir "${CMAKE_CURRENT_BINARY_DIR}/idl")
  set(sources ${${var}})
  foreach(idl ${ARGN})
    get_filename_component(base "${idl}" NAME_WE)
    add_custom_command(
      OUTPUT "${outdir}/${base}.hh" "${outdir}/${base}.cc"
      COMMAND "${CMAKE_COMMAND}" -E env "PYTHONPATH=${RUSTORB_IDL_PYTHONPATH}"
              "${Python3_EXECUTABLE}" -m rustorb_idl -bc++11 -C "${outdir}" "${idl}"
      DEPENDS "${idl}" ${RUSTORB_IDL_GENERATOR}
      COMMENT "rustorb-idl -bc++11 ${base}.idl"
      VERBATIM)
    list(APPEND sources "${outdir}/${base}.cc")
  endforeach()
  set(${var} ${sources} PARENT_SCOPE)
endfunction()

# -DRUSTORB_SANITIZE=address builds the tests with AddressSanitizer, whose
# LeakSanitizer reports at exit what the C++ runtime and librustorb_ffi did
# not free (both allocate with the same malloc).
set(RUSTORB_SANITIZE "" CACHE STRING "sanitizers for the tests (-fsanitize=...)")
if(RUSTORB_SANITIZE)
  add_compile_options(-fsanitize=${RUSTORB_SANITIZE} -fno-omit-frame-pointer)
  add_link_options(-fsanitize=${RUSTORB_SANITIZE})
endif()

if(MSVC)
  set(RUSTORB_WARNINGS /W4)
else()
  set(RUSTORB_WARNINGS -Wall -Wextra -Werror)
endif()

enable_testing()

rustorb_idl_generate(MAPPING_SOURCES "${CMAKE_CURRENT_SOURCE_DIR}/tests/idl/CxxMapping.idl")
add_executable(test_mapping tests/test_mapping.cc ${MAPPING_SOURCES})
target_include_directories(test_mapping PRIVATE "${CMAKE_CURRENT_BINARY_DIR}/idl")
target_link_libraries(test_mapping PRIVATE rustorb)
target_compile_options(test_mapping PRIVATE ${RUSTORB_WARNINGS})
add_test(NAME mapping COMMAND test_mapping)

# The IDL 4 constructs of the C++ generator (int8, bitmask, bitset,
# annotations).
rustorb_idl_generate(IDL4_SOURCES "${CMAKE_CURRENT_SOURCE_DIR}/tests/idl/Idl4.idl")
add_executable(test_idl4 tests/test_idl4.cc ${IDL4_SOURCES})
target_include_directories(test_idl4 PRIVATE "${CMAKE_CURRENT_BINARY_DIR}/idl")
target_link_libraries(test_idl4 PRIVATE rustorb)
target_compile_options(test_idl4 PRIVATE ${RUSTORB_WARNINGS})
add_test(NAME idl4 COMMAND test_idl4)

# Servant managers, adapter activators and default servants.
add_executable(test_poa tests/test_poa.cc ${MAPPING_SOURCES})
target_include_directories(test_poa PRIVATE "${CMAKE_CURRENT_BINARY_DIR}/idl")
target_link_libraries(test_poa PRIVATE rustorb)
target_compile_options(test_poa PRIVATE ${RUSTORB_WARNINGS})
add_test(NAME poa COMMAND test_poa)

# The interop test cases of interop/impls/rustorbcpp, in one process.
rustorb_idl_generate(INTEROP_SOURCES "${RUSTORB_ROOT}/interop/idl/InteropTest.idl")
add_executable(test_interop tests/test_interop.cc ${INTEROP_SOURCES})
target_include_directories(test_interop PRIVATE
  "${CMAKE_CURRENT_BINARY_DIR}/idl" "${RUSTORB_ROOT}/interop/impls/rustorbcpp")
target_link_libraries(test_interop PRIVATE rustorb)
target_compile_options(test_interop PRIVATE ${RUSTORB_WARNINGS})
add_test(NAME interop COMMAND test_interop)
# any and TypeCode: the stage 2 interop cases, in one process.
rustorb_idl_generate(ANY_SOURCES "${RUSTORB_ROOT}/interop/idl/AnyTest.idl"
                     "${RUSTORB_ROOT}/interop/idl/AnyTestHidden.idl")
add_executable(test_any tests/test_any.cc ${ANY_SOURCES})
target_include_directories(test_any PRIVATE
  "${CMAKE_CURRENT_BINARY_DIR}/idl" "${RUSTORB_ROOT}/interop/impls/rustorbcpp")
target_link_libraries(test_any PRIVATE rustorb)
target_compile_options(test_any PRIVATE ${RUSTORB_WARNINGS})
add_test(NAME any COMMAND test_any)
# Valuetypes and abstract interfaces: their interop cases, in one process.
rustorb_idl_generate(VALUE_SOURCES "${RUSTORB_ROOT}/interop/idl/ValueTest.idl")
rustorb_idl_generate(ABSTRACT_SOURCES "${RUSTORB_ROOT}/interop/idl/AbstractTest.idl")
add_executable(test_value tests/test_value.cc ${VALUE_SOURCES} ${ABSTRACT_SOURCES})
target_include_directories(test_value PRIVATE
  "${CMAKE_CURRENT_BINARY_DIR}/idl" "${RUSTORB_ROOT}/interop/impls/rustorbcpp")
target_link_libraries(test_value PRIVATE rustorb)
target_compile_options(test_value PRIVATE ${RUSTORB_WARNINGS})
add_test(NAME value COMMAND test_value)
# DynamicAny, on the types of AnyTest.idl and ValueTest.idl.
add_executable(test_dynany tests/test_dynany.cc ${ANY_SOURCES} ${VALUE_SOURCES})
target_include_directories(test_dynany PRIVATE
  "${CMAKE_CURRENT_BINARY_DIR}/idl" "${RUSTORB_ROOT}/interop/impls/rustorbcpp")
target_link_libraries(test_dynany PRIVATE rustorb)
target_compile_options(test_dynany PRIVATE ${RUSTORB_WARNINGS})
add_test(NAME dynany COMMAND test_dynany)
# Client call timeouts, against a server that never answers (POSIX sockets).
if(UNIX)
  add_executable(test_timeout tests/test_timeout.cc)
  target_link_libraries(test_timeout PRIVATE rustorb)
  target_compile_options(test_timeout PRIVATE ${RUSTORB_WARNINGS})
  add_test(NAME timeout COMMAND test_timeout)
  set_tests_properties(timeout PROPERTIES TIMEOUT 120)
endif()
# ORB operations: create_policy, initial references, policy overrides.
add_executable(test_orb_ops tests/test_orb_ops.cc)
target_link_libraries(test_orb_ops PRIVATE rustorb)
target_compile_options(test_orb_ops PRIVATE ${RUSTORB_WARNINGS})
add_test(NAME orb_ops COMMAND test_orb_ops)
# Servants, references, exceptions, TypeCodes and anys made and dropped
# many times; with RUSTORB_SANITIZE=address, LeakSanitizer checks the rest.
add_executable(test_leak tests/test_leak.cc ${INTEROP_SOURCES} ${ANY_SOURCES})
target_include_directories(test_leak PRIVATE
  "${CMAKE_CURRENT_BINARY_DIR}/idl" "${RUSTORB_ROOT}/interop/impls/rustorbcpp")
target_link_libraries(test_leak PRIVATE rustorb)
target_compile_options(test_leak PRIVATE ${RUSTORB_WARNINGS})
add_test(NAME leak COMMAND test_leak)
set_tests_properties(mapping poa interop any value dynany orb_ops leak PROPERTIES TIMEOUT 120)
if(RUSTORB_SANITIZE MATCHES "address")
  # The servant of this one receives and returns valuetype graphs with
  # cycles, which the shared_ptr based mapping does not free (the
  # application breaks them, but cannot for a result; docs/valuetype.md).
  # It still runs with AddressSanitizer.
  set_tests_properties(value PROPERTIES ENVIRONMENT "ASAN_OPTIONS=detect_leaks=0")
endif()
