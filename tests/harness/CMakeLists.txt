# Test harness: runs self-authored trees under BehaviorTree.CPP 4.9.0 (MIT)
# and writes the oracle files the Rust tests compare against (spec §8).
cmake_minimum_required(VERSION 3.16.3)
project(coppice_harness LANGUAGES CXX)

set(CMAKE_CXX_STANDARD 17)
set(CMAKE_CXX_STANDARD_REQUIRED ON)

include(FetchContent)

# Extracted files take the time of extraction.
if(POLICY CMP0135)
  cmake_policy(SET CMP0135 NEW)
endif()

# Only the core library: no tools, examples, tests, ZeroMQ or SQLite.
set(BTCPP_SHARED_LIBS OFF CACHE BOOL "" FORCE)
set(BTCPP_BUILD_TOOLS OFF CACHE BOOL "" FORCE)
set(BTCPP_EXAMPLES OFF CACHE BOOL "" FORCE)
set(BUILD_TESTING OFF CACHE BOOL "" FORCE)
set(BTCPP_GROOT_INTERFACE OFF CACHE BOOL "" FORCE)
set(BTCPP_SQLITE_LOGGING OFF CACHE BOOL "" FORCE)

# The release archive, pinned by hash. To build offline, pass
# -DFETCHCONTENT_SOURCE_DIR_BEHAVIORTREE_CPP=<extracted 4.9.0 tree>. That
# skips the hash, so the version label of its project() line is checked
# below; the copy's contents are not.
FetchContent_Declare(
  behaviortree_cpp
  URL https://github.com/BehaviorTree/BehaviorTree.CPP/archive/refs/tags/4.9.0.tar.gz
  URL_HASH SHA256=74a22cf46d7cd423d7065616528cfd68bcd925b3fc2b819a99413cdd3334c02a
)
FetchContent_MakeAvailable(behaviortree_cpp)

file(STRINGS "${behaviortree_cpp_SOURCE_DIR}/CMakeLists.txt" btcpp_project
     REGEX "^project\\(behaviortree_cpp VERSION ")
if(NOT btcpp_project MATCHES "VERSION 4\\.9\\.0[ )]")
  message(FATAL_ERROR "expected BehaviorTree.CPP 4.9.0 in ${behaviortree_cpp_SOURCE_DIR}, found: ${btcpp_project}")
endif()

add_executable(btlog_harness btlog_harness.cpp)
target_link_libraries(btlog_harness PRIVATE behaviortree_cpp)
