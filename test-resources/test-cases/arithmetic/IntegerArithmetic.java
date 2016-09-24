/**
 * Created by cfrancia on 24/09/16.
 */
public class IntegerArithmetic {

    public static void main(String[] args) {
        add(100, 5);
        subtract(100, 5);
        multiply(100, 5);
        divide(100, 5);
    }

    public static void add(int a, int b) {
        println(a + b);
    }

    public static void subtract(int a, int b) {
        println(a - b);
    }

    public static void multiply(int a, int b) {
        println(a * b);
    }

    public static void divide(int a, int b) {
        println(a / b);
    }

    public static native void println(int val);

}
