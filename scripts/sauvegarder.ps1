<#
.SYNOPSIS
Sauvegarde la base et les documents d'une ou plusieurs installations (Windows).

.DESCRIPTION
Pour chaque fichier .env donné, le script lit DATABASE_URL et DOCUMENTS_DIR, puis :
  1. écrit la base dans <Destination>\<nom de la base>\base-AAAA-MM-JJ-HHMMSS.dump
     (pg_dump --format=custom : compressé, restaurable table par table) ;
  2. copie les documents nouveaux ou modifiés dans <Destination>\<nom de la base>\documents\
     (robocopy sans /MIR : un document supprimé dans l'application reste dans la copie) ;
  3. supprime les .dump plus vieux que -Conserver jours, en gardant toujours les trois derniers ;
  4. ajoute une ligne à <Destination>\journal.txt.

Le mot de passe de la base ne quitte pas le .env : il est passé à pg_dump par la variable
d'environnement PGPASSWORD, jamais sur la ligne de commande ni dans le journal.

Code de sortie : 0 si chaque installation a été sauvegardée, 1 sinon (le détail est dans le
journal et à l'écran). Une installation en échec n'empêche pas les suivantes.

.EXAMPLE
powershell -NoProfile -File scripts\sauvegarder.ps1 -Configuration C:\parc\.env, C:\parc-particulier\.env -Destination "C:\Users\moi\OneDrive\Sauvegardes Autoclair"

.PARAMETER Configuration
Un ou plusieurs fichiers .env (un par installation). DOCUMENTS_DIR relatif s'entend depuis le dossier du .env.

.PARAMETER Destination
Dossier qui reçoit les sauvegardes ; créé s'il n'existe pas. De préférence hors de la machine (dossier synchronisé, disque externe).

.PARAMETER Conserver
Nombre de jours pendant lesquels les .dump sont gardés (30 par défaut). Les documents copiés ne sont jamais supprimés.

.PARAMETER PgDump
Chemin de pg_dump.exe. Par défaut : celui du PATH, sinon le plus récent sous C:\Program Files\PostgreSQL\*\bin.
#>
param(
  [Parameter(Mandatory = $true)][Alias('Env')][string[]]$Configuration,
  [Parameter(Mandatory = $true)][string]$Destination,
  [int]$Conserver = 30,
  [string]$PgDump = ''
)

$ErrorActionPreference = 'Stop'
# Avec `powershell -File`, une liste passée en ligne de commande arrive en une seule chaîne « a,b » : on la sépare ici.
$Configuration = @($Configuration | ForEach-Object { $_ -split ',' } | ForEach-Object { $_.Trim().Trim('"') } | Where-Object { $_ })
[Console]::OutputEncoding = [System.Text.Encoding]::UTF8
$utf8SansBom = New-Object System.Text.UTF8Encoding($false)

function Journal($texte) {
  $ligne = "{0}  {1}" -f (Get-Date -Format 'yyyy-MM-dd HH:mm:ss'), $texte
  Write-Host $ligne
  [System.IO.File]::AppendAllText((Join-Path $Destination 'journal.txt'), $ligne + [Environment]::NewLine, $utf8SansBom)
}

# Lit un fichier .env : une variable par ligne, `CLE=valeur`, guillemets facultatifs, `#` pour un commentaire.
function Lire-Env($chemin) {
  $valeurs = @{}
  foreach ($ligne in Get-Content -LiteralPath $chemin -Encoding UTF8) {
    if ($ligne -match '^\s*(#|$)') { continue }
    if ($ligne -match '^\s*(?:export\s+)?([A-Za-z_][A-Za-z0-9_]*)\s*=\s*(.*?)\s*$') {
      $valeur = $Matches[2]
      if ($valeur -match '^"(.*)"$' -or $valeur -match "^'(.*)'$") { $valeur = $Matches[1] }
      $valeurs[$Matches[1]] = $valeur
    }
  }
  return $valeurs
}

# Sépare le mot de passe du reste de l'URL : pg_dump reçoit l'URL sans lui, et lui par PGPASSWORD.
function Decomposer-Url($url) {
  if ($url -notmatch '^(postgres(?:ql)?://)(?:([^:/@]+)(?::([^@]*))?@)?([^/?#]+)/([^?#]+)(.*)$') {
    throw "DATABASE_URL n'a pas la forme postgres://utilisateur:motdepasse@hote:port/base"
  }
  $utilisateur = $Matches[2]
  $motDePasse = if ($null -ne $Matches[3]) { [Uri]::UnescapeDataString($Matches[3]) } else { $null }
  $sansMotDePasse = $Matches[1] + $(if ($utilisateur) { "$utilisateur@" } else { '' }) + $Matches[4] + '/' + $Matches[5] + $Matches[6]
  return @{ Base = [Uri]::UnescapeDataString($Matches[5]); Url = $sansMotDePasse; MotDePasse = $motDePasse }
}

function Trouver-PgDump() {
  if ($PgDump) { return $PgDump }
  $cmd = Get-Command pg_dump.exe -ErrorAction SilentlyContinue
  if ($cmd) { return $cmd.Source }
  $candidats = Get-ChildItem 'C:\Program Files\PostgreSQL\*\bin\pg_dump.exe' -ErrorAction SilentlyContinue |
    Sort-Object { [int]($_.Directory.Parent.Name -replace '\D', '') } -Descending
  if ($candidats) { return $candidats[0].FullName }
  throw "pg_dump.exe introuvable : installer les outils client de PostgreSQL ou donner -PgDump."
}

function Sauvegarder-Base($pgDump, $url, $motDePasse, $fichier) {
  $erreurs = [System.IO.Path]::GetTempFileName()
  try {
    if ($null -ne $motDePasse) { $env:PGPASSWORD = $motDePasse }
    $p = Start-Process -FilePath $pgDump -NoNewWindow -Wait -PassThru -RedirectStandardError $erreurs `
      -ArgumentList @('--dbname', "`"$url`"", '--format=custom', '--no-owner', '--file', "`"$fichier`"")
    if ($p.ExitCode -ne 0) {
      Remove-Item -LiteralPath $fichier -ErrorAction SilentlyContinue
      $detail = (Get-Content -LiteralPath $erreurs -Raw -ErrorAction SilentlyContinue)
      throw ("pg_dump a rendu le code {0} : {1}" -f $p.ExitCode, ($detail -replace '\s+', ' ').Trim())
    }
  } finally {
    Remove-Item Env:\PGPASSWORD -ErrorAction SilentlyContinue
    Remove-Item -LiteralPath $erreurs -ErrorAction SilentlyContinue
  }
}

# robocopy copie ce qui est nouveau ou modifié, ne supprime rien. Codes 0 à 7 : réussite ; 8 et plus : échec.
function Copier-Documents($source, $cible) {
  $avant = @(Get-ChildItem -LiteralPath $cible -Recurse -File -ErrorAction SilentlyContinue).Count
  & robocopy.exe $source $cible /E /R:2 /W:2 /NP /NFL /NDL /NJH /NJS | Out-Null
  $code = $LASTEXITCODE
  if ($code -ge 8) { throw "robocopy a rendu le code $code en copiant $source" }
  $apres = @(Get-ChildItem -LiteralPath $cible -Recurse -File).Count
  return @{ Nouveaux = ($apres - $avant); Total = $apres }
}

function Faire-De-La-Place($dossier) {
  $limite = (Get-Date).AddDays(-$Conserver)
  $dumps = @(Get-ChildItem -LiteralPath $dossier -Filter 'base-*.dump' | Sort-Object LastWriteTime -Descending)
  $supprimes = 0
  foreach ($d in ($dumps | Select-Object -Skip 3)) {
    if ($d.LastWriteTime -lt $limite) { Remove-Item -LiteralPath $d.FullName; $supprimes++ }
  }
  return $supprimes
}

# --- Déroulement ---------------------------------------------------------------------------

New-Item -ItemType Directory -Force -Path $Destination | Out-Null
$Destination = (Resolve-Path -LiteralPath $Destination).Path
$horodatage = Get-Date -Format 'yyyy-MM-dd-HHmmss'
$echecs = 0

$pgDumpExe = $null
try { $pgDumpExe = Trouver-PgDump } catch { Journal ("ECHEC  {0}" -f $_.Exception.Message); exit 1 }

foreach ($fichierEnv in $Configuration) {
  $nom = $fichierEnv
  try {
    if (-not (Test-Path -LiteralPath $fichierEnv -PathType Leaf)) { throw "fichier introuvable" }
    $reglages = Lire-Env $fichierEnv
    if (-not $reglages['DATABASE_URL']) { throw "DATABASE_URL absent de $fichierEnv" }
    $connexion = Decomposer-Url $reglages['DATABASE_URL']
    $nom = $connexion.Base

    $dossier = Join-Path $Destination $connexion.Base
    New-Item -ItemType Directory -Force -Path $dossier | Out-Null

    $dump = Join-Path $dossier "base-$horodatage.dump"
    Sauvegarder-Base $pgDumpExe $connexion.Url $connexion.MotDePasse $dump
    $taille = [math]::Round((Get-Item -LiteralPath $dump).Length / 1KB)

    $documents = $reglages['DOCUMENTS_DIR']
    if (-not $documents) { $documents = './documents' }
    if (-not [System.IO.Path]::IsPathRooted($documents)) {
      $documents = Join-Path (Split-Path -Parent (Resolve-Path -LiteralPath $fichierEnv).Path) $documents
    }
    $copie = @{ Nouveaux = 0; Total = 0 }
    $noteDocuments = 'aucun dossier de documents'
    if (Test-Path -LiteralPath $documents -PathType Container) {
      $copie = Copier-Documents (Resolve-Path -LiteralPath $documents).Path (Join-Path $dossier 'documents')
      $noteDocuments = "documents +{0} ({1} au total)" -f $copie.Nouveaux, $copie.Total
    }

    $supprimes = Faire-De-La-Place $dossier
    Journal ("OK     {0}  base {1} Ko ({2}), {3}, {4} ancien(s) dump(s) supprimé(s)" -f $nom, $taille, (Split-Path -Leaf $dump), $noteDocuments, $supprimes)
  } catch {
    $echecs++
    Journal ("ECHEC  {0}  {1}" -f $nom, $_.Exception.Message)
  }
}

if ($echecs -gt 0) { exit 1 }
exit 0
