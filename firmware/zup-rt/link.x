INCLUDE common.x;

SECTIONS
{
  .text ORIGIN(ATCM) :
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
  } > ATCM

  .bss : ALIGN(4)
  {
    *(.bss .bss.*);
    . = ALIGN(4);
  } > BTCM0

  .data : ALIGN(4)
  {
    *(.data .data.*);
    . = ALIGN(4);
  } > BTCM0

  .resource_table : ALIGN(4)
  {
    KEEP(*(.resource_table));
  } > BTCM0

  /* Discarded sections */
  /DISCARD/ :
  {
    /* Unused exception related info that only wastes space */
    *(.ARM.exidx.*);
  }
}
