set -euxo pipefail

main() {
    case $TARGET in
        arm*v7r-none-eabi*)
            rustup target add $TARGET

            if [ ${PAC:-0} == 1 ]; then
                curl -LSfs https://japaric.github.io/trust/install.sh | \
                    sh -s -- \
                       --force \
                       --git rust-embedded/svd2rust \
                       --tag v0.14.0 \
                       --target x86_64-unknown-linux-musl

                ./fetch-third-party.sh
            fi

            ;;
        *)
            mkdir gcc
            curl -L https://developer.arm.com/-/media/Files/downloads/gnu-rm/7-2018q2/gcc-arm-none-eabi-7-2018-q2-update-linux.tar.bz2?revision=bc2c96c0-14b5-4bb4-9f18-bceb4050fee7?product=GNU%20Arm%20Embedded%20Toolchain,64-bit,,Linux,7-2018-q2-update | tar --strip-components=1 -C gcc -xj
            ;;
    esac
}

main
