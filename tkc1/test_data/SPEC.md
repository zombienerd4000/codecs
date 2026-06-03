SPINE-64 SPECIFICATION v0.2
===========================

OVERVIEW
--------
64-bit assembly language compiling to x86-64 binary
assembly layer of the ThOS stack
readable syntax, not x86

STACK
-----
Thorn -> userland/apps
ThOS -> operating system
Ground -> systems C-like language
SPINE-64 -> assembly
x86-64 -> bare metal

COMMENTS
--------
# this is a comment

REGISTERS
---------
ret    rax    return value / accumulator
r1     rbx    general purpose
cnt    rcx    counter
dta    rdx    second return value
src    rsi    source
dst    rdi    destination
r2     r8     general purpose
r3     r9     general purpose
r4     r10    general purpose
r5     r11    general purpose
r6     r12    general purpose
r7     r13    general purpose
r8     r14    general purpose
r9     r15    general purpose
m1     -      reserved for processes, kernel cannot touch (assembler enforced with --kernel flag)
m2     -      reserved for processes, kernel cannot touch (assembler enforced with --kernel flag)

special (compiler managed, not written manually):
stp    stack top pointer (rsp) - points to top of stack, managed automatically
sbp    stack base pointer (rbp) - tracks bottom of current stack frame
sip    stack instruction pointer (rip) - tracks current executing instruction

flags register (set automatically by arithmetic):
carry    set if unsigned overflow
overflow set if signed overflow
zero     set if result is exactly 0

SIZE SPECIFIERS
---------------
appended to memory instructions with dot notation
.1    1 byte
.2    2 bytes
.4    4 bytes
.8    8 bytes

example:
move.4 ret [0x1000]
move.1 [0x2000] ret

MODULES
-------
module code
    all code goes here

module data
    all initialized data goes here

module reserve
    all uninitialized data goes here

module keyword only parsed at start of line, never inside strings

DATA DEFINITION (inside module data)
-------------------------------------
name value          assembler infers minimum size
name value size     force specific size in bytes

examples:
mynum 300           assembler picks 2 bytes
mynum 300 4         forced 4 bytes
mystring "hello"    auto null terminated
mystring "hello" 16 16 byte buffer

assembler throws error if value does not fit in specified size

ADDRESSING MODES
----------------
[0x1000]            fixed address
[r1]                address in register
[r1 + 8]            base + fixed offset
[r1 + r2]           base + register offset
[r1 + r2 * 4]       base + scaled index
[r1 + r2 * 4 + 16]  base + scaled index + offset

works for both reads and writes:
move.4 ret [r1 + 8]
move.4 [r1 + 8] ret

INSTRUCTIONS
------------

DATA MOVEMENT
move        copy value
move.1/2/4/8    copy with size specifier
push        push to stack
pop         pop from stack
lma         load memory address into register
swap        swap two values
movesx      move + sign extend
movezx      move + zero extend

ARITHMETIC
add         add
sub         subtract
mul         unsigned multiply
smul        signed multiply
div         unsigned divide
sdiv        signed divide
inc         increment by 1
dec         decrement by 1
neg         negate
awc         add with carry (for 128-bit math)
swb         subtract with borrow

BITWISE
and         bitwise AND
or          bitwise OR
xor         bitwise XOR
not         bitwise NOT
shl         shift left
shr         shift right
asr         arithmetic shift right
rotl        rotate left
rotr        rotate right
racl        rotate left through carry
racr        rotate right through carry

COMPARISON AND JUMP
cmp reg op reg go label
cmp carry go label
cmp overflow go label
cmp zero go label
go label

operators: == != >= <= > <

examples:
cmp ret >= r1 go done
cmp r1 == r2 go equal
cmp carry go overflow_handler
cmp overflow go signed_overflow_handler
cmp zero go is_zero
go somewhere

FUNCTIONS
---------
fn funcname
start
    ...
    return
end

fn global funcname
start
    ...
    return
end

raw fn funcname
start
    ...
end

extern funcname

calling convention:
- arguments pushed onto stack right to left before call
- return value in ret
- caller responsible for cleaning up stack after call

CALL INSTRUCTION
----------------
call funcname    call a function, pushes return address onto stack
return           return from function, pops return address from stack

LOOPS
-----
loop
start
    ...
end

SYSTEM
------
syscall     call kernel
nope        do nothing (nop)
halt        stop CPU
ioff        disable interrupts
ion         enable interrupts
pr          read from I/O port
pw          write to I/O port
int         software interrupt
reti        return from interrupt handler

INTERRUPT HANDLERS
------------------
on interrupt 0x21
start
    ...
    reti
end

assembler automatically saves and restores all registers around handler body
multiple handlers allowed, one per interrupt number
interrupt number specified in hex

MEMORY BLOCK OPERATIONS
-----------------------
rep movs    copy cnt bytes from [src] to [dst], increments src and dst automatically
rep stos    fill cnt bytes at [dst] with value in ret, increments dst automatically

example copy:
move cnt 100
lma src mydata
lma dst buffer
rep movs

example zero fill:
move cnt 100
lma dst buffer
move ret 0
rep stos

FLOATING POINT
--------------
basic float registers: f1 f2 f3 f4 f5 f6 f7 f8

fadd    float add
fsub    float subtract
fmul    float multiply
fdiv    float divide
fmove   move float value
fcmp    compare floats

example:
fmove f1 3.14
fmove f2 2.0
fmul f1 f2

CONDITIONAL SET
---------------
set reg carry      reg = 1 if carry set, 0 if not
set reg zero       reg = 1 if zero flag set, 0 if not
set reg overflow   reg = 1 if overflow set, 0 if not

example:
add r1 r2
set r3 carry    # r3 = 1 if overflowed, 0 if fine

ORIGIN DIRECTIVE
----------------
origin 0x7C00    tells assembler this code loads at address 0x7C00
                 required for bootloader so addresses calculate correctly
                 goes at top of file before any code

BINARY OUTPUT FORMAT
--------------------
bootloader -> flat binary (raw bytes, no header)
kernel and userland -> ELF format (allows linking multiple files)

STACK ALIGNMENT
---------------
stp must be 16-byte aligned before any call instruction
assembler handles this automatically in normal fn
raw fn must handle alignment manually if needed

DIVIDE BY ZERO
--------------
CPU automatically triggers interrupt 0x00 on divide by zero
handle with:
on interrupt 0x00
start
    # handle divide by zero error
    reti
end

UNDEFINED BEHAVIOR
------------------
read from address 0x0 -> returns 0
write to address 0x0 -> ignored, no crash
divide by zero -> triggers interrupt 0x00
stack overflow on bare metal -> corrupts memory, no protection
stack overflow on ThOS -> guard page triggers, process killed cleanly

LABELS
------
mylabel:
    ...

cmp r1 == r2 go mylabel
go mylabel

MEMORY READ/WRITE
-----------------
move ret [0x1000]       read from address
move [0x2000] ret       write to address
move.4 ret [r1 + 8]    read 4 bytes from r1+8
move.1 [dst] src        write 1 byte

PORT IO
-------
pr ret 0x60     read from port 0x60 into ret
pw 0x60 ret     write ret to port 0x60

KERNEL COMPILATION FLAG
-----------------------
--kernel flag prevents use of m1 and m2 registers
assembler throws error if kernel code tries to use m1 or m2
all ThOS kernel code compiled with --kernel flag

CARRY, OVERFLOW AND ZERO FLAGS
-------------------------------
set automatically by arithmetic instructions
carry = 1 if unsigned overflow occurred
overflow = 1 if signed overflow occurred
zero = 1 if result is exactly 0
check with cmp carry go label, cmp overflow go label, cmp zero go label
awc instruction adds carry flag to result for 128-bit chained arithmetic

EXAMPLES
--------

count down from 10:
move cnt 10
loop
start
    dec cnt
    cmp cnt == 0 go done
end
done:
    halt

function with global visibility:
fn global add_two
start
    pop r1
    pop r2
    add r1 r2
    move ret r1
    return
end

call from another file:
extern add_two
push 20
push 10
call add_two

struct field access:
move.4 ret [r1 + 8]
move.4 [r1 + 16] r2

128-bit addition:
add r1 r3
awc r2 r4

overflow check:
add r1 r2
cmp carry go too_large
move [result] r1
halt
too_large:
    move r1 0xFFFFFFFFFFFFFFFF
    halt

keyboard interrupt handler:
on interrupt 0x21
start
    call handle_keyboard
    reti
end

complete module structure:
module data
    message "hello world" 0
    count 0 4

module reserve
    buffer 1024

module code
    fn global main
    start
        lma src message
        move cnt 11
        loop
        start
            move.1 ret [src]
            inc src
            dec cnt
            cmp cnt == 0 go done
        end
        done:
        halt
    end


extra md i found, dont know if this has any changes but if it does this is the correct one:

""

bootloader entry point:
origin 0x7C00

module code
    raw fn boot_entry
    start
        move stp 0x90000
        go kernel_main
    end

    fn global kernel_main
    start
        ioff
        # kernel starts here
        halt
    end
