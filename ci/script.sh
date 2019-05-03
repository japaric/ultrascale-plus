set -euxo pipefail

main() {
    if [ ${PAC:-0} == 1 ]; then
        ( cd firmware/zup && ./generate.sh )
    fi

    case $TARGET in
        arm*v7r-none-eabi*)
            pushd firmware/zup-quickstart

            # single-core examples
            local features=""
            local examples=

            if [ ${PAC:-0} == 1 ]; then
                features="--features pac"
                examples=(
                    ipi
                    ipi-apu
                    leds-off
                    leds-on
                    rtfm-interrupt
                    rtfm-lock
                    rtfm-message
                    rtfm-time
                )
            else
                examples=(
                    abort
                    hello
                    panic
                    trace
                )
            fi

            for ex in ${examples[@]}; do
                cargo build --example $ex $features
                cargo build --example $ex $features --release
            done

            popd

            # multi-core examples
            if [ ${PAC:-0} == 1 ]; then
                 cargo install microamp-tools --debug --git https://github.com/japaric/microamp -f

                examples=(
                    amp
                    cross
                    late-1
                    late-2
                    late-3
                    lock
                    message
                    rv
                    time
                )

                pushd firmware/zup-rtfm
                for ex in ${examples[@]}; do
                    cargo microamp --example $ex --check -v
                    cargo microamp --example $ex -v
                    cargo microamp --example $ex --release
                done
                popd
            fi

            ;;
        aarch64*)
            cd host
            pushd mrd
            cargo build --target $TARGET
            popd

            if [ ${PAC:-0} == 1 ]; then
                pushd zup-linux
                cargo build --target $TARGET --examples
                popd
            fi
            ;;
        *)
            cd firmware/zup-rt

            ./check-blobs.sh
            ;;
    esac
}

# fake Travis variables to be able to run this on a local machine
if [ -z ${TARGET-} ]; then
    TARGET=$(rustc -Vv | grep host | cut -d ' ' -f2)
fi


main
