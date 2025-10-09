import {DosHeader} from "@/structures/DosHeader";
import {CoffHeader} from "@/structures/CoffHeader";
import {OptionalHeader} from "@/structures/OptionalHeader";
import {PeWriter} from "@/pe-writer/PeAssembler";


/**
 * PE file headers structure
 */
export class PeHeaders {
    // DOS Header
    public dos_header: DosHeader = new DosHeader();
    // NT Headers
    public nt_signature: number = 0x00004550; // "PE\0\0"
    // File Header (COFF Header)
    public coff_header = new CoffHeader();
    // Optional Header (PE32+)
    public optional_header = new OptionalHeader();
    // Data Directories
    public export_table_rva: number = 0;
    public export_table_size: number = 0;
    public import_table_rva: number = 0x2000;
    public import_table_size: number = 0x100;
    public resource_table_rva: number = 0;
    public resource_table_size: number = 0;
    public exception_table_rva: number = 0;
    public exception_table_size: number = 0;
    public certificate_table_rva: number = 0;
    public certificate_table_size: number = 0;
    public base_relocation_table_rva: number = 0;
    public base_relocation_table_size: number = 0;
    public debug_rva: number = 0;
    public debug_size: number = 0;
    public architecture_rva: number = 0;
    public architecture_size: number = 0;
    public global_ptr_rva: number = 0;
    public global_ptr_size: number = 0;
    public tls_table_rva: number = 0;
    public tls_table_size: number = 0;
    public load_config_table_rva: number = 0;
    public load_config_table_size: number = 0;
    public bound_import_rva: number = 0;
    public bound_import_size: number = 0;
    public iat_rva: number = 0x2000;
    public iat_size: number = 0x100;
    public delay_import_descriptor_rva: number = 0;
    public delay_import_descriptor_size: number = 0;
    public com_runtime_descriptor_rva: number = 0;
    public com_runtime_descriptor_size: number = 0;

    public write(writer: PeWriter) {
        throw Error("unimplement")
    }
}