export class PeSection {
    private name: string;
    private characteristics: number;
    private virtual_size: number;
    private virtual_address: number;
    private raw_data: Uint8Array;
    private raw_offset: number;

    constructor(name: string, characteristics: number) {
        this.name = name;
        this.characteristics = characteristics;
        this.virtual_size = 0;
        this.virtual_address = 0;
        this.raw_data = new Uint8Array(0);
        this.raw_offset = 0;
    }

    set_raw_data(data: Uint8Array): void {
        this.raw_data = data;
        this.virtual_size = data.length;
    }

    get_raw_data(): Uint8Array {
        return this.raw_data;
    }

    get_virtual_size(): number {
        return this.virtual_size;
    }

    get_raw_data_size(): number {
        return this.raw_data.length;
    }

    set_virtual_address(address: number): void {
        this.virtual_address = address;
    }

    get_virtual_address(): number {
        return this.virtual_address;
    }

    set_raw_offset(offset: number): void {
        this.raw_offset = offset;
    }

    get_raw_offset(): number {
        return this.raw_offset;
    }

    get_characteristics(): number {
        return this.characteristics;
    }
}