"""Tests for the pure Python modeling DSL and builder classes."""

from __future__ import annotations

import pytest
from f8 import Cell, CellInputValue


def test_scenario_9_cell_payload_validation_and_sentinel_nulls() -> None:
    # Value cell
    c1 = Cell(100.0)
    assert c1.value == 100.0
    assert c1.expr is None

    # Formula cell
    c2 = Cell(expr="PREV * 1.05")
    assert c2.expr == "PREV * 1.05"

    # Explicit null value
    c3 = Cell(value=None)
    assert c3.value is None

    # Cell with presentation metadata
    c4 = Cell(
        42.0,
        style="total",
        highlight=True,
        format="currency",
        decimals=2,
        currency="USD",
        indent=1,
        note="Source: audit",
    )
    assert c4.value == 42.0
    assert c4.style == "total"
    assert c4.highlight is True
    assert c4.format == "currency"
    assert c4.decimals == 2
    assert c4.currency == "USD"
    assert c4.indent == 1
    assert c4.note == "Source: audit"

    # Both value and expr raises ValueError
    with pytest.raises(ValueError, match="both"):
        Cell(100.0, expr="A + B")

    # Neither value nor expr raises ValueError
    with pytest.raises(ValueError, match="either"):
        Cell()


def test_cell_input_value_runtime_evaluation() -> None:
    from f8.builder import CellInputValue as BuilderCellInputValue
    from f8.types import CellInputValue as TypesCellInputValue

    assert CellInputValue is BuilderCellInputValue
    assert TypesCellInputValue is BuilderCellInputValue
    assert BuilderCellInputValue.__value__ is not None
