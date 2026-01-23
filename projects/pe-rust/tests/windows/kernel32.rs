use pe_assembler::{
    exports::nyar::pe_assembly::{
        reader::{Guest, ReadConfig},
        types::PeError,
    },
    PeContext,
};

#[test]
fn analyzer_kernel32() -> Result<(), PeError> {
    let file_path = "C:\\Windows\\System32\\kernel32.dll";
    println!("Analyzing file: {}", file_path);

    let pe_data = std::fs::read(file_path)?;

    let result = PeContext::read(
        pe_data,
        ReadConfig {
            validate_structure: false,
            parse_sections: false,
            parse_imports: false,
            parse_exports: false,
            parse_debug_info: false,
            max_sections: 0,
            max_imports: 0,
        },
    )?;
    println!("{:#?}", result.header);
    Ok(())
}
