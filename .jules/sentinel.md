<!--
SPDX-FileCopyrightText: 2026 Knitli Inc.
SPDX-License-Identifier: MIT OR Apache-2.0
-->
<!--
Jules -- Before you write a date for your log entry: **Please check the actual date from an authoritative source**. Remember that your training was ~1 year ago, so the current date will feel like "the future."
The year **is 2026**. 
-->

## 2026-04-21 - [HIGH] Unintentional External Exposure
**Vulnerability:** The default bind address for marque-server was `0.0.0.0`, which binds to all network interfaces.
**Learning:** Defaulting to `0.0.0.0` exposes the server to external networks unintentionally. This poses a security risk if the server is deployed without explicit network access controls.
**Prevention:** Always default network service bindings to the local loopback interface (`127.0.0.1`) unless external access is explicitly required and configured.

## 2026-04-23 - [HIGH] Path Traversal in Custom Dev Server
**Vulnerability:** The basic path traversal check `!absPath.startsWith(DEMO_ROOT)` in the static dev server `demo/bin/serve.js` was flawed. Using `startsWith` allows bypassing the check if a sibling directory exists that starts with the same prefix as `DEMO_ROOT` (e.g., `DEMO_ROOT`="demo", `absPath`="demo-secrets/foo.txt").
**Learning:** `String.prototype.startsWith()` is never a sufficient mechanism for ensuring a directory constraint or path containment, because file paths are structured with directories separated by slashes, whereas strings are plain character arrays without hierarchy.
**Prevention:** For custom static servers, properly validate path boundaries by converting user paths to relative paths relative to the intended web root, and verifying they do not start with `..` (ensuring boundary containment) and are not absolute paths. Using `path.relative()` combined with proper boundaries ensures containment without prefix overlap vulnerabilities.
