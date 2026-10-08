# OASIS : vrai besoin ou gadget ? Monétisable ? Concurrence (2026-10-06)

Recherche documentaire. Les tailles de marché viennent de cabinets d'études et sont
peu fiables : à lire comme des ordres de grandeur. Aucune donnée client directe
n'existe encore (zéro entretien) : ce document dit où chercher, pas ce que les
clients pensent.

> Rappel, puisque ce document parle de marchés où le mot « sûreté » a un sens précis :
> **OASIS n'est pas une fonction de sûreté certifiée** (ni PL au sens d'ISO 13849-1, ni SIL
> au sens d'IEC 62061) et **ne réduit aucun risque machine**. Il décide si un ordre est
> authentique, habilité, frais et dans les limites. Voir
> [`docs/compliance/IEC_TS_63074.md`](../docs/compliance/IEC_TS_63074.md).

## 1. Verdict

| Question | Réponse courte |
|---|---|
| Vrai besoin ? | **Oui pour les actionneurs de terrain industriels et les machines autonomes** (incidents réels, réglementation). **Faible pour les drones civils européens**. **Secondaire pour les drones militaires**, où la menace dominante est le brouillage et le leurre GNSS, pas le vol de commande |
| Monétisable ? | **Oui, mais pas comme bibliothèque seule ni comme module « cyber » pour drones.** L'argent se trouve dans la vente aux **fabricants** (composant intégré, licence par produit) et dans la **gestion du cycle de vie** des identités et des révocations (service récurrent) |
| Gadget ? | **Le risque est réel** si OASIS reste un « mesh sécurisé » vendu aux opérateurs de drones : ce marché n'active même pas la signature MAVLink gratuite |

## 2. Le besoin : ce qui est prouvé, ce qui ne l'est pas

### Preuves d'un vrai besoin
- **FrostyGoop (Ukraine, janvier 2024)** : des commandes Modbus **non authentifiées** envoyées à des contrôleurs ont coupé le chauffage de **plus de 600 immeubles** pendant deux jours. Dragos recense environ **46 000 équipements ICS exposés** parlant ce protocole.
- **Unitronics (2023-2024)** : au moins **75 automates** compromis (eau et assainissement) par des mots de passe par défaut.
- **Réglementation** : Règlement Machines 2023/1230 (annexe III 1.1.9, protection contre la corruption **intentionnelle** et trace des interventions), CRA, directive RED (EN 18031). Voir `POSITIONING_GAPS.md` §A.

### Ce qui affaiblit le besoin
- **Les budgets OT vont ailleurs.** Enquête SANS 2025 : priorités = architecture réseau défendable, visibilité des actifs (n° 1 pour 50 %, puis 54 %), réponse aux incidents, accès distant. L'authentification des ordres **au niveau de l'équipement** n'apparaît pas dans le haut de la liste, et les budgets « restent en retard ».
- **L'échéance du Règlement Machines peut glisser.** En janvier 2026, plusieurs fédérations industrielles (CECIMO, CEMA et d'autres) ont demandé de **reporter les clauses de cybersécurité** (1.1.9 et 1.2.1) pour les aligner sur le CRA (11 décembre 2027), faute de normes harmonisées avant fin 2026. **La décision n'était pas connue dans mes sources** : à vérifier.
- **Drones militaires** : en Ukraine, la Russie cherche surtout à **aveugler** (brouillage, leurre GNSS), pas à prendre le contrôle. OASIS ne traite ni le brouillage ni le leurre GNSS.
- **Drones civils européens** : l'EASA a **retiré** les exigences de cybersécurité de SORA 2.5. La signature MAVLink 2, gratuite, est **désactivée par défaut** et souvent laissée ainsi : signe d'une demande faible.

## 3. Qui gagne de l'argent sur ce terrain, et comment

| Acteur | Ce qu'il vend | Signal économique | Leçon pour OASIS |
|---|---|---|---|
| **Silvus Technologies** | Radios MANET chiffrées, FIPS 140-3 niveau 2 | **Racheté par Motorola 4,4 Md$** (août 2025) | La valeur se capte dans la **radio certifiée**, la sécurité y est intégrée |
| **Doodle Labs**, **Persistent Systems** | Radios maillées pour drones (AES-256, Blue UAS) | Contrats défense (NGC2 : 87,5 M$ en 2026) | Même leçon : partenaire, pas concurrent |
| **Auterion** | Système d'exploitation de drones (PX4), essaims Nemyx | **Série B de 130 M$** (2025), contrat de 50 M$ | La sécurité est une fonction de la **plateforme** |
| **Mobilicom** (Nasdaq) | Liaisons de données et logiciel cyber ICE pour drones | **CA 2025 : 3,36 M$, perte nette : 23,8 M$** | **Le logiciel cyber seul pour drones se vend mal** |
| **Xage Security** | Zero trust pour l'OT (passerelles, contrôle d'accès) | +81 % de CA sur un an, 17 M$ avec l'US Space Force | L'OT paie, au niveau **réseau et entreprise** |
| **Veridify Security (DOME)** | Authentification d'équipements OT sur microcontrôleurs STM32, sans accélérateur crypto | Partenariat fabricant (KMC Controls), modèle SaaS | **L'analogue commercial le plus proche d'OASIS** : vente via les fabricants + service de gestion |
| **Kudelski IoT keySTREAM + Microchip ECC608** | Approvisionnement et cycle de vie des identités | Puce à 0,75 $ (10 000 pièces) + abonnement à l'usage | **L'argent récurrent est dans le cycle de vie des clés** |
| **wolfSSL** | Bibliothèques crypto embarquées | Double licence GPL / commerciale, **7 500 $ par produit** | Modèle viable pour une bibliothèque, **incompatible avec la licence MIT** |
| **Alias Robotics** | Sécurité des robots (ROS) : tests d'intrusion, produit RIS | Financé par Telefónica (Wayra) ; revenus non publiés | La sécurité robotique se vend d'abord en **services** |
| Gratuits | Signature MAVLink, secure boot PX4, SROS2, Reticulum | 0 € | Il faut un avantage net sur le gratuit, et il existe (voir `COMPETITIVE_ANALYSIS.md`) |

Taille de marché (indicatif) : « cybersécurité des drones » estimée à 3,1–3,5 Md$
en 2024-2025 et 8,8–10 Md$ en 2030 (CAGR 15–19 %), dont environ 2,1 Md$ attribués au
chiffrement des liaisons de commande. Rachats récents dans l'identité machine :
Venafi par CyberArk (1,54 Md$, 2024).

## 4. Ce que ça implique pour OASIS

1. **Vendre aux fabricants, pas aux opérateurs.** Le client est le constructeur de
   l'actionneur, du robot ou du contrôleur, qui doit prouver sa conformité (marquage
   CE, CRA, RED). L'opérateur n'achète pas un composant de sécurité ; il achète une
   machine conforme.
2. **Trois sources de revenus réalistes :**
   - **licence commerciale par produit** (modèle wolfSSL). Cela suppose de passer
     les **versions futures** en double licence (le code déjà publié sous MIT le
     reste) ;
   - **service de gestion** : enrôlement, révocation, vérification du journal
     infalsifiable, console opérateur (modèle Veridify, Kudelski). C'est le revenu
     récurrent ;
   - **intégration et pilotes**, et **subventions** (Fonds européen de la défense
     via consortium, BPI, EIC).
3. **S'allier aux radios et aux puces de sécurité** (fabricants de modules LoRa,
   Doodle Labs, Microchip, NXP) plutôt que les concurrencer.
4. **Segment de tête recommandé : actionneurs de terrain industriels et machines
   autonomes soumis au Règlement Machines** (vannes, pompes, portails, robots
   agricoles), avec la correspondance 1.1.9 / IEC TS 63074 / IEC 62443 comme
   argument. Le drone devient un **second segment**, via un intégrateur PX4 ou
   un fabricant de radios, pas en direct.
5. **Hypothèses à valider en entretien avant tout développement lourd :**
   - les fabricants d'actionneurs ont-ils un budget conformité « cybersécurité
     Règlement Machines / CRA », et qui le décide ?
   - préfèrent-ils un composant logiciel, une puce de sécurité, ou une passerelle ?
   - quel prix par produit ou par appareil est acceptable ?
   - la demande de report du Règlement Machines change-t-elle leur calendrier ?

## Sources

- Besoin, incidents : [Dragos via CyberScoop — FrostyGoop](https://cyberscoop.com/frostygoop-ics-malware-dragos-ukraine/) ; [The Record — 600 households without heat](https://therecord.media/frostygoop-malware-ukraine-heat) ; [CISA — Unitronics](https://www.cisa.gov/news-events/alerts/2023/11/28/exploitation-unitronics-plcs-used-water-and-wastewater-systems)
- Budgets OT : [SANS 2025 ICS/OT Budget](https://www.sans.org/white-papers/2025-ics-ot-cybersecurity-budget-spending-trends-challenges-future) ; [Industrial Cyber — OPSWAT/SANS survey](https://industrialcyber.co/reports/new-opswat-sans-survey-detects-growing-gap-in-ics-ot-cybersecurity-budgets-amid-rising-threats/)
- Report du Règlement Machines : [CEMA — joint industry position (PDF, 2026-01-19)](https://www.cema-agri.org/images/publications/News/2026-01-19-Joint_industry_Position_on_cybersecurity_provisions_in_the_Machinery_Regulation.pdf) ; [CECIMO — joint industry call](https://www.cecimo.eu/news/joint-industry-call-on-cybersecurity-and-machinery-regulation/) ; [IBF — postponement](https://www.ibf-solutions.com/en/news-and-knowledge/technical-papers-and-news-on-ce-marking/postponement-of-mr-requirements-for-cybersecurity-and-ai) ; [ETUC contre le report (PDF)](https://www.ibf-solutions.com/fileadmin/dateidownloads/etuc-position-on-industry-paper.pdf)
- Guerre électronique : [GIS — Ukraine's DIY drones defy jamming](https://www.gisreportsonline.com/r/ukraine-diy-drones/) ; [CEPA — Spoofing and spillover](https://cepa.org/article/spoofing-and-spillover-russia-targets-nato/)
- SORA 2.5 et MAVLink : [Blakistons — SORA 2.5](https://blakistons.co.uk/what-the-uk-drone-industry-can-learn-from-easas-adoption-of-sora-2-5/) ; [DEV — MAVLink not secure by default](https://dev.to/oliopti/mavlink-the-protocol-behind-millions-of-drones-and-why-it-isnt-secure-by-default-3n8m)
- Acteurs : [Silvus FIPS 140-3](https://silvustechnologies.com/blog/silvus-streamcaster-leading-the-way-the-industrys-first-manet-radio-to-receive-fips-140-3-level-2-validation/) ; [eRegion Research — drone supply chain (rachat Silvus)](https://eregionresearch.substack.com/p/drone-supply-chain-analysis) ; [Doodle Labs — Blue Framework](https://www.commercialuavnews.com/security/from-blue-to-you-doodle-labs-brings-blue-framework-smart-radios-to-the-commercial-sector) ; [Virginia Business — Auterion 130 M$](https://virginiabusiness.com/arlington-drone-software-firm-raises-130m-in-series-b-funding/) ; [Mobilicom — résultats 2025](https://www.manilatimes.net/2026/03/24/tmt-newswire/globenewswire/mobilicom-reports-2025-year-end-financial-results/2305920/amp) ; [Mobilicom — ICE Suite](https://mobilicom.com/products/ice-suite/) ; [Xage — record growth](https://xage.com/press/xage-security-hits-record-growth-by-architecting-the-future-of-zero-trust-for-ai-and-critical-infrastructure/) ; [Veridify — DOME](https://www.veridify.com/dome/) ; [ST Blog — Veridify on STM32](https://blog.st.com/veridify/) ; [Microchip — ECC608 TrustMANAGER](https://www.microchip.com/en-us/about/news-releases/products/microchip-technology-introduces-ecc608-trustmanager) ; [wolfSSL — license](https://www.wolfssl.com/license/) ; [Alias Robotics](https://aliasrobotics.com/)
- Marché : [Future Market Insights — drone cybersecurity](https://www.futuremarketinsights.com/reports/drone-cybersecurity-market) ; [Strategic Market Research — 10 Md$ en 2030](https://www.strategicmarketresearch.com/market-report/drone-cybersecurity-market) ; [Tracxn — embedded IoT security](https://tracxn.com/d/trending-business-models/startups-in-embedded-iot-security/__DUypkjv9WKt2jRI2X9Swq-nkCxVOKkHU-xuG5q6fhlk)
