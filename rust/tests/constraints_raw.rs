//! Declarations to and from their C ABI form: what the packed form cannot carry is refused at
//! declaration, `Constraints::from_raw` refuses exactly what `check_constraints` refuses, and
//! `Constraints::pack_enum` writes the form both read.

use hypercast::{
    Constraints, Decimal, Door, Keyword, NumFormat, Numeric, NumericRules, RawConstraints, Refusal,
    Rejection, Text, TextRules, cast_checked, check_constraints,
};

/// `len` zeroed bytes as a `&'static str`. NUL is valid UTF-8 and not `White_Space`. Panics
/// if the allocator refuses: a test that cannot build its input fails, never passes.
///
/// Only glibc's `calloc` maps an allocation this size lazily, reads of it mapping the shared
/// zero page, so the 4 GiB entries below cost address space rather than memory only on Linux.
/// macOS's allocator faults in every page (4.3 GB resident, measured), Windows commits it, and
/// under qemu-user (s390x) the checker's two UTF-8 scans of 4 GiB are emulated: there the tests
/// are ignored, with that reason.
#[cfg(target_pointer_width = "64")]
fn zeroed(len: usize) -> (&'static str, std::alloc::Layout) {
    let layout = std::alloc::Layout::array::<u8>(len).expect("a layout");
    // SAFETY: the layout has a non-zero size.
    let ptr = unsafe { std::alloc::alloc_zeroed(layout) };
    assert!(!ptr.is_null(), "the allocator refused {len} zeroed bytes");
    // SAFETY: `ptr` is valid for `len` initialized (zeroed) bytes until `free`.
    let bytes = unsafe { std::slice::from_raw_parts(ptr, len) };
    (core::str::from_utf8(bytes).expect("zeros are UTF-8"), layout)
}

#[cfg(target_pointer_width = "64")]
fn free(text: &'static str, layout: std::alloc::Layout) {
    // SAFETY: allocated by `zeroed` with this layout, and no borrow outlives this call.
    unsafe { std::alloc::dealloc(text.as_ptr().cast_mut(), layout) }
}

#[cfg(target_pointer_width = "64")]
#[test]
#[cfg_attr(
    any(not(target_os = "linux"), target_arch = "s390x"),
    ignore = "4 GiB entries cost real memory or emulated time off native Linux (see `zeroed`)"
)]
fn text_entry_past_u32_is_not_held() {
    let (big, layout) = zeroed(u32::MAX as usize + 1);
    let entries = [big];
    let refused = Constraints::text(TextRules::new(), Some(&entries)).map(|_| ());
    free(big, layout);
    assert_eq!(refused, Err(Refusal::NotHeldByDoor));
}

#[cfg(target_pointer_width = "64")]
#[test]
#[cfg_attr(
    any(not(target_os = "linux"), target_arch = "s390x"),
    ignore = "4 GiB entries cost real memory or emulated time off native Linux (see `zeroed`)"
)]
fn text_entry_at_u32_is_held() {
    let (big, layout) = zeroed(u32::MAX as usize);
    let entries = [big];
    let declared = Constraints::text(TextRules::new(), Some(&entries)).map(|_| ());
    free(big, layout);
    assert_eq!(declared, Ok(()));
}

/// Entries packed as the C ABI passes text: a little-endian `u32` length, then the bytes.
fn packed_text(entries: &[&str]) -> Vec<u8> {
    let mut packed = Vec::new();
    for entry in entries {
        packed.extend_from_slice(&(entry.len() as u32).to_le_bytes());
        packed.extend_from_slice(entry.as_bytes());
    }
    packed
}

fn raw(flags: u32) -> RawConstraints {
    RawConstraints { flags, ..RawConstraints::ZERO }
}

/// `from_raw` at `T`'s door answers what `check_constraints` answers there.
fn same_as_check<T: Numeric>(raw: RawConstraints, entries: &[u8]) -> Result<(), Refusal> {
    let from_raw = Constraints::<T>::from_raw(raw, entries).map(drop);
    assert_eq!(from_raw, check_constraints(T::DOOR, &raw, entries), "{raw:?} {entries:?}");
    from_raw
}

fn same_as_check_text(raw: RawConstraints, entries: &[u8]) -> Result<(), Refusal> {
    let from_raw = Constraints::<Text>::from_raw(raw, entries).map(drop);
    assert_eq!(from_raw, check_constraints(Door::Text, &raw, entries), "{raw:?} {entries:?}");
    from_raw
}

/// An empty `enum` is a valid declaration that no value satisfies (JSON Schema's "SHOULD
/// have at least one element").
#[test]
fn from_raw_enum_flag_without_entries_declares() {
    let listed = raw(RawConstraints::HAS_ENUM);
    assert_eq!(same_as_check::<i32>(listed, &[]), Ok(()));
    assert_eq!(same_as_check_text(listed, &[]), Ok(()));
    let none = Constraints::<i32>::from_raw(listed, &[]).unwrap();
    let rejected = cast_checked("7", &NumFormat::INVARIANT, &none);
    assert!(matches!(rejected, Err(Rejection::Violation(v)) if v.keyword == Keyword::Enum));
}

#[test]
fn from_raw_entries_without_enum_flag_are_malformed() {
    let integers = 7u64.to_ne_bytes().into_iter().chain([0; 8]).collect::<Vec<u8>>();
    assert_eq!(same_as_check::<i32>(raw(0), &integers), Err(Refusal::MalformedLayout));
    assert_eq!(same_as_check_text(raw(0), &packed_text(&["a"])), Err(Refusal::MalformedLayout));
}

#[test]
fn from_raw_ignores_the_other_types_keywords_and_refuses_misplaced_extensions() {
    // JSON Schema: a keyword for another type is ignored.
    assert_eq!(same_as_check::<i32>(raw(RawConstraints::HAS_MIN_LENGTH), &[]), Ok(()));
    assert_eq!(same_as_check_text(raw(RawConstraints::HAS_MINIMUM), &[]), Ok(()));
    // HyperCast's own extensions belong to one door each.
    assert_eq!(
        same_as_check::<i32>(raw(RawConstraints::CASE_SENSITIVE), &[]),
        Err(Refusal::KeywordNotForDoor)
    );
    assert_eq!(
        same_as_check::<f64>(raw(RawConstraints::TRIM), &[]),
        Err(Refusal::KeywordNotForDoor)
    );
    assert_eq!(
        same_as_check_text(
            RawConstraints {
                flags: RawConstraints::HAS_MULTIPLE_OF,
                rounding: 1,
                multiple_of: Decimal::from(1),
                ..RawConstraints::ZERO
            },
            &[]
        ),
        Err(Refusal::KeywordNotForDoor)
    );
    // Packed text handed to an integer door is read as packed integers, never as text: the
    // answer is check_constraints' at that door, whatever it is.
    assert!(same_as_check::<i32>(raw(RawConstraints::HAS_ENUM), &packed_text(&["Male"])).is_err());
}

#[test]
fn from_raw_with_another_size_is_malformed() {
    let short = RawConstraints { size: 71, ..RawConstraints::ZERO };
    assert_eq!(same_as_check::<f64>(short, &[]), Err(Refusal::MalformedLayout));
    assert_eq!(same_as_check_text(short, &[]), Err(Refusal::MalformedLayout));
}

/// Packs `constraints` into buffers of every length below the packed length, each prefilled
/// with `0xAA`, and requires each to come back untouched with the full length; then into one
/// longer buffer, whose tail past the packed bytes stays `0xAA`. Returns the packed bytes.
fn packed_with_short_buffers<T>(constraints: &Constraints<'_, T>) -> Vec<u8> {
    let len = constraints.pack_enum(&mut []).expect("packable");
    for short in 0..len {
        let mut out = vec![0xAA; short];
        assert_eq!(constraints.pack_enum(&mut out), Some(len), "length {short}");
        assert!(out.iter().all(|&b| b == 0xAA), "length {short} was written");
    }
    let mut out = vec![0xAA; len + 3];
    assert_eq!(constraints.pack_enum(&mut out), Some(len));
    assert_eq!(&out[len..], &[0xAA; 3], "past the packed bytes");
    out.truncate(len);
    out
}

fn listed() -> RawConstraints {
    raw(RawConstraints::HAS_ENUM)
}

#[test]
fn pack_short_buffer_writes_nothing() {
    let entries = [10, -20, 30].map(Decimal::from);
    let integers = Constraints::<i32>::numeric(NumericRules::new(), Some(&entries)).unwrap();
    let packed_integers = packed_with_short_buffers(&integers);
    assert_eq!(packed_integers.len(), 3 * 16);
    let entries = ["Female", "Male", "Other"];
    let texts = Constraints::text(TextRules::new(), Some(&entries)).unwrap();
    let packed_texts = packed_with_short_buffers(&texts);
    assert_eq!(packed_texts, packed_text(&entries));
    let from_integers = Constraints::<i32>::from_raw(listed(), &packed_integers).unwrap();
    assert_eq!(packed_with_short_buffers(&from_integers), packed_integers);
    let from_texts = Constraints::<Text>::from_raw(listed(), &packed_texts).unwrap();
    assert_eq!(packed_with_short_buffers(&from_texts), packed_texts);
    let none = Constraints::<f64>::numeric(NumericRules::new(), None).unwrap();
    assert_eq!(packed_with_short_buffers(&none), Vec::<u8>::new());
}

#[test]
fn pack_no_enum_is_zero() {
    let none = Constraints::text(TextRules::new().trim(true), None).unwrap();
    assert_eq!(none.pack_enum(&mut [0xAA; 4]), Some(0));
    let from_raw = Constraints::<u8>::from_raw(raw(0), &[]).unwrap();
    assert_eq!(from_raw.pack_enum(&mut []), Some(0));
}

/// One packed integer entry, decoded field by field as the C ABI lays it out.
fn decode(chunk: &[u8]) -> i128 {
    assert_eq!(chunk.len(), 16);
    let lo = u64::from_ne_bytes(chunk[0..8].try_into().unwrap());
    let hi = u32::from_ne_bytes(chunk[8..12].try_into().unwrap());
    assert_eq!(chunk[12], 0, "scale");
    assert!(chunk[13] <= 1, "sign byte");
    assert_eq!(&chunk[14..16], &[0, 0], "padding");
    let magnitude = (i128::from(hi) << 64) | i128::from(lo);
    if chunk[13] == 1 { -magnitude } else { magnitude }
}

#[test]
fn pack_integer_edges() {
    let signed = [i64::MIN as i128, -1, 0, i64::MAX as i128];
    let entries = [i64::MIN, -1, 0, i64::MAX].map(Decimal::from);
    let i64s = Constraints::<i64>::numeric(NumericRules::new(), Some(&entries)).unwrap();
    let packed = packed_with_short_buffers(&i64s);
    assert_eq!(packed.chunks(16).map(decode).collect::<Vec<_>>(), signed);
    let read = Constraints::<i64>::from_raw(i64s.raw(), &packed).unwrap();
    let format = NumFormat::INVARIANT;
    assert_eq!(cast_checked("-9223372036854775808", &format, &read).map(|c| c.value), Ok(i64::MIN));
    assert!(cast_checked("-2", &format, &read).is_err(), "not a member");

    let unsigned = [u64::MAX as i128, 0];
    let entries = [u64::MAX, 0].map(Decimal::from);
    let u64s = Constraints::<u64>::numeric(NumericRules::new(), Some(&entries)).unwrap();
    let packed = packed_with_short_buffers(&u64s);
    assert_eq!(packed.chunks(16).map(decode).collect::<Vec<_>>(), unsigned);
    let read = Constraints::<u64>::from_raw(u64s.raw(), &packed).unwrap();
    assert_eq!(cast_checked("18446744073709551615", &format, &read).map(|c| c.value), Ok(u64::MAX));
}

#[test]
fn pack_is_identity_for_packed() {
    let texts = packed_text(&["Female", "Male", "Other"]);
    let read = Constraints::<Text>::from_raw(listed(), &texts).unwrap();
    let mut out = vec![0; texts.len()];
    assert_eq!(read.pack_enum(&mut out), Some(texts.len()));
    assert_eq!(out, texts);
    // A packed integer entry at a scale above 0 (`7.0`) is copied as given, not normalized.
    let mut seven = Vec::new();
    seven.extend_from_slice(&70u64.to_ne_bytes());
    seven.extend_from_slice(&0u32.to_ne_bytes());
    seven.extend_from_slice(&[1, 0, 0, 0]);
    let read = Constraints::<i32>::from_raw(listed(), &seven).unwrap();
    let mut out = vec![0; 16];
    assert_eq!(read.pack_enum(&mut out), Some(16));
    assert_eq!(out, seven);
}

/// On a 32-bit target, a borrowed text list whose packed size passes `usize::MAX`: five
/// overlapping 2³⁰-byte entries of one buffer, each starting with a different letter. Every
/// allocation and slice stays below `isize::MAX`, the most Rust can validly build there.
#[cfg(target_pointer_width = "32")]
#[test]
fn pack_total_overflows_on_32_bit() {
    const ENTRY: usize = 1 << 30;
    let layout = std::alloc::Layout::array::<u8>(ENTRY + 4).expect("a layout");
    // SAFETY: the layout has a non-zero size.
    let ptr = unsafe { std::alloc::alloc_zeroed(layout) };
    assert!(!ptr.is_null(), "the allocator refused {} zeroed bytes", ENTRY + 4);
    // SAFETY: `ptr` is valid for ENTRY + 4 zeroed bytes until the dealloc below.
    let buffer = unsafe { std::slice::from_raw_parts_mut(ptr, ENTRY + 4) };
    buffer[..5].copy_from_slice(b"abcde");
    let buffer: &[u8] = buffer;
    let mut out = [0xAA; 8];
    let packed = {
        let entries: Vec<&str> =
            (0..5).map(|i| core::str::from_utf8(&buffer[i..i + ENTRY]).expect("UTF-8")).collect();
        Constraints::text(TextRules::new(), Some(&entries)).map(|texts| texts.pack_enum(&mut out))
    };
    // SAFETY: allocated above with this layout; no borrow of it is used past here.
    unsafe { std::alloc::dealloc(ptr, layout) };
    assert_eq!(packed, Ok(None), "5 × (4 + 2³⁰) bytes overflow a 32-bit usize");
    assert_eq!(out, [0xAA; 8], "nothing written");
}

/// `enum_entries` reads a numeric `enum` back as declared: order and duplicates kept.
#[test]
fn enum_entries_read_back_as_declared() {
    let entries = [20, 10, 20].map(Decimal::from);
    let declared = Constraints::<i32>::numeric(NumericRules::new(), Some(&entries)).unwrap();
    let read = declared.enum_entries().expect("an enum is declared");
    assert_eq!(read.len(), 3);
    assert_eq!(read.collect::<Vec<_>>(), entries);
}

/// `enum_entries` reads a text `enum` back as declared, bytes and case as written.
#[test]
fn text_enum_entries_read_back_as_declared() {
    let entries = ["Male", "male", "Male"];
    let declared = Constraints::text(TextRules::new(), Some(&entries)).unwrap();
    let read = declared.enum_entries().expect("an enum is declared").collect::<Vec<_>>();
    assert_eq!(read, [&b"Male"[..], b"male", b"Male"]);
}

/// A list read by `from_raw` reads back as it was packed. A number in a text `enum` is packed
/// as an empty entry, and reads back as one.
#[test]
fn from_raw_packed_entries_read_back() {
    let numbers = [7, -3, 7].map(Decimal::from);
    let declared = Constraints::<i64>::numeric(NumericRules::new(), Some(&numbers)).unwrap();
    let mut packed = vec![0; declared.pack_enum(&mut []).unwrap()];
    declared.pack_enum(&mut packed).unwrap();
    let read = Constraints::<i64>::from_raw(declared.raw(), &packed).unwrap();
    assert_eq!(read.enum_entries().unwrap().collect::<Vec<_>>(), numbers);

    let text = packed_text(&["a", "", "b"]);
    let read = Constraints::<Text>::from_raw(raw(RawConstraints::HAS_ENUM), &text).unwrap();
    let entries = read.enum_entries().unwrap();
    assert_eq!(entries.len(), 3);
    assert_eq!(entries.collect::<Vec<_>>(), [&b"a"[..], b"", b"b"]);
}

/// No `enum` declared reads as `None`, at either kind of door.
#[test]
fn no_enum_reads_none() {
    assert!(
        Constraints::<i32>::numeric(NumericRules::new(), None).unwrap().enum_entries().is_none()
    );
    assert!(Constraints::text(TextRules::new(), None).unwrap().enum_entries().is_none());
    assert!(Constraints::<i32>::from_raw(raw(0), &[]).unwrap().enum_entries().is_none());
    assert!(Constraints::<Text>::from_raw(raw(0), &[]).unwrap().enum_entries().is_none());
}

/// A declared empty `enum` reads back present and empty, never as none, however it was
/// declared, and every value violates it.
#[test]
fn an_empty_enum_reads_back_empty() {
    let listed = raw(RawConstraints::HAS_ENUM);
    let from_raw = Constraints::<i32>::from_raw(listed, &[]).unwrap();
    assert_eq!(from_raw.enum_entries().map(|entries| entries.len()), Some(0));
    let rejected = cast_checked("7", &NumFormat::INVARIANT, &from_raw);
    assert!(matches!(rejected, Err(Rejection::Violation(v)) if v.keyword == Keyword::Enum));
    let numeric = Constraints::<i32>::numeric(NumericRules::new(), Some(&[])).unwrap();
    assert_eq!(numeric.enum_entries().map(|entries| entries.len()), Some(0));
    let text = Constraints::<Text>::from_raw(listed, &[]).unwrap();
    assert_eq!(text.enum_entries().map(|entries| entries.len()), Some(0));
    let empty: &[&str] = &[];
    let text = Constraints::text(TextRules::new(), Some(empty)).unwrap();
    assert_eq!(text.enum_entries().map(|entries| entries.len()), Some(0));
}
