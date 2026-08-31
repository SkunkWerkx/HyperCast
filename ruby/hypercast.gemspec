Gem::Specification.new do |spec|
  spec.name = "hypercast"
  spec.version = "0.1.0"
  spec.authors = ["Brian Buvinghausen"]
  spec.summary = "Allocation-free scalar parsing as Success/Fault verdicts over a native Rust core"
  spec.description = "Booleans, numerics, UUIDs, and temporals cast from untrusted text via " \
                     "Fiddle bindings straight into libhypercast — every parse returns a verdict " \
                     "(the value, or a reason plus the offending span), never an exception for " \
                     "bad data. No runtime bridge, no dependencies beyond stdlib Fiddle."
  spec.homepage = "https://github.com/SkunkWerkx/HyperCast"
  spec.license = "MIT"
  # 3.2 floor: Data.define for the verdict case types (pattern-matchable value objects).
  spec.required_ruby_version = ">= 3.2"

  spec.files = Dir["lib/**/*.rb"] + Dir["lib/hypercast/native/*/*"] + ["README.md"]
  spec.require_paths = ["lib"]

  spec.add_dependency "fiddle"
  spec.add_development_dependency "rake", "~> 13.0"
  spec.add_development_dependency "yard", "~> 0.9"

  spec.metadata["source_code_uri"] = spec.homepage
end
