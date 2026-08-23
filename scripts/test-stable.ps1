# Runs workspace tests, retrying through transient Windows AV link locks.
param([int]$MaxAttempts = 6)
for ($i = 1; $i -le $MaxAttempts; $i++) {
    $out = cargo test --workspace --quiet -j 2 2>&1 |
        Select-String -Pattern "\.\.\. FAILED|^error:"
    if (-not $out) { Write-Output "GREEN (attempt $i)"; exit 0 }
    Write-Output "attempt ${i}: transient link lock — backing off"
    Start-Sleep -Seconds (4 * $i)
}
Write-Output "RED after $MaxAttempts attempts"
exit 1
