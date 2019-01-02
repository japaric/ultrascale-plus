conn

targets -set -nocase -filter {name =~ "*R5*$::env(CORE)"}

# NOTE this will NOT reset peripherals like the RPU_GIC
rst -processor

configparams force-mem-access 1

dow -clear [lindex $argv 0]

configparams force-mem-access 0

set f [open dcc$::env(CORE).log w]
readjtaguart -start -handle $f
