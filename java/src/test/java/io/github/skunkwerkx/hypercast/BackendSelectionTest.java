package io.github.skunkwerkx.hypercast;

import static org.junit.jupiter.api.Assertions.assertEquals;

import org.junit.jupiter.api.Test;

/**
 * Pins that the property really is what picks the path: the plain {@code test} task sets
 * nothing and must land on FFM (this build always stages the platform's native library first),
 * while {@code testWasm} sets {@code hypercast.backend=wasm} and must land on GraalWasm. The
 * rest of the suite runs identically under both; this is the one assertion that would catch
 * the switch silently doing nothing.
 */
class BackendSelectionTest {

    @Test
    void backendFollowsTheProperty() {
        String expected = System.getProperty(Cast.BACKEND_PROPERTY, "native");
        assertEquals(expected, Cast.backend());
    }
}
