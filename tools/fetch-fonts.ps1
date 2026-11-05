param([string]$Destination = "target/release/fonts")
$ErrorActionPreference = 'Stop'
# Pinned, unmodified Noto Sans CJK OTC: Japanese, Korean, Simplified/Traditional Chinese.
# Verify the upstream Git blob identifier before packaging third-party bytes.
$url = 'https://raw.githubusercontent.com/notofonts/noto-cjk/f8d157532fbfaeda587e826d4cd5b21a49186f7c/Sans/OTC/NotoSansCJK-Regular.ttc'
$expected = 'a2033d0e4e53f568c3f418a7d5d8c951af3f76c1'
New-Item -ItemType Directory -Path $Destination -Force | Out-Null
$path = Join-Path $Destination 'NotoSansCJK-Regular.ttc'
Invoke-WebRequest -Uri $url -OutFile $path
$bytes = [IO.File]::ReadAllBytes((Resolve-Path $path))
$header = [Text.Encoding]::ASCII.GetBytes("blob $($bytes.Length)`0")
$sha = [Security.Cryptography.SHA1]::Create()
try {
    [void]$sha.TransformBlock($header, 0, $header.Length, $header, 0)
    [void]$sha.TransformFinalBlock($bytes, 0, $bytes.Length)
    $actual = ([BitConverter]::ToString($sha.Hash)).Replace('-', '').ToLowerInvariant()
} finally { $sha.Dispose() }
if ($actual -ne $expected) { Remove-Item $path; throw "Noto font checksum mismatch: $actual" }
Copy-Item (Join-Path $PSScriptRoot '../docs/NOTO_OFL.txt') (Join-Path $Destination 'OFL.txt')
Write-Output "Verified Noto Sans CJK: $actual ($($bytes.Length) bytes)"
