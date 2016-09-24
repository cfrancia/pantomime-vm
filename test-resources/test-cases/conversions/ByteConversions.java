/**
 * Created by cfrancia on 24/09/16.
 */
public class ByteConversions {

    public static void main(String[] args) {
        int a = 5;
        println((byte) a);
    }

    public static native void println(byte val);

}
