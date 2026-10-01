//go:build darwin || freebsd || linux || netbsd

package hypercast

import "github.com/ebitengine/purego"

// RTLD_LOCAL: every symbol this binding calls is resolved through the returned handle, so
// nothing needs the library's exports in the process-wide namespace — where a second
// library exporting the same cast_* names would collide with them.
func openLibrary(path string) (uintptr, error) {
	return purego.Dlopen(path, purego.RTLD_NOW|purego.RTLD_LOCAL)
}
