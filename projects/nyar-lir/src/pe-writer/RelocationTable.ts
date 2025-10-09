export interface Relocation {
    offset: number;
    symbol: string;
    type: string;
}

export type RelocationTable = Relocation[];