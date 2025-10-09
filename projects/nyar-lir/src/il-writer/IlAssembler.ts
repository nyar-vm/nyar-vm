import {PeAssembler, PeTargetArchitecture} from '.././pe-writer';

export class IlAssembler {
    private pe: PeAssembler;

    constructor(architecture: PeTargetArchitecture) {
        this.pe = new PeAssembler(architecture);
    }
}
