# `mrd`

> A tool for reading memory from Linux userspace

This tool can be used to inspect the OCM which starts at address `0xFFFC_0000`.

## Installation

``` console
$ # on the build machine
$ cargo build --release

$ scp target/aarch64-unknown-linux-musl/release/mrd me@ultrascale-plus:/some/where
```

## Usage

``` console
$ # on the ultrascale+
$ # syntax: mrd $address $words

$ # OCM
$ mrd 0xFFFC0000 4
0xFFFC0000: 0x1400024E
0xFFFC0004: 0x00000000
0xFFFC0008: 0x00000000
0xFFFC000C: 0x00000000

$ # Peripheral memory (IPI)
$ mrd 0xFF300000 2
0xFF300000: 0x00000000
0xFF300004: 0x00000000
```
