//! Le rappel par courriel (#176) : composé depuis une vraie base, puis envoyé à un faux serveur
//! SMTP qui tient dans le test. Sans TLS : c'est le cas « serveur local » (`Securite::Aucune`).

mod commun;

use sqlx::PgPool;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::net::TcpListener;

use gestion_parc_auto::config::{Courriel, Securite};
use gestion_parc_auto::metier::alertes::Horizon;
use gestion_parc_auto::metier::echeances::{self, SaisieEcheance};
use gestion_parc_auto::metier::{courriel, rappels};

use commun::{aujourd_hui, creer_vehicule};

/// Un serveur SMTP minimal : accepte tout, garde ce qu'il reçoit après `DATA`, puis rend
/// l'échange complet (commandes et message) au test.
async fn faux_serveur_smtp() -> (u16, tokio::task::JoinHandle<String>) {
    let ecoute = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let port = ecoute.local_addr().unwrap().port();
    let tache = tokio::spawn(async move {
        let (flux, _) = ecoute.accept().await.unwrap();
        let (lecture, mut ecriture) = flux.into_split();
        let mut lignes = BufReader::new(lecture).lines();
        let mut transcription = String::new();
        ecriture
            .write_all(b"220 faux.local ESMTP\r\n")
            .await
            .unwrap();
        let mut dans_les_donnees = false;
        while let Ok(Some(ligne)) = lignes.next_line().await {
            transcription.push_str(&ligne);
            transcription.push('\n');
            if dans_les_donnees {
                if ligne == "." {
                    dans_les_donnees = false;
                    ecriture.write_all(b"250 OK\r\n").await.unwrap();
                }
                continue;
            }
            let commande = ligne.to_ascii_uppercase();
            let reponse: &[u8] = if commande.starts_with("EHLO") {
                b"250-faux.local\r\n250 AUTH PLAIN LOGIN\r\n"
            } else if commande.starts_with("AUTH PLAIN") {
                b"235 OK\r\n"
            } else if commande.starts_with("DATA") {
                dans_les_donnees = true;
                b"354 Envoyez\r\n"
            } else if commande.starts_with("QUIT") {
                ecriture.write_all(b"221 Au revoir\r\n").await.unwrap();
                break;
            } else {
                b"250 OK\r\n"
            };
            ecriture.write_all(reponse).await.unwrap();
        }
        transcription
    });
    (port, tache)
}

#[sqlx::test]
async fn le_rappel_du_jour_part_vers_le_serveur_smtp(pool: PgPool) {
    // Une échéance dépassée, une à venir, un compteur jamais relevé depuis la création.
    let vehicule = creer_vehicule(&pool, "RP-100-EL").await;
    // La date fixe des tests (01/06/2026) : le relevé posé à la création du véhicule, daté du
    // jour réel, est dans le futur et ne compte pas, donc pas de compteur à relever.
    let aujourd_hui = aujourd_hui();
    for (type_echeance, date) in [
        ("controle_technique", "2026-05-28"),
        ("assurance", "2026-06-11"),
    ] {
        echeances::creer(
            &pool,
            &SaisieEcheance {
                vehicule_id: vehicule.to_string(),
                type_echeance: type_echeance.to_string(),
                date_limite: date.to_string(),
                ..SaisieEcheance::default()
            },
        )
        .await
        .expect("échéance");
    }
    let rappel = rappels::du_jour(
        &pool,
        aujourd_hui,
        Horizon {
            jours: 30,
            km: 1000,
        },
        "Autoclair",
        "https://parc.exemple.fr",
    )
    .await
    .expect("composition")
    .expect("il y a quelque chose à dire");
    assert_eq!(rappel.sujet, "Autoclair : 1 alerte dépassée, 1 à venir");
    assert!(
        rappel
            .corps
            .contains("Contrôle technique : 28/05/2026, depuis 4 jours"),
        "{}",
        rappel.corps
    );
    assert!(
        rappel
            .corps
            .contains("Assurance : 11/06/2026, dans 10 jours"),
        "{}",
        rappel.corps
    );
    assert!(
        rappel
            .corps
            .contains(&format!("https://parc.exemple.fr/vehicules/{vehicule}")),
        "{}",
        rappel.corps
    );

    let (port, serveur) = faux_serveur_smtp().await;
    let config = Courriel {
        hote: "127.0.0.1".to_string(),
        port,
        securite: Securite::Aucune,
        identifiant: Some("parc".to_string()),
        mot_de_passe: Some("secret-de-test".to_string()),
        expediteur: "Autoclair <parc@exemple.fr>".to_string(),
        destinataires: vec![
            "gestion@exemple.fr".to_string(),
            "second@exemple.fr".to_string(),
        ],
        adresse_application: "https://parc.exemple.fr".to_string(),
    };
    // Les destinataires (#182) : les comptes actifs qui ont demandé le rappel, puis ceux du
    // `.env`, sans doublon (la casse ne compte pas). « muet » n'a pas coché, « parti » est
    // désactivé, « second » a la même adresse que le `.env` avec une majuscule.
    for (login, adresse, rappels, actif) in [
        ("gestion", "gestion@exemple.fr", true, true),
        ("muet", "muet@exemple.fr", false, true),
        ("parti", "parti@exemple.fr", true, false),
        ("second", "Second@exemple.fr", true, true),
    ] {
        let compte = gestion_parc_auto::metier::utilisateurs::creer(
            &pool,
            login,
            "mot-de-passe-de-test",
            "lecteur",
        )
        .await
        .expect("compte");
        gestion_parc_auto::donnees::utilisateurs::definir_courriel(
            &pool, compte.id, adresse, rappels,
        )
        .await
        .expect("adresse");
        gestion_parc_auto::donnees::utilisateurs::definir_actif(&pool, compte.id, actif)
            .await
            .expect("état");
    }
    let destinataires = rappels::destinataires(&pool, Some(&config))
        .await
        .expect("destinataires");
    assert_eq!(destinataires, ["gestion@exemple.fr", "Second@exemple.fr"]);
    courriel::envoyer(&config, &destinataires, &rappel.sujet, &rappel.corps)
        .await
        .expect("envoi");
    let transcription = serveur.await.unwrap();
    assert!(
        transcription.contains("MAIL FROM:<parc@exemple.fr>"),
        "{transcription}"
    );
    assert!(
        transcription.contains("RCPT TO:<gestion@exemple.fr>"),
        "{transcription}"
    );
    assert!(
        transcription.contains("RCPT TO:<Second@exemple.fr>"),
        "{transcription}"
    );
    assert_eq!(
        transcription.matches("RCPT TO:").count(),
        2,
        "{transcription}"
    );
    assert!(transcription.contains("AUTH PLAIN"), "{transcription}");
    assert!(transcription.contains("Subject: "), "{transcription}");
    // Le corps part en texte brut ; les accents passent par un encodage de transfert, mais
    // l'adresse et les chiffres restent lisibles tels quels.
    assert!(
        transcription.contains("Content-Type: text/plain"),
        "{transcription}"
    );
    assert!(
        transcription.contains("https://parc.exemple.fr/alertes"),
        "{transcription}"
    );
    assert!(transcription.contains("28/05/2026"), "{transcription}");
    assert!(
        !transcription.contains("secret-de-test"),
        "le mot de passe ne passe qu'encodé en AUTH PLAIN"
    );
}

#[sqlx::test]
async fn rien_a_dire_rien_a_envoyer(pool: PgPool) {
    let rappel = rappels::du_jour(
        &pool,
        aujourd_hui(),
        Horizon {
            jours: 30,
            km: 1000,
        },
        "Autoclair",
        "",
    )
    .await
    .expect("composition");
    assert_eq!(rappel, None);
}
