/**
 * Created by cfrancia on 26/09/16.
 */
public class IntLoop {

    public static void main(String[] args) {
        int total = 0;

        for (int i = 0; i < 10; i++) {
            total += i;
        }

        println(total);
    }

    public static native void println(int val);

}
