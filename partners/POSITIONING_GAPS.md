# Manques du positionnement « autorité de commande » (2026-10-06)

Positionnement analysé : `partners/POSITIONING.md`. Méthode : recherche documentaire
(réglementations, normes, solutions en place, incidents), relecture du code sur la
branche `claude/eloquent-ptolemy-oojjn0`, et confrontation aux questions qu'un
acheteur posera. Sévérité : 🔴 bloque une vente ou rend une affirmation fausse ·
🟠 sera demandé tôt par un acheteur sérieux · 🟡 à prévoir.

Sources en fin de document. « Estimation » signale un calcul de ma part, non mesuré.

---

## A. Ce que les réglementations vont exiger (le vrai déclencheur d'achat en Europe)

| # | Texte | Ce qu'il exige | OASIS aujourd'hui | Sév. |
|---|---|---|---|---|
| A1 | **Règlement Machines (UE) 2023/1230**, applicable le **20 janvier 2027** | Annexe III 1.1.9 « protection contre la corruption » : les composants logiciels et matériels liés à la sécurité doivent résister à la corruption accidentelle **et intentionnelle**, et la machine doit **conserver une trace des interventions légitimes et illégitimes**. 1.2.1 : prendre en compte les « tentatives malveillantes raisonnablement prévisibles ». La machine doit pouvoir dire **quel logiciel elle exécute** | ✅ Rejette les ordres corrompus. ❌ **Aucun journal des ordres acceptés ou refusés résistant à la falsification.** ❌ Aucune identité du firmware | 🔴 |
| A2 | **Cyber Resilience Act**, signalement des vulnérabilités dès le **11 septembre 2026**, application complète le **11 décembre 2027** | Sécurité par défaut, gestion des vulnérabilités, SBOM, mises à jour de sécurité pendant la durée de support, signalement sous 24 h / 72 h | ❌ Pas de SBOM, pas de mise à jour à distance signée, `SECURITY.md` obsolète (règles bio-inspirées, `drone_bridge.exe`, pas de contact ni de délais) | 🟠 |
| A3 | **Directive RED, acte délégué**, obligatoire depuis le **1er août 2025**, normes harmonisées **EN 18031-1/2/3** | Pour tout équipement radio : authentification et contrôle d'accès, **mécanisme de mise à jour sécurisé**, protection du réseau | ✅ Authentification des messages. ❌ Mise à jour sécurisée. Tout nœud LoRa vendu dans l'UE y est soumis | 🟠 |
| A4 | **IEC 62443-4-2** (composants industriels) et **62443-4-1** (cycle de développement) | CR 1.2 identité et authentification de chaque appareil ✅ ; journal des événements auditables (CR 2.8) ❌ ; intégrité du logiciel et du démarrage (CR 3.4, EDR 3.14) ❌ ; résistance au déni de service (CR 7.1) ❌ ; mise à jour (EDR 3.10) ❌ ; processus de développement sécurisé documenté ❌ | Partiel | 🟠 |
| A5 | **IEC TS 63074:2023** (sécurité informatique des fonctions de sûreté des machines) | Fait le lien entre 62443 et la sûreté de fonctionnement. **C'est exactement le terrain d'OASIS** | Aucune correspondance documentée | 🟠 (opportunité) |
| A6 | **Feuille de route post-quantique de l'UE** : infrastructures critiques migrées **au plus tard fin 2030** | Ed25519 n'est pas post-quantique. Une signature ML-DSA-44 pèse **2 420 octets** : elle ne tient pas dans une trame LoRa de 255 octets | ❌ Pas d'agilité cryptographique (le magic `SPORE\x0B` fige l'algorithme) ; aucun plan écrit | 🟠 pour l'industriel critique, 🟡 ailleurs |
| A7 | Drones, Europe : **SORA 2.5** (EASA) | L'EASA a **retiré** les exigences de cybersécurité de JARUS, jugées disproportionnées ; l'évaluation reste une « bonne pratique » à partir de SAIL III | Faible pression réglementaire côté drone civil européen : **la conformité ne fera pas vendre ici** | 🟡 (information) |
| A8 | Drones, États-Unis : **Blue UAS / Green UAS** | Évaluation de cybersécurité (pentest) et de la chaîne d'approvisionnement des composants | Hors périmètre aujourd'hui ; possible levier si un intégrateur américain est visé | 🟡 |

**Conséquence :** pour l'industriel et les machines mobiles autonomes, le Règlement
Machines de janvier 2027 est **le** déclencheur d'achat. OASIS couvre la moitié de
la clause 1.1.9 (rejeter la corruption) et pas l'autre (la trace des interventions).
⚠️ À vérifier : les aéronefs sont en principe exclus du Règlement Machines ; les
drones relèvent d'autres textes (règlements (UE) 2019/945 et 2019/947).

## B. Ce que l'acheteur a déjà, et que le positionnement ne traite pas

| # | Solution en place | Ses limites (sourcées) | Ce qu'OASIS devrait dire et prouver | Sév. |
|---|---|---|---|---|
| B1 | **Signature MAVLink 2** (PX4, ArduPilot) | Clé **partagée** par lien, HMAC-SHA-256 tronqué à **48 bits**, **désactivée par défaut**, **déclassement vers MAVLink 1** possible, horodatage qui peut être faussé par usurpation GPS (ArduPilot #13860), fenêtre de rejeu d'une minute | Chaque point a une réponse OASIS : clé par nœud, mode strict, fraîcheur liée au `boot_id`. **Mais aucune intégration MAVLink n'est testée.** C'est le coin d'entrée le plus naturel côté drones | 🔴 (opportunité manquée) |
| B2 | **Secure boot PX4** (bootloader Ed25519) | Couvre l'intégrité du firmware au démarrage, pas l'autorité des ordres en vol | Se présenter comme **complémentaire** : PX4 garantit le firmware, OASIS l'ordre | 🟡 |
| B3 | **ROS 2 / SROS2 (DDS-Security)** | Permissions par nœud, mais **7 artefacts de sécurité par nœud**, lourd à gérer en flotte, non conçu pour les microcontrôleurs | OASIS pour les actionneurs à microcontrôleur d'une flotte ROS 2. **Aucun pont ROS 2** | 🟡 |
| B4 | Automates industriels (cas **Unitronics**, eau, 2023) | Au moins 75 automates compromis via des mots de passe par défaut et une exposition à Internet | Montre la douleur : un ordre sur une vanne n'est pas authentifié. Mais l'acheteur industriel veut un **produit certifié** (module, passerelle), pas une bibliothèque | 🟠 (forme du produit) |
| B5 | **Reticulum / microReticulum** | Déjà traité (`COMPETITIVE_ANALYSIS.md`) | Le mode « couche par-dessus un transport existant » **n'est pas testé** | 🟠 |

## C. Manques techniques que le marché fera apparaître

| # | Manque | Pourquoi c'est grave | Sév. |
|---|---|---|---|
| C1 | **Déni de service par vérification forcée.** Un attaquant extérieur, sans clé, peut fabriquer des messages qui passent tous les filtres bon marché (réseau, origine connue, compteur frais) et forcent une vérification Ed25519 de ≈ 179 ms sur le relais. Estimation : **≈ 5 faux paquets par seconde suffisent à saturer un relais Cortex-M0+**. L'attaquant ne respecte pas le duty-cycle | Les acheteurs OT et défense posent cette question en premier (IEC 62443 CR 7.1). Piste : un pré-filtre MAC à clé de réseau (quelques µs), puis Ed25519 ; et une limite de débit par origine | 🔴 |
| C2 | **Arrêt d'urgence et perte de liaison non spécifiés.** Un ingénieur sûreté exigera qu'un **ordre d'arrêt** ne soit jamais bloqué par la cryptographie (il va dans le sens sûr), et qu'une **perte de liaison** mène à un état sûr défini | Absent de `REVOCATION_AND_ACTUATION_SPEC.md`. Sans réponse, la porte est vue comme un **risque** pour la sûreté, pas comme une protection | 🔴 |
| C3 | **Budget radio.** L'en-tête v0B fait 99 octets, un ordre complet 153 octets, une révocation **234 à 242 octets** pour 1 à 2 nœuds révoqués (logs silicium) ; avec 16 nœuds, on dépasse 255 octets. En LoRaWAN EU868, la charge utile maximale est de 51 octets aux débits les plus longue portée. Estimation : une trame de 255 octets en SF12 dure **≈ 9 s**, soit environ une trame toutes les 15 min avec un duty-cycle de 1 % | La promesse « flotte entière révoquée » ne tient pas à longue portée sans fragmentation ni encodage compact | 🔴 |
| C4 | **Protocole d'émission d'un ordre.** La fraîcheur dépend de l'horloge et du `boot_id` **de l'actionneur** ; sur silicium, le PC lisait l'horloge de C avant chaque ordre. En opération, comment le commandant connaît-il ces valeurs (balise, défi-réponse) ? | Non spécifié : la garantie de fraîcheur est démontrée en labo, pas utilisable telle quelle sur le terrain | 🟠 |
| C5 | **Journal infalsifiable** des ordres exécutés et refusés (chaîne de hachages, signée, persistée) | Exigé par le Règlement Machines (A1) et IEC 62443 (CR 2.8). Les compteurs de rejets actuels sont en RAM | 🔴 |
| C6 | **Cycle de vie des clés** : injection en fabrication, enrôlement, rotation, stockage de la clé opérateur (HSM), récupération | C'est la première question d'un RSSI. Rien n'est écrit | 🔴 |
| C7 | **Mise à jour signée et identité du firmware** | A1, A2, A3 et A4 l'exigent. La protection RP2350 a été contournée par des attaques physiques (challenge public, 4 lauréats), mais elle exige un accès physique et un labo | 🟠 |
| C8 | **« R14 / entropie »** : vocabulaire bio-inspiré, non reconnu en sûreté de fonctionnement | Un ingénieur sûreté rejettera « porte d'entropie » s'il croit qu'on revendique une fonction de sûreté (SIL ou PL). Il faut dire « verrou d'état des capteurs », **pas une fonction de sûreté certifiée** ; la sûreté reste dans le système de commande certifié (ISO 13849, IEC 62061) | 🟠 |
| C9 | **Ordres critiques à deux signatures** (règle des deux personnes) et ordres de groupe | Le k-sur-n n'existe que pour la révocation. Pour ouvrir une vanne critique ou armer une flotte, c'est un argument fort, et le code existe déjà (`oasis-operator-key`) | 🟡 (opportunité) |
| C10 | **Assurance logicielle** : pas de fuzzing des parseurs, pas de `cargo-audit`, `cargo-deny` ni SBOM, suite Kani complète jamais exécutée | Attendu par CRA et 62443-4-1 ; peu coûteux | 🟠 |
| C11 | Routage (inondation), radio réelle, énergie | Connus, déjà listés | 🟠 |

## D. Manques côté marché et entreprise

| # | Manque | Sév. |
|---|---|---|
| D1 | **Aucune validation client.** Zéro entretien. Les 4 segments du positionnement sont des hypothèses | 🔴 |
| D2 | **Trop de segments.** Drones, robots agricoles, actionneurs industriels et infrastructures n'ont ni les mêmes normes, ni les mêmes acheteurs, ni le même déclencheur. Il faut **un segment de tête** | 🔴 |
| D3 | **Forme du produit** : bibliothèque Rust, module matériel, passerelle placée devant un actionneur existant, ou firmware de référence ? L'industriel achète un composant certifiable, l'intégrateur drone une bibliothèque | 🟠 |
| D4 | **Licence MIT** : n'importe qui peut l'utiliser sans payer. Le CRA crée des obligations pour les « stewards » open source (signalement à partir du 11 décembre 2027) | 🟠 |
| D5 | **Une seule personne** : un acheteur industriel demandera qui assure le support et les correctifs pendant 5 à 10 ans | 🟠 |
| D6 | **Financement non ciblé** : le Fonds européen de la défense (programme 2026 avec des lignes drones et commande et contrôle) et le projet DECODER (3,5 à 5 Md€) passent par des **consortiums** ; il faut un partenaire industriel | 🟡 (opportunité) |
| D7 | Contrôle des exportations (cryptographie et drones, biens à double usage) | À vérifier avant tout prospect hors UE | 🟡 |

## E. Ce que je recommande de corriger en premier

1. **Choisir le segment de tête (D2)**. Deux candidats solides :
   - **flottes de drones PX4** : coin d'entrée technique (B1), déjà en simulation SITL, mais faible pression réglementaire en Europe (A7) ;
   - **machines mobiles autonomes et actionneurs industriels** : échéance du Règlement Machines en **janvier 2027** (A1), donc un déclencheur d'achat daté, mais un acheteur qui exigera certification et support.
2. **Fermer les 🔴 techniques avant de parler à un client** : C1 (déni de service), C2 (arrêt d'urgence et perte de liaison), C3 (budget radio), C5 (journal), C6 (cycle de vie des clés).
3. **Écrire les tableaux de correspondance** avec le Règlement Machines 1.1.9, IEC 62443-4-2, IEC TS 63074 et EN 18031 : c'est ce qui transforme « sécurisé » en « aide à obtenir le marquage CE ».
4. **Faire 10 entretiens** avant d'écrire plus de code de positionnement (D1).

---

## Sources

- Règlement Machines : [ABB — EU Machinery Regulation 2027](https://www.abb.com/global/en/areas/motion/drives/expertise-technology/eu-machinery-regulation-2027) ; [Checkmate Experts — Cybersecurity becomes a CE requirement](https://www.checkmate.expert/en/maschinenverordnung-cybersicherheit.html) ; [SOPX — OEM checklist for 2027](https://sopx.io/insights/eu-machinery-regulation-2023-1230/)
- Cyber Resilience Act : [Commission européenne — CRA reporting](https://digital-strategy.ec.europa.eu/en/policies/cra-reporting) ; [Freshfields — reporting from 11 September 2026](https://www.freshfields.com/en/our-thinking/blogs/technology-quotient/cyber-resilience-act-reporting-obligations-take-effect-on-11-september-2026-102nzmk) ; [ORC WG — CRA](https://orcwg.org/cra/)
- RED / EN 18031 : [Nemko — EN 18031 harmonised](https://www.nemko.com/blog/cybersecurity-in-europe-en-18031-is-now-a-harmonized-standard) ; [Schutzwerk — RED cybersecurity](https://www.schutzwerk.com/en/compliance/red/)
- IEC 62443-4-2 : [ISASecure — SL2 as a minimum](https://www.isasecure.org/hubfs/The-Case-for-ISA-IEC-62443-Security-Level-2-as-a-Minimum-FINAL.pdf) ; [Promwad — IEC 62443 in practice](https://promwad.com/news/iec-62443-practice-ot-it-convergence-embedded-security-architecture)
- IEC TS 63074 : [AFNOR — IEC TS 63074:2023](https://www.boutique.afnor.org/en-gb/standard/iec-ts-630742023/safety-of-machinery-security-aspects-related-to-functional-safety-of-safety/xs142937/342154) ; [CENELEC CLC IEC/TS 63074:2024](https://standards.iteh.ai/catalog/standards/clc/e50d6c7d-e5d6-419b-9e02-86ef6c183a14/clc-iec-ts-63074-2024)
- Post-quantique : [Commission européenne — Post-Quantum Cryptography](https://digital-strategy.ec.europa.eu/en/policies/post-quantum-cryptography) ; [Industrial Cyber — EU PQC 2030](https://industrialcyber.co/regulation-standards-and-compliance/eu-begins-coordinated-effort-for-member-states-to-switch-critical-infrastructure-to-quantum-resistant-encryption-by-2030/)
- SORA 2.5 : [Blakistons — EASA's adoption of SORA 2.5](https://blakistons.co.uk/what-the-uk-drone-industry-can-learn-from-easas-adoption-of-sora-2-5/)
- Blue / Green UAS : [AUVSI — Green UAS](https://www.auvsi.org/certification-training/green-uas/) ; [DIU — Blue UAS](https://www.diu.mil/latest/blue-uas-refresh-list-and-framework-platforms-and-capabilities-selected)
- MAVLink : [MAVLink — Message signing](https://mavlink.io/en/guide/message_signing.html) ; [PX4 — Message signing](https://docs.px4.io/main/en/mavlink/message_signing) ; [ArduPilot #13860 — signing broken under GPS spoofing](https://github.com/ArduPilot/ardupilot/issues/13860) ; [DEV — why MAVLink isn't secure by default](https://dev.to/oliopti/mavlink-the-protocol-behind-millions-of-drones-and-why-it-isnt-secure-by-default-3n8m)
- PX4 secure boot : [PX4 — Bootloader secure boot](https://docs.px4.io/main/en/advanced_config/bootloader_secure_boot)
- ROS 2 : [SROS2 paper (arXiv)](https://arxiv.org/pdf/2208.02615) ; [ROS 2 DDS-Security integration](https://design.ros2.org/articles/ros2_dds_security.html)
- Unitronics : [CISA alert](https://www.cisa.gov/news-events/alerts/2023/11/28/exploitation-unitronics-plcs-used-water-and-wastewater-systems)
- RP2350 : [Raspberry Pi — Hacking Challenge results](https://www.raspberrypi.com/news/security-through-transparency-rp2350-hacking-challenge-results-are-in/) ; [USENIX WOOT'25](https://www.usenix.org/system/files/woot25-muench.pdf)
- Défense européenne : [EU Council — DECODER factsheet](https://defence-industry-space.ec.europa.eu/system/files/2026-07/Factsheet-DronE-Counter-Drone-European-Resolve.pdf) ; [Dronehub — EDF 2026 calls](https://dronehub.ai/blog/european-defence-fund-2026-cuas-calls)
- Code OASIS : `oasis-rt/src/mesh.rs` (ordre des vérifications v0B), `oasis-rt/src/mesh_revocation.rs` (`MAX_REVOKED = 16`), `oasis-lora-transport/src/lib.rs` (`max_payload() = 255`), `evidence/silicon/2026-10-06/ef/61_rev_e1_A.log` (`len=234`), `SECURITY.md`.
