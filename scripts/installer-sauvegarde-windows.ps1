<#
.SYNOPSIS
Crée (ou remplace) la tâche planifiée Windows qui lance scripts\sauvegarder.ps1 chaque jour.

.DESCRIPTION
La tâche s'appelle « Autoclair - sauvegarde », tourne sous le compte qui lance ce script, sans
mot de passe enregistré (donc seulement quand ce compte a une session ouverte, écran verrouillé
compris), à l'heure donnée, et se rattrape au prochain démarrage si le poste était éteint.
Elle appelle sauvegarder.ps1 du même dossier que ce script : ne pas déplacer le dépôt ensuite,
ou relancer l'installation.

.EXAMPLE
powershell -NoProfile -File scripts\installer-sauvegarde-windows.ps1 -Configuration C:\parc\.env, C:\parc-particulier\.env -Destination "C:\Users\moi\OneDrive\Sauvegardes Autoclair" -Heure 20:30

.EXAMPLE
powershell -NoProfile -File scripts\installer-sauvegarde-windows.ps1 -Essayer
Lance la tâche tout de suite et affiche son résultat (0 = réussite) : pour vérifier l'installation.

.EXAMPLE
powershell -NoProfile -File scripts\installer-sauvegarde-windows.ps1 -Supprimer
Retire la tâche. Les sauvegardes déjà faites restent.
#>
param(
  [Alias('Env')][string[]]$Configuration,
  [string]$Destination,
  [string]$Heure = '20:00',
  [int]$Conserver = 30,
  [string]$Nom = 'Autoclair - sauvegarde',
  [switch]$Essayer,
  [switch]$Supprimer
)

$ErrorActionPreference = 'Stop'
# Avec `powershell -File`, une liste passée en ligne de commande arrive en une seule chaîne « a,b » : on la sépare ici.
$Configuration = @($Configuration | ForEach-Object { $_ -split ',' } | ForEach-Object { $_.Trim().Trim('"') } | Where-Object { $_ })
[Console]::OutputEncoding = [System.Text.Encoding]::UTF8

if ($Supprimer) {
  if (Get-ScheduledTask -TaskName $Nom -ErrorAction SilentlyContinue) {
    Unregister-ScheduledTask -TaskName $Nom -Confirm:$false
    Write-Host "Tâche « $Nom » retirée."
  } else { Write-Host "Aucune tâche « $Nom »." }
  exit 0
}

if ($Essayer -and -not $Configuration) {
  $tache = Get-ScheduledTask -TaskName $Nom -ErrorAction SilentlyContinue
  if (-not $tache) { throw "Aucune tâche « $Nom » : l'installer d'abord (-Configuration et -Destination)." }
} else {
  if (-not $Configuration -or -not $Destination) { throw "Donner -Configuration (un ou plusieurs .env) et -Destination (dossier des sauvegardes)." }
  foreach ($f in $Configuration) { if (-not (Test-Path -LiteralPath $f -PathType Leaf)) { throw "Fichier introuvable : $f" } }
  if ($Heure -notmatch '^\d{1,2}:\d{2}$') { throw "-Heure attend HH:MM (par exemple 20:30)." }

  $script = Join-Path $PSScriptRoot 'sauvegarder.ps1'
  if (-not (Test-Path -LiteralPath $script)) { throw "sauvegarder.ps1 introuvable à côté de ce script ($script)." }
  $configs = ($Configuration | ForEach-Object { '"' + (Resolve-Path -LiteralPath $_).Path + '"' }) -join ','
  New-Item -ItemType Directory -Force -Path $Destination | Out-Null
  $dest = (Resolve-Path -LiteralPath $Destination).Path
  $arguments = "-NoProfile -ExecutionPolicy Bypass -WindowStyle Hidden -File `"$script`" -Configuration $configs -Destination `"$dest`" -Conserver $Conserver"

  $action = New-ScheduledTaskAction -Execute 'powershell.exe' -Argument $arguments
  $declencheur = New-ScheduledTaskTrigger -Daily -At $Heure
  $reglages = New-ScheduledTaskSettingsSet -StartWhenAvailable -MultipleInstances IgnoreNew `
    -ExecutionTimeLimit (New-TimeSpan -Hours 2) -AllowStartIfOnBatteries -DontStopIfGoingOnBatteries
  $compte = New-ScheduledTaskPrincipal -UserId ([System.Security.Principal.WindowsIdentity]::GetCurrent().Name) -LogonType Interactive -RunLevel Limited

  Register-ScheduledTask -TaskName $Nom -Action $action -Trigger $declencheur -Settings $reglages -Principal $compte -Force `
    -Description "Sauvegarde quotidienne de la base et des documents (scripts/sauvegarder.ps1 du dépôt gestion-parc-auto)." | Out-Null

  Write-Host "Tâche « $Nom » installée : chaque jour à $Heure, vers $dest" -ForegroundColor Green
  Write-Host "  Configurations : $configs"
  Write-Host "  Retirer : installer-sauvegarde-windows.ps1 -Supprimer ; essayer : -Essayer"
}

if ($Essayer) {
  Write-Host "Lancement de la tâche..."
  Start-ScheduledTask -TaskName $Nom
  for ($i = 0; $i -lt 120; $i++) {
    Start-Sleep -Seconds 1
    if ((Get-ScheduledTask -TaskName $Nom).State -ne 'Running') { break }
  }
  $info = Get-ScheduledTaskInfo -TaskName $Nom
  $resultat = $info.LastTaskResult
  if ($resultat -eq 0) { Write-Host ("Dernier passage : {0}, résultat 0 (réussite)." -f $info.LastRunTime) -ForegroundColor Green }
  else { Write-Host ("Dernier passage : {0}, résultat {1} : lire journal.txt dans le dossier des sauvegardes." -f $info.LastRunTime, $resultat) -ForegroundColor Red; exit 1 }
}
