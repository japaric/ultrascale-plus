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
                cargo build --examples --features pac
                cargo build --examples --features pac --release
            else
                cargo build --examples --release
            fi

            popd

            # multi-core examples
            if [ ${PAC:-0} == 1 ]; then
                 cargo install microamp-tools --debug --git https://github.com/japaric/microamp -f

                examples=(
                    amp-channel
                    amp-hello
                    amp-shared
                    cross
                    global
                    ipi
                    late-1
                    late-2
                    late-3
                    local
                    lock
                    message
                    pool
                    rv
                    time
                )

                pushd firmware/zup-rtfm
                # quickly check all the examples
                for ex in ${examples[@]}; do
                    cargo microamp --example $ex --check -v
                done

                # now link-test them
                for ex in ${examples[@]}; do
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
