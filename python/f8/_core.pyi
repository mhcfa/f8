from collections.abc import Sequence
from pathlib import Path

from f8.audit import TableAuditor
from f8.types import (
    CellInspection,
    DependencyTree,
    EmissionOptions,
    ManifestInput,
    ModelSummary,
    SheetInspection,
    TableInput,
    TableInspection,
    ThemeInput,
)

def compile(
    manifest: ManifestInput,
    tables: Sequence[TableInput],
    theme: ThemeInput | None = None,
) -> CompiledModel: ...

class CompiledModel:
    def emit_workbook(self, options: EmissionOptions | None = None) -> bytes: ...
    def save(self, path: str | Path, options: EmissionOptions | None = None) -> None: ...
    def summary(self) -> ModelSummary: ...
    def inspect_cell(
        self,
        target: str,
        row: str | None = None,
        col: str | None = None,
    ) -> CellInspection: ...
    def inspect_table(self, table: str) -> TableInspection: ...
    def table(self, table: str) -> TableAuditor: ...
    def inspect_sheet(self, sheet: str) -> SheetInspection: ...
    def precedents_of(
        self,
        target: str,
        row: str | None = None,
        col: str | None = None,
        recursive: bool = False,
    ) -> DependencyTree: ...
    def dependents_of(
        self,
        target: str,
        row: str | None = None,
        col: str | None = None,
        recursive: bool = False,
    ) -> DependencyTree: ...

class F8Error(Exception):
    message: str
    def __init__(self, message: str = ...) -> None: ...

class CycleDetectedError(F8Error):
    cycle: list[str]

class ReferenceResolutionError(F8Error):
    coordinate: str
    target_table: str | None
    row: str | None
    col: str | None

class SpecificationError(F8Error):
    table: str | None
    reason: str

class FormulaParseError(F8Error):
    table: str
    row: str | None
    col: str | None
    token_offset: int | None

class InspectionError(F8Error):
    entity: str

class EmissionError(F8Error): ...
