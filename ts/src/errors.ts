export class F8Error extends Error {
  constructor(message: string, options?: ErrorOptions) {
    super(message, options);
    this.name = this.constructor.name;
    Object.setPrototypeOf(this, new.target.prototype);
  }
}

export class CycleDetectedError extends F8Error {
  readonly cycle: readonly string[];

  constructor(message: string, cycle: readonly string[]) {
    super(message);
    this.cycle = cycle;
  }
}

export class ReferenceResolutionError extends F8Error {
  readonly coordinate: string;
  readonly targetTable: string | null;
  readonly row: string | null;
  readonly col: string | null;

  constructor(
    message: string,
    coordinate: string,
    targetTable?: string | null,
    row?: string | null,
    col?: string | null,
  ) {
    super(message);
    this.coordinate = coordinate;
    this.targetTable = targetTable ?? null;
    this.row = row ?? null;
    this.col = col ?? null;
  }
}

export class SpecificationError extends F8Error {
  readonly table: string | null;
  readonly reason: string;

  constructor(message: string, table?: string | null, reason?: string) {
    super(message);
    this.table = table ?? null;
    this.reason = reason ?? message;
  }
}

export class FormulaParseError extends F8Error {
  readonly table: string;
  readonly row: string | null;
  readonly col: string | null;
  readonly tokenOffset: number | null;

  constructor(
    message: string,
    table: string,
    row?: string | null,
    col?: string | null,
    tokenOffset?: number | null,
  ) {
    super(message);
    this.table = table;
    this.row = row ?? null;
    this.col = col ?? null;
    this.tokenOffset = tokenOffset ?? null;
  }
}

export class InspectionError extends F8Error {
  readonly entity: string;

  constructor(message: string, entity: string) {
    super(message);
    this.entity = entity;
  }
}

export class EmissionError extends F8Error {
  constructor(message: string) {
    super(message);
  }
}

interface RawWasmError {
  readonly kind?: string;
  readonly message?: string;
  readonly cycle?: string[];
  readonly coordinate?: string;
  readonly target_table?: string | null;
  readonly row?: string | null;
  readonly col?: string | null;
  readonly table?: string | null;
  readonly reason?: string;
  readonly token_offset?: number | null;
  readonly entity?: string;
}

export function toF8Error(err: unknown): Error {
  if (err instanceof F8Error || err instanceof TypeError) {
    return err;
  }

  if (typeof err === "object" && err !== null && "kind" in err) {
    const raw = err as RawWasmError;
    const msg = raw.message ?? "Unknown F8 error";
    switch (raw.kind) {
      case "CycleDetectedError":
        return new CycleDetectedError(msg, raw.cycle ?? []);
      case "ReferenceResolutionError":
        return new ReferenceResolutionError(
          msg,
          raw.coordinate ?? "",
          raw.target_table ?? null,
          raw.row ?? null,
          raw.col ?? null,
        );
      case "SpecificationError":
        return new SpecificationError(msg, raw.table ?? null, raw.reason ?? msg);
      case "FormulaParseError":
        return new FormulaParseError(
          msg,
          raw.table ?? "",
          raw.row ?? null,
          raw.col ?? null,
          raw.token_offset ?? null,
        );
      case "InspectionError":
        return new InspectionError(msg, raw.entity ?? "");
      case "EmissionError":
        return new EmissionError(msg);
      case "TypeError":
        return new TypeError(msg);
      case "F8Error":
      default:
        return new F8Error(msg);
    }
  }

  if (err instanceof Error) {
    return err;
  }

  return new F8Error(String(err));
}
