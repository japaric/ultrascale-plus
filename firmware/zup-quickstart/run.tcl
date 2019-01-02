conn

targets -set -nocase -filter {name =~"*R5*0"} -index 1

configparams force-mem-access 1

dow -clear [lindex $argv 0]

configparams force-mem-access 0

con
