# Brokenithm 32-Key Setup

This repository contains a customized Brokenithm controller with two ground
rows: 16 top keys and 16 bottom keys.

## Fastest Setup: Portable EXE

1. Download `slidershim-32key-brokenithm.exe` from the repository's latest
   GitHub release.
2. Run the EXE. No source build is required.
3. In slidershim, select Brokenithm, choose an output mode, and click
   **Apply**.
4. Allow the app through Windows Firewall when prompted.
5. Open `http://PC-IP:1606/` in Safari on the iPad.

The portable build still requires Microsoft Edge WebView2 Runtime, which is
already present on current Windows 10 and Windows 11 installations.

## First-Time Setup

Use this method when building from source:

1. Double-click `setup-brokenithm.cmd`.
2. Approve Windows installation or administrator prompts.
3. Wait for dependencies and the customized `slidershim.exe` to build.
4. When slidershim opens, choose:
   - **Input Device:** `Brokenithm`
   - **Brokenithm Port:** `1606`
   - **Output Mode:** the keyboard layout required by your game
   - **LED Mode:** optional
5. Click **Apply**.

The first build can take several minutes. The setup script installs these
components if they are missing:

- Node.js LTS
- Rustup and nightly Rust
- Visual Studio 2022 C++ Build Tools
- Microsoft Edge WebView2 Runtime

## Connect the iPad

1. Connect the Windows PC and iPad to the same Wi-Fi or local network.
2. Start slidershim and click **Apply** after selecting Brokenithm.
3. Open Safari on the iPad.
4. Enter one of the addresses printed by the start script, for example:

   ```text
   http://192.168.1.25:1606/
   ```

5. The ground controller should show two rows of 16 keys.
6. Optionally use Safari's **Add to Home Screen** command and iPad Guided
   Access to prevent accidental navigation.

You can also click **Brokenithm QR** inside slidershim and scan the generated
QR code.

## Everyday Start

Double-click:

```text
start-brokenithm.cmd
```

Then select Brokenithm and click **Apply** if it is not already active.

## Useful Commands

Run these from PowerShell in the repository directory:

```powershell
# Check installed tools and whether an executable exists
.\setup-brokenithm.ps1 -Action Doctor

# Rebuild after editing the controller
.\setup-brokenithm.ps1 -Action Build

# Recreate the Windows Firewall rule
.\setup-brokenithm.ps1 -Action Firewall

# Start the existing build without rebuilding
.\setup-brokenithm.ps1 -Action Run

# Build without launching
.\setup-brokenithm.ps1 -Action All -InstallPrerequisites -NoLaunch
```

To use a different Brokenithm port:

```powershell
.\setup-brokenithm.ps1 -Action Run -Port 1700
```

Set the same port in the slidershim window and iPad URL.

## Troubleshooting

### Safari Cannot Open the Page

- Confirm slidershim is running and Brokenithm was selected before clicking
  **Apply**.
- Confirm the PC and iPad are on the same network.
- Test `http://127.0.0.1:1606/` in a browser on the Windows PC.
- Run `setup-brokenithm.ps1 -Action Firewall` from PowerShell.
- On a Windows hotspot or public network, ensure the firewall prompt permits
  public-network access.
- Disable VPN isolation temporarily if it prevents local-device connections.

### The Controller Is Stuck

Click **Apply** again in slidershim, then refresh Safari. This rebuilds the
controller context and clears its state.

### The Build Fails

Run:

```powershell
.\setup-brokenithm.ps1 -Action Doctor
```

If tools were installed during the failed attempt, restart Windows or open a
new terminal and run `setup-brokenithm.cmd` again.

### Windows Defender Warning

The executable is locally built and unsigned, so Windows may warn about it.
Use the generated MSI installer when available, or allow the executable only
if you trust this source tree.
