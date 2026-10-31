# The C++ example (examples/README.md), on the installed rustorb C++ package:
#
#   cmake -S examples/cpp -B build/examples-cpp -DCMAKE_PREFIX_PATH=PREFIX
#   cmake --build build/examples-cpp
#
# PREFIX is where rustorb-native was unpacked, or where
# `cmake --install` of cpp/ put it (docs/release.md). rustorb-idl must be
# on PATH.

cmake_minimum_required(VERSION 3.16)
project(rustorb_example CXX)

find_package(rustorb 0.1 REQUIRED)

# Echo.idl, and CosNaming.idl from rustorb-idl's standard IDL files.
rustorb_idl_generate(SOURCES ../idl/Echo.idl "${RUSTORB_IDL_DIR}/CosNaming.idl")
add_library(echo_idl STATIC ${SOURCES})
target_include_directories(echo_idl PUBLIC "${CMAKE_CURRENT_BINARY_DIR}/idl")
target_link_libraries(echo_idl PUBLIC rustorb::rustorb)

add_executable(server server.cc)
target_link_libraries(server PRIVATE echo_idl)
add_executable(client client.cc)
target_link_libraries(client PRIVATE echo_idl)
