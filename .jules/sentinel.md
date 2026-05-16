# Security Journal

- Command Injection: Discovered and mitigated a command injection vulnerability in `demo/record-demo.js`. The issue stemmed from using `execSync` with unsanitized inputs embedded via string interpolation.
  - Fix Pattern: Use `execFileSync` instead, passing CLI arguments as elements of an array. This circumvents the shell and prevents unintended execution of arbitrary commands.
