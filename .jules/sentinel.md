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
## 2026-04-26 - [MEDIUM] Missing Security Headers
**Vulnerability:** The custom dev server in `demo/bin/serve.js` did not set the `X-Content-Type-Options: nosniff` header.
**Learning:** Without this header, browsers might perform MIME-sniffing and interpret files with incorrect MIME types as executable scripts, which could lead to XSS if a user uploads a malicious file.
**Prevention:** Always include `X-Content-Type-Options: nosniff` in responses from custom HTTP servers to enforce strict MIME type checking.

## 2026-04-27 - [MEDIUM] Missing Security Headers on REST API
**Vulnerability:** The `marque-server` axum REST API lacked the `X-Content-Type-Options: nosniff` header.
**Learning:** Axum does not add security headers by default. If `nosniff` is missing, API clients or browsers directly interacting with the endpoints might perform MIME sniffing and misinterpret the response, which could pose XSS risks.
**Prevention:** Use `tower_http::set_header::SetResponseHeaderLayer` to globally enforce `X-Content-Type-Options: nosniff` across all axum routes. Ensure `tower-http` has the `set-header` feature enabled.
## 2026-04-28 - [MEDIUM] Missing X-Frame-Options Header
**Vulnerability:** The `marque-server` axum REST API lacked the `X-Frame-Options: DENY` header.
**Learning:** Without this header, the API responses could potentially be embedded in an iframe on a malicious site, enabling clickjacking attacks.
**Prevention:** Use `tower_http::set_header::SetResponseHeaderLayer` to globally enforce `X-Frame-Options: DENY` across all axum routes in the router configuration.

## 2026-05-04 - [MEDIUM] Missing X-Frame-Options Header in Demo Server
**Vulnerability:** The static dev server in `demo/bin/serve.js` was missing the `X-Frame-Options` HTTP response header.
**Learning:** Without this header, the application could be embedded in an iframe on a malicious site, potentially leading to clickjacking attacks.
**Prevention:** Always enforce `X-Frame-Options: DENY` (or `SAMEORIGIN`) on custom Node.js HTTP servers to mitigate clickjacking.

## 2026-05-06 - [MEDIUM] Missing Content-Security-Policy Header in Demo Server
**Vulnerability:** The custom Node.js static dev server `demo/bin/serve.js` did not set a `Content-Security-Policy` (CSP) header.
**Learning:** Without a CSP, the application lacks defense-in-depth against Cross-Site Scripting (XSS) attacks. Even if the codebase itself avoids insecure practices (like using `innerHTML`), a CSP restricts where scripts and styles can be loaded from or executed, mitigating the impact of any potential future injection flaws.
**Prevention:** Always implement a restrictive `Content-Security-Policy` header in custom HTTP servers. For applications utilizing WebAssembly, ensure `script-src` includes `'wasm-unsafe-eval'` to allow WASM compilation without resorting to the broader `'unsafe-eval'` where possible.
## 2025-05-28 - [CRITICAL] Prevent Sensitive Data Leakage via Browser/Proxy Caching
**Vulnerability:** The `marque-server` REST API responses were missing the `Cache-Control: no-store, max-age=0` header.
**Learning:** By default, REST API responses might be cached by browsers or intermediate proxies (like CDNs). For an engine dealing with security classification labels and text, caching these responses could leak sensitive text or audit data to unauthorized viewers on shared networks or machines.
**Prevention:** Always explicitly include `Cache-Control: no-store, max-age=0` on sensitive API endpoints to ensure responses are never cached anywhere in the chain. This was addressed by adding a global `SetResponseHeaderLayer` in `crates/server/src/middleware.rs`.
