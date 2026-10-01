package hypercast

import (
	"errors"
	"fmt"
	"sync"
)

// ErrNativeUnavailable is what a load failure wraps: the native library (or, under the
// hypercast_wasm tag, the wasm module) could not be loaded — an unsupported platform, no
// embedded build for it, a failed extraction or dlopen, or a core that does not export the
// ABI this binding was built against. LoadError returns it wrapped around the specific
// reason, and it is the value every door panics with in that case, so test with errors.Is.
var ErrNativeUnavailable = errors.New("hypercast: native library unavailable")

var (
	initOnce sync.Once
	initErr  error

	// nativeVersion is the packed major<<16 | minor<<8 | patch the loaded core reported, set
	// by each backend's loadBackend as its final step — so a successful load has already
	// made one real call through the ABI, not merely resolved its symbols.
	nativeVersion uint32
)

// ensureLoaded loads whichever backend this build selected (loadBackend, one per backend
// file), exactly once, and caches the outcome: the name and signature are the same on all
// three backends so cast.go needs no knowledge of which one it got.
func ensureLoaded() error {
	initOnce.Do(func() {
		initErr = loadFailure(loadBackend())
	})
	return initErr
}

// loadFailure is the one place a backend's load error becomes this package's public one:
// it wraps ErrNativeUnavailable around the reason, so errors.Is finds the sentinel and the
// message still says what actually went wrong. A nil stays nil.
func loadFailure(err error) error {
	if err == nil {
		return nil
	}
	return fmt.Errorf("%w: %w", ErrNativeUnavailable, err)
}

// mustLoad is every door's first step. The doors return (value, *Fault) and a *Fault is a
// verdict about the input, so a library that never loaded has nowhere to go but a panic —
// with LoadError's own error as the value, which a recover can still errors.Is.
func mustLoad() {
	if err := ensureLoaded(); err != nil {
		panic(err)
	}
}

// Available reports whether the native library (or, under the hypercast_wasm tag, the
// wasm module) loaded and exports the ABI this binding was built against — every door's
// symbol resolved and hypercast_version answered. Probed once and cached; a false is
// permanent for the process, and LoadError says why. Neither probe ever panics on a load
// failure: a consumer keeping a fallback for a platform this module does not cover gates
// on them instead of recovering around its first cast.
func Available() bool {
	return ensureLoaded() == nil
}

// LoadError returns nil when the native library loaded, and otherwise the reason it did
// not, wrapping ErrNativeUnavailable — the same error every door panics with in that case.
// Probed once and cached, like Available.
func LoadError() error {
	return ensureLoaded()
}

// NativeVersion reports the loaded core's own version as "major.minor.patch" — read from
// the library itself, not from this module — so a deployment can name a mismatch between
// the binary it resolved and the one this binding was built against before making its
// first cast. Panics if the native library cannot be loaded, the same way every door does;
// Available and LoadError are the probes that do not.
func NativeVersion() string {
	mustLoad()
	v := nativeVersion
	return fmt.Sprintf("%d.%d.%d", v>>16, (v>>8)&0xFF, v&0xFF)
}
