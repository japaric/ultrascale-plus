#!/bin/bash

set -euxo pipefail

main() {
    local html=../../third-party/ug1087-zynq-ultrascale-registers/html
    if [ ! -d $html ]; then
        echo 'run `./fetch-third-party.sh` first'
        exit 1
    fi

    local td=$(mktemp -d)

    pushd ../../tools/html2svd
    cargo run --release -- $html > $td/zup.svd
    popd

    svd2rust --target none -i $td/zup.svd > lib.rs
    rm -rf $td

    rm -rf src
    if [ ${PAC:-0} == 1 ]; then
        # CI
        mkdir src
        mv lib.rs src/lib.rs
    else
        form -i lib.rs -o src
        rm lib.rs
        cargo fmt
        cargo check
    fi
}

main
