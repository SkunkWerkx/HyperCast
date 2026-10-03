package hypercast

import (
	"errors"
	"fmt"
)

// ErrNativeUnavailable is never returned and never panicked with: the core is linked into
// the binary, so there is no load that can fail.
//
// Deprecated: kept only so code that tests for it keeps compiling.
var ErrNativeUnavailable = errors.New("hypercast: native library unavailable")

// Available reports whether the core can be called. The core is linked into the binary
// (backend_static.go), so there is nothing to load and nothing that can fail: it is always
// true. It stays for code written against the load probe every binding carries.
func Available() bool {
	return true
}

// LoadError returns why the core could not be loaded. A linked core always can, so it is
// always nil. Kept for the reason Available is.
func LoadError() error {
	return nil
}

// NativeVersion reports the linked core's own version as "major.minor.patch" — read from the
// core itself, not from this module — so a deployment can confirm which build of the archive
// went into the binary. It cannot fail.
func NativeVersion() string {
	v := packedVersion()
	return fmt.Sprintf("%d.%d.%d", v>>16, (v>>8)&0xFF, v&0xFF)
}
