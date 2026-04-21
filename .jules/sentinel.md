## 2026-04-21 - [HIGH] Unintentional External Exposure
**Vulnerability:** The default bind address for marque-server was `0.0.0.0`, which binds to all network interfaces.
**Learning:** Defaulting to `0.0.0.0` exposes the server to external networks unintentionally. This poses a security risk if the server is deployed without explicit network access controls.
**Prevention:** Always default network service bindings to the local loopback interface (`127.0.0.1`) unless external access is explicitly required and configured.
