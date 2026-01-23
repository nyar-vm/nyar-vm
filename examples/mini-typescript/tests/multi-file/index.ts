import { add, multiply } from "./math";

export function main() {
    let x = 10;
    let y = 20;
    let sum = add(x, y);
    let prod = multiply(x, y);
    return sum + prod;
}
