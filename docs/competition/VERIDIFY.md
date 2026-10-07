# Veridify Security (DOME) — veille sur sources publiques

**Méthode.** Uniquement des sources publiques : pages produit et partenaires de
Veridify, communiqués, publications scientifiques, rapports du NIST, prépublications.
Aucun produit, service ou système de Veridify n'a été testé, sondé ni téléchargé ;
personne n'a été contacté. Toutes les pages ont été consultées le **2026-10-06** ;
la date indiquée est celle de la page ou de la publication. **« Non documenté » veut
dire « je n'ai pas trouvé de source publique », jamais « absent ».**

## 1. Fiche produit

| Point | Ce qui est documenté | Source (date) |
|---|---|---|
| Société | Veridify Security, **anciennement SecureRF** (fondée en 2004) | [Digi-Key](https://www.digikey.com/en/design-services-providers/veridify-security-formerly-securerf) ; [About](https://www.veridify.com/about-us/) |
| Produit | **DOME** = *Device Ownership Management and Enrollment* | [Renesas — DOME](https://www.renesas.com/en/document/prb/veridifys-dome) |
| Fonctions | Authentification mutuelle au niveau de l'équipement, chiffrement de tout le trafic, « Zero Trust » conforme NIST, protection des équipements neufs **et existants** « sans changement réseau », livraison de firmware sécurisée, gestion des identifiants | [DOME](https://www.veridify.com/dome/) (2025-11-10) |
| Enrôlement | « Zero-Touch Provisioning » : création, distribution et renouvellement des certificats | [Zero trust for legacy OT](https://www.veridify.com/article/zero-trust-security-for-legacy-ot-devices/) (2025-05-09) |
| Propriété | « Every processor and device has its own embedded blockchain pedigree » ; transfert de propriété ; « in-field key and firmware updates » | [IoT Device Ownership](https://www.veridify.com/dome-iot-device-ownership/) (2025-11-14) |
| Équipements existants | **DOME Sentry**, appliance placée devant les équipements : Modbus TCP ; **Modbus RTU en 1:N derrière un routeur ou une passerelle** ; BACnet/IP et MS/TP, DNP3, EtherNet/IP, MQTT | [DOME](https://www.veridify.com/dome/) ; [Modbus PR](https://www.veridify.com/press-release/veridify-announces-cybersecurity-for-new-and-legacy-modbus-devices/) |
| Déploiement | « Cloud-based **or on-prem** » (2025) ; décrit comme « SaaS » par KMC Controls en 2021 ; application mobile DOME | [DOME](https://www.veridify.com/dome/) ; [KMC (2021-09-28)](https://www.veridify.com/press-release/kmc-controls-partners-with-veridify-security-to-make-buildings-cyber-safe/) ; [Google Play](https://play.google.com/store/apps/details?id=com.veridify.domeapp.v3.dome_mobile_app&hl=en_US) |
| Matériel cible | Microcontrôleurs 8, 16 et 32 bits (RL78, MSP430, AVR cités), FPGA Intel et Microchip ; partenaires ST, Renesas, onsemi, Synopsys | [FAQ](https://www.veridify.com/frequently-asked-questions/) ; [Partners](https://www.veridify.com/partners/) (2025-11-05) |
| Partenaires et clients | 17 partenaires listés (dont AWS, Intel « Titanium », Microsoft, Microchip, Renesas, ST, Distech, **KMC Controls**, Raytheon) ; financements SBIR de la NSF et de l'US Air Force | [Partners](https://www.veridify.com/partners/) (2025-11-05) |
| Prix | Non documenté (SDK « sur demande ») | [FAQ](https://www.veridify.com/frequently-asked-questions/) |

## 2. Cryptographie

**Algorithmes propres.** WalnutDSA (signature) et Ironwood KAP (accord de clé), tous
deux fondés sur les groupes de tresses et présentés comme résistants au quantique.
La FAQ ne cite **que** ces deux algorithmes ([FAQ](https://www.veridify.com/frequently-asked-questions/), non datée).

**Post-quantique standardisé.** La page DOME de 2025 annonce le support de
« FIPS 203 (ML-KEM), FIPS 204 (ML-DSA), and FALCON » ([DOME](https://www.veridify.com/dome/), 2025-11-10).
**Par défaut, en option ou en hybride : non documenté.** Le rôle de WalnutDSA dans
DOME aujourd'hui : non documenté.

**Historique public de WalnutDSA**, dans l'ordre :

| Date | Événement | Source |
|---|---|---|
| 2018 (PKC) | Hart, Kim, Micheli, Pascual-Perez, Petit, Quek : **falsification universelle** à partir de quelques signatures valides, « in approximately two minutes » | [Springer](https://link.springer.com/chapter/10.1007/978-3-319-76578-5_13) ; [ePrint 2017/1160](https://eprint.iacr.org/2017/1160) |
| 2018 (Asiacrypt) | Beullens et Blackburn : falsification et calcul d'une **clé secrète équivalente** en moins d'une seconde (128 bits) et d'une minute (256 bits). Les concepteurs annoncent de nouveaux paramètres | [ePrint 2018/318](https://eprint.iacr.org/2018/318) ; [Springer](https://link.springer.com/chapter/10.1007/978-3-030-03326-2_2) ; [commentaire officiel NIST](https://csrc.nist.gov/CSRC/media/Projects/Post-Quantum-Cryptography/documents/round-1/official-comments/WalnutDSA-official-comment.pdf) |
| 2018–2019 | Kotov, Menshov, Ushakov : clé de substitution récupérée avec « 100% success rate … **for recently suggested parameters values** (including a new way to generate cloaking elements) » | [ePrint 2018/393](https://eprint.iacr.org/2018/393) ; Designs, Codes and Cryptography 87 (2019) 2231-2250 |
| 30 janv. 2019 | WalnutDSA **non retenu** pour le 2e tour du NIST (pas un retrait : je n'ai pas trouvé de retrait) | [NISTIR 8240](https://nvlpubs.nist.gov/nistpubs/ir/2019/NIST.IR.8240.pdf) ; [annonce NIST](https://csrc.nist.gov/News/2019/pqc-standardization-process-2nd-round-candidates) |
| 2019 | Réponse de SecureRF : des éléments de camouflage bien choisis « renders WalnutDSA completely secure against this attack » | [Defeating the KMU attack](https://veridify.com/wp-content/uploads/2019/03/Defeating_kmu_attack-2019.pdf) |
| sept. 2020 | Article révisé : « When implemented using parameters that defeat all known attacks… » | [WalnutDSA (2020-09)](https://www.veridify.com/wp-content/uploads/2020/10/walnutdsa-paper-202009.pdf) |
| depuis | **Aucune analyse tierce publique** des paramètres de 2020 trouvée, ni pour ni contre | recherche du 2026-10-06 |

Ironwood : ses auteurs écrivent qu'il exige « secure provisioning of key material from
a trusted key distribution system prior to deployment »
([arXiv 1702.02450](https://arxiv.org/abs/1702.02450)). Analyse tierce publique
d'Ironwood : non trouvée. Son prédécesseur, l'Algebraic Eraser, a été cassé
([Ben-Zvi, Blackburn, Tsaban](https://www.researchgate.net/publication/305472575_A_Practical_Cryptanalysis_of_the_Algebraic_Eraser)) ;
Ironwood est présenté comme conçu pour y résister.

## 3. Modèle de confiance

- **Qui détient les clés** : non documenté. Ironwood suppose un système de
  distribution de clés de confiance avant déploiement (voir ci-dessus).
- **« Chaîne de blocs » de propriété** : un « pedigree » embarqué dans chaque
  équipement, vérifiable « without the need for a pervasive network or cloud
  connection ». Contenu, signataires et format : non documentés.
- **Hors ligne** : documenté comme possible pour l'authentification (« without the
  need for a network connection or database look-up », FAQ). Le rôle exact du
  service cloud ou local (gestion, renouvellement, journal) : non documenté.

## 4. Topologie

Authentification **d'équipement à équipement**, plus des appliances DOME Sentry
devant les équipements existants. Un mesh multi-saut où **chaque relais vérifie
l'origine et le contenu** : non documenté. Le comportement quand un équipement ou une
passerelle intermédiaire est compromis : non documenté.

## 5. Actionnement

Documenté : les équipements non authentifiés sont bloqués, l'événement est journalisé
et notifié ([Modbus article](https://www.veridify.com/article/modbus-security-issues-and-how-to-mitigate-cyber-risks/), 2023-05-15).
**Non documentés** : autorisation par commande (code fonction, registre), par
actionneur, selon l'état de la machine, l'expiration ou des limites physiques.

## 6. Assurance

| Point | Veridify |
|---|---|
| Certification | **ISO 26262 ASIL D** pour leur bibliothèque de sécurité, juin 2019 (déclaré dans la FAQ) |
| IEC 62443, FIPS 140-3 | Non documenté |
| Audit externe publié | Non documenté |
| Preuves formelles | Non documenté |
| Code | Algorithmes « published to support peer review » ; code non publié, SDK sur demande |

## 7. Tableau de synthèse

OASIS : **prouvé** = test, log silicium ou preuve Kani ; **conçu** = spécifié, pas
prouvé ; **absent** = n'existe pas.

| Point | Veridify | OASIS | Preuve OASIS |
|---|---|---|---|
| Authentification d'équipement à équipement | documenté | prouvé | `evidence/silicon/2026-10-06/REPORT_MESH_V0B.md` |
| Vérification de l'origine **et du contenu** à chaque relais, clé propre à l'émetteur | non documenté | prouvé (fil UART, pas de radio) | `…/followup/REPORT.md` : 150/150 tracés, 0 accepté |
| Fonctionnement sans cloud | documenté (authentification) | prouvé (aucun service distant n'existe) | tous les tests silicium |
| Révocation d'un nœud | non documenté (propriété en chaîne de blocs documentée) | prouvé (k-sur-n sur PC seulement) | `…/ef/REPORT.md` |
| Porte d'actionnement : habilitation par actionneur, expiration, état des capteurs, limites physiques | non documenté | prouvé | `…/ef/REPORT.md` ; `evidence/kani/2026-10-06/` |
| Preuve formelle de la règle d'actionnement | non documenté | prouvé (16 harnais vérifiés ; suite complète : CI seulement) | `evidence/kani/2026-10-06/` |
| Post-quantique standardisé | documenté (support ML-KEM, ML-DSA, Falcon) | **absent** | — |
| Enrôlement sans contact | documenté | **absent** (le firmware de test compile des graines fixes) | — |
| Transfert de propriété | documenté | **absent** | — |
| Mise à jour du firmware signée | documenté (détails non documentés) | **absent** | — |
| Équipements existants, Modbus RTU et TCP | documenté (DOME Sentry) | **absent** | — |
| Certification | ISO 26262 ASIL D (2019) | **absent** | — |
| Partenaires, clients | documentés (fabricants, distributeurs, AWS, Intel…) | **absents** | — |
| Primitives standard uniquement | non (WalnutDSA, Ironwood) + ML-DSA/ML-KEM/Falcon | oui (Ed25519, X25519, ChaCha20-Poly1305, SHA-256) | `CLAUDE.md` |
| Preuves brutes publiques | non | **non à ce jour : le dépôt est privé** (HTTP 404 anonyme, 2026-10-06) | — |

## 8. Ce que cette veille corrige dans nos propres documents

1. **WalnutDSA « retiré » du NIST** (prompt) : il a été **non retenu** au 2e tour.
2. `DIFFERENTIATORS.md` M1 dit « cassé publiquement en 2018 ». C'est exact pour les
   paramètres d'origine **et** révisés de 2018, mais il faut ajouter la réponse de
   Veridify (2019, 2020) et l'absence d'analyse tierce depuis.
3. `MARKET_VALIDATION.md` dit « modèle SaaS » : c'était vrai en 2021 (KMC) ; en 2025
   Veridify annonce **cloud ou on-prem**.
4. **Le mode passerelle Modbus de la phase 1.4 est du rattrapage**, pas un
   différenciateur : DOME Sentry protège déjà Modbus TCP et RTU. Notre seul écart
   possible serait l'autorisation **par écriture** (qui, sur quel registre, dans quel
   état), non documentée chez Veridify.
5. **Veridify a une certification** (ISO 26262 ASIL D, 2019) que nos documents
   n'indiquent pas dans « ce qu'ils font mieux ».
6. **« Preuves brutes publiques » (M4) est faux tant que le dépôt est privé.**
7. **Enrôlement** : notre firmware de test utilise des graines compilées
   (`seed_for`), ce qui est l'opposé d'une clé générée sur l'équipement.

## 9. Pour la phase 1 : un signal de faisabilité

Une étude mesure ML-KEM et ML-DSA **sur RP2040 à 125 MHz** (PQClean, C de
référence) : échange ML-KEM-512 complet en 35,7 ms, signature ML-DSA très variable
(coefficient de variation 66 à 73,5 %)
([arXiv 2603.19340](https://arxiv.org/abs/2603.19340), révisé le 2026-08-11). Les
chiffres ML-DSA-44 de vérification n'étaient pas dans le résumé : à lire avant la
spec. Nous mesurerons notre propre implémentation Rust sur nos cartes.
