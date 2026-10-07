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
