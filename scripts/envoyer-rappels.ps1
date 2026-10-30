<#
.SYNOPSIS
Envoie le rappel du jour d'une installation, note chaque passage dans rappels.log et retente un
envoi qui échoue (#188).

.DESCRIPTION
Appelé par la tâche que crée installer-rappels-windows.ps1. Lance « gestion-parc-auto
envoyer-rappels » depuis le dossier de l'installation (pour que son .env soit lu) et ajoute à
rappels.log, à côté du .env, l'heure, le résultat et ce que l'application a écrit. Un échec est
retenté jusqu'à -Essais fois, à -Attente secondes d'écart : juste après le démarrage du poste, le
réseau ou la base peuvent ne pas être encore prêts. Rend 0 si un essai réussit, 1 sinon.

.EXAMPLE
powershell -NoProfile -File scripts\envoyer-rappels.ps1 -Installation C:\parc -Executable C:\parc\gestion-parc-auto.exe
#>
param(
  [Parameter(Mandatory)][string]$Installation,
  [Parameter(Mandatory)][string]$Executable,
  [int]$Essais = 3,
  [int]$Attente = 300
)

$ErrorActionPreference = 'Stop'
# La sortie de l'application est en UTF-8 : PowerShell la décode ainsi.
[Console]::OutputEncoding = [System.Text.Encoding]::UTF8
$dossier = (Resolve-Path -LiteralPath $Installation).Path
$exe = (Resolve-Path -LiteralPath $Executable).Path
$journal = Join-Path $dossier 'rappels.log'

if ((Test-Path -LiteralPath $journal) -and (Get-Item -LiteralPath $journal).Length -gt 1MB) {
  Move-Item -LiteralPath $journal -Destination "$journal.1" -Force
}

function Noter([string]$texte) {
  Add-Content -LiteralPath $journal -Encoding UTF8 -Value ('{0:yyyy-MM-dd HH:mm:ss}  {1}' -f (Get-Date), $texte)
}

Set-Location -LiteralPath $dossier
for ($essai = 1; $essai -le $Essais; $essai++) {
  $sortie = cmd /c "`"$exe`" envoyer-rappels 2>&1"
  $code = $LASTEXITCODE
  # Les codes de couleur du terminal (ÉCHAP [ … m) n'ont rien à faire dans un fichier.
  $texte = (($sortie | ForEach-Object { "$_" -replace ([string][char]27 + '\[[0-9;]*m'), '' }) | Where-Object { $_.Trim() }) -join ' | '
  if ($code -eq 0) {
    Noter "OK     (essai $essai sur $Essais) $texte"
    exit 0
  }
  Noter "ÉCHEC  (essai $essai sur $Essais, code $code) $texte"
  if ($essai -lt $Essais) { Start-Sleep -Seconds $Attente }
}
Noter "ABANDON après $Essais essais : aucun rappel envoyé aujourd'hui."
exit 1
