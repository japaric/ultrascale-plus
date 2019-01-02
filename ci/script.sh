set -euxo pipefail

main() {
    case $TARGET in
        arm*v7r-none-eabi*)
            pushd firmware/zup-quickstart
            cargo build --examples
            cargo build --examples --release
            ;;
        *)
            pushd firmware/zup-rt

            ./check-blobs.sh
            ;;
    esac
    popd
}

main
