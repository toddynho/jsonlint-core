/** @jsonlint/core — diagnostics-grade JSON engine. */

export type Mode = "strict" | "jsonc";
export type Severity = "error" | "warning";

export interface Diagnostic {
  /** Stable, documentable code (e.g. "E008", "W060"). */
  code: string;
  message: string;
  severity: Severity;
  /** UTF-16 offsets into the source. */
  start: number;
  end: number;
  hint?: string;
  /** Secondary location, e.g. the first occurrence of a duplicated key. */
  related?: { start: number; end: number };
}

export interface ParseOptions {
  /** Dialect. Default "strict" (RFC 8259). "jsonc" adds comments + trailing commas. */
  mode?: Mode;
  /** Duplicate object keys: warn (default), error, or allow silently. */
  duplicateKeys?: "warn" | "error" | "allow";
  /** "__proto__" handling: "safe" (default, own-property, no pollution) or "allow". */
  protoKeys?: "safe" | "allow";
  /** Maximum nesting depth. Default 512. */
  maxDepth?: number;
  /** JSON.parse-compatible reviver (parse() only). */
  reviver?: (this: unknown, key: string, value: unknown) => unknown;
}

export type Input = string | Uint8Array | ArrayBuffer;

/** Validate only — fastest path; strings are scanned, never materialized. */
export function validate(input: Input, options?: ParseOptions): {
  ok: boolean;
  diagnostics: Diagnostic[];
};

/** Parse without throwing. value is best-effort recovered even on errors. */
export function tryParse(input: Input, options?: ParseOptions): {
  ok: boolean;
  value: unknown;
  diagnostics: Diagnostic[];
};

/** JSON.parse-compatible: returns the value or throws SyntaxError (with .code and .diagnostics). */
export function parse(input: Input, options?: ParseOptions): unknown;

/** 1-based line/column for a UTF-16 offset. Handles \n, \r\n, \r. */
export function lineColumn(src: string, offset: number): { line: number; column: number };
