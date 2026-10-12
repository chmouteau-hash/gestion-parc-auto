# Architecture

Ce document explique comment l'application est construite et pourquoi. Il est écrit pour une
personne qui apprend Rust et PostgreSQL : chaque terme technique est défini à sa première
apparition.

## 1. Vue d'ensemble

L'application est un **serveur web monolithique** : un seul programme (`gestion-parc-auto`) reçoit
les requêtes HTTP du navigateur, interroge PostgreSQL, et renvoie des pages HTML déjà construites.
Il n'y a pas d'application JavaScript séparée : le navigateur affiche des pages classiques et envoie
des formulaires.

```
 Navigateur du gestionnaire
        |  HTTP (pages HTML, formulaires)
        v
 +-----------------------------------------------------+
 |  gestion-parc-auto (Rust / Axum)                     |
 |                                                     |
 |   web/      routes HTTP, session, rendu des pages   |
 |     |                                               |
 |     v                                               |
 |   metier/   regles de gestion, validation           |
 |     |                                               |
 |     v                                               |
 |   donnees/  requetes SQL parametrees (SQLx)         |
 +-----+-----------------------------------------------+
       |  protocole PostgreSQL
       v
 PostgreSQL 16+        Disque : DOCUMENTS_DIR/ (fichiers deposes)
```

La dépendance va **toujours dans le même sens** : `web → metier → donnees`. La couche `web` ne
contient aucune requête SQL ; la couche `donnees` ne sait rien du HTTP ; la couche `metier` ne
connaît ni l'un ni l'autre, sauf par les types qu'elle reçoit et renvoie.

Un quatrième module, `commun/`, ne dépend de rien et est utilisé par les trois : il contient les
vocabulaires fermés (statuts, types, avec code stocké et libellé affiché) et la mise en forme à la
française (dates, montants). Sans lui, `donnees` devrait importer `metier` pour écrire un code de
statut, ce qui créerait une dépendance circulaire.

## 2. Les trois couches

### `src/web/` : routes HTTP

Une **route** associe une adresse (`GET /vehicules/12`) à une fonction qui construit la réponse.
Cette couche :

- lit la requête (chemin, formulaire, fichier envoyé). L'identifiant d'une adresse comme
  `/vehicules/12` passe par l'extracteur `Identifiant` (`chemin.rs`) : une adresse mal formée
  (`/vehicules/abc`) donne la page « Introuvable », comme un identifiant qui n'existe pas ;
- vérifie qui est connecté (voir § 5) ;
- appelle la couche métier ;
- rend un **template** HTML (fichier `templates/*.html` compilé avec le programme par Askama)
  ou redirige vers une autre page.

Elle ne décide jamais d'une règle de gestion. Une variante JSON des mêmes opérations est exposée
sous `/api/v1/` (`src/web/api.rs`) : mêmes règles, même session, mais la réponse est du JSON plutôt
que du HTML. Elle sert à automatiser (scripts, import) sans dupliquer la logique.

### `src/metier/` : logique métier

Ici vivent les règles du parc automobile, indépendantes du web et de la base :

- validation des saisies (immatriculation obligatoire, kilométrage positif, date valide) ;
- règles d'affectation : pas de véhicule réformé, pas de conducteur inactif ou au permis expiré,
  pas deux périodes qui se chevauchent pour le même conducteur sur le même véhicule
  (`metier/affectations.rs`) ;
- classement des échéances en « dépassée » / « à venir » (`metier/alertes.rs`) ;
- hachage et vérification des mots de passe (`metier/authentification.rs`) ;
- orchestration d'une opération complète : valider, vérifier une règle en base, écrire, traduire
  les refus de PostgreSQL en messages lisibles.

Les vocabulaires fermés (statuts, types de carburant, d'intervention, d'échéance) sont dans
`commun/types.rs`, partagés avec les autres couches.

Les fonctions purement calculatoires (sans base) sont testées par des **tests unitaires** dans le
même fichier (`#[cfg(test)]`). Les fonctions qui orchestrent une opération complète (valider, puis
vérifier une règle en base, puis écrire) reçoivent la connexion en paramètre et sont testées par les
**tests d'intégration** du dossier `tests/`.

### `src/donnees/` : accès aux données

Chaque fichier correspond à une table et ne contient que des requêtes SQL **paramétrées** : les
valeurs sont transmises à PostgreSQL séparément du texte de la requête (`$1`, `$2`…), ce qui rend
l'injection SQL impossible par construction. Aucune requête n'est construite par concaténation de
chaînes.

Les fonctions renvoient des structures Rust plates (une ligne = une `struct`) et laissent remonter
les erreurs SQL à la couche métier, qui les traduit (par exemple une violation d'unicité devient
« cette immatriculation existe déjà »).

## 3. Flux d'une écriture : créer une affectation

1. Le gestionnaire soumet le formulaire `POST /affectations`.
2. `web/affectations.rs` extrait l'utilisateur de session et exige le rôle administrateur
   (sinon 403). Il désérialise le formulaire en `SaisieAffectation` (que des chaînes de
   caractères : la validation est faite en métier, pas par le format du formulaire).
3. `metier/affectations.rs::creer` valide les champs, puis lit le véhicule et le conducteur dans
   `donnees` et applique trois règles, chacune écrite comme une fonction pure :
   - le véhicule n'est pas réformé ;
   - le conducteur est actif et son permis n'est pas expiré ;
   - la période ne chevauche aucune autre affectation de ce conducteur sur ce véhicule.

   La première règle non respectée renvoie une erreur métier lisible.
4. Sinon il demande à `donnees/affectations.rs::inserer` d'écrire la ligne. Si deux requêtes
   identiques arrivent exactement en même temps, l'index unique partiel de la base refuse la
   seconde : le métier traduit l'erreur `23505` en erreur lisible.
5. `web` renvoie une redirection vers la liste (succès) ou ré-affiche le formulaire avec le message
   (erreur métier).

Un véhicule peut avoir plusieurs conducteurs en même temps, et un conducteur plusieurs véhicules :
aucune règle ne limite le nombre d'affectations en cours.

Une affectation est **en cours** quand la date du jour tombe dans sa période : à partir de son
jour de début, et jusqu'à la veille de son jour de fin, qui est le jour où elle s'arrête. Avant
elle est « à venir », ensuite « terminée ». Cet état n'est pas stocké : `Affectation::etat` le
calcule à partir de la date du jour, que la couche `web` fournit. Une affectation datée change
donc d'état toute seule, et poser la date du jour comme date de fin (bouton « Terminer
aujourd'hui », réforme, désactivation) prend effet immédiatement.

Le cas le plus courant du chevauchement, deux affectations sans date de fin, est tenu **deux
fois** : en base, par un index unique partiel (garantie absolue, même en cas de concurrence ou
d'écriture directe en SQL), et en application (message compréhensible avant même d'essayer). Les
autres règles dépendent de dates ou de statuts : elles sont vérifiées en application seulement.
Celles sur le véhicule et le conducteur portent sur l'acte d'affecter : `modifier` ne les rejoue
que si le véhicule ou le conducteur change, ou si l'affectation est prolongée au-delà
d'aujourd'hui. Clôturer ou corriger une date reste donc possible quand le permis a expiré
entre-temps. Le test `tests/affectations.rs` prouve chaque règle.

Le formulaire (`web/affectations.rs`) grise les véhicules et conducteurs qui seraient refusés,
avec la raison. C'est un confort d'écran : la règle reste dans `metier`.

Le pendant de ces règles est la **clôture automatique**. Enregistrer un véhicule au statut
« Réformé » (`metier/vehicules.rs::modifier`) ou un conducteur au statut « Inactif »
(`metier/conducteurs.rs::modifier`) met fin, à la date du jour, à ses affectations encore
ouvertes : celles sans date de fin et celles dont la fin prévue est postérieure. La mise à jour de
la fiche et la clôture sont dans une même transaction. L'expiration d'un permis, elle, ne
déclenche rien : elle arrive avec le temps, sans écriture.

## 4. Flux d'une lecture : la vue alertes

`GET /alertes` → `metier/alertes.rs::calculer` demande à `donnees/alertes.rs` les échéances
ouvertes dont la date limite est antérieure à « aujourd'hui + horizon » et les conducteurs actifs
dont le permis expire dans le même délai, puis classe chaque ligne avec la fonction pure
`niveau(date_limite, aujourd_hui)` : **dépassée** si la date est passée, **à venir** sinon. L'horizon
(30 jours par défaut) vient de la configuration.

## 5. Authentification et rôles

- Table `utilisateurs` : identifiant de connexion, mot de passe haché avec **Argon2id** (un
  algorithme conçu pour être lent et coûteux en mémoire, ce qui rend les attaques par force brute
  impraticables), rôle `administrateur` ou `lecteur`.
- La **session** est un cookie chiffré et signé (`axum-extra`, `PrivateCookieJar`) qui contient
  l'identifiant de l'utilisateur. La clé vient de `SESSION_SECRET`. Le cookie est `HttpOnly`
  (invisible au JavaScript) et `SameSite=Lax` (un site tiers ne peut pas le faire envoyer par un
  formulaire caché : c'est la protection contre la falsification de requête inter-sites, CSRF).
- Deux **extracteurs** Axum portent la règle : `UtilisateurConnecte` (connecté, sinon redirection
  vers `/connexion`) et `Administrateur` (rôle administrateur, sinon 403). Toute route qui écrit
  (`POST`) prend un `Administrateur` en paramètre : la règle est visible dans la signature de la
  fonction, et un oubli se voit à la relecture.
- Les comptes se créent en ligne de commande (`cargo run -- creer-utilisateur`), pas depuis le site :
  pas de page d'inscription publique en v1. Un mot de passe oublié se remplace de la même façon
  (`cargo run -- changer-mot-de-passe`) : qui a la main sur la machine du serveur a déjà accès à
  la base, la commande n'ouvre donc aucun droit nouveau.
- Le **profil** de l'installation (`PROFIL` : `entreprise` ou `particulier`) n'est pas un droit :
  il règle ce que les pages montrent. Les deux extracteurs le lisent dans la configuration et le
  posent dans `Contexte` (`web/rendu.rs`), que chaque page reçoit déjà. Un gabarit ne compare
  jamais le profil lui-même : il demande `ctx.montre_affectations()` ou `ctx.montre_service()`,
  et la correspondance entre profil et écrans tient en un seul endroit. Le profil **masque sans
  interdire** : les routes des affectations répondent toujours, et l'API ne change pas.

## 6. Choix techniques et justification

| Choix | Pourquoi | Alternative écartée |
| --- | --- | --- |
| Rust + Axum | Typage fort, erreurs attrapées à la compilation, serveur léger ; objectif d'apprentissage de l'utilisateur. | Node/Express, Django : plus rapides à écrire, mais hors de l'objectif pédagogique. |
| SQLx avec requêtes vérifiées **à l'exécution** (`query_as`) et non à la compilation (`query!`) | Les macros `query!` exigent une base accessible à chaque compilation ou un cache `.sqlx` à régénérer à chaque requête modifiée : une source de friction pour un débutant. Les tests d'intégration jouent chaque requête sur une base réelle, ce qui apporte la même garantie au moment où elle compte. **Décision à valider** (voir README). | `query!` : plus sûr à la compilation, plus lourd à opérer. |
| Pages rendues côté serveur (Askama), pas de HTMX | Askama compile les templates : une variable manquante est une erreur de compilation. HTMX n'est pas nécessaire pour des listes et des formulaires. | React/Vue : chaîne de build JavaScript, hors périmètre v1. |
| Migrations SQLx embarquées et appliquées au démarrage | « L'application démarre en une commande » : `cargo run` applique les migrations manquantes. `sqlx migrate run` reste possible. | Migrations manuelles seulement. |
| Identifiants `BIGINT` auto-incrémentés | Lisibles dans les URL et les journaux, suffisants pour un outil interne. | UUID : utile pour fusionner des bases ou exposer publiquement, inutile ici. |
| Montants en **centimes entiers** | Pas d'arrondi flottant ; conversion à la saisie et à l'affichage. | `NUMERIC` : correct aussi, mais demande une bibliothèque décimale côté Rust. |
| Statuts en `TEXT` + contrainte `CHECK` | Lisible dans `psql`, la liste des valeurs est dans la migration et dans `metier/types.rs`. | Type `ENUM` PostgreSQL : plus rigide à faire évoluer. |
| Documents sur disque, chemin en base | Une base ne doit pas grossir avec des PDF ; la sauvegarde est `pg_dump` + copie du dossier. | Stockage `BYTEA` en base. |
| Photo d'un document par le champ de fichier du navigateur (`<input type="file" capture>`) | Sur un téléphone, ce champ ouvre l'appareil photo, dans tout navigateur et sans code. L'envoi automatique dès la photo prise est un confort ajouté par le script ; sans lui, un bouton « Déposer » reste. | Accès direct à la caméra en JavaScript (`getUserMedia`) : exige HTTPS, une autorisation, et bien plus de code. |
| Sur écran étroit (moins de 700 px), une ligne de tableau devient une fiche | Tout reste visible sans défilement horizontal, y compris les actions de la ligne. La feuille de style fait la mise en page ; le script recopie l'intitulé de chaque colonne sur ses cellules, pour qu'il ne soit écrit qu'une fois, dans l'en-tête. Sans script, les fiches s'affichent sans intitulés. | Un bloc défilant autour du tableau : les actions, en bout de ligne, restent hors de l'écran. |
| Catalogue d'interface avec Storybook, dans `storybook/`, dont les stories recopient le HTML des gabarits | Voir chaque élément d'interface et quelques écrans dans tous leurs états et à toutes les largeurs, sans lancer le serveur ni la base. Storybook ne sait pas exécuter Askama : la feuille de style et le script sont les vrais, le HTML est une copie, à reporter à la main quand un gabarit change. Node ne sert qu'à cet outil : l'application reste sans chaîne de build JavaScript et se compile sans lui (décidé le 12/10/2026). Comme l'application, il n'écoute que sur `127.0.0.1`, et il ne demande rien à son éditeur (statistiques, recherche de mise à jour, page des nouveautés) ; `tests/catalogue.rs` garde ces réglages. | Une page de catalogue rendue par l'application : rien à installer, mais sans l'interface de Storybook. Un Storybook alimenté par le HTML réel des gabarits : fidèle, mais nettement plus long à construire. |
| Livraison par étiquette de version : un exécutable autonome pour Linux et Windows, attaché à une release GitHub, sans déploiement | Les pages, la feuille de style, le script et les migrations sont compilés dans l'exécutable : un seul fichier suffit à l'installer, sans Rust sur la machine. L'hébergement n'est pas choisi : la livraison s'arrête donc à l'archive téléchargeable (voir `exploitation.md`, § 8). | Déploiement automatique sur un serveur : à construire quand l'hébergement sera décidé. Image Docker : le serveur n'écoute que sur `127.0.0.1`, il faudrait d'abord en faire un réglage. |
| Bibliothèques contrôlées par `cargo deny` (règles dans `deny.toml`) | Une faille publiée sur une bibliothèque, ou une licence qui interdirait un usage commercial, fait échouer la CI au lieu de passer inaperçue. Seules les licences permissives sont admises. | Ne rien contrôler : on l'apprend par hasard. |
| Profil réglé par installation (`PROFIL`), pas par compte | Une installation sert un seul parc : celui d'une entreprise, ou celui d'un particulier, qui n'a que faire des affectations et des services. Un même code sert les deux, et chaque installation garde sa base, son dossier de documents et sa clé de session : les données de l'une n'existent pas dans l'autre (décidé le 12/10/2026). | Deux applications : chaque correction serait à faire deux fois. Un site unique où chaque compte a son espace : une colonne de plus sur chaque table et un filtre dans chaque requête, où un oubli montre les données d'un client à un autre. |
| Cookie de session chiffré, sans table de sessions | Zéro écriture en base par requête ; suffisant pour quelques utilisateurs. Limite : on ne peut pas révoquer une session à distance avant son expiration (24 h). | Table `sessions` : révocable, plus de code. |

## 7. Sécurité de base

- Requêtes SQL paramétrées uniquement (§ 2).
- Validation de toutes les saisies dans `metier` ; les formulaires sont reçus comme chaînes et
  convertis explicitement.
- Secrets uniquement par variables d'environnement (`.env` non versionné, `.env.example`
  documenté).
- En-têtes HTTP de sécurité posés par un intergiciel (`web/mod.rs`) sur toutes les réponses :
  `Content-Security-Policy: default-src 'self'`, `X-Content-Type-Options: nosniff`,
  `X-Frame-Options: DENY`, `Referrer-Policy: same-origin`.
- Taille des fichiers déposés limitée (10 Mo). Le contrôle est fait par le serveur ; le navigateur
  le fait aussi dès que le fichier est choisi, pour répondre sans attendre l'envoi. Le nom de
  fichier stocké est un UUID, jamais le nom d'origine (qui ne sert qu'à l'affichage et au
  téléchargement, après nettoyage).
- Les documents ne sont servis que par une route authentifiée, jamais par un dossier statique.
  Seuls une image ou un PDF s'affichent dans le navigateur (`/documents/{id}/voir`) ; tout autre
  fichier part en pièce jointe, pour qu'un fichier HTML déposé ne s'exécute jamais dans le
  contexte du site. Le type d'un fichier est celui que le navigateur a déclaré au dépôt :
  `nosniff` interdit de l'interpréter autrement.

## 8. Structure des dossiers

```
src/
  main.rs          point d'entrée : configuration, migrations, serveur, sous-commandes
  lib.rs           déclare les modules (permet aux tests d'intégration d'utiliser le code)
  config.rs        lecture des variables d'environnement
  erreurs.rs       type d'erreur unique et sa traduction en réponse HTTP
  etat.rs          état partagé du serveur (pool de connexions, configuration, clé de session)
  web/             routes HTTP (une entité par fichier), session, identifiant des adresses
                   (chemin.rs), api.rs (JSON)
  metier/          règles, validation, authentification
  donnees/         requêtes SQL (une table par fichier)
  commun/          vocabulaires (types.rs) et mise en forme (format.rs)
templates/         pages HTML Askama (base.html + un dossier par entité)
static/            feuille de style et script (confirmations, taille et envoi des fichiers à déposer,
                   intitulés des tableaux sur écran étroit)
migrations/        fichiers SQL horodatés, appliqués dans l'ordre
tests/             tests d'intégration (base PostgreSQL réelle, une base jetable par test) ;
                   catalogue.rs, sans base, garde les réglages de confidentialité de Storybook
storybook/         catalogue d'interface (Storybook) : configuration dans .storybook/, stories
                   dans stories/ (primitives, mise en page, écrans), dépendances Node à part
docs/              ce dossier
design-system/     charte graphique (CHARTE.md) et jetons (tokens.md) : ce que la feuille de style
                   définit, rôle par rôle, et les contrastes mesurés
.github/workflows/ intégration continue (ci.yml) et livraison d'une version (livraison.yml)
deny.toml          règles de `cargo deny` : failles connues, licences et provenance des
                   bibliothèques
```
