namespace HyperCast;

/// <summary>
/// The date system an Excel serial number is expressed in. Spreadsheets carry no marker for
/// this — it is a workbook-level setting — so the caller states it, the same way
/// <see cref="UnixPrecision"/> and <see cref="DateOrder"/> are declared rather than guessed.
/// Values match the native core's discriminants.
/// </summary>
public enum ExcelEpoch : uint
{
	/// <summary>Sentinel CLR default — never a valid epoch; rejected by the Excel door.</summary>
	Unspecified = 0,

	/// <summary>
	/// The 1900 system (the Windows default): serial <c>1</c> is 1900-01-01, and serial
	/// <c>60</c> is a February 29th that never existed.
	/// </summary>
	Y1900 = 1,

	/// <summary>
	/// The 1904 system (legacy Macintosh workbooks, still selectable today): serial
	/// <c>0</c> is 1904-01-01, with no phantom day anywhere in it.
	/// </summary>
	Y1904 = 2
}
