module HyperCast
  # The native core's C ABI values, presented — for a gem that carries HyperCast's verdicts
  # across a C ABI of its own (HyperTabular does): how each door's out-value unpacks and what
  # builds the Ruby value from it, the verdict code and span of a fault, the packed version
  # word, and where a gem's native library is. The Fiddle backend's doors decode with exactly
  # these, so a value read out of another library's buffer is the value the door would have.
  #
  # Every directive below unpacks one value. A gem decoding a whole column appends "*" and
  # steps +fields+ at a time.
  module Interop
    # The doors whose value is one String#unpack directive away: the directive for one value.
    # A bool is the byte 0 or 1, unpacked as "C" and compared with zero.
    SCALARS = {
      bool: "C", i8: "c", i16: "s<", i32: "l<", i64: "q<", u8: "C", u16: "S<", u32: "L<",
      u64: "Q<", f32: "e", f64: "E", time: "Q<"
    }.freeze

    # The doors whose value is a record: the directive that unpacks one, how many fields that
    # yields, and what builds the door's Ruby value from them, starting at field +at+.
    RECORDS = {
      decimal: ["Q<L<CCx2", 4, ->(fields, at) { decimal(fields[at], fields[at + 1], fields[at + 2], fields[at + 3]) }],
      uuid: ["H8H4H4H4H12", 5, ->(fields, at) { fields[at, 5].join("-") }],
      timestamp: ["q<l<x4", 2, ->(fields, at) { instant(fields[at], fields[at + 1]) }],
      date: ["S<CC", 3, ->(fields, at) { Date.new(fields[at], fields[at + 1], fields[at + 2]) }],
      datetime: ["S<CCx4Q<", 4, ->(fields, at) { civil(fields[at], fields[at + 1], fields[at + 2], fields[at + 3]) }],
      duration: ["q<l<x4", 2, ->(fields, at) { duration(fields[at], fields[at + 1]) }]
    }.tap do |records|
      records[:unix] = records[:excel_serial] = records[:timestamp]
      records[:date_ordered] = records[:date]
    end.freeze

    # The bytes one value of each door takes.
    VALUE_BYTES = {
      bool: 1, i8: 1, u8: 1, i16: 2, u16: 2, i32: 4, u32: 4, f32: 4, date: 4, date_ordered: 4,
      i64: 8, u64: 8, f64: 8, time: 8,
      decimal: 16, uuid: 16, timestamp: 16, unix: 16, excel_serial: 16, datetime: 16, duration: 16
    }.freeze

    # Encodings whose bytes already are the UTF-8 (or byte-identical) form the core reads.
    BYTE_COMPATIBLE = [Encoding::UTF_8, Encoding::US_ASCII, Encoding::ASCII_8BIT].freeze

    module_function

    # One value of +door+, from the bytes the core wrote for it.
    def decode(door, bytes)
      if (directive = SCALARS[door])
        value = bytes.unpack1(directive)
        door == :bool ? value != 0 : value
      else
        directive, _fields, build = RECORDS.fetch(door)
        build.call(bytes.unpack(directive), 0)
      end
    end

    # A UTC Time from the core's protobuf-shaped {seconds, nanos} pair, exactly — the
    # timestamp, unix and excel_serial doors' value. (Time.at then #utc is the same Time as
    # Time.at(..., in: "UTC"), at half the cost: the zone's name is not read each time.)
    def instant(seconds, nanos)
      Time.at(seconds, nanos, :nanosecond).utc
    end

    # The zone-less DateTime a civil date-time names, with exact Rational seconds — the
    # datetime door's value. Its +00:00 offset is a carrier artifact, not data.
    def civil(year, month, day, nanos_of_day)
      second_of_day, frac = nanos_of_day.divmod(1_000_000_000)
      hour, rest = second_of_day.divmod(3600)
      minute, second = rest.divmod(60)
      DateTime.new(year, month, day, hour, minute, second + Rational(frac, 1_000_000_000))
    end

    # Exact Rational seconds from the core's same-signed {seconds, nanos} pair — the duration
    # door's value.
    def duration(seconds, nanos)
      Rational(seconds * 1_000_000_000 + nanos, 1_000_000_000)
    end

    # A Decimal from the core's {lo, hi, scale, negative} — the decimal door's value.
    def decimal(lo, hi, scale, negative)
      Decimal.new(magnitude: (hi << 64) | lo, scale: scale, negative: negative != 0)
    end

    # The Fault a nonzero verdict code and its span name. A code that names no reason is a
    # binding bug, not data: a KeyError.
    def fault(code, offset, length)
      Fault.new(reason: REASONS.fetch(code), offset: offset, length: length)
    end

    # The core's byte span over +bytes+, in the units String#[] slices by: an identity for a
    # binary String (its characters are its bytes) and for ASCII text (`ascii_only?` reads the
    # cached coderange — no scan), a byte-to-character remap otherwise. Character counts
    # survive transcoding, so a span mapped on the UTF-8 form indexes the caller's own String
    # whatever encoding it arrived in.
    def characters(bytes, offset, length)
      return [offset, length] if bytes.encoding == Encoding::ASCII_8BIT || bytes.ascii_only?

      [bytes.byteslice(0, offset).length, bytes.byteslice(offset, length).length]
    end

    # A native library's packed version word — major << 16 | minor << 8 | patch, as a
    # *_version export returns it — as "major.minor.patch".
    def version(word)
      "#{word >> 16}.#{(word >> 8) & 0xFF}.#{word & 0xFF}"
    end

    # The shared library to dlopen for +library+ ("hypercast" for libhypercast.so): the
    # gem's native/{rid}/{file} under +native_dir+, or — the development loop — the cargo
    # build under +repo_root+/rust/target/release, exactly what the other bindings' local
    # staging does. Nil when neither exists.
    def library_path(library, native_dir, repo_root)
      rid, file = NativePlatform.rid_and_library_name(library: library)
      path = File.join(native_dir, rid, file)
      return path if File.exist?(path)

      repo_build = File.join(repo_root, "rust", "target", "release", file)
      File.exist?(repo_build) ? repo_build : nil
    end
  end
end
