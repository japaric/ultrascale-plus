#!/bin/bash

set -euxo pipefail

main() {
    local file=ug1087-zynq-ultrascale-registers.zip
    local dir=${file%.zip}

    mkdir -p third-party
    cd third-party
    rm -rf $dir
    mkdir $dir
    cd $dir
    curl -LO https://www.xilinx.com/Attachment/$file
    unzip $file >/dev/null
    rm -f $file
}

main
