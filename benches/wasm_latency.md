# WASM Interactive Latency Measurement (SC-001b)

## Target

`lint(text)` completes in ≤32ms p95 on ≤10KB inputs when driven from the
`harness.html` page in a current Chromium-family browser on the reference
machine. This is **advisory** for the MVP (logged, not CI-blocking); it
becomes a hard gate in the browser-extension slice.

## Method

1. Build the WASM artifact:
   ```bash
   wasm-pack build crates/marque-wasm --target web --release
   ```

2. Serve the harness page:
   ```bash
   cd crates/marque-wasm
   python3 -m http.server 8080
   # Open http://localhost:8080/examples/harness.html
   ```

3. Open the browser developer console and paste the measurement script below.

4. Record the p50, p95, and p99 values along with browser version and input
   sizes.

## Measurement Script

Paste into the browser console after the WASM module has loaded:

```javascript
import('../pkg/marque_wasm.js').then(async (mod) => {
  await mod.default();

  const inputs = {
    '100B':  'TOP SECRET//SI//NF\n'.repeat(5),
    '1KB':   'SECRET//NOFORN\n(TS//SI//NF) Lorem ipsum dolor sit amet.\n'.repeat(15),
    '5KB':   'SECRET//NF\n(S//NF) Portion text here.\n'.repeat(100),
    '10KB':  'TOP SECRET//SI//NF\n(TS//SI) Classified portion.\n'.repeat(200),
  };

  for (const [label, text] of Object.entries(inputs)) {
    const times = [];
    const iterations = 100;
    for (let i = 0; i < iterations; i++) {
      const t0 = performance.now();
      mod.lint(text, undefined);
      times.push(performance.now() - t0);
    }
    times.sort((a, b) => a - b);
    const p50  = times[Math.floor(iterations * 0.50)].toFixed(3);
    const p95  = times[Math.floor(iterations * 0.95)].toFixed(3);
    const p99  = times[Math.floor(iterations * 0.99)].toFixed(3);
    console.log(`${label}: p50=${p50}ms  p95=${p95}ms  p99=${p99}ms  (${text.length} bytes, ${iterations} iterations)`);
  }
});
```

## Recording Template

| Input Size | Bytes | p50 (ms) | p95 (ms) | p99 (ms) | Iterations |
|------------|-------|----------|----------|----------|------------|
| 100B       |       |          |          |          | 100        |
| 1KB        |       |          |          |          | 100        |
| 5KB        |       |          |          |          | 100        |
| 10KB       |       |          |          |          | 100        |

**Browser**: (e.g., Chrome 124.0.6367.91)
**OS**: (e.g., Ubuntu 24.04 on WSL2)
**Machine**: (reference machine per plan.md)
**Date**: YYYY-MM-DD
**WASM artifact size**: (output of `ls -la crates/marque-wasm/pkg/marque_wasm_bg.wasm`)

## Pass/Fail Criteria

- **Advisory pass**: p95 ≤ 32ms on 10KB input (two display frames at 60fps).
- **Hard gate** (browser-extension slice): same threshold, enforced in CI.

## Notes

- The 32ms target accounts for the JS↔WASM boundary overhead.
- Warm-up iterations are included in the measurement (first few runs may be
  slower due to JIT compilation of the WASM module).
- Run measurements with the browser devtools closed (devtools overhead can
  skew timing) or document that devtools were open.
