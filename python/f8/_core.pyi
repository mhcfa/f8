"""Native Rust compiler bindings (`f8._core`)."""

from collections.abc import Sequence
from pathlib import Path

from f8.audit import TableAuditor
from f8.types import (
    CellInspection,
    CellScalarValue,
    DependencyTree,
    EmissionOptions,
    ManifestInput,
    ModelSummary,
    SheetInspection,
    TableInput,
    TableInspection,
)

__version__: str

def compile(
    manifest: ManifestInput,
    tables: Sequence[TableInput],
) -> CompiledModel:
    """Compiles a project manifest and table specifications into an evaluated `CompiledModel`."""

class CompiledModel:
    """Compiled, verified, and evaluated financial model ready for auditing and `.xlsx` emission."""

    def emit_workbook(self, options: EmissionOptions | None = None) -> bytes:
        """Emits the compiled OpenXML (`.xlsx`) workbook into an in-memory `bytes` buffer."""

    def save(self, path: str | Path, options: EmissionOptions | None = None) -> None:
        """Emits and writes the compiled OpenXML (`.xlsx`) workbook to `path`."""

    def summary(self) -> ModelSummary:
        """Returns high-level structural and cell classification statistics for the model."""

    def value(
        self,
        target: str,
        row: str | None = None,
        col: str | None = None,
    ) -> CellScalarValue:
        """Returns the unboxed scalar value (`float | str | bool | None`) for a cell coordinate."""

    def num(
        self,
        target: str,
        row: str | None = None,
        col: str | None = None,
    ) -> float:
        """Returns the evaluated numeric cell value as a `float` (raises `TypeError` otherwise)."""

    def formatted(
        self,
        target: str,
        row: str | None = None,
        col: str | None = None,
        *,
        unit: str = "",
    ) -> str:
        """Returns the formatted display string for a coordinate with an optional unit suffix."""

    def inspect_cell(
        self,
        target: str,
        row: str | None = None,
        col: str | None = None,
    ) -> CellInspection:
        """Returns the cell inspection payload including formulas, formatting, and neighbors."""

    def inspect_table(self, table: str) -> TableInspection:
        """Returns the raw 2D `TableInspection` dictionary for `table`."""

    def table(self, table: str) -> TableAuditor:
        """Returns a high-level `TableAuditor` wrapper for inspecting and exporting `table`."""

    def inspect_sheet(self, sheet: str) -> SheetInspection:
        """Returns the physical grid `SheetInspection` dictionary for `sheet`."""

    def precedents_of(
        self,
        target: str,
        row: str | None = None,
        col: str | None = None,
        recursive: bool = False,
    ) -> DependencyTree:
        """Traces direct or recursive upstream precedent cells for the target coordinate."""

    def dependents_of(
        self,
        target: str,
        row: str | None = None,
        col: str | None = None,
        recursive: bool = False,
    ) -> DependencyTree:
        """Traces direct or recursive downstream dependent cells for the target coordinate."""

class F8Error(Exception):
    """Base exception for all f8 compiler, evaluation, inspection, and emission errors."""

    message: str
    def __init__(self, message: str = ...) -> None: ...

class CycleDetectedError(F8Error):
    """Raised when the dependency graph contains a circular cell reference."""

    cycle: list[str]

class ReferenceResolutionError(F8Error):
    """Raised when a formula coordinate or range reference cannot be resolved."""

    coordinate: str
    target_table: str | None
    row: str | None
    col: str | None

class SpecificationError(F8Error):
    """Raised when a model manifest or table specification violates schema or IR invariants."""

    table: str | None
    reason: str

class FormulaParseError(F8Error):
    """Raised when an f8 DSL expression fails lexical or syntactic parsing."""

    table: str
    row: str | None
    col: str | None
    token_offset: int | None

class InspectionError(F8Error):
    """Raised when an inspected sheet, table, or cell coordinate does not exist."""

    entity: str

class EmissionError(F8Error):
    """Raised when OpenXML (.xlsx) workbook generation or file I/O fails."""
