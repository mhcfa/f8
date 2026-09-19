# Intermediate Representation (IR)

This document specifies Finx's intermediate representations, focusing on cell-level granularity, the 2D grid expansion pass, and the deterministic property resolution lattice.

## 1. Cell-Level IR Granularity

Dependency analysis and calculation ordering operate strictly on **individual cells**, never on whole rows or tables.

### 1.1. Cell Identity (`CellId`)

Every discrete cell in the model is identified by a 3-tuple:

$$\text{CellId} = (\text{TableId}, \text{RowId}, \text{ColId})$$

- `TableId`: Globally unique table identifier across the workbook (e.g. `IncomeStatement`).
- `RowId`: Row identifier within the table (e.g. `revenue`).
- `ColId`: Column identifier within the table (e.g. `2024E`).

The workbook symbol table maintains a 1-to-1 mapping from each `TableId` to its parent `SheetId`.

### 1.2. Why Cell-Level IR is Essential

In financial modeling, roll-forward schedules (corkscrews) link consecutive periods:

$$\text{EndingBalance}_t = \text{BeginningBalance}_t + \Delta_t$$
$$\text{BeginningBalance}_t = \text{EndingBalance}_{t-1}$$

At the row level, `EndingBalance` and `BeginningBalance` appear mutually dependent. At the cell level across time, the graph forms a pure Directed Acyclic Graph (DAG):

$$\text{End}_{t-1} \rightarrow \text{Beg}_t \rightarrow \text{End}_t \rightarrow \text{Beg}_{t+1}$$

Cell-level IR prevents false-positive cyclic dependency errors.

---

## 2. 2D Grid Expansion Pass

The grid expansion pass transforms the high-level specification AST into discrete cell IR nodes using the 3-tier specificity lattice:

```mermaid
flowchart TD
    A["Spec AST (TableSpec, AxisRoles, Vector Rules, Cells)"] --> B["Grid Expansion Engine (Matrix2D)"]
    B --> C["Symbol Table (TableId -> SheetId)"]
    B --> D["Resolve Group & Dynamic Aliases\n(e.g. @col, @row, cols.forecast)"]
    B --> E["Evaluate 3-Tier Specificity Lattice\n(Point Override > Vector Rule > Explicit Null)"]
    B --> F["Cell IR Matrix\n(Vec of Cell Nodes with CellId)"]
```

### 2.1. Expansion Pipeline

1. **Axis enumeration**: Iterates over all declared table rows and columns $(r, c)$ in declared order.
2. **First-class cell registration**: Both descriptor coordinates (labels, units, metadata) and measure coordinates are registered as full calculation or literal nodes in the cell grid.
3. **Property lattice evaluation**: Evaluates payload, formatting, and notes through the deterministic resolution rules.
4. **Group & dynamic alias lowering**: Lowers dynamic coordinate tokens (`@col`, `@row`, `col.prev`, `col.next`, `row.prev`, `row.next`, `cols.<group>`, `rows.<group>`) into concrete coordinates or bounded range boundaries.

---

## 3. The 2D Specificity Resolution Lattice

Every cell coordinate $(r, c)$ in table $T$ resolves its payload, format, and notes through a deterministic evaluation lattice.

### 3.1. Cell Payload Resolution Algorithm

```mermaid
flowchart TD
    A["Evaluate Cell (r, c)"] --> B{"Explicit entry in\nrow.cells[c]?"}
    B -->|"Yes"| C["Assign Cell Entry\n(Literal, Formula, or Null)"]
    B -->|"No"| D{"Is either axis\na Descriptor?"}
    D -->|"Yes"| G["COMPILE ERROR:\nUnassignedCellError at (r, c)"]
    D -->|"No (Both Measure)"| H{"Do BOTH Row r AND Col c\ndefine an expr?"}
    H -->|"Yes"| I["COMPILE ERROR:\nAmbiguousExpressionConflict"]
    H -->|"No"| J{"Does Row r define expr?"}
    J -->|"Yes"| K["Assign Row expr to (r, c)"]
    J -->|"No"| L{"Does Col c define expr?"}
    L -->|"Yes"| M["Assign Col expr to (r, c)"]
    L -->|"No"| G
```

1. **Step 1 (cell override)**: Check if `row.cells` explicitly defines an entry for column $c$. If present, use this payload (literal, formula, or explicit `null`).
2. **Step 2 (descriptor boundary check)**: If row $r$ or column $c$ is marked `role: "descriptor"`, vector rules do not broadcast across it. If no explicit entry exists in `row.cells`, halt compilation with `UnassignedCellError`.
3. **Step 3 (ambiguity check)**: For measure-on-measure intersections, check if row $r$ defines `row.expr` **AND** column $c$ defines `col.expr`. If both exist without an explicit cell override, halt compilation with `AmbiguousExpressionConflict`.
4. **Step 4 (row vector rule)**: If only `row.expr` is defined, assign `row.expr` to cell $(r, c)$.
5. **Step 5 (column vector rule)**: If only `col.expr` is defined, assign `col.expr` to cell $(r, c)$.
6. **Step 6 (exhaustive verification)**: If neither defines an expression and no entry was provided in `row.cells`, halt compilation with `UnassignedCellError`.

### 3.2. Format Preset Resolution Algorithm

1. **Cell format override**: An explicit point format declared on the cell always wins.
2. **Descriptor boundary**: A `descriptor` axis specifies text formatting for its labels. Measure formats (`currency`, `percentage`) never broadcast to descriptor coordinates.
3. **Measure vector conflict**: If row $r$ and column $c$ are both measures and specify differing format presets (e.g. `row.format: "currency_total"` and `col.format: "percentage"`), compilation halts with `FormatConflictError` unless resolved by an explicit cell format override.
4. **Single vector format**: If only the row or only the column specifies a format preset, that preset applies.
5. **Default**: Unspecified numbers default to general numeric presentation; unspecified text defaults to standard string alignment.

### 3.3. Note & Provenance Resolution

- **Precedence**: `Cell.note > Row.note > Col.note`.
- **Inheritance**: A cell inherits its row or column note/source unless explicitly overridden at the cell level.

---

## 4. Cell IR Node Structure

Each expanded cell node contains:

- **Identity**: `CellId` (`TableId`, `RowId`, `ColId`).
- **Role**: `AxisRole` (`Measure` or `Descriptor`).
- **Sheet**: Target `SheetId` resolved from the parent table.
- **Payload**: Either:
  - `Literal(ValueLiteral)`: Number, string, boolean, or null.
  - `Formula(AstNode)`: Parsed expression AST containing reference targets.
- **Formatting**: Resolved format preset token (e.g. `currency`, `percentage`, `text`).
- **Notes & Provenance**: Inherited or overridden note string and provenance metadata.
- **Spatial Position**: Populated later by the layout engine (`sheet_name`, `col_idx`, `row_idx`).

See [Dependency Graph](./dependency-graph.md) for how cell nodes are assembled into a DAG, and [Spatial Layout Planning](spatial-layout.md) for coordinate assignment.
