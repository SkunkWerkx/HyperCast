import HyperCast
import Testing

/// `NumStyles` can hold any bits, so the constructor and `withStyles(_:)` are where a bad word stops.
/// They trap, so each case is its own process. Exit tests exist only where a test can spawn one, and
/// each spells its value out, since the floor toolchain (6.2) can't capture into an exit test.
#if os(macOS) || os(Linux) || os(Windows)
    @Suite struct StylesRefusalExitTests {
        // 351: the stale word, the retired bit 8 over the default styles.
        @Test func theConstructorTrapsOn351() async {
            await #expect(processExitsWith: .failure) {
                _ = NumFormat(decimalSeparator: ".", groupSeparator: ",", styles: NumStyles(rawValue: 351))
            }
        }

        @Test func withStylesTrapsOn351() async {
            await #expect(processExitsWith: .failure) {
                _ = NumFormat.invariant.withStyles(NumStyles(rawValue: 351))
            }
        }

        // 863: bits 8 and 9.
        @Test func theConstructorTrapsOn863() async {
            await #expect(processExitsWith: .failure) {
                _ = NumFormat(decimalSeparator: ".", groupSeparator: ",", styles: NumStyles(rawValue: 863))
            }
        }

        @Test func withStylesTrapsOn863() async {
            await #expect(processExitsWith: .failure) {
                _ = NumFormat.invariant.withStyles(NumStyles(rawValue: 863))
            }
        }

        // 256: the retired bit alone.
        @Test func theConstructorTrapsOn256() async {
            await #expect(processExitsWith: .failure) {
                _ = NumFormat(decimalSeparator: ".", groupSeparator: ",", styles: NumStyles(rawValue: 256))
            }
        }

        @Test func withStylesTrapsOn256() async {
            await #expect(processExitsWith: .failure) {
                _ = NumFormat.invariant.withStyles(NumStyles(rawValue: 256))
            }
        }

        // 768: the refused field.
        @Test func theConstructorTrapsOn768() async {
            await #expect(processExitsWith: .failure) {
                _ = NumFormat(decimalSeparator: ".", groupSeparator: ",", styles: NumStyles(rawValue: 768))
            }
        }

        @Test func withStylesTrapsOn768() async {
            await #expect(processExitsWith: .failure) {
                _ = NumFormat.invariant.withStyles(NumStyles(rawValue: 768))
            }
        }

        // 607: the Indian bit, which is a policy, never a style.
        @Test func theConstructorTrapsOn607() async {
            await #expect(processExitsWith: .failure) {
                _ = NumFormat(decimalSeparator: ".", groupSeparator: ",", styles: NumStyles(rawValue: 607))
            }
        }

        @Test func withStylesTrapsOn607() async {
            await #expect(processExitsWith: .failure) {
                _ = NumFormat.invariant.withStyles(NumStyles(rawValue: 607))
            }
        }

        // The control: the same process shape exits cleanly on a word the type allows.
        @Test func theDefaultStylesDontTrap() async {
            await #expect(processExitsWith: .success) {
                let format = NumFormat(decimalSeparator: ".", groupSeparator: ",", styles: NumStyles(rawValue: 95))
                _ = format.withStyles(NumStyles(rawValue: 95))
            }
        }
    }
#endif
