<#
.SYNOPSIS
    xezim — Cross-platform installer for Windows.
.DESCRIPTION
    Installs Rust, Git, and MSVC build tools, then clones and builds
    xezim (SystemVerilog simulator) from source.

    Usage (latest):
        irm https://raw.githubusercontent.com/aionhw/xezim/main/scripts/install.ps1 | iex

    Usage (specific tag):
        $env:XEZIM_TAG='v0.9.6'; irm https://raw.githubusercontent.com/aionhw/xezim/main/scripts/install.ps1 | iex
#>

$ErrorActionPreference = 'Stop'

# ---- Config ----
$Workspace = Join-Path $HOME "xezim-workspace"
$GitHubOrg = "aionhw"
$Repos = @("xezim-core", "xezim")

# ---- Helper functions ----
function Write-Log($msg)  { Write-Host " ✅ $msg" -ForegroundColor Green }
function Write-Warn($msg) { Write-Host " ⚠️  $msg" -ForegroundColor Yellow }
function Write-Info($msg) { Write-Host " ➡️  $msg" -ForegroundColor Cyan }
function Write-Step($msg) { Write-Host " ➡️  $msg" -ForegroundColor White }
function Write-Fail($msg) { Write-Host " ❌ $msg" -ForegroundColor Red; exit 1 }

# ---- Banner ----
Write-Host ""
Write-Host "🚀  xezim Installer — Windows" -ForegroundColor White
Write-Host "   Workspace: $Workspace"
Write-Host ""

# ---- Step 1: Check / Install Rust ----
Write-Info "Step 1/4: Checking Rust toolchain..."
$haveRust = $false
try {
    $rustVersion = & rustc --version 2>$null
    if ($rustVersion) {
        Write-Log "Rust already installed: $rustVersion"
        $haveRust = $true
    }
} catch { }

if (-not $haveRust) {
    Write-Warn "Rust not found. Installing via rustup..."
    $rustupUrl = "https://win.rustup.rs"
    $rustupPath = Join-Path $env:TEMP "rustup-init.exe"

    try {
        Write-Info "Downloading rustup-init.exe..."
        $wc = New-Object System.Net.WebClient
        $wc.DownloadFile($rustupUrl, $rustupPath)
    } catch {
        Invoke-WebRequest -Uri $rustupUrl -OutFile $rustupPath -UseBasicParsing
    }

    Write-Info "Running rustup installer (quiet mode)..."
    Start-Process -FilePath $rustupPath -ArgumentList "-y", "--quiet" -Wait -NoNewWindow
    Remove-Item $rustupPath -ErrorAction SilentlyContinue

    # Add to PATH for this session
    $cargoBin = Join-Path $env:USERPROFILE ".cargo\bin"
    if (Test-Path $cargoBin) {
        $env:Path = "$cargoBin;$env:Path"
    }

    try {
        $rustVersion = & rustc --version 2>$null
        Write-Log "Rust installed: $rustVersion"
    } catch {
        Write-Fail "Rust installation may have failed. Please install manually from https://rustup.rs"
    }
}

# ---- Step 2: Check / Install Git ----
Write-Info "Step 2/4: Checking Git..."
$haveGit = $false
try {
    $gitVersion = & git --version 2>$null
    if ($gitVersion) {
        Write-Log "Git already installed: $gitVersion"
        $haveGit = $true
    }
} catch { }

if (-not $haveGit) {
    Write-Warn "Git not found. Attempting install..."

    # Method 1: Try winget (Windows 10 1809+ / 11)
    $installed = $false
    try {
        $wingetVersion = & winget --version 2>$null
        if ($wingetVersion) {
            Write-Info "Installing Git via winget..."
            & winget install --id Git.Git -e --silent --accept-package-agreements 2>$null
            $installed = $true
        }
    } catch { }

    # Method 2: If winget didn't work, try direct download
    if (-not $installed) {
        try {
            Write-Info "Downloading Git for Windows..."
            # Use GitHub releases latest endpoint — no hardcoded version
            $gitReleasesUrl = "https://api.github.com/repos/git-for-windows/git/releases/latest"
            $releaseData = Invoke-WebRequest -Uri $gitReleasesUrl -UseBasicParsing | ConvertFrom-Json
            $gitAsset = $releaseData.assets | Where-Object { $_.name -like "*-64-bit.exe" } | Select-Object -First 1
            if (-not $gitAsset) {
                throw "No Git installer asset found"
            }
            $gitUrl = $gitAsset.browser_download_url
            $gitInstaller = Join-Path $env:TEMP "git-installer.exe"
            Invoke-WebRequest -Uri $gitUrl -OutFile $gitInstaller -UseBasicParsing
            Write-Info "Running Git installer..."
            Start-Process -FilePath $gitInstaller -ArgumentList "/VERYSILENT", "/NORESTART", "/NOCANCEL", "/SP-", "/SUPPRESSMSGBOXES" -Wait -NoNewWindow
            Remove-Item $gitInstaller -ErrorAction SilentlyContinue
            $installed = $true
        } catch {
            Write-Warn "Direct download failed (may need a newer URL)."
        }
    }

    # Refresh PATH from all sources
    $userPath = [Environment]::GetEnvironmentVariable("Path", [EnvironmentVariableTarget]::User)
    $machinePath = [Environment]::GetEnvironmentVariable("Path", [EnvironmentVariableTarget]::Machine)
    $env:Path = "$machinePath;$userPath;$env:Path"

    # Git for Windows default install locations
    $gitPaths = @(
        "$env:ProgramFiles\Git\cmd",
        "${env:ProgramFiles(x86)}\Git\cmd",
        "$env:LOCALAPPDATA\Programs\Git\cmd"
    )
    foreach ($p in $gitPaths) {
        if (Test-Path "$p\git.exe" -and $env:Path -notlike "*$p*") {
            $env:Path = "$p;$env:Path"
        }
    }

    # Final verification
    try {
        $gitVersion = & git --version 2>$null
        if ($gitVersion) {
            Write-Log "Git installed: $gitVersion"
            $haveGit = $true
        }
    } catch { }
}

if (-not $haveGit) {
    Write-Warn "Could not install Git automatically."
    Write-Warn "Please install Git from https://git-scm.com/download/win, then re-run this script."
    Write-Warn "After installing Git, restart your terminal and try again."
}

# ---- Step 3: Clone repos ----
Write-Info "Step 3/4: Setting up workspace..."

# Create workspace
if (-not (Test-Path $Workspace)) {
    New-Item -ItemType Directory -Path $Workspace -Force | Out-Null
}

foreach ($repo in $Repos) {
    $repoDir = Join-Path $Workspace $repo
    $repoUrl = "https://github.com/${GitHubOrg}/${repo}.git"

    if (Test-Path $repoDir) {
        Write-Info "Updating $repo (existing clone)..."
        Push-Location $repoDir
        try {
            & git fetch --tags --quiet 2>$null
            Write-Log "$repo updated."
        } finally {
            Pop-Location
        }
    } else {
        Write-Info "Cloning $repo..."
        & git clone --quiet $repoUrl $repoDir 2>&1 | Out-Null
        if ($LASTEXITCODE -ne 0) {
            Write-Fail "Failed to clone $repoUrl"
        }
        Write-Log "$repo cloned."
    }
}

# Verify sibling layout
$coreParserDir = Join-Path $Workspace "xezim-core" "xezim-parser"
if (-not (Test-Path $coreParserDir)) {
    Write-Fail "xezim-core\xezim-parser not found! The clone may be incomplete."
}
Write-Log "Workspace structure verified."

# ---- Step 4: Detect tag and checkout ----
$XezimTagExplicit = $false
$XezimTag = if ($env:XEZIM_TAG) { $XezimTagExplicit = $true; $env:XEZIM_TAG } else { $null }
if (-not $XezimTag) {
    Write-Step "Detecting latest release tag from git..."
    Push-Location (Join-Path $Workspace "xezim")
    try {
        $latestTag = & git tag --sort=-creatordate | Select-Object -First 1
        if ($latestTag) {
            $XezimTag = $latestTag.Trim()
            Write-Log "Tag: $XezimTag"
        } else {
            $XezimTag = "main"
            Write-Warn "No tags found, using 'main' branch."
        }
    } finally {
        Pop-Location
    }
}

Write-Step "Checking out $XezimTag..."
foreach ($repo in $Repos) {
    $repoDir = Join-Path $Workspace $repo
    Push-Location $repoDir
    try {
        & git checkout $XezimTag 2>$null
        if ($LASTEXITCODE -eq 0) {
            Write-Log "$repo: checked out $XezimTag"
        } else {
            if ($XezimTagExplicit) {
                Write-Fail "Tag/branch '$XezimTag' not found in $repo. Please verify the tag name and try again."
            } else {
                Write-Warn "Tag/branch '$XezimTag' not found in $repo, staying on default branch."
            }
        }
    } finally {
        Pop-Location
    }
}
# ---- Step 5: Build ----
Write-Info "Step 5/5: Building xezim (release mode)..."
Write-Info "This may take 5-15 minutes on first build."
Write-Host ""

$mainDir = Join-Path $Workspace "xezim"
Push-Location $mainDir
try {
    # FIXME: remove -Awarnings once source warnings are cleaned up
    $oldRustflags = $env:RUSTFLAGS
    $env:RUSTFLAGS = "-Awarnings"
    try {
        & cargo build --release
        if ($LASTEXITCODE -ne 0) {
            Write-Fail "Build failed. Check the output above for errors."
        }
    } finally {
        $env:RUSTFLAGS = $oldRustflags
    }
} finally {
    Pop-Location
}

$binary = Join-Path $mainDir "target" "release" "xezim.exe"
if (-not (Test-Path $binary)) {
    # Try without .exe
    $binary = Join-Path $mainDir "target" "release" "xezim"
}
if (Test-Path $binary) {
    Write-Log "Build successful! Binary: $binary"
} else {
    Write-Fail "Build failed — binary not found at target/release/xezim.exe"
}

# ---- Auto-PATH setup ----
Write-Info "Installing xezim globally..."
$binDir = Join-Path $mainDir "target" "release"
$userPath = [Environment]::GetEnvironmentVariable("Path", [EnvironmentVariableTarget]::User)

if ($userPath -notlike "*$binDir*") {
    try {
        [Environment]::SetEnvironmentVariable("Path", "$userPath;$binDir", [EnvironmentVariableTarget]::User)
        Write-Log "xezim added to system PATH."
    } catch {
        Write-Warn "Could not update system PATH automatically."
    }
}

# Make available in current session
$env:Path = "$binDir;$env:Path"

if (Get-Command xezim -ErrorAction SilentlyContinue) {
    Write-Log "xezim is now available in your terminal."
} else {
    Write-Warn "xezim may not be immediately available. Restart your terminal or run:"
    Write-Warn "  `$env:Path += `";$binDir`""
}

# ---- Smoke test ----
Write-Info "Running smoke test..."
try {
    $help = & $binary --help 2>&1
    if ($LASTEXITCODE -eq 0 -or $LASTEXITCODE -eq 1) {
        Write-Log "xezim --help works!"
    } else {
        Write-Warn "xezim --help returned unexpected exit code ($LASTEXITCODE)."
    }
} catch {
    Write-Warn "xezim --help did not run as expected."
}

# ---- Done ----
Write-Host ""
Write-Host "🎉  Installation Complete!" -ForegroundColor White
Write-Host ""
Write-Host "   Version:   $XezimTag"
Write-Host "   Binary:    $binary"
Write-Host "   Workspace: $Workspace"
Write-Host ""
Write-Host "   To verify, open a new terminal and run:"
Write-Host "      xezim --help"
Write-Host ""
Write-Host "   Try running:"
Write-Host "      cd $mainDir"
Write-Host "      .\target\release\xezim examples\full_adder.sv examples\tb_adder.sv"
Write-Host ""
Write-Host "   Update later:"
Write-Host "      irm https://raw.githubusercontent.com/aionhw/xezim/main/scripts/install.ps1 | iex"
Write-Host "      `$env:XEZIM_TAG='v0.9.6'; irm https://raw.githubusercontent.com/aionhw/xezim/main/scripts/install.ps1 | iex"
Write-Host ""
