#if !HYPERCAST_LOCAL_CORE && (os(macOS) || os(Linux) || os(Windows))
    import Foundation
    import XCTest

    /// The core freshness check. Without `HYPERCAST_LOCAL_CORE` the suite links the bundle
    /// committed as `swift/HyperCastCore.artifactbundle`, which stage-native-binaries.yml
    /// refreshes only at release, so after a core change a plain `swift test` would test the
    /// core as it was at the last staging. This test fails when this checkout's rust/ differs
    /// from the commit that bundle was built from; XCTest cannot stop the run early, so it is a
    /// named failure beside the rest of the suite. CI builds its own core and writes it over
    /// the bundle before the suite, and sets CI, so the check is skipped there; so it is
    /// wherever it cannot tell (no git, no staging commit, a shallow clone without its source
    /// commit), and when the bundle itself has working-tree changes, since then someone wrote
    /// fresh archives into it.
    final class CoreFreshnessTests: XCTestCase {
        func testTheLinkedCoreWasBuiltFromThisCheckout() {
            if let message = Self.staleCoreMessage() {
                XCTFail(message)
            }
        }

        /// Why the suite would run against a stale core, or nil when the core is fresh or the
        /// check cannot tell.
        private static func staleCoreMessage() -> String? {
            let tree = "swift/HyperCastCore.artifactbundle"
            let environment = ProcessInfo.processInfo.environment
            if let ci = environment["CI"], !ci.isEmpty { return nil }
            guard let git = executable(named: "git", in: environment) else { return nil }
            let here = URL(fileURLWithPath: #filePath).deletingLastPathComponent()
            guard let top = run(git, ["rev-parse", "--show-toplevel"], in: here).output, !top.isEmpty
            else { return nil }
            let root = URL(fileURLWithPath: top, isDirectory: true)
            guard
                let staging = run(
                    git, ["log", "-1", "--format=%H", "--grep=^chore: stage native binaries", "HEAD"], in: root
                ).output, !staging.isEmpty
            else { return nil }
            let trailer = run(
                git, ["log", "-1", "--format=%(trailers:key=Release-Source-SHA,valueonly)", staging], in: root
            ).output
            var source =
                (trailer ?? "").split(separator: "\n").first.map {
                    $0.trimmingCharacters(in: .whitespaces)
                } ?? ""
            if source.isEmpty {
                guard let subject = run(git, ["log", "-1", "--format=%s", staging], in: root).output,
                    let found = sourceInSubject(subject)
                else { return nil }
                source = found
            }
            guard run(git, ["cat-file", "-e", "\(source)^{commit}"], in: root).status == 0 else { return nil }
            let treeStatus = run(
                git, ["diff", "--quiet", "HEAD", "--", tree, ":(exclude,glob)\(tree)/**/README.md"], in: root
            ).status
            guard treeStatus == 0 else { return nil }
            var stale =
                run(
                    git, ["diff", "--quiet", source, "--", "rust/src", "rust/Cargo.toml", "rust/Cargo.lock"],
                    in: root
                ).status == 1
            if !stale {
                let untracked = run(git, ["ls-files", "--others", "--exclude-standard", "--", "rust/src"], in: root)
                stale = !(untracked.output ?? "").isEmpty
            }
            guard stale else { return nil }
            return "\(tree) was built from rust/ at \(source.prefix(12)), and this checkout's rust/ differs from it, "
                + "so these tests would run against a stale core. Build the checkout's own with "
                + ".github/scripts/local-core.sh (FORGE=<path> if the SkunkWerkx/.github checkout is not "
                + "../.github or ../HyperForge), then run HYPERCAST_LOCAL_CORE=1 swift test instead; "
                + "CI builds its own core and skips this check."
        }

        /// The 40-hex source commit in parentheses at the end of a staging commit's subject:
        /// `chore: stage native binaries from ci.yml run N (<sha>)`.
        private static func sourceInSubject(_ subject: String) -> String? {
            let trimmed = subject.trimmingCharacters(in: .whitespaces)
            guard trimmed.hasSuffix(")"), let open = trimmed.lastIndex(of: "(") else { return nil }
            let sha = trimmed[trimmed.index(after: open)..<trimmed.index(before: trimmed.endIndex)]
            let hex = Set("0123456789abcdef")
            return sha.count == 40 && sha.allSatisfy(hex.contains) ? String(sha) : nil
        }

        /// The first file named `name` on PATH, or nil.
        private static func executable(named name: String, in environment: [String: String]) -> URL? {
            #if os(Windows)
                let separator: Character = ";"
                let names = [name + ".exe", name]
                let path = environment["Path"] ?? environment["PATH"] ?? ""
            #else
                let separator: Character = ":"
                let names = [name]
                let path = environment["PATH"] ?? ""
            #endif
            for directory in path.split(separator: separator) {
                for candidate in names {
                    let url = URL(fileURLWithPath: String(directory)).appendingPathComponent(candidate)
                    if FileManager.default.isExecutableFile(atPath: url.path) { return url }
                }
            }
            return nil
        }

        /// Runs `executable` with `arguments` in `directory`: its exit status (-1 when it did not
        /// run), and its trimmed standard output when that status is 0.
        private static func run(
            _ executable: URL, _ arguments: [String], in directory: URL
        ) -> (status: Int32, output: String?) {
            let process = Process()
            process.executableURL = executable
            process.arguments = arguments
            process.currentDirectoryURL = directory
            let stdout = Pipe()
            process.standardOutput = stdout
            process.standardError = FileHandle.nullDevice
            process.standardInput = FileHandle.nullDevice
            do { try process.run() } catch { return (-1, nil) }
            let data = stdout.fileHandleForReading.readDataToEndOfFile()
            process.waitUntilExit()
            let status = process.terminationStatus
            let output = String(decoding: data, as: UTF8.self).trimmingCharacters(in: .whitespacesAndNewlines)
            return (status, status == 0 ? output : nil)
        }
    }
#endif
