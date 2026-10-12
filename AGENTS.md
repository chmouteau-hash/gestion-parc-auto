# Consignes de travail dans ce dépôt

Ce fichier dit comment travailler ici, pour un assistant de code comme pour une personne. Ce que
fait l'application, ses commandes et ses choix techniques sont dans `README.md` et dans `docs/`.

Le dépôt est public : le texte d'une issue, d'une pull request ou d'un commit est lisible par tous.

## Issues GitHub : structure et cases à cocher

Une issue suit l'un des deux modèles de `.github/ISSUE_TEMPLATE/`. Le modèle ne s'applique pas
tout seul en ligne de commande : reprendre sa structure à chaque création ou réécriture d'issue
avec `gh`.

- **Retour d'utilisation** (`retour_utilisateur.md`) : titre `[Retour] Écran : constat`. Le
  contexte dit qui a constaté, quand, sur quelle instance, et à quel commit de `main` le code a été
  relu.
- **Réalisation** (`realisation.md`) : Objectif, Déjà livré, Reste à réaliser, Recette, Hors
  périmètre.

Les cases disent l'avancement :

- Chaque point est une case : `- [x]` pour ce qui est fait, `- [ ]` pour ce qui reste. La syntaxe
  est stricte (`[x]`, sans espace) : sinon GitHub affiche du texte au lieu d'une case.
- Une case « Déjà livré » se coche sur preuve : le point a été lu dans le code ou essayé, jamais
  supposé.
- Une case « Reste à réaliser » se coche quand la PR qui livre le point est fusionnée.
- Une grosse issue se découpe en blocs numérotés, dans l'ordre conseillé.
- Aucune donnée réelle dans une issue : ni nom de conducteur, ni immatriculation, ni document
  déposé, ni capture non expurgée.

Trois étiquettes disent la nature de l'issue. Comme le modèle, elles ne se posent pas toutes seules
en ligne de commande : ajouter `--label` à `gh issue create`.

- `réalisation` : issue au modèle « Réalisation ».
- `retour` : issue au modèle « Retour d'utilisation ».
- `idée` : piste non engagée ; s'ajoute à `retour` quand le retour est une idée.

Les étiquettes par défaut de GitHub (`bug`, `enhancement`, `documentation`, `question`) ne servent
pas : le type d'un retour se dit par la case cochée dans « Type de retour ».

## Issues GitHub : l'étiquette `à-tester`

Une seule étiquette dit l'avancement : `à-tester`. Le travail est fusionné et attend l'essai de
Christophe. Tout le reste de l'avancement se lit dans les cases.

Elle ne se pose pas à chaque fusion : seulement quand l'issue contient vraiment quelque chose à
essayer (décidé le 12/10/2026).

- **Il y a quelque chose à essayer** quand la PR change ce que Christophe voit ou fait dans son
  usage : un écran, un comportement, un message, une commande qu'il lance.
- **Il n'y a rien à essayer** quand elle ne change que ce qu'il n'utilise pas directement : la
  documentation, les tests, le code réorganisé sans effet visible, la CI.
- La description de la PR le dit en clair, sous le titre « À essayer » : soit les points à
  essayer, soit « rien à essayer » et pourquoi. Christophe peut ainsi corriger ce jugement avant
  de fusionner.

Quand il y a quelque chose à essayer :

- La PR ne ferme pas l'issue qu'elle livre. Elle la cite (`Réf. #N`), sans mot-clé de fermeture
  (`Closes #N`, `Fixes #N`), ni dans son texte ni dans un commit : GitHub fermerait l'issue à la
  fusion. « Fermée » veut alors dire « essayée », pas « fusionnée ».
- À la fusion, l'issue reste ouverte et reçoit `à-tester`, sans demander, avec un commentaire qui
  liste ce qui est à essayer.
- L'essai est celui de Christophe, dans son usage. Les essais à l'écran faits avant la PR ne le
  remplacent pas.
- Sur son accord, les cases « Résultat attendu » d'un retour se cochent, l'étiquette se retire, et
  l'issue se ferme s'il ne lui reste rien à réaliser. Si l'essai échoue, l'étiquette se retire et
  un commentaire dit ce qui ne va pas.

Quand il n'y a rien à essayer :

- L'issue ne reçoit pas l'étiquette, et ses cases se cochent à la fusion.
- La PR peut la fermer (`Closes #N`) si elle livre tout ce qui restait. Sinon elle la cite
  (`Réf. #N`) et l'issue reste ouverte.

Les essais à faire se listent avec `gh issue list --label à-tester`, ou avec le filtre
`label:à-tester` sur GitHub.
