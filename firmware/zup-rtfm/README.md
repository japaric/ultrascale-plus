# `zup-rtfm`

> Like [`cortex-m-rtfm`] but tailored to the Zynq Ultrascale+

[`cortex-m-rtfm`]: https://crates.io/crates/cortex-m-rtfm

This crate assumes that the target device has a [GIC][gic] (GICv1), secure
extension and 5 priority bits and thus will not work correctly, or at all, for
other ARM devices.

[gic]: https://developer.arm.com/products/system-ip/system-controllers/interrupt-controllers
