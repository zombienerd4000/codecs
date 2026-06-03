# SPINE-64 Specification v0.2

64-bit assembly language compiling to x86-64 binary. Assembly layer of the ThOS stack.

## Architecture

```
Thorn -> userland/apps
ThOS -> operating system
Ground -> systems C-like language
SPINE-64 -> assembly
x86-64 -> bare metal
```

## Registers

| Name | x86-64 | Purpose |
|------|--------|---------|
| `ret` | rax | Return value / accumulator |
| `r1` | rbx | General purpose |
| `cnt` | rcx | Counter |
| `dta` | rdx | Second return value |
| `src` | rsi | Source |
| `dst` | rdi | Destination |
| `r2` | r8 | General purpose |
| `r3` | r9 | General purpose |
| `r4` | r10 | General purpose |
| `r5` | r11 | General purpose |
| `r6` | r12 | General purpose |
| `r7` | r13 | General purpose |
| `r8` | r14 | General purpose |
| `r9` | r15 | General purpose |
| `m1` | - | Reserved for processes (kernel cannot touch) |
| `m2` | - | Reserved for processes (kernel cannot touch) |

**Compiler managed:**
- `stp` - Stack top pointer (rsp)
- `sbp` - Stack base pointer (rbp)
- `sip` - Stack instruction pointer (rip)

**Flags:**
- `carry` - Set if unsigned overflow
- `overflow` - Set if signed overflow
- `zero` - Set if result is exactly 0

## Size Specifiers

Memory instructions support size specifiers with dot notation:
- `.1` - 1 byte
- `.2` - 2 bytes
- `.4` - 4 bytes
- `.8` - 8 bytes

```asm
move.4 ret [0x1000]
move.1 [0x2000] ret
```

## Modules

```asm
module code     # all code
module data     # initialized data
module reserve  # uninitialized data (bss)
```

## Data Definition

Inside `module data`:
```asm
name value          # assembler infers size
name value size    # force size in bytes
mynum 300          # picks minimum bytes
mystring "hello"   # auto null terminated
mystring "hello" 16
```

## Aliases

Map symbolic names to registers:
```asm
alias r1 counter
alias r2 index
```

## Addressing Modes

```asm
[0x1000]            # fixed address
[r1]                # address in register
[r1 + 8]            # base + offset
[r1 + r2]           # base + register
[r1 + r2 * 4]       # base + scaled index
[r1 + r2 * 4 + 16]  # base + scaled + offset
```

## Instructions

### Data Movement
- `move` / `move.1/2/4/8` - Copy value
- `push` / `pop` - Stack operations
- `lma` - Load memory address
- `swap` - Swap values
- `movesx` / `movezx` - Move with sign/zero extend

### Arithmetic
- `add`, `sub`, `mul`, `smul`, `div`, `sdiv`
- `inc`, `dec`, `neg`
- `awc` - Add with carry (128-bit math)
- `swb` - Subtract with borrow

### Bitwise
- `and`, `or`, `xor`, `not`
- `shl`, `shr`, `asr` - Shifts
- `rotl`, `rotr` - Rotate
- `racl`, `racr` - Rotate through carry

### Comparison and Jump
```asm
cmp reg op reg go label
cmp carry go label
cmp overflow go label
cmp zero go label
go label
# operators: == != >= <= > <
```

### Functions
```asm
fn global funcname
start
    ...
    return
end

raw fn funcname   # raw function (no return)
start
    ...
end

extern funcname   # external reference
```

**Calling convention:**
- Arguments pushed right to left
- Return value in `ret`
- Caller cleans up stack

### Loops
```asm
loop
start
    ...
end
```

### Floating Point
- `fmove`, `fadd`, `fsub`, `fmul`, `fdiv`, `fcmp`
- Float registers: `f1` - `f8`

### Memory Block Operations
```asm
rep movs   # copy [src] to [dst], cnt bytes
rep stos   # fill [dst] with ret, cnt bytes
```

### System
- `syscall`, `nope`, `halt`
- `ioff`, `ion` - Interrupt enable/disable
- `pr`, `pw` - Port I/O
- `int` - Software interrupt
- `reti` - Return from interrupt

### Interrupt Handlers
```asm
on interrupt 0x21
start
    ...
    reti
end
```

### Conditional Set
```asm
set r3 carry
set r3 zero
set r3 overflow
```

## Directives

```asm
origin 0x7C00   # load address (required for bootloaders)
```

## Labels

```asm
mylabel:
    ...
go mylabel
cmp r1 == r2 go mylabel
```

## Example

```asm
origin 0x7C00

module data
    message "hello world" 16

module reserve
    buffer 1024

module code
    fn global main
    start
        move counter 10
        call add_two
        halt
    end

    fn global add_two
    start
        pop r1
        pop r2
        add r1 r2
        move ret r1
        return
    end
```

## Building

```bash
python lexer.py source.s64   # Tokenize
python parser.py source.s64   # Parse to AST
python codegen.py source.s64  # Generate binary
```

## Notes

- **Stack alignment:** `stp` must be 16-byte aligned before `call` (handled automatically)
- **Divide by zero:** Triggers interrupt `0x00`
- **Kernel flag:** `--kernel` prevents use of `m1`/`m2` registers
