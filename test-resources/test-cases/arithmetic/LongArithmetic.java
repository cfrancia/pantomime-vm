/**
 * Created by cfrancia on 24/09/16.
 */
public class LongArithmetic {

    public static void main(String[] args) {
        add(100L, 5L);
        subtract(100L, 5L);
        multiply(100L, 5L);
        divide(100L, 5L);
    }

    public static void add(long a, long b) {
        println(a + b);
    }

    public static void subtract(long a, long b) {
        println(a - b);
    }

    public static void multiply(long a, long b) {
        println(a * b);
    }

    public static void divide(long a, long b) {
        println(a / b);
    }

    public static native void println(long val);

}
