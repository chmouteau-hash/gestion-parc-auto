<#
.SYNOPSIS
Lance le serveur d'une installation sans fenêtre, avec son journal dans serveur.log (#188).

.DESCRIPTION
Appelé par la tâche que crée installer-demarrage-windows.ps1, à l'ouverture de session. Le serveur
tourne depuis le dossier de l'installation, pour que son .env soit lu ; tout ce qu'il écrit va dans
serveur.log, à côté du .env.

Juste après le démarrage du poste, la base peut ne pas être encore prête : un serveur qui s'arrête
moins d'une minute après son lancement est relancé, jusqu'à -Essais fois en tout, puis le script
abandonne en le notant. Un serveur qui a tourné plus longtemps a été arrêté exprès (mise à jour,
arrêt du poste) : il n'est pas relancé. Si le port de l'installation est déjà occupé (serveur déjà
lancé), rien n'est lancé.

.EXAMPLE
powershell -NoProfile -File scripts\lancer-serveur.ps1 -Installation C:\parc -Executable C:\parc\gestion-parc-auto.exe
#>
param(
  [Parameter(Mandatory)][string]$Installation,
  [Parameter(Mandatory)][string]$Executable,
  [int]$Essais = 5,
  [int]$Attente = 30
)

$ErrorActionPreference = 'Stop'
# La sortie du serveur est en UTF-8 : PowerShell la lit ainsi.
[Console]::OutputEncoding = [System.Text.Encoding]::UTF8
$dossier = (Resolve-Path -LiteralPath $Installation).Path
$exe = (Resolve-Path -LiteralPath $Executable).Path
$journal = Join-Path $dossier 'serveur.log'

# Au-delà de 5 Mo, le journal passe en serveur.log.1 (l'ancien .1 est remplacé) : il ne grossit pas
# sans fin. Si un serveur l'écrit encore, on le laisse grossir jusqu'à la fois suivante.
if ((Test-Path -LiteralPath $journal) -and (Get-Item -LiteralPath $journal).Length -gt 5MB) {
  try { Move-Item -LiteralPath $journal -Destination "$journal.1" -Force } catch { }
}

# Le journal s'ouvre en partage : un second lancement peut y noter « port déjà occupé » pendant
# qu'un serveur y écrit.
$utf8 = New-Object System.Text.UTF8Encoding($false)
function Ouvrir {
  $flux = New-Object System.IO.FileStream($journal, [System.IO.FileMode]::Append, [System.IO.FileAccess]::Write, [System.IO.FileShare]::ReadWrite)
  $ecrivain = New-Object System.IO.StreamWriter($flux, $utf8)
  $ecrivain.AutoFlush = $true
  $ecrivain
}
function Noter([string]$texte) {
  $ecrivain = Ouvrir
  try { $ecrivain.WriteLine(('{0:yyyy-MM-dd HH:mm:ss}  {1}' -f (Get-Date), $texte)) } finally { $ecrivain.Dispose() }
}

# Le port de l'installation : la ligne PORT= de son .env, 3000 sans elle (comme l'application).
$port = 3000
$fichierEnv = Join-Path $dossier '.env'
if (Test-Path -LiteralPath $fichierEnv) {
  foreach ($ligne in Get-Content -LiteralPath $fichierEnv) {
    if ($ligne -match '^\s*PORT\s*=\s*["'']?(\d+)') { $port = [int]$Matches[1] }
  }
}

for ($essai = 1; $essai -le $Essais; $essai++) {
  if (Get-NetTCPConnection -LocalPort $port -State Listen -ErrorAction SilentlyContinue) {
    Noter "Port $port déjà occupé : un serveur tourne déjà, rien n'est lancé."
    exit 0
  }
  Noter "Démarrage du serveur (essai $essai sur $Essais), port $port : $exe"
  $debut = Get-Date
  Set-Location -LiteralPath $dossier
  # Chaque ligne du serveur est recopiée au journal à mesure. Une redirection de cmd (>>) garderait
  # le fichier fermé aux autres écritures tant que le serveur tourne.
  $sortie = Ouvrir
  try { cmd /c "`"$exe`" 2>&1" | ForEach-Object { $sortie.WriteLine("$_") } } finally { $sortie.Dispose() }
  $code = $LASTEXITCODE
  $duree = [int]((Get-Date) - $debut).TotalSeconds
  Noter "Serveur arrêté après $duree s, code $code."
  if ($duree -ge 60) { exit $code }
  if ($essai -lt $Essais) { Start-Sleep -Seconds $Attente }
}
Noter "Abandon : le serveur s'est arrêté au démarrage $Essais fois de suite. Le motif est dans les lignes ci-dessus."
exit 1
