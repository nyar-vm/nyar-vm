# nyar-gc

Garbage collector implementation for the Nyar virtual machine.

## Features

- **Block-based Allocation**: Efficient memory management using fixed-size blocks.
- **TLAB Support**: Thread-Local Allocation Buffers for high-performance allocation.
- **Stack Scanning**: Precise stack scanning for root discovery.
- **Async Ready**: Optional integration with `tokio`.

## License

Licensed under MIT OR Apache-2.0.
