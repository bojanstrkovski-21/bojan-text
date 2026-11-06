# Pull, commit everything, and push this repo.
# Windows version of push.sh. Uses the current remote and the main branch.
# Sign-in is the GitHub account already saved in Windows Credential Manager.
#
#   .\push.ps1
#   .\push.ps1 "commit message"

param(
    [Parameter(ValueFromRemainingArguments = $true)]
    [string[]]$Message
)

Set-Location -Path $PSScriptRoot

# Git Credential Manager reads these and uses Windows Credential Manager.
# The saved target is git:https://github.com. This script never asks for a token.
$env:GCM_CREDENTIAL_STORE = "wincredman"
$env:GIT_TERMINAL_PROMPT = "0"
$env:GIT_EDITOR = "true"

$credLines = @(& cmd.exe /c "cmdkey /list")
$account = $null
for ($i = 0; $i -lt $credLines.Count; $i++) {
    if ($credLines[$i] -notmatch "target=git:https://github.com") { continue }
    $limit = [Math]::Min($i + 6, $credLines.Count)
    for ($j = $i + 1; $j -lt $limit; $j++) {
        if ($credLines[$j] -match "Target:") { break }
        if ($credLines[$j] -match "^\s*User:\s*(.+)\s*$") {
            $account = $Matches[1].Trim()
            break
        }
    }
    break
}

if (-not $account) {
    Write-Host "No GitHub login in Windows Credential Manager."
    Write-Host "Sign in once, then run this again:"
    Write-Host "  git credential-manager github login"
    exit 1
}

$env:GCM_GITHUB_ACCOUNT = $account
Write-Host "Using Windows Credential Manager ($account)"

Write-Host "Checking for newer files online first"
git -c credential.helper=manager -c credential.credentialStore=wincredman pull --no-rebase
if ($LASTEXITCODE -ne 0) { exit $LASTEXITCODE }

git add --all .
if ($LASTEXITCODE -ne 0) { exit $LASTEXITCODE }

git diff --cached --quiet
$nothingToCommit = $LASTEXITCODE -eq 0

if ($nothingToCommit) {
    Write-Host "Nothing to commit."
    git -c credential.helper=manager -c credential.credentialStore=wincredman push -u origin main
    exit $LASTEXITCODE
}

$comment = ""
if ($Message) { $comment = ($Message -join " ").Trim() }

if (-not $comment) {
    Write-Host "####################################"
    Write-Host "Write your commit comment!"
    Write-Host "####################################"
    $comment = Read-Host
}

if (-not $comment) {
    Write-Host "Commit comment is empty. Stopped."
    exit 1
}

git commit -m $comment
if ($LASTEXITCODE -ne 0) { exit $LASTEXITCODE }

git -c credential.helper=manager -c credential.credentialStore=wincredman push -u origin main
if ($LASTEXITCODE -ne 0) { exit $LASTEXITCODE }

Write-Host "################################################################"
Write-Host "###################    Git Push Done      ######################"
Write-Host "################################################################"
