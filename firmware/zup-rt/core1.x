INCLUDE common.x;

SECTIONS
{
  .text :
  {
    KEEP(*(.vectors));
    *(.start);
    *(.main);
    *(.text .text.*);
    . = ALIGN(4);
  } > ATCM

  .local : ALIGN(4)
  {
    *(.local.*);
    . = ALIGN(4);
  } > BTCM0

  .bss : ALIGN(4)
  {
    *(.bss .bss.*);
    . = ALIGN(4);
  } > OCM1

  .data : ALIGN(4)
  {
    *(.data .data.*);
    . = ALIGN(4);
  } > OCM1

  .rodata : ALIGN(4)
  {
    *(.rodata .rodata.*);
    . = ALIGN(4);
  } > OCM1

  .resource_table : ALIGN(4)
  {
    KEEP(*(.resource_table));
  } > OCM1

  /* NOTE(NOLOAD) core 0 will initialize this shared section  */
  .shared (NOLOAD) : ALIGN(4)
  {
    KEEP(microamp-data.o(.shared));
    . = ALIGN(4);
  } > OCM2

  /* Discarded sections */
  /DISCARD/ :
  {
    /* Unused exception related info that only wastes space */
    *(.ARM.exidx.*);
  }
}
