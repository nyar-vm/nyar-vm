import {PeWriter} from "@/pe-writer/PeAssembler";

export class DosHeader {
    // "MZ"
    public dos_signature: bigint = 0x5A4Dn;
    public bytes_on_last_page: bigint = 0x90n;
    public pages_in_file: bigint = 0x03n;
    public relocations: bigint = 0x00n;
    public size_of_header: bigint = 0x04n;
    public minimum_extra_paragraphs: bigint = 0x00n;
    public maximum_extra_paragraphs: bigint = 0xFFFFn;
    public initial_relative_ss: bigint = 0x00n;
    public initial_sp: bigint = 0xB8n;
    public checksum: bigint = 0x00n;
    public initial_ip: bigint = 0x00n;
    public initial_relative_cs: bigint = 0x00n;
    public addr_relocation_table: bigint = 0x40n;
    public overlay_number: bigint = 0x00n;
    // Offset to NT headers
    public e_lfanew: bigint = 0x80n;

    public write(writer: PeWriter) {
        throw Error("unimplement")
    }
}