INCLUDE common.x;

SECTIONS
{
  .text ORIGIN(TCM0) :
  {
    KEEP(*(.vectors));
    *(.start);
    *(.main);
    *(.text .text.*);
    . = ALIGN(4);
  } > TCM0

  .rodata : ALIGN(4)
  {
    *(.rodata .rodata.*);
    . = ALIGN(4);
  } > TCM0

  .bss : ALIGN(4)
  {
    *(.bss .bss.*);
    . = ALIGN(4);
  } > TCM0

  .data : ALIGN(4)
  {
    *(.data .data.*);
    . = ALIGN(4);
  } > TCM0

  .resource_table : ALIGN(4)
  {
    KEEP(*(.resource_table));
  } > TCM0

  /* Discarded sections */
  /DISCARD/ :
  {
    /* Unused exception related info that only wastes space */
    *(.ARM.exidx.*);
  }
}
