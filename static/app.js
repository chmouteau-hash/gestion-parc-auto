// Seul script de l'application. Sans JavaScript, les formulaires partent directement et le
// serveur refait les contrôles : rien d'essentiel ne dépend de ce fichier.

// Demande confirmation avant d'envoyer un formulaire. La question s'affiche dans la boîte
// <dialog> de la page (aux couleurs de la charte) ; sans <dialog>, dans celle du navigateur.
// `submit()` n'émet pas l'événement « submit » : l'envoi après accord ne repasse pas ici.
// `choix` : { question, bouton, danger, alternative, alternativeLibelle } ; `danger` (vrai par
// défaut) colore le bouton d'accord comme une suppression, faux pour un simple « quand même » ;
// l'alternative est un autre choix que l'envoi (data-confirmer-alternative : l'adresse où il
// mène, …-libelle : ce qu'on y lit) ; sans les deux, le lien reste caché.
var boite = document.querySelector("[data-confirmation]");
var boiteUtilisable = boite && typeof boite.showModal === "function";
function confirmerPuisEnvoyer(formulaire, choix) {
  if (!boiteUtilisable) {
    if (window.confirm(choix.question)) {
      formulaire.submit();
    }
    return;
  }
  boite.querySelector("[data-confirmation-texte]").textContent = choix.question;
  var oui = boite.querySelector("[data-confirmation-oui]");
  oui.textContent = choix.bouton || "Supprimer";
  oui.classList.toggle("danger", choix.danger !== false);
  var autre = boite.querySelector("[data-confirmation-alternative]");
  if (autre) {
    var adresse = choix.alternative;
    autre.hidden = !adresse;
    if (adresse) {
      autre.href = adresse;
      autre.textContent = choix.alternativeLibelle || "Autre choix";
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
}

// Les suppressions : un formulaire portant data-confirmer="Question ?" n'est envoyé qu'après
// accord.
document.querySelectorAll("form[data-confirmer]").forEach(function (formulaire) {
  formulaire.addEventListener("submit", function (evenement) {
    evenement.preventDefault();
    confirmerPuisEnvoyer(formulaire, {
      question: formulaire.dataset.confirmer,
      bouton: formulaire.dataset.confirmerBouton,
      alternative: formulaire.dataset.confirmerAlternative,
      alternativeLibelle: formulaire.dataset.confirmerAlternativeLibelle,
    });
  });
});

// Un saut de kilométrage suspect (#4) : le compteur du véhicule suit celui d'une intervention ou
// d'un plein s'il est plus haut, et ne redescend jamais tout seul. Une faute de frappe
// (99 000 pour 13 000) le fausserait durablement. Quand le kilométrage saisi dépasse le compteur
// du véhicule choisi (data-kilometrage sur son option) de plus du seuil que porte le champ
// (data-seuil-km, fixé par le serveur), le formulaire demande confirmation avant de partir.
// Sans ce script, le formulaire part directement : le serveur accepte, comme avant.
document.querySelectorAll("form input[name=kilometrage][data-seuil-km]").forEach(function (champ) {
  var formulaire = champ.form;
  var vehicule = formulaire && formulaire.querySelector("select[name=vehicule_id]");
  if (!vehicule) {
    return;
  }
  var enKm = function (valeur) {
    return Number(valeur).toLocaleString("fr-FR") + "\u202fkm";
  };
  formulaire.addEventListener("submit", function (evenement) {
    var option = vehicule.selectedOptions[0];
    var compteur = option && Number(option.dataset.kilometrage);
    var saisi = Number(String(champ.value).replace(/[^0-9]/g, ""));
    var seuil = Number(champ.dataset.seuilKm);
    if (champ.disabled || !champ.value || isNaN(compteur) || !(saisi > compteur + seuil)) {
      return;
    }
    evenement.preventDefault();
    confirmerPuisEnvoyer(formulaire, {
      question:
        "Le kilométrage saisi (" + enKm(saisi) + ") dépasse de " + enKm(saisi - compteur) +
        " le compteur du véhicule (" + enKm(compteur) + "). Le compteur sera relevé à " +
        enKm(saisi) + " et ne redescendra pas tout seul. Enregistrer quand même ?",
      bouton: "Enregistrer",
      danger: false,
    });
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

// Un tableau plus large que son cadre (téléphone en paysage ou tablette avec le texte agrandi,
// barre latérale ouverte) se présente en fiches, comme sur un téléphone en portrait (#83) : le
// cadre reçoit la classe `en-fiches`, que la feuille de style habille. Un tableau qui tient reste
// un tableau (et un écran de moins de 500 px de haut, un téléphone en paysage, garde le sien : la
// feuille de style n'habille `.en-fiches` que sur un écran assez haut). La mesure se fait tableau en
// forme de tableau (classe retirée, puis reposée) et se
// refait quand un cadre change de taille : rotation, fenêtre, section repliée qu'on ouvre. Sans ce
// script, le cadre défile, comme avant.
var cadres = document.querySelectorAll(".tableau-cadre");
if (cadres.length > 0) {
  var mesureEnAttente = false;
  var mesurerCadres = function () {
    mesureEnAttente = false;
    // Écritures, puis lectures, puis écritures : une seule mise en page de chaque côté.
    cadres.forEach(function (cadre) {
      cadre.classList.remove("en-fiches");
    });
    var debordent = Array.prototype.filter.call(cadres, function (cadre) {
      return cadre.scrollWidth > cadre.clientWidth + 1;
    });
    debordent.forEach(function (cadre) {
      cadre.classList.add("en-fiches");
    });
  };
  var planifierMesure = function () {
    if (!mesureEnAttente) {
      mesureEnAttente = true;
      window.requestAnimationFrame(mesurerCadres);
    }
  };
  mesurerCadres();
  if (typeof window.ResizeObserver === "function") {
    var observateur = new window.ResizeObserver(planifierMesure);
    cadres.forEach(function (cadre) {
      observateur.observe(cadre);
    });
  } else {
    window.addEventListener("resize", planifierMesure);
  }
  // Une police qui arrive après coup change la largeur des colonnes.
  if (document.fonts && document.fonts.ready) {
    document.fonts.ready.then(planifierMesure);
  }
}

// Dans le formulaire d'intervention, deux champs suivent le type de l'intervention : le résultat
// du contrôle ([data-resultat-controle]) n'apparaît que pour un contrôle technique ou une
// contre-visite (#107) ; le type du document joint (select[name=type_document]) devient « Contrôle
// technique » pour ces deux-là et « Facture » sinon, tant qu'on n'y a pas touché (#104). Sans ce
// script, le serveur règle les deux à l'ouverture de la page, et l'utilisateur corrige.
document.querySelectorAll("form select[name=type_intervention]").forEach(function (typeIntervention) {
  var formulaire = typeIntervention.form;
  if (!formulaire) {
    return;
  }
  var typeDocument = formulaire.querySelector("select[name=type_document]");
  var resultat = formulaire.querySelector("[data-resultat-controle]");
  var estUnControle = function () {
    return typeIntervention.value === "controle_technique" || typeIntervention.value === "contre_visite";
  };
  var touche = false;
  if (typeDocument) {
    typeDocument.addEventListener("change", function () {
      touche = true;
    });
  }
  typeIntervention.addEventListener("change", function () {
    if (resultat) {
      resultat.hidden = !estUnControle();
    }
    if (!typeDocument || touche) {
      return;
    }
    var voulu = estUnControle() ? "controle_technique" : "facture";
    if (typeDocument.querySelector('option[value="' + voulu + '"]')) {
      typeDocument.value = voulu;
    }
  });
});

// Formulaire d'échéance : ce qu'il y a à faire (data-pour-type="a_faire") n'apparaît que pour le
// type « À faire », et le libellé (data-pour-operation="autre") que pour l'opération « Autre ».
// Une zone masquée est aussi désactivée : le navigateur n'envoie alors pas ses champs, et le
// serveur ne reçoit pas de pièce sur un contrôle technique. Sans ce script, tout reste visible.
document.querySelectorAll("form select[name=type_echeance]").forEach(function (type) {
  var formulaire = type.form;
  if (!formulaire) {
    return;
  }
  var operation = formulaire.querySelector("select[name=operation]");
  var regler = function (zone, visible) {
    zone.hidden = !visible;
    zone.querySelectorAll("input, select").forEach(function (champ) {
      champ.disabled = !visible;
    });
  };
  var appliquer = function () {
    // Dans l'ordre du document : la zone du type d'abord, celle de l'opération (dedans) ensuite.
    formulaire.querySelectorAll("[data-pour-type]").forEach(function (zone) {
      regler(zone, type.value === zone.dataset.pourType);
    });
    formulaire.querySelectorAll("[data-pour-operation]").forEach(function (zone) {
      var visible = type.value === "a_faire" && operation !== null && operation.value === zone.dataset.pourOperation;
      regler(zone, visible);
    });
  };
  type.addEventListener("change", appliquer);
  if (operation) {
    operation.addEventListener("change", appliquer);
  }
  appliquer();
});

// Formulaire de dépense (#126) : le détail d'un plein (data-pour-type="carburant" : quantité,
// compteur, plein complet) n'apparaît que pour le type « Carburant ». Masquée, la zone est
// désactivée : ses champs ne partent pas, et le serveur ne reçoit pas de quantité sur un péage.
// Le carburant du plein (select[name=carburant]) est proposé d'après le véhicule choisi
// (data-carburant sur son option), tant qu'on n'y a pas touché ; l'unité de la quantité
// (span[data-unite]) suit le carburant du plein (data-unite sur son option) : litres, kWh pour
// l'électricité, kg pour le GNV. Le span se vise par son nom de balise : les options portent
// le même attribut, et la première viendrait avant lui (#131). Sans ce script, tout reste visible, le serveur propose le
// carburant à l'ouverture de la page et refuse ce qui n'irait pas avec le type.
document.querySelectorAll("form select[name=type_depense]").forEach(function (type) {
  var formulaire = type.form;
  if (!formulaire) {
    return;
  }
  var appliquer = function () {
    formulaire.querySelectorAll("[data-pour-type]").forEach(function (zone) {
      var visible = type.value === zone.dataset.pourType;
      zone.hidden = !visible;
      zone.querySelectorAll("input, select").forEach(function (champ) {
        champ.disabled = !visible;
      });
    });
  };
  type.addEventListener("change", appliquer);
  appliquer();
  var vehicule = formulaire.querySelector("select[name=vehicule_id]");
  var carburant = formulaire.querySelector("select[name=carburant]");
  var unite = formulaire.querySelector("span[data-unite]");
  if (!carburant) {
    return;
  }
  var touche = false;
  var suivreLUnite = function () {
    var option = carburant.selectedOptions[0];
    if (unite && option && option.dataset.unite) {
      unite.textContent = option.dataset.unite;
    }
  };
  carburant.addEventListener("change", function () {
    touche = true;
    suivreLUnite();
  });
  if (vehicule) {
    vehicule.addEventListener("change", function () {
      var option = vehicule.selectedOptions[0];
      var propose = option && option.dataset.carburant;
      if (!touche && propose && carburant.querySelector('option[value="' + propose + '"]')) {
        carburant.value = propose;
        suivreLUnite();
      }
    });
  }
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

// Un lien vers une section repliée (`/vehicules#reformes`) l'ouvre : un navigateur n'ouvre pas de
// lui-même le <details> que vise l'adresse. À l'arrivée sur la page, et quand l'adresse change sans
// recharger la page.
function ouvrirSectionVisee() {
  if (location.hash.length > 1) {
    var visee = document.getElementById(decodeURIComponent(location.hash.slice(1)));
    if (visee && visee.tagName === "DETAILS") {
      visee.open = true;
    }
  }
}
ouvrirSectionVisee();
window.addEventListener("hashchange", ouvrirSectionVisee);
