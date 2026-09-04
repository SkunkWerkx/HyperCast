Gem::Specification.new do |spec|
  spec.name = "hypercast"
  spec.version = "0.3.0"
  spec.authors = ["Brian Buvinghausen"]
  spec.summary = "Allocation-free scalar parsing as Success/Fault verdicts over a native Rust core"
  spec.description = "Booleans, numerics, UUIDs, and temporals cast from untrusted text by a " \
                     "native Rust core — every parse returns a verdict (the value, or a reason " \
                     "plus the offending span), never an exception for bad data. Two backends " \
                     "behind one surface, selected automatically with nothing ever compiled: a " \
                     "Magnus extension where a precompiled platform gem matches, stdlib Fiddle " \
                     "everywhere else. No runtime bridge, no dependencies beyond Fiddle."
  spec.homepage = "https://github.com/SkunkWerkx/HyperCast"
  spec.license = "MIT"
  # 3.2 floor: Data.define for the verdict case types (pattern-matchable value objects).
  spec.required_ruby_version = ">= 3.2"

  # LICENSE is a local copy of the repo root's, not a reference to it: RubyGems stores a
  # "../LICENSE" entry with the `..` intact (a path-traversal entry no installer accepts),
  # and a symlink is stored *as* a symlink — `gem build` warns, and it dangles once the gem
  # is unpacked somewhere else entirely. Same reason rust/ and python/ carry their own.
  spec.files = Dir["lib/**/*.rb"] + Dir["lib/hypercast/native/*/*"] + ["README.md", "LICENSE"]
  spec.require_paths = ["lib"]

  spec.add_dependency "fiddle"
  # wasmtime — the engine behind the WebAssembly backend (lib/hypercast/wasm_runtime.rb) —
  # is deliberately absent here. It is never a dependency of this gem, and it is not a
  # development dependency either: it lives in the Gemfile's own `:wasm` group, so CI can
  # install it only where the wasm suite actually runs. See the Gemfile.
  spec.add_development_dependency "rake", "~> 13.0"
  spec.add_development_dependency "yard", "~> 0.9"

  spec.metadata["source_code_uri"] = spec.homepage
end
