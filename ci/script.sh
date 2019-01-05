set -euxo pipefail

main() {
    if [ ${PAC:-0} == 1 ]; then
        ( cd firmware/zup && ./generate.sh )
    fi

    case $TARGET in
        arm*v7r-none-eabi*)
            ( cd tools/cargo-amp && cargo install --debug --path . -f )

            cd firmware/zup-quickstart

            # single-core examples
            local features=""
            local examples=

            if [ ${PAC:-0} == 1 ]; then
                features="--features pac"
                examples=(
                    ipi
                    leds-off
                    leds-on
                )
            else
                examples=(
                    abort
                    hello
                    icdicer
                    icdipr
                    lock
                    nested
                    panic
                    rtfm-lock
                    rtfm-message
                    sgi
                    trace
                )
            fi

            for ex in ${examples[@]}; do
                cargo build --example $ex $features
                cargo build --example $ex $features --release
            done

            # multi-core examples
            if [ ${PAC:-0} == 1 ]; then
                examples=(
                    ipi-rpu
                )
            else
                examples=(
                    amp
                    rtfm-mc-cross
                    rtfm-mc-lock
                    rtfm-mc-message
                )
            fi

            for ex in ${examples[@]}; do
                cargo amp --example $ex $features
                cargo amp --example $ex $features --release
            done
            ;;
        aarch64*)
            cd host
            pushd mrd
            cargo build --target $TARGET
            popd

            pushd zup-linux
            cargo build --target $TARGET --examples
            popd
            ;;
        *)
            cd firmware/zup-rt

            ./check-blobs.sh
            ;;
    esac
}

main
