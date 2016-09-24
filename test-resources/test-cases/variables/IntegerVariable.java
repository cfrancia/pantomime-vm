/**
 * Created by cfrancia on 24/09/16.
 */
public class IntegerVariable {

    public static void main(String[] args) {
        int i = 2_048_000;
        println(i);
    }

    public static native void println(int val);

}
