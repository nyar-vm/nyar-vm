# PE Analyzer - DLL Analysis Tool

A comprehensive PE (Portable Executable) analyzer written in Rust that can analyze Windows DLL files and executables. This tool provides detailed information about PE file structure, sections, imports, exports, and debug information.

## Features

- **PE File Validation**: Validates PE file structure including DOS header, NT header, and COFF header
- **Architecture Detection**: Identifies target architecture (x86, x64)
- **Subsystem Analysis**: Determines Windows subsystem type (GUI, Console, etc.)
- **Section Analysis**: Parses and displays all PE sections with detailed information
- **Import Table Analysis**: Lists all imported DLLs and functions
- **Export Table Analysis**: Displays exported functions from DLLs
- **Debug Information**: Extracts debug directory information
- **Configurable Parsing**: Control which parts of the PE file to parse

## Usage

Add this library to your `Cargo.toml`:

```toml
[dependencies]
pe-rust = { path = "../pe-rust" }
```

### Basic Example

```rust
use pe_assembler::exports::nyar::pe_assembly::reader::{Guest, ReadConfig};
use pe_assembler::PeContext;

fn analyze_pe_file(pe_data: Vec<u8>) -> Result<(), Box<dyn std::error::Error>> {
    // Configure the PE reader
    let config = ReadConfig {
        validate_structure: true,
        parse_sections: true,
        parse_imports: true,
        parse_exports: true,
        parse_debug_info: true,
        max_sections: 96,
        max_imports: 1000,
    };

    // Validate the PE file
    let is_valid = PeContext::validate(pe_data.clone())?;
    println!("PE file is valid: {}", is_valid);

    // Get basic PE information
    let info = PeContext::get_info(pe_data.clone())?;
    println!("Architecture: {:?}", info.arch);
    println!("Subsystem: {:?}", info.subsystem);
    println!("Entry Point: 0x{:08X}", info.entry_point);
    println!("Image Base: 0x{:08X}", info.image_base);

    // Parse the complete PE structure
    let program = PeContext::read(pe_data, config)?;
    println!("Number of sections: {}", program.sections.len());
    println!("Number of import tables: {}", program.import_tables.len());
    
    Ok(())
}
```

### Reading PE Headers

```rust
use pe_rust::reader;

let pe_data = std::fs::read("example.exe")?;
let pe_info = reader::read(&pe_data)?;

// Access DOS header
println!("DOS Header: {:?}", pe_info.dos_header);

// Access NT headers
println!("NT Headers: {:?}", pe_info.nt_headers);

// Access section headers
for section in &pe_info.section_headers {
    println!("Section: {:?}", section.name);
}
```

### Writing a Simple PE File

```rust
use pe_rust::writer;

// Create a minimal PE structure
let mut pe_info = PeInfo::new();
pe_info.dos_header = DosHeader::default();
pe_info.nt_headers = NtHeaders::default();
// Add sections, imports, etc.

// Write to file
let pe_data = writer::write(&pe_info)?;
std::fs::write("simple.exe", pe_data)?;
```

## Features

- **Safe Rust API**: Memory-safe operations with proper error handling
- **PE Format Compliance**: Full support for PE/COFF file format
- **Extensible**: Modular design for easy extension
- **Cross-platform**: Works on all Rust-supported platforms
- **Zero-copy Parsing**: Efficient memory usage when reading PE files
- **Error Handling**: Comprehensive error types and messages

## API Reference

### Reader Module

The `reader` module provides functions for parsing PE files:

- `read(data: &[u8]) -> Result<PeInfo, PeError>`: Parse PE file data
- `read_file(path: &str) -> Result<PeInfo, PeError>`: Read and parse a PE file

### Writer Module

The `writer` module provides functions for generating PE files:

- `write(pe_info: &PeInfo) -> Result<Vec<u8>, PeError>`: Generate PE file data
- `write_file(path: &str, pe_info: &PeInfo) -> Result<(), PeError>`: Write PE file to disk

### Data Structures

Key data structures include:

- `PeInfo`: Complete PE file representation
- `DosHeader`: DOS header structure
- `NtHeaders`: NT headers structure
- `CoffHeader`: COFF header structure
- `OptionalHeader`: Optional header structure
- `SectionHeader`: Section header structure
- `PeError`: Error type for PE operations

## Development

### Building

```bash
cargo build
```

### Testing

```bash
cargo test
```

### Documentation

```bash
cargo doc --open
```

### Code Formatting

```bash
cargo fmt
```

### Linting

```bash
cargo clippy
```

## Integration

This library is designed to be used by:

- **pe-wasm32**: WebAssembly bindings for browser usage
- **il-rust**: Intermediate Language assembly library
- **CLI Tools**: Command-line PE assembly utilities
- **Other Applications**: Any Rust project needing PE file manipulation

## Examples

See the `examples` directory for more detailed usage examples:

- `read_pe.rs`: Demonstrates reading and displaying PE file information
- `write_pe.rs`: Shows how to create a simple PE file
- `modify_pe.rs`: Example of modifying an existing PE file

## License

See the root [License.md](../../License.md) for details.

## Contributing

Contributions are welcome! Please feel free to submit a pull request.

## Roadmap

- [ ] Add support for more PE file features (resources, debug info, etc.)
- [ ] Improve error handling and reporting
- [ ] Add more comprehensive test coverage
- [ ] Develop performance benchmarks
- [ ] Create more detailed examples and tutorials