from typing import Any

def compile(
    manifest: dict[str, Any],
    tables: list[dict[str, Any]],
    theme: dict[str, Any] | str | None = None,
) -> CompiledModel: ...

class CompiledModel:
    def emit_workbook(self, options: dict[str, Any] | None = None) -> bytes: ...
    def summary(self) -> dict[str, Any]: ...
    def inspect_cell(
        self,
        target: str,
        row: str | None = None,
        col: str | None = None,
    ) -> dict[str, Any]: ...
    def inspect_table(self, table: str) -> dict[str, Any]: ...
    def inspect_sheet(self, sheet: str) -> dict[str, Any]: ...
    def precedents_of(
        self,
        target: str,
        row: str | None = None,
        col: str | None = None,
        recursive: bool = False,
    ) -> dict[str, Any]: ...
    def dependents_of(
        self,
        target: str,
        row: str | None = None,
        col: str | None = None,
        recursive: bool = False,
    ) -> dict[str, Any]: ...

class F8Error(Exception):
    message: str

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

class EmissionError(F8Error):
    pass
