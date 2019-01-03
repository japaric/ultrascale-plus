# `ultrascale-plus`

Rust on the [Zynq
UltraScale+](https://www.xilinx.com/products/silicon-devices/soc/zynq-ultrascale-mpsoc.html)
MPSoC

> **IMPORTANT**: All the code in *this* repository is **experimental**.
> Eventually parts of this repository will be moved into their own repositories
> and be published on crates.io. Do **not** rely on any crate in this
> repository.

## Organization

Under the `firmware` directory you'll find crates that are meant to be compiled
for the ARM Cortex-R5 architecture. `firmware/zup-quickstart/examples` contains
examples that you can run on any of the R5 cores. `firmware/README.md` contains
more information about how to build, load, run and debug these programs.

Under the `host` directory you'll find crates that are meant to be compiled for
the 64-bit ARMv8-A architecture.

Under the `tools` directory you'll find crates that are meant to be compiled for
the architecture of the build machine (usually x86_64).

## References

- [Zynq Ultrascale+ Device Technical Reference Manual (UG1085)][trm]

[trm]: https://www.xilinx.com/support/documentation/user_guides/ug1085-zynq-ultrascale-trm.pdf

- [Zynq UltraScale+ Devices Register Reference (UG1087)][rr]

[rr]: https://www.xilinx.com/html_docs/registers/ug1087/ug1087-zynq-ultrascale-registers.html

- [Xilinx Software Command-Line Tool (XSCT) Reference Guide (UG1208)][xsct]

[xsct]: https://www.xilinx.com/support/documentation/sw_manuals/xilinx2016_2/ug1208-xsct-reference-guide.pdf

## License

The code in this repository is distributed under the terms of both the MIT
license and the Apache License (Version 2.0).

See [LICENSE-APACHE](LICENSE-APACHE) and [LICENSE-MIT](LICENSE-MIT) for details.
