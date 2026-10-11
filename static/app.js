// Seul script de l'application : demande confirmation avant les suppressions.
// Un formulaire portant l'attribut data-confirmer="Question ?" n'est envoyé qu'après accord.
// Sans JavaScript, le formulaire part directement : rien d'essentiel ne dépend de ce fichier.
document.querySelectorAll("form[data-confirmer]").forEach(function (formulaire) {
  formulaire.addEventListener("submit", function (evenement) {
    if (!window.confirm(formulaire.dataset.confirmer)) {
      evenement.preventDefault();
    }
  });
});
