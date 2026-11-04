using System.Buffers;
using System.Text;

namespace HyperCast;

// The core's white space, restated: the 25 code points with the Unicode White_Space property
// (rust/src/text.rs holds the table; corpus/formats.json holds this one to it). U+200B,
// U+FEFF and U+180E are not white space. Used where the binding judges input on its own side
// of the crossing: the declaration validators, and the span `Cast.Char` computes.
internal static class WhiteSpace
{
	internal static bool IsWhiteSpace(int codePoint) =>
		codePoint is (>= 0x09 and <= 0x0D) or 0x20 or 0x85 or 0xA0 or 0x1680 or (>= 0x2000 and <= 0x200A)
			or 0x2028 or 0x2029 or 0x202F or 0x205F or 0x3000;

	// The bytes of the white-space code point `utf8` starts with, or 0. Only a complete, valid
	// encoding counts: a stray lead byte, a cut sequence or an overlong one is never white space.
	internal static int LeadingLength(ReadOnlySpan<byte> utf8) =>
		Rune.DecodeFromUtf8(utf8, out var rune, out var consumed) == OperationStatus.Done && IsWhiteSpace(rune.Value)
			? consumed
			: 0;

	// The same for the code point `utf8` ends with.
	internal static int TrailingLength(ReadOnlySpan<byte> utf8) =>
		Rune.DecodeLastFromUtf8(utf8, out var rune, out var consumed) == OperationStatus.Done && IsWhiteSpace(rune.Value)
			? consumed
			: 0;
}

// Where a declared symbol may hold white space. A sign, the percent symbol and a decimal
// separator never (the input's white space around them is ignored, and a numeral admits white
// space only as a group separator); a currency symbol inside but not at its edges (CLDR's
// `F CFA`); a group separator or a designator's interior anywhere the rule above leaves it.
internal enum Spaces : byte
{
	Allowed,
	Nowhere,
	AtEdges,
}
