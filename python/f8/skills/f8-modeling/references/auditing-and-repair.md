# Auditing and Self-Healing Reference

Specification for model inspection via `TableAuditor`, tabular data export, and automated error diagnostics using structured `F8Error` exceptions.

---

## 1. Inspecting Models with `TableAuditor`

Compiling a model produces an in-memory `CompiledModel` containing interactive auditors for each table:

```python
compiled = model.compile()
auditor = compiled.table(table_id)
```

### Coordinate Indexing

Direct 2D bracket indexing returns unboxed Python scalars (`float`, `int`, `str`, `bool`, or `None`):

```python
val = auditor[row_id, col_id]
```

### Inspection Methods & Properties

| Member | Return Type | Description |
| :--- | :--- | :--- |
| `auditor.shape` | `tuple[int, int]` | Dimensions `(row_count, col_count)` |
| `auditor.rows` | `list[str]` | List of evaluated data row IDs in layout order |
| `auditor.columns` | `list[str]` | List of column IDs in layout order |
| `auditor.expr(row, col)` | `str \| None` | Raw declarative `f8` vector expression |
| `auditor.formula(row, col)` | `str \| None` | Lowered physical Excel formula (e.g. `"=C5*0.2"`) |
| `auditor.address(row, col)` | `str \| None` | Physical Excel cell address (e.g. `"C6"`) |
| `auditor.value(row, col)` | `Any` | Evaluated scalar value |

---

## 2. Tabular Export & Visual Markdown

`TableAuditor` provides zero-dependency formatted table generation and tabular dictionary export.

### Visual Markdown Table

Generate GitHub-Flavored Markdown tables for inspection or agent context:

```python
# Evaluated values view
print(auditor.to_markdown(view="values"))

# Lowered Excel formulas view
print(auditor.to_markdown(view="formulas"))

# Declarative f8 expressions view
print(auditor.to_markdown(view="expr"))

# Physical Excel cell addresses view
print(auditor.to_markdown(view="addresses"))
```

### Tabular Dictionary Export

Export rectangular matrices compatible with `pandas.DataFrame`:

```python
# List of row records
records = auditor.to_dict(orient="records")
# [{'item': 'Revenue', '2024E': 1000.0}, ...]

# Split format (headers, rows, data)
split = auditor.to_dict(orient="split")
# {'columns': [...], 'index': [...], 'data': [...]}
```

---

## 3. Structured Exception Hierarchy

The `f8` compiler raises structured exceptions inheriting from `F8Error`. All compiler exceptions provide typed attributes for diagnostic root-cause analysis:

| Exception | Attributes | Root Cause |
| :--- | :--- | :--- |
| `CycleDetectedError` | `cycle: list[str]` | Dependency cycle in the calculation graph |
| `ReferenceResolutionError` | `coordinate: str`, `target_table: str` | Reference to an undeclared table, row, or column ID |
| `FormulaParseError` | `table: str`, `row: str`, `col: str` | Formula syntax violation (unquoted key, lowercase function, etc.) |
| `SpecificationError` | `table: str`, `reason: str` | Structural model constraint violation |

---

## 4. Generic Self-Healing Diagnostic Patterns

Autonomous agents can intercept structured exceptions to perform automated error recovery.

### Pattern 1: Breaking Dependency Loops (`CycleDetectedError`)

A cycle occurs when rows reference each other without a temporal boundary:

```python
from f8 import CycleDetectedError

try:
    compiled = model.compile()
except CycleDetectedError as err:
    # err.cycle contains the ordered sequence of participating coordinates
    # Recovery: break circular feedback by inserting col.prev lag or supplying an exogenous input
    first_node = err.cycle[0]
    table.set_cell(row_id, col_id, expr='Table["other_row", col.prev]')
    compiled = model.compile()
```

### Pattern 2: Correcting Coordinates (`ReferenceResolutionError`)

Occurs when a formula references an identifier not declared in the model:

```python
from f8 import ReferenceResolutionError

try:
    compiled = model.compile()
except ReferenceResolutionError as err:
    # err.coordinate identifies the missing target coordinate
    # err.target_table identifies the target table ID
    # Recovery: update the formula to match declared row/column identifiers
    table.set_cell(row_id, col_id, expr='Table["valid_row", @col]')
    compiled = model.compile()
```

### Pattern 3: Repairing Syntax (`FormulaParseError`)

Occurs when a formula violates grammar invariants:

```python
from f8 import FormulaParseError

try:
    compiled = model.compile()
except FormulaParseError as err:
    # err.table, err.row, and err.col identify the failure site
    # Common fixes:
    # 1. Wrap identifiers in double quotes: Table["row", @col]
    # 2. Uppercase function names: SUM, IF, AVERAGE
    # 3. Specify explicit axis: @col or @row
    table.set_cell(err.row, err.col, expr='Table["row", @col]')
    compiled = model.compile()
```
