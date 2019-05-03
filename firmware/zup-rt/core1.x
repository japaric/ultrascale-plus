INCLUDE common.x;

/* Initial stack pointer */
__stack_top__ = ORIGIN(BTCM1_1) + LENGTH(BTCM1_1);

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
  } > BTCM0_1

  .bss : ALIGN(4)
  {
    *(.bss .bss.*);
    . = ALIGN(4);
  } > BTCM0_1

  .data : ALIGN(4)
  {
    *(.data .data.*);
    . = ALIGN(4);
  } > BTCM0_1

  .resource_table : ALIGN(4)
  {
    KEEP(*(.resource_table));
  } > BTCM0_1

  /* NOTE(NOLOAD) core 0 will initialize this shared section  */
  .shared (NOLOAD) : ALIGN(4)
  {
    KEEP(microamp-data.o(.shared));
    . = ALIGN(4);
  } > OCM0

  .ocm : ALIGN(4)
  {
    *(.ocm.*);
    . = ALIGN(4);
  } > OCM2

  /* Discarded sections */
  /DISCARD/ :
  {
    /* Unused exception related info that only wastes space */
    *(.ARM.exidx.*);
  }
}
