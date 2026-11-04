"""Strict digit grouping: the policy a ``NumFormat`` declares beside its leniences, the default
(strict Western) and its escape hatches (legacy, Indian), what ``INVARIANT``, ``DETECT`` and the
locale bridge declare, and the refusals. The corpus rows that name a policy are replayed in
test_corpus.py."""

from __future__ import annotations

import locale
import subprocess
from decimal import Decimal

import pytest

import hypercast
from hypercast import CastFailure, Detail, Fault, NumFormat, Success

ALL = NumFormat.ALL
#: The platform's own: CPython reports a localeconv() size list ended by it, 255 where ``char`` is
#: unsigned (aarch64 Linux) and 127 elsewhere.
CHAR_MAX = locale.CHAR_MAX
LEGACY, WESTERN, INDIAN = (
    NumFormat.GROUPING_LEGACY,
    NumFormat.GROUPING_WESTERN,
    NumFormat.GROUPING_INDIAN,
)


def _format(grouping: int | None = None, flags: int = ALL, **keywords) -> NumFormat:
    if grouping is None:
        return NumFormat(".", ",", flags, **keywords)
    return NumFormat(".", ",", flags, grouping=grouping, **keywords)


def test_the_policy_constants_are_bits_eight_and_nine():
    """GROUPING_LEGACY is 0, WESTERN is 1 << 8, INDIAN is 2 << 8, and the mask is both bits; the
    lenience flags (ALL and each flag) never carry any of them."""
    assert (LEGACY, WESTERN, INDIAN) == (0, 1 << 8, 2 << 8)
    assert NumFormat.GROUPING_POLICY == 3 << 8
    for flag in (
        ALL,
        NumFormat.GROUPING,
        NumFormat.PARENTHESES,
        NumFormat.EXPONENT,
        NumFormat.RADIX_PREFIXES,
        NumFormat.PERCENT,
        NumFormat.CURRENCY,
        NumFormat.TRAILING_SIGN,
        NumFormat.SEPARATOR_DETECT,
    ):
        assert flag & NumFormat.GROUPING_POLICY == 0


def test_a_new_format_is_strict_western_unless_it_declares_otherwise():
    """The constructor's policy defaults to strict Western: ``1,234`` reads, ``12,34`` is a
    malformed group, and an ungrouped numeral stays valid."""
    fmt = NumFormat(".", ",", ALL)
    assert fmt.grouping == WESTERN
    assert hypercast.cast_i32("1,234", fmt) == Success(1234)
    assert hypercast.cast_i32("1234", fmt) == Success(1234)
    assert hypercast.cast_i32("12,34", fmt) == Fault(CastFailure.MALFORMED, 2, 3)
    verdict = hypercast.cast_i32("12,34", fmt)
    assert isinstance(verdict, Fault) and verdict.detail is Detail.GROUP_SIZE
    # Once a numeral groups, every group conforms; the leftmost invalid one wins.
    assert hypercast.cast_i32("1234,567", fmt) == Fault(CastFailure.MALFORMED, 0, 4)


def test_the_named_formats_are_strict_western():
    """INVARIANT and DETECT declare strict Western, so what a caller who never names a policy
    gets is the strict reading."""
    for fmt in (NumFormat.INVARIANT, NumFormat.DETECT):
        assert fmt.grouping == WESTERN
        assert hypercast.cast_i32("12,34,567", fmt) == Fault(CastFailure.MALFORMED, 2, 3)
    assert NumFormat.INVARIANT.flags == ALL
    assert NumFormat.DETECT.flags == ALL | NumFormat.SEPARATOR_DETECT
    # Detection stays independent of the policy: a lone ambiguous group is still ambiguous.
    assert hypercast.cast_i32("1,234", NumFormat.DETECT) == Fault(CastFailure.MALFORMED, 1, 1)


def test_legacy_is_an_explicit_option_and_reads_a_group_anywhere():
    """The escape hatch: legacy reads a separator anywhere between two digits, as before strict
    grouping."""
    legacy = _format(LEGACY)
    assert legacy.grouping == LEGACY
    assert hypercast.cast_i32("12,34", legacy) == Success(1234)
    assert hypercast.cast_i32("1,2,3", legacy) == Success(123)
    assert hypercast.cast_i32("12,34", _format(WESTERN)) == Fault(CastFailure.MALFORMED, 2, 3)


def test_indian_is_an_explicit_option_and_reads_threes_then_twos():
    """Indian: the group beside the decimal separator holds three digits, every group left of it
    two, the leftmost one or two."""
    indian = _format(INDIAN)
    assert indian.grouping == INDIAN
    assert hypercast.cast_i64("12,34,567", indian) == Success(1234567)
    assert hypercast.cast_i64("1,23,456", indian) == Success(123456)
    assert hypercast.cast_i64("1,234", indian) == Success(1234)
    assert hypercast.cast_f64("12,34,567.5", indian) == Success(1234567.5)
    for refused in ("123,456", "1,234,567", "12,34"):
        verdict = hypercast.cast_i64(refused, indian)
        assert isinstance(verdict, Fault) and verdict.detail is Detail.GROUP_SIZE, refused


def test_grouping_leaves_the_fraction_and_the_exponent_alone():
    """No group separator after the decimal separator or in an exponent, under a strict policy."""
    western = _format(WESTERN)
    verdict = hypercast.cast_f64("1.5,000", western)
    assert isinstance(verdict, Fault) and verdict.detail is Detail.GROUP_IN_FRACTION
    verdict = hypercast.cast_f64("1e1,000", western)
    assert isinstance(verdict, Fault) and verdict.detail is Detail.GROUP_IN_FRACTION


def test_a_strict_policy_does_not_enable_grouping():
    """With the GROUPING lenience off a group separator stays forbidden, whatever the policy."""
    for grouping in (WESTERN, INDIAN):
        fmt = _format(grouping, flags=ALL & ~NumFormat.GROUPING)
        verdict = hypercast.cast_i32("1,234", fmt)
        assert verdict == Fault(CastFailure.MALFORMED, 1, 1)
        assert isinstance(verdict, Fault) and verdict.detail is Detail.UNEXPECTED_CHARACTER


def test_native_digits_and_a_space_like_separator_are_counted_as_digits():
    """With Arabic-Indic digits and U+202F as the group separator, ``١ ٢٣٤`` is 1234, and
    ``١٢ ٣٤`` is a malformed group spanning the separator and its two digits, counted in digits
    (code points for a str), never in bytes."""
    fmt = NumFormat(".", "\u202f", ALL, digit_zero="\u0660")
    assert hypercast.cast_i32("\u0661\u202f\u0662\u0663\u0664", fmt) == Success(1234)
    assert hypercast.cast_i32("\u0661\u0662\u202f\u0663\u0664", fmt) == Fault(
        CastFailure.MALFORMED, 2, 3
    )
    as_bytes = "\u0661\u0662\u202f\u0663\u0664".encode()
    verdict = hypercast.cast_i32(as_bytes, fmt)
    assert verdict == Fault(CastFailure.MALFORMED, 4, 7)
    assert isinstance(verdict, Fault) and verdict.detail is Detail.GROUP_SIZE


def test_the_currency_group_separator_follows_the_policy():
    """A currency amount's own group separator is judged by the same policy."""
    fmt = NumFormat(",", ".", ALL, "€", currency_decimal_sep=".", currency_group_sep=",")
    assert hypercast.cast_decimal("€1,234.5", fmt) == Success(Decimal("1234.5"))
    verdict = hypercast.cast_decimal("€12,34.5", fmt)
    assert isinstance(verdict, Fault) and verdict.detail is Detail.GROUP_SIZE


def test_the_policy_is_replaced_never_ored_and_flags_carry_none():
    """``flags`` reads back the leniences alone and ``grouping`` the policy; the packed layout
    carries both in its flags word; ``repr`` builds the format again for every policy."""
    for grouping in (LEGACY, WESTERN, INDIAN):
        fmt = _format(grouping, NumFormat.ALL | NumFormat.TRAILING_SIGN)
        assert fmt.flags == NumFormat.ALL | NumFormat.TRAILING_SIGN
        assert fmt.grouping == grouping
        assert fmt.packed[4:8] == (fmt.flags | grouping).to_bytes(4, "little")
        again = eval(repr(fmt), {"NumFormat": NumFormat})  # noqa: S307
        assert again.packed == fmt.packed
    assert repr(NumFormat(".", ",", ALL)) == "NumFormat('.', ',', 95, '')"
    assert repr(_format(LEGACY)).endswith(", grouping=NumFormat.GROUPING_LEGACY)")
    assert repr(_format(INDIAN)).endswith(", grouping=NumFormat.GROUPING_INDIAN)")


def test_a_policy_is_refused_where_it_names_none_or_hides_in_the_flags():
    """Policy ``11``, any other bit pattern in ``grouping``, a policy carried in ``flags``, and a
    flag bit this version does not define are each a caller's ``ValueError``, worded by rule."""
    for grouping in (NumFormat.GROUPING_POLICY, 1, ALL, WESTERN | 1, 4 << 8):
        with pytest.raises(ValueError, match="Grouping must be"):
            _format(grouping)
    for flags in (ALL | WESTERN, INDIAN, NumFormat.GROUPING_POLICY):
        with pytest.raises(ValueError, match="must not carry grouping bits"):
            _format(None, flags)
    for flags in (1 << 10, 1 << 31, ALL | 1 << 12):
        with pytest.raises(ValueError, match="does not define"):
            _format(None, flags)
    # The policy rule wins over an undefined flag, as the core's own check orders them.
    with pytest.raises(ValueError, match="Grouping must be"):
        _format(NumFormat.GROUPING_POLICY, 1 << 10)
    # A policy is a u32, and a wrong type is the TypeError argument extraction raises.
    with pytest.raises(TypeError):
        NumFormat(".", ",", ALL, grouping="western")
    # A policy no 32 bits hold is no policy: a refused declaration like any other.
    for wide in (-1, 2**32, 2**70, -(2**70)):
        with pytest.raises(ValueError, match="Grouping must be"):
            NumFormat(".", ",", ALL, grouping=wide)


# --- the locale bridge: the platform's own digit-group patterns, never its name --------------
#
# A pattern is C's: sizes from the decimal separator outward, a trailing 0 repeating the last
# size, the platform's CHAR_MAX ending grouping, no size at all (``[]``, ``[CHAR_MAX]``, ``[0]``)
# not grouping.
# One format has one policy and one GROUPING flag for numbers and currency alike.

NONE, WESTERN_P, INDIAN_P, OTHER = [], [3, 0], [3, 2, 0], [2, 0]


def _bridge(number=None, currency=None) -> NumFormat:
    conv = {"decimal_point": ".", "thousands_sep": ","}
    if number is not None:
        conv["grouping"] = number
    if currency is not None:
        conv["mon_grouping"] = currency
    return NumFormat.from_localeconv(conv)


def _declares(fmt: NumFormat) -> tuple[int, bool]:
    """The policy and whether the GROUPING lenience is on."""
    return fmt.grouping, bool(fmt.flags & NumFormat.GROUPING)


@pytest.mark.parametrize(
    ("pattern", "declared"),
    [
        ([3, 2, 0], (INDIAN, True)),
        ((3, 2, 0), (INDIAN, True)),
        ([3, 2, 2, 0], (INDIAN, True)),
        ([3, 0], (WESTERN, True)),
        ([3, 3, 0], (WESTERN, True)),
        ([3, 3, 3, 0], (WESTERN, True)),
        # A list with no terminator is read as C reads a string that ends: as a 0, repeating.
        ([3], (WESTERN, True)),
        ([3, 2], (INDIAN, True)),
        # No size at all is no grouping: the flag goes, whatever the policy says.
        ([], (WESTERN, False)),
        ([CHAR_MAX], (WESTERN, False)),
        ([0], (WESTERN, False)),
        # Any other shape is legacy: groups of two (Toki Pona), a size that ends (CHAR_MAX
        # after a size), a size no group can have, a pattern that is neither threes nor 3, 2s.
        ([2, 0], (LEGACY, True)),
        ([2, 2, 0], (LEGACY, True)),
        ([4, 0], (LEGACY, True)),
        ([3, CHAR_MAX], (LEGACY, True)),
        ([3, 2, CHAR_MAX], (LEGACY, True)),
        ([3, 2, 3, 0], (LEGACY, True)),
        ([3, 3, 2, 0], (LEGACY, True)),
        ([1, 0], (LEGACY, True)),
        # Said nothing a pattern is: absent, or not a list of ints.
        ("3;2", (WESTERN, True)),
        (None, (WESTERN, True)),
    ],
)
def test_the_bridge_reads_a_number_pattern_as_c_defines_it(pattern, declared):
    """What each shape of ``grouping`` declares, with no currency pattern to weigh."""
    assert _declares(_bridge(pattern)) == declared


@pytest.mark.parametrize(
    ("number", "currency", "declared"),
    [
        # Agreeing patterns that group.
        (WESTERN_P, WESTERN_P, (WESTERN, True)),
        (INDIAN_P, INDIAN_P, (INDIAN, True)),
        (OTHER, OTHER, (LEGACY, True)),
        # Enabled patterns that disagree, or one of another shape: legacy.
        (WESTERN_P, INDIAN_P, (LEGACY, True)),
        (INDIAN_P, WESTERN_P, (LEGACY, True)),
        (WESTERN_P, OTHER, (LEGACY, True)),
        (OTHER, INDIAN_P, (LEGACY, True)),
        # One that does not group is not weighed: the other decides, and the flag stays.
        (INDIAN_P, NONE, (INDIAN, True)),
        (NONE, INDIAN_P, (INDIAN, True)),
        (WESTERN_P, [CHAR_MAX], (WESTERN, True)),
        ([CHAR_MAX], WESTERN_P, (WESTERN, True)),
        (OTHER, NONE, (LEGACY, True)),
        (INDIAN_P, None, (INDIAN, True)),
        (None, WESTERN_P, (WESTERN, True)),
        # Neither groups: GROUPING is cleared.
        (NONE, NONE, (WESTERN, False)),
        ([CHAR_MAX], [0], (WESTERN, False)),
        (NONE, None, (WESTERN, False)),
        (None, [CHAR_MAX], (WESTERN, False)),
        # Nothing said at all declares nothing: the constructor's default.
        (None, None, (WESTERN, True)),
    ],
)
def test_the_bridge_weighs_the_patterns_that_group(number, currency, declared):
    """One policy and one flag for both kinds of value, over the patterns that group."""
    assert _declares(_bridge(number, currency)) == declared


def test_the_fallback_is_a_function_of_the_patterns_alone():
    """Never inferred from input: the same patterns declare the same format, and the bridge
    reads nothing but the dict it is given."""
    one = _bridge(INDIAN_P, WESTERN_P)
    two = _bridge(INDIAN_P, WESTERN_P)
    assert one.packed == two.packed
    assert one.grouping == LEGACY


# The actual patterns, as C reports what ICU and glibc publish for each locale.
ACTUAL = {
    "tok_001 (groups of two)": ([2, 0], [2, 0], (LEGACY, True)),
    "en-US-POSIX (secondary size 0: repeat)": ([3, 0], [3, 0], (WESTERN, True)),
    "en-BG (no currency grouping)": ([3, 0], [CHAR_MAX], (WESTERN, True)),
    "en-IN": ([3, 2, 0], [3, 2, 0], (INDIAN, True)),
    "ml / ta / mr / or (Indian numbers, Western currency)": ([3, 2, 0], [3, 0], (LEGACY, True)),
    "C (nothing groups)": ([], [], (WESTERN, False)),
}


@pytest.mark.parametrize("name", ACTUAL)
def test_the_actual_patterns_declare_what_the_owner_rule_says(name):
    """tok, en-US-POSIX, en-BG, en-IN and ml/ta, and the C locale."""
    number, currency, declared = ACTUAL[name]
    assert _declares(_bridge(number, currency)) == declared


def test_the_declared_format_reads_the_values_the_platform_formats():
    """The point of the rule: the bridge never rejects its platform's own output. Toki Pona's
    twos read (legacy), ml's Indian numbers and Western currency both read, en-BG reads an
    ungrouped currency and a grouped one (the flag stays for the number), and a locale that
    groups nothing reads ungrouped values."""
    tok = _bridge([2, 0], [2, 0])
    assert hypercast.cast_i64("12,34,56,78", tok) == Success(12345678)
    ml = NumFormat.from_localeconv(
        {
            "decimal_point": ".",
            "thousands_sep": ",",
            "currency_symbol": "\u20b9",
            "grouping": [3, 2, 0],
            "mon_grouping": [3, 0],
        }
    )
    assert hypercast.cast_decimal("12,34,567.5", ml) == Success(Decimal("1234567.5"))
    assert hypercast.cast_decimal("\u20b91,234,567.50", ml) == Success(Decimal("1234567.5"))
    bg = NumFormat.from_localeconv(
        {
            "decimal_point": ".",
            "thousands_sep": ",",
            "currency_symbol": "\u043b\u0432.",
            "grouping": [3, 0],
            "mon_grouping": [CHAR_MAX],
        }
    )
    assert hypercast.cast_decimal("1,234,567.5", bg) == Success(Decimal("1234567.5"))
    assert hypercast.cast_decimal("1234567.50 \u043b\u0432.", bg) == Success(Decimal("1234567.5"))
    # The format cannot forbid currency grouping on its own: a grouped currency still reads.
    assert hypercast.cast_decimal("1,234,567.50 \u043b\u0432.", bg) == Success(Decimal("1234567.5"))
    assert isinstance(hypercast.cast_decimal("12,34,567.5", bg), Fault)
    none = _bridge([], [])
    assert not none.flags & NumFormat.GROUPING
    assert hypercast.cast_i64("1234567", none) == Success(1234567)
    assert isinstance(hypercast.cast_i64("1,234", none), Fault)
    indian = _bridge(INDIAN_P, NONE)
    assert hypercast.cast_i64("12,34,567", indian) == Success(1234567)
    assert isinstance(hypercast.cast_i64("1,234,567", indian), Fault)


def test_a_locale_that_disagrees_with_itself_is_no_longer_a_value_error():
    """The earlier rule raised for patterns needing two policies; the bridge never rejects what
    its platform writes, so it declares legacy instead."""
    fmt = _bridge(INDIAN_P, WESTERN_P)
    assert fmt.grouping == LEGACY
    assert hypercast.cast_decimal("12,34,567.5", fmt) == Success(Decimal("1234567.5"))
    assert hypercast.cast_decimal("1,234,567.5", fmt) == Success(Decimal("1234567.5"))


def _each_locale():
    """Every locale ``setlocale`` accepts, with the process's locale put back after."""
    saved = locale.setlocale(locale.LC_ALL)
    try:
        for name in ["C", "POSIX", *_installed()]:
            try:
                locale.setlocale(locale.LC_ALL, name)
            except locale.Error:
                continue
            yield name
    finally:
        locale.setlocale(locale.LC_ALL, saved)


def _shape_of(pattern) -> str:
    """The shape of a real ``localeconv()`` pattern, independently of the bridge: what its
    sizes make of a seven-digit numeral."""
    sizes, repeating = list(pattern), True
    if sizes and sizes[-1] in (0, CHAR_MAX):
        repeating = sizes.pop() == 0
    if not sizes:
        return "none"
    if repeating and all(size == 3 for size in sizes):
        return "western"
    if repeating and sizes[0] == 3 and len(sizes) > 1 and all(size == 2 for size in sizes[1:]):
        return "indian"
    return "other"


def test_the_bridge_declares_what_every_locale_patterns_say():
    """Swept over each locale ``setlocale`` accepts: the declaration follows the rule from the
    locale's own two patterns, and no locale raises."""
    swept = 0
    for name in _each_locale():
        swept += 1
        conv = locale.localeconv()
        shapes = {_shape_of(conv["grouping"]), _shape_of(conv["mon_grouping"])} - {"none"}
        expected = (WESTERN, False)
        if shapes:
            expected = (
                {"western": WESTERN, "indian": INDIAN}.get(next(iter(shapes)), LEGACY)
                if len(shapes) == 1
                else LEGACY,
                True,
            )
        try:
            fmt = NumFormat.from_localeconv()
        except ValueError as err:  # pragma: no cover - the sweep's whole point
            pytest.fail(f"from_localeconv() threw under {name}: {err}")
        assert _declares(fmt) == expected, (name, conv["grouping"], conv["mon_grouping"])
    assert swept >= 1


def test_the_bridge_reads_the_values_every_locale_formats():
    """For each locale the machine has, the bridge's format reads that locale's own formatted
    numbers (``locale.format_string(..., grouping=True)``, negative too) and currency amounts
    (``locale.currency(..., grouping=True)``, rounded to the locale's own fraction digits)."""
    checked = 0
    for name in _each_locale():
        fmt = NumFormat.from_localeconv()
        for value in (1234567.89, -1234567.89):
            number = locale.format_string("%.2f", value, grouping=True)
            assert hypercast.cast_decimal(number, fmt) == Success(Decimal(str(value))), (
                name,
                number,
            )
        whole = locale.format_string("%d", 1234567, grouping=True)
        assert hypercast.cast_i64(whole, fmt) == Success(1234567), (name, whole)
        checked += 1
        digits = locale.localeconv()["frac_digits"]
        # CHAR_MAX says the locale has no currency formatting. locale.currency only knows 127, so
        # where char is unsigned (aarch64 Linux) it would format the C locale's 255 as digits.
        if not 0 <= digits <= 10:
            continue
        for value in (1234567.89, -1234567.89):
            money = locale.currency(value, grouping=True)
            want = Decimal(str(value)).quantize(Decimal(1).scaleb(-digits))
            assert hypercast.cast_decimal(money, fmt) == Success(want), (name, money)
    assert checked >= 1


def _installed() -> list[str]:
    try:
        listed = subprocess.run(["locale", "-a"], capture_output=True, text=True, check=True)
    except (OSError, subprocess.SubprocessError):
        return []
    return sorted(set(listed.stdout.split()))
