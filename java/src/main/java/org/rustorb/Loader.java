// Loads librustorb_jni.
//
// Licensed under MIT OR Apache-2.0, like the rest of the rustorb core.

package org.rustorb;

final class Loader {
    private Loader() {}

    /**
     * Loads the library from the path in the system property
     * {@code rustorb.jni.library} or the environment variable
     * {@code RUSTORB_JNI_LIBRARY}, else as {@code rustorb_jni} from
     * {@code java.library.path}.
     */
    static void load() {
        String path = System.getProperty("rustorb.jni.library");
        if (path == null || path.isEmpty()) {
            path = System.getenv("RUSTORB_JNI_LIBRARY");
        }
        if (path != null && !path.isEmpty()) {
            System.load(new java.io.File(path).getAbsolutePath());
        } else {
            System.loadLibrary("rustorb_jni");
        }
    }
}
