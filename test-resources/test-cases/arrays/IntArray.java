/**
 * Created by cfrancia on 26/09/16.
 */
public class IntArray {

    public static void main(String[] args) {
        int[] charArray = new int[]{1, 2, 3, 4, 5};

        for (int i = 0; i < charArray.length; i++) {
            println(charArray[i]);
        }
    }

    public static native void println(int val);

}
