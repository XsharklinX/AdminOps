<div align="center">

<img src="src/assets/logo.svg" alt="AdminOps" width="96" />

# AdminOps

**Diagnostics, maintenance and tech support for Windows 10 and 11, in a single app.**

[🇪🇸 Español](README.md) · 🇬🇧 **English**

[![Version](https://img.shields.io/badge/version-1.1.10-22d3ee?style=flat-square)](https://github.com/XsharklinX/AdminOps/releases)
[![Platform](https://img.shields.io/badge/Windows-10%20%7C%2011-0078d4?style=flat-square&logo=windows)](#requirements)
[![Built with Tauri](https://img.shields.io/badge/Tauri%202-Rust%20%2B%20React-a855f7?style=flat-square&logo=tauri)](docs/DEVELOPMENT.md)

</div>

---

> The app's interface and documentation are in Spanish. This page is a summary in English.

## What it is

AdminOps is a desktop app for people who do **tech support**: you get to a PC that is slow, won't
print or can't reach the network, and you have to find out why, fix it and leave a record of what
you did.

It puts in one window what is usually spread across a dozen tools, explains it in plain language,
and adds what Windows lacks: a **memory** of what was changed, when, on which machine and for
which client.

- **Undoable.** Every change stores the exact previous value. Risky ones create a restore point first.
- **Private.** No accounts, no telemetry, no server. Data stays on the PC or on your USB drive.
- **Portable.** Runs from a USB drive with your clients, contacts and settings, leaving nothing on the client's PC.
- **Lightweight.** Rust and WebView2: an installer of about 11 MB.
- **Two modes.** Technician (everything) and user (the essentials). It is an interface mode, not a security boundary.

## What it does

| Area | Main features |
| --- | --- |
| **Home** | Live dashboard. Guided troubleshooting. Step-by-step service session with a **PDF report** comparing before and after. |
| **Machine** | Diagnostics ranked by severity. Hardware and temperatures. Security score. Disks: space, health, repair and file rescue. Reversible Windows tweaks. Processes grouped by program. Change journal. |
| **Support** | Your company's ticketing site, mail and Teams inside the app. Domain people. Clients with their machines, visits and warranties. Calendar. Contacts. Step-by-step solutions. |
| **Network** | Router and Wi‑Fi, connected devices, speed test, network repair, ping, traceroute, ports, DNS and hosts file. |
| **Apps** | Update, bulk-install and uninstall programs; Windows Update; remove preinstalled apps; templates to prepare a new PC. |
| **Administration** | Local users, accounts and domain, printers, shared folders, remote access, inventory, Windows tools and keyboard shortcuts. |
| **Data** | User data migration, encrypted vault, secure erase, deleted file recovery and parental controls. |

## Download

Get the latest version from **[Releases](https://github.com/XsharklinX/AdminOps/releases/latest)**:
the installer (`-Setup.exe`), the classic installer (supports `/S` for silent deployment) and the
portable zip.

> [!NOTE]
> AdminOps is not code-signed, so Windows SmartScreen may show *"Windows protected your PC"*.
> Click **More info → Run anyway**. Only download it from this page.

### Requirements

- Windows 10 or 11, 64-bit.
- Microsoft Edge WebView2 (included in Windows 11 and up-to-date Windows 10).
- **Administrator rights** to apply changes. Without them the app is read-only.

## Privacy and responsibility

- No accounts, ads or telemetry. Data is stored locally; saved passwords are encrypted.
- It only connects to the Internet for what you ask: update check, speed test, installing programs with winget and the websites you open inside it.
- Use AdminOps only on machines you own or are authorised to work on. It is provided without warranty: see the **[terms of use](src-tauri/terminos.txt)** (in Spanish).

## Build from source

```bash
npm install
npm run tauri dev        # development
npm run build:release    # installer + portable in release/v<version>/
```

Requires Node.js 22+, stable Rust and the Visual Studio C++ build tools.

**Stack:** Tauri 2 · Rust · React 19 · TypeScript · Vite · Tailwind CSS 4 · WebView2.

## Author

Created by **David Bonilla**. To report a bug, use *Acerca de → Reportar un problema* in the app,
or write to Contactoyerlindavid@gmail.com.

© 2026 David Bonilla. All rights reserved.
