package io.github.skunkwerkx.hypercast.probe;

import io.github.skunkwerkx.hypercast.Cast;
import java.lang.reflect.Method;

/**
 * The module-path child's program. It lives in its own package: the library's package belongs to
 * the automatic module there, so a class of this package name on the class path would be hidden.
 * The backend is read reflectively (an automatic module opens all its packages).
 */
public final class ModuleMain {
    private ModuleMain() {}

    public static void main(String[] args) throws Exception {
        System.out.println("i32: " + Cast.i32("42", io.github.skunkwerkx.hypercast.NumFormat.INVARIANT));
        Method backend =
                Class.forName("io.github.skunkwerkx.hypercast.NativeCore").getDeclaredMethod("backend");
        backend.setAccessible(true);
        System.out.println("backend: " + backend.invoke(null));
        System.out.println("module: " + Cast.class.getModule().getName());
    }
}
