# OASIS face à Veridify DOME (mis à jour le 2026-10-08)

> ⚠️ **OASIS n'est pas une fonction de sûreté certifiée** : ni PL (ISO 13849-1) ni SIL
> (IEC 62061). Il ne réduit aucun risque machine — il décide si un ordre est authentique,
> habilité, frais et dans les limites. L'arrêt d'urgence reste un circuit dédié qu'OASIS ne
> peut pas atteindre. Voir [`docs/compliance/IEC_TS_63074.md`](../docs/compliance/IEC_TS_63074.md).

**Comment lire cette page.** Côté Veridify, uniquement des **sources publiques**, datées :
pages produit, communiqués, publications, documents du NIST. Aucun produit Veridify n'a
été testé, sondé ni téléchargé ; personne n'a été contacté. **« Non documenté » veut dire
« aucune source publique trouvée », jamais « absent »** : un produit fermé peut très bien
le faire sans le publier. La veille complète est dans `docs/competition/VERIDIFY.md`.

Côté OASIS, **prouvé** = un test, un journal silicium ou une preuve Kani référencés ici.
Rien n'est revendiqué sans sa preuve. OASIS est à **TRL 4** : banc de trois RP2040 sur
fil, pas de produit, pas de client. Veridify vend un produit déployé.

---

## 1. Tableau point par point

| Point | Veridify (public, daté) | OASIS | Preuve OASIS |
| --- | --- | --- | --- |
| Authentification d'équipement à équipement | documenté ([DOME](https://www.veridify.com/dome/), 2025-11-10) | prouvé | `evidence/silicon/2026-10-06/REPORT_MESH_V0B.md` |
| **Vérification de l'origine ET du contenu à chaque relais, avec la clé propre de l'émetteur** | **non documenté** (authentification d'équipement à équipement ; mesh multi-saut non documenté) | prouvé, fil UART | 0/150 inversions de bit acceptées ; contenu remplacé rejeté ; `…/followup/REPORT.md` |
| **Porte d'actionnement par ordre** : habilitation par actionneur, expiration dans l'horloge de l'actionneur, état des capteurs, limites physiques, anti-rejeu | **non documenté** (les équipements non authentifiés sont bloqués et journalisés — [Modbus article](https://www.veridify.com/article/modbus-security-issues-and-how-to-mitigate-cyber-risks/), 2023-05-15) | prouvé, 9 conditions | `evidence/silicon/2026-10-06/ef/REPORT.md` |
| **Preuve formelle de la règle d'actionnement** | non documenté | prouvé | 3 harnais Kani, `evidence/kani/2026-10-06/` |
| Post-quantique standardisé | documenté : ML-KEM (FIPS 203), ML-DSA (FIPS 204), Falcon ([DOME](https://www.veridify.com/dome/)). Par défaut, en option ou hybride : non documenté | **hybride Ed25519 + ML-DSA-44** sur les messages d'autorité ; pas de KEM, pas de Falcon | `evidence/silicon/2026-10-06/pq/REPORT.md` (377 ms, 48,7 Ko de pile sur RP2040) |
| Enrôlement | documenté : « Zero-Touch Provisioning », certificats créés, distribués, renouvelés ([article](https://www.veridify.com/article/zero-trust-security-for-legacy-ot-devices/), 2025-05-09) | prouvé : clé générée **sur la carte**, jamais sortie, attestation signée par le propriétaire après preuve de possession | `evidence/silicon/2026-10-06/enroll/REPORT.md` |
| Transfert de propriété | documenté : « embedded blockchain pedigree », transfert ([page](https://www.veridify.com/dome-iot-device-ownership/), 2025-11-14). Contenu, signataires, format : non documentés | prouvé : signature de l'ancien **et** du nouveau propriétaire, lié à l'empreinte de l'offre | `…/enroll/REPORT.md` (o1→o2 sur 3 cartes) |
| Mise à jour du firmware signée | documenté ([DOME](https://www.veridify.com/dome/)) ; mécanisme non documenté | prouvé : A/B, manifeste hybride, plancher anti-retour persisté, **retour automatique**, testé sur **deux coupures de courant réelles** | `evidence/silicon/2026-10-06/fwupdate/REPORT.md` |
| Équipements existants | documenté et **plus large** : DOME Sentry devant les équipements, Modbus TCP, Modbus RTU en 1:N, BACnet/IP et MS/TP, DNP3, EtherNet/IP, MQTT | prouvé mais **étroit** : Modbus RTU, FC06 et FC16, un équipement, TTL sans RS-485 | `evidence/silicon/2026-10-07/modbus/REPORT.md` |
| **Autorisation par écriture** (qui, quel registre, quelle valeur, dans quel état) | **non documenté** | prouvé : seules les décisions « agir » atteignent l'équipement — 4 décisions = 4 trames = 4 écritures, octet pour octet | `…/modbus/REPORT.md` |
| **Déni de service par vérification forcée** | non documenté | prouvé : pré-filtre à clé de lien, **179,4 ms → 0,70 ms** par trame forgée (256×) ; sous flot à 8/s, **60 messages légitimes sur 60** livrés à 19 % de charge contre 39/60 à 110 % sans lui | `evidence/silicon/2026-10-07/prefilter/REPORT.md` |
| Fonctionnement sans cloud | documenté pour l'authentification (FAQ) ; rôle exact du service de gestion non documenté | prouvé : aucun service distant n'existe dans le produit | tous les journaux silicium |
| Révocation d'un nœud | non documenté (registre de propriété documenté) | prouvé : signée par l'opérateur, k-sur-n, propagée et appliquée par chaque relais ; k-sur-n sur PC seulement | `…/ef/REPORT.md` |
| Primitives cryptographiques | WalnutDSA et Ironwood (propres, fondés sur les tresses) **plus** ML-KEM, ML-DSA, Falcon | **uniquement des standards** : Ed25519 (RFC 8032), X25519 (RFC 7748), ChaCha20-Poly1305 (RFC 8439), SHA-256, HMAC (RFC 2104), HKDF (RFC 5869), ML-DSA-44 (FIPS 204) | `CLAUDE.md`, `deny.toml` |
| Chaîne d'approvisionnement | non documenté | prouvé : SBOM CycloneDX par crate, **0 vulnérabilité connue** sur 6 arbres, `cargo-deny` vert | `evidence/supply-chain/2026-10-07/REPORT.md` |
| Fuzzing des analyseurs | non documenté | prouvé : 8 heures-cible, **1,50 × 10⁹ exécutions**, 1 défaut trouvé et corrigé | `evidence/fuzz/2026-10-07/REPORT.md` |
| Preuves formelles de l'ensemble | non documenté | **130 des 152 harnais vérifiés**, aucun contre-exemple ; 22 inconnus (mémoire) | `evidence/kani/2026-10-07/full/` |
| Certification | **ISO 26262 ASIL D** pour leur bibliothèque, juin 2019 (déclaré en FAQ) | **aucune** | — |
| Audit externe publié | non documenté | **aucun** | — |
| Clients, partenaires | documentés : 17 partenaires (AWS, Intel, Microsoft, Microchip, Renesas, ST, Distech, KMC Controls, Raytheon), financements SBIR NSF et US Air Force ([Partners](https://www.veridify.com/partners/), 2025-11-05) | **aucun client, aucun entretien** | `partners/POSITIONING_GAPS.md` D1 |
| Matériel | 8, 16 et 32 bits (RL78, MSP430, AVR cités), FPGA Intel et Microchip | RP2040 (Cortex-M0+) testé ; `thumbv7em` compile, **jamais démarré** | `CLAUDE.md` |
| Radio | non applicable (filaire et IP) | **aucune** : fil UART uniquement, pas de LoRa | — |

---

## 2. Ce que Veridify fait mieux

À dire sans détour à un fabricant, parce que c'est vrai et vérifiable :

1. **Des clients et un réseau de distribution.** 17 partenaires publics, dont des
   fabricants de semi-conducteurs (ST, Renesas, Microchip), des intégrateurs
   (KMC Controls, Distech) et des financements SBIR de la NSF et de l'US Air Force.
   OASIS n'a **aucun client et aucun entretien client** : les segments visés sont des
   hypothèses, pas des faits.
2. **Une certification.** ISO 26262 ASIL D pour leur bibliothèque de sécurité (2019).
   OASIS n'a aucune certification, aucun audit externe, et sa « porte d'actionnement »
   n'est **pas** une fonction de sûreté certifiée.
3. **Une couverture de protocoles bien plus large.** BACnet/IP et MS/TP, DNP3,
   EtherNet/IP, MQTT, Modbus TCP et RTU en 1:N. OASIS ne fait que du Modbus RTU, deux
   codes de fonction, un équipement, sur du TTL sans RS-485.
4. **Le post-quantique plus complet.** ML-KEM (échange de clés) et Falcon en plus de
   ML-DSA. OASIS n'a **aucun KEM post-quantique** : son échange de clés reste X25519,
   donc classique.
5. **Un déploiement industrialisé.** Provisionnement sans contact à l'échelle, gestion
   cloud ou sur site, application mobile, renouvellement des identifiants. OASIS a un
   outil en ligne de commande sur PC.
6. **La maturité.** Produit déployé contre TRL 4 ; une équipe contre une personne ; du
   support sur plusieurs années contre aucun engagement.

**Sur WalnutDSA, la formulation exacte compte.** Leur algorithme propre a été cassé
publiquement en 2018 — falsification universelle en « environ deux minutes »
([Hart et al., PKC 2018](https://link.springer.com/chapter/10.1007/978-3-319-76578-5_13)),
puis clé secrète équivalente calculée en moins d'une seconde
([Beullens et Blackburn, Asiacrypt 2018](https://eprint.iacr.org/2018/318)) — y compris
sur les paramètres révisés ([Kotov et al.](https://eprint.iacr.org/2018/393)), et il
**n'a pas été retenu** pour le 2ᵉ tour du NIST en janvier 2019
([NISTIR 8240](https://nvlpubs.nist.gov/nistpubs/ir/2019/NIST.IR.8240.pdf)) — ce n'est
pas un retrait. Veridify a répondu en 2019 et 2020 avec de nouveaux paramètres, et
**aucune analyse tierce publique de ces paramètres de 2020 n'existe, ni pour ni contre**.
Le rôle exact de WalnutDSA dans DOME aujourd'hui est **non documenté**. Dire « leur
crypto est cassée » serait donc faux ; la formulation défendable est : *leur algorithme
propriétaire a une histoire publique d'attaques réussies et n'a pas été retenu par le
NIST, et ses paramètres actuels n'ont pas été analysés publiquement — là où OASIS
n'utilise que des primitives standardisées.*

---

## 3. Trois phrases utilisables face à un fabricant

Chacune ne dit que ce qui est mesuré, et la preuve est citable.

> **1.** « Dans un maillage à plusieurs sauts, chaque relais vérifie l'origine **et le
> contenu** avec la clé propre de l'émetteur : sur trois RP2040, **aucune des 150
> inversions de bit** n'a été acceptée, un contenu remplacé est rejeté au premier saut,
> et un rejeu après coupure de courant est refusé depuis la fenêtre restaurée en flash. »
> — `evidence/silicon/2026-10-06/REPORT_MESH_V0B.md`

> **2.** « Un ordre n'atteint un actionneur que si **neuf conditions** sont vraies, dont
> l'état des capteurs et les limites physiques de la machine. La règle est **prouvée
> formellement** et vérifiée sur silicium : ordre non habilité, expiré, rejoué, capteur
> perdu, hors limites et NaN sont tous refusés. »
> — `evidence/silicon/2026-10-06/ef/REPORT.md`, `evidence/kani/2026-10-06/`

> **3.** « Devant un équipement Modbus existant qui n'authentifie rien, **aucune écriture
> n'a atteint l'équipement sans décision « agir »** : 4 décisions = 4 trames = 4 écritures,
> octet pour octet, alors que le même équipement exécute la trame forgée si on la lui
> envoie directement. »
> — `evidence/silicon/2026-10-07/modbus/REPORT.md`

**La phrase à ne pas dire :** « nous résistons au déni de service ». Ce qui est mesuré,
c'est qu'une trame forgée par un **extérieur** coûte 0,70 ms au lieu de 179,4 ms, et que
60 messages légitimes sur 60 passent sous un flot à 8/s. Contre un **initié** — un nœud
enrôlé, ou une clé de lien volée en lisant la flash d'un RP2040 — le budget par liaison
borne le calcul du relais mais **affame le trafic légitime** (4 messages sur 60). C'est
un plafond de calcul, pas de l'équité.

---

## 4. Loyauté envers Veridify

Aucune faiblesse de Veridify n'est affirmée ici au-delà de ce qui est public : le
cassage de WalnutDSA en 2018 est publié, et Veridify y a répondu en 2019 et 2020,
sans analyse indépendante publiée depuis ; tout le reste est **« non
documenté »**. Veridify peut parfaitement faire, sans le publier, ce que ce tableau
marque non documenté. Aucun de leurs produits n'a été testé, sondé ni
téléchargé ; personne n'a été contacté.

## 5. Ce qui reste à faire avant de parler à un client

État au 2026-10-08, d'après la **section G** de
[`POSITIONING_GAPS.md`](POSITIONING_GAPS.md), qui est la source de vérité du statut de
chaque manque :

- **Des entretiens client.** Zéro à ce jour. C'est **le seul manque 🔴 encore
  ouvert** (D1) ; [`CUSTOMER_DISCOVERY.md`](CUSTOMER_DISCOVERY.md) est prêt et aucune de
  ses cinq hypothèses n'est testée.
- **Le stockage sécurisé de la clé** (C14) : la clé privée est lisible en flash
  par BOOTSEL ou SWD. **Ne se ferme pas en logiciel** — c'est l'avantage matériel de
  Veridify et la première objection d'un RSSI.
- **La rotation de clé de nœud** (C6) : elle n'existe pas, et `MAX_REVOKED = 16` est un
  plafond dur. C'est le prérequis qui gouverne tout calendrier de migration
  cryptographique.
- **Une radio réelle** (C11) : aucune radio n'a jamais émis, tous les budgets sont
  calculés. Le duty-cycle, lui, **est appliqué par le code** depuis le 2026-10-08 (C3 fermé) :
  le transport refuse d'émettre hors budget.
- **Un KEM post-quantique** : l'échange de clés reste X25519. C'est l'écart réel
  avec le ML-KEM documenté de Veridify.
- **La confidentialité des ordres** (C13) : v0B et v0C authentifient, ils ne chiffrent
  pas.
- **L'équité du pré-filtre contre un initié** : le seau à jetons est un plafond
  de calcul, et la livraison légitime tombe à 4/60. À ne jamais présenter comme une
  protection de disponibilité.
- **Une suite Kani complète sur les 182 harnais** : elle n'a jamais tourné, ni en CI
  ni en local. « 182/182 » serait faux.

Fermés depuis la version précédente de ce document, et donc retirés de cette liste :
**C2** (arrêt asymétrique et perte de liaison), **C5** (journal infalsifiable),
**C12** (vivacité de la supervision) et **C4** (protocole d'émission d'un ordre) —
tous les quatre prouvés sur silicium le 2026-10-08.
