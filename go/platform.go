package hypercast

import (
	"fmt"
	"runtime"
)

// target names the RID-style directory (matching the other bindings' runtimes/{rid}/
// convention) and native library filename for the running GOOS/GOARCH.
type target struct {
	rid     string
	libName string
}

// currentTarget maps the running GOOS/GOARCH to the embedded native library it should load.
func currentTarget() (target, error) {
	isArm := runtime.GOARCH == "arm64"

	switch runtime.GOOS {
	case "windows":
		if isArm {
			return target{"win-arm64", "hypercast.dll"}, nil
		}
		return target{"win-x64", "hypercast.dll"}, nil
	case "darwin":
		if isArm {
			return target{"osx-arm64", "libhypercast.dylib"}, nil
		}
		return target{"osx-x64", "libhypercast.dylib"}, nil
	case "linux":
		if isArm {
			return target{"linux-arm64", "libhypercast.so"}, nil
		}
		return target{"linux-x64", "libhypercast.so"}, nil
	default:
		return target{}, fmt.Errorf("hypercast: unsupported platform %s/%s", runtime.GOOS, runtime.GOARCH)
	}
}
