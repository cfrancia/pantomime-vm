/**
 * Created by cfrancia on 25/09/16.
 */
public class NestedObjectFieldRetrieval {

    public static void main(String[] args) {
        MyObject object = new MyObject(5);
        println(object.getValue());
    }

    public static native void println(int val);

}
