cmake_minimum_required(VERSION 3.16)
project(saikuro_cpp_adapter LANGUAGES CXX)

enable_testing()

add_library(saikuro_cpp_adapter INTERFACE)
target_compile_features(saikuro_cpp_adapter INTERFACE cxx_std_11)

target_include_directories(saikuro_cpp_adapter
    INTERFACE
        ${CMAKE_CURRENT_SOURCE_DIR}/include
        ${CMAKE_CURRENT_SOURCE_DIR}/../c/include
)

# Consumer links the C adapter library, which should be built via Cargo as saikuro_c.

add_library(saikuro_cpp_schema_extractor STATIC
    src/schema_extractor.cpp
)
target_compile_features(saikuro_cpp_schema_extractor PUBLIC cxx_std_11)
target_include_directories(saikuro_cpp_schema_extractor
    PUBLIC
        ${CMAKE_CURRENT_SOURCE_DIR}/include
)

add_executable(saikuro_cpp_schema_cli src/saikuro_cpp_schema.cpp)
target_link_libraries(saikuro_cpp_schema_cli PRIVATE saikuro_cpp_schema_extractor)
set_target_properties(saikuro_cpp_schema_cli PROPERTIES OUTPUT_NAME "saikuro-cpp-schema")

# TESTS
add_executable(saikuro_cpp_header_compile_test tests/header_compile.cpp)
target_link_libraries(saikuro_cpp_header_compile_test PRIVATE saikuro_cpp_adapter)

add_executable(saikuro_cpp_wrapper_behavior_test tests/wrapper_behavior.cpp)
target_link_libraries(saikuro_cpp_wrapper_behavior_test PRIVATE saikuro_cpp_adapter)

add_executable(saikuro_cpp_schema_extractor_test tests/schema_extractor_cli.cpp)
target_link_libraries(saikuro_cpp_schema_extractor_test PRIVATE saikuro_cpp_schema_extractor)
target_compile_definitions(saikuro_cpp_schema_extractor_test PRIVATE
    SAIKURO_SCHEMA_FIXTURE="${CMAKE_CURRENT_SOURCE_DIR}/../c/tests/fixtures/schema_service.h"
)

add_test(NAME saikuro_cpp_header_compile_test COMMAND saikuro_cpp_header_compile_test)
add_test(NAME saikuro_cpp_wrapper_behavior_test COMMAND saikuro_cpp_wrapper_behavior_test)
add_test(NAME saikuro_cpp_schema_extractor_test COMMAND saikuro_cpp_schema_extractor_test)

add_test(
    NAME saikuro_cpp_schema_cli_success_test
    COMMAND $<TARGET_FILE:saikuro_cpp_schema_cli> --namespace parityns ${CMAKE_CURRENT_SOURCE_DIR}/../c/tests/fixtures/schema_service.h
)

add_test(
    NAME saikuro_cpp_schema_cli_missing_file_test
    COMMAND $<TARGET_FILE:saikuro_cpp_schema_cli> --namespace parityns ${CMAKE_CURRENT_SOURCE_DIR}/../c/tests/fixtures/does_not_exist.h
)
set_tests_properties(saikuro_cpp_schema_cli_missing_file_test PROPERTIES WILL_FAIL TRUE)
