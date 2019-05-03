INCLUDE common.x;

/* Initial stack pointer */
__stack_top__ = ORIGIN(BTCM1_0) + LENGTH(BTCM1_0);

INPUT(microamp-data.o);

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

  .rodata : ALIGN(4)
  {
    *(.rodata .rodata.*);
    . = ALIGN(4);
  } > BTCM0_0

  .bss : ALIGN(4)
  {
    *(.bss .bss.*);
    . = ALIGN(4);
  } > BTCM0_0

  .data : ALIGN(4)
  {
    *(.data .data.*);
    . = ALIGN(4);
  } > BTCM0_0

  .resource_table : ALIGN(4)
  {
    KEEP(*(.resource_table));
  } > BTCM0_0

  .shared : ALIGN(4)
  {
    KEEP(microamp-data.o(.shared));
    . = ALIGN(4);
  } > OCM0

  .ocm : ALIGN(4)
  {
    *(.ocm.*);
    . = ALIGN(4);
  } > OCM1

  /* Discarded sections */
  /DISCARD/ :
  {
    /* Unused exception related info that only wastes space */
    *(.ARM.exidx.*);
  }
}
