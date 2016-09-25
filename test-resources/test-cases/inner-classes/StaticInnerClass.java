/**
 * Created by cfrancia on 24/09/16.
 */
public class StaticInnerClass {

    public static void main(String[] args) {
        Inner.innerPrint(5);
    }

    public static class Inner {
        private static int CONTENTS;

        public static void innerPrint(int val) {
            CONTENTS = val;
            println(CONTENTS);
        }

        public static native void println(int val);
    }

}
