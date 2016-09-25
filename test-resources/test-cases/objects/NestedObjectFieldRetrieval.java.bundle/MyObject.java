/**
 * Created by cfrancia on 25/09/16.
 */
public class MyObject {

    private NestedObject nestedObject;

    public MyObject(int value) {
        this.nestedObject = new NestedObject(value);
    }

    public int getValue() {
        return nestedObject.getValue();
    }

}
