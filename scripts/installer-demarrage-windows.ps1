<#
.SYNOPSIS
Crée (ou remplace) la tâche planifiée Windows qui démarre le serveur d'une installation à
l'ouverture de session (#188).

.DESCRIPTION
Sans elle, un redémarrage du poste laisse l'application arrêtée jusqu'à ce qu'on la relance à la
main. La tâche lance scripts\lancer-serveur.ps1 (du même dossier que ce script) quand le compte qui
l'installe ouvre sa session, sans fenêtre : le serveur écrit son journal dans serveur.log, dans le
dossier de l'installation, et il est relancé s'il s'arrête au démarrage parce que la base n'est
pas encore prête. Une installation = une tâche : donner un -Nom différent pour la seconde. Ne pas
déplacer le dépôt ensuite, ou relancer l'installation.

Le serveur tourne alors sans fenêtre : pour l'arrêter, arrêter le processus gestion-parc-auto.exe
(Gestionnaire des tâches). Il n'est pas relancé : il avait tourné plus d'une minute.

.EXAMPLE
powershell -NoProfile -File scripts\installer-demarrage-windows.ps1 -Installation C:\parc -Executable C:\parc\gestion-parc-auto.exe -Essayer
Installe la tâche, puis la lance tout de suite et attend que le serveur réponde.

.EXAMPLE
powershell -NoProfile -File scripts\installer-demarrage-windows.ps1 -Nom "Autoclair - serveur" -Supprimer
Retire la tâche. Un serveur déjà lancé continue de tourner.
#>
param(
  [string]$Installation,
  [string]$Executable,
  [string]$Nom = 'Autoclair - serveur',
  [switch]$Essayer,
  [switch]$Supprimer
)

$ErrorActionPreference = 'Stop'
[Console]::OutputEncoding = [System.Text.Encoding]::UTF8

if ($Supprimer) {
  if (Get-ScheduledTask -TaskName $Nom -ErrorAction SilentlyContinue) {
    Unregister-ScheduledTask -TaskName $Nom -Confirm:$false
    Write-Host "Tâche « $Nom » retirée. Un serveur déjà lancé continue de tourner."
  } else { Write-Host "Aucune tâche « $Nom »." }
  exit 0
}

if (-not $Installation -or -not $Executable) { throw "Donner -Installation (le dossier qui contient .env) et -Executable (gestion-parc-auto.exe)." }
if (-not (Test-Path -LiteralPath (Join-Path $Installation '.env') -PathType Leaf)) { throw "Pas de .env dans $Installation." }
if (-not (Test-Path -LiteralPath $Executable -PathType Leaf)) { throw "Exécutable introuvable : $Executable" }
$script = Join-Path $PSScriptRoot 'lancer-serveur.ps1'
if (-not (Test-Path -LiteralPath $script)) { throw "lancer-serveur.ps1 introuvable à côté de ce script ($script)." }
$dossier = (Resolve-Path -LiteralPath $Installation).Path
$exe = (Resolve-Path -LiteralPath $Executable).Path

$arguments = "-NoProfile -ExecutionPolicy Bypass -WindowStyle Hidden -File `"$script`" -Installation `"$dossier`" -Executable `"$exe`""
$action = New-ScheduledTaskAction -Execute 'powershell.exe' -Argument $arguments -WorkingDirectory $dossier
$utilisateur = [System.Security.Principal.WindowsIdentity]::GetCurrent().Name
$declencheur = New-ScheduledTaskTrigger -AtLogOn -User $utilisateur
# Pas de durée maximale : un serveur tourne tant que la session est ouverte (par défaut, Windows
# arrête une tâche au bout de trois jours).
$reglages = New-ScheduledTaskSettingsSet -MultipleInstances IgnoreNew -ExecutionTimeLimit ([TimeSpan]::Zero) `
  -AllowStartIfOnBatteries -DontStopIfGoingOnBatteries
$compte = New-ScheduledTaskPrincipal -UserId $utilisateur -LogonType Interactive -RunLevel Limited
Register-ScheduledTask -TaskName $Nom -Action $action -Trigger $declencheur -Settings $reglages -Principal $compte -Force `
  -Description "Démarre le serveur gestion-parc-auto de $dossier à l'ouverture de session (journal : serveur.log)." | Out-Null
Write-Host "Tâche « $Nom » installée : à l'ouverture de session, le serveur de $dossier (journal : $dossier\serveur.log)." -ForegroundColor Green

if ($Essayer) {
  $port = 3000
  foreach ($ligne in Get-Content -LiteralPath (Join-Path $dossier '.env')) {
    if ($ligne -match '^\s*PORT\s*=\s*["'']?(\d+)') { $port = [int]$Matches[1] }
  }
  Write-Host "Lancement de la tâche, puis attente du serveur sur le port $port..."
  Start-ScheduledTask -TaskName $Nom
  for ($i = 0; $i -lt 60; $i++) {
    Start-Sleep -Seconds 1
    try {
      $r = Invoke-WebRequest "http://127.0.0.1:$port/health" -UseBasicParsing -TimeoutSec 2
      if ($r.StatusCode -eq 200) { Write-Host "Le serveur répond sur le port $port." -ForegroundColor Green; exit 0 }
    } catch { }
  }
  Write-Host "Pas de réponse sur le port $port au bout d'une minute : lire $dossier\serveur.log." -ForegroundColor Red
  exit 1
}
