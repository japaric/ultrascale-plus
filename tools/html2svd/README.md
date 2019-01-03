# `html2svd`

Xilinx doesn't provide an SVD file for the Zynq Ultrascale+, but they provide
[HTML documentation] that contains all the information a SVD file would have.

[HTML documentation]: https://www.xilinx.com/support/answers/67576.html

This tool transforms that HTML documentation into a SVD file which then can be
fed to [`svd2rust`].

[`svd2rust`]: https://crates.io/crates/svd2rust
