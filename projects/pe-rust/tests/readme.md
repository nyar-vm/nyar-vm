## Tests

```bash
wee test
```


```asm
; hello_world.asm
  BITS 64                 ; change to 64-bit mode
  GLOBAL main
  SECTION .data
    hello db "Hello World!", 10 ; 10 is the ASCII code for newline
  SECTION .text
  main:
    ; write "Hello World!" to stdout
    mov eax, 1            ; system call for write
    mov edi, 1            ; file descriptor for stdout
    mov rsi, hello        ; pointer to string to write
    mov edx, 13           ; length of string to write
    syscall               ; invoke the system call
    ; exit with status code 0
    mov eax, 60      ; system call number for exit
    xor edi, edi     ; exit status code (0)
    syscall          ; invoke the system call
```