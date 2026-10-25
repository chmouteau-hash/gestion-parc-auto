// Service worker de l'application (#180) : le petit programme que le navigateur garde pour
// afficher la page « Pas de réseau » quand le serveur est injoignable. Il ne fait que cela.
//
// - À l'installation, il met en cache la page hors ligne et ce qu'elle affiche (feuilles de
//   style, polices, logo de la marque) : la liste FICHIERS.
// - Une navigation (ouvrir une page, suivre un lien) part au réseau ; si le réseau refuse
//   (téléphone hors connexion, serveur arrêté), la page hors ligne la remplace. Une réponse du
//   serveur, même une erreur, est rendue telle quelle.
// - Un fichier de la liste part lui aussi au réseau d'abord : le cache ne sert que sans réseau,
//   une mise à jour du serveur se voit donc tout de suite.
// - Tout le reste (formulaires, API, documents, autres fichiers) ne passe pas par ici.
//
// Le serveur remplace __VERSION__ (empreinte du contenu des fichiers embarqués) et __FICHIERS__
// (la liste, qui dépend de la marque) avant de servir ce fichier (`web/hors_ligne.rs`). Une
// version nouvelle fait réinstaller le service worker : le cache se remplit à neuf, l'ancien
// est effacé.
var VERSION = "__VERSION__";
var FICHIERS = __FICHIERS__;
var CACHE = "autoclair-" + VERSION;
var PAGE_HORS_LIGNE = "/hors-ligne";

self.addEventListener("install", function (evenement) {
  evenement.waitUntil(
    caches
      .open(CACHE)
      .then(function (cache) {
        return cache.addAll(FICHIERS);
      })
      .then(function () {
        return self.skipWaiting();
      })
  );
});

self.addEventListener("activate", function (evenement) {
  evenement.waitUntil(
    caches
      .keys()
      .then(function (noms) {
        return Promise.all(
          noms
            .filter(function (nom) {
              return nom !== CACHE;
            })
            .map(function (nom) {
              return caches.delete(nom);
            })
        );
      })
      .then(function () {
        return self.clients.claim();
      })
  );
});

function depuisLeCache(chemin) {
  return caches.match(chemin).then(function (reponse) {
    return reponse || Response.error();
  });
}

self.addEventListener("fetch", function (evenement) {
  var requete = evenement.request;
  if (requete.method !== "GET") {
    return;
  }
  var adresse = new URL(requete.url);
  if (adresse.origin !== self.location.origin) {
    return;
  }
  if (requete.mode === "navigate") {
    evenement.respondWith(
      fetch(requete).catch(function () {
        return depuisLeCache(PAGE_HORS_LIGNE);
      })
    );
    return;
  }
  if (FICHIERS.indexOf(adresse.pathname) !== -1) {
    evenement.respondWith(
      fetch(requete).catch(function () {
        return depuisLeCache(adresse.pathname);
      })
    );
  }
});
