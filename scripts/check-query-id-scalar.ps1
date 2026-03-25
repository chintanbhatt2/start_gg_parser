$ErrorActionPreference = 'Stop'

$root = Split-Path -Parent $PSScriptRoot
$queriesDir = Join-Path $root 'src-tauri/src/queries'

if (-not (Test-Path $queriesDir)) {
  Write-Error "Queries directory not found: $queriesDir"
  exit 1
}

$files = Get-ChildItem -Path $queriesDir -Filter '*.rs' -File -Recurse |
  Where-Object { $_.Name -ne 'scalars.rs' }

# Reject direct usage of cynic::Id or bare Id in query fields/imports.
$patterns = @(
  'cynic::Id',
  '\bOption\s*<\s*Id\s*>',
  '\bpub\s+\w+\s*:\s*Id\b',
  '\buse\s+cynic\s*::\s*\{[^}]*\bId\b[^}]*\}'
)

$matches = @()

foreach ($file in $files) {
  foreach ($pattern in $patterns) {
    $found = Select-String -Path $file.FullName -Pattern $pattern
    if ($found) {
      $matches += $found
    }
  }
}

if ($matches.Count -gt 0) {
  Write-Host 'Found raw GraphQL ID usage in query modules. Use StartggId from queries/scalars.rs instead.' -ForegroundColor Red
  foreach ($m in $matches) {
    $relativePath = Resolve-Path -Relative $m.Path
    Write-Host ("{0}:{1}: {2}" -f $relativePath, $m.LineNumber, $m.Line.Trim())
  }
  exit 1
}

Write-Host 'OK: query ID usage is standardized to StartggId.' -ForegroundColor Green
exit 0
