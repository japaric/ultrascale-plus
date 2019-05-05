MEMORY
{
  ATCM   : ORIGIN = 0x00000000, LENGTH = 64K

  BTCM0   : ORIGIN = 0x00020000, LENGTH = 32K
  BTCM1   : ORIGIN = 0x00028000, LENGTH = 32K

  OCM0 : ORIGIN = 0xFFFC0000, LENGTH = 64K
  OCM1 : ORIGIN = 0xFFFD0000, LENGTH = 64K
  OCM2 : ORIGIN = 0xFFFE0000, LENGTH = 64K
  OCM3 : ORIGIN = 0xFFFF0000, LENGTH = 64K

  DDR  : ORIGIN = 0x00100000, LENGTH = 2047M
}

/* Initial stack pointer (the stack grows towards smaller addresses) */
__stack_top__ = ORIGIN(BTCM1) + LENGTH(BTCM1);

/* Entry point = reset handler */
ENTRY(start);
EXTERN(Vectors);

/* Exceptions */
PROVIDE(Undefined = DefaultHandler);
PROVIDE(SVC = DefaultHandler);
PROVIDE(PrefetchAbort = DefaultHandler);
PROVIDE(DataAbort = DefaultHandler);
PROVIDE(FIQ = DefaultHandler);

/* Interrupts */
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
