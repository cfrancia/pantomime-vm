/**
 * Created by cfrancia on 25/09/16.
 */
public class ObjectValueRetrieval {

    public static void main(String[] args) {
        MyObject object = new MyObject();
        println(object.getHelloWorld());
    }

    public static native void println(int val);

}
