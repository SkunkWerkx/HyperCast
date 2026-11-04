"""The TypeScript, R, Java, PHP, Go and Swift sites of check_exports.py: the spelling map, the
catch_unwind rule, the R shim, the JNI sites, the JNI no-panic attribute rule, the PHP guard
rule and the Swift calls.

Every napi function must carry `catch_unwind`, or a panic in the core aborts the Node
process instead of surfacing as a JavaScript error. The R site is pinned too: its shim is the
package's only ABI surface, so a site that reads nothing would hide a drift."""
import importlib.util
from pathlib import Path
import tempfile
import unittest

ROOT = Path(__file__).resolve().parents[3]


def load(name):
    spec = importlib.util.spec_from_file_location(name, ROOT / '.github/scripts' / f'{name}.py')
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module


class TypeScriptSiteTests(unittest.TestCase):
    def setUp(self):
        self.mod = load('check_exports')
        self.tmp = tempfile.TemporaryDirectory()
        self.addCleanup(self.tmp.cleanup)

    def source(self, text):
        path = Path(self.tmp.name) / 'lib.rs'
        path.write_text(text, encoding='utf-8')
        return path

    def test_napi_site_requires_catch_unwind(self):
        path = self.source('#[napi]\npub fn cast_bool() {}\n')
        with self.assertRaises(SystemExit) as caught:
            self.mod.napi_catch_unwind(path)
        self.assertIn('cast_bool', str(caught.exception))
        self.mod.napi_catch_unwind(self.source('#[napi(catch_unwind)]\npub fn cast_bool() {}\n'))

    def test_napi_catch_unwind_reads_an_attribute_over_several_lines(self):
        ok = '#[napi(\n    js_name = "castBool",\n    catch_unwind,\n)]\npub fn cast_bool() {}\n'
        self.mod.napi_catch_unwind(self.source(ok))
        with self.assertRaises(SystemExit):
            self.mod.napi_catch_unwind(self.source(ok.replace('    catch_unwind,\n', '')))

    def test_napi_catch_unwind_flags_a_function_with_other_options_only(self):
        with self.assertRaises(SystemExit):
            self.mod.napi_catch_unwind(self.source('#[napi(js_name = "x")]\n\npub fn cast_i32() {}\n'))

    def test_napi_catch_unwind_reads_a_macro_template(self):
        template = 'macro_rules! doors {\n    ($door:ident) => {\n        #[napi(catch_unwind)]\n        pub fn $door() {}\n    };\n}\n'
        self.mod.napi_catch_unwind(self.source(template))
        with self.assertRaises(SystemExit):
            self.mod.napi_catch_unwind(self.source(template.replace('(catch_unwind)', '')))

    def test_napi_catch_unwind_ignores_consts_structs_and_enums(self):
        self.mod.napi_catch_unwind(self.source(
            '#[napi]\npub const MAX: u32 = 1;\n#[napi(object)]\npub struct Fault { a: u32 }\n'
            '#[napi(string_enum)]\npub enum Kind { A }\n#[napi(catch_unwind)]\npub fn cast_bool() {}\n'))

    def test_napi_catch_unwind_takes_a_module_exports_hook_that_catches_its_own(self):
        hook = ('#[napi(module_exports)]\npub fn init(mut exports: Object, env: Env) -> Result<()> {\n'
                '    std::panic::catch_unwind(AssertUnwindSafe(|| install(&mut exports, env)))\n'
                '        .unwrap_or_else(|_| Err(bug()))\n}\n')
        self.mod.napi_catch_unwind(self.source(hook))
        bare = '#[napi(module_exports)]\npub fn init(mut exports: Object, env: Env) -> Result<()> {\n    install(&mut exports, env)\n}\n'
        with self.assertRaises(SystemExit) as caught:
            self.mod.napi_catch_unwind(self.source(bare))
        self.assertIn('init', str(caught.exception))
        # Named only in a comment or a string, it catches nothing.
        for fake in ('    // catch_unwind(...) would go here\n', '    let _ = "catch_unwind(";\n'):
            with self.assertRaises(SystemExit):
                self.mod.napi_catch_unwind(self.source(bare.replace('{\n', '{\n' + fake, 1)))
        # The carve-out is the hook alone: an ordinary napi function still needs the option.
        with self.assertRaises(SystemExit):
            self.mod.napi_catch_unwind(self.source(hook.replace('(module_exports)', '(js_name = "init")')))

    def test_napi_catch_unwind_holds_on_the_addon(self):
        self.mod.napi_catch_unwind('typescript/src/lib.rs')

    def test_ts_spelling(self):
        self.assertIsNone(self.mod.ts_spelling('cast_date_ordered'))
        self.assertEqual(self.mod.ts_spelling('hypercast_version'), 'native_version')
        self.assertEqual(self.mod.ts_camel('cast_decimal_from_f64'), 'castDecimalFromNumber')
        self.assertEqual(self.mod.ts_camel('cast_excel_serial_from_f64'), 'castExcelSerialFromNumber')

    def test_camel(self):
        self.assertEqual(self.mod.camel('cast_excel_duration'), 'castExcelDuration')
        self.assertEqual(self.mod.camel('native_version'), 'nativeVersion')

    def test_the_site_is_the_generated_declarations(self):
        site = next(s for s in self.mod.SITES if s[0] == 'TypeScript (binding.d.ts)')
        self.assertEqual([g for g, _ in site[1]], ['typescript/binding.d.ts'])
        self.assertIs(site[2], self.mod.ts_camel)
        expected = {self.mod.ts_camel(e) for e in self.mod.core_exports()} - {None}
        self.assertEqual(self.mod.declared(site[1]), expected)


def required(mod, got):
    """Every core export a site that declares `got` must declare: all of them but the 1.0
    exports superseded by a detailed export it calls."""
    return mod.core_exports() - mod.superseded(str, got)


class RSiteTests(unittest.TestCase):
    def test_r_shim_site_reads_every_required_core_export(self):
        mod = load('check_exports')
        site = next(s for s in mod.SITES if s[0] == 'R (C shim)')
        self.assertEqual([g for g, _ in site[1]], ['r/src/hypercast.c'])
        self.assertEqual(site[2:], ('all', ()))
        text = (ROOT / 'r/src/hypercast.c').read_text(encoding='utf-8')
        got = set(site[1][0][1](text))
        self.assertTrue(got <= mod.core_exports(), got - mod.core_exports())
        self.assertEqual(required(mod, got) - got, set())
        self.assertTrue(mod.DETAILED <= got)

    def test_r_shim_comment_is_not_a_call(self):
        mod = load('check_exports')
        site = next(s for s in mod.SITES if s[0] == 'R (C shim)')
        calls = site[1][0][1]
        self.assertEqual(calls('/* cast_bool(x) */ cast_i16(y);\n// hypercast_version()\n'), ['cast_i16'])
        self.assertEqual(calls('Rf_error("cast_uuid(a) failed"); cast_text(p, n, &o, &f);'), ['cast_text'])


class JavaSiteTests(unittest.TestCase):
    def setUp(self):
        self.mod = load('check_exports')

    def test_java_sites_are_the_ffm_tree_and_the_four_jni_sites(self):
        sites = {s[0]: s for s in self.mod.SITES if s[0].startswith('Java')}
        self.assertEqual(
            list(sites),
            ['Java (FFM)', 'Java (JNI natives)', 'Java (JNI table)', 'Java (JNI no-panic)', 'Java (JNI imports)'],
        )
        base = 'java/src/main/java/io/github/skunkwerkx/hypercast/NativeCore.java'
        ffm = 'java/src/main/java25/io/github/skunkwerkx/hypercast/NativeCore.java'
        self.assertEqual([g for g, _ in sites['Java (FFM)'][1]], [ffm])
        self.assertEqual([g for g, _ in sites['Java (JNI natives)'][1]], [base])
        self.assertEqual([g for g, _ in sites['Java (JNI table)'][1]], ['java/jni/src/lib.rs'])
        self.assertEqual([g for g, _ in sites['Java (JNI no-panic)'][1]], ['java/jni/examples/no_panic.rs'])
        self.assertEqual([g for g, _ in sites['Java (JNI imports)'][1]], ['java/jni/src/lib.rs'])
        self.assertIs(sites['Java (JNI natives)'][2], self.mod.camel)
        self.assertIs(sites['Java (JNI table)'][2], self.mod.camel)
        self.assertEqual(sites['Java (JNI natives)'][3], ('jniNatives',))
        self.assertEqual(sites['Java (JNI table)'][3], ('jniNatives',))
        self.assertEqual(sites['Java (JNI no-panic)'][2:], ('all', ('jni_natives', 'JNI_OnLoad')))
        self.assertEqual(sites['Java (JNI imports)'][2:], ('all', ()))
        self.assertEqual(self.mod.UNIQUE_SITES, ('Java (JNI table)',))

    def test_jni_table_site_rejects_a_repeated_name(self):
        import contextlib
        import io
        import shutil
        mod = self.mod
        table = next(s for s in mod.SITES if s[0] == 'Java (JNI table)')
        exports = mod.core_exports()
        with tempfile.TemporaryDirectory() as tmp:
            lib = Path(tmp) / 'java/jni/src/lib.rs'
            lib.parent.mkdir(parents=True)
            shutil.copy(ROOT / 'java/jni/src/lib.rs', lib)
            text = lib.read_text(encoding='utf-8')
            line = '    native!("castBool", "([B[J)I", native_cast_bool),\n'
            self.assertIn(line, text)
            lib.write_text(text.replace(line, line + line), encoding='utf-8')
            saved = (mod.ROOT, mod.SITES, mod.core_exports, mod.napi_catch_unwind, mod.php_guard)
            self.addCleanup(lambda: (setattr(mod, 'ROOT', saved[0]), setattr(mod, 'SITES', saved[1]),
                                     setattr(mod, 'core_exports', saved[2]),
                                     setattr(mod, 'napi_catch_unwind', saved[3]),
                                     setattr(mod, 'php_guard', saved[4])))
            mod.ROOT, mod.SITES = Path(tmp), [table]
            mod.core_exports = lambda: exports
            mod.napi_catch_unwind = lambda path: None
            mod.php_guard = lambda path: None
            self.addCleanup(setattr, mod, 'php_link', mod.php_link)
            mod.php_link = lambda path: None
            out = io.StringIO()
            with contextlib.redirect_stdout(out), self.assertRaises(SystemExit) as caught:
                mod.main()
        self.assertEqual(caught.exception.code, 1)
        self.assertIn('repeats castBool', out.getvalue())

    def test_jni_import_site_ignores_native_fns(self):
        import re
        pattern = next(p for b, s, _, _ in self.mod.SITES if b == 'Java (JNI imports)' for _, p in s)
        self.assertEqual(re.findall(pattern, 'pub unsafe extern "system" fn native_cast_bool('), [])
        self.assertEqual(re.findall(pattern, '    fn cast_bool(ptr: *const u8, len: usize) -> i32;'), ['cast_bool'])

    def test_jni_no_panic_requires_the_attribute_on_every_jni_fn(self):
        attr = '#[cfg_attr(feature = "no-panic", no_panic::no_panic)]\n'
        with tempfile.TemporaryDirectory() as tmp:
            path = Path(tmp) / 'lib.rs'
            path.write_text('#[unsafe(no_mangle)]\n' + attr + 'pub unsafe extern "system" fn JNI_OnLoad() {}\n'
                            + attr + 'pub unsafe extern "system" fn native_cast_bool() {}\n', encoding='utf-8')
            self.mod.jni_no_panic(path)
            path.write_text(attr + 'pub unsafe extern "system" fn native_cast_bool() {}\n'
                            '#[unsafe(no_mangle)]\npub unsafe extern "system" fn JNI_OnLoad() {}\n', encoding='utf-8')
            with self.assertRaisesRegex(SystemExit, 'JNI_OnLoad'):
                self.mod.jni_no_panic(path)

    def test_jni_no_panic_holds_on_the_shim(self):
        self.mod.jni_no_panic('java/jni/src/lib.rs')


class PhpLinkTests(unittest.TestCase):
    """The PHP extension's Windows link rule: an extern block that names the engine's import
    library names php8 on an NTS PHP and php8ts on a TS one, chosen by `php_zts`, which build.rs
    sets from the same PHP ext-php-rs builds against."""

    NTS = '#[cfg_attr(all(windows, not(php_zts)), link(name = "php8"))]\n'
    TS = '#[cfg_attr(all(windows, php_zts), link(name = "php8ts"))]\n'
    BLOCK = 'unsafe extern "C" {\n    static spl_ce_RuntimeException: *mut c_void;\n}\n'

    def setUp(self):
        self.mod = load('check_exports')
        self.tmp = tempfile.TemporaryDirectory()
        self.addCleanup(self.tmp.cleanup)

    def source(self, text):
        (Path(self.tmp.name) / 'a.rs').write_text(text, encoding='utf-8')
        return self.tmp.name

    def test_the_pair_passes(self):
        self.mod.php_link(self.source('use x;\n\n' + self.NTS + self.TS + self.BLOCK + '\n#[cfg(windows)]\n'
                                      + self.NTS + self.TS + 'unsafe extern "vectorcall" {\n}\n'))

    def test_php8_alone_is_named_with_its_line(self):
        # What every block said before ZTS: a TS PHP has no php8.lib to link and no php8.dll to
        # load the DLL against.
        for bare in ('#[cfg_attr(windows, link(name = "php8"))]\n', '#[link(name = "php8")]\n', self.NTS, self.TS):
            with self.subTest(bare=bare), self.assertRaisesRegex(SystemExit, r'a\.rs:3'):
                self.mod.php_link(self.source('use x;\n\n' + bare + self.BLOCK))

    def test_the_pair_out_of_order_or_apart_is_named(self):
        for text in (self.TS + self.NTS + self.BLOCK, self.NTS + '\n' + self.TS + self.BLOCK,
                     self.NTS + self.NTS + self.TS + self.BLOCK):
            with self.subTest(text=text), self.assertRaisesRegex(SystemExit, r'a\.rs:1'):
                self.mod.php_link(self.source(text))

    def test_the_pair_must_open_an_extern_block(self):
        with self.assertRaisesRegex(SystemExit, r'a\.rs:1'):
            self.mod.php_link(self.source(self.NTS + self.TS + 'fn f() {}\n'))
        self.mod.php_link(self.source(self.NTS + self.TS + '#[cfg(windows)]\n' + self.BLOCK))

    def test_another_library_is_not_this_rules(self):
        self.mod.php_link(self.source('#[link(name = "kernel32")]\n' + self.BLOCK))

    def test_a_comment_naming_php8_is_not_an_attribute(self):
        self.mod.php_link(self.source('// #[link(name = "php8")] was the NTS-only spelling\n' + self.BLOCK))

    def test_the_real_extension_holds(self):
        self.mod.php_link('rust/src/php_ext')

    def test_build_rs_sets_the_cfg_from_the_php_ext_php_rs_asks(self):
        build = (ROOT / 'rust/build.rs').read_text(encoding='utf-8')
        self.assertIn('ext_php_rs_build::find_php()', build)
        self.assertIn('.thread_safety()', build)
        self.assertIn('cargo:rustc-cfg=php_zts', build)
        self.assertIn('cargo::rustc-check-cfg=cfg(php_zts)', build)
        manifest = (ROOT / 'rust/Cargo.toml').read_text(encoding='utf-8')
        self.assertRegex(manifest, r'(?m)^ext-php-rs-build = \{ version = "[^"]+", optional = true \}$')
        self.assertRegex(manifest, r'(?m)^php = \[[^\]]*"dep:ext-php-rs-build"')


class PhpGuardTests(unittest.TestCase):
    """The PHP extension's panic rule: every PHP-callable fn starts with `guard(`."""

    def setUp(self):
        self.mod = load('check_exports')
        self.tmp = tempfile.TemporaryDirectory()
        self.addCleanup(self.tmp.cleanup)

    def source(self, text):
        (Path(self.tmp.name) / 'a.rs').write_text(text, encoding='utf-8')
        return self.tmp.name

    GUARDED = (
        '#[php_impl]\nimpl U {\n    const X: i64 = 1;\n'
        '    #[php(name = "castBool")]\n    pub fn cast_bool(\n        unixMillis: i64,\n    ) -> PhpResult<U> {\n'
        '        guard(|| { let s = "}"; Ok(U) })\n    }\n}\n'
    )

    def test_an_unguarded_method_is_named(self):
        text = self.GUARDED.replace('}\n}\n', '}\n    pub fn bare(&self) -> i64 { 1 }\n}\n')
        with self.assertRaises(SystemExit) as caught:
            self.mod.php_guard(self.source(text))
        self.assertIn('php_ext has PHP-callable functions without guard: bare', str(caught.exception))
        self.assertNotIn('cast_bool', str(caught.exception))

    def test_all_guarded_passes(self):
        self.mod.php_guard(self.source(self.GUARDED))

    def test_a_php_function_needs_guard_too(self):
        with self.assertRaisesRegex(SystemExit, 'loose'):
            self.mod.php_guard(self.source('#[php_function]\npub fn loose() -> i64 { 1 }\n'))
        self.mod.php_guard(self.source('#[php_function]\npub fn ok() -> PhpResult<i64> { guard(|| Ok(1)) }\n'))

    def test_a_modifier_handler_must_call_guard(self):
        text = (
            '#[php(modifier = add)]\nstruct S;\n'
            'fn add(b: B) -> B { b.method(FunctionBuilder::new("__unserialize", handler), F) }\n'
            'zend_fastcall! {\n    extern "C" fn handler(ex: &mut E) {\n        %s\n    }\n}\n'
        )
        with self.assertRaisesRegex(SystemExit, 'handler'):
            self.mod.php_guard(self.source(text % 'run_handler(|| work(ex));'))
        self.mod.php_guard(self.source(text % 'run_handler(|| { let _ = guard(|| work(ex)); });'))

    def test_a_lifecycle_callback_must_catch_unwinds(self):
        text = (
            self.GUARDED
            + 'extern "C" fn request_shutdown(_t: i32, _m: i32) -> i32 {\n    %s\n    0\n}\n'
            + 'static CB: Option<unsafe extern "C" fn() -> i32> = None;\n'
        )
        with self.assertRaisesRegex(
            SystemExit, 'extern "C" functions without catch_unwind or run_handler: request_shutdown$'
        ):
            self.mod.php_guard(self.source(text % 'release();'))
        self.mod.php_guard(self.source(text % 'let _ = catch_unwind(release);'))
        self.mod.php_guard(self.source(text % 'run_handler(AssertUnwindSafe(|| release()));'))

    def test_a_return_type_with_a_semicolon_is_not_a_declaration(self):
        bare = '#[php_impl]\nimpl U {\n    pub fn b(&self) -> [u8; 16] { self.x }\n}\n'
        with self.assertRaisesRegex(SystemExit, 'without guard: b$'):
            self.mod.php_guard(self.source(bare))
        with self.assertRaisesRegex(SystemExit, 'without guard: c$'):
            self.mod.php_guard(self.source(
                '#[php_impl]\nimpl U {\n    pub fn c() -> PhpResult<Option<[u8; 4]>> { Ok(None) }\n}\n'))
        self.mod.php_guard(self.source(
            '#[php_impl]\nimpl U {\n    pub fn b(&self) -> PhpResult<[u8; 16]> { guard(|| Ok(self.x)) }\n}\n'))

    def test_a_fn_without_a_body_in_an_impl_fails(self):
        with self.assertRaisesRegex(SystemExit, 'without guard: d$'):
            self.mod.php_guard(self.source('#[php_impl]\nimpl U {\n    pub fn d();\n}\n'))

    def test_no_callable_functions_at_all_fails(self):
        with self.assertRaisesRegex(SystemExit, 'php_ext has no PHP-callable functions to check'):
            self.mod.php_guard(self.source('fn helper() {}\n#[php_class]\nstruct S;\n'))

    def test_the_real_extension_holds(self):
        self.mod.php_guard('rust/src/php_ext')

    def test_main_runs_the_rule(self):
        import contextlib
        import io
        mod = self.mod
        calls = []
        saved = (mod.SITES, mod.php_guard)
        self.addCleanup(lambda: (setattr(mod, 'SITES', saved[0]), setattr(mod, 'php_guard', saved[1])))
        mod.SITES = []
        mod.php_guard = calls.append
        with contextlib.redirect_stdout(io.StringIO()), self.assertRaises(SystemExit):
            mod.main()
        self.assertEqual(calls, ['rust/src/php_ext'])

    def test_the_ffi_site_is_gone(self):
        self.assertNotIn('PHP (FFI cdef)', [s[0] for s in self.mod.SITES])


class GoSiteTests(unittest.TestCase):
    """The two Go sites read the translated core (every export is a method on the generated
    Module) and the binding's calls into it (every export is called). The `(?:cast|hypercast)_`
    prefix is load-bearing: wasm2go also emits Xmemory, which a bare X(\\w+) would report as a
    stale export."""

    def setUp(self):
        import re
        self.re = re
        self.mod = load('check_exports')
        self.core = next(s for s in self.mod.SITES if s[0] == 'Go (translated core)')
        self.calls = next(s for s in self.mod.SITES if s[0] == 'Go (binding calls)')

    def test_the_go_sites_are_the_generated_core_and_the_binding(self):
        self.assertEqual(self.core[1:], ([('go/internal/core/core.go', r'func \(m \*Module\) X((?:cast|hypercast)_\w+)\(')], 'all', ()))
        self.assertEqual([g for g, _ in self.calls[1]], ['go/core.go'])
        self.assertEqual(self.calls[1][0][1].pattern, r'\.X((?:cast|hypercast)_\w+)\b')
        self.assertEqual(self.calls[2:], ('all', ()))
        self.assertEqual(self.mod.declared(self.core[1]), self.mod.core_exports(), self.core[0])
        got = self.mod.declared(self.calls[1])
        self.assertEqual(required(self.mod, got) - got, set(), self.calls[0])

    def found(self, site, text):
        pattern = site[1][0][1]
        if callable(pattern):
            return set(pattern(text))
        return set(self.re.findall(pattern, text, self.re.MULTILINE))

    def test_a_call_the_binding_drops_is_missing(self):
        text = (ROOT / 'go/core.go').read_text(encoding='utf-8')
        self.assertIn('.Xcast_bool\n', text)
        whole = self.found(self.calls, text)
        got = self.found(self.calls, text.replace('.Xcast_bool\n', '.Xnot_an_export\n'))
        self.assertEqual(whole - got, {'cast_bool'})
        # A comment naming it is not a call either.
        got = self.found(self.calls, text.replace('.Xcast_bool\n', '.Xnot_an_export // .Xcast_bool\n'))
        self.assertEqual(whole - got, {'cast_bool'})

    def test_an_export_the_core_grows_is_stale(self):
        text = (ROOT / 'go/internal/core/core.go').read_text(encoding='utf-8')
        got = self.found(self.core, text + '\nfunc (m *Module) Xcast_bogus(v0 int32) int32 {\n\treturn 0\n}\n')
        self.assertEqual(got - self.mod.core_exports(), {'cast_bogus'})

    def test_the_memory_accessor_is_not_an_export(self):
        text = (ROOT / 'go/internal/core/core.go').read_text(encoding='utf-8')
        self.assertIn('func (m *Module) Xmemory() Memory {', text)
        self.assertFalse({'memory', 'Xmemory'} & self.found(self.core, text))

    def test_the_cgo_sites_are_gone(self):
        names = [s[0] for s in self.mod.SITES]
        self.assertNotIn('Go (cgo)', names)
        self.assertNotIn('Go (TinyGo)', names)


class SwiftSiteTests(unittest.TestCase):
    """Swift reaches the C exports through the HyperCastCore module, so its site is the calls
    in swift/Sources/HyperCast: every export called, or passed as a function, in code. A
    comment naming an export, whole-line or trailing, is not a call."""

    def setUp(self):
        import re
        self.re = re
        self.mod = load('check_exports')
        self.site = next(s for s in self.mod.SITES if s[0] == 'Swift (HyperCastCore calls)')

    def calls(self, text):
        return self.mod.swift_calls(text)

    def test_the_site_is_the_swift_sources_and_reads_every_export(self):
        self.assertEqual(self.site[1:], ([('swift/Sources/HyperCast/*.swift', self.mod.swift_calls)], 'all', ()))
        # Swift still calls every 1.0 export (its legacy table), so nothing is superseded here.
        self.assertEqual(self.mod.declared(self.site[1]), self.mod.core_exports())

    def test_a_call_or_a_function_passed_counts(self):
        self.assertEqual(self.calls('        uuid: { cast_uuid($0, $1, .init(OpaquePointer($2))) },'), ['cast_uuid'])
        self.assertEqual(self.calls('                cast_text('), ['cast_text'])
        self.assertEqual(self.calls('        version: hypercast_version)'), ['hypercast_version'])

    def test_a_comment_is_not_a_call(self):
        self.assertEqual(self.calls('    /// the library\'s own answer (`hypercast_version()`), not this'), [])
        self.assertEqual(self.calls('// through cast_checked(…) and cast_text(…)'), [])
        self.assertEqual(self.calls('        uuid: { _, _, _, _ in 0 },  // cast_uuid($0, $1) once called here'), [])
        self.assertEqual(self.calls('// cast_datetime: the unix shape'), [])

    def test_every_call_on_a_line_counts(self):
        self.assertEqual(self.calls('        let a = cast_bool(x), b = cast_i32(y)'), ['cast_bool', 'cast_i32'])
        self.assertEqual(self.calls('    f(cast_text(s), hypercast_version)'), ['cast_text', 'hypercast_version'])

    def test_a_string_is_not_code(self):
        # `//` inside a string literal is not a comment, so the call after it counts.
        self.assertEqual(self.calls('        let url = "https://example.org"; cast_uuid(a, b)'), ['cast_uuid'])
        self.assertEqual(self.calls('        let esc = "a \\" // b"; cast_bool(x)'), ['cast_bool'])
        self.assertEqual(self.calls('        let raw = #"say "//" twice"#; cast_i64(x)'), ['cast_i64'])
        # An export named inside a string is not a call.
        self.assertEqual(self.calls('        fatalError("cast_uuid(a) failed")'), [])
        self.assertEqual(self.calls('        let raw = #"cast_bool(x)"#'), [])
        self.assertEqual(self.calls('        let doc = """\n    cast_text(x)\n    """\n    cast_i8(y)'), ['cast_i8'])

    def test_a_block_comment_is_not_a_call(self):
        self.assertEqual(self.calls('/* cast_bool(x) */ cast_i16(y)'), ['cast_i16'])
        self.assertEqual(self.calls('/*\n cast_bool(x)\n*/'), [])

    def test_a_dropped_call_fails_the_check(self):
        import contextlib
        import io
        import shutil
        mod = self.mod
        exports = mod.core_exports()
        with tempfile.TemporaryDirectory() as tmp:
            sources = Path(tmp) / 'swift/Sources/HyperCast'
            shutil.copytree(ROOT / 'swift/Sources/HyperCast', sources)
            # cast_declared_decimal: no detailed export supersedes it, so dropping its one call
            # (leaving it named in a comment) fails.
            cast = sources / 'Constraints.swift'
            text = cast.read_text(encoding='utf-8')
            line = ('            cast_declared_decimal(Cast.inputPointer(utf8), UInt(utf8.count), '
                    '.init(OpaquePointer(out.baseAddress)))\n')
            self.assertIn(line, text)
            cast.write_text(text.replace(line, '            Int32(0)  // cast_declared_decimal(utf8)\n'),
                            encoding='utf-8')
            saved = (mod.ROOT, mod.SITES, mod.core_exports, mod.napi_catch_unwind, mod.php_guard,
                     mod.jni_no_panic)
            self.addCleanup(lambda: (setattr(mod, 'ROOT', saved[0]), setattr(mod, 'SITES', saved[1]),
                                     setattr(mod, 'core_exports', saved[2]),
                                     setattr(mod, 'napi_catch_unwind', saved[3]),
                                     setattr(mod, 'php_guard', saved[4]),
                                     setattr(mod, 'jni_no_panic', saved[5])))
            mod.ROOT, mod.SITES = Path(tmp), [self.site]
            mod.core_exports = lambda: exports
            self.addCleanup(setattr, mod, 'php_link', mod.php_link)
            mod.napi_catch_unwind = mod.php_guard = mod.php_link = mod.jni_no_panic = lambda path: None
            out = io.StringIO()
            with contextlib.redirect_stdout(out), self.assertRaises(SystemExit) as caught:
                mod.main()
        self.assertEqual(caught.exception.code, 1)
        self.assertIn('missing cast_declared_decimal', out.getvalue())


class SupersedesTests(unittest.TestCase):
    """The one allowance every site shares: a site that calls a 1.1 detailed export may omit
    the 1.0 exports that export reaches by door code, and must still declare the rest."""

    KEPT = {'hypercast_version', 'cast_constraints_check', 'cast_declared_decimal'}

    def setUp(self):
        self.mod = load('check_exports')

    def test_every_export_is_superseded_kept_or_detailed(self):
        mod = self.mod
        olds = [name for names in mod.SUPERSEDES.values() for name in names]
        self.assertEqual(len(olds), len(set(olds)))
        self.assertEqual(mod.DETAILED, {'cast_detailed', 'cast_detailed_from_f64', 'cast_checked_detailed',
                                        'cast_checked_detailed_from_f64'})
        self.assertEqual(set(olds) | mod.DETAILED | self.KEPT, mod.core_exports())
        self.assertFalse(set(olds) & (mod.DETAILED | self.KEPT))
        self.assertEqual(len(mod.SUPERSEDES['cast_detailed']), 23)  # Door's codes 1 to 23

    def test_only_a_called_detailed_export_supersedes(self):
        mod = self.mod
        self.assertEqual(mod.superseded(str, {'cast_detailed'}), set(mod.SUPERSEDES['cast_detailed']))
        self.assertEqual(mod.superseded(str, {'cast_checked_detailed'}), {'cast_checked'})
        self.assertEqual(mod.superseded(str, {'cast_i32', 'cast_checked'}), set())
        self.assertEqual(mod.superseded(mod.camel, {'castDetailedFromF64'}),
                         {'castDecimalFromF64', 'castExcelSerialFromF64', 'castExcelTime', 'castExcelDuration'})

    def test_a_folded_detailed_export_supersedes_nothing(self):
        mod = self.mod
        for spelling in (mod.pyo3_spelling, mod.ruby_spelling):
            for name in mod.DETAILED:
                self.assertIsNone(spelling(name))
            self.assertEqual(mod.superseded(spelling, {spelling(n) for n in mod.core_exports()}), set())
        self.assertEqual(mod.ruby_spelling('cast_decimal_from_f64'), 'decimal_from_float')
        self.assertEqual(mod.pyo3_spelling('hypercast_version'), 'native_version')

    def run_site(self, text, supersedes=None):
        import contextlib
        import io
        mod = self.mod
        exports = mod.core_exports()
        with tempfile.TemporaryDirectory() as tmp:
            (Path(tmp) / 'site.c').write_text(text, encoding='utf-8')
            saved = {name: getattr(mod, name) for name in
                     ('ROOT', 'SITES', 'core_exports', 'napi_catch_unwind', 'php_guard', 'php_link',
                      'jni_no_panic', 'SUPERSEDES', 'DETAILED')}
            self.addCleanup(lambda: [setattr(mod, k, v) for k, v in saved.items()])
            mod.ROOT = Path(tmp)
            mod.SITES = [('site', [('site.c', mod.code_only(r'\b((?:cast|hypercast)_\w+)\('))], 'all', ())]
            mod.core_exports = lambda: exports
            mod.napi_catch_unwind = mod.php_guard = mod.php_link = mod.jni_no_panic = lambda path: None
            if supersedes is not None:
                mod.SUPERSEDES, mod.DETAILED = supersedes, set(supersedes)
            out = io.StringIO()
            with contextlib.redirect_stdout(out), self.assertRaises(SystemExit) as caught:
                mod.main()
        return caught.exception.code, out.getvalue()

    def calls(self, names):
        return ''.join(f'{name}();\n' for name in sorted(names))

    def test_a_site_calling_the_detailed_exports_and_the_kept_ones_passes(self):
        code, out = self.run_site(self.calls(self.mod.DETAILED | self.KEPT))
        self.assertEqual(code, 0, out)
        self.assertIn('ok   site: 7 (29 1.0 exports superseded by the detailed ones)', out)

    def test_a_site_calling_everything_passes(self):
        code, out = self.run_site(self.calls(self.mod.core_exports()))
        self.assertEqual(code, 0, out)
        self.assertIn('ok   site: 36\n', out)

    def test_a_kept_export_is_never_superseded(self):
        code, out = self.run_site(self.calls((self.mod.DETAILED | self.KEPT) - {'cast_constraints_check'}))
        self.assertEqual(code, 1)
        self.assertIn('missing cast_constraints_check', out)

    def test_without_its_detailed_export_a_dropped_door_is_missing(self):
        names = self.mod.DETAILED | self.KEPT | set(self.mod.SUPERSEDES['cast_checked_detailed_from_f64'])
        code, out = self.run_site(self.calls(names - {'cast_checked_detailed_from_f64', 'cast_checked_from_f64'}))
        self.assertEqual(code, 1)
        self.assertIn('missing cast_checked_detailed_from_f64\n', out)
        self.assertIn('missing cast_checked_from_f64\n', out)
        self.assertNotIn('missing cast_i32', out)

    def test_a_comment_is_not_a_call(self):
        code, out = self.run_site(self.calls((self.mod.DETAILED | self.KEPT) - {'cast_detailed'})
                                  + '// cast_detailed(door, ...)\n/* cast_detailed() */\n')
        self.assertEqual(code, 1)
        self.assertIn('missing cast_detailed\n', out)
        self.assertIn('missing cast_i32\n', out)

    def test_a_table_naming_no_export_fails(self):
        table = dict(self.mod.SUPERSEDES, cast_detailed=('cast_bogus',))
        code, _ = self.run_site(self.calls(self.mod.core_exports()), supersedes=table)
        self.assertIn('cast_bogus', str(code))


if __name__ == '__main__':
    unittest.main()
