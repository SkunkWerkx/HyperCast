package io.github.skunkwerkx.hypercast.interop;

import java.io.IOException;
import java.io.InputStream;
import java.io.UncheckedIOException;
import java.lang.foreign.Arena;
import java.lang.foreign.SymbolLookup;
import java.nio.charset.StandardCharsets;
import java.nio.file.Files;
import java.nio.file.Path;
import java.nio.file.StandardCopyOption;
import java.util.Locale;
import java.util.stream.Stream;

/**
 * Maps the running JVM's OS, architecture and — on Linux — C library to the RID-style
 * directory (matching the C# binding's {@code runtimes/{RID}/native/} convention) and file
 * name a native library was built for, and loads that library out of a jar. Every SkunkWerkx
 * core ships the same way — one jar carrying each platform's build under
 * {@code /native/{rid}/{file}} — so this is public for any of them: HyperCast loads
 * {@code hypercast} with it, HyperTabular {@code hypertabular}.
 *
 * <p>A {@code .jar} has no package-manager-level platform selection the way NuGet's RID
 * folders do — one jar has to work everywhere, so it bundles every platform's build and this
 * picks the right one at runtime instead.
 *
 * <p>Nothing here rounds to the nearest RID. An architecture that is neither x64 nor arm64,
 * or an OS that is none of Linux, macOS and Windows, resolves to no target, because the
 * nearest-looking library would only fail to load. Linux resolves to one of two families:
 * glibc ({@code linux-x64}) or musl ({@code linux-musl-x64}, Alpine's), told apart by the
 * loader this very process has mapped.
 */
public final class NativePlatform {
    /**
     * Where one platform's build of a library lives in the jar.
     *
     * @param rid the RID-style directory, {@code linux-x64} say
     * @param libraryFileName the library's file name, {@code libhypercast.so} say
     */
    public record Target(String rid, String libraryFileName) {
        /**
         * The classpath resource path of this target's bundled library.
         *
         * @return {@code /native/{rid}/{file}}
         */
        public String resourcePath() {
            return "/native/" + rid + "/" + libraryFileName;
        }
    }

    // Read once, on first access to this class.
    private static final boolean MUSL = runsOnMusl();

    private NativePlatform() {}

    /**
     * This platform's target for a library.
     *
     * @param baseName the library's base name, {@code hypercast} for {@code libhypercast.so} and {@code hypercast.dll}
     * @return the target, or {@code null} when no build of any library exists for this platform
     */
    public static Target current(String baseName) {
        return resolve(baseName, System.getProperty("os.name"), System.getProperty("os.arch"), MUSL);
    }

    /**
     * The OS and architecture as the JVM names them, for a message about a missing build.
     *
     * @return {@code os.name=… os.arch=…}
     */
    public static String describe() {
        return "os.name=" + System.getProperty("os.name") + " os.arch=" + System.getProperty("os.arch");
    }

    /**
     * The target for an OS name and architecture as {@code os.name}/{@code os.arch} spell them.
     *
     * @param baseName the library's base name
     * @param osName the OS, as {@code os.name} spells it
     * @param osArch the architecture, as {@code os.arch} spells it
     * @param musl whether the C library is musl; only matters on Linux
     * @return the target, or {@code null} for a combination no build exists for
     */
    public static Target resolve(String baseName, String osName, String osArch, boolean musl) {
        String arch = switch (osArch.toLowerCase(Locale.ROOT)) {
            case "amd64", "x86_64", "x64" -> "x64";
            case "aarch64", "arm64" -> "arm64";
            // riscv64, ppc64le, s390x, 32-bit x86 and arm, and whatever comes next.
            default -> null;
        };
        if (arch == null) {
            return null;
        }
        String os = osName.toLowerCase(Locale.ROOT);
        if (os.startsWith("windows")) {
            return new Target("win-" + arch, baseName + ".dll");
        }
        if (os.startsWith("mac") || os.startsWith("darwin")) {
            return new Target("osx-" + arch, "lib" + baseName + ".dylib");
        }
        if (os.startsWith("linux")) {
            return new Target((musl ? "linux-musl-" : "linux-") + arch, "lib" + baseName + ".so");
        }
        return null;
    }

    /**
     * Why there is no native library to load: no build exists for this platform at all, or
     * one should and the jar was packed without it.
     *
     * @param baseName the library's base name
     * @param target the platform's target, or {@code null} when there is none
     * @return the message
     */
    public static String missing(String baseName, Target target) {
        return target == null
                ? baseName + ": this jar carries no native library for " + describe()
                : target.resourcePath() + " classpath resource not found (this jar was built "
                        + "without a native library for this platform)";
    }

    /**
     * Loads a bundled library for the life of the process: copies the jar's build for this
     * platform to a temporary file (deleted on exit) and opens it into the global arena, since
     * the library must outlive every downcall made through it.
     *
     * @param anchor a class in the jar that carries the library
     * @param baseName the library's base name
     * @param target the platform's target, from {@link #current(String)}
     * @return the opened library
     * @throws IllegalStateException there is no build for this platform, or the jar lacks it
     * @throws UncheckedIOException the temporary copy could not be made
     */
    public static SymbolLookup load(Class<?> anchor, String baseName, Target target) {
        if (target == null) {
            throw new IllegalStateException(missing(baseName, null));
        }
        try (InputStream resource = anchor.getResourceAsStream(target.resourcePath())) {
            if (resource == null) {
                throw new IllegalStateException(missing(baseName, target));
            }
            String libraryFileName = target.libraryFileName();
            String extension = libraryFileName.substring(libraryFileName.lastIndexOf('.'));
            Path tmp = Files.createTempFile(baseName, extension);
            tmp.toFile().deleteOnExit();
            Files.copy(resource, tmp, StandardCopyOption.REPLACE_EXISTING);
            return SymbolLookup.libraryLookup(tmp, Arena.global());
        } catch (IOException e) {
            throw new UncheckedIOException(e);
        }
    }

    // The same rule every binding applies: the process is a musl one when its own memory map
    // names musl's loader. A map that cannot be read — no /proc, or not Linux at all — means
    // glibc, the common case and the build that has always shipped. Latin-1 because the map
    // is paths, not text: it decodes any byte, so an oddly named mapping cannot turn a
    // readable map into an unreadable one.
    private static boolean runsOnMusl() {
        try (Stream<String> maps = Files.lines(Path.of("/proc/self/maps"), StandardCharsets.ISO_8859_1)) {
            return mentionsMusl(maps);
        } catch (IOException | RuntimeException unreadable) {
            return false;
        }
    }

    /**
     * Whether any line of a {@code /proc/{pid}/maps} listing names musl's loader or libc.
     *
     * @param maps the listing's lines
     * @return {@code true} for a musl process
     */
    public static boolean mentionsMusl(Stream<String> maps) {
        return maps.anyMatch(line -> line.contains("ld-musl-") || line.contains("libc.musl-"));
    }
}
