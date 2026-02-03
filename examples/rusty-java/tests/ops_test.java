public class OpsTest {
    public static void main(String[] args) {
        // Arithmetic
        int a = 10;
        int b = 3;
        System.out.println(a + b); // 13
        System.out.println(a - b); // 7
        System.out.println(a * b); // 30
        System.out.println(a / b); // 3
        System.out.println(a % b); // 1

        // Comparison
        System.out.println(a == 10); // true
        System.out.println(a != b);  // true
        System.out.println(a > b);   // true
        System.out.println(a < b);   // false

        // Bitwise
        System.out.println(a & b);   // 2 (1010 & 0011 = 0010)
        System.out.println(a | b);   // 11 (1010 | 0011 = 1011)
        System.out.println(a ^ b);   // 9 (1010 ^ 0011 = 1001)
        System.out.println(~a);      // -11

        // Logical
        boolean t = true;
        boolean f = false;
        System.out.println(t && f); // false
        System.out.println(t || f); // true
        System.out.println(!t);     // false

        // Compound Assignment
        int c = 5;
        c += 10;
        System.out.println(c); // 15
        c *= 2;
        System.out.println(c); // 30

        // Increment/Decrement
        int d = 10;
        System.out.println(++d); // 11
        System.out.println(d++); // 11
        System.out.println(d);   // 12
    }
}
