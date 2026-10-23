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

Les vocabulaires fermés (statuts, types de carburant, d'intervention, d'échéance, de dépense) sont dans
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
avec la raison. C'est un confort d'écran : la règle reste dans `metier`. Un conducteur s'y lit
« Prénom Nom, Service » (le nom seul s'il n'a pas de service, et dans le profil particulier, qui
n'a pas de service) : deux personnes de même nom se distinguent par leur service. La liste des
affectations et la fiche d'un véhicule (conducteurs actuels, historique) l'écrivent de même, à la
suite du nom (`Affectation::conducteur_service_apres_le_nom`). La page Alertes garde le nom seul :
chaque ligne y mène à la fiche, et la date la distingue déjà.

Le pendant de ces règles est la **clôture automatique**. Enregistrer un véhicule au statut
« Réformé » (`metier/vehicules.rs::modifier`) ou un conducteur au statut « Inactif »
(`metier/conducteurs.rs::modifier`) met fin, à la date du jour, à ses affectations encore
ouvertes : celles sans date de fin et celles dont la fin prévue est postérieure. La mise à jour de
la fiche et la clôture sont dans une même transaction. L'expiration d'un permis, elle, ne
déclenche rien : elle arrive avec le temps, sans écriture.

Les échéances ont une règle du même genre, plus simple : un véhicule n'a pas deux échéances
**ouvertes** de même type, de même date limite et de même chose à faire (pour un « à faire », l'opération et son libellé). Elle s'écrit tout entière en base, par l'index
unique partiel `echeances_pas_de_doublon_ouverte` : l'égalité s'exprime en SQL, il n'y a donc pas de
vérification applicative à doubler. `metier/echeances.rs::traduire` rend l'erreur `23505` en
message lisible, pour la création, la modification et la réouverture d'une échéance traitée.
Une échéance traitée n'entre pas dans la règle. Le test `tests/echeances.rs` la prouve.

Les conducteurs, eux, ne sont pas refusés pour un homonyme : deux personnes peuvent réellement
porter le même nom. **Créer** un conducteur de même nom et de même prénom qu'un autre est donc
*suspendu pour avertir* par le formulaire (`web/conducteurs.rs::creer`), pas par la couche métier.
La comparaison (`metier/conducteurs.rs::homonymes_parmi`) ignore la casse, les accents, les
blancs, le trait d'union (« Jean-Pierre » vaut « Jean Pierre ») et l'apostrophe typographique ;
elle avertit plutôt trop que pas assez, puisque l'avertissement ne bloque pas et qu'un homonyme
manqué coûte une fiche en double. Elle compte les conducteurs inactifs, avec l'invitation à
réactiver la fiche existante, et ne regarde pas le service (l'avertissement l'affiche). Le
formulaire revient avec l'avertissement et un bouton « Créer quand même », qui envoie le même
formulaire vers `/conducteurs?creer_quand_meme=1` (attribut HTML `formaction`) : la confirmation
voyage dans l'adresse, comme le mot-clé `retour` des échéances. Seule la **création** avertit : la
modification, l'API JSON et `metier::conducteurs::creer` ne changent pas. Aucun index n'est posé :
un refus par la base obligerait une migration à supprimer des fiches en double, et supprimer un
conducteur emporte ses affectations et ses documents. Deux requêtes simultanées (un double clic)
ne sont donc pas arrêtées. Le test `tests/conducteurs.rs` prouve chaque cas.

## 4. Flux d'une lecture : la vue alertes

`GET /alertes` → `metier/alertes.rs::calculer` demande à `donnees/alertes.rs` les échéances
ouvertes dont la date limite est antérieure à « aujourd'hui + horizon » et les conducteurs actifs
dont le permis expire dans le même délai, puis classe chaque ligne avec la fonction pure
`niveau(date_limite, aujourd_hui)` : **dépassée** si la date est passée, **à venir** sinon. L'horizon
(30 jours par défaut) vient de la configuration.

La même fonction décide du statut qu'affichent la liste des échéances et la fiche d'un véhicule
(`metier/echeances.rs::etat`) : « Dépassée », « À venir » (dans l'horizon des alertes, `jusqu_au`),
« À traiter » ou « Traitée ». Une échéance ne peut
donc pas être dépassée sur un écran et à traiter sur un autre. Cet état n'est pas stocké, il se
calcule à l'affichage, comme celui d'une affectation. Un permis expiré se signale de même, par la
mention « Expiré » ou « À venir » à côté de sa date (`Conducteur::permis_expire`,
`alertes::permis_a_venir`). « Expiré » est un fait, écrit pour tous les conducteurs ; « À venir »
est un avertissement, réservé aux conducteurs actifs, comme la page Alertes.

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
| Au-dessus de 700 px, chaque tableau est dans un cadre qui défile seul (`.tableau-cadre`), et sa colonne d'actions reste collée au bord droit (décidé le 15/10/2026, #68) | Sur un téléphone en paysage, surtout avec un texte agrandi (115 à 130 % : 800 à 976 px de tableau pour 750 à 850 px d'écran), « Véhicules » et « Entretien » sont plus larges que la page. Sans cadre, la page entière défile. Avec un cadre seul, « Supprimer » est hors de l'écran et il faut deviner qu'on peut faire glisser le tableau : le premier essai sur téléphone l'a montré. La colonne d'actions collée règle les deux ; les autres colonnes défilent. Sur écran tactile, la barre de défilement du cadre est masquée : on ne peut pas la saisir, elle ne bougeait pas quand on la touchait, et le tableau défile au doigt sans elle ; à la souris elle reste. Le cadre ne change rien quand le tableau tient. | Remonter le seuil des fiches à 900 px : un téléphone en paysage garderait les fiches, mais la règle des 700 px est écrite dans le catalogue et la documentation, et une fenêtre d'ordinateur de 800 px aurait des fiches. Un cadre défilant seul : les actions restent hors de l'écran, comme le disait déjà la ligne précédente. |
| Entre 700 et 1 280 px, les cellules d'un tableau ont une marge de 8 px au lieu de 16, et les boutons de la colonne des actions passent à la ligne (décidé le 15/10/2026, #78 ; seuil porté de 1 100 à 1 280 px pour #76) | La colonne des actions est collée à droite (#68) : quand le tableau dépasse le cadre, même de quelques pixels, elle recouvre la fin de la colonne d'avant. Sur la page Affectations d'un téléphone en paysage à 130 % de texte, la pastille d'état se lisait « En co » (mesuré : 46 px recouverts à 892 px). Mesuré avec les deux retouches, sur 892 px : plus aucun recouvrement jusqu'à 130 % de texte, et 2 px à 150 % (100 mesures, de 740 à 1 024 px et de 100 à 150 % de texte : 45 avaient un recouvrement, 16 le gardent, presque toutes à 740 ou 1 024 px et à 130 % de texte ou plus). Au-delà de 1 280 px rien ne change, et les colonnes ne prennent la largeur de leurs boutons que quand elles la trouvent. Le seuil est passé de 1 100 à 1 280 px quand « Véhicules » a reçu sa colonne « Conducteur » (#76) : à 1 100 px la liste dépassait son cadre de 148 px, de 20 px après (90 mesures, de 1 024 à 1 366 px et de 100 à 130 % de texte : 28 avec un dépassement ou un recouvrement avant, 17 après, tous à 1 024 px ou à 115 % de texte et plus, sauf « Véhicules » à 1 100 px et 100 %). | Des fiches à la place du tableau quand il ne tient pas (requête de conteneur) : plus sûr, mais en paysage une fiche par ligne fait défiler une page déjà basse de 412 px, et le changement est plus grand que le défaut. Retirer la colonne collée : « Supprimer » retournerait hors de l'écran (#68). |
| L'immatriculation s'affiche en plaque française dessinée (`.plaque`) partout où elle est une clé de ligne : listes des véhicules, des échéances, de l'entretien et des affectations, fiches du véhicule et du conducteur, accueil (décidé le 15/10/2026) | Une plaque se reconnaît avant de se lire : dans une liste de véhicules, le regard trouve la ligne cherchée sans lire. Le décor (bandes bleues, cercle d'étoiles en masque, « F ») est en CSS et en jetons de la charte : aucune image, aucune requête de plus, et le texte reste l'immatriculation pour les lecteurs d'écran (le « F » a un texte de remplacement vide). Le département et la région ne figurent pas : on ne les connaît pas. | Un fichier image par plaque : une requête de plus et une couleur écrite hors des jetons. Le département et la région devinés d'après l'immatriculation : un format de plaque ancien (« 123 ABC 45 ») n'a pas de lettres de série. |
| Chaque véhicule peut avoir une **couleur de repère** (`vehicules.couleur`, vingt couleurs ou aucune : le gris, le rouge, le vert et le bleu ont trois niveaux, clair, normal et foncé, le marron deux, clair et normal, et il y a un beige, chacun étant une couleur à part entière, un seul code par véhicule ; ajoutés le 21/10/2026), choisie dans un nuancier du formulaire. La bande droite de la plaque dessinée la prend partout où la plaque s'affiche, la silhouette (carte d'accueil, fiche) aussi quand il n'y a pas de photo, et la fiche écrit son nom (décidé le 16/10/2026, sur le retour « cinq plaques, aucun véhicule ne ressort ») | Dans une liste de plaques qui se ressemblent toutes, une couleur se reconnaît avant qu'on lise ; le plus souvent c'est celle de la carrosserie, que l'utilisateur connaît par cœur. Une classe `couleur-*` pose une variable CSS `--repere` que la plaque, la silhouette et l'échantillon reprennent : un seul mécanisme, aucune couleur écrite hors des jetons. La couleur n'informe jamais seule : la plaque dit l'immatriculation, la fiche écrit « Rouge ». | Une bande de couleur sur le bord de la carte : la charte l'interdit (« Langage graphique »). Une couleur attribuée automatiquement : elle ne correspondrait à rien que l'utilisateur sache, et changerait au gré des créations. Un surnom libre (« la Clio de Marie ») : utile aussi, mais c'est un autre champ, à décider à part. Un second réglage « niveau » (clair, normal, foncé) valable pour toutes les couleurs : une colonne de plus et trente combinaisons à contrôler, dont des non-sens (« noir clair », « blanc foncé »). |
| La boîte « Supprimer » d'un véhicule nomme le statut « Réformé » avec son chemin (Modifier, champ Statut) et propose un lien « Modifier le statut » (attributs `data-confirmer-alternative` et `-libelle`, lien caché sinon) (décidé le 15/10/2026, #67) | Le geste qui protège l'historique n'était pas trouvé : la boîte conseillait un statut sans dire où il se règle, et on supprimait. Le lien mène au formulaire « Modifier », où « Passer à Réformé met fin aux affectations en cours » est écrit ; la boîte n'agit pas à la place. Un véhicule déjà réformé n'en reçoit pas. Aucun geste de plus sur la fiche, qui a déjà « Modifier » et « Supprimer ». | Un bouton « Réformer » qui réforme en un geste : « Réformé » met fin aux affectations, la personne doit voir le formulaire et ses conséquences. Un bouton « Réformer » de plus sur la fiche : un geste de plus pour un cas rare. |
| Les véhicules réformés sont repliés sous la liste des véhicules, « Réformés (N) » (`<details id="reformes">`), avec une suppression définitive par ligne pour l'administrateur ; la page n'ouvre la section que si l'adresse la vise (`/vehicules#reformes`, par `app.js`) (décidé le 15/10/2026) | Un véhicule réformé n'est plus de travail : il restait, mêlé aux autres, dans la liste que Christophe regarde chaque jour. Le même geste que les affectations terminées (#75) : une section repliée, titre à toucher. La suppression est celle de la fiche (même route, même boîte), avec un message qui dit ce qu'elle emporte : échéances, entretien, affectations, documents (cascade en base, fichiers effacés). L'export CSV et la recherche gardent les réformés. | Les masquer sans moyen de les retrouver : l'historique d'un véhicule réformé doit rester consultable. Un filtre « Afficher les réformés » : un réglage à refaire à chaque visite. |
| Un véhicule réformé par erreur se remet en service d'un clic : « Remettre en service » dans la section « Réformés » de la liste et sur la fiche d'un réformé, sans boîte de confirmation, qui le repasse « En service » (`POST /vehicules/{id}/remettre-en-service`, administrateur ; sans effet sur un véhicule qui n'est pas réformé) ; les affectations que la réforme a terminées ne sont pas rétablies, et le message le dit dans le profil entreprise (décidé le 15/10/2026, #87) | Le seul chemin existant, « Modifier » puis le champ « Statut », n'était écrit nulle part ; or une réforme par erreur est le cas qu'on a envie de défaire tout de suite. Pas de confirmation : le geste se défait avec « Modifier », et la boîte de confirmation de l'application est celle des suppressions (bouton rouge). La condition `statut = 'reforme'` est dans la requête SQL : deux administrateurs qui cliquent ensemble ne s'écrasent pas, et un véhicule en maintenance n'est jamais touché. Les affectations ne sont pas rouvertes : la base ne garde pas lesquelles la réforme a terminées, et rouvrir au hasard des affectations anciennes (conducteur parti, autre véhicule depuis) serait pire que les recréer ; « Affecter » reste à un clic et le dernier conducteur est nommé sur la fiche. Le véhicule revient « En service » : le statut d'avant n'est pas gardé, « Modifier » règle le cas « En maintenance ». | Rouvrir les affectations terminées le jour de la réforme : on ne sait pas distinguer celles de la réforme de celles terminées le matin même (« Terminer aujourd'hui »). Une boîte de confirmation : un geste de plus pour un geste sans danger, et son bouton est rouge. Garder le statut d'avant la réforme : une colonne et une migration pour un cas rare, que « Modifier » traite. |
| Un tableau qui déborde de son cadre se présente en fiches, comme sous 700 px, **sur un écran d'au moins 500 px de haut** (un téléphone en paysage garde son tableau, #83) : `app.js` mesure chaque cadre (`scrollWidth > clientWidth`, tableau en forme de tableau), pose `.en-fiches` sur celui qui déborde, et refait la mesure quand un cadre change de taille (`ResizeObserver`, section repliée qu'on ouvre). Les fiches se rangent sur plusieurs colonnes (`auto-fit`, 20 rem) au-dessus de 700 px (décidé le 15/10/2026, #83) | Le tableau défilait dans son cadre avec la colonne des actions collée au bord (#68, #78, #80), mais à la position de départ cette colonne recouvrait la fin de la colonne d'avant et rien n'invitait à faire glisser : il restait des cas : sur 312 mesures (neuf pages, de 740 à 1 366 px, de 100 à 150 % de texte), 111 tableaux débordaient et 83 voyaient la colonne des actions recouvrir la colonne d'avant, dès 740 px à 100 % de texte, 1 024 px avec la barre latérale, et « Véhicules » à 1 024 px à toute taille de texte. Après : aucun tableau en forme de tableau ne déborde (0 sur 312), les fiches ne sont posées que sur les 111 qui débordaient, et à 1 280 px et plus à 100 % de texte rien n'est touché. Mesurer le tableau est exact quel que soit le contenu (noms longs, texte agrandi) et ne demande aucun seuil par page ; un tableau qui tient n'est pas touché, donc rien ne bouge sur ordinateur. Rangées sur plusieurs colonnes, les fiches n'allongent pas la page d'une tablette. Sans script, le cadre défile comme avant. Les règles « fiche » du bloc `.en-fiches` sont celles de `@media (max-width: 700px)`, comparées par un test. La limite de hauteur vient de l'essai sur le téléphone (15/10/2026) : en paysage (891 × 411 px, texte à 115-130 %), une fiche de huit lignes dépasse l'écran, on n'en voit jamais une entière, et la liste était moins lisible qu'en tableau, qu'il avait validé (#75 à #78) ; une fiche seule laissait aussi sa moitié droite vide (`auto-fill`, corrigé en `auto-fit`). | Des seuils de requête de conteneur en `em`, un par page : sans script, mais calés sur des données d'essai (un nom plus long et le tableau déborde de nouveau). Une fiche par ligne : une liste de quinze véhicules fait six écrans sur une tablette. Passer tous les tableaux en fiches dès 1 280 px : un ordinateur perdrait la lecture en colonnes. |
| Une intervention détaille **ce qui a été fait** dans une table à part, `lignes_intervention` : une ligne par opération d'un vocabulaire fermé (vidange, filtre à huile, bougies…, « autre » avec un libellé), avec la pièce, sa quantité et son prix pour mémoire. Le formulaire les saisit par des cases à cocher et arrive au serveur en paires `nom=valeur` (`Form<Vec<(String, String)>>`), décodées par `SaisieIntervention::depuis_formulaire` ; l'API JSON reçoit `lignes` directement (décidé le 16/10/2026, #94, bloc 1) | Pour qui entretient lui-même, « Révision » dans les notes ne dit ni quoi ni avec quelle pièce. Un vocabulaire fermé permet, au bloc 2, de calculer « la dernière vidange » comme la plus récente intervention qui en contient une ligne : rien n'est stocké qui pourrait dévier, et l'historique ancien se saisit dans n'importe quel ordre. Les cases à cocher sont le geste le plus court sur un téléphone ; un formulaire HTML ne sachant pas envoyer une liste de structures, le décodage se fait à la main, dans une fonction pure testée sans base. | Une colonne « opérations » en texte libre : rien à calculer dessus. Des champs répétés `lignes[0][operation]` décodés par une bibliothèque de plus : une dépendance pour un seul formulaire. Un vocabulaire ouvert : « vidange » et « Vidange moteur » seraient deux opérations. |
| L'en-tête de la dernière colonne d'un tableau n'est collé au bord que s'il est vide (`th:last-child:empty`), comme les cellules d'actions (décidé le 15/10/2026, #83) | La règle visait toute dernière colonne sans `data-label`, et un `<th>` n'en a jamais : dans « Véhicules » (profil entreprise), l'en-tête « Conducteur », dernier, se collait à droite et recouvrait « Kilométrage » et « Statut » sur un écran étroit, alors que ses cellules, elles, restaient en place. Vu sur le tableau d'un téléphone en paysage, texte agrandi. | Retirer le collage de l'en-tête : une colonne d'actions sans son en-tête collé se décale à l'ouverture du défilement. |
| Après « Marquer traitée », la prochaine échéance proposée part de l'ancienne date limite pour l'assurance, et de la plus tardive de l'ancienne date limite et du jour pour le contrôle technique et l'entretien périodique (`metier::echeances::prochaine`, décidé le 16/10/2026, #95) | L'anniversaire d'un contrat d'assurance ne dépend pas du jour où l'on clique. Le contrôle technique suivant, lui, est dû deux ans après la visite : traité avec trois mois de retard, l'ancienne règle proposait une date trois mois trop tôt ; et une échéance ancienne saisie pour mémoire puis traitée proposait une suivante déjà passée, donc « dépassée » aussitôt, à traiter encore, une cascade. La date reste pré-remplie et modifiable. La vraie date de réalisation viendra des interventions (#94, bloc 3), qui solderont l'échéance. | Partir toujours du jour : faux pour l'assurance, et faux pour un contrôle passé à l'heure mais coché quelques jours plus tard. Demander la date de réalisation dans une boîte : un formulaire de plus pour un geste qu'on voulait en un clic ; le formulaire pré-rempli joue déjà ce rôle. |
| Les cartes de l'accueil Autoclair ont une colonne de 20 rem au plus étroit, plafonnée à la largeur de l'écran (`min(20rem, 100%)`) (décidé le 15/10/2026, #71) | Une largeur en `rem` grandit avec le texte : 416 px à 130 %, plus que les 328 px utiles d'un écran de 360 px. La carte élargissait la page et, avec elle, la barre d'onglets du bas, fixée à la largeur de la page. Plafonner la colonne, laisser la ligne d'actions et la saisie du kilométrage rétrécir règle le cas, sans rien changer à texte normal (positions de chaque élément mesurées identiques à six largeurs). | Un seuil en pixels pour passer à une carte par ligne : il ne suit pas le réglage de texte de l'utilisateur. |
| Les deux colonnes du tableau de bord d'Autoclair Pro (`.colonnes`) ont la même colonne de 20 rem au plus étroit, plafonnée à la largeur de la grille (`minmax(min(20rem, 100%), 1fr)`) (décidé le 22/10/2026, #138) | Même cause que #71, oubliée sur cette grille : 20 rem grandit avec le texte (416 px à 130 %), plus que les 358 px utiles d'un écran de 390 px. La section de gauche élargissait la page (+42 px à 390 px et 130 %, +136 px à 360 px et 150 %). Plafonnée, la colonne se replie dans l'écran, et rien ne bouge là où elle ne dépassait pas : la position de chaque section est mesurée identique avant et après, à huit largeurs d'écran, à 100 et 130 % de texte. | Un seuil en pixels pour passer à une colonne : il ne suit pas le réglage de texte de l'utilisateur. Une colonne minimale plus petite en `rem` : elle grandit encore avec le texte. |
| La barre du haut d'Autoclair Pro se replie en deux paliers quand l'écran est étroit ou le texte grand : le bouton « Menu » ne garde que son icône (son libellé reste lu par les lecteurs d'écran) quand il ne tient plus à côté du logo entier, puis le logo rétrécit, sans passer sous la hauteur minimale de la charte (24 px, soit 180 px de large) ; le palier est une requête de conteneur en rem, `@container (max-width: calc(46px + 16.7rem))`, posée sur `.laterale-haut` (décidé le 22/10/2026, #93) | Avant, ni le logo ni le bouton ne pouvaient rétrécir : à 360 px de large la barre sortait de la page dès 115 % de texte (+47 px à 130 %), et de 53 px à 275 px de large, ce que donne le zoom de page de Chrome à 150 % sur 412 px. Le palier suit la taille du texte parce qu'il est en rem ; un seuil en pixels n'aurait pas suivi. Mesuré avant et après sur la grille de #93 plus les largeurs du zoom : la barre tient partout jusqu'à 250 px de large, texte à 100 % (+4 px à 240 px, +38 px à 206 px), et sa position est identique à celle d'avant sur 360, 390 et 412 px à 100 % et sur ordinateur à 100 %. Sur ordinateur à 130 % de texte, le logo ne dépasse plus de la barre latérale de 260 px (il la dépassait de 41 px). | Un seuil en pixels (`@media (max-width: …px)`) : il ne suit pas le réglage de texte de l'utilisateur. Passer à la ligne le bouton sous le logo : la barre sticky doublerait de hauteur pour quelques pixels manquants. Toujours l'icône seule sous 1 024 px : le mot « Menu » change de place à 100 % sur les écrans où il tient. |
| Dans une fiche (un tableau présenté en fiches sur un écran étroit), une **plaque d'immatriculation qui ne tient pas à côté de son intitulé passe dessous, entière** : l'intitulé de sa cellule flotte (`float: left`) au lieu d'être posé à part. Seulement pour les cellules qui contiennent une plaque (`td[data-label]:has(> a > .plaque)`), dans le bloc sous 700 px et dans le bloc « en fiches » (décidé le 22/10/2026, #142) | Une plaque ne se coupe pas (`white-space: nowrap`) et grandit avec le texte, comme la colonne d'intitulé de 8,5 rem. À 360 px de large et 130 % de texte, elle sortait de sa fiche et la page débordait (+16 px ; +71 px à 150 % ; +19 px à 275 px de large, ce que donne le zoom de page de Chrome à 150 % sur 412 px), sur les six listes qui en montrent (véhicules, entretien, dépenses, échéances, affectations, fiche d'un conducteur). Quand elle tient, la géométrie est celle d'avant (intitulé à 12 px, plaque à 8,5 rem) : positions mesurées identiques à 100 % de texte sur 330, 360, 390 et 412 px, et à 768 px en fiches. | Réduire la colonne d'intitulé (un plafond en pourcentage de la fiche) : toutes les fiches changent, et cela ne suffit pas à 150 %. Rétrécir ou tronquer la plaque : l'immatriculation doit se lire entière. Faire passer toutes les valeurs sous leur intitulé : la lecture des fiches changerait même quand tout tient. |
| Une **fiche trop étroite pose chaque intitulé au-dessus de sa valeur** : chaque fiche est un conteneur nommé (`container: fiche / inline-size`) et, quand elle mesure moins de `calc(16.5rem + 12px)` (la colonne d'intitulé de 8,5 rem, la marge de 12 px, et 8 rem pour la valeur), ses intitulés passent au-dessus de leurs valeurs. Dans le bloc sous 700 px et dans le bloc « en fiches » (décidé le 22/10/2026, #73) | La colonne d'intitulé fait 8,5 rem et grandit avec le texte : la place de la valeur fondait d'autant. Une pastille (« En maintenance », 7,9 rem) ou un délai (« · depuis 400 jours ») ne se coupent pas et sortaient de l'écran (la page débordait jusqu'à +50 px à 360 px et 150 % de texte, +30 px à 250 px de large) ; une date ou un mot (« Pneumatiques ») se coupaient en deux. Le seuil est en rem : il suit le texte agrandi et le zoom de page. Mesuré avant/après sur 19 pages (Autoclair Pro et Autoclair), de 206 à 412 px de large et de 100 à 170 % de texte : plus aucune valeur qui ne tient pas dans sa place ; positions identiques à 100 % de texte de 320 à 412 px, à 115 %, à 130 % sur 390 et 412 px, et sur ordinateur. | Réduire la colonne d'intitulé (un plafond en pourcentage, une largeur en `em`) : à 150 % sur 360 px, il ne reste pas 8 rem à côté, même réduite. Laisser le délai et la pastille se couper : les dates et les mots resteraient coupés en deux. Faire flotter l'intitulé comme pour la plaque (#142) : un mot se coupe au lieu de passer dessous (`overflow-wrap: anywhere`), et un délai renvoyé à la ligne passerait sous l'intitulé même à 100 %. |
| **Jusqu'au zoom de page de 200 %** (206 px de large sur un téléphone de 412 px), **aucune page ne défile de côté** (décidé le 23/10/2026, #145 ; au-delà, on ne va pas). L'**icône Pro remplace le logo** quand il passerait sous 24 px de haut : dans la barre du haut, sous 1 024 px, quand le logo de 180 px et le bouton « Menu » ne tiennent plus côte à côte (requête de conteneur `calc(211px + 1.125rem)`), et sur la page de connexion, dont le grand logo suit la largeur de l'écran (sous 178 px). Une **plaque** rétrécit d'après la plus large (`WW-888-WW`, 11,2 em) : `min(0.9375rem, calc((100vw - 66px) / 11.2))`, et `min(1.5rem, calc((100vw - 32px) / 11.2))` dans un titre. Les boutons d'une barre de titre passent à la ligne ; « Relever » passe sous son champ sous 700 px ; le cadre « Ce qui a été fait » suit la largeur du formulaire (`min-inline-size: 0`, colonnes `minmax(min(9rem, 100%), 1fr)`) | Mesuré avant/après sur 32 pages d'Autoclair Pro et 24 d'Autoclair (formulaires et connexion compris), de 412 à 206 px de large : à 206 px, 40 pages défilaient de côté (jusqu'à +107 px, la page de connexion), aucune après. La plaque `WW-888-WW` dépassait dès le zoom de 150 % dans le titre de sa fiche. Positions identiques avant et après à 412 et 330 px (zoom de 100 et 125 %), à 768 et 1 280 px ; à 275 px, seuls changent la plaque d'un titre (24 → 21,7 px de police) et le logo de connexion d'Autoclair Pro. Une plaque rétrécie reste plus grande à l'écran qu'à 100 % : 12,5 px de police à 200 % valent 25 px. La charte dit : « sous 24 px de hauteur, utiliser l'icône Pro seule ». | Aller au-delà de 200 % (250 et 300 %) : logo, titres, onglets et boutons partout à reprendre, pour un usage rare. Laisser le logo passer sous 24 px : contraire à la charte. Une plaque taillée pour chaque immatriculation : impossible sans script ; la plus large sert de mesure. La largeur du contenu comme conteneur (`.contenu`) : une contrainte de taille sur toute la page, pour une seule règle. « Relever » qui passe à la ligne à toute largeur : à 768 px, il tenait au pixel près et passait sous son champ. |
| « Affectations » ne liste que les affectations en cours et à venir ; les terminées sont dans une section repliée dessous (`<details class="repliee">`) (décidé le 15/10/2026, #75) | Une affectation terminée n'agit plus : elle reste utile comme historique, qui est déjà sur la fiche du véhicule et sur celle du conducteur. Mêlée à la liste de travail, elle la noie : le premier essai sur téléphone n'avait plus que trois lignes terminées sous les yeux, et la liste grossit avec le temps. L'export CSV garde toutes les affectations. | Une page « Historique » à part : un écran et un lien de plus pour ce que les fiches disent déjà. Un filtre « en cours » : un réglage à refaire à chaque visite. |
| Un véhicule dit qui l'a : ses conducteurs en cours, à défaut le dernier avec sa date de fin, dans la liste (colonne « Conducteur ») et en tête de fiche, profil entreprise (décidé le 15/10/2026, #76) | « Terminer aujourd'hui » prend effet tout de suite : la ligne « Conducteur actuel » disait « aucun » et le nom n'était plus que dans l'historique, en bas de la fiche. Une affectation à venir n'est pas citée : le conducteur n'a pas encore le véhicule. La liste lit toutes les affectations en une requête (`conduites_par_vehicule`), pas une par véhicule. | Écrire le dernier conducteur comme « conducteur actuel » : c'est faux dès que l'affectation est terminée. |
| « Exporter (CSV) » est masqué sous 700 px et sur un écran tactile de 500 px de haut au plus, c'est-à-dire un téléphone en paysage (`a.bouton-discret.export-csv`) (décidé le 15/10/2026, #77 ; paysage ajouté après l'essai de #83) | Sur un téléphone, le fichier arrive dans les téléchargements et demande un tableur ; le profil entreprise s'utilise surtout sur ordinateur ; le bouton chargeait la barre de titre (#52). Il reste sur ordinateur et tablette (un téléphone en paysage dépasse 700 px et le retrouve). Seul le bouton est masqué : les adresses `.csv` répondent toujours. | Le retirer partout : un gestionnaire sur ordinateur en a besoin. Le masquer sur tout écran tactile (`pointer: coarse`) : une tablette en perdrait l'usage. |
| Sur un téléphone en paysage (écran tactile de 500 px de haut au plus, plus de 700 px de large), la liste « Entretien » perd ses colonnes « Opérations » et « Prestataire » (classe `secondaire` sur les cellules, règle dans `style.css`) (décidé le 16/10/2026, #83) | Mesuré le 16/10/2026 : avec la colonne « Opérations » (#94) et le résultat du contrôle sous le type (#107), le tableau dépassait son cadre dès le texte normal à 740 px et dès 115 % à 892 px, et la colonne des actions, collée à droite, recouvrait « Coût ». Un téléphone en paysage garde son tableau (pas de fiches sous 500 px de haut) : il faut donc qu'il tienne. Les deux colonnes restent sur la fiche du véhicule, en fiches sous 700 px et dans le CSV. | Des fiches aussi en paysage : essayé (PR #89), illisible sur 411 px de haut. Réduire la police du tableau : le texte agrandi est un réglage de l'utilisateur, à respecter. Masquer ces colonnes partout sous 900 px : une tablette en portrait les perdrait alors qu'elle a la place en fiches. |
| La photo de la fiche et de la carte d'accueil d'un véhicule est celle qu'on a choisie (« Mettre en tête », dans le bloc « Photos » de la fiche, à partir de deux photos) ; sans choix, la plus récente, comme avant. Le choix est un marqueur `photo_de_tete` sur le document : au plus un par véhicule (index unique partiel), seulement sur une photo avec sa vignette (contrainte `CHECK`), et il disparaît avec le document qui le porte (décidé le 15/10/2026, #66) | La plus récente n'est pas toujours la bonne : un véhicule a vite une photo de l'avant, de l'arrière, du sinistre. Le marqueur tient l'intégrité par construction : le document n'appartient qu'à un véhicule, supprimer la photo choisie supprime le choix, aucune clé étrangère circulaire. Une photo déposée après le choix ne le reprend pas : sinon choisir n'aurait de sens que jusqu'au dépôt suivant. Le bloc n'apparaît qu'à partir de deux photos : avec une seule, c'est celle de l'en-tête. | Une colonne `photo_id` sur le véhicule : une clé étrangère de plus, circulaire avec `documents.vehicule_id`, et rien n'empêche d'y mettre la photo d'un autre véhicule sans contrainte composite. Un rang par photo (réordonner) : plus que le besoin, qui est une seule photo de tête. |
| Un document se **renomme** (page du document et liste de la fiche, administrateur seulement) : on ne saisit que la racine du nom, l'extension du fichier reste affichée à côté et est remise à l'enregistrement. Le nom est celui de `documents.nom_original` ; le fichier sur disque (`chemin`, un UUID) ne bouge jamais. `/`, `\` et `:` deviennent des tirets, les caractères que Windows interdit disparaissent. Le téléchargement dit un nom accentué en `filename*=UTF-8''…` (décidé le 19/10/2026, sur le retour « j'ai que des chiffres et des lettres ») | Les photos prises au téléphone arrivent sous un nom d'appareil (une suite de chiffres) : impossible de s'y retrouver dans une liste de documents ou dans le dossier Téléchargements. Le nom affiché n'est pas le nom sur disque, donc renommer est une écriture d'une colonne, sans toucher au fichier ni à la vignette. L'extension fixe évite un fichier `.jpg` rebaptisé `.pdf` qui ne s'ouvrirait plus ; une date tapée « 12/03/2025 » reste lisible en « 12-03-2025 » plutôt que d'être amputée de ses barres. Sans `filename*`, les accents d'un nom arrivent abîmés dans certains navigateurs : c'est ce que le renommage allait rendre courant. | Renommer à la volée sur la page, sans écran dédié : un champ éditable en place demande du JavaScript que l'application évite. Proposer un nom d'office au dépôt (type et date) : utile aussi, mais c'est une autre décision, à prendre à part. |
| Ce qu'il y a à remplacer ou à faire se note comme une **échéance d'un type de plus**, « À faire » (`a_faire`), qui porte une opération du vocabulaire de l'entretien (« Autre » avec son libellé pour une pièce absente de la liste) et un constat libre ; elle n'a pas de suivante (`prochaine` rend `None`) ; son libellé « À faire : Câble d'embrayage » n'est écrit qu'à un endroit (`donnees::echeances::libelle_du_type`) et sert partout (décidé le 19/10/2026, #98, sur le retour « je ne peux marquer que ce qui a été fait ») | Une échéance a déjà tout ce qu'il faut : fiche, liste, alertes, carte d'accueil, impression, API, export, « Marquer traitée ». Un objet à part aurait refait les alertes et l'accueil une seconde fois pour la même chose. Le type s'appelle « À faire » et non « Remplacement » (nom prévu à l'ouverture de #98) parce que la demande était « à remplacer **ou à faire** » : « Remplacement : Vidange » aurait sonné faux, « À faire : Vidange » et « À faire : Câble d'embrayage » se lisent bien, et le mot est celui de l'accueil (« une chose à faire »). Les zones du formulaire n'apparaissent que pour ce type ; masquées, elles sont désactivées, donc non envoyées. Un contrôle technique qui reçoit une pièce est refusé plutôt que corrigé en silence ; la base garde les mêmes règles par `CHECK` (`echeances_a_faire_precise`, `echeances_autre_precisee`…). La valeur de l'opération n'a pas de `CHECK` en base : la liste est dans `Operation`, un seul endroit | Un type « Remplacement » limité aux pièces. Un objet « tâche » à part, avec ses écrans. Une table de pièces propre aux échéances. |
| La facture d'une intervention se joint depuis le formulaire d'intervention (photo ou PDF, le même bouton que la photo du véhicule à sa création) et devient un document « Facture » du véhicule, relié à l'intervention par `documents.intervention_id` (`ON DELETE SET NULL`) ; la fiche et la liste « Entretien » y mènent, la page du document nomme l'intervention (décidé le 16/10/2026, #100) | Le geste naturel à l'atelier : noter ce qui a été fait et photographier la facture dans la foulée, sur le téléphone. Un document reste un document : mêmes contrôles (taille, type), même page, même suppression ; le lien vers l'intervention est une colonne facultative, et le document survit à la suppression de l'intervention, qui n'a pas à emporter une pièce comptable. La facture est contrôlée avant d'écrire l'intervention, comme la photo avant le véhicule : un refus rend le formulaire avec sa saisie. | Une colonne `facture_id` sur l'intervention : une seule facture, et une clé circulaire avec `documents.vehicule_id`. Une table de factures à part : des fichiers à servir, protéger et supprimer une seconde fois. |
| Un contrôle technique a un **résultat** (`interventions.resultat_controle` : favorable, défavorable, critique), obligatoire pour lui et pour la **contre-visite**, nouveau type d'intervention ; la date limite de la contre-visite (deux mois) et le contrôle qu'elle suit sont **calculés** à l'affichage, jamais stockés (décidé le 16/10/2026, #107, bloc 1) | L'application savait qu'un contrôle était fait, pas s'il était réussi : rien ne rappelait la contre-visite. Calculer le rattachement (le dernier contrôle défavorable qui précède) suit le principe de #94 : corriger ou supprimer un contrôle corrige la contre-visite sans rien d'autre à faire. Le résultat reste vide sur les contrôles saisis avant : rien n'est inventé sur l'existant. | Une case « contre-visite » sur le contrôle : elle ne dit ni quand ni avec quel résultat. Une clé `controle_id` sur la contre-visite : à maintenir quand on corrige une date, pour une information qui se déduit. |
| Les **dépenses** pour rouler (pleins de carburant, tickets de péage) sont une seule table `depenses` et un seul écran, avec un type d'un vocabulaire fermé (`TypeDepense`) ; seul un plein porte le carburant pris ce jour-là (`CarburantPlein` : SP95, SP95-E10, SP98, E85, gazole, GPL, GNV, électricité, autre ; proposé par le formulaire d'après le carburant du véhicule à la création, obligatoire à l'enregistrement, et c'est lui qui fixe l'unité de la quantité, litres, kWh ou kg ; #128), une quantité, le compteur et « plein complet », et le formulaire ne montre ce détail que pour le type « Carburant », la zone masquée étant désactivée pour que ses champs ne partent pas ; un plein noté avec son compteur relève le kilométrage du véhicule, jamais en arrière ; le kilométrage n'est pas pré-rempli depuis la fiche (décidé le 21/10/2026, #126, bloc 1) | Le geste est le même pour un plein et un péage : une date, un montant, un véhicule ; les séparer en deux écrans doublerait le code et les totaux à venir (bloc 3). Le vocabulaire fermé permet d'ajouter parking ou lavage par une migration sans nouvel écran. Le compteur d'un plein est une vraie mesure : l'application la prend comme elle prend celle d'une intervention. Il n'est pas pré-rempli parce qu'il servira à la consommation : un compteur d'hier passé pour celui du plein la fausserait sans bruit, alors qu'un champ vide dit « je n'ai pas regardé ». La case « plein complet » est cochée d'office : c'est le cas courant, et seul un plein complet fera borne au calcul. | Deux tables et deux écrans « Carburant » et « Péages » : deux fois le même formulaire. Une colonne `unite` sur la dépense : elle se déduit du carburant du plein. Prendre l'unité sur le carburant du véhicule plutôt que sur celui du plein : un hybride rechargeable aurait ses recharges en litres. Un texte libre pour le carburant : « sp 98 » et « SP98 » seraient deux carburants, et rien ne se calculerait dessus. Rendre le kilométrage obligatoire : on bloquerait qui n'a pas regardé le compteur, pour une donnée qui n'a d'effet que sur la consommation. |
| La **consommation, les totaux et le coût au kilomètre des dépenses** se calculent à la lecture (`metier::consommation`, des fonctions pures) et ne se stockent pas : un plein complet avec kilométrage est une **borne**, la consommation d'un intervalle est `(tout ce qui a été mis depuis la borne précédente, plein d'arrivée compris) ÷ kilomètres × 100`, par unité (L, kWh, kg jamais mélangés) ; les pleins se rangent par date puis kilométrage, pas par ordre de saisie ; un intervalle sans kilométrage croissant ou avec une quantité manquante n'a pas de consommation. Le coût au kilomètre prend les dépenses entre le premier et le dernier kilométrage relevé (plein ou intervention) des douze derniers mois, sans le plein du premier relevé, divisées par les kilomètres entre les deux. La liste se filtre par véhicule et par période **après** ce calcul, en mémoire, parce que la consommation d'un plein dépend d'un plein d'avant la période ; l'export CSV et l'API prennent le même filtre et refusent un filtre illisible au lieu de l'ignorer ; le résumé d'un véhicule est le même pour la fiche et pour l'API (décidé le 22/10/2026, #126, bloc 3) | « Calculé, pas stocké » (#94) : corriger un plein, une date ou un kilométrage corrige tout, sans migration ni recalcul à lancer. Les volumes d'un parc (quelques milliers de lignes) rendent la lecture complète et le filtre en mémoire sans coût perceptible ; un filtre en base ferait perdre le plein de départ d'un intervalle. Ne pas mélanger les unités évite une moyenne sans sens sur un hybride rechargeable. Le coût de l'entretien s'y joint dans la ligne suivante (#94, bloc 6), pour n'avoir qu'un seul bloc « Coût ». | Une colonne `consommation` sur la dépense, recalculée à chaque écriture : faux dès qu'un plein ancien est corrigé ou saisi après coup. Une consommation estimée sans plein complet (un plein partiel ne dit pas le niveau du réservoir). Une moyenne des consommations de chaque intervalle plutôt que le total des quantités sur le total des kilomètres : un intervalle court de 50 km pèserait autant qu'un de 800 km. |
| Le **coût d'un véhicule** additionne son entretien, son carburant et ses péages, et se calcule à la lecture comme le reste (`metier::consommation`, #94, bloc 6) : une intervention coûte sa **facture**, ou la somme des prix de ses lignes « ce qui a été fait » si elle est plus grande (le détail ne coûte pas plus que le tout) ; ces prix sont les **pièces**, et le **reste** est la différence. Un seul bloc « Coût » sur la fiche, après les dépenses, même quand le véhicule n'a que des interventions ; le coût au kilomètre y compte l'entretien, sans celui du premier relevé, comme pour les pleins (décidé le 22/10/2026) | Un seul endroit où lire ce que coûte un véhicule, au lieu de deux totaux à additionner de tête. La facture fait foi parce que c'est la somme payée ; les prix des lignes sont « pour mémoire » et facultatifs, donc une facture à 0 € avec des prix saisis (un coût oublié) ne les fait pas disparaître. Aucune migration : tout se déduit des interventions et de leurs lignes, corriger l'une corrige le bloc. | Additionner la facture et les prix des lignes : les pièces seraient comptées deux fois. Une colonne « coût d'entretien de l'année » : fausse dès qu'on corrige une intervention. Un bloc « Entretien » et un bloc « Dépenses » sans total commun : ce que #94 voulait éviter. |
| Un **saut de kilométrage suspect** (plus de `SEUIL_SAUT_KILOMETRAGE`, 20 000 km, au-dessus du compteur du véhicule choisi) demande confirmation avant l'envoi du formulaire d'intervention ou de dépense, dans la boîte de confirmation de la page ; le compteur de l'option du véhicule (`data-kilometrage`) et le seuil du champ (`data-seuil-km`) viennent du serveur, le contrôle est dans `app.js` et le serveur accepte dans tous les cas ; l'aide du formulaire dit que le compteur ne redescend jamais tout seul et où le corriger (décidé le 22/10/2026, #4) | Le compteur suit le kilométrage saisi s'il est plus haut et ne recule pas : une faute de frappe (99 000 pour 13 000) le fausse durablement, sans message. Arrêter la faute avant qu'elle n'entre coûte une question ; la recalculer après coup demanderait l'historique des relevés (#94, bloc 4), qui réglera le fond. Un garde-fou d'écran suffit : sans script, le serveur accepte comme avant, et l'API n'est pas concernée. | Refuser côté serveur : bloquerait une vraie remise à niveau d'un compteur longtemps non relevé, et casserait l'import par l'API. Faire reculer le compteur à la correction : écraserait un kilométrage saisi à la main sur la fiche (#4, piste « recalculer »). |
| Un **formulaire déjà envoyé ne repart pas** : `app.js` ignore un second envoi du même formulaire (`submit` annulé), tant que la page n'a pas changé ; le bouton s'éteint (`envoi-unique`, `aria-busy`) ; le formulaire se libère au retour en arrière du navigateur et, par sécurité, après trente secondes ; une confirmation demandée plus haut (suppression, saut de kilométrage) annule l'envoi avant cette garde, qui ne marque alors rien ; les formulaires de dépôt gardent la leur (`data-envoi-auto`) (décidé le 22/10/2026, #62, piste (a)) | Reproduit le 22/10/2026 sur `main` : avec une réponse lente et un second clic 300 ms après le premier, chaque envoi partait deux fois et créait deux fiches (deux pleins, deux interventions) ; la base ne peut pas l'empêcher, deux fiches identiques étant légitimes (deux homonymes, deux pleins le même jour). Une garde d'écran coûte vingt lignes, ne change rien côté serveur et ne gêne pas « Créer quand même » (`formaction`). | Un jeton à usage unique par affichage du formulaire (piste (b)) : protégerait aussi du rechargement de page, mais ajoute un état serveur et des refus à expliquer. Désactiver le bouton (`disabled`) : un bouton désactivé n'envoie plus son `formaction`. Ne rien faire (piste (c)) : le pire cas est une fiche en double, dont la suppression emporte ses affectations et ses documents. |
| Le **ticket** d'une dépense se joint depuis le formulaire de dépense et devient un document « Ticket de carburant ou de péage » du véhicule, relié par `documents.depense_id` (`ON DELETE SET NULL`), rangé dans un dossier « Dépenses » de la fiche ; la fiche et la liste « Dépenses » y mènent, la page du document nomme le plein ou le péage (décidé le 21/10/2026, #126, bloc 2) | Le même geste que la facture d'une intervention (#100), avec le même code : contrôle du fichier avant d'écrire la dépense, document ordinaire ensuite (même page, même suppression), qui survit à la suppression de la dépense. Le type est fixé (un ticket est un ticket) : rien à choisir sur le téléphone, à la pompe. Son dossier à part évite de mêler les tickets aux factures d'entretien. | Réutiliser `intervention_id` avec un type « ticket » : un ticket n'est pas une intervention. Une colonne `ticket_id` sur la dépense : un seul ticket, et une clé circulaire. |
| Catalogue d'interface avec Storybook, dans `storybook/`, dont les stories recopient le HTML des gabarits | Voir chaque élément d'interface et quelques écrans dans tous leurs états et à toutes les largeurs, sans lancer le serveur ni la base. Storybook ne sait pas exécuter Askama : la feuille de style et le script sont les vrais, le HTML est une copie, à reporter à la main quand un gabarit change. Node ne sert qu'à cet outil : l'application reste sans chaîne de build JavaScript et se compile sans lui (décidé le 12/10/2026). Comme l'application, il n'écoute que sur `127.0.0.1`, et il ne demande rien à son éditeur (statistiques, recherche de mise à jour, page des nouveautés) ; `tests/catalogue.rs` garde ces réglages. | Une page de catalogue rendue par l'application : rien à installer, mais sans l'interface de Storybook. Un Storybook alimenté par le HTML réel des gabarits : fidèle, mais nettement plus long à construire. |
| Livraison par étiquette de version : un exécutable autonome pour Linux et Windows, attaché à une release GitHub, sans déploiement | Les pages, la feuille de style, le script et les migrations sont compilés dans l'exécutable : un seul fichier suffit à l'installer, sans Rust sur la machine. L'hébergement n'est pas choisi : la livraison s'arrête donc à l'archive téléchargeable (voir `exploitation.md`, § 8). | Déploiement automatique sur un serveur : à construire quand l'hébergement sera décidé. Image Docker : le serveur n'écoute que sur `127.0.0.1`, il faudrait d'abord en faire un réglage. |
| Bibliothèques contrôlées par `cargo deny` (règles dans `deny.toml`) | Une faille publiée sur une bibliothèque, ou une licence qui interdirait un usage commercial, fait échouer la CI au lieu de passer inaperçue. Seules les licences permissives sont admises. | Ne rien contrôler : on l'apprend par hasard. |
| Profil réglé par installation (`PROFIL`), pas par compte | Une installation sert un seul parc : celui d'une entreprise, ou celui d'un particulier, qui n'a que faire des affectations et des services. Un même code sert les deux, et chaque installation garde sa base, son dossier de documents et sa clé de session : les données de l'une n'existent pas dans l'autre (décidé le 12/10/2026). | Deux applications : chaque correction serait à faire deux fois. Un site unique où chaque compte a son espace : une colonne de plus sur chaque table et un filtre dans chaque requête, où un oubli montre les données d'un client à un autre. |
| Charte Autoclair appliquée par ses jetons, les classes de l'application conservées (choix du 14/10/2026, à valider) | Les valeurs (couleurs, espacements, polices) viennent de `design-system/autoclair/code/tokens.css`, servi tel quel : la feuille de style n'en recopie aucune, et `tests/contrastes.rs` mesure ses couples de couleurs. Chaque profil prend sa marque (Autoclair pour un particulier, Autoclair Pro pour une entreprise). Polices et logos sont embarqués dans l'exécutable : aucun appel à un tiers. | Adopter les classes `ac-*` de la charte : fidèle au paquet, mais les dix-neuf gabarits, leurs tests et les stories du catalogue à réécrire pour un rendu identique. |
| Cookie de session chiffré, sans table de sessions | Zéro écriture en base par requête ; suffisant pour quelques utilisateurs. Limite : on ne peut pas révoquer une session à distance avant son expiration (24 h). | Table `sessions` : révocable, plus de code. |

## 7. Sécurité de base

- Requêtes SQL paramétrées uniquement (§ 2).
- Validation de toutes les saisies dans `metier` ; les formulaires sont reçus comme chaînes et
  convertis explicitement.
- Secrets uniquement par variables d'environnement (`.env` non versionné, `.env.example`
  documenté).
- En-têtes HTTP de sécurité posés par un intergiciel (`web/mod.rs`) sur toutes les réponses :
  `Content-Security-Policy: default-src 'self'; img-src 'self' data:; frame-ancestors 'none';
  form-action 'self'`, `X-Content-Type-Options: nosniff`, `X-Frame-Options: DENY`,
  `Referrer-Policy: same-origin`. Les images `data:` ne servent qu'aux icônes de statut, dessinées
  en CSS ; polices, logos, icônes d'application et manifest sont servis par l'application
  elle-même (jamais par un tiers).
- Taille des fichiers déposés limitée (10 Mo). Le contrôle est fait par le serveur ; le navigateur
  le fait aussi dès que le fichier est choisi, pour répondre sans attendre l'envoi. Le nom de
  fichier stocké est un UUID, jamais le nom d'origine (qui ne sert qu'à l'affichage et au
  téléchargement, après nettoyage).
- Les documents ne sont servis que par une route authentifiée, jamais par un dossier statique.
  Seuls une image ou un PDF s'affichent dans le navigateur (`/documents/{id}/voir`) ; tout autre
  fichier part en pièce jointe, pour qu'un fichier HTML déposé ne s'exécute jamais dans le
  contexte du site. Le type d'un fichier est celui que le navigateur a déclaré au dépôt :
  `nosniff` interdit de l'interpréter autrement. La page d'un document (`/documents/{id}`)
  encadre le fichier : une image par `<img>`, un PDF par un lien vers un autre onglet, car
  `frame-ancestors 'none'` interdit d'inclure une réponse du site dans un cadre, même depuis le
  site lui-même ; cette règle n'est pas assouplie pour si peu.
- Le message qui confirme une action (« Échéance marquée traitée. ») voyage dans un cookie
  `fait` posé par la redirection (`src/web/fait.rs`) : un code d'un vocabulaire fermé, jamais du
  texte libre ; la page qui l'affiche l'efface dans sa réponse (intergiciel de `web/mod.rs`).
- La photo du véhicule est le seul fichier que le serveur **lit** : la bibliothèque `image`
  (décodeurs JPEG, PNG et WebP seulement) en tire une vignette JPEG de 640 px au plus, dans un fil
  bloquant, avant d'écrire quoi que ce soit ; une image illisible est refusée comme un fichier
  trop gros. La vignette (`/documents/{id}/vignette`) se garde un jour en cache privé : un nouveau
  dépôt fait un nouvel identifiant. Le dépôt se fait en deux temps (`metier::documents::preparer`
  contrôle et calcule la vignette, `ecrire` pose le fichier et la ligne) pour que la création d'un
  véhicule avec sa photo contrôle la photo avant que le véhicule n'existe : un refus ne laisse
  rien. Le formulaire « Nouveau véhicule » part donc en `multipart/form-data` ; la route accepte
  encore l'encodage classique, sans photo.
- Parmi plusieurs photos, celle de la fiche et de la carte d'accueil est la choisie
  (`documents.photo_de_tete`, geste `POST /documents/{id}/mettre-en-tete`, administrateur), à
  défaut la plus récente : une seule requête les ordonne (`photo_de_tete DESC, cree_le DESC`).

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
templates/         pages HTML Askama (base.html, macros.html, accueil/ par profil, un dossier par
                   entité)
static/            feuille de style, script (confirmations, taille et envoi des fichiers à déposer,
                   intitulés des tableaux sur écran étroit) et polices WOFF2 (polices/)
migrations/        fichiers SQL horodatés, appliqués dans l'ordre
tests/             tests d'intégration (base PostgreSQL réelle, une base jetable par test) ;
                   catalogue.rs, sans base, garde les réglages de confidentialité de Storybook
storybook/         catalogue d'interface (Storybook) : configuration dans .storybook/, stories
                   dans stories/ (primitives, mise en page, écrans), dépendances Node à part
docs/              ce dossier
design-system/     charte Autoclair (autoclair/ : jetons, logos SVG et PNG, textes) et CHARTE.md : comment
                   l'application l'applique, les écarts, les contrastes contrôlés
.github/workflows/ intégration continue (ci.yml) et livraison d'une version (livraison.yml)
deny.toml          règles de `cargo deny` : failles connues, licences et provenance des
                   bibliothèques
```
