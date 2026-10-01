import Foundation

/// The one error a door in this package throws: the bundled native `libhypercast` itself
/// couldn't be used — found, opened, or resolved against the exports this binding was built
/// for. Bad data is never thrown — it is a ``Verdict``'s ``Fault`` — so catching this type
/// is catching "the library isn't there", and nothing else.
///
/// The load is attempted once per process and its outcome kept, so every call after a
/// failed one throws the same value; ``Cast/isAvailable`` asks the same question without a
/// `do`/`catch`.
public enum NativeLibraryError: Error, CustomStringConvertible, LocalizedError {
    /// The library file wasn't found or couldn't be opened. `path` is where it was expected
    /// and `reason` is the loader's own explanation — most often a deployment that copied the
    /// executable without the resource directory beside it.
    case openFailed(path: String, reason: String)
    /// The library opened but doesn't export `name` — it is a different build than the one
    /// this binding was written against.
    case symbolNotFound(name: String)

    /// What failed, naming the path or the symbol involved.
    public var description: String {
        switch self {
        case .openFailed(let path, let reason):
            return "hypercast: failed to load native library at \(path): \(reason)"
        case .symbolNotFound(let name):
            return "hypercast: symbol \(name) not found in native library"
        }
    }

    /// The same text as ``description``, so `localizedDescription` names the failure too
    /// instead of Foundation's generic "The operation couldn't be completed".
    public var errorDescription: String? { description }
}
