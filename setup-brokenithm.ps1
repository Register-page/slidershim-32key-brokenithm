[CmdletBinding()]
param(
  [ValidateSet("All", "Doctor", "Setup", "Build", "Firewall", "Run")]
  [string]$Action = "All",
  [switch]$InstallPrerequisites,
  [switch]$NoLaunch,
  [ValidateRange(1024, 65535)]
  [int]$Port = 1606
)

$ErrorActionPreference = "Stop"
$ProjectRoot = Split-Path -Parent $MyInvocation.MyCommand.Path
$ExePath = Join-Path $ProjectRoot "src-tauri\target\release\slidershim.exe"
$FirewallRuleName = "slidershim Brokenithm"

Set-Location $ProjectRoot

function Write-Step([string]$Message) {
  Write-Host ""
  Write-Host "==> $Message" -ForegroundColor Cyan
}

function Refresh-Path {
  $machinePath = [Environment]::GetEnvironmentVariable("Path", "Machine")
  $userPath = [Environment]::GetEnvironmentVariable("Path", "User")
  $cargoPath = Join-Path $env:USERPROFILE ".cargo\bin"
  $env:Path = "$machinePath;$userPath;$cargoPath"
}

function Test-Command([string]$Name) {
  return $null -ne (Get-Command $Name -ErrorAction SilentlyContinue)
}

function Test-VcTools {
  if (Test-Command "cl.exe") {
    return $true
  }

  $vswhere = Join-Path ${env:ProgramFiles(x86)} "Microsoft Visual Studio\Installer\vswhere.exe"
  if (-not (Test-Path -LiteralPath $vswhere)) {
    return $false
  }

  $installation = & $vswhere `
    -latest `
    -products * `
    -requires Microsoft.VisualStudio.Component.VC.Tools.x86.x64 `
    -property installationPath
  return -not [string]::IsNullOrWhiteSpace(($installation | Select-Object -First 1))
}

function Test-WebView2 {
  $registryPaths = @(
    "HKLM:\SOFTWARE\Microsoft\EdgeUpdate\Clients\*",
    "HKLM:\SOFTWARE\WOW6432Node\Microsoft\EdgeUpdate\Clients\*",
    "HKCU:\SOFTWARE\Microsoft\EdgeUpdate\Clients\*"
  )

  foreach ($path in $registryPaths) {
    $runtime = Get-ItemProperty -Path $path -ErrorAction SilentlyContinue |
      Where-Object { $_.name -like "*WebView2*" } |
      Select-Object -First 1
    if ($runtime) {
      return $true
    }
  }
  return $false
}

function Install-WingetPackage {
  param(
    [Parameter(Mandatory = $true)]
    [string]$Id,
    [string[]]$OverrideArguments
  )

  if (-not (Test-Command "winget.exe")) {
    throw "winget is unavailable. Install 'App Installer' from Microsoft Store, then rerun this script."
  }

  $arguments = @(
    "install",
    "--id", $Id,
    "--exact",
    "--accept-package-agreements",
    "--accept-source-agreements"
  )
  if ($OverrideArguments) {
    $arguments += "--override"
    $arguments += ($OverrideArguments -join " ")
  }

  & winget.exe @arguments
  if ($LASTEXITCODE -ne 0) {
    throw "winget failed while installing $Id (exit code $LASTEXITCODE)."
  }
  Refresh-Path
}

function Get-MissingPrerequisites {
  $missing = [System.Collections.Generic.List[string]]::new()
  if (-not (Test-Command "node.exe")) {
    $missing.Add("Node.js LTS")
  }
  if (-not (Test-Command "rustup.exe")) {
    $missing.Add("Rustup")
  }
  if (-not (Test-VcTools)) {
    $missing.Add("Visual Studio 2022 C++ build tools")
  }
  if (-not (Test-WebView2)) {
    $missing.Add("Microsoft Edge WebView2 Runtime")
  }
  return $missing
}

function Show-Doctor {
  Write-Step "Checking prerequisites"

  $checks = @(
    [pscustomobject]@{
      Requirement = "Node.js"
      Ready = Test-Command "node.exe"
      Details = if (Test-Command "node.exe") { (& node.exe --version) } else { "missing" }
    },
    [pscustomobject]@{
      Requirement = "Rustup"
      Ready = Test-Command "rustup.exe"
      Details = if (Test-Command "rustup.exe") { (& rustup.exe --version) } else { "missing" }
    },
    [pscustomobject]@{
      Requirement = "Rust nightly"
      Ready = (Test-Command "rustup.exe") -and ((& rustup.exe toolchain list) -match "^nightly")
      Details = if (Test-Command "rustup.exe") {
        ((& rustup.exe toolchain list) -join ", ")
      } else {
        "missing"
      }
    },
    [pscustomobject]@{
      Requirement = "MSVC C++ tools"
      Ready = Test-VcTools
      Details = if (Test-VcTools) { "installed" } else { "missing" }
    },
    [pscustomobject]@{
      Requirement = "WebView2 Runtime"
      Ready = Test-WebView2
      Details = if (Test-WebView2) { "installed" } else { "missing" }
    },
    [pscustomobject]@{
      Requirement = "Built slidershim"
      Ready = Test-Path -LiteralPath $ExePath
      Details = if (Test-Path -LiteralPath $ExePath) { $ExePath } else { "not built yet" }
    }
  )

  $checks | Format-Table -AutoSize
}

function Install-Prerequisites {
  Refresh-Path
  $missing = Get-MissingPrerequisites

  if ($missing.Count -eq 0) {
    Write-Host "All system prerequisites are already installed." -ForegroundColor Green
    return
  }

  if (-not $InstallPrerequisites) {
    throw @"
Missing prerequisites: $($missing -join ", ")

Rerun setup-brokenithm.cmd, or run:
  powershell -ExecutionPolicy Bypass -File .\setup-brokenithm.ps1 -Action Setup -InstallPrerequisites
"@
  }

  if (-not (Test-Command "node.exe")) {
    Write-Step "Installing Node.js LTS"
    Install-WingetPackage -Id "OpenJS.NodeJS.LTS"
  }

  if (-not (Test-Command "rustup.exe")) {
    Write-Step "Installing Rustup"
    Install-WingetPackage -Id "Rustlang.Rustup"
  }

  if (-not (Test-VcTools)) {
    Write-Step "Installing Visual Studio 2022 C++ build tools"
    Install-WingetPackage `
      -Id "Microsoft.VisualStudio.2022.BuildTools" `
      -OverrideArguments @(
        "--wait",
        "--passive",
        "--norestart",
        "--add", "Microsoft.VisualStudio.Workload.VCTools",
        "--includeRecommended"
      )
  }

  if (-not (Test-WebView2)) {
    Write-Step "Installing Microsoft Edge WebView2 Runtime"
    Install-WingetPackage -Id "Microsoft.EdgeWebView2Runtime"
  }

  Refresh-Path
}

function Install-ProjectDependencies {
  Install-Prerequisites

  if (-not (Test-Command "rustup.exe")) {
    throw "Rustup was installed but is not visible yet. Open a new terminal and rerun the script."
  }

  Write-Step "Installing and selecting nightly Rust"
  & rustup.exe toolchain install nightly
  if ($LASTEXITCODE -ne 0) {
    throw "Failed to install the nightly Rust toolchain."
  }
  & rustup.exe override set nightly
  if ($LASTEXITCODE -ne 0) {
    throw "Failed to select nightly Rust for this repository."
  }

  Write-Step "Installing JavaScript dependencies"
  & npx.cmd --yes yarn@1.22.22 install --frozen-lockfile
  if ($LASTEXITCODE -ne 0) {
    throw "JavaScript dependency installation failed."
  }
}

function Build-Slidershim {
  Install-ProjectDependencies

  Write-Step "Building slidershim"
  & npx.cmd --yes yarn@1.22.22 tauri build
  if ($LASTEXITCODE -ne 0) {
    throw "slidershim build failed."
  }
  if (-not (Test-Path -LiteralPath $ExePath)) {
    throw "Build completed but slidershim.exe was not found at $ExePath."
  }

  Write-Host "Built: $ExePath" -ForegroundColor Green

  $installers = Get-ChildItem `
    -Path (Join-Path $ProjectRoot "src-tauri\target\release\bundle") `
    -File `
    -Recurse `
    -ErrorAction SilentlyContinue |
    Where-Object { $_.Extension -in @(".msi", ".exe") }
  if ($installers) {
    Write-Host "Installer packages:"
    $installers.FullName | ForEach-Object { Write-Host "  $_" }
  }
}

function Test-Administrator {
  $identity = [Security.Principal.WindowsIdentity]::GetCurrent()
  $principal = [Security.Principal.WindowsPrincipal]::new($identity)
  return $principal.IsInRole([Security.Principal.WindowsBuiltInRole]::Administrator)
}

function Add-BrokenithmFirewallRule {
  if (-not (Test-Path -LiteralPath $ExePath)) {
    throw "Build slidershim before adding its firewall rule."
  }

  if (-not (Test-Administrator)) {
    Write-Step "Requesting administrator access for the firewall rule"
    $arguments = @(
      "-NoLogo",
      "-NoProfile",
      "-ExecutionPolicy", "Bypass",
      "-File", "`"$PSCommandPath`"",
      "-Action", "Firewall",
      "-Port", $Port
    )
    $process = Start-Process powershell.exe -Verb RunAs -ArgumentList $arguments -Wait -PassThru
    if ($process.ExitCode -ne 0) {
      throw "The elevated firewall setup did not complete successfully."
    }
    return
  }

  Write-Step "Configuring Windows Firewall"
  Get-NetFirewallRule -DisplayName $FirewallRuleName -ErrorAction SilentlyContinue |
    Remove-NetFirewallRule

  New-NetFirewallRule `
    -DisplayName $FirewallRuleName `
    -Direction Inbound `
    -Action Allow `
    -Program $ExePath `
    -Protocol TCP `
    -Profile Any `
    -Description "Allows iPad connections to the slidershim Brokenithm server." |
    Out-Null

  Write-Host "Firewall access enabled for slidershim.exe (Brokenithm port $Port)." -ForegroundColor Green
}

function Show-ConnectionAddresses {
  $addresses = Get-NetIPAddress -AddressFamily IPv4 -ErrorAction SilentlyContinue |
    Where-Object {
      $_.IPAddress -ne "127.0.0.1" -and
      $_.IPAddress -notlike "169.254.*"
    } |
    Select-Object -ExpandProperty IPAddress -Unique

  Write-Host ""
  Write-Host "On the iPad, open one of these addresses:" -ForegroundColor Yellow
  if ($addresses) {
    $addresses | ForEach-Object { Write-Host "  http://${_}:$Port/" }
  } else {
    Write-Host "  No LAN IPv4 address was detected. Connect both devices to the same network."
  }
}

function Start-Slidershim {
  if (-not (Test-Path -LiteralPath $ExePath)) {
    throw "slidershim.exe has not been built. Run setup-brokenithm.cmd first."
  }

  $existing = Get-Process -Name "slidershim" -ErrorAction SilentlyContinue |
    Select-Object -First 1
  if ($existing) {
    Write-Step "slidershim is already running"
  } else {
    Write-Step "Starting slidershim"
    Start-Process -FilePath $ExePath -WorkingDirectory (Split-Path -Parent $ExePath)
  }
  Show-ConnectionAddresses

  Write-Host ""
  Write-Host "In slidershim:" -ForegroundColor Cyan
  Write-Host "  1. Set Input Device to Brokenithm."
  Write-Host "  2. Keep Brokenithm Port at $Port (or use the same port on the iPad URL)."
  Write-Host "  3. Choose a keyboard output layout."
  Write-Host "  4. Click Apply."
}

try {
  Refresh-Path

  switch ($Action) {
    "Doctor" {
      Show-Doctor
    }
    "Setup" {
      Install-ProjectDependencies
      Show-Doctor
    }
    "Build" {
      Build-Slidershim
    }
    "Firewall" {
      Add-BrokenithmFirewallRule
    }
    "Run" {
      Start-Slidershim
    }
    "All" {
      Build-Slidershim
      Add-BrokenithmFirewallRule
      if (-not $NoLaunch) {
        Start-Slidershim
      }
    }
  }
} catch {
  Write-Host ""
  Write-Host "ERROR: $($_.Exception.Message)" -ForegroundColor Red
  exit 1
}
