#!/usr/bin/env bash
# Sauvegarde la base et les documents d'une ou plusieurs installations (Linux, macOS, Git Bash).
#
#   scripts/sauvegarder.sh -d DESTINATION [-c JOURS] FICHIER.env [FICHIER.env ...]
#
# Pour chaque .env : lit DATABASE_URL et DOCUMENTS_DIR, puis
#   1. écrit la base dans DESTINATION/<nom de la base>/base-AAAA-MM-JJ-HHMMSS.dump (pg_dump --format=custom) ;
#   2. copie les documents nouveaux dans DESTINATION/<nom de la base>/documents/ (cp -n : rien n'est
#      supprimé dans la copie, même un document supprimé dans l'application) ;
#   3. supprime les .dump plus vieux que JOURS jours (30 par défaut), en gardant toujours les trois derniers ;
#   4. ajoute une ligne à DESTINATION/journal.txt.
#
# Le mot de passe de la base ne quitte pas le .env : il est passé à pg_dump par PGPASSWORD, jamais sur la
# ligne de commande ni dans le journal. Code de sortie : 0 si chaque installation a été sauvegardée, 1 sinon.
#
# Planifier chaque nuit (crontab -e) :
#   15 2 * * * /chemin/scripts/sauvegarder.sh -d /sauvegardes/autoclair /srv/autoclair/.env /srv/autoclair-pro/.env
set -u

destination=''
conserver=30
while getopts 'd:c:h' option; do
  case "$option" in
    d) destination=$OPTARG ;;
    c) conserver=$OPTARG ;;
    *) sed -n '2,14p' "$0"; exit 2 ;;
  esac
done
shift $((OPTIND - 1))
if [ -z "$destination" ] || [ $# -eq 0 ]; then sed -n '2,14p' "$0"; exit 2; fi

mkdir -p "$destination"
journal="$destination/journal.txt"
horodatage=$(date +%Y-%m-%d-%H%M%S)
echecs=0

journal() {
  ligne="$(date '+%Y-%m-%d %H:%M:%S')  $*"
  echo "$ligne"
  echo "$ligne" >> "$journal"
}

# Valeur d'une variable d'un .env : `CLE=valeur`, guillemets facultatifs, `#` pour un commentaire.
lire_env() {
  grep -E "^[[:space:]]*(export[[:space:]]+)?$2[[:space:]]*=" "$1" | tail -n 1 \
    | sed -E 's/^[^=]*=[[:space:]]*//; s/[[:space:]]*$//; s/^"(.*)"$/\1/; s/^'"'"'(.*)'"'"'$/\1/'
}

# Décode les %XX d'une URL (le mot de passe peut en contenir).
decoder() { printf '%b' "$(printf '%s' "$1" | sed 's/%/\\x/g')"; }

if ! command -v pg_dump >/dev/null 2>&1; then
  journal "ECHEC  pg_dump introuvable : installer les outils client de PostgreSQL"
  exit 1
fi

for fichier_env in "$@"; do
  nom=$fichier_env
  if [ ! -f "$fichier_env" ]; then journal "ECHEC  $nom  fichier introuvable"; echecs=$((echecs + 1)); continue; fi

  url=$(lire_env "$fichier_env" DATABASE_URL)
  if ! printf '%s' "$url" | grep -Eq '^postgres(ql)?://([^:/@]+(:[^@]*)?@)?[^/?#]+/[^?#]+'; then
    journal "ECHEC  $nom  DATABASE_URL absent ou sans la forme postgres://utilisateur:motdepasse@hote:port/base"
    echecs=$((echecs + 1)); continue
  fi
  base=$(printf '%s' "$url" | sed -E 's#^postgres(ql)?://([^/]*@)?[^/]+/([^?\#]+).*$#\3#')
  mot_de_passe=$(printf '%s' "$url" | sed -nE 's#^postgres(ql)?://[^:/@]+:([^@]*)@.*$#\2#p')
  url_sans_mot_de_passe=$(printf '%s' "$url" | sed -E 's#^(postgres(ql)?://[^:/@]+):[^@]*@#\1@#')
  nom=$base

  dossier="$destination/$base"
  mkdir -p "$dossier"
  dump="$dossier/base-$horodatage.dump"

  if [ -n "$mot_de_passe" ]; then export PGPASSWORD; PGPASSWORD=$(decoder "$mot_de_passe"); fi
  erreur=$(pg_dump --dbname="$url_sans_mot_de_passe" --format=custom --no-owner --file="$dump" 2>&1)
  code=$?
  unset PGPASSWORD
  if [ $code -ne 0 ]; then
    rm -f "$dump"
    journal "ECHEC  $nom  pg_dump a rendu le code $code : $(printf '%s' "$erreur" | tr '\n' ' ')"
    echecs=$((echecs + 1)); continue
  fi
  taille=$(( $(wc -c < "$dump") / 1024 ))

  documents=$(lire_env "$fichier_env" DOCUMENTS_DIR)
  [ -z "$documents" ] && documents='./documents'
  case "$documents" in /*|[A-Za-z]:*) ;; *) documents="$(dirname "$fichier_env")/$documents" ;; esac
  note_documents='aucun dossier de documents'
  if [ -d "$documents" ]; then
    mkdir -p "$dossier/documents"
    avant=$(find "$dossier/documents" -type f | wc -l)
    if ! cp -Rn "$documents/." "$dossier/documents/"; then
      journal "ECHEC  $nom  la copie des documents a échoué ($documents)"
      echecs=$((echecs + 1)); continue
    fi
    apres=$(find "$dossier/documents" -type f | wc -l)
    note_documents="documents +$((apres - avant)) ($apres au total)"
  fi

  # Les trois plus récents sont toujours gardés ; au-delà, ceux de plus de $conserver jours partent.
  supprimes=0
  while IFS= read -r ancien; do
    [ -n "$ancien" ] || continue
    if [ -n "$(find "$ancien" -mtime +"$conserver" 2>/dev/null)" ]; then rm -f "$ancien"; supprimes=$((supprimes + 1)); fi
  done <<EOF
$(ls -t "$dossier"/base-*.dump 2>/dev/null | tail -n +4)
EOF

  journal "OK     $nom  base $taille Ko ($(basename "$dump")), $note_documents, $supprimes ancien(s) dump(s) supprimé(s)"
done

[ $echecs -eq 0 ]
