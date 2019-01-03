set -euxo pipefail

main() {
    case $TARGET in
        arm*v7r-none-eabi*)
            pushd firmware/zup-quickstart

            # single-core examples
            local examples=(
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

            for ex in ${examples[@]}; do
                cargo build --example $ex
                cargo build --example $ex --release
            done

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
