INCLUDE common.x;

INPUT(amp-data.o);

SECTIONS
{
  .text :
  {
    __svectors = .;
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
  } > OCM0

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

  .shared : ALIGN(4)
  {
    KEEP(amp-data.o(.shared));
    . = ALIGN(4);
  } > OCM2

  .local.data : ALIGN(4)
  {
    *(.local.data.*);
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

ASSERT(__svectors == 0, "vector table is missing");
