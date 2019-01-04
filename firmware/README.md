# `firmware`

Code for the R5 cores on the Zynq SoC.

## Standalone

This section describes how to develop programs that directly run on the R5 cores.

### Requirements

To load and debug the programs on the R5s you'll need the following:

- The [XSCT][xsct] tool. [Installation instructions][xsct-install]

[xsct]: https://www.xilinx.com/html_docs/xilinx2018_2/SDK_Doc/xsct/intro/xsct_introduction.html
[xsct-install]: https://www.xilinx.com/html_docs/xilinx2018_2/SDK_Doc/xsct/intro/xsct_install_launch.html

> **NOTE**: On Arch Linux the `xsct` script might segfault. The [Arch Linux
> wiki][xsct-fix] contains information on how to solve the problem.

[xsct-fix]: https://wiki.archlinux.org/index.php/Xilinx_Vivado#xsct_segfault

- Boot mode must be set to JTAG. All other boot modes are pretty much untested.

- `psu_init.tcl` and `zynqmp_utils.tcl`. More details below.

### How-to

#### SoC initialization

Every time you power the board you'll need to initialize the PSU (Processing
System Unit) running this command from the `firmware` directory.

``` console
$ xsct init.tcl
```

There must be a `psu_init.tcl` file next to `init.tcl`. This file is device, and
maybe also board, specific. You must procure it yourself; the file can be
created using the Vivado SDK. In the case of the Ultra96 board this file is
provided by [Avnet]. For other boards, you may be able to find this file in the
[XSDK].

[Avnet]: http://ultra96.org/sites/default/files/design/Ultra96%20Tutorials%2001%20to%2004%20Solution.zip
[XSDK]: https://www.xilinx.com/products/design-tools/embedded-software/sdk.html

There must also be a `zynqmp_utils.tcl` file next to `init.tcl`. This file can
be obtained from the XSDK. The default installation path of the file is
`/opt/Xilinx/SDK/201*.*/scripts/sdk/util/`.

#### Building a program

There are plenty of examples in the `zup-quickstart` directory. To build the
examples run this command from within that directory.

``` console
$ cargo build --example $name
```

You'll find the output binary in `firmware/target/armv7r-none-eabi`.

#### Loading and debugging a program

A `debug.tcl` script is provided in the `zup-quickstart` directory. This script
can be used to load and debug a program:

> **NOTE:** programs are loaded into RAM so after a power cycle the program will
> be discarded.

``` console
$ CORE=0 xsdb -interactive debug.tcl ../target/armv7r-none-eabi/debug/examples/hello
(..)
xsdb% # start program execution
xsdb% con
```

You must specify on which R5 core (the MPSoC has 2) you want to load the program
using the `CORE` environment variable. The only values that `CORE` accepts are
`0` and `1`.

This command will also create a file named `dcc$CORE.log` that will contain the
DCC messages logged by the R5 core.

``` console
$ tail -f dcc0.log
Hello, world!
```

You can debug the firmware using the `xsdb` tool or the GDB tool.

``` console
$ # on another terminal; keep `xsdb` running
$ arm-none-eabi-gdb -x xsdb.gdb ../target/armv7r-none-eabi/debug/examples/hello

(gdb) disassemble $pc
Dump of assembler code for function main:
   0x000000b8 <+0>:     movw    r0, #19456      ; 0x4c00
   0x000000bc <+4>:     movt    r0, #0
   0x000000c0 <+8>:     mov     r1, #14
   0x000000c4 <+12>:    bl      0x338 <dcc::write_str>
   0x000000c8 <+16>:    b       0xcc <main+20>
   0x000000cc <+20>:    b       0xd0 <main+24>
=> 0x000000d0 <+24>:    b       0xd0 <main+24>
End of assembler dump.
```

Alternatively, you can execute the `cargo run` command to build and load the
program.

``` console
$ CORE=0 cargo run --example hello
(..)
     Running `xsdb -interactive debug.tcl /tmp/firmware/target/armv7r-none-eabi/debug/examples/hello`
```

#### Loading and running a program

If you want to directly run the program you can use the `run.tcl` script.

``` console
$ CORE=0 xsdb run.tcl ../target/armv7r-none-eabi/debug/examples/hello
```

Note that this script will not create the `dcc$CORE.log` file. You can tweak
`.cargo/config` to use `run.tcl` instead of `debug.tcl`.

``` console
$ head -n5 .cargo/config
[target.armv7r-none-eabi]
# load and debug
# runner = "xsdb -interactive debug.tcl"
# load and run
runner = "xsdb run.tcl"
```

#### Multi-core debugging

You can debug each core independently using the `xsdb` tool. You will *not* be
able to use GDB to debug both cores, though. GDB will connect to the first core.

``` console
$ CORE=0 cargo run --example hello
```

``` console
$ # on another terminal
$ CORE=1 cargo run --example hello
```

``` console
$ tail dcc*.log
==> dcc0.log <==
Hello, world!

==> dcc1.log <==
Hello, world!
```

## Hosted

This section describes how to develop R5 programs from the Linux environment
that runs on the APU (A53 cores).

### Loading and running a program

You can load programs on the R5 cores using the [remoteproc] interface.

[remoteproc]: https://www.kernel.org/doc/Documentation/remoteproc.txt

``` console
$ # on the build machine
$ scp ../target/armv7r-none-eabi/debug/examples/leds-off me@ultrascale-plus:/some/where
```

``` console
$ # on the ultrascale+

$ # copy the elf file to /lib/firmware
$ cp leds-off /lib/firmware/

$ # pick one of the cores; in this example we pick core 0
$ cd /sys/class/remoteproc/remoteproc0

$ # tell the ELF loader which file to load
$ echo leds-off > firmware

$ # load and run the program
$ echo start > state
[ 1615.909542] remoteproc remoteproc0: powering up ff9a0100.zynqmp_r5_rproc
[ 1615.917771] remoteproc remoteproc0: Booting fw image leds-off, size 646968
[ 1615.924659] zynqmp_r5_remoteproc ff9a0100.zynqmp_r5_rproc: RPU boot from TCM.
[ 1615.932249] remoteproc remoteproc0: remote processor ff9a0100.zynqmp_r5_rproc is now up

$ # halt the processor; this is required to load a different program
$ echo stop > state
[ 1644.997783] remoteproc remoteproc0: stopped remote processor ff9a0100.zynqmp_r5_rproc
```

### Trace buffers

**TODO**
