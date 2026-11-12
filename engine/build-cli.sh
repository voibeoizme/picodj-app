#!/bin/sh
# Builds the command-line test tool (Linux/macOS/MSYS):  ./build-cli.sh && ./build/picodj-cli selftest
set -e
cd "$(dirname "$0")"
mkdir -p build
INC="-Isrc -Ithird_party/qm-dsp -Ithird_party/qm-dsp/include -Ithird_party/dr_libs"
DEF="-Dkiss_fft_scalar=double -D_USE_MATH_DEFINES"
for f in third_party/qm-dsp/ext/kissfft/kiss_fft.c third_party/qm-dsp/ext/kissfft/tools/kiss_fftr.c; do
  ${CC:-cc} -O2 -w $INC -Ithird_party/qm-dsp/ext/kissfft $DEF -c "$f" -o "build/$(basename "$f").o"
done
for f in $(cat sources.txt) tools/cli.cpp; do
  ${CXX:-c++} -std=c++17 -O2 -w $INC $DEF -c "$f" -o "build/$(basename "$f").o" &
done
wait
${CXX:-c++} build/*.o -o build/picodj-cli -lpthread
echo "built build/picodj-cli"
