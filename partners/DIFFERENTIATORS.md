# Ce qu'OASIS a de plus que ses concurrents (2026-10-07)

Comparaison de propriétés **démontrées** côté OASIS (logs silicium, tests, Kani)
avec ce que chaque concurrent **documente publiquement**. « Non documenté » ne veut
pas dire « absent » : un produit fermé peut le faire sans le publier. OASIS est à
TRL 4 ; ses concurrents commerciaux sont déployés.

## 1. Concurrents retenus

| Concurrent | Ce que c'est | Où il tourne |
|---|---|---|
| **Veridify DOME** | Authentification mutuelle d'équipements OT, chiffrement par paquet, enrôlement sans contact, propriété en chaîne de blocs, mises à jour ; signatures WalnutDSA, plus ML-DSA, ML-KEM et Falcon | Microcontrôleurs STM32, Renesas RA, RISC-V ; vendu via des fabricants (KMC Controls) |
| **Xage Security** | Zero trust pour l'OT : points d'application (XEP) placés **devant** les équipements, synchronisés en chaîne de blocs | Passerelles et serveurs ; énergie, US Space Force |
| **Mobilicom ICE** | Cybersécurité « 360° » pour drones : détection d'anomalies par IA, chiffrement multicouche, évitement d'interférences | Sur leurs liaisons de données (SkyHopper) ; Blue UAS |
| **Radios MANET** (Silvus, Doodle Labs) | Chiffrement AES-256 du lien, FIPS 140-3 | Dans la radio |
| **seL4** (Kry10, DornerWorks) | Micronoyau formellement vérifié, isolation | Cortex-A, RISC-V ; **pas de Cortex-M** |
| **Bluetooth Mesh, Thread** | Mesh avec clé réseau partagée, vérifiée à chaque saut | Microcontrôleurs |
| **Reticulum, microReticulum** | Mesh chiffré, anonyme, routé ; les relais ne vérifient pas les données | Linux ; ESP32, nRF52 |
| **Signature MAVLink 2, SROS2** | Authentification de lien (clé partagée) ; permissions par nœud ROS 2 | Autopilote ; Linux |

## 2. Ce qu'OASIS est seul à faire (démontré)

| # | Propriété | Pourquoi personne d'autre ne l'a | Preuve OASIS |
|---|---|---|---|
| U1 | **Chaque relais d'un mesh multi-saut vérifie l'origine et le contenu avec la clé propre de l'émetteur** | Bluetooth Mesh et Thread vérifient à chaque saut, mais avec une clé **partagée** (un nœud capturé peut tout forger). Reticulum et microReticulum ne vérifient pas en transit. Veridify authentifie d'équipement à équipement, sans relais multi-saut documenté. Xage filtre dans une passerelle, pas dans chaque nœud | 0/150 inversions de bit acceptées au relais, contenu remplacé rejeté, nœud forgé rejeté (`evidence/silicon/2026-10-06/`) |
| U2 | **Une porte d'actionnement qui combine sécurité et état de la machine** : ordre vérifié, origine habilitée **pour cet actionneur**, non révoquée, non expirée **dans l'horloge de l'actionneur**, verrou d'état des capteurs, limites physiques, non déjà exécuté | Veridify et Mobilicom bloquent les ordres non autorisés, mais aucun ne documente un verrou lié à l'état des capteurs ni aux limites physiques. Les fonctions de sûreté certifiées (ISO 13849) ne vérifient pas l'origine cryptographique d'un ordre | Silicium : ordres non habilité, expiré, rejoué, capteur perdu, 1000 N et NaN refusés ; broche lue à 0 |
| U3 | **Cette règle d'actionnement est prouvée formellement** : « agir » seulement si les 7 conditions sont vraies ; jamais si l'état des capteurs est défavorable | Aucun concurrent cité ne publie de preuve formelle de son logiciel embarqué. seL4 prouve un noyau, pas une règle d'actionnement, et ne tourne pas sur Cortex-M | 3 harnais Kani vérifiés (`evidence/kani/2026-10-06/`) |
| U4 | **Fraîcheur liée au démarrage de l'actionneur**, sans horloge partagée ni GPS | La signature MAVLink dépend d'un horodatage qui peut suivre le GPS : un leurre GPS ouvre le rejeu (ArduPilot #13860). Les autres ne documentent pas leur mécanisme | Ordre d'un ancien `boot_id` refusé sur silicium |

## 3. Ce qu'OASIS fait mieux, sans être seul

| # | Propriété | Concurrents proches | Avantage OASIS |
|---|---|---|---|
| M1 | **Primitives standard uniquement** (Ed25519, X25519, ChaCha20-Poly1305, SHA-256) | Veridify utilise WalnutDSA, un algorithme propriétaire **cassé publiquement en 2018** (falsification universelle en environ deux minutes, puis attaque de la version corrigée) ; il propose aussi ML-DSA | Aucun algorithme maison : c'est ce qu'un auditeur vérifie en premier |
| M2 | **Révocation signée par l'opérateur, k-sur-n possible, propagée et appliquée par chaque relais, sans renouveler les autres clés** | Bluetooth Mesh et Thread : renouvellement de clé pour tous. Reticulum : listes « blackhole » locales. Veridify, Xage : registre de propriété ou de politique en chaîne de blocs | Révoquer un nœud ne touche pas les autres ; k-sur-n prouvé sur PC seulement |
| M3 | **Code sûr en mémoire et vérifié** : Rust, 608 tests, 130 des 152 harnais Kani vérifiés (aucun contre-exemple), 1,5 × 10⁹ exécutions de fuzzing, SBOM et 0 vulnérabilité connue | Les autres : C/C++ ou fermé | Moins de classes de failles mémoire ; preuves publiées |
| M4 | **Preuves brutes publiques** : logs, firmware, SHA-256 vérifiés dans un clone frais, défauts publiés | Produits fermés | Confiance vérifiable par un tiers ; aussi une limite pour la monétisation |
| M5 | **Tourne sur Cortex-M0+ sans FPU ni accélérateur crypto** | Veridify revendique aussi les petits STM32 sans accélérateur | **Égalité probable** avec Veridify ; avantage sur seL4, Xage, Mobilicom |
| M6 | **Résistance mesurée au déni de service par vérification forcée** : un pré-filtre à clé de lien ramène le coût d'une trame forgée de 179,4 ms à 0,70 ms sur RP2040, et sous un flot de 8 trames/s le relais livre 60 messages légitimes sur 60 à 19 % de charge (contre 39/60 à 110 % sans lui) | Personne parmi les concurrents cités ne documente ce risque ni sa parade. Bluetooth Mesh et Thread filtrent avec une clé **partagée** ; nos clés de lien sont dérivées des identités et ne sont jamais distribuées | Le chiffre est mesuré, pas estimé (`evidence/silicon/2026-10-07/prefilter/`). ⚠️ Contre un **initié**, le budget borne le calcul mais affame le trafic légitime (4/60) : plafond de calcul, pas équité |

## 4. Là où OASIS est derrière

| Concurrent | Ce qu'il a de plus |
|---|---|
| Veridify | **ML-KEM et Falcon** (OASIS n'a aucun KEM post-quantique : son échange de clés reste X25519) ; **couverture de protocoles** bien plus large (BACnet/IP et MS/TP, DNP3, EtherNet/IP, MQTT, Modbus TCP et RTU en 1:N, contre Modbus RTU FC06/FC16 sur un seul équipement) ; **provisionnement industrialisé** (sans contact à l'échelle, gestion cloud ou sur site, application mobile) ; **certification ISO 26262 ASIL D** (2019) ; clients et fabricants partenaires. ✔️ Rattrapés depuis le 2026-10-07 : signature hybride Ed25519 + ML-DSA-44, enrôlement avec clé générée sur la carte, transfert de propriété à deux signatures, mise à jour A/B signée avec retour automatique (voir `OASIS_VS_VERIDIFY.md`) |
| Silvus, Doodle Labs | Radios réelles, **certification FIPS 140-3**, contrats défense |
| Mobilicom | Détection d'anomalies, résistance au brouillage, Blue UAS |
| Xage | Clients grands comptes, déploiement à l'échelle de l'entreprise |
| Reticulum, microReticulum | Radio LoRa, routage, écosystème, anonymat |
| Tous | Maturité (TRL 6 à 9 contre TRL 4), support, équipe |

## 5. La phrase qui résume l'avantage

> **OASIS est le seul à vérifier, à chaque relais et dans chaque machine, qu'un ordre
> vient d'un nœud habilité, n'a pas été modifié, n'est ni périmé ni révoqué, et que la
> machine est en état de l'exécuter. Cette règle est prouvée formellement et tourne
> sur un microcontrôleur à 1 $.**

Chacun des concurrents fait une partie de cette chaîne : le lien (radios), le réseau
(Xage), l'équipement (Veridify), le noyau (seL4). **Aucun ne la fait de bout en bout
jusqu'à l'actionneur.** C'est l'avantage à défendre.

Le post-quantique (en hybride), l'enrôlement, le transfert de propriété et la mise à
jour signée ont été rattrapés et démontrés sur silicium le 2026-10-07. Restent à
rattraper ou à obtenir par partenariat : **un KEM post-quantique**, la **largeur de
protocoles**, la **radio**, une **certification** et, avant tout, des **clients** —
zéro entretien à ce jour (`POSITIONING_GAPS.md` D1).

## Sources

- Veridify : [DOME](https://www.veridify.com/dome/) ; [Quantum-resistant IoT security](https://www.veridify.com/quantum-resistant-iot-security/) ; [Renesas — DOME](https://www.renesas.com/en/document/prb/veridifys-dome) ; [ST Blog](https://blog.st.com/veridify/)
- WalnutDSA cassé : [Hart et al., A Practical Cryptanalysis of WalnutDSA (PKC 2018)](https://link.springer.com/chapter/10.1007/978-3-319-76578-5_13) ; [NIST — commentaire officiel de Beullens](https://csrc.nist.gov/CSRC/media/Projects/Post-Quantum-Cryptography/documents/round-1/official-comments/WalnutDSA-official-comment.pdf) ; [The Cracking of WalnutDSA: A Survey (MDPI)](https://mdpi.com/2073-8994/11/9/1072/htm)
- Xage : [Zero Trust Access](https://xage.com/zero-trust-access/) ; [SecurityWeek — blockchain for IIoT](https://www.securityweek.com/new-blockchain-solution-iiot-aims-solve-scaling-problem/)
- Mobilicom : [ICE Suite](https://mobilicom.com/products/ice-suite/) ; [Secured Communication](https://mobilicom.com/secured-communication/)
- Radios : [Silvus FIPS 140-3](https://silvustechnologies.com/blog/silvus-streamcaster-leading-the-way-the-industrys-first-manet-radio-to-receive-fips-140-3-level-2-validation/) ; [Doodle Labs](https://www.commercialuavnews.com/security/from-blue-to-you-doodle-labs-brings-blue-framework-smart-radios-to-the-commercial-sector)
- seL4 : [seL4 in use](https://sel4.systems/use.html) ; [Microkit supported platforms](https://docs.sel4.systems/projects/microkit/platforms.html)
- MAVLink : [ArduPilot #13860](https://github.com/ArduPilot/ardupilot/issues/13860) ; [MAVLink — Message signing](https://mavlink.io/en/guide/message_signing.html)
- Reticulum, Bluetooth Mesh, Thread : voir `COMPETITIVE_ANALYSIS.md`
