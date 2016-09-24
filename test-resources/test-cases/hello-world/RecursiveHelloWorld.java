/**
 * Created by cfrancia on 24/09/16.
 */
public class RecursiveHelloWorld {

    public static void main(String[] args) {
        chain("Hello world!");
    }

    public static void chain(String val) {
        println(val);
    }

    public static native void println(String val);

}
