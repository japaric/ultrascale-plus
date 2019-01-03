set -euxo pipefail

main() {
    case $TARGET in
        arm*v7r-none-eabi*)
            ( cd tools/cargo-amp && cargo install --debug --path . -f )

            pushd firmware/zup-quickstart

            # single-core examples
            local features=""
            local examples=

            if [ ${PAC:-0} == 1 ]; then
                features="--features pac"
                examples=(
                    leds-off
                    leds-on
                )

                ( cd ../zup && ./generate.sh )
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
                )
            fi

            for ex in ${examples[@]}; do
                cargo build --example $ex $features
                cargo build --example $ex $features --release
            done

            if [ ${PAC:-0} == 1 ]; then
                return
            fi

            # multi-core examples
            examples=(
                amp
            )

            for ex in ${examples[@]}; do
                cargo amp --example $ex
                cargo amp --example $ex --release
            done

            ;;
        *)
            pushd firmware/zup-rt

            ./check-blobs.sh
            ;;
    esac
    popd
}

main
