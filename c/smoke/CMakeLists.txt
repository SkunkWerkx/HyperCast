# The smoke consumers, through find_package: run.sh points CMAKE_PREFIX_PATH at an unpacked
# release archive and builds them once against each library the archive has.
#
#   smoke_c<std>                 main.c as C99, C11 and C23, each where the compiler has it
#   smoke_cxx<std>               main.cpp through hypercast.hpp as C++17, C++20 and C++23: the
#                                header's surface as code written before 1.1 uses it, unchanged
#   smoke_detailed<std>          detailed.cpp: the 1.1 surface (detail codes, grouping policies,
#                                the detailed doors, date options, month names), the same standards
#   smoke_cxx<std>_noexceptions  the same two with exceptions off (-fno-exceptions; /EHs-c- and
#   smoke_detailed<std>_noexceptions
#                                _HAS_EXCEPTIONS=0 on MSVC)
#   smoke_corpus                 corpus.cpp: the conformance corpus replayed through the header
#   smoke_downgrade              downgrade.cpp: the header's guard against a core older than 1.1
#                                (run.sh also builds it against a 1.0 core when it has one)
#
# Every one with warnings as errors (-Wall -Wextra -Wpedantic -Werror; /W4 /WX on MSVC).
# programs.txt lists what was built, and <program>-path-<config>.txt where each landed (inside
# an .app on iOS; under the configuration's folder in Visual Studio).
cmake_minimum_required(VERSION 3.21)
project(hypercast_smoke C)

option(HYPERCAST_SMOKE_STATIC "Link hypercast::hypercast_static instead of hypercast::hypercast" OFF)
# The C++ programs are built wherever there is a C++ compiler; ON makes its absence an error.
option(HYPERCAST_SMOKE_CXX "Require the C++ smoke programs" OFF)

find_package(hypercast REQUIRED CONFIG)

if(MSVC)
  set(_warnings /W4 /WX)
else()
  set(_warnings -Wall -Wextra -Wpedantic -Werror)
endif()

set(_programs)

# The version of the archive, packed as hypercast_version() packs it: main.c refuses a library
# older than the headers it was built against.
string(REGEX MATCH "^([0-9]+)\\.([0-9]+)\\.([0-9]+)" _version "${hypercast_VERSION}")
math(EXPR _packed "(${CMAKE_MATCH_1} << 16) | (${CMAKE_MATCH_2} << 8) | ${CMAKE_MATCH_3}")

foreach(standard 99 11 23)
  if(DEFINED CMAKE_C${standard}_STANDARD_COMPILE_OPTION)
    add_executable(smoke_c${standard} main.c)
    target_compile_definitions(smoke_c${standard} PRIVATE HYPERCAST_SMOKE_MIN_VERSION=${_packed})
    set_target_properties(smoke_c${standard} PROPERTIES C_STANDARD ${standard} C_STANDARD_REQUIRED ON
      C_EXTENSIONS OFF)
    list(APPEND _programs smoke_c${standard})
  else()
    message(STATUS "smoke: no C${standard} here, smoke_c${standard} not built")
  endif()
endforeach()

include(CheckLanguage)
check_language(CXX)
if(CMAKE_CXX_COMPILER)
  enable_language(CXX)
  if(MSVC)
    # Exception handling is chosen per program below, so the default /EHsc goes.
    string(REPLACE "/EHsc" "" CMAKE_CXX_FLAGS "${CMAKE_CXX_FLAGS}")
  endif()
  foreach(standard 17 20 23)
    if(NOT DEFINED CMAKE_CXX${standard}_STANDARD_COMPILE_OPTION)
      if(HYPERCAST_SMOKE_CXX)
        message(FATAL_ERROR "HYPERCAST_SMOKE_CXX is on, but the C++ compiler here has no C++${standard}")
      endif()
      message(STATUS "smoke: no C++${standard} here, smoke_cxx${standard} not built")
      continue()
    endif()
    foreach(variant "" _noexceptions)
      foreach(kind cxx detailed)
        set(program smoke_${kind}${standard}${variant})
        if(kind STREQUAL "cxx")
          add_executable(${program} main.cpp)
        else()
          add_executable(${program} detailed.cpp)
        endif()
        set_target_properties(${program} PROPERTIES CXX_STANDARD ${standard} CXX_STANDARD_REQUIRED ON
          CXX_EXTENSIONS OFF)
        if(variant STREQUAL "")
          target_compile_options(${program} PRIVATE $<$<CXX_COMPILER_ID:MSVC>:/EHsc>)
        elseif(MSVC)
          target_compile_options(${program} PRIVATE /EHs-c-)
          target_compile_definitions(${program} PRIVATE _HAS_EXCEPTIONS=0)
        else()
          target_compile_options(${program} PRIVATE -fno-exceptions)
        endif()
        list(APPEND _programs ${program})
      endforeach()
    endforeach()
  endforeach()
  add_executable(smoke_corpus corpus.cpp)
  set_target_properties(smoke_corpus PROPERTIES CXX_STANDARD 17 CXX_STANDARD_REQUIRED ON CXX_EXTENSIONS OFF)
  target_compile_options(smoke_corpus PRIVATE $<$<CXX_COMPILER_ID:MSVC>:/EHsc>)
  list(APPEND _programs smoke_corpus)
  add_executable(smoke_downgrade downgrade.cpp)
  set_target_properties(smoke_downgrade PROPERTIES CXX_STANDARD 17 CXX_STANDARD_REQUIRED ON CXX_EXTENSIONS OFF)
  target_compile_options(smoke_downgrade PRIVATE $<$<CXX_COMPILER_ID:MSVC>:/EHsc>)
  list(APPEND _programs smoke_downgrade)
elseif(HYPERCAST_SMOKE_CXX)
  message(FATAL_ERROR "HYPERCAST_SMOKE_CXX is on, but there is no C++ compiler here")
endif()

foreach(program IN LISTS _programs)
  target_compile_options(${program} PRIVATE ${_warnings})
  if(HYPERCAST_SMOKE_STATIC)
    target_link_libraries(${program} PRIVATE hypercast::hypercast_static)
  else()
    target_link_libraries(${program} PRIVATE hypercast::hypercast)
    # Windows has no rpath: the DLL goes beside the program.
    if(WIN32)
      add_custom_command(TARGET ${program} POST_BUILD
        COMMAND ${CMAKE_COMMAND} -E copy_if_different $<TARGET_RUNTIME_DLLS:${program}> $<TARGET_FILE_DIR:${program}>
        COMMAND_EXPAND_LISTS)
    endif()
  endif()
  file(GENERATE OUTPUT "${CMAKE_BINARY_DIR}/${program}-path-$<CONFIG>.txt" CONTENT "$<TARGET_FILE:${program}>")
endforeach()

list(JOIN _programs "\n" _program_lines)
file(WRITE "${CMAKE_BINARY_DIR}/programs.txt" "${_program_lines}\n")

# What find_package resolved, for run.sh to hold against what the library reports.
file(WRITE "${CMAKE_BINARY_DIR}/package-version.txt" "${hypercast_VERSION}")
