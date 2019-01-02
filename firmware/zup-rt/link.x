MEMORY
{
  /* Global address: R5_0 - 0xFFE0_0000 | R5_1 - 0xFFE9_0000 */
  ATCM : ORIGIN = 0x00000000, LENGTH = 64K

  /* Global address: R5_0 - 0xFFE2_0000 | R5_1 - 0xFFEB_0000 */
  BTCM : ORIGIN = 0x00020000, LENGTH = 64K

  DDR  : ORIGIN = 0x00100000, LENGTH = 2047M

  /* NOTE The OCM is split in 4 64K banks */
  OCM  : ORIGIN = 0xFFFC0000, LENGTH = 256K
}

/* Entry point = reset handler */
ENTRY(ResetTrampoline);
EXTERN(Vectors);

PROVIDE(Undefined = DefaultHandler);
PROVIDE(SVC = DefaultHandler);
PROVIDE(PrefetchAbort = DefaultHandler);
PROVIDE(DataAbort = DefaultHandler);
PROVIDE(FIQ = DefaultHandler);
PROVIDE(SG0 = DefaultHandler);
PROVIDE(SG1 = DefaultHandler);
PROVIDE(SG2 = DefaultHandler);
PROVIDE(SG3 = DefaultHandler);
PROVIDE(SG4 = DefaultHandler);
PROVIDE(SG5 = DefaultHandler);
PROVIDE(SG6 = DefaultHandler);
PROVIDE(SG7 = DefaultHandler);
PROVIDE(SG8 = DefaultHandler);
PROVIDE(SG9 = DefaultHandler);
PROVIDE(SG10 = DefaultHandler);
PROVIDE(SG11 = DefaultHandler);
PROVIDE(SG12 = DefaultHandler);
PROVIDE(SG13 = DefaultHandler);
PROVIDE(SG14 = DefaultHandler);
PROVIDE(SG15 = DefaultHandler);
PROVIDE(IPI_CH1 = DefaultHandler);
PROVIDE(IPI_CH2 = DefaultHandler);

/* Initial stack pointer */
__stack_top__ = ORIGIN(ATCM) + LENGTH(ATCM);

SECTIONS
{
  .text 0 :
  {
    KEEP(*(.vectors));
    *(.ResetTrampoline);
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
  } > ATCM

  .data : ALIGN(4)
  {
    *(.data .data.*);
    . = ALIGN(4);
  } > ATCM

  /* NOTE the remoteproc ELF loader doesn't like it when this section is marked
    as allocatable so we mark it as non-allocatable (NOLOAD) */
  /* (NOLOAD) also means that's impossible to initialize this section at runtime
    using stuff like r0::init_data. Therefore all variables in this section are
    effectively MaybeUninit */
  .shared (NOLOAD) : ALIGN(4)
  {
    *(.shared .shared.*);
    . = ALIGN(4);
  } > OCM

  .resource_table : ALIGN(4)
  {
    KEEP(*(.resource_table));
  } > ATCM

  /* Discarded sections */
  /DISCARD/ :
  {
    /* Unused exception related info that only wastes space */
    *(.ARM.exidx.*);
  }
}
