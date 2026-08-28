package hypercast

import (
	"fmt"
	"os"
)

// extractNativeLib copies this platform's embedded native library to a temp file and
// returns its path, ready for dlopen. The temp file is deliberately never removed — Go has
// no reliable process-exit hook, the same best-effort tradeoff every binding here makes.
func extractNativeLib() (string, error) {
	t, err := currentTarget()
	if err != nil {
		return "", err
	}

	resourcePath := "native/" + t.rid + "/" + t.libName
	data, err := nativeFS.ReadFile(resourcePath)
	if err != nil {
		return "", fmt.Errorf("hypercast: %s not found in embedded native libs (unsupported platform, or this module was built without a native library for it): %w", resourcePath, err)
	}

	tmp, err := os.CreateTemp("", "libhypercast-*-"+t.libName)
	if err != nil {
		return "", fmt.Errorf("hypercast: creating temp file for native library: %w", err)
	}
	_, writeErr := tmp.Write(data)
	closeErr := tmp.Close()
	// The write handle must be closed before dlopen opens the same path — Windows enforces
	// exclusive access far more strictly than Unix.
	if writeErr != nil {
		return "", fmt.Errorf("hypercast: writing native library to temp file: %w", writeErr)
	}
	if closeErr != nil {
		return "", fmt.Errorf("hypercast: closing temp file for native library: %w", closeErr)
	}
	return tmp.Name(), nil
}
