/**
 * Created by cfrancia on 24/09/16.
 */
public class StaticVariable {

    private static int STATIC_INT = 2;

    public static void main(String[] args) {
        println(STATIC_INT);
        STATIC_INT = 3;
        println(STATIC_INT);
    }

    public static native void println(int val);

}
