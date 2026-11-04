using System.Buffers.Binary;
using System.Text;

namespace HyperCast;

/// <summary>
/// A caller-built month vocabulary: the names a date door reads in its month field, each mapped to
/// a month 1 to 12 (<c>07-Jan-26</c>, <c>7 janvier 2026</c>). No platform builds one — aliases,
/// abbreviations and grammatical forms (<c>janvier</c>, <c>janv.</c>, <c>de janeiro</c>) are the
/// caller's decisions. Declared through <see cref="DateOptions.Months"/>.
/// </summary>
/// <remarks>
/// <para>
/// A name replaces the numeric month in the position the declared <see cref="DateOrder"/> selects,
/// with the date's own repeated <c>/</c>, <c>-</c> or <c>.</c> separator. Matching folds simple case; the
/// longest entry followed by the separator wins; punctuation is part of the name and never stripped,
/// so with <c>janv.</c> declared, <c>7.janv..2026</c> reads and <c>7.janv.2026</c> does not.
/// </para>
/// <para>
/// Each entry is judged as the core judges it: a month 1 to 12; every month has at least one entry; no entry
/// is empty, longer than <see cref="MaxNameBytes"/> bytes of UTF-8, or holds an ASCII digit, <c>/</c>, <c>-</c> or any
/// white space (<c>.</c> is allowed); two entries that fold to the same spelling are refused, even for the
/// same month. A vocabulary the core would refuse is a caller bug: the constructor throws an
/// <see cref="ArgumentException"/>.
/// </para>
/// <para>
/// <b>Building one calls the native core:</b> the constructor and <see cref="FromPacked"/> ask it to decide
/// the case-fold repeats the binding cannot see, so a missing or stale library surfaces here as a
/// <see cref="DllNotFoundException"/> or <see cref="EntryPointNotFoundException"/>, not at the first cast.
/// </para>
/// </remarks>
public sealed class MonthNames
{
	/// <summary>The most UTF-8 bytes one name may occupy.</summary>
	public const int MaxNameBytes = 64;

	readonly byte[] _packed;
	readonly (int Month, string Name)[] _entries;

	MonthNames((int Month, string Name)[] entries, byte[] packed) => (_entries, _packed) = (entries, packed);

	/// <summary>Declares the vocabulary from its entries.</summary>
	/// <param name="entries">Each name with the month (1 to 12) it stands for; aliases are separate entries.</param>
	/// <exception cref="ArgumentNullException"><paramref name="entries"/> or a name is <see langword="null"/>.</exception>
	/// <exception cref="ArgumentException">An entry or the vocabulary breaks a rule above.</exception>
	/// <exception cref="DllNotFoundException">The native library did not load: the core is asked to judge the vocabulary.</exception>
	/// <exception cref="EntryPointNotFoundException">The library loaded is a core older than 1.1.</exception>
	public MonthNames(params (int Month, string Name)[] entries)
	{
		ArgumentNullException.ThrowIfNull(entries);
		var walk = new Walk();
		foreach (var (month, name) in entries)
			walk.Add(month, name);
		(_entries, _packed) = walk.Finish();
	}

	/// <inheritdoc cref="MonthNames(ValueTuple{int, string}[])"/>
	public MonthNames(IEnumerable<(int Month, string Name)> entries)
		: this(entries is null ? throw new ArgumentNullException(nameof(entries)) : [.. entries])
	{
	}

	/// <summary>The entries as declared.</summary>
	public IReadOnlyList<(int Month, string Name)> Entries => _entries;

	/// <summary>
	/// The vocabulary in the core's packed layout: each entry a month byte (1 to 12), a little-endian
	/// <see cref="uint"/> byte length, then that many UTF-8 bytes, with no padding.
	/// </summary>
	public ReadOnlySpan<byte> Packed => _packed;

	/// <summary>
	/// Reads a vocabulary from its packed layout (see <see cref="Packed"/>) — for a library that carries
	/// the core's own bytes across a C ABI of its own — judging it entry by entry as the core does.
	/// </summary>
	/// <param name="packed">Each entry a month byte, a little-endian <see cref="uint"/> byte length, then that many UTF-8 bytes.</param>
	/// <exception cref="ArgumentException">The bytes are cut short or have trailing bytes, hold a name that is not UTF-8, or break a rule above.</exception>
	/// <exception cref="DllNotFoundException">The native library did not load: the core is asked to judge the vocabulary.</exception>
	/// <exception cref="EntryPointNotFoundException">The library loaded is a core older than 1.1.</exception>
	public static MonthNames FromPacked(ReadOnlySpan<byte> packed)
	{
		var walk = new Walk();
		var strict = new UTF8Encoding(false, true);
		while (!packed.IsEmpty)
		{
			if (packed.Length < 5 || BinaryPrimitives.ReadUInt32LittleEndian(packed[1..]) > (uint)(packed.Length - 5))
				throw new ArgumentException("The packed month names are cut short, or have trailing bytes after the last entry.", Parameter);
			var month = packed[0];
			var length = (int)BinaryPrimitives.ReadUInt32LittleEndian(packed[1..]);
			if (month is < 1 or > 12)
				throw new ArgumentException($"Month {month} is not 1 to 12.", Parameter);
			string name;
			try
			{
				name = strict.GetString(packed.Slice(5, length));
			}
			catch (DecoderFallbackException)
			{
				throw new ArgumentException($"The name of month {month} is not well-formed UTF-8 text.", Parameter);
			}
			walk.Add(month, name);
			packed = packed[(5 + length)..];
		}
		var (entries, bytes) = walk.Finish();
		return new MonthNames(entries, bytes);
	}

	const string Parameter = "entries";

	// The core's judgement, an entry at a time and in its order: the month, the encoding, the length,
	// the characters, then a repeat; then, once every entry is in, that every month has one.
	sealed class Walk
	{
		readonly List<(int Month, string Name)> _entries = [];
		readonly List<string> _folded = [];
		readonly bool[] _months = new bool[13];
		int _size;

		public void Add(int month, string name)
		{
			if (month is < 1 or > 12)
				throw new ArgumentException($"Month {month} is not 1 to 12.", Parameter);
			ArgumentNullException.ThrowIfNull(name, Parameter);
			for (var i = 0; i < name.Length; i++)
				if (char.IsSurrogate(name[i]) && !(char.IsHighSurrogate(name[i]) && ++i < name.Length && char.IsLowSurrogate(name[i])))
					throw new ArgumentException($"Month name '{name}' is not well-formed UTF-8 text: a lone surrogate half.", Parameter);
			var bytes = Encoding.UTF8.GetByteCount(name);
			if (bytes == 0 || bytes > MaxNameBytes)
				throw new ArgumentException($"Month name '{name}' must be 1 to {MaxNameBytes} bytes long.", Parameter);
			foreach (var c in name)
				if (char.IsAsciiDigit(c) || c is '/' or '-' || WhiteSpace.IsWhiteSpace(c))
					throw new ArgumentException($"Month name '{name}' must not contain a digit, '/', '-' or white space.", Parameter);
			var folded = name.ToLowerInvariant();
			if (_folded.Contains(folded))
				throw new ArgumentException($"Month name '{name}' repeats an earlier name once case is folded.", Parameter);
			_folded.Add(folded);
			_months[month] = true;
			_entries.Add((month, name));
			_size = checked(_size + 5 + bytes);
		}

		public ((int Month, string Name)[] Entries, byte[] Packed) Finish()
		{
			for (var month = 1; month <= 12; month++)
				if (!_months[month])
					throw new ArgumentException($"Month {month} has no name.", Parameter);
			var packed = new byte[_size];
			var at = 0;
			foreach (var (month, name) in _entries)
			{
				packed[at] = (byte)month;
				var length = Encoding.UTF8.GetBytes(name, packed.AsSpan(at + 5));
				BinaryPrimitives.WriteUInt32LittleEndian(packed.AsSpan(at + 1), (uint)length);
				at += 5 + length;
			}
			// What the folding above cannot decide — the core's simple case folding is wider than the
			// invariant lower-casing — the core decides: the one rule left, a repeat, is its refusal.
			if (Cast.RefusesMonthNames(packed))
				throw new ArgumentException("A month name repeats an earlier name once the core folds case.", Parameter);
			return ([.. _entries], packed);
		}
	}
}
