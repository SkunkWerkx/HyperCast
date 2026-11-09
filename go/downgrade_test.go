//go:build !tinygo && !js && !wasip1

package hypercast

// The downgrade guard. Go has no core to load: internal/core/core.go is the core, translated
// and compiled into the package, regenerated with the crate and diffed by TestCoreIsCurrent,
// and TestRegenerationCarriesTheCrateVersion holds the version the translation carries to the
// crate's. Every door crosses through the core's detailed exports, the only ones that read a
// grouping policy, so the one way to pair this binding with a 1.0 core is to put a 1.0
// translation in its place, and then the package does not build: no request can reach a core
// that would parse it by 1.0's rule. This test proves it with a real 1.0 core: the generator
// translates the Rust tree HYPERCAST_DOWNGRADE_ROOT names (a checkout of tag v1.0.0, holding
// rust/) into a copy of this package, whose build must fail on the missing detailed export;
// the same copy with the current core, the control, must build. Skipped without the variable,
// since the 1.0 tree is an input.

import (
	"os"
	"os/exec"
	"path/filepath"
	"strings"
	"testing"
)

func TestADowngradedCoreDoesNotBuild(t *testing.T) {
	root := os.Getenv("HYPERCAST_DOWNGRADE_ROOT")
	if root == "" {
		t.Skip("HYPERCAST_DOWNGRADE_ROOT (a checkout of tag v1.0.0) is not set")
	}
	requireRegeneration(t, root)
	if data := mustRead(t, filepath.Join(root, "rust", "Cargo.toml")); !strings.Contains(string(data), "\nversion = \"1.0.0\"") {
		t.Fatalf("%s/rust is not the 1.0.0 crate", root)
	}
	old := filepath.Join(t.TempDir(), "core.go")
	if err := regenerate(t, root, old); err != nil {
		t.Fatalf("translating the 1.0 core: %v", err)
	}
	if strings.Contains(string(mustRead(t, old)), "Xcast_detailed") {
		t.Fatal("the 1.0 translation carries a detailed export: not a 1.0 core")
	}

	control, err := buildWithCore(t, "internal/core/core.go")
	if err != nil {
		t.Fatalf("control: the package with its own core does not build: %v\n%s", err, control)
	}
	out, err := buildWithCore(t, old)
	if err == nil {
		t.Fatal("the package built against a 1.0 core")
	}
	if !strings.Contains(out, "Xcast_detailed") {
		t.Fatalf("the build against a 1.0 core failed for another reason:\n%s", out)
	}
	t.Logf("against the 1.0 core, go build fails:\n%s", out)
}

// buildWithCore copies this package's non-test sources and module files into a temporary
// module with core as its internal/core/core.go, and runs go build there.
func buildWithCore(t *testing.T, core string) (string, error) {
	t.Helper()
	dir := t.TempDir()
	sources, err := filepath.Glob("*.go")
	if err != nil {
		t.Fatal(err)
	}
	for _, name := range append(sources, "go.mod", "go.sum") {
		if strings.HasSuffix(name, "_test.go") {
			continue
		}
		if err := os.WriteFile(filepath.Join(dir, name), mustRead(t, name), 0o644); err != nil {
			t.Fatal(err)
		}
	}
	if err := os.MkdirAll(filepath.Join(dir, "internal", "core"), 0o755); err != nil {
		t.Fatal(err)
	}
	if err := os.WriteFile(filepath.Join(dir, "internal", "core", "core.go"), mustRead(t, core), 0o644); err != nil {
		t.Fatal(err)
	}
	cmd := exec.Command("go", "build", ".")
	cmd.Dir = dir
	cmd.Env = append(os.Environ(), "CGO_ENABLED=0")
	out, err := cmd.CombinedOutput()
	return string(out), err
}
