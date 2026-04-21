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
