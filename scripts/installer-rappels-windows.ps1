<#
.SYNOPSIS
Crée (ou remplace) la tâche planifiée Windows qui envoie le rappel par courriel chaque matin (#176).

.DESCRIPTION
La tâche lance scripts\envoyer-rappels.ps1 (du même dossier que ce script), qui appelle la
sous-commande `envoyer-rappels` depuis le dossier de l'installation (pour que son `.env` soit lu :
c'est là que sont les réglages COURRIEL_*), note chaque passage dans rappels.log, à côté du .env,
et retente un envoi qui échoue (#188). Elle tourne sous le compte qui lance ce script, sans mot de
passe enregistré (donc seulement quand ce compte a une session ouverte), à l'heure locale donnée,
changement d'heure compris, et se rattrape au prochain démarrage si le poste était éteint. Une
installation = une tâche : donner un -Nom différent pour la seconde. Ne pas déplacer le dépôt
ensuite, ou relancer l'installation.

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
  $script = Join-Path $PSScriptRoot 'envoyer-rappels.ps1'
  if (-not (Test-Path -LiteralPath $script)) { throw "envoyer-rappels.ps1 introuvable à côté de ce script ($script)." }
  $dossier = (Resolve-Path -LiteralPath $Installation).Path
  $exe = (Resolve-Path -LiteralPath $Executable).Path

  $arguments = "-NoProfile -ExecutionPolicy Bypass -WindowStyle Hidden -File `"$script`" -Installation `"$dossier`" -Executable `"$exe`""
  $action = New-ScheduledTaskAction -Execute 'powershell.exe' -Argument $arguments -WorkingDirectory $dossier
  $declencheur = New-ScheduledTaskTrigger -Daily -At $Heure
  # Windows inscrit l'heure en temps universel (« 19:00:00Z » pour 20:00 en hiver) : la tâche
  # glisserait d'une heure au changement d'heure. Sans décalage, elle suit l'heure locale (#188).
  $declencheur.StartBoundary = (Get-Date $Heure).ToString('yyyy-MM-ddTHH:mm:ss')
  # 30 minutes : trois essais à cinq minutes d'écart, et de la marge.
  $reglages = New-ScheduledTaskSettingsSet -StartWhenAvailable -MultipleInstances IgnoreNew `
    -ExecutionTimeLimit (New-TimeSpan -Minutes 30) -AllowStartIfOnBatteries -DontStopIfGoingOnBatteries
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
  # Le journal est dans le dossier de l'installation, celui d'où la tâche se lance.
  $journal = Join-Path (Get-ScheduledTask -TaskName $Nom).Actions[0].WorkingDirectory 'rappels.log'
  $derniere = if (Test-Path -LiteralPath $journal) { Get-Content -LiteralPath $journal -Encoding UTF8 -Tail 1 } else { '(pas encore de journal)' }
  if ((Get-ScheduledTask -TaskName $Nom).State -eq 'Running') {
    Write-Host "Le premier essai a échoué ; la tâche retente dans cinq minutes. Suivre $journal :" -ForegroundColor Yellow
    Write-Host "  $derniere"
    exit 1
  }
  $info = Get-ScheduledTaskInfo -TaskName $Nom
  if ($info.LastTaskResult -eq 0) { Write-Host ("Dernier passage : {0}, résultat 0 (envoyé, ou rien à rappeler)." -f $info.LastRunTime) -ForegroundColor Green }
  else { Write-Host ("Dernier passage : {0}, résultat {1}." -f $info.LastRunTime, $info.LastTaskResult) -ForegroundColor Red }
  Write-Host "  $derniere"
  if ($info.LastTaskResult -ne 0) { exit 1 }
}
