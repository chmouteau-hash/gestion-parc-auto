// Seul script de l'application. Sans JavaScript, les formulaires partent directement et le
// serveur refait les contrôles : rien d'essentiel ne dépend de ce fichier.

// Demande confirmation avant les suppressions : un formulaire portant l'attribut
// data-confirmer="Question ?" n'est envoyé qu'après accord.
document.querySelectorAll("form[data-confirmer]").forEach(function (formulaire) {
  formulaire.addEventListener("submit", function (evenement) {
    if (!window.confirm(formulaire.dataset.confirmer)) {
      evenement.preventDefault();
    }
  });
});

// Refuse un fichier trop gros dès qu'il est choisi, sans l'envoyer. Le champ porte la limite en
// octets (data-taille-max) et le message (data-trop-gros). Le serveur refait le contrôle, mais
// devant un fichier très au-delà de la limite il cesse de recevoir : le navigateur n'affiche
// alors que sa propre page d'erreur, d'où ce contrôle préalable.
document.querySelectorAll("input[type=file][data-taille-max]").forEach(function (champ) {
  champ.addEventListener("change", function () {
    var fichier = champ.files[0];
    var tropGros = fichier && fichier.size > Number(champ.dataset.tailleMax);
    champ.setCustomValidity(tropGros ? champ.dataset.tropGros : "");
    champ.reportValidity();
  });
});
