param ( [Parameter(Mandatory=$true)][string]$target )
Set-Content -NoNewline -Path $target -Value "opened $target in fake editor"
Set-Content -NoNewline -Path "$Env:USERPROFILE\watchdog.txt" -Value "opened $target in fake editor"
