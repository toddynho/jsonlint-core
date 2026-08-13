// integration/jsonlint-com/validator.mjs
// Drop-in for jsonlint.com. Two-phase rollout:
//   Phase 1 (shadow): keep the existing validator as the verdict; run this in
//     parallel and beacon divergences. Zero user-facing risk.
//   Phase 2 (flip): render these diagnostics; old validator removed.
//
// Import from your bundle, or directly in a <script type="module"> during
// shadow phase:  import * as V from "https://esm.sh/@jsonlint/core";

import { validate, lineColumn } from "@jsonlint/core";

/** Run validation and return render-ready diagnostics. */
export function lint(text, { mode = "strict" } = {}) {
  const t0 = performance.now();
  const { ok, diagnostics } = validate(text, { mode });
  return {
    ok,
    ms: performance.now() - t0,
    diagnostics: diagnostics.map((d) => {
      const { line, column } = lineColumn(text, d.start);
      const related = d.related ? lineColumn(text, d.related.start) : null;
      return { ...d, line, column, related };
    }),
  };
}

/** Minimal HTML renderer (swap classes for the site's design system). */
export function renderDiagnostics(result, sourceText) {
  if (result.ok && result.diagnostics.length === 0) {
    return `<div class="jl-valid">Valid JSON <span class="jl-ms">(${result.ms.toFixed(1)}ms)</span></div>`;
  }
  const lines = sourceText.split(/\r\n|\r|\n/);
  const esc = (s) => s.replace(/[&<>"]/g, (c) => ({ "&": "&amp;", "<": "&lt;", ">": "&gt;", '"': "&quot;" }[c]));
  const items = result.diagnostics.map((d) => {
    const src = lines[d.line - 1] ?? "";
    const caret = " ".repeat(Math.min(d.column - 1, 200)) + "^";
    return `<li class="jl-diag jl-${d.severity}">
      <div class="jl-head"><span class="jl-code">${d.code}</span> ${esc(d.message)}
        <span class="jl-loc">line ${d.line}, col ${d.column}</span></div>
      <pre class="jl-context">${esc(src.slice(0, 200))}\n${caret}</pre>
      ${d.related ? `<div class="jl-related">first occurrence at line ${d.related.line}, col ${d.related.column}</div>` : ""}
      ${d.hint ? `<div class="jl-hint">${esc(d.hint)}</div>` : ""}
      <a class="jl-docs" href="/errors/${d.code}">what does ${d.code} mean?</a>
    </li>`;
  });
  return `<ul class="jl-diags">${items.join("")}</ul>`;
}

/** Phase-1 shadow comparison: beacon divergences, never affect the UI. */
export function shadowCompare(text, legacyVerdictOk) {
  try {
    const r = lint(text);
    if (r.ok !== legacyVerdictOk) {
      navigator.sendBeacon?.("/api/shadow-divergence", JSON.stringify({
        legacy: legacyVerdictOk, core: r.ok,
        firstCode: r.diagnostics[0]?.code ?? null,
        len: text.length,
        // never send the document itself — length + code only
      }));
    }
    return r;
  } catch (e) {
    navigator.sendBeacon?.("/api/shadow-divergence", JSON.stringify({ threw: String(e).slice(0, 200) }));
    return null;
  }
}
