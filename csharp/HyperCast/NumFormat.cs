using System.Globalization;
using System.Runtime.CompilerServices;
using System.Text;

namespace HyperCast;

/// <summary>
/// Caller-declared numeric notation for the integer, real and decimal doors. The native
/// core carries no culture data — this is the bridge from .NET's culture machinery to the
/// core's declared fields. A call site parsing culture-sensitive text declares its format
/// out loud (<see cref="Invariant"/>, <see cref="From(CultureInfo)"/>,
/// <see cref="From(IFormatProvider)"/>, or the constructor with the separators spelled
/// directly), or hands each numeric door the <see cref="IFormatProvider"/> it would hand a
/// BCL <c>TryParse</c>: those overloads map it through <see cref="From(IFormatProvider)"/>,
/// and read <see langword="null"/> as the current culture, as the BCL does.
/// </summary>
/// <remarks>
/// <para>
/// Everything past the separators and the flags is declared, never looked up, and every
/// one of those fields defaults to declaring nothing, which reads exactly as the invariant
/// culture: ASCII signs and percent, ASCII digits, the number separators for a currency
/// amount. The rule for each declared symbol is the same: the ASCII spelling is always
/// read, and the declared one is a further spelling read beside it — so nothing that parsed
/// before a symbol was declared stops parsing once it is. None is localized further:
/// <c>$</c> is matched as <c>$</c>, and U+2212 only where it is declared.
/// </para>
/// <para>
/// A separator, a sign and the percent symbol are 1 to
/// <see cref="MaxSymbolBytes"/> bytes of UTF-8, which covers every real culture's (<c>.</c>,
/// <c>,</c>, U+00A0, U+202F, de-CH's <c>’</c>, and a two-character <c>, </c>); the currency
/// symbol up to <see cref="MaxCurrencyBytes"/>.
/// </para>
/// </remarks>
/// <param name="DecimalSeparator">The declared decimal separator: one to <see cref="MaxSymbolBytes"/> bytes of UTF-8, no ASCII digit.</param>
/// <param name="GroupSeparator">The declared digit-group separator, likewise. Must differ from <paramref name="DecimalSeparator"/>.</param>
/// <param name="Styles">The lenience flags.</param>
public readonly record struct NumFormat(string DecimalSeparator, string GroupSeparator, NumStyles Styles)
{
	/// <summary>
	/// The most UTF-8 bytes a separator, a sign or the <see cref="PercentSymbol"/> may
	/// occupy — the core's inline buffer for each.
	/// </summary>
	public const int MaxSymbolBytes = 8;

	/// <summary>The most UTF-8 bytes a <see cref="CurrencySymbol"/> may occupy — the core's inline buffer.</summary>
	public const int MaxCurrencyBytes = 16;

	// What every numeric door calls the parameter this value arrives in, so a malformed
	// format is reported against the argument the caller actually passed.
	const string DoorParameter = "format";

	/// <summary>The invariant profile — <c>.</c> decimal, <c>,</c> grouping, every lenience on, nothing else declared.</summary>
	public static readonly NumFormat Invariant = new(".", ",", NumStyles.All);

	/// <summary>
	/// The detection profile — every lenience on, <c>.</c>/<c>,</c> roles resolved per
	/// input by <see cref="NumStyles.SeparatorDetect"/>'s structural rules.
	/// </summary>
	public static readonly NumFormat Detect = new(".", ",", NumStyles.All | NumStyles.SeparatorDetect);

	/// <summary>Declares single-character separators and the flags — the common case, spelled as characters.</summary>
	/// <param name="decimalSeparator">The declared decimal separator.</param>
	/// <param name="groupSeparator">The declared digit-group separator. Must differ from <paramref name="decimalSeparator"/>.</param>
	/// <param name="styles">The lenience flags.</param>
	public NumFormat(char decimalSeparator, char groupSeparator, NumStyles styles)
		: this(decimalSeparator.ToString(), groupSeparator.ToString(), styles)
	{
	}

	/// <summary>Declares single-character separators, the flags and the currency symbol together.</summary>
	/// <param name="decimalSeparator">The declared decimal separator.</param>
	/// <param name="groupSeparator">The declared digit-group separator. Must differ from <paramref name="decimalSeparator"/>.</param>
	/// <param name="styles">The lenience flags.</param>
	/// <param name="currencySymbol">The declared currency symbol, honored under <see cref="NumStyles.Currency"/>; empty declares none.</param>
	public NumFormat(char decimalSeparator, char groupSeparator, NumStyles styles, string currencySymbol)
		: this(decimalSeparator.ToString(), groupSeparator.ToString(), styles) =>
		CurrencySymbol = currencySymbol;

	/// <summary>Declares the separators, flags and currency symbol together.</summary>
	/// <param name="decimalSeparator">The declared decimal separator.</param>
	/// <param name="groupSeparator">The declared digit-group separator. Must differ from <paramref name="decimalSeparator"/>.</param>
	/// <param name="styles">The lenience flags.</param>
	/// <param name="currencySymbol">The declared currency symbol, honored under <see cref="NumStyles.Currency"/>; empty declares none.</param>
	public NumFormat(string decimalSeparator, string groupSeparator, NumStyles styles, string currencySymbol)
		: this(decimalSeparator, groupSeparator, styles) =>
		CurrencySymbol = currencySymbol;

	/// <summary>
	/// The declared decimal separator. One to <see cref="MaxSymbolBytes"/> bytes of UTF-8
	/// with no ASCII digit; anything else is a caller bug the doors reject with an
	/// <see cref="ArgumentException"/>.
	/// </summary>
	/// <remarks>
	/// Never <see langword="null"/>, whatever built the value: a <see langword="null"/> handed
	/// to the constructor or the initializer, and the unset field of a
	/// <c>default(NumFormat)</c>, both read back as empty — which, for a number separator,
	/// is the caller bug the doors report — rather than surfacing as a
	/// <see cref="NullReferenceException"/> inside the first door that reads it. Every
	/// string field here reads the same way.
	/// </remarks>
	public string DecimalSeparator { get => field ?? ""; init; } = DecimalSeparator;

	/// <summary>The declared digit-group separator, as <see cref="DecimalSeparator"/>. Must differ from it.</summary>
	public string GroupSeparator { get => field ?? ""; init; } = GroupSeparator;

	/// <summary>
	/// The declared currency symbol, honored only while <see cref="NumStyles.Currency"/> is
	/// set (it is part of <see cref="NumStyles.All"/>). Empty — the default — declares none.
	/// At most <see cref="MaxCurrencyBytes"/> bytes of UTF-8, containing no ASCII digit or
	/// whitespace; anything else is a caller bug the doors reject with an
	/// <see cref="ArgumentException"/>.
	/// </summary>
	public string CurrencySymbol { get => field ?? ""; init; } = "";

	/// <summary>
	/// The decimal separator of a currency amount — read in place of
	/// <see cref="DecimalSeparator"/> when <see cref="NumStyles.Currency"/> is set and the
	/// declared <see cref="CurrencySymbol"/> is in the text, as the BCL switches to a
	/// culture's <see cref="NumberFormatInfo.CurrencyDecimalSeparator"/> when it sees the
	/// symbol (several English-in-Europe cultures, en-DE among them, write a decimal comma
	/// in numbers and a decimal point in currency). Empty — the default — keeps
	/// <see cref="DecimalSeparator"/>. Under <see cref="NumStyles.SeparatorDetect"/>
	/// detection governs the whole token and this is ignored.
	/// </summary>
	public string CurrencyDecimalSeparator { get => field ?? ""; init; } = "";

	/// <summary>
	/// The digit-group separator of a currency amount, as
	/// <see cref="CurrencyDecimalSeparator"/>. Empty keeps <see cref="GroupSeparator"/>; the
	/// pair in force must differ.
	/// </summary>
	public string CurrencyGroupSeparator { get => field ?? ""; init; } = "";

	/// <summary>
	/// A further spelling of the minus sign, read wherever ASCII <c>-</c> is: ahead of the
	/// digits, before or after a leading currency symbol, and after the digits under
	/// <see cref="NumStyles.TrailingSign"/> (U+2212 in et-EE and eu-ES). ASCII <c>-</c> is
	/// always read; empty — the default — declares no other. At most
	/// <see cref="MaxSymbolBytes"/> bytes of UTF-8, with no ASCII digit or whitespace.
	/// </summary>
	public string MinusSign { get => field ?? ""; init; } = "";

	/// <summary>A further spelling of the plus sign, as <see cref="MinusSign"/> is of the minus, beside ASCII <c>+</c>.</summary>
	public string PlusSign { get => field ?? ""; init; } = "";

	/// <summary>
	/// A further spelling of the percent symbol, read as a trailing <c>%</c> is while
	/// <see cref="NumStyles.Percent"/> is set (<c>٪</c> in Arabic-script cultures, <c>％</c>
	/// fullwidth). ASCII <c>%</c> is always read; empty — the default — declares no other.
	/// At most <see cref="MaxSymbolBytes"/> bytes of UTF-8, with no ASCII digit or whitespace.
	/// </summary>
	public string PercentSymbol { get => field ?? ""; init; } = "";

	/// <summary>
	/// The zero of a set of native digits read beside ASCII <c>0</c>–<c>9</c> everywhere a
	/// digit is (integer part, fraction, exponent; a radix-prefixed number stays ASCII): the
	/// ten code points from it up, the way ICU and <see cref="NumberFormatInfo.NativeDigits"/>
	/// describe a culture's digits (U+0660 <c>٠</c> declares the Arabic-Indic digits, U+FF10
	/// <c>０</c> the fullwidth ones). <see langword="null"/> — the default — declares none.
	/// Never ASCII, and never a code point whose ten successors would not share its UTF-8
	/// prefix (one whose value modulo 64 is past 54 — no Unicode digit block starts at one);
	/// either is a caller bug the doors reject with an <see cref="ArgumentException"/>.
	/// </summary>
	public Rune? DigitZero { get; init; }

	/// <summary>
	/// Derives a format from a culture's number formatting — every field
	/// <see cref="From(NumberFormatInfo)"/> fills, from the culture's
	/// <see cref="CultureInfo.NumberFormat"/>.
	/// </summary>
	/// <param name="culture">The culture to derive from. Never null.</param>
	/// <exception cref="ArgumentNullException"><paramref name="culture"/> is null.</exception>
	public static NumFormat From(CultureInfo culture)
	{
		ArgumentNullException.ThrowIfNull(culture);
		return From(culture.NumberFormat);
	}

	/// <summary>
	/// Derives a format from whatever number formatting <paramref name="provider"/> resolves
	/// to — the shape every BCL <c>TryParse</c> already takes, so a caller that is itself
	/// <see cref="IFormatProvider"/>-shaped needs no <see cref="CultureInfo"/> cast first.
	/// Resolution is <see cref="NumberFormatInfo.GetInstance"/>'s: a
	/// <see cref="NumberFormatInfo"/> directly, a <see cref="CultureInfo"/>'s number format,
	/// or the current culture's for any other provider.
	/// </summary>
	/// <param name="provider">The format provider to derive from. Never null.</param>
	/// <exception cref="ArgumentNullException"><paramref name="provider"/> is null.</exception>
	public static NumFormat From(IFormatProvider provider)
	{
		ArgumentNullException.ThrowIfNull(provider);
		return From(NumberFormatInfo.GetInstance(provider));
	}

	/// <summary>
	/// Derives a format from number-format data directly: every field the core reads,
	/// filled from the culture's own, with every lenience on.
	/// </summary>
	/// <remarks>
	/// <list type="bullet">
	/// <item><description><see cref="NumberFormatInfo.NumberDecimalSeparator"/> and
	/// <see cref="NumberFormatInfo.NumberGroupSeparator"/> whole, however many characters
	/// each is (<c>,</c> for a culture that declares no group separator), and
	/// <see cref="NumberFormatInfo.CurrencySymbol"/>.</description></item>
	/// <item><description><see cref="NumberFormatInfo.CurrencyDecimalSeparator"/> and
	/// <see cref="NumberFormatInfo.CurrencyGroupSeparator"/>, each where it differs from the
	/// number separator it stands in for, so a currency amount is read as the BCL reads one.</description></item>
	/// <item><description>Whatever a runtime's data holds, the bridge never throws: a symbol
	/// the core could not read (an ASCII digit in it, whitespace against the digits, past
	/// its buffer) is left undeclared, and the ASCII spelling is read alone; a group separator
	/// equal to the decimal one (NLS gives mn-Mong-MN a comma for both) takes the other of
	/// <c>.</c> and <c>,</c>; a currency pair that would collide declares nothing.</description></item>
	/// <item><description><see cref="NumberFormatInfo.NegativeSign"/>,
	/// <see cref="NumberFormatInfo.PositiveSign"/> and <see cref="NumberFormatInfo.PercentSymbol"/>,
	/// each where it is not the ASCII one the core reads anyway (U+2212 in et-EE and eu-ES;
	/// the bidi-marked signs of the Arabic-script cultures; <c>٪</c>).</description></item>
	/// <item><description>The first of <see cref="NumberFormatInfo.NativeDigits"/> as
	/// <see cref="DigitZero"/>, where it is not <c>0</c> (ar-SA's <c>٠</c>, fa-IR's <c>۰</c>,
	/// bn-BD's <c>০</c>).</description></item>
	/// <item><description><see cref="NumStyles.TrailingSign"/>, where the culture writes a
	/// negative number or amount with the sign after the digits:
	/// <see cref="NumberFormatInfo.NumberNegativePattern"/> 3 (<c>n-</c>) or 4 (<c>n -</c>),
	/// or a <see cref="NumberFormatInfo.CurrencyNegativePattern"/> of 3, 6, 7, 10, 11 or 13
	/// (<c>$n-</c>, <c>n-$</c>, <c>n$-</c>, <c>n $-</c>, <c>$ n-</c>, <c>n- $</c>;
	/// fy-NL writes <c>€ 5,00-</c>).</description></item>
	/// </list>
	/// The leniences are HyperCast's, not <c>TryParse</c>'s: every one is on, so percent and
	/// radix prefixes (<c>0x</c>, <c>&amp;H</c>, <c>0b</c>) are read where <c>TryParse</c> would
	/// reject them, and the ASCII spelling of every symbol is read beside the culture's.
	/// Narrow them on the result (<c>From(culture) with { Styles = … }</c>), or declare a
	/// <see cref="NumFormat"/> outright. Boolean words are not a format's business: the
	/// boolean door's lexicon is English, by decision, since no platform carries culture data
	/// for one — a caller maps its own words before the door.
	/// </remarks>
	/// <param name="numberFormat">The number formatting to derive from. Never null.</param>
	/// <exception cref="ArgumentNullException"><paramref name="numberFormat"/> is null.</exception>
	public static NumFormat From(NumberFormatInfo numberFormat)
	{
		ArgumentNullException.ThrowIfNull(numberFormat);
		// The number pair: the culture's where the core can read it, else the ASCII default;
		// and the two must differ, which a runtime's data does not promise (NLS gives
		// mn-Mong-MN a comma for both), so a group separator equal to the decimal one takes
		// the other of the ASCII pair, as the BCL itself then reads only the decimal.
		var decimalSeparator = Separator(numberFormat.NumberDecimalSeparator, ".");
		var groupSeparator = Separator(numberFormat.NumberGroupSeparator, ",");
		if (groupSeparator == decimalSeparator)
			groupSeparator = decimalSeparator == "," ? "." : ",";
		// The currency pair, where it differs from the number pair; a pair in force that
		// collides (a runtime's data again) declares nothing, and currency amounts read with
		// the number pair.
		var currencyDecimalSeparator = Declared(Separator(numberFormat.CurrencyDecimalSeparator, decimalSeparator), decimalSeparator);
		var currencyGroupSeparator = Declared(Separator(numberFormat.CurrencyGroupSeparator, groupSeparator), groupSeparator);
		if ((currencyDecimalSeparator.Length != 0 ? currencyDecimalSeparator : decimalSeparator)
			== (currencyGroupSeparator.Length != 0 ? currencyGroupSeparator : groupSeparator))
			currencyDecimalSeparator = currencyGroupSeparator = "";
		var styles = NumStyles.All;
		if (numberFormat.NumberNegativePattern is 3 or 4 || numberFormat.CurrencyNegativePattern is 3 or 6 or 7 or 10 or 11 or 13)
			styles |= NumStyles.TrailingSign;
		var digits = FirstNativeDigit(numberFormat);
		return new(decimalSeparator, groupSeparator, styles, Symbol(numberFormat.CurrencySymbol, "", MaxCurrencyBytes))
		{
			CurrencyDecimalSeparator = currencyDecimalSeparator,
			CurrencyGroupSeparator = currencyGroupSeparator,
			MinusSign = Symbol(numberFormat.NegativeSign, "-", MaxSymbolBytes),
			PlusSign = Symbol(numberFormat.PositiveSign, "+", MaxSymbolBytes),
			PercentSymbol = Symbol(numberFormat.PercentSymbol, "%", MaxSymbolBytes),
			DigitZero = digits.Length != 0 && digits[0] != '0' && Rune.TryGetRuneAt(digits, 0, out var zero) && StartsDigitBlock(zero)
				? zero
				: null,
		};
	}

	// A culture's spelling where it differs from the one the core reads regardless.
	// Declaring the ASCII default again would be harmless, but a format that declares
	// nothing past its separators is the one the core checks fastest, and most do.
	static string Declared(string symbol, string ascii) => symbol == ascii ? "" : symbol;

	// A culture's separator where the core can read it, else `fallback`: one to
	// MaxSymbolBytes bytes of UTF-8 with no ASCII digit (whitespace is a separator some
	// cultures declare, and is read as one).
	static string Separator(string separator, string fallback) =>
		Declarable(separator, MaxSymbolBytes, flush: false) ? separator : fallback;

	// A culture's sign, percent or currency symbol where it differs from the ASCII one the
	// core reads regardless and the core can read it, else nothing: the bridge never throws
	// on a runtime's data, however a culture is spelled there.
	static string Symbol(string symbol, string ascii, int maxBytes) =>
		symbol != ascii && Declarable(symbol, maxBytes, flush: true) ? symbol : "";

	// Whether `Encode` would accept the symbol: the same rules, answered rather than thrown,
	// for a bridge that must map every culture a runtime ships.
	internal static bool Declarable(string symbol, int maxBytes, bool flush)
	{
		if (symbol.Length == 0)
			return false;
		if (symbol.Length == 1 && char.IsAscii(symbol[0]))
			return !char.IsAsciiDigit(symbol[0]) && !(flush && IsAsciiWhitespace(symbol[0]));
		var bytes = 0;
		for (var i = 0; i < symbol.Length; i++)
		{
			var c = symbol[i];
			if (char.IsAsciiDigit(c) || (flush && IsAsciiWhitespace(c)))
				return false;
			if (char.IsSurrogate(c))
			{
				if (!(char.IsHighSurrogate(c) && ++i < symbol.Length && char.IsLowSurrogate(symbol[i])))
					return false;
				bytes += 4;
			}
			else
				bytes += c < 0x80 ? 1 : c < 0x800 ? 2 : 3;
		}
		return bytes <= maxBytes;
	}

	internal static bool IsAsciiWhitespace(char c) => c is ' ' or '\t' or '\n' or '\f' or '\r';

	// A zero the core can start a digit block at: outside ASCII, and with ten successors
	// that share its UTF-8 prefix, which every Unicode digit block has.
	static bool StartsDigitBlock(Rune zero) => !zero.IsAscii && zero.Value % 64 <= 54;

	// The culture's zero, read without the copy the public getter makes:
	// NumberFormatInfo.NativeDigits clones its ten strings on every read, and the provider
	// doors bridge on every call — 104 bytes per cast where nothing else on the crossing
	// allocates. UnsafeAccessor binds to the BCL's own field (named so since .NET Core 1.0,
	// on CoreCLR, Native AOT and Mono alike) at no cost. Should a runtime ever rename it,
	// the first read raises once and every read after takes the public getter: the bridge
	// stays right, and only the allocation is back.
	static string FirstNativeDigit(NumberFormatInfo numberFormat)
	{
		if (!_cloneNativeDigits)
		{
			try
			{
				return NativeDigits(numberFormat)[0];
			}
			catch (MissingMemberException)
			{
				_cloneNativeDigits = true;
			}
		}
		return numberFormat.NativeDigits[0];
	}

	static volatile bool _cloneNativeDigits;

	[UnsafeAccessor(UnsafeAccessorKind.Field, Name = "_nativeDigits")]
	static extern ref string[] NativeDigits(NumberFormatInfo numberFormat);

	/// <summary>
	/// Validates and converts to the native core's 92-byte layout — what every numeric door
	/// does with its format argument, public for a library that declares numeric columns
	/// across a C ABI of its own.
	/// </summary>
	/// <exception cref="ArgumentException">
	/// A contract violation — a caller bug, not a data verdict: a number separator missing
	/// or past <see cref="MaxSymbolBytes"/> bytes, the two equal, or the currency pair in
	/// force equal; any symbol past its buffer, carrying an ASCII digit, or not whole code
	/// points (a lone UTF-16 surrogate); ASCII whitespace in a sign, the percent or the
	/// currency symbol; a <see cref="DigitZero"/> that starts no digit block.
	/// </exception>
	public Interop.RawNumFormat ToRaw()
	{
		var decimalSeparator = DecimalSeparator;
		var groupSeparator = GroupSeparator;
		if (decimalSeparator.Length == 0 || groupSeparator.Length == 0)
			throw new ArgumentException("Decimal and group separators must both be declared.", DoorParameter);
		if (decimalSeparator == groupSeparator)
			throw new ArgumentException(
				$"Decimal and group separators must differ; both are '{decimalSeparator}'.", DoorParameter);
		var currencySymbol = CurrencySymbol;
		// The fast lane: one ASCII character per separator, the currency symbol one or none,
		// nothing else declared — the invariant profile and most Latin-script cultures. Decided
		// and written here without the encoder, and without lending the layout's address to
		// any call, which is what lets the JIT keep it off the slow path below.
		if (decimalSeparator.Length == 1 && groupSeparator.Length == 1 && currencySymbol.Length <= 1
			&& char.IsAscii(decimalSeparator[0]) && char.IsAscii(groupSeparator[0])
			&& (currencySymbol.Length == 0 || char.IsAscii(currencySymbol[0]))
			&& CurrencyDecimalSeparator.Length == 0 && CurrencyGroupSeparator.Length == 0
			&& MinusSign.Length == 0 && PlusSign.Length == 0 && PercentSymbol.Length == 0 && DigitZero is null)
		{
			Check(decimalSeparator[0], decimalSeparator, "Decimal separator", flush: false);
			Check(groupSeparator[0], groupSeparator, "Group separator", flush: false);
			var fast = new Interop.RawNumFormat
			{
				Size = (uint)Unsafe.SizeOf<Interop.RawNumFormat>(),
				Flags = (uint)Styles,
			};
			fast.DecimalSep.Bytes[0] = (byte)decimalSeparator[0];
			fast.DecimalSep.Length = 1;
			fast.GroupSep.Bytes[0] = (byte)groupSeparator[0];
			fast.GroupSep.Length = 1;
			if (currencySymbol.Length != 0)
			{
				Check(currencySymbol[0], currencySymbol, "Currency symbol", flush: true);
				fast.Currency.Bytes[0] = (byte)currencySymbol[0];
				fast.Currency.Length = 1;
			}
			return fast;
		}
		var raw = new Interop.RawNumFormat
		{
			Size = (uint)Unsafe.SizeOf<Interop.RawNumFormat>(),
			Flags = (uint)Styles,
		};
		raw.DecimalSep.Length = Encode(decimalSeparator, raw.DecimalSep.Bytes, "Decimal separator", flush: false);
		raw.GroupSep.Length = Encode(groupSeparator, raw.GroupSep.Bytes, "Group separator", flush: false);
		// Everything below is declared by few formats, and each empty field costs one length check.
		var currencyDecimalSeparator = CurrencyDecimalSeparator;
		var currencyGroupSeparator = CurrencyGroupSeparator;
		if (currencyDecimalSeparator.Length != 0 || currencyGroupSeparator.Length != 0)
		{
			var decimalInForce = currencyDecimalSeparator.Length != 0 ? currencyDecimalSeparator : decimalSeparator;
			var groupInForce = currencyGroupSeparator.Length != 0 ? currencyGroupSeparator : groupSeparator;
			if (decimalInForce == groupInForce)
				throw new ArgumentException(
					$"Currency decimal and group separators must differ; both are '{decimalInForce}'.", DoorParameter);
			raw.CurrencyDecimalSep.Length = Encode(currencyDecimalSeparator, raw.CurrencyDecimalSep.Bytes, "Currency decimal separator", flush: false);
			raw.CurrencyGroupSep.Length = Encode(currencyGroupSeparator, raw.CurrencyGroupSep.Bytes, "Currency group separator", flush: false);
		}
		if (MinusSign is { Length: > 0 } minusSign)
			raw.MinusSign.Length = Encode(minusSign, raw.MinusSign.Bytes, "Minus sign", flush: true);
		if (PlusSign is { Length: > 0 } plusSign)
			raw.PlusSign.Length = Encode(plusSign, raw.PlusSign.Bytes, "Plus sign", flush: true);
		if (PercentSymbol is { Length: > 0 } percentSymbol)
			raw.Percent.Length = Encode(percentSymbol, raw.Percent.Bytes, "Percent symbol", flush: true);
		if (currencySymbol.Length != 0)
			raw.Currency.Length = Encode(currencySymbol, raw.Currency.Bytes, "Currency symbol", flush: true);
		if (DigitZero is { } zero)
		{
			if (!StartsDigitBlock(zero))
				throw new ArgumentException($"Digit zero U+{zero.Value:X4} starts no digit block.", DoorParameter);
			raw.DigitZero = (uint)zero.Value;
		}
		return raw;
	}

	// Encodes one declared symbol into its buffer and returns its length, validating as the
	// core's own check does so a contract violation is an ArgumentException here and never
	// a -1 from the core: at most the buffer's bytes, whole code points (a lone surrogate
	// would transcode to U+FFFD and read as a symbol nobody declared), no ASCII digit, and
	// no ASCII whitespace where `flush` says the symbol sits against the digits. Empty
	// declares nothing and is length 0; a one-character ASCII symbol, which nearly every
	// separator is, is decided here, inline, without the encoder. DateFormat's designators
	// encode the same way.
	[MethodImpl(MethodImplOptions.AggressiveInlining)]
	internal static byte Encode(string symbol, Span<byte> bytes, string what, bool flush)
	{
		if (symbol.Length == 0)
			return 0;
		if (symbol.Length == 1 && char.IsAscii(symbol[0]))
		{
			Check(symbol[0], symbol, what, flush);
			bytes[0] = (byte)symbol[0];
			return 1;
		}
		return Transcode(symbol, bytes, what, flush);
	}

	[MethodImpl(MethodImplOptions.NoInlining)]
	static byte Transcode(string symbol, Span<byte> bytes, string what, bool flush)
	{
		for (var i = 0; i < symbol.Length; i++)
		{
			var c = symbol[i];
			Check(c, symbol, what, flush);
			if (char.IsSurrogate(c) && !(char.IsHighSurrogate(c) && ++i < symbol.Length && char.IsLowSurrogate(symbol[i])))
				throw new ArgumentException($"{what} '{symbol}' must be whole code points, not surrogate halves.", DoorParameter);
		}
		if (!Encoding.UTF8.TryGetBytes(symbol, bytes, out var written))
			throw new ArgumentException($"{what} '{symbol}' exceeds {bytes.Length} UTF-8 bytes.", DoorParameter);
		return (byte)written;
	}

	// The two checks every symbol passes, with the throws out of line so the check itself
	// inlines to two compares.
	[MethodImpl(MethodImplOptions.AggressiveInlining)]
	static void Check(char c, string symbol, string what, bool flush)
	{
		if (char.IsAsciiDigit(c))
			ThrowDigit(symbol, what);
		if (flush && c is ' ' or '\t' or '\n' or '\f' or '\r')
			ThrowWhitespace(symbol, what);
	}

	[MethodImpl(MethodImplOptions.NoInlining)]
	static void ThrowDigit(string symbol, string what) =>
		throw new ArgumentException($"{what} '{symbol}' must not contain a digit.", DoorParameter);

	[MethodImpl(MethodImplOptions.NoInlining)]
	static void ThrowWhitespace(string symbol, string what) =>
		throw new ArgumentException($"{what} '{symbol}' must not contain whitespace.", DoorParameter);
}
