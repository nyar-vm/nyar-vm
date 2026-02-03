int main() {
    int i = 0;
    int sum = 0;
    while (i < 10) {
        if (i % 2 == 0) {
            sum = sum + i;
        }
        i = i + 1;
    }
    return sum;
}
