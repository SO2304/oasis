# OASIS face à Veridify DOME (2026-10-07)

Chaque affirmation sur Veridify vient d'une source publique, datée dans
`docs/competition/VERIDIFY.md` (pages consultées le 2026-10-06). Aucun produit Veridify
n'a été testé, sondé ni téléchargé. **« Non documenté » veut dire « pas de source
publique trouvée », jamais « absent ».** Chaque affirmation sur OASIS renvoie à un log
silicium, un test ou une preuve Kani du dépôt.

## 1. Point par point

| Point | Veridify DOME (public) | OASIS (preuve) |
|---|---|---|
| Authentification d'équipement à équipement | Documentée | Prouvée sur 3 RP2040 : v0B, 0/150 inversions de bit acceptées (`evidence/silicon/2026-10-06/followup/`) |
| Vérification de l'origine **et du contenu à chaque relais**, avec la clé propre de l'émetteur | Non documentée (authentification d'équipement à équipement, appliances Sentry) | Prouvée : chaque saut vérifie avant de relayer ; contenu remplacé et nœud forgé rejetés au premier saut (`…/2026-10-06/REPORT_MESH_V0B.md`) |
| Post-quantique | « Supporte » ML-DSA, ML-KEM, Falcon ; défaut, option ou hybride : non documenté | **Hybride Ed25519 + ML-DSA-44** sur les messages d'autorité, suite minimale persistée et jamais abaissée ; rétrogradation refusée en 4 ms ; 377 ms, 48,7 Ko de pile sur Cortex-M0+ (`…/2026-10-06/pq/`). **Pas de ML-KEM** |
| Enrôlement | Zero-touch, application mobile | Clé générée sur la carte, preuve de possession, attestation signée ; nœud non enrôlé refusé au premier saut (`…/2026-10-06/enroll/`) |
| Transfert de propriété | Documenté (« pedigree » par équipement) | Ancien **et** nouveau propriétaire requis ; attaquant et ancien propriétaire refusés ensuite, aussi après coupure (`…/enroll/`). Propriété **par réseau**, pas par équipement |
| Mise à jour du firmware | Documentée, sans détails | A/B signée, plancher anti-retour, retour automatique ; image modifiée, ancienne ou mal signée refusée ; **coupure pendant l'envoi et pendant l'échange**, récupérées (`…/2026-10-06/fwupdate/`). USB seulement |
| Équipements existants | **DOME Sentry** : Modbus TCP et RTU, BACnet, DNP3, EtherNet/IP, MQTT | Passerelle **Modbus RTU** seulement (fil TTL, un équipement) : 4 décisions « agir » = 4 écritures sur l'équipement, à l'octet près ; tout le reste bloqué (`…/2026-10-07/modbus/`) |
| Autorisation par commande (registre, valeur, actionneur, expiration, état des capteurs) | Non documentée (blocage des équipements non authentifiés documenté) | Prouvée sur silicium et par 3 + 3 harnais Kani (porte d'actionnement ; passerelle : « aucune trame sans décision agir ») |
| Révocation d'un nœud | Non documentée (registre de propriété documenté) | Signée par l'opérateur, propagée, persistée, appliquée par chaque relais (`…/2026-10-06/ef/`) |
| Preuves formelles | Non documentées | 149 harnais Kani : 123 vérifiés en CI, 20 nouveaux vérifiés un par un, 6 non vérifiés et nommés (`CLAUDE.md`) |
| Algorithmes | WalnutDSA et Ironwood (maison, à base de groupes de tresses) + ML-DSA, ML-KEM, Falcon | Standards uniquement : Ed25519, X25519, ChaCha20-Poly1305, SHA-256, ML-DSA-44 |
| Protection matérielle de la clé, démarrage sécurisé | Non documentés pour DOME | **Absents** sur RP2040 : clé lisible en BOOTSEL ou SWD, aucune vérification de l'image au démarrage |
| Fonctionnement hors ligne | Documenté pour l'authentification | Prouvé : aucun service distant n'existe |
| Certification | ISO 26262 ASIL D (2019, bibliothèque) | Aucune |
| Clients, partenaires | 17 partenaires, dont ST, Renesas, Microchip, KMC Controls | Aucun |
| Code et preuves | Code fermé, SDK sur demande | Code et preuves archivés ; **dépôt encore privé** |

## 2. Ce que Veridify fait mieux

- **Marché** : des fabricants partenaires et des clients réels. OASIS n'en a aucun.
- **Certification** : ISO 26262 ASIL D. OASIS n'a ni certification ni audit externe.
- **Largeur** : cinq familles de protocoles industriels, par le réseau. OASIS ne gère
  que Modbus RTU, par fil.
- **Produit** : enrôlement zero-touch, application mobile, console cloud ou locale.
  OASIS n'a qu'un outil en ligne de commande.
- **Matériel** : un large choix de microcontrôleurs et de FPGA. OASIS est prouvé sur
  un seul, le RP2040, sans protection de la clé.
- **ML-KEM** : OASIS n'a pas d'échange de clés post-quantique.
- **Maturité** : produit vendu depuis des années. OASIS est à TRL 4.

## 3. Trois phrases pour un fabricant

1. « **Chaque relais vérifie l'origine et le contenu d'un ordre avec la clé propre de
   son émetteur ; un nœud capturé ne peut parler que pour lui-même et se révoque.** »
   Preuve : 0/150 inversions acceptées, contenu remplacé et nœud forgé rejetés au premier
   saut, révocation appliquée par chaque relais (3 RP2040).
2. « **Devant votre équipement Modbus existant, seule une écriture autorisée pour ce
   registre, dans cette plage, fraîche et non rejouée, atteint le bus.** » Preuve :
   équipement non modifié qui compte chaque octet ; 4 décisions « agir » = 4 écritures ;
   ordres forgés, rejoués, expirés, hors plage et trames brutes injectées bloqués ;
   règle « aucune trame sans décision agir » prouvée par Kani.
3. « **Les messages d'autorité sont signés en hybride classique et post-quantique, et
   une rétrogradation est refusée avant tout calcul, sur un microcontrôleur à 1 $.** »
   Preuve : ML-DSA-44 + Ed25519 en 377 ms sur Cortex-M0+ ; message rétrogradé refusé en
   4 ms ; politique conservée après une coupure de courant.

## 4. Ce que ce document ne dit pas

Aucune faiblesse de Veridify n'est affirmée ici au-delà de ce qui est public : le
cassage de WalnutDSA en 2018 est publié (et Veridify y a répondu en 2019 et 2020, sans
analyse indépendante publiée depuis) ; tout le reste est « non documenté ». Veridify
peut faire, sans le publier, ce que ce tableau marque non documenté.
