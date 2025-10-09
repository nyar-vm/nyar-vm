import { PeSection } from "./PeSection";

export interface Relocation {
    offset: number;
    symbol: string;
    type: 'rip_relative' | 'rip_relative_import';
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