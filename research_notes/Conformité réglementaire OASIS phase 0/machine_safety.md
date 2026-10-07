# Normes de sûreté machines et robotique — frontière « fonction de sûreté » / « mesure de cybersécurité »

> **Date de recherche : 2026-10-07.**
>
> **Convention de vérification** (ces normes sont payantes) :
> - **[SOURCE VÉRIFIÉE]** = j'ai lu le texte normatif lui-même, dans un extrait officiel gratuit
>   (aperçu « ISO sample » PDF publié par ISO/iTeh, page de périmètre officielle IEC) et je cite
>   verbatim.
> - **[NON VÉRIFIÉ À LA SOURCE]** = je n'ai pas lu le texte de la norme ; j'indique systématiquement
>   l'origine tierce de l'information.
>
> **Mise en garde terminologique, applicable à tout ce document** : aucune des normes examinées ne
> permet de qualifier un composant de cybersécurité (signature, authentification, anti-rejeu,
> révocation, chiffrement) de « fonction de sûreté certifiée » PL ou SIL. Les sections 6 et 7
> ci-dessous documentent précisément pourquoi ce sont deux objets normatifs distincts, et ce que les
> normes exigent réellement à l'interface entre les deux.

---

## Q1 — ISO 13849-1 et -2 : titre, édition, périmètre, détermination du PL, et position sur la cybersécurité

### Takeaway

ISO 13849-1:2023 (4ᵉ édition) détermine le Performance Level PL a→e par la combinaison
catégorie d'architecture + MTTFD + couverture de diagnostic (DC) + maîtrise des défaillances
d'origine commune (CCF). **La norme exclut explicitement de son périmètre les aspects de
sécurité informatique** tout en reconnaissant qu'ils peuvent affecter les fonctions de sûreté, et
renvoie à des documents séparés (IEC/TS 63074, ISO/TR 22100-x). C'est le point de frontière le plus
net et le plus citable de tout le corpus.

### Cited Findings

**Titres et éditions**

- Titre officiel de la partie 1 : « Safety of machinery — Safety-related parts of control systems — Part 1: General principles for design », ISO 13849-1:2023, publiée le 26 avril 2023, 152 pages — [AFNOR / boutique](https://www.boutique.afnor.org/en-gb/standard/iso-1384912023/safety-of-machinery-safetyrelated-parts-of-control-systems-part-1-general-p/xs137212/344966) **[NON VÉRIFIÉ À LA SOURCE** — métadonnées de revendeurs de normes ; la date du 26 avril 2023 et la pagination viennent de catalogues commerciaux, non du texte**]**
- Périmètre (extrait de l'« Overview » ISO repris par les revendeurs) : « specifies a methodology and provides related requirements, recommendations and guidance for the design and integration of safety-related parts of control systems (SRP/CS) that perform safety functions, including the design of software » ; s'applique « regardless of the type of technology and energy (e.g. electrical, hydraulic, pneumatic, and mechanical) » ; **« does not apply to low demand mode of operation »** — [AFNOR](https://www.boutique.afnor.org/en-gb/standard/iso-1384912023/safety-of-machinery-safetyrelated-parts-of-control-systems-part-1-general-p/xs137212/344966) **[NON VÉRIFIÉ À LA SOURCE]**
- La 4ᵉ édition est une **norme de type B** (aspect horizontal, non spécifique à une machine), publiée par le CEN le 17 mai 2023, élaborée par le CEN/TC 114 — [CEN-CENELEC](https://www.cencenelec.eu/news-events/news/2023/eninthespotlight/2023-05-25-en-iso-13849-1-2023/) **[NON VÉRIFIÉ À LA SOURCE]**
- Partie 2 : **ISO 13849-2:2012, édition 2 (2012-10)**, « Safety of machinery — Safety-related parts of control systems — Part 2: Validation ». Confirmée en 2018, elle porte le statut ISO **90.92 « International Standard to be revised »**. Une **ISO/DIS 13849-2** est en préparation, avec un titre modifié : « Part 2: **Application of principles for the design and validation** » — [page ISO 53640 / DIN Media](https://dinmedia.de/en/standard/iso-13849-2/167356377) **[NON VÉRIFIÉ À LA SOURCE** — pages catalogue, pas le texte**]**

**Périmètre — mode de sollicitation [SOURCE VÉRIFIÉE via publication officielle IFA/DGUV]**

- « the standard only applies to SRP/CS for a high demand or continuous mode of operation. According to definition 3.1.44, the demand for this mode is more frequent than once a year. For SRP/CS in low demand mode, i.e. where the frequency is less than once a year […] the authors now refer expressly to the IEC 61508 series of standards. » — [IFA/DGUV, « Fourth edition of EN ISO 13849-1 – new features », §2](https://publikationen.dguv.de/widgets/pdf/download/article/4894)
- Une nouvelle note précise que le contenu est « geared towards stationary machinery, although other types of machinery, such as **mobile machinery**, are also expressly covered » — [IFA/DGUV, §1](https://publikationen.dguv.de/widgets/pdf/download/article/4894)
- La validation, auparavant dans la partie 2, est désormais **dans la partie 1, section 10** ; en période transitoire « the more recent requirements from part 1 take priority […] until the revision of EN ISO 13849-2 has been completed » — [IFA/DGUV, §1](https://publikationen.dguv.de/widgets/pdf/download/article/4894)

**Détermination du PL [SOURCE VÉRIFIÉE via IFA/DGUV]**

- Les paramètres sont la **catégorie** (architecture), le **MTTFD**, le **DC** et la **CCF**, chacun traité dans une sous-section propre de la section 6 : catégories en **6.1.3**, MTTFD en **6.1.4**, **CCF en 6.1.6** (« The requirement to avoid common cause failures is now specified in a separate subsection 6.1.6, which refers to the familiar Annex F »), défaillances systématiques en **6.1.7** — [IFA/DGUV, §6.1–6.4](https://publikationen.dguv.de/widgets/pdf/download/article/4894)
- Changement de vocabulaire important : « the previous "probability of dangerous failure per hour" is now referred to as "**average frequency of a dangerous failure per hour**". Its abbreviation has been changed back to **PFH** (without the "D" index), to ensure that it has the same name as in the IEC standards on functional safety. **This does not result in technical changes.** » — [IFA/DGUV, §3](https://publikationen.dguv.de/widgets/pdf/download/article/4894)
- Catégorie 2 : fault detection requise « for each of its parts (input unit, logic and output unit), at least with "low" diagnostic coverage (DC), i.e. **60 % fault detection** » ; le MTTFD du canal de test « must be greater than **half** the MTTFD of the entire functional channel » — [IFA/DGUV, §6.1](https://publikationen.dguv.de/widgets/pdf/download/article/4894)
- Catégorie 4 : « "only" a **high DC of 99 %** is required for PFH calculation », mais par définition « the accumulation of (up to two) undetected dangerous failures must not lead to the loss of the safety function » — [IFA/DGUV, §6.1](https://publikationen.dguv.de/widgets/pdf/download/article/4894)
- Exclusion de défaut : « A **PL e for subsystems must not be based on fault exclusions alone** » (restriction auparavant connue seulement de l'ISO/TR 23849, désormais en 6.1.10) — [IFA/DGUV, §6.7](https://publikationen.dguv.de/widgets/pdf/download/article/4894)
- Procédure alternative sans MTTFD (6.1.9), étendue aux parties entrée et logique, limitée aux sous-systèmes mécaniques / (électro)hydrauliques / (électro)pneumatiques sans données de fiabilité : « The achievable maximum is **PL c** » — [IFA/DGUV, §6.6](https://publikationen.dguv.de/widgets/pdf/download/article/4894)
- Nouveauté 6.1.7 : « the express requirement for a **functional safety plan** that defines functional safety management in order to protect against systematic failures in specification, implementation or modification » (détails en Annexe G) — [IFA/DGUV, §6.4](https://publikationen.dguv.de/widgets/pdf/download/article/4894)
- Annexe A (informative) sert à déterminer le **PLr** (PL requis) « unless it is specified in a product standard (type-C standard) » — [IFA/DGUV, §1](https://publikationen.dguv.de/widgets/pdf/download/article/4894)

**Logiciel [SOURCE VÉRIFIÉE via IFA/DGUV]**

- Les exigences logicielles sont regroupées dans une **section 7** dédiée. « In the introduction, the authors state that the section **does not contain any specific requirements on software using artificial intelligence**. » — [IFA/DGUV, §7](https://publikationen.dguv.de/widgets/pdf/download/article/4894)
- Distinction **SRESW** (safety-related embedded software, « usually requires a full variability language (FVL) ») vs **SRASW** (safety-related application software, « can also be written in LVL »). Un langage est LVL s'il est conforme à IEC 61131-3 (ladder, FBD, SFC, algèbre booléenne), ou « if a defined programming flow has been put in place through restrictions realised by programming guidelines, compilers or development tools » — [IFA/DGUV, §7](https://publikationen.dguv.de/widgets/pdf/download/article/4894)
- Nouvelle **Annexe N** : quatre cas d'usage fondés sur le PL (a/b, c, d, e), « which can be downgraded by one level, depending on where the software is used » (canal de test d'une catégorie 2, canaux diversifiés d'une catégorie 3 ou 4) — [IFA/DGUV, §7](https://publikationen.dguv.de/widgets/pdf/download/article/4894)
- Le paramétrage manuel par logiciel est traité séparément en **6.3** : « intentionally distinct from the rest of the software part of the standard » — [IFA/DGUV, §6.9](https://publikationen.dguv.de/widgets/pdf/download/article/4894)

**Position sur la cybersécurité — LE point clé [SOURCE VÉRIFIÉE via IFA/DGUV, citation verbatim]**

- > « **IT security aspects are not covered by the standard**, but the authors point out that the such aspects **can also impact safety functions**. For further details, reference is given to ISO/TR 22100-2 [3] and IEC/TR 63074 [4]. »
  — [IFA/DGUV, §2 (section « Scope »)](https://publikationen.dguv.de/widgets/pdf/download/article/4894)
- La bibliographie du même document identifie la référence [4] comme : « **IEC/TS 63074**: Safety of machinery — Security aspects related to functional safety of safety-related control systems (**02.23**) », et la référence [3] comme « ISO/TR 22100-2: Safety of machinery — Relationship with ISO 12100 — Part 2: How ISO 12100 relates to ISO 13849-1 (12.13) » — [IFA/DGUV, bibliographie](https://publikationen.dguv.de/widgets/pdf/download/article/4894)
- Formulation concordante rapportée par une synthèse commerciale : la norme « does not provide specific measures for security aspects (e.g. physical, IT-security, cyber security). However, security issues can have an effect on safety functions, and reference is given to ISO/TR 22100-4 and IEC/TR 63074 » — [GT Engineering](https://www.gt-engineering.it/en/technical-standards/en-iso-standards/en-iso-13849-1-performance-level-estimation/iso-13849-1-new-edition-2023/) **[NON VÉRIFIÉ À LA SOURCE]**
  - ⚠️ **Conflit de références à signaler** : l'IFA/DGUV cite **ISO/TR 22100-2**, la source commerciale cite **ISO/TR 22100-4**. Les deux TR existent et sont distincts (22100-2 = relation ISO 12100 ↔ ISO 13849-1 ; 22100-4 = guidance IT-security pour fabricants de machines). Le texte exact de la note de périmètre d'ISO 13849-1:2023 n'a pas pu être lu : **considérer les deux renvois comme plausibles et vérifier sur le texte acheté.**

**Statut de norme harmonisée UE**

- « A publication in the EU Official Journal for the **Machinery Directive 2006/42/EC** took place on **15 May 2024** » ; la norme précédente (EN ISO 13849-1:2015) « will lose its presumption of conformity on **15 May 2027** » (période transitoire de 3 ans) — [IBF Solutions / Safexpert](https://www.ibf-solutions.com/en/seminars-and-news/news/new-en-iso-13849-12023) **[NON VÉRIFIÉ À LA SOURCE** — je n'ai pas ouvert la décision d'exécution de la Commission au JOUE**]**
- « Both standards [ISO 13849-1 et IEC 62061] have been **harmonised under the Machinery Directive** and can be used for the design and integration of safety-related control systems of machinery (including software) » — [IFA/DGUV, §1](https://publikationen.dguv.de/widgets/pdf/download/article/4894) **[SOURCE VÉRIFIÉE** (publication officielle d'un institut), mais c'est une affirmation de l'IFA, pas le JOUE lui-même**]**

### Inferences

- La structure de preuve d'ISO 13849-1 est **fiabiliste et probabiliste** (taux de défaillance aléatoire, couverture de diagnostic, architecture redondante), pas **adversariale**. Un attaquant n'est pas un mode de défaillance aléatoire : le renvoi explicite à IEC/TS 63074 est la reconnaissance, par les auteurs de la norme eux-mêmes, que leur appareil mathématique ne modélise pas la menace intentionnelle. C'est l'argument le plus solide pour refuser l'équivalence « mesure de cybersécurité = fonction PL ».
- Le renommage PFHD → PFH, explicitement « sans changement technique », est aligné sur le vocabulaire IEC : il facilite la lecture croisée avec IEC 62061/61508 mais n'autorise aucune conversion de propriétés de sécurité informatique en chiffres de fiabilité.
- L'exclusion du *low demand mode* a une conséquence pratique : une fonction sollicitée moins d'une fois par an relève d'IEC 61508, pas d'ISO 13849-1.

### Gaps

- Le **numéro et le libellé exact de la clause ou note de périmètre** d'ISO 13849-1:2023 qui exclut la sécurité informatique n'ont pas été lus (seule la paraphrase IFA/DGUV, qui est fidèle et officielle, a été vérifiée).
- Ambiguïté 22100-2 vs 22100-4 non résolue (voir ci-dessus).
- Je n'ai **pas** trouvé, dans les sources accessibles, de clause d'ISO 13849-1:2023 posant une exigence générale explicite du type « les fonctions non liées à la sécurité ne doivent pas dégrader les fonctions de sûreté ». Le résumé automatique d'un outil de lecture a affirmé que « non-safety-related functions shall not degrade safety function performance » **sans pouvoir citer la clause** ; je considère cette formulation comme **non étayée** et ne la retiens pas comme citation. Voir Q6 pour ce qui est réellement vérifiable.
- Le tableau PL ↔ plages de PFH (PL a : 10⁻⁵ à 10⁻⁴ /h, etc.) n'a pas été vérifié à la source ; je ne le reproduis pas de mémoire.

---

## Q2 — IEC 62061 : titre, édition, périmètre, SIL appliqué aux machines, et différence pratique avec ISO 13849

### Takeaway

IEC 62061:2021 (édition 2.0) est la voie « IEC » de la sécurité fonctionnelle des machines, exprimée en
SIL plutôt qu'en PL. Depuis l'édition 2.0, son titre ne se limite plus à l'électrique/électronique et son
périmètre a convergé avec ISO 13849-1 ; les deux normes sont harmonisées sous la directive Machines et
ISO 13849-1:2023 intègre désormais une correspondance PL ↔ SIL.

### Cited Findings

**[SOURCE VÉRIFIÉE — page de périmètre officielle IEC, webstore.iec.ch]**

- Titre officiel : « **Safety of machinery - Functional safety of safety-related control systems** ». **Édition 2.0**, date de publication **22 mars 2021** — [IEC Webstore, publication 59927](https://webstore.iec.ch/publication/59927)
- Périmètre : spécifie les exigences de conception, intégration et validation des systèmes de commande relatifs à la sécurité, pour « **control systems used, either singly or in combination, to carry out safety functions on machines that are not portable by hand while working** » — [IEC Webstore](https://webstore.iec.ch/publication/59927)
- Centré sur la sécurité fonctionnelle en **mode de sollicitation élevée/continue**, et sur « risks arising directly from the hazards of the machine itself or from a group of machines working together in a coordinated manner » — [IEC Webstore](https://webstore.iec.ch/publication/59927)
- Exclusions : les dangers électriques de l'équipement de commande lui-même, et les mesures de protection générales non liées à la sécurité fonctionnelle — [IEC Webstore](https://webstore.iec.ch/publication/59927)

> **Note de titre importante** : l'édition 1 s'intitulait « …safety-related **electrical, electronic and programmable electronic** control systems ». L'édition 2.0 a abandonné cette restriction technologique. ISO 13850:2015 référence encore IEC 62061 sous l'ancien libellé long (voir Q5), ce qui est normal pour une norme de 2015. — titre long constaté **[SOURCE VÉRIFIÉE** dans la liste de références normatives d'ISO 13850:2015, aperçu officiel**]**

**Articulation PL ↔ SIL [SOURCE VÉRIFIÉE via IFA/DGUV]**

- ISO 13849-1:2023 traite désormais explicitement la correspondance : « the first focus is on the **correlation between PL and SIL**. Referring to subsystems developed under IEC 61508 and IEC 62061, the standard only deals with the integration of those intended for use under high demand or continuous mode of operation and designed according to **Route 1H** (see IEC 61508-2:2011, 7.4.42). » — [IFA/DGUV, §6](https://publikationen.dguv.de/widgets/pdf/download/article/4894)
- « an SRP/CS may also include subsystems of **different categories** and subsystems with **SIL classifications** » — [IFA/DGUV, §6.1](https://publikationen.dguv.de/widgets/pdf/download/article/4894)
- Le rôle des sous-systèmes dans une chaîne de sécurité a été précisé « especially concerning the integration of subsystems that have been given **safety integrity levels (SILs)** under IEC standards […] on functional safety » — [IFA/DGUV, introduction](https://publikationen.dguv.de/widgets/pdf/download/article/4894)
- L'**ISO/TR 23849** (application conjointe d'ISO 13849-1 et IEC 62061), **retiré en 2020**, a vu son contenu absorbé dans les éditions courantes des deux normes — [IFA/DGUV, §1](https://publikationen.dguv.de/widgets/pdf/download/article/4894)

### Inferences

- **Différence pratique** : les deux normes visent le même objectif (intégrité d'une fonction de sûreté en mode de sollicitation élevée) avec deux métriques et deux cultures d'ingénierie. ISO 13849-1 part d'une **architecture typée** (catégories B/1/2/3/4) et d'un calcul simplifié par graphe/barre, ce qui la rend praticable pour des systèmes mixtes incluant hydraulique, pneumatique et mécanique. IEC 62061 part d'une **décomposition en sous-systèmes et éléments de sous-système** avec allocation de SIL, dans la filiation d'IEC 61508, ce qui convient aux architectures majoritairement électroniques/programmables complexes. Le retrait de la restriction technologique dans le titre de l'édition 2.0 et l'ajout de la correspondance PL↔SIL dans ISO 13849-1:2023 indiquent une convergence délibérée des deux comités. **Estimation** : pour un véhicule mobile autonome, ISO 13849-1 reste la voie dominante en pratique car les normes de type C du domaine (dont ISO 3691-4) expriment leurs exigences en PLr — ce point est confirmé pour ISO 3691-4 (voir Q3), mais le caractère « dominant en pratique » est une **estimation**, non une donnée sourcée.
- Les deux normes sont des voies **alternatives et équivalentes** pour la présomption de conformité, pas des exigences cumulatives.

### Gaps

- Le texte normatif d'IEC 62061:2021 n'a **pas** été lu ; seule la page de périmètre officielle IEC l'a été. Les plages numériques de PFH par SIL, la méthode d'allocation, et la position d'IEC 62061:2021 sur la cybersécurité **n'ont pas été vérifiées**. En particulier, **je n'ai pas pu déterminer si IEC 62061:2021 contient une note de renvoi à IEC/TS 63074 analogue à celle d'ISO 13849-1** — à vérifier sur le texte acheté.
- La date de citation d'EN IEC 62061:2021 au JOUE n'a pas été vérifiée.

---

## Q3 — ISO 3691-4 : titre, édition, périmètre, AGV/AMR, commande à distance, arrêt d'urgence, zones, et PERTE DE COMMUNICATION

### Takeaway

**[SOURCE VÉRIFIÉE — aperçu officiel ISO 3691-4:2023]** ISO 3691-4:2023 (2ᵉ édition, 2023-06) est la
norme de type C des chariots sans conducteur. Deux résultats majeurs et contre-intuitifs : (1) la norme
**exclut explicitement de son périmètre les chariots télécommandés**, qui « ne sont pas considérés comme
des chariots sans conducteur » ; (2) **la table des matières officielle de l'article 4 s'arrête à 4.14 et ne
contient aucune clause intitulée « Loss of communication »**. L'affirmation très répandue d'une
« section 4.18 Loss of communication » dans ISO 3691-4 **ne correspond pas à l'édition 2023**.

### Cited Findings

**Titre, édition — [SOURCE VÉRIFIÉE, page de titre de l'aperçu officiel ISO]**

- « INTERNATIONAL STANDARD **ISO 3691-4** — **Second edition 2023-06** — Industrial trucks — Safety requirements and verification — **Part 4: Driverless industrial trucks and their systems** »
- Titre français officiel sur la même page : « **Chariots de manutention — Exigences de sécurité et vérification — Partie 4: Chariots sans conducteur et leurs systèmes** »
- Référence : ISO 3691-4:2023(E), © ISO 2023 — [aperçu officiel ISO 3691-4:2023 (PDF sample ISO/iTeh, réf. 83545)](https://cdn.standards.iteh.ai/samples/83545/a3d9d057a08d4f9c8e8e87cdc947583c/ISO-3691-4-2023.pdf)
- Version européenne : « EVS-EN ISO 3691-4:2023 … Base documents: ISO 3691-4:2023; EN ISO 3691-4:2023 », **valide depuis le 01.08.2023**, l'édition 2020 étant retirée à cette date — [EVS (organisme estonien de normalisation)](https://evs.ee/en/evs-en-iso-3691-4-2023) **[NON VÉRIFIÉ À LA SOURCE** — page catalogue**]**

**Périmètre — citations verbatim [SOURCE VÉRIFIÉE, clause 1 « Scope » de l'aperçu officiel]**

- > « This document specifies safety requirements and the means for their verification for driverless industrial trucks (hereafter referred to as trucks) and their systems. »
- > « Examples of driverless industrial trucks (trucks as defined in ISO 5053-1:2020) include: "automated guided vehicle", "**autonomous mobile robot**", "bots", "automated guided cart", "tunnel tugger", "under cart", etc. »
  → **Les AMR sont donc nommément dans le périmètre**, au même titre que les AGV.
- Également applicable aux chariots dotés de : « automatic modes which either require operators' action(s) to initiate or enable such automatic operations » ; « the capability to transport one or more riders (which are neither considered as drivers nor as operators) » ; « additional manual modes which allow operators to operate the truck manually » ; « a maintenance mode which allows manual operation of truck functions for maintenance reasons ».
- **Exclusion décisive sur la commande à distance** :
  > « This document is **not applicable** to trucks solely guided by mechanical means (rails, guides, etc.) or to **remotely-controlled trucks, which are not considered to be driverless trucks**. »
- > « For the purposes of this document, a driverless industrial truck is a powered truck, which is designed to operate automatically. **A driverless truck system comprises the control system, which can be part of the truck and/or separate from it, guidance means and power system.** Requirements for power sources are not covered in this document. »
- > « The condition of the operating zone has a significant effect on the safe operation of the driverless industrial truck. The preparations of the operating zone to eliminate the associated hazards are specified in **Annex A**. »
- Applicable à tous les dangers significatifs listés en **Annexe B**, sur tout le cycle de vie (ISO 12100:2010, 5.4), en usage normal et en mésusage raisonnablement prévisible.
- Exclusions de dangers : bruit ; vibrations ; rayonnements ionisants et non ionisants ; rayonnement laser ; littérature commerciale ; déclaration des vibrations transmises. Et exclusions de contextes : conditions sévères (climats extrêmes, applications frigorifiques, champs magnétiques forts) ; environnements nucléaires ; **chariots destinés à opérer en zones publiques (renvoi à ISO 13482:2014)** ; **opération sur voie publique**.
  — toutes citations : [aperçu officiel ISO 3691-4:2023](https://cdn.standards.iteh.ai/samples/83545/a3d9d057a08d4f9c8e8e87cdc947583c/ISO-3691-4-2023.pdf)

**Structure de l'article 4 — table des matières officielle [SOURCE VÉRIFIÉE]**

Article 4 « Safety requirements and/or protective/risk reduction measures » (p. 10 à 31) :

| Clause | Titre | Page |
|---|---|---|
| 4.1 | General (4.1.1 Overall requirements → 4.1.27) | 10 |
| 4.1.3 | Electrical requirements | 11 |
| 4.1.10 | Electro-sensitive protective equipment | 12 |
| 4.1.14 | Avoidance of automatic restart | 13 |
| **4.1.26** | **Normal stop** | 15 |
| **4.1.27** | **Operational stop** | 16 |
| 4.2 | Braking system | 16 |
| 4.3 | Speed control (4.3.1 Overspeed detection ; 4.3.2 Speed and stability) | 16 |
| 4.4 | Automatic battery charging | 16 |
| 4.5 | Load handling | 16 |
| 4.6 | Steering | 17 |
| 4.7 | Stability | 17 |
| 4.8 | Protective devices and complementary measures | 18 |
| **4.8.1** | **Emergency stop** | 18 |
| **4.8.2** | **Detection of persons in the path** | 19 |
| 4.9 | Modes of operation (4.9.1 General ; **4.9.2 Automatic mode** ; 4.9.3 Manual mode ; 4.9.4 Maintenance mode) | 21 |
| 4.10 | Trucks intended to tow trailers | 24 |
| **4.11** | **Safety-related parts of the control system** | 24 |
| 4.12 | Electromagnetic immunity | 29 |
| 4.13 | Conveyors fitted to a truck | 30 |
| **4.14** | **Warning systems** (dernière clause de l'article 4) | 31 |

Puis : **5** Verification of the safety requirements (5.1 General ; 5.2 Tests for detection of persons ; 5.3 Stability tests ; 5.4 Fitness for purpose) p. 32 ; **6** Information for use p. 35 ; **Annexe A (normative)** Requirements for preparation of the operating zones p. 41 ; **Annexe B (informative)** List of significant hazards p. 51 ; **Annexe C (normative)** Determination of rated capacity p. 57 ; **Annexe D (informative)** Load transfer operations p. 59 ; **Annexe E (normative)** Verification of the safety requirements and/or protective/risk reduction measures p. 62 ; Bibliography p. 74.
— [aperçu officiel ISO 3691-4:2023, table des matières](https://cdn.standards.iteh.ai/samples/83545/a3d9d057a08d4f9c8e8e87cdc947583c/ISO-3691-4-2023.pdf)

> ⚠️ *Note de lecture* : l'extraction PDF désaligne légèrement les numéros 4.10–4.14 (mise en page à deux colonnes). L'attribution « **4.11 = Safety-related parts of the control system** » et « 4.13 = Conveyors » est **corroborée indépendamment** par le livre blanc TÜV Rheinland, qui situe le Tableau 1 des PLr « in section 4.11 » et les convoyeurs « see section 4.13 ». Les deux sources concordent.

**PERTE DE COMMUNICATION — résultat principal**

- **Constat négatif vérifié** : dans la table des matières officielle d'ISO 3691-4:2023, **l'article 4 s'arrête à 4.14 ; il n'existe ni clause 4.15, ni 4.16, ni 4.17, ni 4.18, et aucune clause intitulée « Communication » ou « Loss of communication »** — [aperçu officiel ISO 3691-4:2023](https://cdn.standards.iteh.ai/samples/83545/a3d9d057a08d4f9c8e8e87cdc947583c/ISO-3691-4-2023.pdf) **[SOURCE VÉRIFIÉE]**
- La communication apparaît dans l'aperçu uniquement comme **composant ancillaire** dans une note de définition : « Ancillary components can be integrated or external (e.g. guidance, traffic control, power system, **communication system**, guarding, signs, warnings, floor marking). » — [aperçu officiel ISO 3691-4:2023, note 1 de la définition 3.6](https://cdn.standards.iteh.ai/samples/83545/a3d9d057a08d4f9c8e8e87cdc947583c/ISO-3691-4-2023.pdf) **[SOURCE VÉRIFIÉE]**
- **Affirmations tierces contradictoires, à traiter avec prudence** :
  - Des synthèses de résultats de recherche affirment qu'« ISO 3691-4 includes requirements for "Loss of communication" (**section 4.18**) » et que la norme « specifies tests for detection systems, side protection, load handling, **communications loss**, and structural integrity » — agrégation de [résultats de recherche incluant Pilz et iTeh](https://www.pilz.com/en-INT/company/news/articles/238928) **[NON VÉRIFIÉ À LA SOURCE ; et en contradiction directe avec la table des matières officielle 2023 que j'ai lue]**
  - Une documentation d'éditeur (outillage d'audit, non normative) place la communication en « **§5.15 Communication** » avec la mention « verify safe-state on heartbeat timeout » — [Roboticks docs](https://docs.roboticks.io/standards/iso-3691-4) **[NON VÉRIFIÉ À LA SOURCE ; numérotation incompatible avec la table des matières officielle, où l'article 5 s'arrête à 5.4]**
  - **Hypothèse de résolution (à vérifier, non confirmée)** : une **ISO/DIS 3691-4** distincte est en cours d'élaboration sous la référence ISO 88615, en plus de l'édition publiée 2023 (réf. 83545) — [page ISO/DIS 3691-4](https://www.iso.org/standard/88615.html) (référence relevée dans des résultats de recherche ; **page non consultable, iso.org renvoie HTTP 403**). Il est **possible** que la clause « 4.18 Loss of communication » appartienne à ce projet de révision ou à un autre référentiel (p. ex. ANSI/ITSDF B56.5), mais **je n'ai aucune preuve de cela : c'est une piste, pas un fait.**

**Arrêt, zones, PL — ce qui est vérifiable**

- Définitions vérifiées dans l'aperçu officiel : **3.45 « protective stop: safety-related stop function initiated by a protective device »** ; **3.16 « emergency stop device: manually actuated control device used to initiate an emergency stop function » [SOURCE: ISO 13850:2015, 3.3]** — [aperçu officiel ISO 3691-4:2023](https://cdn.standards.iteh.ai/samples/83545/a3d9d057a08d4f9c8e8e87cdc947583c/ISO-3691-4-2023.pdf) **[SOURCE VÉRIFIÉE]**
- **ISO 13850:2015 est une référence normative d'ISO 3691-4:2023** — [aperçu officiel, clause 2](https://cdn.standards.iteh.ai/samples/83545/a3d9d057a08d4f9c8e8e87cdc947583c/ISO-3691-4-2023.pdf) **[SOURCE VÉRIFIÉE]**. Conséquence directe : **les exigences de priorité de l'arrêt d'urgence d'ISO 13850 (voir Q5) s'appliquent par renvoi normatif aux AGV/AMR.**
- Évolutions 2020 → 2023, verbatim du Foreword : « the term entries "**active detection field**" and "**operational stop**" have been added to Clause 3 » ; « Clause 4, Clause 5, Clause 6, Annex A, Annex B and Annex C have been updated, with **new requirements added in subclauses 4.1.16 to 4.1.27** » ; « the verification of the safety requirements lists in **Annex E** have been reworded » — [aperçu officiel ISO 3691-4:2023, Foreword](https://cdn.standards.iteh.ai/samples/83545/a3d9d057a08d4f9c8e8e87cdc947583c/ISO-3691-4-2023.pdf) **[SOURCE VÉRIFIÉE]**
- **Tableau 1 des PLr (édition 2020)** : « Table 1, in section 4.11 describes the Safety Function (**27 in total**), the associated risk and the **minimum PLr acc. to ISO 13849-1** » ; exemple reproduit : commande du système de freinage **PLr d**, commande du frein de stationnement **PLr b** — [TÜV Rheinland, « ISO 3691-4:2020 — A Standard for Automated Guided Vehicles », p. 6](https://www.tuv.com/content-media-files/master-content/services/industrial-services/pdf/tuv-rheinland-automatic-guided-vehicles-whitepaper-en_neu.pdf) **[NON VÉRIFIÉ À LA SOURCE** — livre blanc d'organisme notifié, portant sur l'édition **2020**, non 2023**]**
- « The standard clarifies that the SRP/CS of the Detection of Personnel and the Braking System have to comply with a level of **PLr d** » — [TÜV Rheinland, p. 8](https://www.tuv.com/content-media-files/master-content/services/industrial-services/pdf/tuv-rheinland-automatic-guided-vehicles-whitepaper-en_neu.pdf) **[NON VÉRIFIÉ À LA SOURCE]**
- **Zones** : EN 1525 ne définissait que *Restricted Zone* et *Hazard Zone* ; « now with the ISO 3691-4 additionally the **Confined Zone** is defined », avec classification distinguant moyens de détection du personnel actifs ou inactifs ; Tableau A.1 associe dégagements latéraux, détection active (**PL D**), vitesse max (ex. **0,3 m/s** en mode « muted »), zone requise, fonction d'arrêt atteignable sous 600 mm, et autorisation de redémarrage automatique — [TÜV Rheinland, p. 7](https://www.tuv.com/content-media-files/master-content/services/industrial-services/pdf/tuv-rheinland-automatic-guided-vehicles-whitepaper-en_neu.pdf) **[NON VÉRIFIÉ À LA SOURCE]**
- Durcissement par rapport à EN 1525 : la fonction « Detection of persons in the guide path » exige désormais que « trucks shall **stop** before contact between the rigid parts of the truck or load and a stationary person », là où EN 1525 exigeait seulement de « **generate a signal** enabling the truck to be stopped » — [TÜV Rheinland, p. 8](https://www.tuv.com/content-media-files/master-content/services/industrial-services/pdf/tuv-rheinland-automatic-guided-vehicles-whitepaper-en_neu.pdf) **[NON VÉRIFIÉ À LA SOURCE]**
- Conditions de *muting* / contournement / désactivation des moyens de détection : « Following the requirements of section **4.8.2.3** if such function is possible. The EN 1525 only specifies that the Bypass of Safety Functions has to fulfill Category 2. » — [TÜV Rheinland, p. 7](https://www.tuv.com/content-media-files/master-content/services/industrial-services/pdf/tuv-rheinland-automatic-guided-vehicles-whitepaper-en_neu.pdf) **[NON VÉRIFIÉ À LA SOURCE]**
- **Annexe E (normative)** « contains a useful table that lists each sub clause and the required means for verification from methods including: design check, calculation, inspection (visual or audible), measurement, and functional test » — synthèse de résultats de recherche **[NON VÉRIFIÉ À LA SOURCE]** ; en revanche l'existence et le caractère normatif de l'Annexe E sont **[SOURCE VÉRIFIÉE]** (table des matières officielle).
- Aucune mention de cybersécurité dans le livre blanc TÜV sur ISO 3691-4:2020 (lu intégralement, 310 lignes extraites) — [TÜV Rheinland](https://www.tuv.com/content-media-files/master-content/services/industrial-services/pdf/tuv-rheinland-automatic-guided-vehicles-whitepaper-en_neu.pdf) **[constat de lecture, source tierce]**

### Inferences

- **L'exclusion des chariots télécommandés est le point structurant pour tout raisonnement sur une liaison radio.** Si un véhicule dépend d'une liaison de commande distante pour décider de ses mouvements, il sort du cadre d'ISO 3691-4 ; inversement, un véhicule dans le périmètre d'ISO 3691-4 est un véhicule qui **opère automatiquement**, dont la sûreté repose sur une détection de personnes embarquée en PLr d et un freinage en PLr d — c'est-à-dire sur des moyens **locaux**, indépendants de toute liaison. La norme fait donc reposer la sécurité sur l'autonomie locale plutôt que sur la fiabilité du lien.
- Il en découle que, dans l'architecture visée par ISO 3691-4, **la perte de communication n'est pas un événement dangereux au sens du Tableau 1** : la protection des personnes ne passe pas par le lien. C'est cohérent avec l'absence de clause « Loss of communication » dans l'édition 2023, et c'est une explication plus parcimonieuse que l'hypothèse d'un oubli normatif. **Ceci est une inférence, pas une citation.**
- La citation très répandue d'une « clause 4.18 Loss of communication » dans ISO 3691-4 semble être un **artefact de propagation entre sources secondaires** (blogs de fournisseurs, synthèses automatiques). Toute note destinée à être citée devrait éviter de la reprendre sans vérification sur le texte acheté.
- L'Annexe E étant **normative** et associant à chaque sous-clause un moyen de vérification imposé (design check / calcul / inspection / mesure / essai fonctionnel), c'est elle qui détermine le **régime de preuve** attendu, et non le jugement du concepteur.

### Gaps

- **Le texte des clauses 4.1.26 (Normal stop), 4.1.27 (Operational stop), 4.8.1 (Emergency stop), 4.9.2 (Automatic mode) et 4.11 (SRP/CS) n'a PAS été lu** : l'aperçu officiel gratuit s'arrête à la fin de l'article 3 (définitions, p. 9). **Ce qu'ISO 3691-4:2023 exige précisément en matière de comportement de la commande, de reprise après arrêt et de surveillance du lien, dans le corps des clauses, reste donc non vérifié.** C'est la lacune la plus importante de ces notes et elle ne peut être levée qu'en achetant la norme (≈ 40 € chez EVS, ≈ 1190 CHF/USD selon canal) ou en la consultant en bibliothèque de normalisation.
- Le **Tableau 1 de l'édition 2023** (liste des fonctions de sûreté et PLr) n'a pas été vu ; les valeurs citées (27 fonctions, PLr d pour détection + freinage) proviennent d'un livre blanc portant sur l'**édition 2020** et peuvent avoir changé, l'article 4 ayant été mis à jour.
- Le statut de norme harmonisée d'EN ISO 3691-4:2023 au JOUE (sous 2006/42/CE et/ou sous le règlement (UE) 2023/1230) **n'a pas été vérifié**.
- L'existence et le contenu d'une révision en cours (ISO/DIS 3691-4, réf. 88615) n'ont pas pu être confirmés : **iso.org renvoie systématiquement HTTP 403** aux requêtes automatisées, y compris sur la plateforme de consultation en ligne (OBP).
- Je n'ai trouvé **aucune source fiable** indiquant qu'ISO 3691-4 traite de cybersécurité.

---

## Q4 — ISO 10218-1 et -2 : titre, édition 2025, périmètre, arrêts, et cybersécurité

### Takeaway

Les deux parties d'ISO 10218 ont été **révisées en février 2025** (parties 1 et 2 publiées le même jour).
C'est la seule norme du corpus examiné qui introduit une **exigence de cybersécurité dans son corps
normatif** — sous la forme d'une **évaluation documentée des menaces** limitée à ce qui affecte la sûreté
du robot. Point capital : **IEC 62443 n'y est pas une référence normative**, seulement bibliographique.

### Cited Findings

**Titres et éditions**

- **ISO 10218-1:2025**, « **Robotics — Safety requirements — Part 1: Industrial robots** », **3ᵉ édition**, publiée le **5 février 2025**, 95 pages — [Linus Cent, analyse d'ISO 10218:2025](https://linuscent.com/iso-10218/) **[NON VÉRIFIÉ À LA SOURCE]**
- **ISO 10218-2:2025**, « **Part 2: Industrial robot applications and robot cells** », **2ᵉ édition**, publiée le **5 février 2025**, 223 pages — [Linus Cent](https://linuscent.com/iso-10218/) **[NON VÉRIFIÉ À LA SOURCE]**
- Confirmation indépendante de l'édition : « ISO 10218-1:2025 … published as **Edition 3 in 2025-02** » — [ANSI Blog](https://blog.ansi.org/iso-10218-1-2025-robots-and-robotic-devices-safety/) **[NON VÉRIFIÉ À LA SOURCE]**
  - ⚠️ Noter un **changement de titre** : les éditions antérieures s'intitulaient « *Robots and robotic devices* — Safety requirements for industrial robots » ; l'édition 2025 utilise « **Robotics** — Safety requirements ». Le billet ANSI emploie encore l'ancien libellé dans son propre titre, ce qui est une incohérence de la source secondaire. **Le titre exact de 2025 n'a pas été vérifié sur une page officielle ISO** (iso.org : HTTP 403).

**Périmètre**

- ISO 10218-1:2025 « specifies requirements and guidelines for the inherent safe design, protective measures, and information for use of industrial robots, and describes basic hazards associated with robots and provides requirements to eliminate, or adequately reduce, the risks associated with these hazards » — [ANSI Blog](https://blog.ansi.org/iso-10218-1-2025-robots-and-robotic-devices-safety/) **[NON VÉRIFIÉ À LA SOURCE]**
- « ISO 10218-1:2025 **does not address the robot as a complete machine**, and also **does not apply to non-industrial robots**, although the safety principles established in the ISO 10218 series can be utilized for these other robots » — [ANSI Blog](https://blog.ansi.org/iso-10218-1-2025-robots-and-robotic-devices-safety/) **[NON VÉRIFIÉ À LA SOURCE]**
- Applications couvertes citées : soudage à l'arc, assemblage, collage, desserte de machine, manutention, enlèvement de matière, découpe mécanique, peinture, picking, emballage, palettisation, brasage, soudage par points, découpe jet d'eau — [ANSI Blog](https://blog.ansi.org/iso-10218-1-2025-robots-and-robotic-devices-safety/) **[NON VÉRIFIÉ À LA SOURCE]**

**Cybersécurité — contenu normatif**

- Le *foreword* liste parmi les changements majeurs « **adding requirements for cybersecurity to the extent that it applies to industrial robot safety** » — [Linus Cent](https://linuscent.com/iso-10218/) **[NON VÉRIFIÉ À LA SOURCE]**
- Localisation des clauses : **5.1.16** (partie 1) et **5.2.16** (partie 2) = exigences cybersécurité ; **5.3.6** (partie 1) et **5.5.9** (partie 2) = exigences de communication ; **7.5.11** (partie 1) et **7.5.23** (partie 2) = information cybersécurité à porter dans la notice d'instructions transmise aux intégrateurs et utilisateurs — [Linus Cent](https://linuscent.com/iso-10218/) **[NON VÉRIFIÉ À LA SOURCE]**
- Nature de l'exigence : « a **documented cybersecurity threat assessment** », méthodologiquement analogue à une appréciation du risque ISO 12100 (identifier les menaces, évaluer le risque, en déduire des mesures de protection). Les mesures concrètes — « disabling ports, changing default credentials, authenticated protection of the safety configuration, encrypted protocols, secure updates » — « are cited as **notes and guidance rather than mandatory requirements** » — [Linus Cent](https://linuscent.com/iso-10218/) **[NON VÉRIFIÉ À LA SOURCE]**
- **Statut des référentiels de cybersécurité** : « **IEC 62443, IEC/TR 63074 and ISO/TR 22100-4 are cited in the bibliography only** » ; ils « appear in the bibliography of both parts, **not in the normative references** ». Conséquence explicitée par la source : « **ISO 10218:2025 does not normatively require IEC 62443 conformity.** » — [Linus Cent](https://linuscent.com/iso-10218/) **[NON VÉRIFIÉ À LA SOURCE** — point important et plausible, mais reposant sur une **source unique** ; à recouper**]**

### Inferences

- La formule « **to the extent that it applies to industrial robot safety** » délimite exactement la frontière cherchée dans ce travail : ISO 10218:2025 n'importe pas la cybersécurité comme objectif propre, elle l'importe **comme source de menaces susceptibles d'invalider les hypothèses de l'analyse de risque sécurité**. La cybersécurité y est une **entrée** de l'appréciation du risque, non un attribut de la fonction de sûreté.
- Le fait que les contre-mesures concrètes soient placées en notes informatives et qu'IEC 62443 reste bibliographique signifie qu'aucun niveau de sécurité (SL d'IEC 62443) n'est exigible ni convertible en PL/SIL par cette norme. Un robot peut être conforme à ISO 10218-1:2025 **sans** satisfaire IEC 62443, pourvu que l'évaluation des menaces soit documentée.
- ISO 10218-1 ne traitant pas « the robot as a complete machine » et ne s'appliquant pas aux robots non industriels, **elle n'est pas la norme de référence d'un véhicule ou robot mobile autonome** : pour ce dernier, ISO 3691-4 (chariots sans conducteur) ou ISO 13482 (robots de service personnel, cité dans les exclusions d'ISO 3691-4) sont les candidats. **Estimation d'applicabilité**, à confirmer selon le cas d'usage réel.

### Gaps

- **Aucune clause d'ISO 10218-1/-2:2025 n'a été vérifiée à la source** : iso.org bloque l'accès automatisé (HTTP 403) et je n'ai pas trouvé d'aperçu officiel gratuit pour ces deux parties. **Toute la section Q4 repose sur des sources tierces, dont une source unique pour les numéros de clause et le statut bibliographique d'IEC 62443.** À recouper impérativement avant citation.
- **Ce que disent ISO 10218-1/-2:2025 de l'arrêt d'urgence et de l'arrêt protecteur n'a pas pu être établi.** La source consultée indique explicitement ne rien contenir sur ce point : « The provided content contains no specific discussion of emergency stop, protective stop terminology, or whether security measures constitute safety functions » — [Linus Cent](https://linuscent.com/iso-10218/). Les éditions antérieures (ISO 10218-1:2011) distinguaient classiquement arrêt d'urgence et arrêt protecteur avec des catégories d'arrêt associées, mais **je ne reproduis pas ces exigences de mémoire** et n'ai pu vérifier ni leur contenu ni leur reconduction en 2025.
- Le statut harmonisé UE d'EN ISO 10218-1/-2:2025, et la coexistence transitoire avec ISO/TS 15066 (robots collaboratifs), n'ont pas été vérifiés.
- Le titre officiel exact de l'édition 2025 n'est pas confirmé par une page officielle.

---

## Q5 — Existe-t-il une exigence explicite qu'une fonction d'arrêt ne puisse pas être empêchée par une autre fonction ?

### Takeaway

**Oui, et c'est vérifiable verbatim.** ISO 13850:2015, clause **4.1.1.2**, exige que la fonction d'arrêt
d'urgence soit « disponible et opérationnelle à tout moment » et qu'elle « **prime sur toutes les autres
fonctions et opérations dans tous les modes de fonctionnement** », et la clause **4.1.1.4** exige
réciproquement qu'elle « **ne dégrade pas l'efficacité des autres fonctions de sûreté** ». ISO 13850:2015
étant une **référence normative d'ISO 3691-4:2023**, ces exigences s'appliquent par renvoi aux AGV/AMR.

### Cited Findings

**[SOURCE VÉRIFIÉE — texte normatif lu dans l'aperçu officiel ISO 13850:2015, citations verbatim]**

- > **4.1.1.2** « The emergency stop function **shall be available and operational at all times**. It **shall override all other functions and operations in all operating modes of the machine without impairing other protective functions** (e.g. release of trapped persons, fire suppression). »
- > (suite de 4.1.1.2) « When the emergency stop function is activated: — it **shall be maintained until it is manually reset**; — it **shall not be possible for any start command to be effective** on those operations stopped by the initiation of the emergency stop function. » et « The emergency stop function shall be **reset by intentional human action**. »
- > **4.1.1.3** « The emergency stop function is a **complementary protective measure** and **shall not be applied as a substitute for safeguarding measures** and other functions or safety functions. »
- > **4.1.1.4** « The emergency stop function **shall not impair the effectiveness of other safety functions**. » — NOTE : « For this purpose, it can be necessary to ensure the continuing operation of auxiliary equipment such as magnetic chucks or braking devices. »
- > **4.1.1.5** « The emergency stop function shall be so designed, that after actuation of the emergency stop device, hazardous movements and operations of the machine are stopped in an appropriate manner, **without creating additional hazards and without any further intervention**. »
- > **4.1.1.6** « The emergency stop function shall be so designed that **a decision to activate the emergency stop device does not require the consideration of the resultant effects**. »
- > **4.1.1.1** (fragment) « The emergency stop function is to be initiated by a **single human action**. »
- Périmètre : « specifies **functional requirements and design principles for the emergency stop function on machinery, independent of the type of energy used** » ; ne s'applique pas aux machines « where an emergency stop would not reduce the risk » ni aux machines « hand-held or hand-operated » ; NOTE : « The requirements for the realization of the emergency stop function based on **electrical/electronic technology are described in IEC 60204-1**. »
- Références normatives d'ISO 13850:2015 : incluent **IEC 60204-1:2005** et **IEC 62061**.
- Table des matières : 4.1.1 Emergency stop function ; 4.1.2 Span of control of emergency stop device(s) ; 4.1.3 Stop categories ; 4.1.4 Disengagement ; 4.1.5 Emergency stop equipment ; 4.2 Operating conditions, environmental influences ; 4.3 Emergency stop device ; 4.4 Use of wires or ropes as actuators ; **4.5 Prevention of unintended actuation of an emergency stop device** ; **4.6 Portable operator control stations** (4.6.1 Emergency stop functions on portable operator control stations ; **4.6.2 Emergency stop reset for cableless operator control stations**).
  — toutes citations : [aperçu officiel ISO 13850:2015 (PDF sample ISO/iTeh, réf. 59970)](https://cdn.standards.iteh.ai/samples/59970/06ad76004ab746ff90b82cf44a8b0752/ISO-13850-2015.pdf)

**Renvoi applicable aux AGV/AMR [SOURCE VÉRIFIÉE]**

- « **ISO 13850:2015, Safety of machinery — Emergency stop function — Principles for design** » figure dans la **liste des références normatives d'ISO 3691-4:2023** (clause 2), et la définition 3.16 d'ISO 3691-4 est explicitement sourcée « [SOURCE: ISO 13850:2015, 3.3] » — [aperçu officiel ISO 3691-4:2023](https://cdn.standards.iteh.ai/samples/83545/a3d9d057a08d4f9c8e8e87cdc947583c/ISO-3691-4-2023.pdf)

**Corroboration IEC 60204-1 [NON VÉRIFIÉ À LA SOURCE]**

- « According to IEC 60204-1, the emergency stop function **shall override all other functions and operations in all modes** » ; « the emergency stop overrides all other functions and **does not replace the main switch** » ; la puissance vers les actionneurs est soit retirée immédiatement (**catégorie d'arrêt 0**), soit maîtrisée pour arrêter le mouvement dangereux aussi vite que possible (**catégorie d'arrêt 1**) « without creating other hazards » — [Kollmorgen, note technique « Stop and emergency stop function »](https://www.kollmorgen.com/en-us/developer-network/stop-and-emergency-stop-function) **[NON VÉRIFIÉ À LA SOURCE** — documentation constructeur ; concordante avec le texte vérifié d'ISO 13850 4.1.1.2**]**
- Formulations concordantes d'un expert reconnu du domaine et d'un billet ANSI : l'arrêt d'urgence « must override all other control functions, and **no start functions are permitted until the emergency stop has been reset** » ; son usage « **cannot impair the operation of any functions of the machine intended for the release of trapped persons** » ; « It is **not permitted to affect the function of any other safety critical systems or devices** » — [ANSI Blog sur ISO 13850](https://blog.ansi.org/ansi/iso-13850-safety-of-machinery-emergency-stop/) et [machinerysafety101 (D. Nix)](https://machinerysafety101.com/2026/05/18/iso-13850-emergency-stop-requirements/) **[NON VÉRIFIÉ À LA SOURCE, mais concordant avec le verbatim 4.1.1.2 / 4.1.1.4 vérifié]**

**Exigences voisines dans ISO 13849-1:2023 [SOURCE VÉRIFIÉE via IFA/DGUV]**

- La spécification d'une fonction de sûreté doit désormais inclure « **prioritisation of different safety functions that can be enabled at the same time and can trigger conflicting responses** » — [IFA/DGUV, §5](https://publikationen.dguv.de/widgets/pdf/download/article/4894)
- « the standard specifies **additional requirements for the selection of the operating mode, with a view to preventing any negative impacts on other safety functions** » — [IFA/DGUV, §5](https://publikationen.dguv.de/widgets/pdf/download/article/4894)
- Nouvelle sous-section **5.2.3** : « requires designers to **minimise any motivation to defeat safety functions** » — [IFA/DGUV, §5](https://publikationen.dguv.de/widgets/pdf/download/article/4894)
- La spécification doit aussi couvrir « conditions permitting a **restart** after the safety function has been requested » et « behaviour of the machine in the event of **energy loss** » — [IFA/DGUV, §5](https://publikationen.dguv.de/widgets/pdf/download/article/4894)

### Inferences

- La réponse à la question est **oui, de manière bidirectionnelle et explicite** : 4.1.1.2 interdit qu'une autre fonction empêche l'arrêt d'urgence ; 4.1.1.4 interdit que l'arrêt d'urgence dégrade une autre fonction de sûreté. Ce couple d'exigences est le point d'appui normatif le plus net disponible.
- 4.1.1.3 est particulièrement significatif pour la frontière étudiée : l'arrêt d'urgence est une **mesure de protection complémentaire** et ne peut **pas** se substituer aux mesures de protection. Par symétrie de raisonnement, aucune mesure placée en aval d'un danger ne peut tenir lieu de réduction du risque à la source. **Inférence, non citation.**
- 4.1.1.6 (« la décision d'activer l'arrêt d'urgence ne doit pas exiger de considérer les effets résultants ») interdit en pratique qu'un arrêt soit conditionné à une évaluation, une négociation ou une autorisation préalable. Tout composant qui s'interposerait entre la demande d'arrêt et son exécution — y compris pour vérifier une signature, un droit ou une fraîcheur — contreviendrait à l'esprit de 4.1.1.2 et 4.1.1.6 s'il pouvait retarder ou refuser l'arrêt. **Inférence de lecture, à confronter au texte intégral et à IEC 60204-1.**
- L'existence d'une clause **4.6.2 « Emergency stop reset for cableless operator control stations »** montre que le cas du lien sans fil est traité par ISO 13850 — mais du côté du **réarmement**, pas de la perte de lien. Son contenu n'a pas été lu.

### Gaps

- L'aperçu officiel d'ISO 13850:2015 **s'arrête à la clause 4.1.1.6** (fin de la page 3). Les clauses **4.1.2 (span of control), 4.1.3 (stop categories), 4.5 (prevention of unintended actuation) et 4.6 (portable / cableless operator control stations)** n'ont **pas** été lues. Pour la question d'une liaison radio, **4.6 est précisément la clause qu'il faudrait lire** et elle reste non vérifiée.
- Le texte d'**IEC 60204-1** (édition en vigueur : 2016, à confirmer) n'a pas été vérifié à la source ; la numérotation exacte de la clause d'arrêt d'urgence n'est donc pas citable. ISO 13850:2015 référence encore **IEC 60204-1:2005**, référence datée désormais ancienne.
- Je n'ai pas pu vérifier si ISO 3691-4:2023 ou ISO 10218:2025 ajoutent leurs propres exigences de priorité d'arrêt au-delà du renvoi à ISO 13850.

---

## Q6 — Un composant logiciel qui AUTORISE ou REFUSE une commande sans être une fonction de sûreté : comment les normes le traitent-elles ?

### Takeaway

Le corpus ne contient pas de catégorie « composant d'autorisation non lié à la sécurité ». Il contient en
revanche une règle de bascule très nette, vérifiée à la source : **dès qu'une fonction active ou désactive
une fonction de sûreté, elle devient elle-même une fonction de sûreté** et doit être conçue au PL requis.
Le choix architectural est donc binaire : ou bien le composant ne peut pas influer sur une fonction de
sûreté, ou bien il tombe dans le périmètre de la norme avec toutes ses exigences.

### Cited Findings

**La règle de bascule — [SOURCE VÉRIFIÉE via IFA/DGUV, citation verbatim]**

- > « **If operating mode selection enables or disables safety functions, it is treated as a safety function in its own right.** »
  — [IFA/DGUV, « Fourth edition of EN ISO 13849-1 – new features », §5](https://publikationen.dguv.de/widgets/pdf/download/article/4894)
- Contexte immédiat de cette phrase : « As a new element, the standard specifies additional requirements for the selection of the operating mode, **with a view to preventing any negative impacts on other safety functions**. » — [IFA/DGUV, §5](https://publikationen.dguv.de/widgets/pdf/download/article/4894)

**Cohabitation de fonctions de sûreté et de fonctions non liées à la sécurité**

- Observation d'un organisme notifié sur ISO 3691-4 : « The Control System of the AGV/AGC **needs to deal with Safety and non-Safety Functions**, taking into account that **some Safety Functions may affect Safety as well as Non-Safety ones**. » — [TÜV Rheinland, p. 6](https://www.tuv.com/content-media-files/master-content/services/industrial-services/pdf/tuv-rheinland-automatic-guided-vehicles-whitepaper-en_neu.pdf) **[NON VÉRIFIÉ À LA SOURCE** — commentaire d'un organisme notifié, pas texte normatif**]**
- Contournement/neutralisation : le *bypass* d'une fonction de sûreté est encadré (conditions de *muting*, override ou désactivation des moyens de détection du personnel en **4.8.2.3** d'ISO 3691-4 ; EN 1525 exigeait la catégorie 2 pour le bypass) — [TÜV Rheinland, p. 7](https://www.tuv.com/content-media-files/master-content/services/industrial-services/pdf/tuv-rheinland-automatic-guided-vehicles-whitepaper-en_neu.pdf) **[NON VÉRIFIÉ À LA SOURCE]**
- ISO 13849-1:2023 **5.2.3** : obligation de « minimise any motivation to defeat safety functions », avec prise en compte précoce de la faisabilité pratique en usage — [IFA/DGUV, §5](https://publikationen.dguv.de/widgets/pdf/download/article/4894) **[SOURCE VÉRIFIÉE via IFA/DGUV]**
- Logiciel embarqué non accessible : « The boundary conditions for the option of using components with **non-accessible embedded software** have been updated… The authors have clarified that the **associated hardware and existing SRASW must meet the requirements of the standard**. » — [IFA/DGUV, §7](https://publikationen.dguv.de/widgets/pdf/download/article/4894) **[SOURCE VÉRIFIÉE via IFA/DGUV]**

**Le document dédié à la frontière sûreté / sécurité**

- **IEC TR 63074:2019**, devenu **IEC TS 63074:2023** — « Safety of machinery — Security aspects related to functional safety of safety-related control systems ». Objet : « gives guidance on the use of **IEC 62443 (all parts)** related to those aspects of **security threats and vulnerabilities that could influence functional safety** implemented and realized by safety-related control systems (SCS) and could lead to the **loss of the ability to maintain safe operation** of a machine » — [SIS (organisme suédois de normalisation), fiche IEC TR 63074:2019](https://sis.se/produkter/elektroteknik-24c2329a/allmant/iec-tr-630742019) **[NON VÉRIFIÉ À LA SOURCE]**
- Aspects considérés : « vulnerabilities of the SCS either directly or indirectly through the other parts of the machine which can be exploited by security threats that can result in security attacks, and **influence on the safety characteristics and ability of the SCS to properly perform its function(s)** » — [SIS / fiches de normalisation](https://sis.se/produkter/elektroteknik-24c2329a/allmant/iec-tr-630742019) **[NON VÉRIFIÉ À LA SOURCE]**
- **Limite de périmètre importante** : « The focus of this document is on **intentional malicious actions**, but **intentional hardware manipulation or foreseeable misuse by physical manipulation of SCS is not considered** in this document. » — [fiches IEC TS 63074:2023 (SCC/CCN, genorma)](https://scc-ccn.ca/standardsdb/standards/2052244) **[NON VÉRIFIÉ À LA SOURCE]**
- Confirmation de l'existence et du millésime par une source officielle : la bibliographie de la publication IFA/DGUV liste « **IEC/TS 63074**: Safety of machinery — Security aspects related to functional safety of safety-related control systems (**02.23**) » — [IFA/DGUV, bibliographie](https://publikationen.dguv.de/widgets/pdf/download/article/4894) **[SOURCE VÉRIFIÉE]**
- Côté robotique, c'est le même triptyque qui est renvoyé en bibliographie : IEC 62443, IEC/TR 63074, ISO/TR 22100-4 — [Linus Cent](https://linuscent.com/iso-10218/) **[NON VÉRIFIÉ À LA SOURCE]**

### Inferences

- **Les deux objets ne sont pas de même nature, et les normes le disent par construction :**
  1. **Objet de la preuve.** Un PL ou un SIL qualifie l'aptitude d'une fonction à être exécutée malgré des **défaillances aléatoires et systématiques** ; il se démontre par architecture, MTTFD, DC, CCF et PFH (Q1). Une mesure de cybersécurité qualifie la résistance à un **adversaire intentionnel** ; elle ne se démontre pas par des taux de défaillance. Aucune des deux grandeurs n'est convertible dans l'autre.
  2. **Exclusion explicite.** ISO 13849-1:2023 déclare que les aspects de sécurité informatique **ne sont pas couverts** par la norme (Q1, citation verbatim IFA/DGUV). Un composant de cybersécurité ne peut donc pas recevoir de PL *au titre de ses propriétés de sécurité* : ce que la norme pourrait qualifier, c'est au mieux l'intégrité de son exécution en tant que sous-système logique, et seulement s'il est conçu comme SRP/CS.
  3. **Relation de dépendance, non d'équivalence.** IEC TS 63074 traite la cybersécurité comme un facteur pouvant **faire perdre** à un SCS sa capacité à maintenir un état sûr. La sécurité est une **condition de validité des hypothèses** de la démonstration de sûreté, pas un élément de cette démonstration.
  4. **Statut non normatif des référentiels de sécurité.** IEC 62443, IEC/TR 63074 et ISO/TR 22100-4 sont en bibliographie dans ISO 10218:2025, et en renvoi informatif dans ISO 13849-1:2023. Aucune des quatre normes examinées n'impose la conformité à un niveau de sécurité (SL), et aucune ne reconnaît un SL comme équivalent à un PL ou un SIL.
- **Conséquence architecturale pour un composant qui autorise ou refuse une commande** (inférence, formulée comme une règle de conception, non comme une citation) : la règle de bascule vérifiée en 5.x d'ISO 13849-1 implique qu'un tel composant est acceptable **hors** périmètre de sûreté seulement s'il ne peut **ni** empêcher **ni** retarder l'exécution d'une fonction de sûreté — en particulier d'un arrêt. Sa latitude doit être limitée à **refuser une action**, jamais à **autoriser un arrêt** ni à conditionner celui-ci. Un arrêt d'urgence doit rester câblé ou réalisé par un chemin indépendant, conformément à ISO 13850 4.1.1.2 et 4.1.1.6 (Q5). Dès qu'un tel composant devient capable d'inhiber, de *muter* ou de reconfigurer une fonction de sûreté, il entre dans le périmètre SRP/CS et doit être conçu, vérifié et validé au PLr applicable.
- **Formulation honnête disponible** pour décrire un composant de cybersécurité : « mesure de sécurité contribuant à préserver les hypothèses de l'analyse de risque », ou « mesure de protection au sens d'IEC TS 63074 ». **Jamais** « fonction de sûreté PL x » ou « certifiée SIL y » sans évaluation par un organisme compétent selon ISO 13849-1/-2 ou IEC 62061.

### Gaps

- **Je n'ai pas trouvé, dans aucune des quatre normes, de clause générale explicitement intitulée ou formulée comme « les fonctions non liées à la sécurité ne doivent pas dégrader les fonctions de sûreté ».** Ce qui est vérifié est plus étroit mais plus utile : la règle de bascule sur la sélection du mode de fonctionnement, l'exigence de priorisation des fonctions de sûreté concurrentes, et l'obligation de minimiser la motivation à neutraliser. **Il est possible qu'une telle exigence générale existe** (candidats plausibles : ISO 12100:2010 6.2.11, ISO 13849-1:2023 article 5 ou 6.1, IEC 60204-1 article 9) **mais je ne l'ai pas vérifiée et je ne l'affirme pas.** C'est la vérification la plus rentable à faire sur les textes achetés.
- Le texte d'**IEC TS 63074:2023** n'a pas été lu ; il n'a pas de page de périmètre librement consultée dans cette recherche (seulement des fiches de catalogue d'organismes nationaux). Ses recommandations concrètes sur l'interface sûreté/sécurité, qui sont le cœur du sujet, restent **non vérifiées**.
- **ISO/TR 22100-4** (« Guidance to machinery manufacturers for consideration of related IT-security (cyber security) aspects » — libellé **non vérifié**) n'a pas été consulté. Son édition en vigueur n'est pas établie.
- La question de savoir si IEC 62061:2021 contient une clause ou note sur la sécurité informatique n'est pas résolue (voir Q2).

---

## Q7 — Récapitulatif du statut de vérification et des éditions en vigueur au 2026-10-07

### Takeaway

Sur les six normes principales, **deux ont été lues partiellement dans leur texte normatif** via des aperçus
officiels gratuits (ISO 3691-4:2023 et ISO 13850:2015), **une** a sa page de périmètre officielle vérifiée
(IEC 62061:2021), **une** est documentée par une publication officielle d'institut (ISO 13849-1:2023 via
IFA/DGUV), et **une repose entièrement sur des sources tierces** (ISO 10218:2025). iso.org bloque l'accès
automatisé (HTTP 403), y compris sa plateforme de consultation en ligne.

### Cited Findings

| Norme | Édition en vigueur | Révision récente / en cours | Statut de vérification |
|---|---|---|---|
| **ISO 13849-1** | **2023** (4ᵉ éd.), publiée 2023-04-26 | Édition 2023 récente ; EN citée au JOUE 2024-05-15, présomption de conformité de l'éd. 2015 perdue le **2027-05-15** | Texte **non lu**. Documenté par publication officielle **[IFA/DGUV vérifiée]** + catalogues |
| **ISO 13849-2** | **2012** (2ᵉ éd., 2012-10), confirmée 2018 | **Statut 90.92 « to be revised »** ; ISO/DIS 13849-2 en cours, titre modifié en « Application of principles for the design and validation » | Texte **non lu** ; pages catalogue seulement |
| **IEC 62061** | **2021**, éd. **2.0**, 2021-03-22 | Éd. 2.0 a supprimé la restriction « electrical, electronic and programmable electronic » du titre | **Page de périmètre officielle IEC vérifiée** ; texte non lu |
| **ISO 3691-4** | **2023** (2ᵉ éd., **2023-06**) ; EN ISO 3691-4:2023 valide depuis 2023-08-01 | Éd. 2020 retirée ; **une ISO/DIS 3691-4 (réf. 88615) semble en cours — non confirmé** | **Foreword, Scope, table des matières et définitions (art. 1–3) LUS** dans l'aperçu officiel ; **corps des clauses 4 à 6 non lu** |
| **ISO 10218-1** | **2025** (3ᵉ éd., 2025-02-05) | Révision **très récente** (février 2025), ajoute la cybersécurité | **Rien vérifié à la source** ; sources tierces uniquement |
| **ISO 10218-2** | **2025** (2ᵉ éd., 2025-02-05) | idem | **Rien vérifié à la source** |
| **ISO 13850** | **2015** | — | **Clauses 1, 2, 4.1.1.1–4.1.1.6 et table des matières LUES** dans l'aperçu officiel |
| **IEC 63074** | **TS 2023** (ex-**TR 2019**) | Passage de TR à TS en **02.2023** | Existence et millésime **vérifiés** via bibliographie IFA/DGUV ; contenu **non lu** |

- Sources des éditions : [IEC Webstore 59927](https://webstore.iec.ch/publication/59927) ; [aperçu officiel ISO 3691-4:2023](https://cdn.standards.iteh.ai/samples/83545/a3d9d057a08d4f9c8e8e87cdc947583c/ISO-3691-4-2023.pdf) ; [aperçu officiel ISO 13850:2015](https://cdn.standards.iteh.ai/samples/59970/06ad76004ab746ff90b82cf44a8b0752/ISO-13850-2015.pdf) ; [IFA/DGUV](https://publikationen.dguv.de/widgets/pdf/download/article/4894) ; [EVS](https://evs.ee/en/evs-en-iso-3691-4-2023) ; [DIN Media / ISO 53640](https://dinmedia.de/en/standard/iso-13849-2/167356377) ; [ANSI Blog ISO 10218-1:2025](https://blog.ansi.org/iso-10218-1-2025-robots-and-robotic-devices-safety/) ; [Linus Cent](https://linuscent.com/iso-10218/) ; [CEN-CENELEC](https://www.cencenelec.eu/news-events/news/2023/eninthespotlight/2023-05-25-en-iso-13849-1-2023/) ; [IBF Solutions](https://www.ibf-solutions.com/en/seminars-and-news/news/new-en-iso-13849-12023).

### Inferences

- **Les aperçus officiels « ISO sample » sont le meilleur levier de vérification gratuite** et ont été décisifs ici : ils contiennent systématiquement le *foreword* (donc la liste des changements d'édition), la clause 1 (Scope) *in extenso*, la clause 2 (références normatives) et la clause 3 (définitions), ainsi que la **table des matières complète** — ce qui suffit à infirmer une affirmation sur l'existence d'une clause. Pour toute norme payante restante, c'est la voie à tenter en premier.
- iso.org étant inaccessible aux requêtes automatisées (HTTP 403 sur `/standard/*`, `/obp/ui/*` et `/contents/data/*`), les pages de périmètre officielles ISO ne sont **pas** la ressource « vérifiable gratuitement » attendue dans cette recherche ; webstore.iec.ch, en revanche, répond normalement.

### Gaps

- **Aucune information du Journal officiel de l'UE n'a été vérifiée à la source.** Les décisions d'exécution citant les normes harmonisées sous la directive 2006/42/CE et sous le **règlement (UE) 2023/1230** n'ont pas été consultées. Les dates de citation et la liste des normes harmonisées restent donc **non vérifiées** (origine : CEN-CENELEC et IBF Solutions, sources secondaires).
- Je n'ai **pas** établi quelles normes de ce corpus sont harmonisées sous le **règlement Machines (UE) 2023/1230** par opposition à l'ancienne directive, ni le calendrier d'application du règlement. C'est une lacune significative pour une analyse de conformité réglementaire et elle demande une consultation directe d'EUR-Lex.
- **Le corps normatif d'ISO 3691-4:2023 (clauses 4 à 6, Annexes A et E) est la pièce manquante la plus importante** de l'ensemble de ces notes, en particulier pour la question de la perte de communication.
