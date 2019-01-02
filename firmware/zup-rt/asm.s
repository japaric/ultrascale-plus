  .section .ResetTrampoline, "ax"
  .type ResetTrampoline, %function
  .global ResetTrampoline
ResetTrampoline:
  /* initialize registers */
  mov r0,#0
  mov r1,#0
  mov r2,#0
  mov r3,#0
  mov r4,#0
  mov r5,#0
  mov r6,#0
  mov r7,#0
  mov r8,#0
  mov r9,#0
  mov r10,#0
  mov r11,#0
  mov r12,#0
  ldr sp,=__stack_top__        /* initialize the stack pointer */
  mov lr,#0
  mrc p15, 0, r0, c1, c0, 0    /* read SCTLR */
  bic r0, r0, #1 << 13         /* clear V bit to map the vector table to address 0 */
  mcr p15, 0, r0, cr1, cr0, 0  /* write SCTLR */
  mrc p15, 0, r0, c1, c0, 1    /* read ACTLR */
  /*  FIXME properly initialize ECC instead of disabling it */
  bic r0, r0, #1 << 25         /* disable ATCM ECC */
  mcr p15, 0, r0, cr1, cr0, 1  /* write ACTLR */
  b main

  .section .vectors, "ax"
  .type Vectors, %function
  .global Vectors
Vectors:
  ldr pc,=ResetTrampoline           /* 0x00 */
  ldr pc,=UndefinedTrampoline       /* 0x04 */
  ldr pc,=SVCTrampoline             /* 0x08 */
  ldr pc,=PrefetchAbortTrampoline   /* 0x0C */
  ldr pc,=DataAbortTrampoline       /* 0x10 */
  nop                               /* 0x14 */
  ldr pc,=IRQTrampoline             /* 0x18 */
  ldr pc,=FIQTrampoline             /* 0x1C */

  .section .text.UndefinedTrampoline, "ax"
  .type UndefinedTrampoline, %function
  .global UndefinedTrampoline
UndefinedTrampoline:
  cps #19 /* switch back to the supervisor mode to reuse the previous stack */
  b Undefined

  .section .text.SVCTrampoline, "ax"
  .type SVCTrampoline, %function
  .global SVCTrampoline
SVCTrampoline:
  cps #19 /* switch back to the supervisor mode to reuse the previous stack */
  b SVC

  .section .text.PrefetchAbortTrampoline, "ax"
  .type PreftechAbortTrampoline, %function
  .global PrefetchAbortTrampoline
PrefetchAbortTrampoline:
  cps #19 /* switch back to the supervisor mode to reuse the previous stack */
  b PrefetchAbort

  .section .text.DataAbortTrampoline, "ax"
  .type DataAbortTrampoline, %function
  .global DataAbortTrampoline
DataAbortTrampoline:
  cps #19 /* switch back to the supervisor mode to reuse the previous stack */
  b DataAbort

/* Reentrant IRQ handler */
/* Reference: Section 6.12 Reentrant interrupt handlers of "ARM Compiler
   Toolchain Developing Software for ARM Processors" */
  .section .text.IRQTrampoline, "ax"
  .type IRQTrampoline, %function
  .global IRQTrampoline
IRQTrampoline:
  sub lr, lr, #4        /* construct the return address */
  srsdb sp!, #19        /* save LR_irq and SPSR_irq to Supervisor mode stack */
  cps #19               /* switch to Supervisor mode */
  push {r0-r3, ip}      /* push other AAPCS registers */
  and r1, sp, #4        /* test alignment of the stack */
  sub sp, sp, r1        /* remove any misalignment (0 or 4) */
  push {r1, lr}         /* push the adjustment and lr_USR */
  movw r0, #4108
  movt r0, #63744
  ldr r0, [r0]          /* read ICCIAR */
  bl IRQ                /* call IRQ(<ICCIAR>) */
  pop {r1, lr}          /* pop stack adjustment and lr_USR */
  add sp, sp, r1        /* add the stack adjustment (0 or 4) */
  pop {r0-r3, ip}       /* pop registers */
  rfeia sp!             /* return using RFE from System mode stack */

  .section .text.FIQTrampoline, "ax"
  .type FIQTrampoline, %function
  .global FIQTrampoline
FIQTrampoline:
  cps #19 /* switch back to the supervisor mode to reuse the previous stack */
  b FIQ
