Native builds land here as native/{rid}/{lib} — staged locally from the in-repo cargo
build for development, populated for every platform by CI before packaging. The binaries
themselves are gitignored; this README holds the directory (go:embed needs a match).
