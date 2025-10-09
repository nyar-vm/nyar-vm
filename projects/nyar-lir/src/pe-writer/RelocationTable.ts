import {PeSection} from "@/pe-writer/PeSection";

export interface Relocation {
    offset: number;
    symbol: string;
    type: string;
}

export class RelocationTable {
    section: PeSection;
    table: Relocation[];
    symbols: Map<string, number>;

    constructor(section: PeSection, table: Relocation[], symbols: Map<string, number>) {
        this.section = section;
        this.table = table;
        this.symbols = symbols;
    }
}