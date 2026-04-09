cmake_minimum_required(VERSION 3.16)
project(saikuro_cpp_adapter LANGUAGES CXX)

add_library(saikuro_cpp_adapter INTERFACE)
target_compile_features(saikuro_cpp_adapter INTERFACE cxx_std_11)

target_include_directories(saikuro_cpp_adapter
    INTERFACE
        ${CMAKE_CURRENT_SOURCE_DIR}/include
        ${CMAKE_CURRENT_SOURCE_DIR}/../c/include
)

# Consumer links the C adapter library, which should be built via Cargo as saikuro_c.

add_executable(saikuro_cpp_header_compile_test tests/header_compile.cpp)
target_link_libraries(saikuro_cpp_header_compile_test PRIVATE saikuro_cpp_adapter)
