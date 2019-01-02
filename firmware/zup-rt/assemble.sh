#!/bin/bash

set -euxo pipefail

main() {
    local crate=dcc

    # remove existing blobs because otherwise this will append object files to the old blobs
    rm -f bin/*.a

    # NOTE: cflags taken from cc 1.0.28
    arm-none-eabi-as -march=armv7-r -mlittle-endian -mfloat-abi=soft asm.s -o bin/$crate.o
    ar crs bin/armv7r-none-eabi.a bin/$crate.o

    arm-none-eabi-as -march=armv7-r -mbig-endian -mfloat-abi=soft asm.s -o bin/$crate.o
    ar crs bin/armebv7r-none-eabi.a bin/$crate.o

    arm-none-eabi-as -march=armv7-r -mlittle-endian -mfloat-abi=hard -mfpu=vfpv3-d16 asm.s -o bin/$crate.o
    ar crs bin/armv7r-none-eabihf.a bin/$crate.o

    arm-none-eabi-as -march=armv7-r -mbig-endian -mfloat-abi=hard -mfpu=vfpv3-d16 asm.s -o bin/$crate.o
    ar crs bin/armebv7r-none-eabihf.a bin/$crate.o

    rm bin/$crate.o
}

main
