# A project that uses the installed rustorb C++ binding (find_package), as
# an application would. CI installs rustorb and builds this (docs/release.md):
#
#   cmake -S cpp -B build/cpp-install -DRUSTORB_BUILD_TESTS=OFF
#   cmake --build build/cpp-install && cmake --install build/cpp-install --prefix PREFIX
#   cmake -S cpp/tests/consumer -B build/consumer -DCMAKE_PREFIX_PATH=PREFIX
#   cmake --build build/consumer && ctest --test-dir build/consumer

cmake_minimum_required(VERSION 3.16)
project(rustorb_consumer CXX)

find_package(rustorb 0.1 REQUIRED)

rustorb_idl_generate(SOURCES Echo.idl)
add_executable(echo main.cc ${SOURCES})
target_include_directories(echo PRIVATE "${CMAKE_CURRENT_BINARY_DIR}/idl")
target_link_libraries(echo PRIVATE rustorb::rustorb)

enable_testing()
add_test(NAME echo COMMAND echo)
