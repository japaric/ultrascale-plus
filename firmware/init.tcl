# NOTE commands extracted from the SDK logs after running a debug session

conn

# provides `enable_split_mode` command
source zynqmp_utils.tcl

# not strictly required if this runs after boot
targets -set -nocase -filter {name =~"APU*"} -index 1
rst -system
after 3000

# put the RPU in split mode (default is lock step mode)
targets -set -nocase -filter {name =~"RPU*"} -index 1
enable_split_mode

# initialize the PSU
targets -set -nocase -filter {name =~"APU*"} -index 1
configparams force-mem-access 1
source psu_init.tcl
psu_init
after 1000

targets -set -nocase -filter {name =~"*R5*0"} -index 1
rst -processor

configparams force-mem-access 0
