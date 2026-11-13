#!/usr/bin/env bash
# Builds the smoke consumers against an unpacked C release archive, every way a consumer would
# on this platform, runs each program, and holds the version each reports to the archive's:
#
#   CMake       find_package(hypercast), hypercast::hypercast and hypercast::hypercast_static:
#               main.c as each C standard the compiler has, main.cpp and detailed.cpp through
#               hypercast.hpp as C++17, C++20 and C++23 with exceptions on and off, corpus.cpp,
#               which replays the conformance corpus, and downgrade.cpp (CMakeLists.txt has the
#               list; SMOKE_CXX=1 makes the C++ programs required)
#   pkg-config  `pkg-config --cflags --libs hypercast`, and the archive in ${libdir} named
#               directly, from C and C++ (not on Windows, where CMake is the way in)
#
# The downgrade test: the header against a core older than itself. Given SMOKE_CORE_10, a
# directory holding the shared library of a real 1.0 core (`cargo cdylib` in a checkout of the
# v1.0.0 tag), run.sh builds downgrade.cpp against it too, with the archive's header as a release
# has it: its HYPERCAST_VERSION macros set to 1.1.0, newer than that core (a development tree's
# are the crate's own, 1.0.0, which that core meets). The core has no cast_detailed, which a
# program built from the header names, so downgrade_stubs.c stands in for it with a function that
# ends the run with status 99: the header may only reach it behind its version guard, and a call
# that gets through is the failure this test exists to catch. Not on Windows.
#
# A program linked to the shared library must record the library's own name (@rpath/ on
# macOS, the soname elsewhere), never the path it was linked from, and one linked statically
# must not need the shared library at all. The WebAssembly archive has no shared library and
# no runner to execute it, so it is not smoke-tested here.
#
# usage: c/smoke/run.sh <unpacked archive> [<expected version>]
#        SMOKE_ARCH=x86_64 c/smoke/run.sh ...   on Apple silicon, osx-x64 under Rosetta
#        SMOKE_CORPUS=<dir>                     the corpus to replay; the checkout's by default
#        SMOKE_CORE_10=<dir>                    a 1.0 core's shared library, for the downgrade test
set -euo pipefail
pkg="$(cd "$1" && pwd)"
expected="${2:-}"
here="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
corpus="$(cd "${SMOKE_CORPUS:-$here/../../corpus}" && pwd)"
work="$(mktemp -d)"
trap 'rm -rf "$work"' EXIT

case "$(uname -s)" in
  MINGW* | MSYS* | CYGWIN*) os=windows ;;
  Darwin) os=macos ;;
  *) os=linux ;;
esac

fail() { echo "smoke: $*" >&2; exit 1; }

# macOS only: build for SMOKE_ARCH and run under it, so osx-x64 is tested on an arm64 runner.
arch_flags=() cmake_arch=() run_as=()
if [ -n "${SMOKE_ARCH:-}" ]; then
  [ "$os" = macos ] || fail "SMOKE_ARCH is for macOS only"
  arch_flags=(-arch "$SMOKE_ARCH") cmake_arch=(-DCMAKE_OSX_ARCHITECTURES="$SMOKE_ARCH") run_as=(arch "-$SMOKE_ARCH")
fi

# One version for the archive: the CMake package's, pkg-config's and the library's must agree,
# and with the caller's when one is given.
check_version() {
  local how="$1" reported="$2"
  echo "  $how: $reported"
  if [ -z "$expected" ]; then
    expected="$reported"
  elif [ "$reported" != "$expected" ]; then
    fail "$how reports $reported, expected $expected"
  fi
}

# What a program asks the dynamic loader for, one name per line. Its readers below use
# `grep >/dev/null`, never `grep -q`: -q exits at the first match, and under pipefail the
# writer's SIGPIPE then fails the pipeline (musl's readelf, on a program that matched).
needed() {
  case "$os" in
    macos) otool -L "$1" | tail -n +2 | awk '{print $1}' ;;
    linux) readelf -d "$1" | sed -n 's/.*(NEEDED).*\[\(.*\)\]/\1/p' ;;
  esac
}

check_shared_link() {
  [ "$os" = windows ] && return 0
  local want=libhypercast.so
  [ "$os" = macos ] && want=@rpath/libhypercast.dylib
  needed "$1" | grep -x "$want" >/dev/null || fail "$1 does not load $want: $(needed "$1" | tr '\n' ' ')"
}

check_static_link() {
  [ "$os" = windows ] && return 0
  if needed "$1" | grep hypercast >/dev/null; then
    fail "$1, linked statically, still loads $(needed "$1" | grep hypercast)"
  fi
}

# Runs a smoke program, after checking what it links, and holds its version to the archive's.
# The corpus replay takes the corpus directory.
run_program() {
  local how="$1" program="$2" linkage="$3" output args=()
  if [ "$linkage" = static ]; then check_static_link "$program"; else check_shared_link "$program"; fi
  case "$program" in
    *smoke_corpus*) args=("$corpus") ;;
    *smoke_downgrade*) args=(accept) ;;
  esac
  output="$(${run_as[@]+"${run_as[@]}"} "$program" ${args[@]+"${args[@]}"})" || fail "$program ($how) failed"
  check_version "hypercast_version() ($how)" "$(printf '%s' "$output" | tr -d '\r')"
  # A plain door on a core of this version reads `1,23,456` as malformed.
  case "$program" in
    *smoke_downgrade*)
      output="$(${run_as[@]+"${run_as[@]}"} "$program" door | tr -d '\r')" || fail "$program door ($how) failed"
      [ "$output" = malformed ] || fail "$program door ($how) printed '$output', expected malformed"
      ;;
  esac
}

cmake_build() {
  local name="$1" static="$2" build="$work/cmake-$1"
  cmake -S "$here" -B "$build" -DCMAKE_PREFIX_PATH="$pkg" -DHYPERCAST_SMOKE_STATIC="$static" ${cmake_arch[@]+"${cmake_arch[@]}"} \
    -DHYPERCAST_SMOKE_CXX="$([ "${SMOKE_CXX:-}" = 1 ] && echo ON || echo OFF)" \
    -DCMAKE_BUILD_TYPE=Release >"$work/$name.log" 2>&1 || { cat "$work/$name.log"; fail "cmake configure ($name)"; }
  cmake --build "$build" --config Release >>"$work/$name.log" 2>&1 || { cat "$work/$name.log"; fail "cmake build ($name)"; }
  sed -n 's/^-- smoke: /  (/p' "$work/$name.log" | sed 's/$/)/'
  check_version "CMake package ($name)" "$(cat "$build/package-version.txt")"
  local program path linkage
  linkage="$([ "$static" = ON ] && echo static || echo shared)"
  # CMake writes its text files with CRLF on Windows: every read strips the CR.
  while read -r program; do
    program=${program%$'\r'}
    [ -n "$program" ] || continue
    path="$(tr -d '\r' <"$build/$program-path-Release.txt")"
    [ -f "$path" ] || fail "no $program at $path"
    run_program "$program, $name" "$path" "$linkage"
  done <"$build/programs.txt"
  tr -d '\r' <"$build/programs.txt" | grep -x smoke_c99 >/dev/null || fail "smoke_c99 was not built ($name)"
}

echo "smoke: $pkg"
# Every function the archive's header declares is called by main.c and wrapped by
# hypercast.hpp, so a door added to the core without either fails here.
functions="$(sed -n -E 's/^[a-z0-9_]+ ([a-z0-9_]+)\(.*\);$/\1/p' "$pkg/include/hypercast.h")"
[ -n "$functions" ] || fail "no functions found in $pkg/include/hypercast.h"
for function in $functions; do
  grep -qE "(^|[^a-z0-9_])$function\(" "$here/main.c" || fail "main.c never calls $function"
  grep -qE "::$function([^a-z0-9_]|\$)" "$pkg/include/hypercast.hpp" || fail "hypercast.hpp never calls $function"
done
echo "  every one of the $(echo "$functions" | wc -l | tr -d ' ') functions hypercast.h declares is in main.c and hypercast.hpp"
# The header's compile-time version is the archive's: a release moves it with the crate's, and
# the macros are what a consumer's own #if reads (and what the header's run-time guard compares
# the library's version to).
macro() { sed -n -E "s/^#define HYPERCAST_VERSION_$1 ([0-9]+)\$/\1/p" "$pkg/include/hypercast.hpp"; }
check_version "hypercast.hpp's HYPERCAST_VERSION macros" "$(macro MAJOR).$(macro MINOR).$(macro PATCH)"
cmake_build shared OFF
cmake_build static ON

if [ "$os" != windows ]; then
  export PKG_CONFIG_PATH="$pkg/lib/pkgconfig"
  check_version "pkg-config --modversion" "$(pkg-config --modversion hypercast)"
  libdir="$(pkg-config --variable=libdir hypercast)"
  # main.c refuses a library older than the headers it is built against: the archive's version.
  IFS=. read -r major minor patch <<<"$expected"
  min_version="-DHYPERCAST_SMOKE_MIN_VERSION=$(((major << 16) | (minor << 8) | patch))"
  # shellcheck disable=SC2046 # pkg-config's flags are meant to split
  cc ${arch_flags[@]+"${arch_flags[@]}"} "$min_version" -o "$work/pc-shared" "$here/main.c" $(pkg-config --cflags --libs hypercast) -Wl,-rpath,"$libdir"
  run_program "pkg-config, shared" "$work/pc-shared" shared
  # shellcheck disable=SC2046
  cc ${arch_flags[@]+"${arch_flags[@]}"} "$min_version" -o "$work/pc-static" "$here/main.c" $(pkg-config --cflags hypercast) "$libdir/libhypercast.a"
  run_program "pkg-config, static" "$work/pc-static" static
  if command -v c++ >/dev/null; then
    # pkg-config's route is -I, so a warning in hypercast.hpp fails here as it would a consumer's.
    cxx_warnings=(-Wall -Wextra -Wpedantic -Wshadow -Werror)
    # shellcheck disable=SC2046
    c++ ${arch_flags[@]+"${arch_flags[@]}"} -std=c++17 "${cxx_warnings[@]}" -o "$work/pc-cxx-shared" "$here/main.cpp" $(pkg-config --cflags --libs hypercast) \
      -Wl,-rpath,"$libdir"
    run_program "pkg-config, C++, shared" "$work/pc-cxx-shared" shared
    # shellcheck disable=SC2046
    c++ ${arch_flags[@]+"${arch_flags[@]}"} -std=c++17 "${cxx_warnings[@]}" -o "$work/pc-cxx-static" "$here/main.cpp" $(pkg-config --cflags hypercast) \
      "$libdir/libhypercast.a"
    run_program "pkg-config, C++, static" "$work/pc-cxx-static" static
  elif [ "${SMOKE_CXX:-}" = 1 ]; then
    fail "SMOKE_CXX is set, but there is no c++ here"
  fi
fi

# The header against a core that cannot drive it: a numeric request is refused, never read by that
# core's rule. A core drives a header when its major version is the header's and its minor at least the
# header's, whatever the patch; each leg below rewrites the header's version macros to say so.
#
# downgrade_leg <name> <header version> <library dir> <stubs: yes|no> <verdict: drives|refuses>
# builds downgrade.cpp from the archive's header as that version, against the shared library in
# the directory (the stand-in for cast_detailed linked too, for a core that has none), and runs it:
# a core that drives the header passes `accept`, a door reads `1,23,456` as malformed and an Indian
# one as 123456; one that cannot passes `refuse`, and both doors write the header's line naming both
# versions to stderr and abort (SIGABRT, 128 + 6 under a shell), printing nothing and never reaching
# the stand-in for the missing export (status 99). On either, with_grouping ends the program on the
# retired 256 and on 768. The exports as new as the header (the text door, and Constraints, which every
# checked door needs) take the same guard: on a core that drives the header the text door reads
# and a declaration is made; on one that cannot, each aborts naming both versions before the
# missing export.
downgrade_leg() {
  local name="$1" header="$2" lib="$3" stubs="$4" verdict="$5" dir="$work/$1" major minor patch
  IFS=. read -r major minor patch <<<"$header"
  mkdir -p "$dir/include"
  cp "$pkg/include/hypercast.h" "$dir/include/"
  sed -E -e "s/^#define HYPERCAST_VERSION_MAJOR .*/#define HYPERCAST_VERSION_MAJOR $major/" \
    -e "s/^#define HYPERCAST_VERSION_MINOR .*/#define HYPERCAST_VERSION_MINOR $minor/" \
    -e "s/^#define HYPERCAST_VERSION_PATCH .*/#define HYPERCAST_VERSION_PATCH $patch/" \
    "$pkg/include/hypercast.hpp" >"$dir/include/hypercast.hpp"
  grep -qx "#define HYPERCAST_VERSION_MINOR $minor" "$dir/include/hypercast.hpp" || fail "$name: could not set the header's version macros"
  local objects=()
  if [ "$stubs" = yes ]; then
    cc ${arch_flags[@]+"${arch_flags[@]}"} -I"$dir/include" -c -o "$dir/stubs.o" "$here/downgrade_stubs.c"
    objects=("$dir/stubs.o")
  fi
  c++ ${arch_flags[@]+"${arch_flags[@]}"} -std=c++17 -I"$dir/include" -o "$dir/downgrade" "$here/downgrade.cpp" \
    ${objects[@]+"${objects[@]}"} -L"$lib" -lhypercast -Wl,-rpath,"$lib"
  local mode=refuse status=0 output
  [ "$verdict" = drives ] && mode=accept
  LEG_CORE="$(${run_as[@]+"${run_as[@]}"} "$dir/downgrade" "$mode" | tr -d '\r')" || fail "$name: downgrade $mode failed"
  output="$(${run_as[@]+"${run_as[@]}"} "$dir/downgrade" door 2>"$dir/door.err" | tr -d '\r')" || status=$?
  [ "$status" -ne 99 ] || fail "$name: the header called cast_detailed on a core without it: $(cat "$dir/door.err")"
  local indian indian_status=0 policy retired retired_status
  indian="$(${run_as[@]+"${run_as[@]}"} "$dir/downgrade" indian 2>"$dir/indian.err" | tr -d '\r')" || indian_status=$?
  [ "$indian_status" -ne 99 ] || fail "$name: the header called cast_detailed on a core without it: $(cat "$dir/indian.err")"
  for policy in 256 768; do
    retired_status=0
    retired="$(${run_as[@]+"${run_as[@]}"} "$dir/downgrade" retired "$policy" 2>/dev/null | tr -d '\r')" || retired_status=$?
    [ "$retired_status" -eq 134 ] && [ -z "$retired" ] || fail "$name: with_grouping($policy) printed '$retired' (status $retired_status), expected the abort (134)"
  done
  local gated gated_status gated_out
  for gated in text constraints; do
    gated_status=0
    gated_out="$(${run_as[@]+"${run_as[@]}"} "$dir/downgrade" "$gated" 2>"$dir/$gated.err" | tr -d '\r')" || gated_status=$?
    [ "$gated_status" -ne 99 ] || fail "$name: $gated called an export on a core without it: $(cat "$dir/$gated.err")"
    if [ "$verdict" = drives ]; then
      local want=read
      [ "$gated" = text ] || want=declared
      [ "$gated_status" -eq 0 ] && [ "$gated_out" = "$want" ] || fail "$name: $gated printed '$gated_out' (status $gated_status), expected $want"
    else
      [ "$gated_status" -eq 134 ] && [ -z "$gated_out" ] || fail "$name: $gated printed '$gated_out' (status $gated_status), expected the header's abort (134)"
      grep -q "the linked library is $LEG_CORE, which cannot drive this header ($header:" "$dir/$gated.err" ||
        fail "$name: $gated's abort does not name both versions: $(cat "$dir/$gated.err")"
    fi
  done
  if [ "$verdict" = drives ]; then
    [ "$status" -eq 0 ] && [ "$output" = malformed ] || fail "$name: a door printed '$output' (status $status), expected malformed"
    [ "$indian_status" -eq 0 ] && [ "$indian" = "read 123456" ] || fail "$name: an Indian door printed '$indian' (status $indian_status), expected read 123456"
  else
    [ "$status" -eq 134 ] || fail "$name: a door exited $status, expected the header's abort (134)"
    [ -z "$output" ] || fail "$name: a door printed '$output': parsed, not refused"
    [ "$indian_status" -eq 134 ] && [ -z "$indian" ] || fail "$name: an Indian door printed '$indian' (status $indian_status), expected the header's abort (134)"
    grep -q "the linked library is $LEG_CORE, which cannot drive this header ($header:" "$dir/door.err" ||
      fail "$name: the abort does not name both versions: $(cat "$dir/door.err")"
  fi
}

if [ "$os" != windows ] && command -v c++ >/dev/null; then
  IFS=. read -r major minor patch <<<"$expected"
  # The patch never matters: a header one patch ahead of the core still drives it.
  downgrade_leg patch-ahead "$major.$minor.$((patch + 1))" "$pkg/lib" no drives
  # Another major version is refused, though its minor is no newer than the core's.
  downgrade_leg other-major "$((major + 1)).$minor.$patch" "$pkg/lib" no refuses
  echo "  a header one patch ahead drives this core; one a major version ahead is refused, aborting with both versions named"
fi

# A real 1.0 core, with the header as the release that follows it has it (1.1.0, newer than that
# core; a development tree's macros are the crate's own, 1.0.0, which that core meets).
if [ -n "${SMOKE_CORE_10:-}" ]; then
  [ "$os" != windows ] || fail "SMOKE_CORE_10 is not for Windows"
  command -v c++ >/dev/null || fail "SMOKE_CORE_10 is set, but there is no c++ here"
  old="$work/core10"
  mkdir -p "$old"
  # Whichever shared library the directory holds, under the name the header's archive uses: the
  # install name (macOS) or soname recorded in a build of the tag is its own, not ours.
  if [ "$os" = macos ]; then
    cp "$SMOKE_CORE_10/libhypercast.dylib" "$old/" || fail "no libhypercast.dylib in $SMOKE_CORE_10"
    install_name_tool -id @rpath/libhypercast.dylib "$old/libhypercast.dylib"
  else
    cp "$SMOKE_CORE_10/libhypercast.so" "$old/" || fail "no libhypercast.so in $SMOKE_CORE_10"
  fi
  downgrade_leg core-1.0 1.1.0 "$old" yes refuses
  case "$LEG_CORE" in
    1.0.*) ;;
    *) fail "SMOKE_CORE_10 holds a core reporting $LEG_CORE, expected 1.0.x" ;;
  esac
  echo "  a $LEG_CORE core: every format refused at declaration, a numeric or text door and a Constraints declaration abort naming both versions, the other doors still read"
fi

echo "smoke: every build of $pkg reports $expected"
