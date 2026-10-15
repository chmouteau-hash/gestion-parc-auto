// Seul script de l'application. Sans JavaScript, les formulaires partent directement et le
// serveur refait les contrôles : rien d'essentiel ne dépend de ce fichier.

// Demande confirmation avant les suppressions : un formulaire portant l'attribut
// data-confirmer="Question ?" n'est envoyé qu'après accord. La question s'affiche dans la boîte
// <dialog> de la page (aux couleurs de la charte) ; sans <dialog>, dans celle du navigateur.
// `submit()` n'émet pas l'événement « submit » : l'envoi après accord ne repasse pas ici.
var boite = document.querySelector("[data-confirmation]");
var boiteUtilisable = boite && typeof boite.showModal === "function";
document.querySelectorAll("form[data-confirmer]").forEach(function (formulaire) {
  formulaire.addEventListener("submit", function (evenement) {
    evenement.preventDefault();
    if (!boiteUtilisable) {
      if (window.confirm(formulaire.dataset.confirmer)) {
        formulaire.submit();
      }
      return;
    }
    boite.querySelector("[data-confirmation-texte]").textContent = formulaire.dataset.confirmer;
    boite.querySelector("[data-confirmation-oui]").textContent = formulaire.dataset.confirmerBouton || "Supprimer";
    // Un autre choix, quand la suppression n'est pas ce qu'on veut (data-confirmer-alternative :
    // l'adresse où il mène, data-confirmer-alternative-libelle : ce qu'on y lit). Sans les deux
    // attributs, le lien reste caché.
    var autre = boite.querySelector("[data-confirmation-alternative]");
    if (autre) {
      var adresse = formulaire.dataset.confirmerAlternative;
      autre.hidden = !adresse;
      if (adresse) {
        autre.href = adresse;
        autre.textContent = formulaire.dataset.confirmerAlternativeLibelle || "Autre choix";
      }
    }
    boite.returnValue = "non";
    boite.addEventListener(
      "close",
      function () {
        if (boite.returnValue === "oui") {
          formulaire.submit();
        }
      },
      { once: true },
    );
    boite.showModal();
  });
});

// Barre latérale d'Autoclair Pro, repliée sous 1 024 px : le bouton « Menu » (data-menu) l'ouvre
// et la ferme. Sans ce script, elle reste repliée ; les mêmes pages restent accessibles par le
// tableau de bord et les liens des écrans.
document.querySelectorAll("[data-menu]").forEach(function (bouton) {
  bouton.addEventListener("click", function () {
    var ouvert = document.body.classList.toggle("menu-ouvert");
    bouton.setAttribute("aria-expanded", ouvert ? "true" : "false");
  });
});

// La touche « / » met le curseur dans la recherche (data-raccourci), sauf quand on tape déjà
// dans un champ.
var recherche = document.querySelector("[data-raccourci]");
if (recherche) {
  document.addEventListener("keydown", function (evenement) {
    if (evenement.key !== "/" || evenement.ctrlKey || evenement.metaKey || evenement.altKey) {
      return;
    }
    var cible = evenement.target;
    var tape =
      cible &&
      (cible.tagName === "INPUT" || cible.tagName === "TEXTAREA" || cible.tagName === "SELECT" || cible.isContentEditable);
    if (!tape) {
      evenement.preventDefault();
      recherche.focus();
    }
  });
}

// Le bouton « Imprimer » (data-imprimer) ouvre l'aperçu d'impression du navigateur ; la feuille
// de style déshabille la page pour le papier. Sans ce script, le menu du navigateur fait pareil.
document.querySelectorAll("[data-imprimer]").forEach(function (bouton) {
  bouton.addEventListener("click", function () {
    window.print();
  });
});

// Sur un écran étroit, la feuille de style présente chaque ligne de tableau comme une fiche.
// Chaque cellule reçoit ici l'intitulé de sa colonne (data-label), repris de l'en-tête : il n'est
// donc écrit qu'une fois, dans la page. Une cellule sans contenu est marquée (data-vide) pour ne
// pas laisser de blanc dans la fiche. Sans ce script, les fiches s'affichent sans intitulés.
document.querySelectorAll("table.tableau").forEach(function (tableau) {
  var intitules = Array.prototype.map.call(tableau.querySelectorAll("thead th"), function (th) {
    return th.textContent.trim();
  });
  tableau.querySelectorAll("tbody tr").forEach(function (ligne) {
    Array.prototype.forEach.call(ligne.cells, function (cellule, rang) {
      if (intitules[rang]) {
        cellule.dataset.label = intitules[rang];
      }
      if (!cellule.textContent.trim() && cellule.children.length === 0) {
        cellule.dataset.vide = "";
      }
    });
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

// Le champ de fichier est caché sous un bouton : le nom du fichier choisi s'affiche à côté, dans
// l'élément que désigne data-nom-vers (son texte « vide » est dans data-vide). Sans ce script, le
// bouton ouvre le choix de fichier tout pareil ; le nom n'est simplement pas rappelé.
document.querySelectorAll("input[type=file][data-nom-vers]").forEach(function (champ) {
  var cible = document.getElementById(champ.dataset.nomVers);
  if (!cible) {
    return;
  }
  champ.addEventListener("change", function () {
    cible.textContent = champ.files.length > 0 ? champ.files[0].name : cible.dataset.vide;
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
