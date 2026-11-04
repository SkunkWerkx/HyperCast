# npm consumer tests

The harness itself (Verdaccio, the bundlers) installs with Bun from `bun.lock`. npm appears here
because the scenarios install as an npm user does, and publish to the local registry.

What a consumer gets from the ten npm tarballs, proven by installing them. `run.sh` serves the
packed tarballs from a local [Verdaccio](https://verdaccio.org) registry on port 4873, then
runs each scenario in a fresh directory under `work/<scenario>/` with its own `package.json`,
`.npmrc` and npm cache, so `npm install` resolves, selects platforms and writes lockfiles
exactly as it would against npmjs. Nothing here reaches npmjs for the packages under test:
`@skunkwerkx/*` has no uplink in `verdaccio.yaml`; everything else (the WASI package's runtime,
the bundlers' dependencies) proxies through. `smoke.cjs` is the consumer: it requires
`@skunkwerkx/hypercast`, prints `available=<bool> target=<native|wasm32-wasip1|none>`, casts an
accounting negative, the `u64` maximum and a contract violation, and checks one timestamp
vector and one spanned integer fault (as bytes) from `HYPERCAST_CORPUS`. When nothing loaded it
prints the `ERR_HYPERCAST_UNAVAILABLE` error's cause chain, one line per cause. `bundled.cjs`
is its public-API half, for the bundler scenario.

The header of `run.sh` lists the scenarios with what each asserts. In CI, `test-npm-package` in
`ci.yml` runs them on Linux, macOS and Windows after the whole suite under
`NAPI_RS_FORCE_WASI=error` on each; the Docker scenarios (`floor`, `s390x`) and `mac-lockfile`
run on Linux. `s390x` asserts a loss, not a platform: see below.

## Running locally

Pack tarballs from this machine's one native and the WASI build (from the repository root;
`bun run build` then `bun run build:wasi` in `typescript/` first):

```sh
mkdir -p /tmp/node-artifacts/node-osx-arm64 /tmp/node-artifacts/node-wasm32-wasip1
cp typescript/hypercast.darwin-arm64.node /tmp/node-artifacts/node-osx-arm64/
cp typescript/hypercast.wasm32-wasip1.wasm typescript/hypercast.wasip1* /tmp/node-artifacts/node-wasm32-wasip1/
python3 ../HyperForge/.github/scripts/npm_package.py pack --package typescript \
  --artifacts /tmp/node-artifacts --out /tmp/npm-local --allow-missing-natives
```

Then the host scenarios:

```sh
typescript/consumer-tests/run.sh /tmp/npm-local negative-unpublished plain omit-optional \
  absent-native forced-wasi negative-deleted-package negative-deleted-wasm mac-lockfile bundlers
```

And the container scenarios, which need Docker (on Docker Desktop the containers reach the
registry through `host.docker.internal`; on a Linux host leave `REGISTRY_HOST` unset and they
share the host network):

```sh
REGISTRY_HOST=host.docker.internal typescript/consumer-tests/run.sh /tmp/npm-local floor
REGISTRY_HOST=host.docker.internal typescript/consumer-tests/run.sh /tmp/npm-local s390x
```

Negative controls: `CORPUS_DIR=$(mktemp -d) run.sh <dir> plain` fails on the vector check;
`FLOOR_IMAGE=node:22.13.0-alpine run.sh <dir> floor` fails the install with `EBADENGINE`.

## What the runs established (2026-11-01, Apple M6, Node 26.13.0, npm 11.20.0, Docker 29.9.2)

Every scenario passed from a local pack of the darwin-arm64 addon and the WASI module, and
both negative controls failed as stated (an empty `CORPUS_DIR` on the timestamp vector, the
22.12.0 image with `EBADENGINE`). The floor scenario loaded `wasm32-wasip1` on
`node:22.13.0-alpine` with engine-strict, `node:22-alpine` and `node:24-alpine`, since a local
pack carries no musl addon.

- **Bundlers.** Neither esbuild (`--bundle --platform=node`) nor webpack produces a working
  self-contained bundle: webpack fails the build on the loader's platform `require`s, and
  esbuild bundles but the bundle cannot load the module from a directory with no
  `node_modules`. The configuration that passes, for both, is `@skunkwerkx/hypercast`
  **external**, installed beside the bundle. That is the configuration the package README
  documents.
- **npm bug 4828.** `mac-lockfile` reproduces npm/cli#4828 with npm 10.9.2, the npm Node
  22.13.0 bundles: a lockfile regenerated beside a darwin-arm64 `node_modules` names only that
  platform, and `npm ci --os=linux --cpu=x64` from it installs no native and loads the
  fallback. Node 22 users with a lockfile a Mac wrote are who the fallback is for.
- **Bun as the package manager.** `bun add` from a cold cache, then Node: native loads with an
  empty stderr, the other platform's package is skipped, the WASI package is installed, `bun
  install --frozen-lockfile` reproduces it, and each `@skunkwerkx` package has the integrity npm's
  lock gives it. Its cold install took 1.2 s where npm 11.20.0's took 1.6 s (Bun 1.4.4, the median of
  five runs, 2026-11-04, Apple M6, Node 26.13, a local Verdaccio already holding every package,
  two platforms' addons published of the eight). That is a small install from localhost, not a
  claim about a real registry or a large tree. The control: with no platform package published,
  the scenario fails.
- **Bun.** Under `NAPI_RS_FORCE_WASI=error`, Bun 1.4.4 cannot initialise the WASI module (its
  `node:wasi` has no reactor `initialize`), so `require('@skunkwerkx/hypercast')` succeeds with
  `isAvailable() === false`, printed as `bun: ...` and never failing the run. The native addon
  on Bun passes the whole suite.
- **Big-endian hosts.** In `node:24-alpine` for `linux/s390x` under QEMU the install resolves
  no native package, the fallback loads and every check of `smoke.cjs` passes, and the process
  then never exits (`smoke exit=143`, killed by the bound): the runtime writes the module's
  threadsafe-function reference count little-endian and reads it back host-endian, so the
  count never reads as zero and the `MessagePort` holding the event loop open is never
  released. `container/consumer.sh` bounds the smoke with
  `timeout 300` and prints `smoke exit=<status>`, and `scenario_s390x` asserts, after the
  checks, that the bound killed it: an exit of 0 means upstream fixed it and the docs are
  stale, so it fails too. Nothing beyond the smoke's checks is claimed for big-endian hosts.
- **Publishing to Verdaccio.** npm refuses to publish with no credentials at all, whatever the
  registry's rule, so `run.sh` creates one user through the registry's own user endpoint
  (`max_users: 1` in `verdaccio.yaml`) and puts its token in a user config of its own
  (`NPM_CONFIG_USERCONFIG=work/npmrc`), so the developer's `~/.npmrc` never reaches a scenario.
- **Project config.** npm reads the project `.npmrc` from the nearest directory holding a
  `package.json` or `node_modules`, so every consumer directory gets a minimal `package.json`
  first; without it the harness's own manifest would be the project.
