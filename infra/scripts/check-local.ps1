$ErrorActionPreference = "Stop"

Write-Host "Checking APMTRACK local endpoints..."

$api = Invoke-RestMethod "http://localhost:8080/health"
Write-Host "API:" $api.status $api.service $api.version

$db = Invoke-RestMethod "http://localhost:8080/health/db"
Write-Host "DB:" $db.status $db.database

Write-Host "Done."

