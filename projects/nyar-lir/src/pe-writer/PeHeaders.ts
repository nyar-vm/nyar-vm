import {BinaryWriter} from '../BinaryWriter';

import {PeTargetArchitecture} from "@/pe-writer/PeTargetArchitecture";

export class PeHeaders {
    generate_dos_header(): Uint8Array {
        const writer = new BinaryWriter();

        // DOS MZ�?        writer.write_u16(0x5a4d); // e_magic: 'MZ'
        writer.write_u16(0x0090); // e_cblp
        writer.write_u16(0x0003); // e_cp
        writer.write_u16(0x0000); // e_crlc
        writer.write_u16(0x0004); // e_cparhdr
        writer.write_u16(0x0000); // e_minalloc
        writer.write_u16(0xffff); // e_maxalloc
        writer.write_u16(0x0000); // e_ss
        writer.write_u16(0x00b8); // e_sp
        writer.write_u16(0x0000); // e_csum
        writer.write_u16(0x0000); // e_ip
        writer.write_u16(0x0000); // e_cs
        writer.write_u32(0x00000040); // e_lfarlc
        writer.write_u32(0x00000000); // e_ovno

        // 保留字段
        for (let i = 0; i < 8; i++) {
            writer.write_u16(0);
        }

        writer.write_u32(0x00000074); // e_lfanew (PE头偏移)

        return writer.get_bytes();
    }

    public get_pe_magic(architecture: PeTargetArchitecture): number {
        return architecture === PeTargetArchitecture.X64 ? 0x020b : 0x010b;
    }
}