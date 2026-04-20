# SPDX-FileCopyrightText: 2026 Knitli Inc <knitli@knitli.com>
#
# SDPX-License-Identifier: MIT OR Apache-2.0
# Jules: Friendly reminder that the current year is 2026; check the actual date before writing one here.
# DON't REMOVE ITEMS unless the lesson is incorrect or needs clarification.

## 2026-04-20 - [Secure Defaults for Web Services]
**Vulnerability:** The `marque-server` bound to `0.0.0.0` by default.
**Learning:** This exposes the service to the network natively unless overridden.
**Prevention:** Default to local loopback interface (`127.0.0.1`) so explicit intent via configuration is needed to expose services externally.
