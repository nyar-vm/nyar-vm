/** Legion 子进程等价结果（矩阵 / harness）。 */
export type LegionSpawnResult = {
    status: number;
    stdout: string;
    stderr: string;
    route: string;
};
