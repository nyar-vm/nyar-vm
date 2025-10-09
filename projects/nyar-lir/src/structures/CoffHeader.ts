import {PeWriter} from "@/pe-writer/PeAssembler";

export class CoffHeader {
    public machine: bigint = 0x8664n;
    /**
     * .text and .idata sections only
     */
    public number_of_sections: bigint = 2n;
    public time_date_stamp: bigint = BigInt(Math.floor(Date.now() / 1000));
    public pointer_to_symbol_table: bigint = 0n;
    public number_of_symbols: bigint = 0n;
    /**
     * 240 bytes for PE32+
     */
    public size_of_optional_header: bigint = 0xF0n;
    /**
     * EXECUTABLE_IMAGE | LARGE_ADDRESS_AWARE
     */
    public characteristics: bigint = 0x0022n;

    public write(writer: PeWriter) {
        throw Error("unimplement")
    }
}