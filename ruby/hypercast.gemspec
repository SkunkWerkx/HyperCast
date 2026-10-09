Gem::Specification.new do |spec|
  spec.name = "hypercast"
  # Kept in lockstep with HyperCast::VERSION (lib/hypercast.rb) and rust/Cargo.toml by the
  # prepare-release workflow, which rewrites all three together.
  spec.version = "0.8.0"
  spec.summary = "Scalar parsing as Success/Fault verdicts over a native Rust core, shipped prebuilt"
  spec.description = <<~DESC
    Booleans, numerics, exact decimals, UUIDs, and temporals cast from untrusted text by a
    native Rust core — every parse returns a verdict (the value, or a reason plus the
    offending span), never an exception for bad data. Two backends behind one surface,
    selected automatically and both shipped prebuilt: a Magnus extension where a precompiled
    platform gem matches, and stdlib Fiddle everywhere else. No runtime bridge, no
    dependencies beyond Fiddle.
  DESC
  spec.authors = ["Brian Buvinghausen"]
  spec.license = "MIT"
  spec.homepage = "https://github.com/SkunkWerkx/HyperCast"
  # 3.3 floor: the oldest Ruby still supported upstream (3.2 reached end of life on
  # 2026-03-31; Data.define, which the verdict case types are built on, arrived there). The
  # precompiled platform gems narrow this further, to the ABIs they carry — see the
  # Rakefile's native:gem task.
  spec.required_ruby_version = ">= 3.3"

  # LICENSE is a local copy of the repo root's, not a reference to it: RubyGems stores a
  # "../LICENSE" entry with the `..` intact (a path-traversal entry no installer accepts),
  # and a symlink is stored *as* a symlink — `gem build` warns, and it dangles once the gem
  # is unpacked somewhere else entirely. Same reason rust/ and python/ carry their own.
  #
  # native/*/* is exactly the staged binaries, one directory per RID.
  spec.files = Dir["lib/**/*.rb"] + Dir["lib/hypercast/native/*/*"] + ["README.md", "LICENSE"]
  spec.require_paths = ["lib"]

  # fiddle was a Ruby default gem (effectively stdlib, no declaration needed) through Ruby
  # 3.x; Ruby 4.0 unbundled it into a regular gem, so it needs an explicit dependency. Still
  # zero *third-party* runtime dependencies: fiddle ships with every Ruby install, just no
  # longer implicitly on the load path. This gemspec is the universal gem's, the one that runs
  # on Fiddle; the precompiled platform gems drop the dependency (Rakefile, native:gem), since
  # they carry no library for Fiddle to open.
  spec.add_dependency "fiddle"
  # The test and benchmark gems live in the Gemfile.
  spec.add_development_dependency "rake", "~> 13.0"
  spec.add_development_dependency "yard", "~> 0.9"

  spec.metadata["source_code_uri"] = spec.homepage
  spec.metadata["changelog_uri"] = "#{spec.homepage}/blob/master/CHANGELOG.md"
  spec.metadata["bug_tracker_uri"] = "#{spec.homepage}/issues"
  spec.metadata["documentation_uri"] = "#{spec.homepage}/tree/master/ruby#readme"
  # Pushing or yanking a version takes an account with multi-factor authentication on.
  spec.metadata["rubygems_mfa_required"] = "true"
end
