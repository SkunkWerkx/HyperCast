"""The v0.9 release's A (build source) -> B (staged bytes) checks. No rebuilding.

Only the generated paths already staged by stage-native-binaries.yml may change.
Build provenance names A; packages assembled by release.yml name B.
"""
import argparse
import json
import os
from pathlib import Path
import re
import subprocess
import sys
import tempfile

PHP = {rid: ('hypercast.dll' if rid.startswith('win') else
             'libhypercast.dylib' if rid.startswith('osx') else 'libhypercast.so')
       for rid in ('linux-x64', 'linux-arm64', 'linux-musl-x64', 'linux-musl-arm64',
                   'osx-x64', 'osx-arm64', 'win-x64')}
BUNDLE = 'swift/HyperCastCore.artifactbundle'
APPLE = 'swift/HyperCastCoreApple.xcframework'
GO_TARGETS = ('android_amd64 android_arm64 darwin_amd64 darwin_arm64 ios_arm64 '
              'iossimulator_arm64 linux_amd64 linux_arm64 maccatalyst_amd64 '
              'maccatalyst_arm64 wasm windows_amd64 windows_arm64').split()
SWIFT_TARGETS = ('aarch64-swift-linux-musl aarch64-unknown-linux-android '
                 'aarch64-unknown-linux-gnu aarch64-unknown-windows-msvc arm64-apple-macosx '
                 'wasm32-unknown-wasip1 x86_64-apple-macosx x86_64-swift-linux-musl '
                 'x86_64-unknown-linux-android x86_64-unknown-linux-gnu '
                 'x86_64-unknown-windows-msvc').split()
SLICES = ('ios-arm64', 'ios-arm64-simulator', 'ios-arm64-maccatalyst')


def command(args, cwd=None):
    return subprocess.check_output(args, cwd=cwd, text=True).strip()


def git(root, *args):
    return command(['git', *args], cwd=root)


def api(repo, endpoint):
    return json.loads(command(['gh', 'api', f'repos/{repo}/{endpoint}']))


def payload_paths(artifacts):
    paths = {f'php/src/native/{rid}/{name}': artifacts / 'native' / f'native-{rid}' / name
             for rid, name in PHP.items()}
    static = []
    for target in GO_TARGETS:
        static.append(f'go/staticlib/{target}/libhypercast.a')
    for target in SWIFT_TARGETS:
        name = 'hypercast.lib' if 'windows' in target else 'libhypercast.a'
        static.append(f'{BUNDLE}/{target}/{name}')
    static += [f'{BUNDLE}/info.json', f'{APPLE}/Info.plist']
    for target in SLICES:
        static.append(f'{APPLE}/{target}/libhypercast.a')
        static += [f'{APPLE}/{target}/Headers/HyperCastCore/{name}'
                   for name in ('hypercast.h', 'module.modulemap')]
    paths.update({path: artifacts / 'static-libs' / path for path in static})
    return paths


def require_master(root, sha):
    if git(root, 'rev-parse', 'origin/master') != sha:
        raise ValueError(f'master advanced beyond preparation source {sha}; prepare again')


def validate_run(run, sha, run_id, repo, completed=True):
    expected = {'id': int(run_id), 'head_sha': sha, 'head_branch': 'master',
                'path': '.github/workflows/ci.yml'}
    if completed:
        expected.update(status='completed', conclusion='success')
    for key, value in expected.items():
        if run.get(key) != value:
            raise ValueError(f'CI run {run_id}: expected {key}={value}, got {run.get(key)}')
    if run.get('event') not in ('workflow_dispatch', 'schedule'):
        raise ValueError('CI run must use the full release-mode matrix')
    if run.get('repository', {}).get('full_name', '').lower() != repo.lower():
        raise ValueError('CI run belongs to a different repository')


def version_at(root, sha):
    return re.search(r'^version = "([^"]+)"', git(root, 'show', f'{sha}:rust/Cargo.toml'), re.M)[1]


def release_record(root, commit, tag, override, repo):
    message = git(root, 'show', '-s', '--format=%B', commit)
    def trailer(key, pattern):
        values = re.findall(rf'^{key}: (.*)$', message, re.M)
        if len(values) != 1 or not re.fullmatch(pattern, values[0]):
            raise ValueError(f'staging commit requires exactly one {key} trailer')
        return values[0]
    sha = trailer('Release-Source-SHA', '[0-9a-f]{40}')
    run_id = trailer('Release-CI-Run-ID', '[1-9][0-9]*')
    if git(root, 'show', '-s', '--format=%P', commit) != sha:
        raise ValueError('staging commit B must have exactly one parent, preparation source A')
    if override and override != run_id:
        raise ValueError('ci_run_id must equal the run recorded by the staging commit')
    want = version_at(root, commit)
    if tag and tag != f'v{want}':
        raise ValueError(f'tag {tag} does not match version {want}')
    if version_at(root, sha) != want:
        raise ValueError('preparation and staging versions differ')
    validate_transition(root, sha, commit)
    validate_staged_tree(root, commit)
    validate_run(api(repo, f'actions/runs/{run_id}'), sha, run_id, repo)
    return sha, run_id



def validate_transition(root, sha, commit):
    allowed = payload_paths(Path('artifacts'))
    changed = git(root, 'diff', '--name-only', '--no-renames', sha, commit).splitlines()
    for path in changed:
        if path not in allowed:
            raise ValueError(f'unrelated A -> B change: {path}')
        entry = git(root, 'ls-tree', commit, '--', path).split()
        if not entry:
            raise ValueError(f'staged payload was removed: {path}')
        mode = entry[0]
        expected_mode = '100755' if path.startswith('php/') else '100644'
        if mode != expected_mode:
            raise ValueError(f'invalid staged file mode for {path}: {mode}')


def validate_staging_index(root, sha):
    if git(root, 'rev-parse', 'HEAD') != sha:
        raise ValueError('staging index is not based on preparation source A')
    tree = git(root, 'write-tree')
    validate_transition(root, sha, tree)
    validate_staged_tree(root, tree)
    compare_payloads(root, payload_paths(root / 'artifacts'), tree)


def validate_staged_tree(root, commit):
    """Validate every shipped path, including files unchanged by staging."""
    expected = payload_paths(Path('artifacts'))
    source_paths = {'php/src/native/README.md', 'go/staticlib/README.md',
                    f'{BUNDLE}/include/hypercast.h', f'{BUNDLE}/include/module.modulemap'}
    entries = {}
    for line in git(root, 'ls-tree', '-r', commit, '--', 'php/src/native', BUNDLE, APPLE,
                    'go/staticlib').splitlines():
        metadata, path = line.split('\t', 1)
        entries[path] = metadata.split()[0]
    for path in expected:
        if path not in entries:
            raise ValueError(f'missing staged payload: {path}')
        mode = '100755' if path.startswith('php/') else '100644'
        if entries[path] != mode:
            raise ValueError(f'invalid staged file mode for {path}: {entries[path]}')
    for path in entries:
        if path not in expected and path not in source_paths:
            raise ValueError(f'unexpected staged payload: {path}')


def verify_attestation(path, sha, repo):
    subprocess.run(['gh', 'attestation', 'verify', str(path), '--repo', repo,
                    '--signer-repo', 'SkunkWerkx/.github', '--source-digest', sha], check=True)


def compare_payloads(root, paths, tree=None):
    for dest, src in paths.items():
        if src.is_symlink() or (root / dest).is_symlink():
            raise ValueError(f'staged payload must be a regular file: {dest}')
        shipped = subprocess.check_output(['git', 'show', f'{tree}:{dest}'], cwd=root) if tree else (root / dest).read_bytes()
        if src.read_bytes() != shipped:
            raise ValueError(f'staged bytes do not match the recorded CI run: {dest}')


def core_exports(root):
    # Numeric exports are generated from a table; the version export lives in abi.rs.
    names = set()
    for relative in ('rust/src/ffi.rs', 'rust/src/abi.rs'):
        text = (root / relative).read_text()
        names.update(re.findall(r'pub extern "C" fn (\w+)', text))
        for table in re.findall(r'numeric_exports! \{(.*?)\n\}', text, re.S):
            names.update(re.findall(r'(\w+) => \(', table))
    names.discard('$export')
    if 'hypercast_version' not in names or len(names) < 2:
        raise ValueError('could not read the current C export set')
    return names


def check_symbols(path, expected):
    # LLVM understands ELF, Mach-O, COFF and WASM archives. GNU nm cannot read them all.
    if path.suffix == '.dll':
        output = command(['llvm-readobj', '--coff-exports', str(path)])
        symbols = re.findall(r'^  Name: (\S+)$', output, re.M)
    else:
        flags = ['--dynamic'] if path.suffix == '.so' else []
        output = command(['llvm-nm', *flags, '--defined-only', '--extern-only', '--format=posix', str(path)])
        symbols = [line.split()[0] for line in output.splitlines() if len(line.split()) >= 2]
    names = set()
    for symbol in symbols:
        name = symbol.removeprefix('_')  # Mach-O C symbol decoration
        if name.startswith(('cast_', 'hypercast_')):
            names.add(name)
    if names != expected:
        raise ValueError(f'{path}: missing exports {sorted(expected - names)}; stale exports {sorted(names - expected)}')


def check_runtime(root, payload, version):
    # Compile against the current declarations, including result codes and ABI structure layouts.
    # Each process loads/links just one payload, avoiding dynamic loader caching.
    packed = sum(int(n) << shift for n, shift in zip(version.split('.'), (16, 8, 0)))
    source = r'''
#include <stdint.h>
#include <string.h>
#include "hypercast.h"
int main(void) {
    uint32_t fault[2]={0,0}; uint8_t boolean=0; int32_t integer=0;
    if(hypercast_version()!=EXPECTED_VERSION) return 1;
    if(cast_bool((const uint8_t*)"yes",3,&boolean,fault)!=0 || boolean!=1) return 2;
    if(cast_bool((const uint8_t*)"maybe",5,&boolean,fault)!=2 || fault[0]!=0 || fault[1]!=5) return 3;
    if(cast_i32((const uint8_t*)"123",3,0,&integer,fault)!=0 || integer!=123) return 4;
    if(cast_i32((const uint8_t*)"",0,0,&integer,fault)!=1) return 5;
    return 0;
}
'''
    with tempfile.TemporaryDirectory() as tmp:
        c, exe = Path(tmp) / 'abi.c', Path(tmp) / 'abi'
        c.write_text(source)
        subprocess.run(['cc', f'-DEXPECTED_VERSION={packed}', '-I', str(root / BUNDLE / 'include'),
                        str(c), str(payload.resolve()), '-ldl', '-lpthread', '-lm', '-o', str(exe)], check=True)
        subprocess.run([str(exe)], check=True)


def validate_payloads(root, paths, sha, repo, compare=False, attest=True):
    exports = core_exports(root)
    version = re.search(r'^version = "([^"]+)"', (root / 'rust/Cargo.toml').read_text(), re.M)[1]
    # Check the complete explicit platform set, including generated metadata, before copying.
    for dest, src in paths.items():
        if not src.is_file() or src.is_symlink():
            raise ValueError(f'missing artifact payload: {src}')
        if src.suffix in ('.a', '.lib', '.so', '.dll', '.dylib'):
            if attest:
                verify_attestation(src, sha, repo)
            check_symbols(src, exports)
    for dest in ('php/src/native/linux-x64/libhypercast.so',
                 f'{BUNDLE}/x86_64-unknown-linux-gnu/libhypercast.a',
                 'go/staticlib/linux_amd64/libhypercast.a'):
        check_runtime(root, paths[dest], version)
    subprocess.run([sys.executable, str(root / '.github/scripts/cdylib_smoke.py'),
                    str(paths['php/src/native/linux-x64/libhypercast.so']),
                    str(root / 'rust/Cargo.toml')], check=True)
    if compare:
        compare_payloads(root, paths)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('mode', choices=('record', 'run', 'master', 'index', 'payloads', 'compare'))
    args = parser.parse_args()
    root = Path.cwd()
    repo, sha = os.environ['GITHUB_REPOSITORY'], os.environ.get('SOURCE_SHA', '')
    run_id = os.environ.get('RUN_ID', '')
    if args.mode == 'record':
        ref = os.environ.get('GITHUB_REF', '')
        tag = ref.removeprefix('refs/tags/') if ref.startswith('refs/tags/') else ''
        if not tag and os.environ.get('GITHUB_REF_TYPE') == 'tag':
            tag = os.environ['GITHUB_REF_NAME']
        sha, run_id = release_record(root, os.environ['GITHUB_SHA'], tag,
                                     os.environ.get('CI_RUN_ID_OVERRIDE', ''), repo)
        with open(os.environ['GITHUB_OUTPUT'], 'a') as output:
            output.write(f'source_sha={sha}\nrun_id={run_id}\n')
    elif args.mode == 'run':
        validate_run(api(repo, f'actions/runs/{run_id}'), sha, run_id, repo)
    elif args.mode == 'master':
        require_master(root, sha)
    elif args.mode == 'index':
        validate_staging_index(root, sha)
    else:
        # Staging's existing verify-and-copy step checks provenance once, after these
        # ABI checks. At release entry, verify it here while comparing the shipped bytes.
        validate_payloads(root, payload_paths(root / 'artifacts'), sha, repo,
                          compare=args.mode == 'compare', attest=args.mode == 'compare')


if __name__ == '__main__':
    main()
