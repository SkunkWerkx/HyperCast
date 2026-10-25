//go:build cgo && !tinygo && !hypercast_local && (darwin || linux || windows) && !ios && !android

package hypercast

// The core freshness check. Without the hypercast_local tag the suite links the archives
// committed under staticlib/, which stage-native-binaries.yml refreshes only at release, so
// after a core change a plain `go test` would test the core as it was at the last staging.
// TestMain refuses to run the suite when this checkout's rust/ differs from the commit those
// archives were built from. CI builds its own core and writes it over staticlib/ before the
// suite, and sets CI, so the check is skipped there; so it is wherever it cannot tell (no
// git, no staging commit, a shallow clone without its source commit), and when staticlib/
// itself has working-tree changes, since then someone wrote fresh archives into it. go test
// replays a cached pass without running TestMain, and it tracks no file outside this module,
// so a pass recorded before a change to rust/ is replayed after it: -count=1 runs the check
// again.

import (
	"errors"
	"fmt"
	"os"
	"os/exec"
	"regexp"
	"strings"
	"testing"
)

func TestMain(m *testing.M) {
	if message := staleCoreMessage(); message != "" {
		fmt.Fprintln(os.Stderr, message)
		os.Exit(1)
	}
	os.Exit(m.Run())
}

// stagedSourceSubject is the source commit at the end of a staging commit's subject:
// `chore: stage native binaries from ci.yml run N (<sha>)`.
var stagedSourceSubject = regexp.MustCompile(`\(([0-9a-f]{40})\)\s*$`)

// staleCoreMessage says why the suite would run against a stale core, or returns "" when
// the core is fresh or the check cannot tell.
func staleCoreMessage() string {
	const tree = "go/staticlib"
	if os.Getenv("CI") != "" {
		return ""
	}
	if _, err := exec.LookPath("git"); err != nil {
		return ""
	}
	top, ok := gitOutput("", "rev-parse", "--show-toplevel")
	if !ok || top == "" {
		return ""
	}
	staging, ok := gitOutput(top, "log", "-1", "--format=%H", "--grep=^chore: stage native binaries", "HEAD")
	if !ok || staging == "" {
		return ""
	}
	source, _ := gitOutput(top, "log", "-1", "--format=%(trailers:key=Release-Source-SHA,valueonly)", staging)
	source = strings.TrimSpace(strings.SplitN(source, "\n", 2)[0])
	if source == "" {
		subject, _ := gitOutput(top, "log", "-1", "--format=%s", staging)
		match := stagedSourceSubject.FindStringSubmatch(subject)
		if match == nil {
			return ""
		}
		source = match[1]
	}
	if _, ok := gitOutput(top, "cat-file", "-e", source+"^{commit}"); !ok {
		return ""
	}
	if gitDiffers(top, "HEAD", "--", tree, ":(exclude,glob)"+tree+"/**/README.md") != 0 {
		return ""
	}
	stale := gitDiffers(top, source, "--", "rust/src", "rust/Cargo.toml", "rust/Cargo.lock") == 1
	if !stale {
		untracked, ok := gitOutput(top, "ls-files", "--others", "--exclude-standard", "--", "rust/src")
		stale = ok && untracked != ""
	}
	if !stale {
		return ""
	}
	return tree + " was built from rust/ at " + source[:12] + ", and this checkout's rust/ differs from it, " +
		"so these tests would run against a stale core. Build the checkout's own with " +
		".github/scripts/local-core.sh (FORGE=<path> if the SkunkWerkx/.github checkout is not " +
		"../.github or ../HyperForge), then run go test -tags hypercast_local ./... instead; " +
		"CI builds its own core and skips this check."
}

// gitOutput runs git in dir and returns its trimmed standard output and whether it exited 0.
func gitOutput(dir string, args ...string) (string, bool) {
	cmd := exec.Command("git", args...)
	cmd.Dir = dir
	out, err := cmd.Output()
	return strings.TrimSpace(string(out)), err == nil
}

// gitDiffers runs `git diff --quiet` in dir and returns its exit code: 0 for no difference,
// 1 for a difference, and anything else (-1 when git did not run) for an error.
func gitDiffers(dir string, args ...string) int {
	cmd := exec.Command("git", append([]string{"diff", "--quiet"}, args...)...)
	cmd.Dir = dir
	err := cmd.Run()
	var exit *exec.ExitError
	switch {
	case err == nil:
		return 0
	case errors.As(err, &exit):
		return exit.ExitCode()
	default:
		return -1
	}
}
