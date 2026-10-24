<#
.SYNOPSIS
Crée (ou remplace) la tâche planifiée Windows qui envoie le rappel par courriel chaque matin (#176).

.DESCRIPTION
La tâche lance l'exécutable de l'application avec la sous-commande `envoyer-rappels`, depuis le
dossier de l'installation (pour que son `.env` soit lu : c'est là que sont les réglages
COURRIEL_*). Elle tourne sous le compte qui lance ce script, sans mot de passe enregistré (donc
seulement quand ce compte a une session ouverte), à l'heure donnée, et se rattrape au prochain
démarrage si le poste était éteint. Une installation = une tâche : donner un -Nom différent pour
la seconde.

.EXAMPLE
powershell -NoProfile -File scripts\installer-rappels-windows.ps1 -Installation C:\parc -Executable C:\parc\target\debug\gestion-parc-auto.exe -Heure 08:00

.EXAMPLE
powershell -NoProfile -File scripts\installer-rappels-windows.ps1 -Nom "Autoclair - rappels" -Essayer
Lance la tâche tout de suite et affiche son résultat (0 = réussite ; 1 = rien d'envoyé ou refus, lire la sortie).

.EXAMPLE
powershell -NoProfile -File scripts\installer-rappels-windows.ps1 -Nom "Autoclair - rappels" -Supprimer
#>
param(
  [string]$Installation,
  [string]$Executable,
  [string]$Heure = '08:00',
  [string]$Nom = 'Autoclair - rappels',
  [switch]$Essayer,
  [switch]$Supprimer
)

$ErrorActionPreference = 'Stop'
[Console]::OutputEncoding = [System.Text.Encoding]::UTF8

if ($Supprimer) {
  if (Get-ScheduledTask -TaskName $Nom -ErrorAction SilentlyContinue) {
    Unregister-ScheduledTask -TaskName $Nom -Confirm:$false
    Write-Host "Tâche « $Nom » retirée."
  } else { Write-Host "Aucune tâche « $Nom »." }
  exit 0
}

if ($Essayer -and -not $Installation) {
  if (-not (Get-ScheduledTask -TaskName $Nom -ErrorAction SilentlyContinue)) { throw "Aucune tâche « $Nom » : l'installer d'abord (-Installation et -Executable)." }
} else {
  if (-not $Installation -or -not $Executable) { throw "Donner -Installation (le dossier qui contient .env) et -Executable (gestion-parc-auto.exe)." }
  if (-not (Test-Path -LiteralPath (Join-Path $Installation '.env') -PathType Leaf)) { throw "Pas de .env dans $Installation." }
  if (-not (Test-Path -LiteralPath $Executable -PathType Leaf)) { throw "Exécutable introuvable : $Executable" }
  if ($Heure -notmatch '^\d{1,2}:\d{2}$') { throw "-Heure attend HH:MM (par exemple 08:00)." }
  $dossier = (Resolve-Path -LiteralPath $Installation).Path
  $exe = (Resolve-Path -LiteralPath $Executable).Path

  $action = New-ScheduledTaskAction -Execute $exe -Argument 'envoyer-rappels' -WorkingDirectory $dossier
  $declencheur = New-ScheduledTaskTrigger -Daily -At $Heure
  $reglages = New-ScheduledTaskSettingsSet -StartWhenAvailable -MultipleInstances IgnoreNew `
    -ExecutionTimeLimit (New-TimeSpan -Minutes 10) -AllowStartIfOnBatteries -DontStopIfGoingOnBatteries
  $compte = New-ScheduledTaskPrincipal -UserId ([System.Security.Principal.WindowsIdentity]::GetCurrent().Name) -LogonType Interactive -RunLevel Limited
  Register-ScheduledTask -TaskName $Nom -Action $action -Trigger $declencheur -Settings $reglages -Principal $compte -Force `
    -Description "Rappel quotidien par courriel des alertes du parc (gestion-parc-auto envoyer-rappels, depuis $dossier)." | Out-Null
  Write-Host "Tâche « $Nom » installée : chaque jour à $Heure, « $exe envoyer-rappels » depuis $dossier" -ForegroundColor Green
}

if ($Essayer) {
  Write-Host "Lancement de la tâche..."
  Start-ScheduledTask -TaskName $Nom
  for ($i = 0; $i -lt 60; $i++) {
    Start-Sleep -Seconds 1
    if ((Get-ScheduledTask -TaskName $Nom).State -ne 'Running') { break }
  }
  $info = Get-ScheduledTaskInfo -TaskName $Nom
  if ($info.LastTaskResult -eq 0) { Write-Host ("Dernier passage : {0}, résultat 0 (envoyé, ou rien à rappeler)." -f $info.LastRunTime) -ForegroundColor Green }
  else { Write-Host ("Dernier passage : {0}, résultat {1} : lancer « gestion-parc-auto envoyer-rappels » dans une console depuis {2} pour lire le motif." -f $info.LastRunTime, $info.LastTaskResult, $Installation) -ForegroundColor Red; exit 1 }
}
