import {PeAssembler, PeTargetArchitecture} from '.././pe-writer';

export class IlAssembler {
    private pe: PeAssembler;

    constructor(architecture: PeTargetArchitecture) {
        this.pe = new PeAssembler(architecture);
    }
}


export class IlReader {
    public read(input: Uint8Array | ArrayBuffer): PeAssembler {
        throw new Error('Not implemented');
    }
}

export class IlParser {
    public read(input: Uint8Array | ArrayBuffer): PeAssembler {
        throw new Error('Not implemented');
    }
}