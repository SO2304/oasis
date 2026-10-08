# Manques du positionnement « autorité de commande » (2026-10-06)

Positionnement analysé : `partners/POSITIONING.md`. Méthode : recherche documentaire
(réglementations, normes, solutions en place, incidents), relecture du code sur la
branche `claude/eloquent-ptolemy-oojjn0`, et confrontation aux questions qu'un
acheteur posera. Sévérité : 🔴 bloque une vente ou rend une affirmation fausse ·
🟠 sera demandé tôt par un acheteur sérieux · 🟡 à prévoir.

Sources en fin de document. « Estimation » signale un calcul de ma part, non mesuré.

> **Révision du 2026-10-07 (phase 0 de `prompts/POSITIONING_ALIGNMENT.md`).** Les sections A et
> B ont été vérifiées **à la source** : les textes officiels sont désormais dans
> [`docs/compliance/`](../docs/compliance/), un fichier par texte, avec les verbatim, les liens
> EUR-Lex et un tableau exigence → couverture OASIS → preuve → manque. La **section F**, en fin
> de document, porte les **corrections** apportées aux lignes existantes et les **manques
> nouveaux** trouvés pendant cette vérification. Les numéros d'origine (A1…D7) n'ont pas été
> renumérotés, pour que les renvois existants restent valides.

> **Révision du 2026-10-08 (phase 5).** La **section G** porte le **statut de chaque
> manque** — fermé avec preuve, partiellement fermé, ouvert — avec le chemin de la preuve.
> C'est la source de vérité : une sévérité 🔴 ci-dessous peut être fermée en G.

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

## F. Phase 0 — corrections et manques nouveaux (2026-10-07)

Vérification à la source de chaque ligne des sections A et B. Détail et verbatim dans
[`docs/compliance/`](../docs/compliance/).

### F.1 Corrections aux lignes existantes

| Ligne | Ce qui était écrit | Ce que dit le texte | Conséquence |
|---|---|---|---|
| **A1** | « les aéronefs sont en principe exclus du Règlement Machines » | L'article 2(2)(e) exclut les « moyens de transport par air » **« à l'exception des machines montées sur ces moyens de transport »** ; l'article 2(2)(f) n'exclut un produit aéronautique que s'il relève de 2018/1139 **et** de la définition de machine, **« dans la mesure où »** 2018/1139 couvre les EESS pertinentes. Le règlement ne contient **ni « drone » ni « aéronef sans équipage »** | Exclusion **conditionnelle et partielle**, à qualifier au cas par cas. Une machine montée sur un drone n'est pas exclue. ⚠️ Aucun guide de la Commission trouvé : **non vérifié à la source** |
| **A1** | — | **Aucune norme harmonisée n'est citée au JO** au titre du règlement (UE) 2023/1230 au 2026-10-07 | 1.1.9 et 1.2.1 se démontrent **sans présomption de conformité** |
| **A1** | — | Les dates du PDF du JO L 165 (« 14 janvier 2027 ») ont été remplacées par un **rectificatif (JO L 169 du 4.7.2023, p. 35)**. Date correcte : **20 janvier 2027** | Citer le **consolidé** `02023R1230-20260727`, jamais le JO L 165 |
| **A3** | « obligatoire depuis le 1er août 2025 » | Exact (report par le règlement délégué (UE) 2023/2444). **Mais** le règlement délégué **(UE) 2026/339** abroge 2022/30 **avec effet au 11 décembre 2027** | Fenêtre RED utile : 01/08/2025 → 10/12/2027. OASIS (TRL 4) ne mettra aucun équipement sur le marché dans cette fenêtre : **le travail utile est le CRA, pas la RED**. Le considérant 3 de 2026/339 dit que l'annexe I du CRA « comprend **tous les éléments** » de RED 3.3 d), e), f) |
| **A6** | « ❌ Pas d'agilité cryptographique (le magic `SPORE\x0B` fige l'algorithme) » | Vrai de `SPORE\x0B`, **faux des messages d'autorité** : octet de suite **signé** dans `OAU1`, suite minimale par type jamais abaissée et persistée, déclassement refusé avant toute vérification | A6 est **partiellement fermé** depuis la phase 1.1. Reste ouvert pour la couche par saut |
| **A6** | « ML-DSA-44 pèse 2 420 octets » | **Exact**, vérifié dans FIPS 204 Table 2 | — |
| **A7** | « faible pression réglementaire côté drone civil européen : la conformité ne fera pas vendre ici » | Confirmé pour SORA (ED Decision **2025/018/R** du 29/09/2025 : exigences de cybersécurité **retirées**, « deemed insufficiently proportionate »). **Mais** le règlement (UE) 2019/945 **impose** aux classes **C2** (partie 3, point 8) et **C3** (partie 4, point 12) — héritées par C5 et C6 — « un système de liaison de commande et contrôle **protégé contre les accès non autorisés aux fonctions de commande et de contrôle** ». C0, C1 et C4 : rien | Nuance importante : il **existe** une exigence, purement téléologique (aucune occurrence de « authentification », « signature », « chiffrement » ou « cryptographie »). **OASIS est un moyen de la satisfaire** pour C2/C3/C5/C6 |
| **B1** | « HMAC-SHA-256 tronqué à 48 bits » | Ce **n'est pas un HMAC** : `sha256_48(secret_key + header + payload + CRC + link-ID + timestamp)`, construction **à clé en préfixe**, confirmée par le code de référence | Dire « 48 bits de signature, et ce n'est pas un HMAC ». ⚠️ **Aucune attaque n'est revendiquée**, aucun audit public trouvé |
| **B1** | « fenêtre de rejeu d'une minute » | La règle réelle est la **monotonie stricte par flux** `(SystemID, ComponentID, LinkID)`. La minute ne s'applique **qu'à l'admission d'un flux inconnu**. **Aucune tolérance au réordonnancement** | Avantage OASIS plus net que prévu : fenêtre glissante de 128 bits, 127 positions de réordonnancement |
| **C3** | « estimation : 255 octets en SF12 ≈ 9 s, une trame toutes les 15 min » | **9,019 s confirmé** [calcul] ; le duty-cycle de 1 % donne **3 trames/heure**, soit une toutes les 20 min | Estimation juste à 25 % près. ⚠️ La formule de time on air est **non vérifiée à la source** (datasheet SX1276 inaccessible) |
| **D4** | « Le CRA crée des obligations pour les “stewards” open source » | **Juridiquement impossible ici** : l'article 3, point 14 exige une « **personne morale** ». Une personne physique ne peut pas être intendant. Et un projet MIT non monétisé est **hors du champ** (considérants 18 et 20) | D4 est **fermé en l'état**, et **rouvert au premier euro facturé** : la monétisation déclenche l'annexe I entière, 5 ans de support et 10 ans de disponibilité des mises à jour |
| **D7** | « à vérifier avant tout prospect hors UE » | Le règlement (UE) 2021/821 ne contrôle en 5A002.a que la « cryptographie pour la **confidentialité** des données », et sa note technique 1 exclut authentification, signature, intégrité et non-répudiation. **Mais** l'exclusion suppose que la fonction ne soit pas « utilisable ou rendue utilisable » pour la confidentialité — **or `oasis-rt` embarque ChaCha20-Poly1305 et X25519 pour la confidentialité** (couche spore v3→v7) | **Sévérité relevée** : l'exemption « authentification seule » **ne s'applique probablement pas** à `oasis-rt` tel quel. Voir F.2 → **D8** |

### F.2 Manques nouveaux

| # | Manque | Pourquoi | Sév. |
|---|---|---|---|
| **A9** | **Voie de présomption de conformité par certification de cybersécurité.** L'article 20(9) du Règlement Machines prévoit qu'un certificat délivré sous un schéma adopté au titre du règlement (UE) 2019/881 vaut présomption de conformité **nommément pour les sections 1.1.9 et 1.2.1** | C'est **la seule voie réglementaire** qui transformerait OASIS en « aide au marquage CE », et elle passe par la famille **EUCC**, pas par IEC 62443. ⚠️ **Non vérifié à la source** : activation effective pour les machines | 🟠 (opportunité) |
| **A10** | **ISO 10218-1:2025 et -2:2025** (publiées le 05/02/2025) ajoutent une **cybersécurité normative** (clauses 5.1.16 / 5.2.16) sous forme d'**évaluation documentée des menaces**, limitée à « the extent that it applies to industrial robot safety ». IEC 62443, IEC/TR 63074 et ISO/TR 22100-4 restent en **bibliographie seulement** | Un deuxième segment (robots industriels) acquiert une exigence de cybersécurité normative, et elle est **documentaire** — donc satisfiable par un dossier, ce qu'OASIS produit déjà. ⚠️ **Source unique, rien vérifié à la source** : à recouper avant tout usage commercial | 🟡 (opportunité) |
| **A11** | **NIS2 : un fabricant de machines est dans le champ.** Annexe II, point 5 d), NACE Rév. 2 section C division 28 ⇒ **entité importante** (art. 3(2)) dès la taille moyenne. L'article 21(2) d) impose la sécurité de la chaîne d'approvisionnement des **fournisseurs directs**, et l'article 21(3) vise « la qualité globale des produits et des pratiques de cybersécurité […] y compris de leurs **procédures de développement sécurisé** » | La répercussion sur OASIS est **contractuelle, pas réglementaire** : aucune obligation directe sur un éditeur de composant. Mais c'est un **deuxième générateur de demande** pour le dossier de preuves, après l'article 13 §5 du CRA. Un **opérateur de flotte de robots n'est pas un type listé** | 🟡 |
| **A12** | **Risque de classification « auto-évolutif à apprentissage automatique ».** L'annexe I, partie A, points 5 et 6 du Règlement Machines soumettent à **organisme notifié** les composants de sécurité et systèmes intégrés « au comportement totalement ou partiellement auto-évolutif **utilisant des approches d'apprentissage automatique** assurant des fonctions de sécurité ». Le considérant 55 borne : exclus, les « logiciels incapables d'apprendre ou d'évoluer ». Vérifié dans le code : le chemin d'actionnement n'utilise qu'un **seuil fixe** (`is_action_safe` = `agent.entropy < threshold`) sur une statistique glissante — **aucun apprentissage**. Mais `oasis-rt` embarque M6 (morphogenèse), M7 (Hebbian/STDP) et M8 (consolidation) | **Obligation de cadrage, pas défaut actuel.** Câbler un mécanisme adaptatif dans le chemin d'actionnement ferait basculer OASIS vers l'organisme notifié. À écrire comme règle d'architecture, et à dire à l'acheteur | 🟠 |
| **B6** | **ISO 3691-4:2023 exclut les chariots télécommandés de son périmètre** — verbatim vérifié : « not applicable to […] **remotely-controlled trucks, which are not considered to be driverless trucks** ». Les AMR y sont nommément inclus. La norme fait reposer la sécurité sur des moyens **locaux** (détection de personnes + freinage, PLr d), **pas** sur la fiabilité d'un lien | Plafond du discours « sécuriser la liaison » : la norme AGV/AMR **ne compte pas sur le lien**. L'argument OASIS doit porter sur l'**autorité de l'ordre**, pas sur la sûreté du lien. ⚠️ **Résultat négatif à connaître** : il n'existe **aucune clause « 4.18 Loss of communication »** dans ISO 3691-4:2023 (l'article 4 s'arrête à 4.14) — plusieurs sources tierces l'affirment, **à ne pas citer** | 🟠 |
| **B7** | **U-space : l'exigence d'authentification existe, mais pas chez l'équipementier.** Règlement (UE) 2021/664, annexe III partie A point 4 : « processus d'authentification approprié permettant aux destinataires de confirmer que les données […] ont été transmises par une **source autorisée** » ; partie B (chiffrement, protection des protocoles) ; annexe V points 3-4 (« méthode de cryptage reconnue ») | C'est l'exigence d'authentification d'origine la plus explicite du corpus drone, et elle pèse sur le **prestataire de service U-space**. Donc un acheteur possible — mais ce n'est **pas** un nœud embarqué : c'est du logiciel de service | 🟡 (opportunité, autre produit) |
| **C12** | **Vivacité du lien de supervision.** Annexe III, partie 3 du Règlement Machines, verbatim : « **Si la fonction de supervision n'est pas active, la machine ne peut pas fonctionner.** » Rien, dans OASIS, ne subordonne l'autorisation d'agir à la **présence** d'un lien de supervision | Distinct de C2 (état sûr sur perte de liaison) et de C4 (fraîcheur) : c'est une condition de **présence**, une balise signée périodique dont l'absence **retire** l'autorisation. Exigé par le texte pour les machines mobiles autonomes | 🔴 |
| **C13** | **v0B et v0C n'assurent pas la confidentialité.** La charge utile d'un ordre mesh circule **en clair** ; AEAD et ECDH existent à la couche spore (v3→v7), pas sur le mesh | L'annexe I-I-2 e) du CRA demande la confidentialité « le cas échéant » ; EN 18031-1 a un mécanisme SCM. À **dire explicitement** à tout acheteur : si la confidentialité est requise sur le mesh, c'est un développement, pas une option | 🟠 |
| **C14** | **Stockage sécurisé et effacement des secrets.** Clé privée lisible en flash (BOOTSEL, SWD) ; aucune procédure d'effacement | Touche quatre exigences d'un coup : EN 18031-1 **SSM**, IEC 62443-4-2 **CR 1.9 RE(1)**, **CR 4.2** et **EDR 3.11/3.14**, et CRA annexe I-I-2 m). **Ne se ferme pas en logiciel** : oriente D3 vers la forme « module matériel » | 🔴 pour l'industriel |
| **D8** | **Contrôle des exportations : sévérité relevée.** Deux angles morts relevés dans le règlement (UE) 2021/821 (annexe I telle que remplacée par le règlement délégué **(UE) 2025/2003**) : (i) l'exclusion de la note technique 1 de 5A002.a suppose que la fonction ne soit pas « utilisable ou rendue utilisable » pour la confidentialité — **or ChaCha20-Poly1305 et X25519 sont présents** ; (ii) **5D002.a.1** vise les logiciels « spécialement conçus […] pour le développement, la production ou l'**utilisation** » d'équipements de 5A002, branche **indépendante** de la nature cryptographique du logiciel | **Un avis juridique est nécessaire avant tout prospect hors UE**, et avant toute diffusion de binaires. ⚠️ **Gaps majeurs** : les chapitres **9D et 9E** (logiciels et technologie pour UAV) **n'ont pas été recherchés** — c'est la lacune la plus critique pour un fournisseur de logiciel ; les articles 1-4 et 9-12 du règlement (clauses « attrape-tout », transferts intangibles) non lus. Observation textuelle non corroborée : les notes « n'exempte pas les logiciels de la catégorie 5, partie 2 » sont attachées aux points a) et c) de la note générale sur les logiciels, **pas au point b)** (« domaine public ») | 🟠 |
| **D9** | **Durée de support contractuelle.** Le CRA impose, dès monétisation, **5 ans** de période d'assistance (art. 13 §8 al. 3), **10 ans** de disponibilité des mises à jour publiées (art. 13 §9) et l'affichage de la date de fin à l'achat (art. 13 §19) | Transforme D5 (« une seule personne ») en obligation chiffrée. **Ce point conditionne le modèle économique plus que n'importe quelle exigence technique** | 🔴 |

### F.3 Textes vérifiés comme non applicables ou sans exigence utile

Consigné pour ne pas y revenir :

- **ISO 13849-1:2023 exclut la cybersécurité de son périmètre**, verbatim via l'IFA/DGUV : « **IT
  security aspects are not covered by the standard**, but the authors point out that the such
  aspects **can also impact safety functions**. For further details, reference is given to
  ISO/TR 22100-2 and IEC/TR 63074. » C'est **l'appui le plus net** pour refuser l'équivalence
  sécurité / sûreté — et le renvoi désigne exactement le terrain d'OASIS.
- **Règle de bascule, ISO 13849-1:2023**, verbatim via l'IFA/DGUV : « **If operating mode
  selection enables or disables safety functions, it is treated as a safety function in its own
  right.** » Il n'existe **pas** de catégorie « composant d'autorisation non lié à la sécurité » :
  c'est la contrainte d'architecture qui rend **C2 bloquant**. Voir
  [`docs/compliance/IEC_TS_63074.md`](../docs/compliance/IEC_TS_63074.md).
- **ISO 13850:2015** (référence normative d'ISO 3691-4:2023), clauses **4.1.1.2** et **4.1.1.4** :
  l'arrêt d'urgence « shall override all other functions and operations in all operating modes »
  et « shall not impair the effectiveness of other safety functions ». L'arrêt d'urgence ne doit
  **pas** passer par OASIS.
- **IEC 62443-4-2 : une certification n'est pas accessible aujourd'hui.** Le document officiel du
  schéma (CSA-100 v4.3, §4.1) exige que « a supplier must hold an ISASecure **SDLA** development
  process certification » — donc une entreprise. Et **viser SL 1 plutôt que SL 2 n'économise
  rien** sur le processus (seul FSA-C varie avec le SL). Formule à employer : « conçu selon
  IEC 62443-4-2, **SL 2 en capacité visé, non certifié** ».
- **SORA 2.5** : exigences de cybersécurité **retirées** de la version EASA, pas rendues
  optionnelles. ED Decision 2025/018/R, note explicative : « *The detailed requirements on
  cybersecurity that were included in JARUS SORA 2.5 have been removed from the EASA version
  […] deemed insufficiently proportionate.* » Seule une évaluation de vulnérabilité est
  « particularly recommended » au-dessus de SAIL III — non contraignante.
- **Catégorie 9 du double usage** : 9A012.b ne liste **que** les kits de conversion d'aéronefs
  habités et les moteurs pour vol > 15 240 m — **aucun logiciel, calculateur de vol ni liaison de
  données**. ⚠️ Mais 9D et 9E n'ont pas été recherchés (voir D8).

---

---

## G. Statut de chaque manque (phase 5, 2026-10-08)

Trois statuts, et un seul donne le droit d'une affirmation publique :

- **Fermé avec preuve** — il existe un artefact vérifiable (log silicium, preuve Kani
  avec contrôle négatif, test, ou document à sources primaires) et le manque ne limite
  plus le discours. Le chemin de la preuve est cité.
- **Partiellement fermé** — l'essentiel est fait, **ce qui reste est nommé**. Rien ne
  peut être affirmé au-delà de la partie prouvée.
- **Ouvert** — rien ou presque. Aucune affirmation possible.

> Le tableau ci-dessous est la **source de vérité** du statut. Si `POSITIONING.md` ou une
> page commerciale dit mieux, c'est elle qui a tort.

### A — réglementations

| # | Statut | Ce qui le ferme, ou ce qui manque |
|---|---|---|
| **A1** Règlement Machines 1.1.9 / 1.2.1 | **partiellement fermé** | Les deux ❌ d'origine sont levés : **journal infalsifiable** (partie I, silicium + coupure de courant réelle, `evidence/silicon/2026-10-08/hardening/`) et **identité du firmware** (phase 1.3, en-tête `OFWI` + version, plancher anti-retour). Reste **hors de notre main** : la conformité est celle du **fabricant de la machine** (art. 8 et 10), pas d'un composant ; et l'art. 10(3) peut exiger le code source — ce que la licence MIT rend trivial, mais qui demande un dossier, pas une bibliothèque |
| **A2** CRA | **partiellement fermé** | SBOM CycloneDX, `cargo-audit`, `cargo-deny` et `SECURITY.md` conforme (contact, délais 24 h/72 h, divulgation coordonnée) : `evidence/supply-chain/2026-10-07/`. Mise à jour signée : phase 1.3. ⚠️ **La mise à jour est USB, pas par le réseau** — le CRA attend des mises à jour **distribuables**. Reste **D9** (5 ans d'assistance) qui est contractuel, pas technique |
| **A3** RED / EN 18031 | **partiellement fermé** | Le mécanisme de mise à jour sécurisé existe (phase 1.3, prouvé sur silicium avec deux coupures de courant). ⚠️ **Aucune évaluation EN 18031 par un laboratoire**, et surtout : **OASIS n'a aucune radio** — la RED s'applique à l'équipement radio, donc l'applicabilité réelle reste à établir sur un produit qui émet |
| **A4** IEC 62443-4-2 / -4-1 | **partiellement fermé** | CR 1.2 identité ✅ ; **CR 2.8 journal auditable ✅** (partie I) ; **CR 7.1 déni de service ✅ mesuré** (phase 2.1 : 179,4 ms → 0,70 ms, 60/60 messages légitimes sous flot) ; EDR 3.10 mise à jour ✅ ; CR 3.4 / EDR 3.14 intégrité **partielle** (le chargeur d'amorçage n'est ni signé ni mis à jour, pas de secure boot RP2040). ⚠️ **-4-1 (processus de développement documenté) reste ouvert**, et une certification exige d'abord un SDLA d'entreprise |
| **A5** IEC TS 63074 | **fermé** | [`docs/compliance/IEC_TS_63074.md`](../docs/compliance/IEC_TS_63074.md) : c'est le document qui porte **la frontière**, et il sert à dire ce qu'OASIS **n'est pas**. ⚠️ Norme payante : **non vérifiée à la source**, la provenance de chaque élément est indiquée |
| **A6** agilité cryptographique | **fermé avec preuve** | La phase 3 demandait « un identifiant d'algorithme **ou** un plan de migration écrit » : [`docs/CRYPTO_MIGRATION.md`](../docs/CRYPTO_MIGRATION.md). L'identifiant existe déjà pour les messages d'autorité (octet de suite signé, minimum par type, déclassement refusé avant vérification) ; la couche par saut migre **par allocation de magic**, et le document dit pourquoi un octet de suite négociable y rouvrirait C1. Tailles ML-KEM vérifiées dans FIPS 203 Table 3 ; chiffres assertés par `docs/lora_budget.py`. ⚠️ Le prérequis **§6-b, l'absence de rotation de clé de nœud**, gouverne le calendrier |
| **A7** SORA 2.5 | **sans objet (information)** | Corrigé en F.1 : les classes **C2 et C3** de (UE) 2019/945 **exigent bien** une liaison protégée. Mais l'exigence est téléologique, sans moyen prescrit : **non différenciante** |
| **A8** Blue UAS / Green UAS | **ouvert** | Hors périmètre. Aucun travail fait, aucune affirmation faite |
| **A9** voie EUCC (art. 20(9)) | **ouvert** | Identifié en phase 0, non instruit |
| **A10** ISO 10218-1/-2:2025 | **ouvert** | Identifié en phase 0. Normes payantes, **non vérifiées à la source** |
| **A11** NIS2, fabricant dans le champ | **ouvert** | Identifié en phase 0 (annexe II 5 d), NACE division 28). Conséquence pour un fournisseur : non instruite |
| **A12** classification « auto-évolutif » | **partiellement fermé** | **C8** a retiré « porte R14 / entropie » et tout le vocabulaire bio-inspiré des documents acheteurs (`5bed8f8`) : le risque de se faire classer machine à apprentissage automatique vient d'abord du vocabulaire. ⚠️ Le **code** garde ses noms (`hyper_state`, `emotion`, `dreams`) et le daemon Face 2 reste décrit comme adaptatif |

### B — ce que l'acheteur a déjà

| # | Statut | Ce qui le ferme, ou ce qui manque |
|---|---|---|
| **B1** couche MAVLink | **fermé avec preuve** | `mavlink_order` : enveloppe v0B dans un `V2_EXTENSION` (msgid 248), armement construit **seulement** dans la branche `Act`. 7 tests sur un vrai `MeshRouter` couvrant les quatre cas demandés, 4 preuves Kani **4/4** avec contrôle négatif 3/3 (`evidence/kani/2026-10-08/b1/`). **Silicium/SITL 2026-10-08** (`evidence/silicon/2026-10-08/b1-sitl/`, PX4 `v1.18.0-beta1-985-gdf387bdec2`, SIH quadx) : **5/5**. L'ordre valide → `GATE Act` → une trame `COMMAND_LONG(400)` → **`INFO [commander] Armed by external command`**. Forgé → `unknown sender`, altéré après signature → `bad mesh signature`, rejoué → `stale counter` sur la deuxième copie (**une trame, pas deux**), révoqué → `Reject(Revoked)` : **0 armement** dans les trois cas, lu dans le journal de PX4. Le `V2_EXTENSION` traverse une **vraie socket UDP entre deux processus**. Construit **sans `sudo`** (venv + roues pip) et **sans Java** (SIH fait voler PX4 avec sa physique interne). ⚠️ Reste de la **simulation** : ni Pixhawk, ni cellule, ni radio. Et B1 sert **S3**, pas le segment retenu |
| **B2** secure boot PX4 | **sans objet (information)** | Sert à se positionner comme complémentaire. Rien à fermer |
| **B3** ROS 2 / SROS2 | **ouvert** | Aucun pont ROS 2. Les primitives équivalentes existent et sont mesurées (A/B contre rclcpp), ce qui n'est pas un pont |
| **B4** automates du parc existant | **partiellement fermé** | La **forme passerelle** est prouvée des deux côtés. **RTU, sur silicium** (phase 1.4) : équipement `rmodbus` indépendant sur trois RP2040, **4 décisions « agir » = 4 trames = 4 écritures** octet pour octet, 12 refus + 3 injections brutes = **0 octet** sur le bus. **TCP, en programmes qui tournent** (pilote phase A, 2026-10-08) : `oasis_mbtcp_gateway` et `oasis_mbtcp_agent` — un audit avait relevé qu'il n'y avait **aucune socket dans `oasis-rt`**, la couche TCP ne sachant que décider sans jamais atteindre un automate. Le test d'intégration passe par quatre vraies sockets contre un serveur **`rmodbus`** et relit la valeur dans son stockage ; 9 cas d'attaque, 5 cas de lecture. Les **lectures passent par la passerelle** (`OMQ1`/`OMV1`, 5 harnais Kani vérifiés 5/5) au lieu d'être refusées — le refus poussait tout déploiement réel vers un canal latéral direct vers l'automate, exactement ce que la passerelle existe pour empêcher. Coût mesuré, K=10 : une écriture autorisée **459–468 µs contre 45–50 µs en direct**, dont **286–317 µs d'OASIS** à forme de chemin identique. ⚠️ **Rien n'a rencontré un vrai automate** : `rmodbus` est une implémentation indépendante, pas un équipement. ⚠️ Les ordres sont servis **un à la fois**. ⚠️ Ce n'est pas un **produit certifié**, et C14 (clé lisible en flash) sera la première objection |
| **B5** couche par-dessus un transport existant | **partiellement fermé** | B1 est exactement ce mode, et il fonctionne — pour **MAVLink**, en logiciel. Reticulum lui-même n'est toujours pas testé |
| **B6** ISO 3691-4 exclut les chariots télécommandés | **ouvert** | Information de périmètre pour S1, à vérifier avec le premier prospect. ⚠️ Norme payante, verbatim obtenu par extrait |
| **B7** U-space | **ouvert** | Identifié en phase 0. L'exigence n'est pas chez l'équipementier |

### C — manques techniques

| # | Statut | Ce qui le ferme, ou ce qui manque |
|---|---|---|
| **C1** déni de service par vérification forcée | **fermé avec preuve** | `SPORE\x0C` : MAC de lien dérivé des identités (rien de distribué) + seau à jetons. Silicium : trame forgée **179,4 ms → 0,70 ms (256×)**, et sous 8 trames forgées/s pendant 60 s, v0C livre **60/60** messages légitimes à 19 % de charge contre **39/60** à 110 % en v0B. ⚠️ **Contre un initié, le seau est un plafond CPU, pas de l'équité** : la livraison légitime tombe à 4/60. Jamais présenter C1 comme une protection de disponibilité contre un initié |
| **C2** arrêt et perte de liaison | **fermé avec preuve** | Partie G (arrêt asymétrique : 3 conditions sur 10, verrou latché, ISO 13850:2015 4.1.1.2) et partie H (vivacité). Silicium 19/20 avec coupure de courant réelle. ⚠️ Un défaut trouvé pendant les essais : l'arrêt latché n'était pas lu par la passerelle Modbus — corrigé (`13a3b5c`), deux tests ajoutés |
| **C3** budget radio | **fermé avec preuve** | Chiffré et sourcé : plafonds LoRaWAN RP002-1.0.3 (51 o en SF10–12), duty-cycle ETSI EN 300 220-2 §4.4.3.2 **par bande**, et `docs/lora_budget.py` qui asserte sa propre formule. Un **arrêt compact `OAS1` de 11 o** a été ajouté (partie J, silicium 6/6). ✅ **Appliqué par le code depuis le 2026-10-08** : `LoRaTransport` **possède** son budget (EU868 1 %), `send_envelope` facture le temps d'antenne de la trame et refuse avec `DutyCycleExceeded { needed_us, available_us, retry_in_ms }` ; y échapper demande `new_without_duty_limit`, qui se lit au point d'appel. 4 tests, dont une radio qui ne fait que compter : **50 trames refusées ne l'atteignent pas**. ⚠️ Reste : la formule de time on air est la forme publiée de la datasheet SX1276, **non vérifiée à la source**, donc l'application est exacte vis-à-vis de cette formule et non d'une radio mesurée — et **aucune radio n'a jamais émis**. |
| **C4** protocole d'émission d'un ordre | **fermé avec preuve** | Partie K : `TimeView`, le commandant construit l'échéance depuis une balise `OTM1` signée, sans jamais lire l'horloge de l'actionneur. Silicium **6/6**, dont un rejeu de balise avec horloge forcée. ⚠️ Pendant l'essai j'ai **refusé** de relayer la balise par le PC, parce que c'est le raccourci de labo que C4 dénonce |
| **C5** journal infalsifiable | **fermé avec preuve** | Partie I : `h_n = SHA-256(DOMAIN‖h_{n−1}‖entrée)`, persisté en anneau, vérifié par `oasis_journal_verify` recoupé avec un oracle Python indépendant. Silicium avec **coupure de courant réelle** (S8). **Étendu le 2026-10-08 aux changements de logiciel et de configuration** — annexe III 1.1.9, alinéa 5, 2ᵉ et 3ᵉ déclencheurs, **vérifiés à la source** : installation de firmware, enrôlement, révocation, élévation de politique, transfert de propriété, chacun avec qui l'a autorisé et un bit appliqué/refusé, sur la **même** chaîne et **sans changer le format d'entrée** (6 harnais Kani 6/6, `evidence/kani/2026-10-08/journal-change/`). ⚠️ **Tamper-évident contre un attaquant distant seulement** : qui a la flash réécrit la chaîne. ⚠️ Le câblage des cinq sites compile mais **n'a pas été flashé** |
| **C6** cycle de vie des clés | **partiellement fermé** | [`docs/KEY_LIFECYCLE.md`](../docs/KEY_LIFECYCLE.md) : les quatre clés, l'injection, l'enrôlement, le stockage, la récupération. ⚠️ **Cinq manques nommés dans le document lui-même**, dont deux graves : **aucune rotation de clé de nœud** (et `MAX_REVOKED = 16` comme plafond dur) et **aucune récupération de la clé propriétaire**. Deux pièges opératoires documentés |
| **C7** mise à jour signée + identité du firmware | **fermé avec preuve** | Phase 1.3 : manifeste `OAU1` kind 3 hybride, `embassy-boot`, essai de démarrage avec retour arrière, plancher de version. Silicium T1–T7 sur 3 RP2040 dont **deux coupures de courant réelles** (pendant le téléversement, pendant l'échange). ⚠️ **Pas de secure boot** : BOOTSEL, SWD ou le `b` du firmware de test contournent tout, plancher inclus. Le chargeur d'amorçage n'est ni signé ni mis à jour |
| **C8** vocabulaire | **fermé** | `5bed8f8` : « porte R14 / entropie » → « verrou d'état des capteurs », et la mention « pas une fonction de sûreté certifiée » ajoutée à **10** documents acheteurs. Aucun fichier Rust touché. « R14 » ne survit que dans `POSITIONING_GAPS.md`, où il est le sujet |
| **C9** ordres à deux signatures | **partiellement fermé** | `quorum` : k parmi n via `oasis_operator_key`, **clés distinctes** exigées, quorum placé **après** l'authenticité et **avant** les neuf conditions. 9 tests, 5 Kani **12/12** avec contrôle négatif 2/2. ⚠️ **Rien sur silicium** : aucune carte ne détient un **jeu** de clés opérateur, donc le chemin k-sur-n reste sur PC, comme pour la révocation. ⚠️ **343 o** contre 255 : un ordre à deux signatures **coûte deux trames** |
| **C10** assurance logicielle | **partiellement fermé** | `cargo-fuzz` sur **13 parseurs en 9 cibles** : neuf à **1,50 × 10⁹ exécutions** (une anomalie trouvée : encodage non canonique d'`OAC1`, corrigée) et quatre ajoutés avec le chemin de lecture TCP à **2,23 × 10⁷, 0 plantage** — chiffres gardés séparés, le second n'ayant pas contribué au premier (`evidence/fuzz/2026-10-08/`), `cargo-audit`, `cargo-deny`, SBOM CycloneDX, `SECURITY.md` : `evidence/fuzz/2026-10-07/` et `evidence/supply-chain/2026-10-07/`. **CI verte sur `main`, 2026-10-08, dépôt public** (exécution `d5ba2aa`, **15 jobs sur 15**) : tests Rust sur ubuntu/macos/windows, `cargo-audit`, l'image Docker, la couverture des shards, et **les sept shards Kani** — soit **182 des 189 harnais vérifiés en CI**. Les **7 lourds** sont dans `kani-heavy.yml`, hebdomadaire, donc « 189/189 » reste faux. Et en local, le **balayage séquentiel** du même jour (`evidence/kani/2026-10-08/sweep/`) avait déjà vérifié 175 des 182 d'alors, 0 réfuté : l'OOM du passage intégral du 2026-10-07 (130/152) venait du **parallélisme**, pas des harnais. |
| **C11** routage, radio réelle, énergie | **ouvert** | **Aucune radio n'a jamais émis.** Le pilote SX1262 existe et n'est testé que contre un mock. Tous les budgets sont calculés. L'énergie n'est pas mesurée |
| **C12** vivacité de la supervision | **fermé avec preuve** | Partie H : `OSB1` (kind 4), `MAX_SUPERVISION_MS = 300_000`, et la supervision est un **choix explicite de constructeur** (`new_supervised()`), pas un défaut caché. Silicium. ⚠️ Un défaut trouvé pendant les essais : `content_kind` ne connaissait pas `OSB1`, donc la partie H ne faisait **rien** en journalisant « vérifié » — corrigé (`a09781e`) |
| **C13** v0B et v0C ne chiffrent pas | **fermé avec preuve** | `sealed` (`OSE1`, 2026-10-08) : la charge utile est scellée **de l'origine à l'actionneur adressé**, clé dérivée des deux identités par `HKDF-SHA256(X25519(...))` avec son propre domaine — **rien n'est distribué** et **un relais n'a aucune clé**, à la différence de la clé par lien du pré-filtre. L'AAD lie domaine, réseau, les deux empreintes et le compteur, donc un chiffré sorti d'une enveloppe ne s'ouvre pas dans une autre ; le nonce vient du compteur v0B, strictement croissant et persisté. **v0B et la porte sont inchangés** : un blob scellé est une charge utile comme une autre, et `open` rend les octets `OAC1` que la règle de la partie F lit sans modification. 8 tests (dont balayage exhaustif d'inversion d'un octet et cinq AAD erronés), **5 preuves Kani 5/5** avec contrôle négatif 2/2 (`evidence/kani/2026-10-08/sealed/`). Surcoût **50 o** : un ordre scellé fait 203 o sur le fil, toujours une trame. ⚠️ **N'occulte pas l'adressage** — origine, compteur, longueur et destinataire restent lisibles, donc l'**analyse de trafic n'est pas fermée**. ⚠️ **Rien sur silicium** : le firmware n'émet ni n'accepte `OSE1` |
| **C14** stockage sécurisé | **ouvert** | **Ne se ferme pas en logiciel.** Clé privée lisible en flash par BOOTSEL ou SWD, aucun élément sécurisé, aucun effacement sûr. C'est la première objection d'un RSSI et la réponse honnête est « il faut un autre silicium » |

### D — marché et entreprise

| # | Statut | Ce qui le ferme, ou ce qui manque |
|---|---|---|
| **D1** aucune validation client | **ouvert** | [`CUSTOMER_DISCOVERY.md`](CUSTOMER_DISCOVERY.md) est prêt : 5 hypothèses falsifiables avec seuils de réfutation, une grille de 30 min qui ne montre rien avant la minute 25, 20 types d'organisations. **Zéro entretien mené.** La préparation est faite, **le manque reste entier** |
| **D2** trop de segments | **fermé** | [`SEGMENT_COMPARISON.md`](SEGMENT_COMPARISON.md), trois candidats chiffrés, **S1 retenu** (machines mobiles autonomes), avec l'argument contre la recommandation écrit en clair |
| **D3** forme du produit | **partiellement fermé** | Tranché pour S1 : **fournisseur de preuve** (campagne d'attaque sur le banc du client + logs + tableaux de correspondance), pas un composant de sécurité au sens de l'art. 3(3). La forme **passerelle** est prouvée pour S2 (phase 1.4). ⚠️ Non validé auprès d'un acheteur réel : c'est **H3** de `CUSTOMER_DISCOVERY.md`, et si elle est réfutée, S1 se ferme |
| **D4** licence MIT / steward CRA | **fermé** | Vérifié à la source en phase 0 : un composant libre **non monétisé** est hors du champ du CRA (considérants 18 et 20), et l'art. 13 §5 met la diligence sur l'intégrateur |
| **D5** une seule personne | **ouvert** | Aucune réponse possible par le code. C'est l'objection la plus dure de S1 avec D9 |
| **D6** financement | **ouvert** | Passe par des consortiums, donc par un partenaire industriel. Rien fait |
| **D7** contrôle des exportations | **remplacé par D8** | Sévérité relevée en phase 0 |
| **D8** angles morts du contrôle des exportations | **ouvert** | Identifié en phase 0 (règlement (UE) 2021/821). À vérifier **avant** tout prospect hors UE |
| **D9** durée de support | **partiellement fermé** | Écrit dans [`SECURITY.md`](../SECURITY.md) § *Support period* : **aujourd'hui aucune période de support n'est due** — un composant libre **non monétisé** est hors du champ du CRA (considérants 18 et 20, vérifié en phase 0) et l'art. 13(5) met la diligence sur l'intégrateur. Le document dit aussi ce qui change **le jour de la monétisation** (5 ans d'assistance, 10 ans de disponibilité) et répond « non, pas seul ». ⚠️ **L'engagement reste impossible à prendre** tant qu'il n'y a pas d'entité pour le porter : c'est une question commerciale avant d'être technique (voir aussi D5) |

### Compte

| Statut | A | B | C | D | **Total** |
|---|---:|---:|---:|---:|---:|
| Fermé avec preuve | 1 | 1 | 8 | 0 | **10** |
| Fermé (documentaire) | 1 | 0 | 1 | 2 | **4** |
| Partiellement fermé | 5 | 2 | 3 | 2 | **12** |
| Ouvert | 4 | 3 | 2 | 4 | **13** |
| Sans objet / information | 1 | 1 | 0 | 1 | **3** |
| | **12** | **7** | **14** | **9** | **42** |

Compté par script depuis le tableau ci-dessus, pas de tête : mon premier total à la main
donnait 41 au lieu de 42, avec trois cases fausses. D7 est en « sans objet » puisque D8 le
remplace. Les 🔴 d'origine — A1, B1, C1, C2, C3, C5, C6, D1, D2 — sont **tous** sortis de
l'état « ouvert » **sauf D1**, qui ne peut être fermé que par des entretiens que je ne
peux pas mener.

### Les trois phrases qu'on ne peut toujours pas dire

1. **« Résistant au déni de service »** sans la suite : contre un **initié**, le seau à
   jetons est un plafond CPU qui **affame le trafic légitime** (4/60).
2. **« Journal infalsifiable »** sans la suite : **contre un attaquant distant
   seulement** ; qui tient la flash réécrit la chaîne (C14).
3. **« Prouvé sur le terrain »** : **aucune radio n'a jamais émis**, aucune machine
   mobile, aucun drone réel. Tout tient sur trois RP2040 reliés par des fils.

---

## Sources

> **Les sources secondaires ci-dessous sont celles de la version du 2026-10-06.** Elles sont
> conservées pour l'historique. **Les sources primaires** (EUR-Lex, JOUE, FIPS, RFC, pages
> officielles IEC, spécifications LoRa Alliance et ETSI, dépôts PX4 et ArduPilot) sont dans
> [`docs/compliance/`](../docs/compliance/), un fichier par texte, avec pour chacune le statut
> « vérifié à la source » ou « non vérifié à la source ». Les notes de recherche brutes, avec les
> verbatim intégraux, sont dans
> `research_notes/Conformité réglementaire OASIS phase 0/` (8 fichiers).

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
