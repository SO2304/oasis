# Feuille de route post-quantique européenne et tailles de signatures PQC face aux contraintes d'une trame LoRa

> Date de rédaction : 2026-10-07. Toutes les valeurs numériques sont tirées de
> sources primaires (FIPS, RFC, spécification LoRa Alliance, norme ETSI, documents
> officiels UE/ANSSI/BSI). Les calculs sont explicités (formule + valeurs) et
> étiquetés **[calcul]**. Les documents non lus à la source sont signalés
> « non vérifié à la source ».

---

## Q1 — Feuille de route officielle de l'UE pour la migration post-quantique : dates cibles et périmètre

### Takeaway

L'UE a un dispositif à deux étages : une **Recommandation (UE) 2024/1101 du 11 avril 2024** (acte non contraignant de la Commission) qui demande la création d'une feuille de route coordonnée, et la **feuille de route elle-même**, publiée par le *EU PQC Workstream* du groupe de coopération NIS (« NIS Cooperation Group ») le **11.06.2025** (Partie 1, version 1.1), qui fixe trois jalons fermes : **31.12.2026**, **31.12.2030** et **31.12.2035**.

### Cited Findings

**Recommandation de la Commission (acte fondateur)**

- Titre et date exacts : « Commission Recommendation (EU) 2024/1101 of 11 April 2024 on a Coordinated Implementation Roadmap for the transition to Post-Quantum Cryptography » — [EUR-Lex, OJ L 2024/1101](https://eur-lex.europa.eu/legal-content/EN/TXT/?uri=OJ%3AL_202401101)
- Mécanisme institutionnel : le §4 demande aux États membres de « coordinate their actions at Union level through a dedicated Member States forum », mis en œuvre comme un **sous-groupe du groupe de coopération NIS** — [EUR-Lex 2024/1101](https://eur-lex.europa.eu/legal-content/EN/TXT/?uri=OJ%3AL_202401101)
- Hybridation : le §6 promeut un déploiement « via hybrid schemes that may combine Post-Quantum Cryptography with existing cryptographic approaches or with Quantum Key Distribution » — [EUR-Lex 2024/1101](https://eur-lex.europa.eu/legal-content/EN/TXT/?uri=OJ%3AL_202401101)
- Délai de production de la feuille de route : §7, « the Post-Quantum Cryptography Coordinated Implementation Roadmap should be available after a period of two years following the publication of this Recommendation » (soit avril 2026) — [EUR-Lex 2024/1101](https://eur-lex.europa.eu/legal-content/EN/TXT/?uri=OJ%3AL_202401101)
- Réévaluation : §11, évaluation « maximum three years after its publication » — [EUR-Lex 2024/1101](https://eur-lex.europa.eu/legal-content/EN/TXT/?uri=OJ%3AL_202401101)

**Feuille de route NIS CG (document opérationnel, dates fermes)**

- Identification exacte du document lu à la source : « A Coordinated Implementation Roadmap for the Transition to Post-Quantum Cryptography — **Part 1, Version: 1.1, EU PQC Workstream**, 11.06.2025 » — [PDF (EC newsroom)](https://ec.europa.eu/newsroom/dae/redirection/document/117507) ; page de référence : [Commission européenne / Digital Strategy](https://digital-strategy.ec.europa.eu/en/library/coordinated-implementation-roadmap-transition-post-quantum-cryptography)
- **Jalon 1 — par le 31.12.2026** (citation textuelle de l'encadré « Timeline for the transition to PQC ») :
  - « At least the First Steps have been implemented by all Member States. »
  - « Initial national PQC transition roadmaps have been established by all Member States. »
  - « PQC transition planning and pilots for high- and medium-risk use cases have been initiated. » — [Roadmap Part 1 v1.1, §4](https://ec.europa.eu/newsroom/dae/redirection/document/117507)
- **Jalon 2 — par le 31.12.2030** :
  - « The Next Steps have been implemented by all Member States. »
  - « **The PQC transition for high-risk use cases has been completed.** »
  - « PQC transition planning and pilots for medium-risk use cases have been completed. »
  - « **Quantum-safe software and firmware upgrades are enabled by default.** » — [Roadmap Part 1 v1.1, §4](https://ec.europa.eu/newsroom/dae/redirection/document/117507)
- **Jalon 3 — par le 31.12.2035** :
  - « The PQC transition for medium-risk use cases has been completed. »
  - « The PQC transition for low-risk use cases has been completed as much as feasible. » — [Roadmap Part 1 v1.1, §4](https://ec.europa.eu/newsroom/dae/redirection/document/117507)
- Règle normative sur l'usage du classique seul : « For high-risk use cases, quantum-vulnerable public-key mechanisms **shall not be used stand-alone after the end of 2030**, analogously after the end of 2035 for medium-risk use cases » — [Roadmap Part 1 v1.1, §4.1](https://ec.europa.eu/newsroom/dae/redirection/document/117507)
- Recommandation d'hybridation : « it is recommended to use **standardised and tested hybrid solutions, whenever feasible and suitable** […] replacing it by a standardized hybrid combination which includes PQC should be considered » — [Roadmap Part 1 v1.1, §4.1](https://ec.europa.eu/newsroom/dae/redirection/document/117507)
- Périmètre / infrastructures visées : le document cible les administrations centrales, régionales et locales « as well as all providers of critical infrastructures » et « notably entities in scope of the **NIS 2 Directive** » — [Roadmap Part 1 v1.1](https://ec.europa.eu/newsroom/dae/redirection/document/117507)
- Portée des jalons : la FAQ officielle précise que « The milestones and deadlines set out in the EU Roadmap on PQC apply to the **deployments** » (et non seulement au développement produit) — [FAQ, Roadmap on PQC, 15.04.2026, §4.1](https://ec.europa.eu/newsroom/dae/redirection/document/132120)
- Confirmation de la date 2030 dans la FAQ : « high-risk cases must be migrated by the **end of 2030** » — [FAQ 15.04.2026, §2](https://ec.europa.eu/newsroom/dae/redirection/document/132120)
- Définition UE d'un « hybride » : « a hybrid is defined as a combination of a post-quantum [mechanism with a classical one] » ; la FAQ précise explicitement que l'UE « does **not** consider hybrid mechanisms using quantum key distribution (QKD) » dans cette définition — [FAQ 15.04.2026, §3.1](https://ec.europa.eu/newsroom/dae/redirection/document/132120)
- Pas de liste d'algorithmes imposée : « The NIS CG workstream on PQC does **not** maintain a list of recommended (hybrid) PQC algorithms. It does, however, recommend that standardised hybrid solutions are [used] » — [FAQ 15.04.2026, §3.4](https://ec.europa.eu/newsroom/dae/redirection/document/132120)
- Signatures hybrides vs KEM hybrides — nuance importante pour un nœud contraint : « there are different guidelines for hybrid key encapsulation mechanisms (KEMs) and for hybrid signature schemes. **Hybrid KEMs are easier to work with than hybrid** [signatures] […] In contrast, hybrid signature [schemes are more problematic] » — [FAQ 15.04.2026, §3.3](https://ec.europa.eu/newsroom/dae/redirection/document/132120)
- Alignement international revendiqué : la date 2035 « is based on the goal set out in the US National [Security Memorandum] » et sur le calendrier publié par le NCSC britannique — [Roadmap Part 1 v1.1, §4.2](https://ec.europa.eu/newsroom/dae/redirection/document/117507)

### Inferences

- La feuille de route UE est **non contraignante en elle-même** (Recommandation + document de groupe de coopération), mais son articulation avec la directive NIS 2 lui donne un effet pratique sur tout opérateur d'infrastructure critique : la date opérationnelle à retenir pour un équipement industriel/critique est **31.12.2030**, et **31.12.2035** pour le reste.
- La clause « quantum-safe software and firmware upgrades are **enabled by default** » au jalon 2030 porte directement sur les nœuds embarqués : c'est une exigence de **capacité de mise à jour signée post-quantique**, pas seulement de chiffrement de données. Un produit à longue durée de vie doit donc être *upgradable* avant 2030, même si la signature PQC n'est pas activée dès le départ.
- La distinction explicite de la FAQ entre KEM hybrides (faciles) et signatures hybrides (difficiles) correspond exactement au point de friction technique sur un lien LoRa : ce sont les **signatures** qui ne passent pas, pas l'échange de clés.

### Gaps

- La feuille de route est publiée en « Part 1 » ; je n'ai pas trouvé de « Part 2 » publiée à la date du 2026-10-07. Les documents « First Steps » et « Next Steps » sont cités comme documents séparés du workstream mais je ne les ai pas lus à la source.
- La définition précise et chiffrée de « high-risk » / « medium-risk » / « low-risk » (scores de risque quantique) figure au §3 et §4.2 du document ; je n'ai extrait que la mention de la méthode de score (« Calculating the quantum risk score ») et le seuil temporel « confidentialité devant rester protégée au-delà de 2035 ⇒ score d'impact de 3 », sans pouvoir citer la grille complète. À vérifier avant de classer un cas d'usage.

---

## Q2 — Positions nationales sur l'hybridation : ANSSI (France) et BSI (Allemagne)

### Takeaway

Les deux agences imposent l'hybridation, mais avec une exception identique et décisive pour un nœud contraint : **les signatures fondées sur le hachage (SLH-DSA, XMSS, LMS) sont acceptées en usage seul, non hybride**, par l'ANSSI comme par le BSI. Le BSI chiffre en plus les échéances : classique seul déconseillé pour l'échange de clés après le **31.12.2031** et pour les signatures après le **31.12.2035**.

### Cited Findings

**ANSSI (France)**

- Hybridation obligatoire sur périmètres réglementés : pour les données classifiées de défense, la Diffusion Restreinte et les systèmes d'information d'importance vitale, « **l'hybridation est obligatoire** » — [ANSSI, FAQ sur la cryptographie post-quantique](https://cyber.gouv.fr/enjeux-technologiques/cryptographie-post-quantique/faq-pqc/)
- Recommandation générale : l'ANSSI « insiste fortement sur le caractère essentiel de l'hybridation des algorithmes de cryptographie post-quantique partout où ils sont déployés, à la fois à court et moyen terme » — [ANSSI FAQ PQC](https://cyber.gouv.fr/enjeux-technologiques/cryptographie-post-quantique/faq-pqc/)
- Portée temporelle : hybridation recommandée « en particulier pour les produits de sécurité destinés à offrir une protection durable des informations (jusqu'à après 2030) » — [ANSSI FAQ PQC](https://cyber.gouv.fr/enjeux-technologiques/cryptographie-post-quantique/faq-pqc/)
- **Exception décisive (hachage)** : « Les seuls algorithmes post-quantiques pour lesquels l'ANSSI ne recommande pas un recours systématique à l'hybridation sont les algorithmes de signature fondés sur le hachage : **SLH-DSA, XMSS et LMS** » — [ANSSI FAQ PQC](https://cyber.gouv.fr/enjeux-technologiques/cryptographie-post-quantique/faq-pqc/)
- Jalons ANSSI : visa de sécurité conditionné à la PQC à l'entrée en qualification visée pour **2027** ; « il ne sera pas raisonnable d'acheter des produits qui n'intègrent pas de la PQC après **2030** » — [ANSSI FAQ PQC](https://cyber.gouv.fr/enjeux-technologiques/cryptographie-post-quantique/faq-pqc/)
- L'ANSSI « ne fournit traditionnellement pas de liste fermée d'algorithmes recommandés » — [ANSSI FAQ PQC](https://cyber.gouv.fr/enjeux-technologiques/cryptographie-post-quantique/faq-pqc/)
- Hybridations citées dans les commentaires de synthèse des publications ANSSI : X25519 + ML-KEM-768 pour l'échange de clés, ECDSA P-256 + ML-DSA-65 pour la signature en contexte Diffusion Restreinte — [page de publication ANSSI « Follow up position paper on Post-Quantum Cryptography »](https://cyber.gouv.fr/en/publications/follow-position-paper-post-quantum-cryptography) ⚠️ **non vérifié à la source** : le PDF du *Follow up position paper* redirige vers une page de portail (`messervices.cyber.gouv.fr`) qui n'expose pas le corps du document ; ce couple d'algorithmes n'a donc pas pu être lu dans le document original.

**BSI (Allemagne) — TR-02102-1, version 2026-01**

- Document et version lus à la source : « BSI TR-02102-1, Cryptographic Mechanisms: Recommendations and Key Lengths, **Version: 2026-01** » — [BSI TR-02102-1 (PDF)](https://www.bsi.bund.de/SharedDocs/Downloads/EN/BSI/Publications/TechGuidelines/TG02102/BSI-TR-02102-1.pdf)
- Échange de clés classique seul : « The sole use of classic key agreement mechanisms is only recommended **until the end of 2031** » ; « the transition to quantum-safe mechanisms should already take place by the **end of 2030** » — [BSI TR-02102-1 §2.1](https://www.bsi.bund.de/SharedDocs/Downloads/EN/BSI/Publications/TechGuidelines/TG02102/BSI-TR-02102-1.pdf)
- Signatures : « the transition to quantum-safe signature schemes is recommended **by 2035 at the latest** » ; alignement explicite sur la feuille de route UE : « According to the roadmap proposed by the EU [1], the transition should take place by 2035 at the latest. This technical guideline follows this recommendation » — [BSI TR-02102-1 §2.1 et §5.3](https://www.bsi.bund.de/SharedDocs/Downloads/EN/BSI/Publications/TechGuidelines/TG02102/BSI-TR-02102-1.pdf)
- Hybridation imposée pour les signatures : « This Technical Guideline recommends the use of a quantum-safe signature scheme **only in combination with a classic signature scheme**. Hybridisation should be implemented in such a way that the hybrid signature scheme is secure as long as **one** of the schemes is secure. A natural and robust hybridisation is the **concatenation** of a quantum-safe signature with a classic signature so that the concatenated signature is accepted as valid if all individual signatures are valid » — [BSI TR-02102-1 §5.3.4](https://www.bsi.bund.de/SharedDocs/Downloads/EN/BSI/Publications/TechGuidelines/TG02102/BSI-TR-02102-1.pdf)
- Séparation des clés hybrides : « Care should be taken here to generate key material for hybrid signatures **specifically for this purpose** and not to use it for non-hybrid signatures as well » — [BSI TR-02102-1 §5.3.4](https://www.bsi.bund.de/SharedDocs/Downloads/EN/BSI/Publications/TechGuidelines/TG02102/BSI-TR-02102-1.pdf)
- **Exception décisive (hachage)** : « The security of hash-based signature schemes is only based on complexity-theoretical assumptions about cryptographic hash functions. Therefore, hash-based signatures can, provided that the implementation security of stateful and stateless hash-based mechanisms is carefully considered, **in principle also be used alone (i.e. not in hybrid form)** and serve as a good method for generating long-term secure signatures » — [BSI TR-02102-1 §5.3.4](https://www.bsi.bund.de/SharedDocs/Downloads/EN/BSI/Publications/TechGuidelines/TG02102/BSI-TR-02102-1.pdf)
- Jeux de paramètres recommandés — signatures quantum-safe (Tables 5.3, 5.6, 5.7, 5.8) :
  - ML-DSA, variante « hedged », **catégories 3 et 5 seulement** : « ML-DSA-65 » et « ML-DSA-87 ». **ML-DSA-44 n'est pas recommandé par le BSI.** — [BSI TR-02102-1 Table 5.7](https://www.bsi.bund.de/SharedDocs/Downloads/EN/BSI/Publications/TechGuidelines/TG02102/BSI-TR-02102-1.pdf)
  - SLH-DSA, variante « hedged », catégories 3 et 5 : 192s, 192f, 256s, 256f (SHA2 et SHAKE). **Les jeux 128s / 128f ne sont pas recommandés par le BSI.** — [BSI TR-02102-1 Table 5.6](https://www.bsi.bund.de/SharedDocs/Downloads/EN/BSI/Publications/TechGuidelines/TG02102/BSI-TR-02102-1.pdf)
  - Signatures à état : « XMSS/XMSS^MT, all parameter sets according to [SP 800-208] » et « LMS/HSS, all parameter sets according to [SP 800-208] » — [BSI TR-02102-1 Table 5.8](https://www.bsi.bund.de/SharedDocs/Downloads/EN/BSI/Publications/TechGuidelines/TG02102/BSI-TR-02102-1.pdf)
- Condition d'emploi des schémas à état, et cas d'usage explicitement visé : « the stateful nature of these schemes poses a challenge for key management: it limits the number of signatures that can be generated per key pair and a **strictly monotonic counter must be incremented without error** each time a signature is generated. Therefore, stateful signature schemes are only recommended in the special scenarios in which secure and error-free state management can be guaranteed. One such special scenario is, for example, **the signing of software and firmware updates** » — [BSI TR-02102-1 §5.3.4.3](https://www.bsi.bund.de/SharedDocs/Downloads/EN/BSI/Publications/TechGuidelines/TG02102/BSI-TR-02102-1.pdf)
- Mise en garde sur LMS + SHA-256 : une attaque publiée contre SPHINCS+/SHA-256 réduit la sécurité d'« approximately 40 bits » (256 → ~216 bits) ; « Theoretically, the idea of this attack can be transferred to **LMS/HSS in combination with SHA-256**, so that this attack should be taken into account » — le BSI précise que les jeux normalisés restent nettement au-dessus du seuil même en tenant compte de l'attaque — [BSI TR-02102-1 Remark 5.11](https://www.bsi.bund.de/SharedDocs/Downloads/EN/BSI/Publications/TechGuidelines/TG02102/BSI-TR-02102-1.pdf)
- Chemin de mise à jour : « products designed with an expected lifetime beyond 2030 can be upgraded to quantum computer-resistant methods and that the upgrade mechanism for software and firmware upgrades **includes post-quantum signature methods** » — [BSI TR-02102-1 §5.3](https://www.bsi.bund.de/SharedDocs/Downloads/EN/BSI/Publications/TechGuidelines/TG02102/BSI-TR-02102-1.pdf)

### Inferences

- **Convergence ANSSI/BSI sur un point exploitable** : les deux agences autorisent le hachage (SLH-DSA, XMSS, LMS) **sans hybridation**. Pour un lien très contraint, cela divise par deux la charge utile à transporter, puisqu'on n'a pas à concaténer une signature classique. C'est le seul chemin « conforme et non hybride » documenté par les deux agences.
- **Tension entre conformité BSI et budget radio** : le BSI écarte ML-DSA-44 (2 420 o) et SLH-DSA-128s (7 856 o) au profit de ML-DSA-65 (3 309 o) et SLH-DSA-192s (16 224 o). Le choix « conforme BSI » est donc systématiquement le plus coûteux en octets. L'ANSSI, qui ne publie pas de liste fermée, laisse plus de marge mais exige l'hybridation hors hachage.
- Le cas d'usage que le BSI désigne nommément comme approprié aux schémas à état — **signature de mises à jour logicielles et firmware** — est aussi celui où la fréquence des messages est la plus faible. Les contraintes réglementaires et les contraintes radio pointent donc dans la même direction.

### Gaps

- Le *Follow up position paper on Post-Quantum Cryptography* de l'ANSSI (et l'« Avis de l'ANSSI sur la migration vers la cryptographie post-quantique ») n'ont pas pu être lus à la source : les deux URL PDF de `cyber.gouv.fr` renvoient une page HTML de portail. Les trois phases de calendrier ANSSI (phase 1 / 2 / 3) et les jeux de paramètres précis par phase restent donc **non vérifiés à la source**. Les éléments ci-dessus proviennent de la FAQ officielle de l'ANSSI, qui est bien un document primaire de l'agence.
- Je n'ai pas vérifié à la source la version allemande ni l'historique de versions de TR-02102-1 : la version lue est l'édition anglaise 2026-01.

---

## Q3 — Standards NIST publiés : FIPS 203, 204, 205

### Takeaway

Les trois standards ont été publiés et sont entrés en vigueur le **13 août 2024**. FIPS 204 (ML-DSA) et FIPS 205 (SLH-DSA) sont les deux seuls standards de signature post-quantique en vigueur ; Falcon (FN-DSA / FIPS 206) n'est pas dans cette liste.

### Cited Findings

- **FIPS 203** : « Module-Lattice-Based Key-Encapsulation Mechanism Standard », publié le **13 août 2024**, normalise **ML-KEM** avec trois jeux : ML-KEM-512, ML-KEM-768, ML-KEM-1024, « ordered by increasing security strength and decreasing performance » — [NIST CSRC, FIPS 203](https://csrc.nist.gov/pubs/fips/203/final)
- **FIPS 204** : « Module-Lattice-Based Digital Signature Standard », publié le **13 août 2024**, DOI `10.6028/NIST.FIPS.204`, normalise **ML-DSA** — [NIST CSRC, FIPS 204](https://csrc.nist.gov/pubs/fips/204/final)
- **FIPS 205** : « Stateless Hash-Based Digital Signature Standard », **Published: August 13, 2024 / Effective: August 13, 2024**, normalise **SLH-DSA** (issu de SPHINCS+) — [NIST FIPS 205 (PDF)](https://nvlpubs.nist.gov/nistpubs/FIPS/NIST.FIPS.205.pdf) ; [page CSRC](https://csrc.nist.gov/pubs/fips/205/final)
- Les standards de signature approuvés sont énumérés dans FIPS 205 lui-même : « Either this standard, **FIPS 204, FIPS 186-5, or NIST Special Publication 800-208** shall be used in designing and [implementing digital signature systems] » — [FIPS 205 §9 (Implementations)](https://nvlpubs.nist.gov/nistpubs/FIPS/NIST.FIPS.205.pdf). SP 800-208 est le document NIST qui approuve **LMS/HSS et XMSS/XMSS^MT**.
- Catégories de sécurité ML-DSA : « the parameter set ML-DSA-44 is claimed to be in security strength **category 2**, ML-DSA-65 is claimed to be in **category 3** [and ML-DSA-87 in category 5] » — [FIPS 204 §4](https://nvlpubs.nist.gov/nistpubs/FIPS/NIST.FIPS.204.pdf)
- Réserve du NIST sur ML-DSA-44 : « the claimed security strength of ML-DSA-44 is **reduced from category 2 to category 1** » dans un modèle d'attaque donné — [FIPS 204 §3](https://nvlpubs.nist.gov/nistpubs/FIPS/NIST.FIPS.204.pdf)
- Limite de signatures SLH-DSA : les 12 jeux de FIPS 205 visent la non-forgeabilité EUF-CMA « when each key pair is used to sign **at most 2^64 messages** » — [FIPS 205 §11](https://nvlpubs.nist.gov/nistpubs/FIPS/NIST.FIPS.205.pdf)

### Inferences

- Pour un équipement vendu en Europe, les deux seules signatures PQC à la fois normalisées (FIPS) et recommandées par le BSI sont **ML-DSA (65/87)** et **SLH-DSA (192/256)**, plus les schémas à état via SP 800-208. C'est le périmètre de choix réaliste.

### Gaps

- **FIPS 206 / FN-DSA (Falcon)** : je n'ai pas pu vérifier à la source si un projet (*initial public draft*) de FIPS 206 est publié à la date du 2026-10-07. Falcon n'apparaît ni dans FIPS 204/205 ni dans les tables de recommandations du BSI que j'ai lues. **Statut de normalisation de Falcon : non vérifié à la source.** Toute décision reposant sur Falcon doit considérer qu'il n'est, à ma connaissance vérifiée, pas un standard FIPS en vigueur.
- Une note de bas de page de FIPS 205 lue à la source mentionne qu'une publication spéciale NIST (chaîne extraite : « SP 800-230 ») « specifies additional parameter sets that are approved for use », avec un nombre de signatures plus limité par paire de clés. Je n'ai pas vérifié l'existence ni le contenu de ce document ; l'extraction du numéro peut être erronée (confusion possible avec SP 800-208). **Non vérifié à la source.** Si ce document existe, il pourrait contenir des jeux SLH-DSA à signatures plus courtes — point à vérifier en priorité, car directement pertinent pour un lien contraint.

---

## Q4 — Tailles exactes (octets) des clés publiques et des signatures

### Takeaway

Aucune signature post-quantique normalisée ne tient dans une trame LoRa : la plus petite option FIPS est **ML-DSA-44 à 2 420 octets**, soit **47 fois** la charge utile de 51 octets disponible à SF12. La plus petite signature post-quantique connue est Falcon-512 à **666 octets** — encore 13 trames à SF12 — mais Falcon n'est pas un standard FIPS vérifié. Les signatures à état (LMS/XMSS) sont du même ordre que ML-DSA, autour de 1,1 à 2,9 kio selon les paramètres.

### Cited Findings

**ML-DSA — FIPS 204, Table 2 « Sizes (in bytes) of keys and signatures of ML-DSA »** — [FIPS 204 (PDF), p. 16](https://nvlpubs.nist.gov/nistpubs/FIPS/NIST.FIPS.204.pdf)

| Jeu | Clé privée (o) | **Clé publique (o)** | **Signature (o)** | Catégorie |
|---|---:|---:|---:|---:|
| ML-DSA-44 | 2 560 | **1 312** | **2 420** | 2 |
| ML-DSA-65 | 4 032 | **1 952** | **3 309** | 3 |
| ML-DSA-87 | 4 896 | **2 592** | **4 627** | 5 |

- Optimisation de stockage prévue par le standard : « if one wants to reduce the space needed for the private key, one can store only the **32-byte seed**, which is sufficient to generate the other parts of the private key » — [FIPS 204 §4](https://nvlpubs.nist.gov/nistpubs/FIPS/NIST.FIPS.204.pdf)

**SLH-DSA — FIPS 205, Table 2 « SLH-DSA parameter sets »** — [FIPS 205 (PDF), §11](https://nvlpubs.nist.gov/nistpubs/FIPS/NIST.FIPS.205.pdf)

| Jeu (SHA2 et SHAKE) | Catégorie | **pk (o)** | **signature (o)** |
|---|---:|---:|---:|
| SLH-DSA-{SHA2,SHAKE}-128s | 1 | **32** | **7 856** |
| SLH-DSA-{SHA2,SHAKE}-128f | 1 | **32** | **17 088** |
| SLH-DSA-{SHA2,SHAKE}-192s | 3 | **48** | **16 224** |
| SLH-DSA-{SHA2,SHAKE}-192f | 3 | **48** | **35 664** |
| SLH-DSA-{SHA2,SHAKE}-256s | 5 | **64** | **29 792** |
| SLH-DSA-{SHA2,SHAKE}-256f | 5 | **64** | **49 856** |

- Paramètres du plus petit jeu, à la source : SLH-DSA-128s a n=16, h=63, d=7, h'=9, a=12, k=14, lg(w)=4, m=30 — [FIPS 205 Table 2](https://nvlpubs.nist.gov/nistpubs/FIPS/NIST.FIPS.205.pdf)
- **Le plus petit jeu SLH-DSA (128s, 7 856 o) est 3,2× plus gros que ML-DSA-44.** La clé publique, en revanche, est minuscule (32 o) — [FIPS 205 Table 2](https://nvlpubs.nist.gov/nistpubs/FIPS/NIST.FIPS.205.pdf)

**Falcon** — spécification officielle du soumissionnaire — [falcon-sign.info](https://falcon-sign.info/)

| Jeu | **pk (o)** | **signature (o)** | sk (o) |
|---|---:|---:|---:|
| Falcon-512 | **897** | **666** | ~1 998 (« about three times that of a signature ») |
| Falcon-1024 | **1 793** | **1 280** | ~3 840 |

⚠️ Les implémentations exposent des tailles différentes selon l'encodage : 666 o (encodage compressé moyen de la spécification), 752 o et 690 o selon les variantes, 809/1 077 o pour les formats *padded*. Seule la valeur **666 o** est celle de la spécification officielle lue à la source ; les autres viennent de documentations d'implémentation — [liboqs, algorithmes de signature Falcon](https://openquantumsafe.org/liboqs/algorithms/sig/falcon). **Conséquence pratique : une taille de signature Falcon doit toujours être qualifiée par son encodage.**

**LMS — RFC 8554** — [RFC 8554](https://www.rfc-editor.org/rfc/rfc8554.html)

- Formule de taille de signature LM-OTS : **4 + n·(p+1)** octets ; clé publique LM-OTS : 24 + n octets — [RFC 8554 §4](https://www.rfc-editor.org/rfc/rfc8554.html)
- Formule de taille de signature LMS : **4 + n·(p+1) + m·h** octets ; clé publique LMS : **24 + m** octets (56 o pour SHA-256, m=32) — [RFC 8554 §5](https://www.rfc-editor.org/rfc/rfc8554.html)
- Jeux LM-OTS et longueurs de signature OTS, à la source (Table 1) :

| LM-OTS | n | w | p | signature OTS (o) |
|---|---:|---:|---:|---:|
| LMOTS_SHA256_N32_W1 | 32 | 1 | 265 | **8 516** |
| LMOTS_SHA256_N32_W2 | 32 | 2 | 133 | **4 292** |
| LMOTS_SHA256_N32_W4 | 32 | 4 | 67 | **2 180** |
| LMOTS_SHA256_N32_W8 | 32 | 8 | 34 | **1 124** |

- Jeux LMS (Table 2) : LMS_SHA256_M32_H5 / H10 / H15 / H20 / H25, avec m=32 et h = 5, 10, 15, 20, 25 — [RFC 8554 §5.1](https://www.rfc-editor.org/rfc/rfc8554.html)
- **[calcul]** Signature LMS complète = signature OTS + m·h. Exemples (SHA-256, m=32) :
  - LMS_SHA256_M32_H10 + LMOTS_..._W8 : 1 124 + 32×10 = **1 444 o** (2^10 = 1 024 signatures max)
  - LMS_SHA256_M32_H10 + LMOTS_..._W4 : 2 180 + 32×10 = **2 500 o**
  - LMS_SHA256_M32_H20 + LMOTS_..._W8 : 1 124 + 32×20 = **1 764 o** (2^20 ≈ 1,05 M signatures)
  - LMS_SHA256_M32_H25 + LMOTS_..._W8 : 1 124 + 32×25 = **1 924 o**
- Caractère à état, à la source : « the LMS signing algorithm is **stateful**; it modifies and updates the private key as a side effect of generating a signature » — [RFC 8554](https://www.rfc-editor.org/rfc/rfc8554.html)

**XMSS — RFC 8391** — [RFC 8391](https://www.rfc-editor.org/rfc/rfc8391.html)

- Clé publique XMSS et XMSS^MT : **2n + 4 octets** (OID 4 o + racine n o + SEED n o) ⇒ **68 o** pour n=32, 132 o pour n=64 — [RFC 8391 §4.1.7 / §4.2.7](https://www.rfc-editor.org/rfc/rfc8391.html)
- Signature XMSS : **4 + n + (len + h)·n octets**. Valeurs à la source :

| Jeu XMSS | **signature (o)** | nb max de signatures |
|---|---:|---:|
| XMSS-SHA2_10_256 | **2 500** | 2^10 = 1 024 |
| XMSS-SHA2_16_256 | **2 692** | 2^16 = 65 536 |
| XMSS-SHA2_20_256 | **2 884** | 2^20 ≈ 1,05 M |

- Signature XMSS^MT : **ceil(h/8) + n + (h + d·len)·n octets**. Valeurs à la source : XMSSMT-SHA2_20/2_256 = **2 920 o** ; _40/2_256 = **4 648 o** ; _40/4_256 = **3 368 o** ; _60/3_256 = **4 952 o** — [RFC 8391 §5.3](https://www.rfc-editor.org/rfc/rfc8391.html)
- Caractère à état, à la source : « If a secret key state is used twice, **no cryptographic security guarantees remain** » — [RFC 8391](https://www.rfc-editor.org/rfc/rfc8391.html)

**Référence classique, pour l'échelle** : Ed25519 produit une signature de 64 o et une clé publique de 32 o (FIPS 186-5 / RFC 8032) — **non revérifié à la source dans cette session**, donné comme ordre de grandeur de comparaison.

### Inferences

- Classement par taille de signature, parmi les options utilisables : **Falcon-512 (666 o) < LMS W8/H10 (1 444 o) < ML-DSA-44 (2 420 o) < XMSS-SHA2_10_256 (2 500 o) < ML-DSA-65 (3 309 o) < ML-DSA-87 (4 627 o) < SLH-DSA-128s (7 856 o) < SLH-DSA-192s (16 224 o)**.
- **Le compromis LMS est le plus intéressant pour un lien contraint parmi les options approuvées** : avec LMOTS_SHA256_N32_W8, la signature descend à 1 444 o (h=10) ou 1 764 o (h=20) — soit 40 % de moins que ML-DSA-44 — et la clé publique tombe à 56 o au lieu de 1 312 o. Le paramètre w est le levier : w=8 contre w=1 fait passer la signature OTS de 8 516 à 1 124 o, au prix d'un coût de calcul plus élevé.
- **La clé publique compte autant que la signature dans un protocole où elle doit circuler.** ML-DSA-44 impose 1 312 o de clé publique contre 32 o pour SLH-DSA et 56 o pour LMS. Un protocole qui transmet la clé publique (attestation, enrôlement) paie donc deux fois avec ML-DSA.
- **L'hybridation, exigée hors hachage par l'ANSSI et le BSI, ajoute la signature classique.** Une signature hybride ECDSA P-256 + ML-DSA-65 pèse donc environ 64 + 3 309 ≈ **3 373 o** [calcul, taille ECDSA P-256 non revérifiée à la source]. À l'inverse, LMS/XMSS/SLH-DSA sont acceptés seuls : pas de surcoût d'hybridation.

### Gaps

- Les tailles de signature ECDSA P-256 et Ed25519 n'ont pas été revérifiées dans FIPS 186-5 / RFC 8032 au cours de cette session.
- Je n'ai pas vérifié à la source les jeux LMS/XMSS **effectivement approuvés** par NIST SP 800-208 (le BSI recommande « all parameter sets according to [SP 800-208] », mais SP 800-208 peut restreindre par rapport aux RFC). À vérifier avant de retenir un jeu précis.

---

## Q5 — Contraintes LoRa/LoRaWAN en EU868 : charge utile par SF, time on air, et origine de la règle de duty-cycle

### Takeaway

À SF12 (DR0) la charge utile applicative est plafonnée à **51 octets** par la spécification régionale LoRaWAN, et ne monte à **242 octets** qu'à partir de DR4/SF8. Le duty-cycle de **1 %** sur la bande 868,0–868,6 MHz vient de la norme harmonisée **ETSI EN 300 220-2**, qui met en œuvre les conditions de la décision européenne sur les dispositifs à courte portée (**Decision 2019/1345/EU**, modifiant 2006/771/CE), dans le cadre de la directive RED 2014/53/UE.

### Cited Findings

**Charge utile maximale par data rate — LoRaWAN RP002-1.0.3, EU863-870**

- Table 8 « EU863-870 TX DataRate table », correspondance DR ↔ SF et débit indicatif, à la source — [RP002-1.0.3 (PDF), §2.4.3, p. 26](https://lora-alliance.org/wp-content/uploads/2021/05/RP002-1.0.3-FINAL-1.pdf)

| DR | Configuration | Débit physique indicatif (bit/s) |
|---:|---|---:|
| 0 | LoRa : **SF12** / 125 kHz | 250 |
| 1 | LoRa : **SF11** / 125 kHz | 440 |
| 2 | LoRa : **SF10** / 125 kHz | 980 |
| 3 | LoRa : **SF9** / 125 kHz | 1 760 |
| 4 | LoRa : **SF8** / 125 kHz | 3 125 |
| 5 | LoRa : **SF7** / 125 kHz | 5 470 |
| 6 | LoRa : SF7 / 250 kHz | 11 000 |
| 7 | FSK : 50 kbps | 50 000 |
| 8–11 | LR-FHSS | 162 à 325 |

- Tables 12 et 13 « EU863-870 maximum payload size », avec M = taille max du MACPayload et N = taille max de la charge utile applicative en l'absence du champ optionnel FOpts, à la source — [RP002-1.0.3, §2.4.6, p. 28-29](https://lora-alliance.org/wp-content/uploads/2021/05/RP002-1.0.3-FINAL-1.pdf)

| DR | SF | M (o) — compatible répéteur | **N (o)** — compatible répéteur | M (o) — sans répéteur | **N (o)** — sans répéteur |
|---:|---|---:|---:|---:|---:|
| 0 | SF12 | 59 | **51** | 59 | **51** |
| 1 | SF11 | 59 | **51** | 59 | **51** |
| 2 | SF10 | 59 | **51** | 59 | **51** |
| 3 | SF9 | 123 | **115** | 123 | **115** |
| 4 | SF8 | 230 | **222** | 250 | **242** |
| 5 | SF7 | 230 | **222** | 250 | **242** |
| 6 | SF7/250 | 230 | **222** | 250 | **242** |
| 7 | FSK | 230 | **222** | 250 | **242** |

- Mise en garde de la spécification : « The value of N **MAY be smaller** if the FOpts field is not empty » — [RP002-1.0.3, §2.4.6](https://lora-alliance.org/wp-content/uploads/2021/05/RP002-1.0.3-FINAL-1.pdf)
- Pas de contrainte de dwell time en EU868 : « There is no dwell time limitation for the EU863-870 PHY layer » — [RP002-1.0.3, §2.4.3](https://lora-alliance.org/wp-content/uploads/2021/05/RP002-1.0.3-FINAL-1.pdf)

**Duty-cycle : la règle et son origine exacte**

- Côté LoRaWAN, la valeur figure dans la Table 6 « EU863-870 default channels » : les trois canaux par défaut 868,10 / 868,30 / 868,50 MHz, DR0 à DR5, portent un **duty cycle « < 1 % »** — [RP002-1.0.3, §2.4.2, p. 25](https://lora-alliance.org/wp-content/uploads/2021/05/RP002-1.0.3-FINAL-1.pdf)
- Justification donnée par la spécification elle-même : « In order to access the physical medium, the **ETSI regulations** impose some restrictions such as the maximum time the transmitter can be on or the maximum time a transmitter can transmit per hour. The ETSI regulations allow the choice of using either a **duty-cycle limitation** or a so-called **Listen Before Talk Adaptive Frequency Agility (LBT AFA)** transmissions management. The current LoRaWAN specification **exclusively uses duty-cycled limited transmissions** to comply with the ETSI regulations » — [RP002-1.0.3, §2.4.2](https://lora-alliance.org/wp-content/uploads/2021/05/RP002-1.0.3-FINAL-1.pdf)
- **Source normative du 1 %** — table des bandes de fréquences autorisées, lue à la source dans **Draft ETSI EN 300 220-2 V3.2.2 (2024-03)** :

| Bande ETSI | Plage | p.i.r.e. max | Règle d'accès | Largeur de bande max |
|---|---|---|---|---|
| K0 | 862–863 MHz | 25 mW e.r.p. | 0,1 % duty cycle | — |
| K | 863–865 MHz | 25 mW e.r.p. | 0,1 % duty cycle ou polite spectrum access | — |
| L | 865–868 MHz | 25 mW e.r.p. | **1 % duty cycle** ou polite spectrum access | 3 MHz |
| **M** | **868,000–868,600 MHz** | **25 mW e.r.p.** | **1 % duty cycle** ou polite spectrum access | **600 kHz** |
| N | 868,700–869,200 MHz | 25 mW e.r.p. | 0,1 % duty cycle ou polite spectrum access | 500 kHz |

— [Draft ETSI EN 300 220-2 V3.2.2 (2024-03), clause 4.4 / tableau des bandes](https://www.etsi.org/deliver/etsi_en/300200_300299/30022002/03.02.02_20/en_30022002v030202ev.pdf)

- **Définition normative du duty cycle**, citée à la source : « Duty cycle is the ratio expressed as a percentage, of the **cumulative duration of transmissions Ton_cum** within an observation interval **Tobs** on an observation bandwidth **Fobs**. […] Unless otherwise specified, **Tobs is 1 hour** and the observation bandwidth Fobs is the **Permitted Frequency Band** » — [ETSI EN 300 220-2 V3.2.2, clause 4.4.3.2](https://www.etsi.org/deliver/etsi_en/300200_300299/30022002/03.02.02_20/en_30022002v030202ev.pdf)
- Point souvent mal compris, à la source : le duty cycle s'applique **par bande permise**, pas par canal — « An equipment may operate on several bands simultaneously (i.e. multi transmissions), the Duty Cycle limit of each individual band applies to the series of transmissions within that band » — [ETSI EN 300 220-2 V3.2.2, clause 4.4.3.2](https://www.etsi.org/deliver/etsi_en/300200_300299/30022002/03.02.02_20/en_30022002v030202ev.pdf)
- Échappatoire normative : « The duty cycle requirement shall **apply to all transmitters except** equipment operating in bands K, L, M, N, O, Q or W **using polite spectrum access** (described in clause 4.6) » — [ETSI EN 300 220-2 V3.2.2, clause 4.4.3.1](https://www.etsi.org/deliver/etsi_en/300200_300299/30022002/03.02.02_20/en_30022002v030202ev.pdf)
- **Chaîne juridique, à la source** : l'annexe B de la norme est intitulée « EU wide harmonised national radio interfaces from 25 MHz to 1 000 MHz » et précise : « Table B.1 summarizes the harmonised frequency bands and their technical key parameters for non specific short-range devices in **EC Decision 2019/1345/EU** » — [ETSI EN 300 220-2 V3.2.2, Annexe B](https://www.etsi.org/deliver/etsi_en/300200_300299/30022002/03.02.02_20/en_30022002v030202ev.pdf). (Decision (UE) 2019/1345 modifie la décision 2006/771/CE sur l'harmonisation du spectre pour les dispositifs à courte portée.)
- Statut de norme harmonisée sous la directive RED, à la source : « **Presumption of conformity** stays valid only as long as a reference to the present document is maintained in the list published in the **Official Journal of the European Union** » — [ETSI EN 300 220-2 V3.2.2](https://www.etsi.org/deliver/etsi_en/300200_300299/30022002/03.02.02_20/en_30022002v030202ev.pdf)

**Time on air — formule et valeurs calculées**

- Formule employée (modulation LoRa, datasheet Semtech SX1276/77/78/79, clause 4.1.1.7 « LoRa Time on Air ») :
  - `Tsym = 2^SF / BW`
  - `Tpreamble = (n_preamble + 4,25) · Tsym`
  - `payloadSymbNb = 8 + max( ceil( (8·PL − 4·SF + 28 + 16·CRC − 20·IH) / (4·(SF − 2·DE)) ) · (CR + 4) , 0 )`
  - `ToA = Tpreamble + payloadSymbNb · Tsym`
  ⚠️ **Non vérifié à la source** : la datasheet Semtech SX1276 n'est pas librement téléchargeable (portail commercial) ; toutes les URL testées renvoient une page HTML. La formule est reproduite de mémoire de cette clause et **doit être revalidée** sur la datasheet originale avant d'être citée dans un document de conformité.
- **[calcul]** Hypothèses : BW = 125 kHz, CR = 4/5 (CR=1), préambule = 8 symboles, en-tête explicite (IH=0), CRC activé (CRC=1), **Low Data Rate Optimize activé pour SF11 et SF12** (DE=1), sinon DE=0. `PL` = taille du PHYPayload en octets.

| SF | Tsym (ms) | **ToA, PL = 51 o** | **ToA, PL = 255 o** |
|---|---:|---:|---:|
| SF7 | 1,024 | **102,7 ms** (88 symboles) | **399,6 ms** (378 symboles) |
| SF8 | 2,048 | **184,8 ms** (78 sym.) | **707,1 ms** (333 sym.) |
| SF9 | 4,096 | **328,7 ms** (68 sym.) | **1 250,3 ms** (293 sym.) |
| SF10 | 8,192 | **616,4 ms** (63 sym.) | **2 295,8 ms** (268 sym.) |
| SF11 | 16,384 | **1 314,8 ms** (68 sym.) | **5 001,2 ms** (293 sym.) |
| SF12 | 32,768 | **2 465,8 ms** (63 sym.) | **9 019,4 ms** (263 sym.) |

- Remarque de lecture importante : **51 octets de charge utile applicative ≠ 51 octets de PHYPayload.** Une trame LoRaWAN ajoute MHDR (1 o) + DevAddr (4 o) + FCtrl (1 o) + FCnt (2 o) + FPort (1 o) + MIC (4 o) = **13 octets** [calcul, d'après la structure de trame LoRaWAN — structure non revérifiée à la source dans cette session]. Un message applicatif de 51 o produit donc un PHYPayload de 64 o, dont le ToA à SF12 est de **2 793,5 ms** et non 2 465,8 ms **[calcul]**.
- L'effet d'escalier est visible dans les symboles : à SF10 et SF12 un PL de 51 o coûte 63 symboles, contre 68 à SF9 et SF11 — la quantification en blocs de (CR+4) symboles fait qu'augmenter la charge utile de quelques octets peut ne rien coûter, ou coûter un bloc entier **[calcul]**.

### Inferences

- **La contrainte dominante n'est pas la taille de la trame mais le temps d'antenne.** À SF12, le débit utile légal est de 36 s/h × 250 bit/s ≈ **1,1 kio par heure, toutes données confondues**. Une seule signature ML-DSA-44 dépasse ce budget horaire.
- Le « polite spectrum access » (LBT + AFA) de la clause 4.6 d'EN 300 220-2 est la seule voie normative pour s'affranchir du 1 % — mais LoRaWAN y renonce explicitement, donc un nœud LoRaWAN conforme n'y a pas accès sans sortir de la spécification.
- Le duty cycle s'appliquant **par bande permise** et non par canal, l'étalement sur les trois canaux par défaut 868,1/868,3/868,5 MHz **ne multiplie pas le budget** : ils sont tous dans la bande M (868,0–868,6 MHz). Un gain réel exigerait d'utiliser aussi la bande L (865–868 MHz, 1 %), ce qui doublerait le budget à 72 s/h au total mais pas sur une seule bande.

### Gaps

- La version lue à la source est le **projet** (« Draft ») ETSI EN 300 220-2 V3.2.2 (2024-03), librement accessible sur etsi.org. La version citée dans la liste du Journal officiel au titre de la RED peut être la V3.2.1 (2018-04), également publique sur etsi.org. **La table des bandes doit être revérifiée dans la version exactement référencée au JOUE** avant tout usage en dossier de conformité.
- La datasheet Semtech SX1276 (source primaire de la formule de time on air) n'a pas pu être lue : **non vérifié à la source**.
- La structure de trame LoRaWAN (surcoût de 13 octets) vient de la spécification TS001, que je n'ai pas lue dans cette session.
- RP002-1.0.3 est la version lue. Des versions plus récentes existent (RP002-1.0.4) ; les valeurs M/N pour EU868 n'ont pas été recoupées avec la dernière révision.

---

## Q6 — Nombre de messages par heure légalement émissibles à chaque facteur d'étalement

### Takeaway

Le budget est de **36 secondes d'émission cumulée par heure et par bande** (1 % de 3 600 s). Cela donne de **305 messages/heure à SF7** à **12 messages/heure à SF12** pour une trame de 64 octets — et seulement **3 messages/heure à SF12** pour une trame pleine de 255 octets.

### Cited Findings

- Base normative du calcul, citée à la source : duty cycle = `Ton_cum / Tobs` exprimé en pourcentage, avec « Tobs is 1 hour » et application « per band » — [ETSI EN 300 220-2 V3.2.2, clause 4.4.3.2](https://www.etsi.org/deliver/etsi_en/300200_300299/30022002/03.02.02_20/en_30022002v030202ev.pdf)
- Limite applicable à la bande M (868,0–868,6 MHz, où se trouvent les trois canaux LoRaWAN par défaut) : **1 % duty cycle** — [ETSI EN 300 220-2 V3.2.2, table des bandes](https://www.etsi.org/deliver/etsi_en/300200_300299/30022002/03.02.02_20/en_30022002v030202ev.pdf) ; confirmé côté LoRaWAN par la Table 6 « < 1 % » — [RP002-1.0.3 §2.4.2](https://lora-alliance.org/wp-content/uploads/2021/05/RP002-1.0.3-FINAL-1.pdf)

**[calcul] Formule**

```
Ton_cum_max = duty_cycle × Tobs = 0,01 × 3 600 s = 36 s par heure et par bande
N_msg/h     = floor( Ton_cum_max / ToA(SF, PL) ) = floor( 36 / ToA )
```

**[calcul] Résultats** (mêmes hypothèses radio que Q5 : BW 125 kHz, CR 4/5, 8 symboles de préambule, en-tête explicite, CRC on, LDRO on pour SF11/SF12)

| SF | ToA, PL = 64 o (51 o applicatifs) | **msg/h** | ToA, PL = 255 o | **msg/h** |
|---|---:|---:|---:|---:|
| SF7 | 118,0 ms | **305** | 399,6 ms | **90** |
| SF8 | 215,6 ms | **167** | 707,1 ms | **50** |
| SF9 | 390,1 ms | **92** | 1 250,3 ms | **28** |
| SF10 | 698,4 ms | **51** | 2 295,8 ms | **15** |
| SF11 | 1 560,6 ms | **23** | 5 001,2 ms | **7** |
| SF12 | 2 793,5 ms | **12** | 9 019,4 ms | **3** |

Exemple détaillé, SF12 / PL = 64 o : `36 s / 2,7935 s = 12,89` ⇒ **12 messages/heure** (un treizième dépasserait les 36 s).

### Inferences

- **Un nœud à SF12 dispose de 12 trames par heure.** Si une signature ML-DSA-44 coûte 48 trames (voir Q7), son émission occupe à elle seule **quatre heures de budget légal**. Cela exclut toute signature PQC sur le trafic courant à SF12 et la réserve strictement à des événements rares.
- À SF7, les 305 trames/heure rendent une signature PQC envisageable (10 trames pour ML-DSA-44, soit 3,3 % du budget horaire), mais SF7 implique une portée très réduite — c'est-à-dire précisément le cas où LoRa perd son intérêt.
- Les chiffres ci-dessus sont des **maxima théoriques** : ils supposent qu'aucun autre trafic (join, MAC commands, retransmissions, ADR) ne consomme de budget, et ignorent le back-off des retransmissions imposé par TS001.

### Gaps

- Je n'ai pas vérifié à la source les règles de « Retransmissions back-off » du chapitre correspondant de TS001, que RP002 invoque pour le duty-cycle des Join-Request — elles réduisent encore le budget disponible en pratique.

---

## Q7 — Approches documentées pour transporter une signature de plusieurs kio sur un lien très contraint

### Takeaway

Quatre familles existent et sont documentées dans des spécifications, non des opinions : la **fragmentation applicative** (LoRa Alliance TS004, FUOTA) ou **au niveau adaptation** (SCHC, RFC 9011, MTU de 2 520 o), les **signatures à état** LMS/XMSS (RFC 8554 / RFC 8391, approuvées par SP 800-208 et recommandées par le BSI pour les mises à jour firmware), le **choix de paramètres minimisant la signature** (LM-OTS w=8, clé publique de 56 o), et le **cantonnement du PQC aux messages rares**. Aucune ne fait entrer une signature PQC dans une trame unique : toutes déplacent le coût.

### Cited Findings

**Fragmentation — niveau adaptation : SCHC over LoRaWAN (RFC 9011)**

- Mode retenu pour la montante : « **ACK-on-Error** mode for uplink fragmentation » ; pour la descendante unicast : « **ACK-Always** mode for downlink fragmentation » ; en multicast : « **No-ACK** », auquel cas « reliability has to be ensured by the upper layer » — [RFC 9011](https://www.rfc-editor.org/rfc/rfc9011.html)
- Paramètres montants, à la source : taille de tuile **10 octets** ; « 4 windows are used, encoded on **M = 2 bits** » ; « N = 6 bits, so **WINDOW_SIZE = 63 tiles** are allowed in a window » — [RFC 9011](https://www.rfc-editor.org/rfc/rfc9011.html)
- **MTU résultante, calculée dans la RFC elle-même** : « MTU is: **4 windows × 63 tiles × 10 bytes per tile = 2 520 bytes** » — [RFC 9011](https://www.rfc-editor.org/rfc/rfc9011.html)
- Descendante : « N = 1 bit, so WINDOW_SIZE = 1 tile » ; taille de tuile variable, « will be the currently available MTU » — [RFC 9011](https://www.rfc-editor.org/rfc/rfc9011.html)
- `MAX_ACK_REQUESTS` = **8**, montante et descendante — [RFC 9011](https://www.rfc-editor.org/rfc/rfc9011.html)

**Fragmentation — niveau applicatif : LoRa Alliance TS004 (FUOTA)**

- Objet de la spécification, cité à la source : « an application layer messaging package running over LoRaWAN to perform the following operations on a fleet of end-devices: – Setup, report and delete fragmentation transport sessions – **Several fragmentation sessions supported simultaneously** – Fragmentation over **multicast or unicast** – Send a fragmented block of data to one or many end-devices » — [LoRaWAN Fragmented Data Block Transport Specification v1.0.0 (PDF)](https://lora-alliance.org/wp-content/uploads/2020/11/fragmented_data_block_transport_v1.0.0.pdf)
- TS004 fait partie du jeu de spécifications FUOTA (mises à jour firmware par radio) du groupe de travail FUOTA du comité technique de la LoRa Alliance ; une version v2.0.0 existe — [LoRa Alliance, page de la spécification](https://resources.lora-alliance.org/document/lorawan-fragmented-data-block-transport-specification-v1-0-0)

**Signatures à état (LMS / XMSS) comme réponse documentée à la contrainte de taille**

- Position du BSI, citée à la source : les schémas à état sont recommandés « only in the special scenarios in which secure and error-free state management can be guaranteed. One such special scenario is, for example, **the signing of software and firmware updates** » — [BSI TR-02102-1 §5.3.4.3](https://www.bsi.bund.de/SharedDocs/Downloads/EN/BSI/Publications/TechGuidelines/TG02102/BSI-TR-02102-1.pdf)
- Et ils peuvent être employés **sans hybridation** : voir Q2, citations ANSSI (« SLH-DSA, XMSS et LMS ») et BSI (« in principle also be used alone »).
- Levier de taille documenté dans la RFC : le paramètre `w` de LM-OTS fait varier la signature OTS de **8 516 o (w=1) à 1 124 o (w=8)** pour la même sécurité — [RFC 8554, Table 1](https://www.rfc-editor.org/rfc/rfc8554.html)
- Clé publique LMS de **24 + m = 56 octets** (SHA-256) contre 1 312 o pour ML-DSA-44 : un facteur 23 sur la distribution de clé — [RFC 8554 §5](https://www.rfc-editor.org/rfc/rfc8554.html) vs [FIPS 204 Table 2](https://nvlpubs.nist.gov/nistpubs/FIPS/NIST.FIPS.204.pdf)
- Contrepartie documentée : nombre fini de signatures par clé (2^h), et « a **strictly monotonic counter must be incremented without error** » — [BSI TR-02102-1 §5.3.4.3](https://www.bsi.bund.de/SharedDocs/Downloads/EN/BSI/Publications/TechGuidelines/TG02102/BSI-TR-02102-1.pdf) ; « If a secret key state is used twice, no cryptographic security guarantees remain » — [RFC 8391](https://www.rfc-editor.org/rfc/rfc8391.html)

**Cantonnement aux messages rares — appui normatif**

- La feuille de route UE elle-même oriente vers cette approche en ciblant au jalon 2030 les « quantum-safe software and firmware upgrades [...] enabled by default » plutôt qu'une signature PQC sur tout le trafic — [Roadmap Part 1 v1.1, §4](https://ec.europa.eu/newsroom/dae/redirection/document/117507)
- Et elle mentionne explicitement l'alternative symétrique : « alternatives, such as **using symmetric methods instead of public-key cryptography** are also worthwhile to consider, depending on the application » — [Roadmap Part 1 v1.1, §4.1](https://ec.europa.eu/newsroom/dae/redirection/document/117507)

**[calcul] Coût réel de la fragmentation, par schéma et par SF**

Formule : `n_trames = ceil( taille_signature / N )`, puis `TX_total = n_trames × ToA(SF, N + 13)`, et `heures_de_budget = TX_total / 36 s`. Hypothèses de Q5 ; N = charge utile applicative de RP002 Table 13 (sans répéteur) ; surcoût de trame LoRaWAN de 13 o.

| Signature | Taille (o) | SF12, N=51 | SF9, N=115 | SF7, N=242 |
|---|---:|---|---|---|
| Falcon-512 | 666 | 14 tr., 39,1 s TX → **1,09 h** | 6 tr., 4,1 s → 0,11 h | 3 tr., 1,2 s → 0,03 h |
| LMS SHA256 w8 h10 | 1 444 | 29 tr., 81,0 s → **2,25 h** | 13 tr., 8,8 s → 0,24 h | 6 tr., 2,4 s → 0,07 h |
| ML-DSA-44 | 2 420 | 48 tr., 134,1 s → **3,72 h** | 22 tr., 14,9 s → 0,41 h | 10 tr., 4,0 s → 0,11 h |
| XMSS-SHA2_10_256 | 2 500 | 50 tr., 139,7 s → **3,88 h** | 22 tr., 14,9 s → 0,41 h | 11 tr., 4,4 s → 0,12 h |
| ML-DSA-65 (choix BSI) | 3 309 | 65 tr., 181,6 s → **5,04 h** | 29 tr., 19,6 s → 0,55 h | 14 tr., 5,6 s → 0,16 h |
| SLH-DSA-128s | 7 856 | 155 tr., 433,0 s → **12,03 h** | 69 tr., 46,7 s → 1,30 h | 33 tr., 13,2 s → 0,37 h |
| SLH-DSA-192s (choix BSI) | 16 224 | 319 tr., 891,1 s → **24,75 h** | 142 tr., 96,1 s → 2,67 h | 68 tr., 27,2 s → 0,75 h |
| *Ed25519 (référence)* | *64* | *2 tr., 5,6 s → 0,16 h* | *1 tr., 0,7 s → 0,02 h* | *1 tr., 0,4 s → 0,01 h* |

Lecture : « 3,72 h » signifie qu'émettre une seule signature ML-DSA-44 à SF12 consomme l'intégralité du budget de duty-cycle de 3,72 heures. **[calcul]**

### Inferences

- **La MTU de SCHC (2 520 o) est la borne dure la plus utile du dossier.** Elle couvre Falcon-512, LMS, ML-DSA-44 et XMSS-SHA2_10_256, mais **pas** ML-DSA-65 (3 309 o), ML-DSA-87 (4 627 o) ni aucun jeu SLH-DSA. Autrement dit : le jeu recommandé par le BSI (ML-DSA-65) ne passe pas dans un datagramme SCHC over LoRaWAN sans mécanisme supplémentaire, alors que ML-DSA-44 passe tout juste.
- **La fragmentation ne résout rien à SF12, elle convertit un problème de taille en un problème de temps.** Même la plus petite option (Falcon-512) coûte plus d'une heure de budget légal par signature. Les seuls régimes viables à SF12 sont : signature très rare (mise à jour firmware, enrôlement, révocation) ou signature à état avec paramètres agressifs.
- **Les deux contraintes se renforcent au lieu de se compenser** : SF12 donne la portée mais 51 o/trame et 12 trames/h ; SF7 donne 242 o/trame et 305 trames/h mais perd la portée. Il n'existe pas de point de fonctionnement où une signature PQC normalisée soit à la fois longue portée et fréquente.
- **La fiabilité de la fragmentation est un risque de second ordre documenté** : en multicast, RFC 9011 précise que No-ACK laisse la fiabilité à la couche supérieure. Une signature fragmentée en 48 à 319 morceaux n'est vérifiable qu'intégralement : une seule tuile perdue invalide toute la dépense de temps d'antenne. Avec ACK-on-Error et `MAX_ACK_REQUESTS = 8`, le coût des retransmissions s'ajoute au budget.
- **Le profil le plus défendable au vu des sources**, pour un nœud EU868 longue portée : signature à état LMS (LMOTS_SHA256_N32_W8 + LMS_SHA256_M32_H10/H20, ~1,4–1,8 kio, clé publique 56 o), sans hybridation (autorisée par l'ANSSI et le BSI), cantonnée aux messages rares et transportée par TS004/SCHC — exactement le scénario que le BSI désigne comme approprié. La contrepartie est l'obligation de gestion d'état sans faute, que le BSI pose comme condition explicite.

### Gaps

- **Signatures agrégées** : je n'ai trouvé aucune spécification (RFC, FIPS, LoRa Alliance) normalisant l'agrégation de signatures post-quantiques. Les travaux existants sont académiques et je n'ai pas identifié de source primaire fiable dans le temps imparti. **Aucune option normalisée d'agrégation à ce jour, à ma connaissance vérifiée.**
- Je n'ai pas lu le corps de TS004 (taille de fragment, nombre maximal de fragments, format de session) : seuls le résumé d'objet et le périmètre ont été cités. Les limites quantitatives de TS004 restent **non vérifiées à la source**, et il faudrait les comparer à la MTU de 2 520 o de SCHC.
- Je n'ai pas trouvé de spécification traitant explicitement du **transport de signatures post-quantiques sur LPWAN** (profil dédié, compression de signature, pré-distribution de clé publique). Les travaux publiés sur le sujet sont des articles de recherche ; aucun n'a été retenu ici faute d'avoir pu les vérifier comme sources primaires.
- Les mesures de **consommation énergétique et de temps de calcul** de la signature/vérification PQC sur microcontrôleur ne font pas partie de mon périmètre et n'ont pas été recherchées ; elles constituent un second facteur limitant indépendant du budget radio.
