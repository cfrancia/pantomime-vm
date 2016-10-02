/**
 * Created by cfrancia on 25/09/16.
 */
public class NestedObjectStringFieldRetrieval {

    public static void main(String[] args) {
        MyObject object = new MyObject("Hello world!");
        println(object.getValue());
    }

    public static native void println(String value);

}
