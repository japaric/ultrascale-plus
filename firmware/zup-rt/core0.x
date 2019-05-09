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
  } > OCM0

  .data : ALIGN(4)
  {
    *(.data .data.*);
    . = ALIGN(4);
  } > OCM0

  .rodata : ALIGN(4)
  {
    *(.rodata .rodata.*);
    . = ALIGN(4);
  } > OCM0

  .resource_table : ALIGN(4)
  {
    KEEP(*(.resource_table));
  } > OCM0

  .shared : ALIGN(4)
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
