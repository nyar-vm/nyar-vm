import {DosHeader, FileHeader, OptionalHeader, SectionHeader} from "@/reader";
import {PeAssembler} from "@/pe-writer";


export class PeReader {
    private view: DataView;
    private dos_header?: DosHeader = undefined;
    private file_header?: FileHeader = undefined;
    private optional_header?: OptionalHeader = undefined;
    private section_headers: SectionHeader[] = [];

    constructor(data: DataView | Uint8Array | ArrayBuffer | File) {
        throw Error("unimplement")
    }

    public read(): PeAssembler {
        throw Error("unimplement")
    }

    private read_u8(offset: number | bigint): bigint {
        return BigInt(this.view.getUint8(offset, true));
    }

    private read_u16(offset: number | bigint): bigint {
        return BigInt(this.view.getUint16(offset, true));
    }

    private read_u32(offset: number | bigint): bigint {
        return BigInt(this.view.getUint32(offset, true));
    }

    private read_u64(offset: bigint): bigint {
        return this.view.getBigUint64(offset, true);
    }
}