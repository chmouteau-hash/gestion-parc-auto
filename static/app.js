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

// Envoie le formulaire de dépôt tout seul (data-envoi-auto) dès qu'une photo est prise ou qu'un
// fichier est choisi, si le reste est rempli. Sinon le navigateur montre le champ qui manque, et
// l'envoi part quand il est renseigné.
document.querySelectorAll("form[data-envoi-auto]").forEach(function (formulaire) {
  var champs = formulaire.querySelectorAll("input[type=file]");
  var etat = formulaire.querySelector("[data-etat-envoi]");
  var texteInitial = etat ? etat.textContent : "";

  // Pendant l'envoi (plusieurs secondes pour une photo en 4G), les boutons sont neutralisés.
  function signalerEnvoi() {
    formulaire.classList.add("envoi-en-cours");
    if (etat) {
      etat.textContent = "Envoi en cours…";
    }
  }

  formulaire.addEventListener("change", function (evenement) {
    var source = evenement.target;
    // Deux champs proposent un fichier (appareil photo, fichier existant) : un seul sert à la fois.
    if (source.type === "file" && source.files.length > 0) {
      champs.forEach(function (autre) {
        if (autre !== source) {
          autre.value = "";
          autre.setCustomValidity("");
        }
      });
    }
    var choisi = Array.prototype.some.call(champs, function (champ) {
      return champ.files.length > 0;
    });
    if (!choisi) {
      return;
    }
    if (!formulaire.checkValidity()) {
      formulaire.reportValidity();
    } else if (formulaire.requestSubmit) {
      formulaire.requestSubmit();
    } else {
      // Navigateur ancien : `submit()` ne déclenche pas l'événement écouté ci-dessous.
      signalerEnvoi();
      formulaire.submit();
    }
  });

  formulaire.addEventListener("submit", signalerEnvoi);

  // Retour en arrière : le navigateur ré-affiche la page telle qu'elle était pendant l'envoi.
  window.addEventListener("pageshow", function (evenement) {
    if (evenement.persisted) {
      formulaire.reset();
      formulaire.classList.remove("envoi-en-cours");
      if (etat) {
        etat.textContent = texteInitial;
      }
    }
  });
});
