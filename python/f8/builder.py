"""Fluent authoring DSL and in-memory specification synthesis."""

from __future__ import annotations

from dataclasses import dataclass

from f8.types import (
    CellValueLiteral,
    CurrencyCode,
    DetailedCellDict,
    FormatPreset,
    SemanticStyle,
)


class _Unset:
    """Sentinel type representing an unprovided value argument."""


_UNSET = _Unset()


@dataclass(frozen=True)
class Cell:
    """Detailed 0D cell override specification."""

    value: CellValueLiteral | _Unset = _UNSET
    expr: str | None = None
    style: SemanticStyle | None = None
    highlight: bool | None = None
    format: FormatPreset | None = None
    decimals: int | None = None
    currency: CurrencyCode | None = None
    indent: int | None = None
    note: str | None = None

    def __post_init__(self) -> None:
        has_value = self.value is not _UNSET
        has_expr = self.expr is not None
        if has_value and has_expr:
            raise ValueError("Cell cannot specify both 'value' and 'expr'")
        if not has_value and not has_expr:
            raise ValueError("Cell must specify either 'value' or 'expr'")


type CellInputValue = CellValueLiteral | Cell | DetailedCellDict

__all__ = ["Cell", "CellInputValue"]
