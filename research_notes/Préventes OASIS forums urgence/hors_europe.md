# Préventes OASIS — opportunités hors Europe (USA, CA, UK, AU, NZ, JP, KR, SG, IN, IL, UAE/KSA, BR, ZA), 2026

Date de la recherche : 2026-10-10. Méthode : ~45 requêtes WebSearch (extended). **Limite majeure** : les
fetchs directs de pages (WebFetch + curl via le proxy) ont été refusés (DNS ENOTFOUND / CONNECT 403) ; toutes
les citations ci-dessous proviennent donc d'extraits de résultats de recherche, pas de la lecture intégrale des
pages. Aucune citation n'est inventée ; quand un extrait verbatim n'a pas pu être obtenu, c'est signalé.

**Constat transversal (honnête)** : la recherche web n'a fait remonter **aucun fil de forum communautaire
(r/PLC, r/wastewater, plctalk.net, control.com, discuss.px4.io, discuss.ardupilot.org, LinkedIn) daté
juin–octobre 2026** où un acheteur demande explicitement « comment signer/authentifier les commandes vers mes
PLC/RTU/onduleurs/drones » ou « journal inviolable des commandes HMI ». Les moteurs indexent mal ces forums
et les requêtes `site:` sont restées vides. Ce que la recherche a bien établi, en revanche, c'est un **contexte
de demande très fort et daté** (incidents eau USA juillet 2026, avis CISA/FBI/EPA, CVE PX4, règles MNRE Inde,
NIST SP 800-82r4, guide CISA « Why Johnny Can't Authenticate »), plus **quelques programmes de financement
non dilutif avec des dates Oct–Déc 2026**. Le classement ci-dessous est donc un classement de *fenêtres
d'opportunité documentées*, pas de fils de discussion individuels — les fils devront être cherchés à la main
(voir « Gaps » de chaque section, avec les requêtes/plateformes à ouvrir).

---

## Q1 — Eau USA après les attaques de juillet 2026 (Braham MN, 30+ systèmes, 7–12 États) : qui demande à stopper les écritures de consignes non autorisées / un audit trail des commandes HMI ?

### Takeaway
L'incident est massif et parfaitement daté (26–27 juillet 2026, >30 systèmes MN, confirmés dans ≥7 États,
Braham plante hors ligne ~1–2 h, PLC MicroLogix 1100/1400 exposés via modems cellulaires). Les mitigations
officielles (CISA AA26-097A mis à jour le 22 juillet 2026, CRS IF13298 du 27 août 2026) parlent de validation
des fichiers projet, de mode RUN physique, de journaux — **pas** encore de signature cryptographique des
ordres : c'est précisément l'angle où OASIS est différenciant. Aucun fil d'opérateur verbatim n'a été trouvé.

### Cited Findings
- Attaque coordonnée contre plus de 30 systèmes d'eau du Minnesota les 26–27 juillet 2026 ; Plymouth,
  South St. Paul, Maple Plain et Braham ont confirmé publiquement — [Wikipedia](https://en.wikipedia.org/wiki/2026_Minnesota_water_system_cyberattack) ; [The Hacker News](https://thehackernews.com/2026/07/coordinated-cyberattack-targets-30.html)
- Braham : les attaquants ont désactivé les commandes du puits et de l'usine de traitement, la ville dépendant
  temporairement du château d'eau ; durée de l'arrêt contestée (≈1 h vs ≈2 h, puis passage en manuel) — [LevelBlue](https://www.levelblue.com/blogs/spiderlabs-blog/review-of-the-july-2026-cyberattacks-against-u.s.-water-and-wastewater-systems) ; [CSA Research Note](https://labs.cloudsecurityalliance.org/research/csa-research-note-minnesota-water-utilities-ot-attack-202607/)
- Plymouth : PLC compromis sur 2 châteaux d'eau et 14 postes de relevage, équipements communiquant « via
  cellular connections commonly used for remote field access », déconnectés et passage en manuel — [StateScoop](https://statescoop.com/coordinated-cyberattack-disrupts-water-utilities-in-30-minnesota-communities/) ; [Dark Reading](https://www.darkreading.com/ics-ot-security/minnesota-water-utility-attacks-expose-sector-cyber-risks)
- FBI/EPA : « no novel exploit or custom malware, only MicroLogix 1100 and 1400 controllers exposed directly
  online » ; les attaquants ont modifié la configuration (adresses IP, mots de passe) — [Avertium](https://www.avertium.com/flash-notices/coordinated-attack-on-rockwell-micrologix-plcs-disrupts-water-systems-across-7-states) ; [Picus](https://www.picussecurity.com/resource/blog/minnesota-water-systems-attacks-internet-exposed-plcs-under-attack)
- CISA, 30 juillet 2026 : hausse significative de l'activité contre l'OT des services d'eau ; FBI/EPA :
  incidents dans ≥7 États depuis le 27 juillet ; des sources secondaires parlent de 12 États au 11 août — [Cybernews](https://cybernews.com/security/fbi-minnesota-water-hacks-7-state-iranian-cyber-campaign/) ; [Tenable](https://www.tenable.com/blog/coordinated-cyberattack-on-minnesota-water-utilities-what-you-need-to-know) ; [WaterVerge](https://www.waterverge.com/news/minnesota-water-systems-cyberattack-seven-states-2026/)
- CISA : « more than 100 internet-exposed water and wastewater systems targeted in July 2026 » (ciblés, pas
  forcément compromis) — [Hackread](https://hackread.com/cisa-hackers-targeted-internet-exposed-water-systems/)
- Attribution préliminaire Iran (CyberAv3ngers / IRGC), non formalisée — [CBS News](https://www.cbsnews.com/news/us-investigating-iran-cyberattack-minnesota-water-systems/) ; [Tenable](https://www.tenable.com/blog/coordinated-cyberattack-on-minnesota-water-utilities-what-you-need-to-know)
- **CISA AA26-097A** (publié 7 avril 2026, mis à jour 22 juillet 2026, étendu à Schneider et Siemens) :
  « prior to switching the device to run mode, review and validate project files, as changing modes will lock
  in the current project file downloaded to the device » ; revue des Add-On Instructions, comparaison au
  logique connue-bonne, isolation de l'architecture modem cellulaire, logs sur ports 44818, 2222, 102, 502 —
  [PDF CISA](https://www.cisa.gov/sites/default/files/2026-07/aa26-097a-iranian-affiliated-cyber-actors-exploit-programmable-logic-controllers-across-us-critical-infrastructure_508c.pdf) ; [WaterISAC relais](https://www.waterisac.org/tlpclear-cisa-updates-iranian-affiliated-plc-targeting-advisory-aa26-097a) ; [ComplianceHub](https://compliancehub.wiki/cisa-aa26-097a-update-iranian-plc-targeting-critical-infrastructure-july-2026/)
- Pour Siemens : « enable programming protection in PLC configuration software (S7 TIA Portal) to limit who
  can modify PLCs remotely » — [HowToFix](https://howtofix.guide/iranian-plc-attacks-cisa-siemens-schneider/)
- **CRS IF13298** « July 2026 Water System Cyber Incidents: Considerations for Congress », 27 août 2026
  (Elena H. Humphreys) : >70 % des systèmes inspectés par l'EPA depuis sept. 2023 en violation des exigences
  RRA/ERP ; le Congrès compare l'approche SDWA aux normes NERC ; évaluations cyber volontaires pour les petits
  systèmes — [congress.gov](https://www.congress.gov/crs-product/IF13298) ; [Industrial Cyber](https://industrialcyber.co/utilities-energy-power-water-waste/us-water-systems-face-persistent-cybersecurity-gaps-as-july-attacks-renew-scrutiny-of-federal-protections-crs-says/)
- Analyse SWIDCH : à Braham « someone remotely changed the PLC administrator password overnight » ; « shared
  credentials make audit trails meaningless » — [SWIDCH](https://www.swidch.com/resources/blogs/what-30-hacked-water-utilities-tell-us-about-ot-security)
- Analyse Elisity (reprenant CISA) : les acteurs ont extrait les fichiers projet via le logiciel de
  configuration, modifié la logique et manipulé les affichages HMI/SCADA, désactivant logique d'arrêt et
  alarmes — [Elisity](https://www.elisity.com/blog/epa-unveils-critical-cybersecurity-planning-tools-to-protect-water-systems-from-rising-cyberattacks)
- Détection recommandée : signaler toute programmation/transfert de logique hors fenêtres d'ingénierie
  approuvées (télémétrie protocole, journaux contrôleur, activité EWS) — [Shieldworkz](https://shieldworkz.com/blogs/investigative-cyber-threat-research-report-colorado-water-utilities-ot-attacks)
- Coût : MicroLogix 1100 discontinué, migration Micro800 recommandée = projet capex ; retirer l'exposition
  Internet = changement de configuration — [Substack T. McAllister](https://timmcallister.substack.com/p/iran-may-be-behind-the-water-attacks)
- Cadre fédéral : pas de règle cyber EPA autonome ; obligations AWIA/SDWA §1433 ; échéance RRA 30 juin 2026
  pour les systèmes 3 301–49 999 personnes ; New York : règles OT finalisées mars 2026, cœur OT au
  1er janvier 2027 (>3 300 personnes) — [Tenable](https://www.tenable.com/blog/water-utilities-cybersecurity-regulatory-compliance) ; [Trout Software](https://www.trout.software/resources/ny-water-cybersecurity-field-guide)
- Guides WaterISAC/AWWA trouvés datent d'août 2025 (Fundamentals, Small Systems) ; H2OSecCon 2026 contient
  une étude de cas « small utility whose HMI was manipulated remotely » — [WaterISAC](http://www.waterisac.org/h2oseccon-2026) ; [Ohio EPA PDF](https://dam.assets.ohio.gov/image/upload/epa.ohio.gov/Portals/28/documents/security/WaterISAC_Cybersecurity_12-Fundamentals_Small-Systems.pdf)
- Guide CISA « Barriers to Secure OT Communication: Why Johnny Can't Authenticate » (10 février 2026) :
  « Malicious actors with access to the OT network can exploit the legacy protocols' lack of integrity
  safeguards and device identity confirmation » ; priorise la **signature** (intégrité/authenticité) sur le
  chiffrement ; invite les acheteurs à exiger ces fonctions à l'achat — [CISA PDF](https://www.cisa.gov/sites/default/files/2026-02/Barriers-to-Secure-Communication-Why-OT-Johnny-Cant-Authenticate_508_2.pdf) ; [Industrial Cyber](https://industrialcyber.co/industrial-cyber-attacks/cisa-issues-new-ot-security-guidance-to-overcome-cost-and-complexity-barriers-in-critical-infrastructure/)
- NSA/CISA « Hardening OT with Zero Trust » CSI U/OO/6070841-26, publié 8 octobre 2026 — [media.defense.gov](https://media.defense.gov/2026/Oct/08/2004013274/-1/-1/0/CSI_HARDENING_OT_WITH_ZT.PDF)

### Inferences
- La fenêtre commerciale est **maintenant** (août–décembre 2026) : ~100 systèmes ciblés, budgets municipaux
  en réaction, État de New York avec échéance OT au 1er janvier 2027, CRS qui pousse le Congrès vers un modèle
  « NERC-like » pour l'eau. Les mitigations CISA restent « hygiène » (exposition, mots de passe, mode RUN) ;
  un gateway qui n'accepte que des ordres signés par origine + journal chaîné répond au trou laissé par le
  « shared credentials make audit trails meaningless ».
- Le vecteur « modem cellulaire → PLC MicroLogix (EtherNet/IP) » implique que l'offre OASIS doit être
  positionnée comme **passerelle en coupure devant le PLC existant** (Modbus RTU/TCP aujourd'hui ; EtherNet/IP
  non couvert — à dire honnêtement) plutôt que comme remplacement.
- Cibles de prise de contact : primacy agencies des États touchés (MN, MI, GA, SD, AL, NJ), MNIT, WaterISAC
  (relais AA26-097A), AWWA, intégrateurs locaux Rockwell.

### Gaps
- **Aucun fil r/PLC, r/wastewater, plctalk.net, control.com ou LinkedIn** avec demande verbatim trouvé via
  recherche web (3 requêtes dédiées vides). À ouvrir manuellement : `reddit.com/r/PLC` (recherche
  « Minnesota », « MicroLogix », « cellular », juillet–sept. 2026), `plctalk.net/threads`, `control.com/forums`,
  WaterISAC member portal (fermé), LinkedIn hashtags #WaterISAC #AA26-097A.
- Texte intégral AA26-097A et CRS IF13298 non lu (fetch bloqué) : vérifier si l'un ou l'autre mentionne
  explicitement « authentication of control commands » ou « command logging ».
- Aucun RFP municipal/étatique daté 2026 avec budget visible trouvé (sam.gov non interrogeable ici).

### Réponse-type (3 lignes) pour un fil « post-Minnesota » (EN puis FR)
EN : « Removing internet exposure is step one, but it doesn't stop a stolen VPN/HMI credential from writing a
setpoint. We built an open-source Rust gateway (Modbus TCP/RTU) that only forwards orders carrying a valid
per-operator Ed25519 signature, checks 9 conditions (expiry, replay, limits, revocation) and keeps a
hash-chained 5-year journal — happy to share the spec and a free pilot kit for small utilities. »
FR : « Retirer l'exposition Internet est la première étape, mais cela n'empêche pas un identifiant VPN/HMI volé
d'écrire une consigne. Nous avons construit une passerelle Rust open source (Modbus TCP/RTU) qui ne transmet
que des ordres portant une signature Ed25519 par opérateur valide, vérifie 9 conditions (expiration, rejeu,
limites, révocation) et tient un journal chaîné sur 5 ans — spec et kit pilote gratuit pour petits réseaux
disponibles. »

---

## Q2 — NERC CIP-003-9 / CIP-012, sécurité onduleurs/BESS (IEEE 1547.3, UL 2941, SunSpec), Californie Rule 21/CSIP, Australie CSIP-AUS/SOCI/AEMO backstop : qui demande des commandes de contrôle signées ?

### Takeaway
Les textes datés 2026 créent une obligation de *contrôle à distance* des onduleurs (backstop AEMO/NSW fin
2026, PRODIST Brésil en consultation, MNRE Inde) sans exiger explicitement la signature des commandes ; aucune
norme (UL 2941, IEEE 1547.3, SunSpec) n'a été trouvée imposant « signed control commands ». La demande est donc
latente, portée par les DNSP/AEMO et les régulateurs, pas par des fils d'acheteurs identifiés.

### Cited Findings
- CIP-003-9 effectif 1er avril 2026 (accès distant fournisseurs sur actifs low-impact : « methods for
  determining and disabling vendor access and detecting known or suspected malicious communications ») ;
  CIP-012-2 effectif 1er juillet 2026 (données temps réel entre centres de conduite) ; CIP-003-11 au
  1er juillet 2029 — [Certrec](https://www.certrec.com/blog/most-significant-nerc-cip-updates-for-2026/) ; [Ampyx](https://ampyxcyber.com/blog/new-cybersecurity-controls-for-vendor-access-to-low-impact-nerc-cip-assets) ; [Shieldworkz](https://shieldworkz.com/blogs/nerc-cip-003-9-is-here-what-you-need-to-know-before-the-april-2026-deadline)
- IEEE 1547.3-2023 = guide cyber DER (authentification, contrôle d'accès, chiffrement) ; IEEE 1547-2018 n'impose
  pas de cyber à l'interface DER mais exige des interfaces permettant d'ajuster modes et consignes — [IEEE SA](https://standards.ieee.org/beyond-standards/cybersecurity-standards-der/)
- UL 2941 « currently in development », mise à jour complète anticipée 2027 ; SunSpec Phase 2 référencera le
  draft UL 2941 — [DOEE DC](https://doee.dc.gov/sites/default/files/dc/sites/doee/service_content/attachments/DOEE_Cybersecurity_WhitePaper_Final.pdf) ; [NREL gaps](https://docs.nlr.gov/docs/fy26osti/92844.pdf) ; [NARUC](https://pubs.naruc.org/pub/364B7A14-D21C-D2DF-BB6B-FFF3F93E1D51)
- UL Solutions : nouveau programme de certification cyber onduleurs (février 2026) — [Solar Power World](https://www.solarpowerworldonline.com/2026/02/ul-solutions-develops-new-standard-for-solar-inverter-cybersecurity/)
- Australie : CSIP-AUS codifié SA TS 5573 (avril 2026) ; Ausgrid : « from late‑2026, new and upgraded solar
  inverters connected in NSW will need to support requirements introduced under the NSW Government's Emergency
  Backstop Mechanism » ; AEMO vise une réponse <60 min et des « fallback mechanisms » ; test annuel backstop
  25 août 2026 (~100 000 systèmes, source vendeur non vérifiée) — [Ausgrid](https://www.ausgrid.com.au/connections/connection-application-help/solar-support/information-for-technology-providers) ; [AEMO](https://www.aemo.com.au/-/media/files/initiatives/der/managing-minimum-system-load/learnings-from-industry-implementation-of-emergency-backstop.pdf?la=en) ; [Reslink](https://www.reslink.org/blogs/csip-aus-solar-compliance-2026-what-every-australian-epc-must-know/)
- Ausgrid, demande de pass-through AER 30 juin 2026 : poste « Software package (including CSIP-AUS compliant
  utility server, Public Key… » (extrait tronqué) — [AER PDF](https://www.aer.gov.au/system/files/2026-07/Ausgrid%20-%20Emergency%20Backstop%20Cost%20Pass%20Through%20Application%20-%20Application%20-%2030%20June%202026%20-%20Public.pdf)
- Australie SOCI : amendements 2026 visant les classes d'actifs à haut risque et « foreign ownership, control
  or influence » — [LK Law](https://www.lk.law/2026/04/stepping-up-security-incoming-amendments-to-australias-security-of-critical-infrastructure-regulatory-framework/)
- Brésil : ANEEL ouvre (sept. 2026) une consultation publique révisant PRODIST Module 3 pour batteries, GD et
  VE, avec « protocolos de comunicação padronizados, monitoramento em tempo real, resposta a comandos remotos »
  ; base existante RN 964/2021 — [Cenário Energia](https://cenarioenergia.com.br/2026/09/09/aneel-lanca-consulta-publica-para-revisar-prodist-e-regular-operacao-de-baterias-gd-e-carros-eletricos/) ; [ANEEL RN 964](https://www2.aneel.gov.br/cedoc/ren2021964.html)
- DOE CESER « Securing Energy Technology Resiliency (SENTRY) » CWX-024 : 3,3 M$, 4 thèmes dont « ICS/OT
  Cybersecurity Enhancement and Threat Emulation », « Secure Decentralized Asset Management Systems (DAMS) »
  et « Counter-Drone » ; **clos le 1er décembre 2025**, lauréats annoncés 14 septembre 2026 — [ConnectWerx](https://www.connectwerx.org/portfolio-items/cwx-024-ceser-securing-energy-technology-resiliency-sentry/) ; [CESER blog](https://www.energy.gov/ceser/listings/ceser-blog)

### Inferences
- Pour l'Australie, l'acheteur réel est le **DNSP / fournisseur de serveur CSIP-AUS** (Ausgrid, Essential,
  United Energy, Ergon) qui doit garantir que les commandes de curtailment ne soient ni forgées ni rejouées ;
  OASIS peut se positionner comme couche de signature d'ordre + journal au-dessus de l'IEEE 2030.5 (travail
  d'intégration à annoncer honnêtement : pas d'implémentation 2030.5 dans le dépôt).
- Pour NERC CIP-003-9, le besoin exprimé est « détecter/couper l'accès fournisseur » : le gate 9 conditions +
  révocation signée répond à « disabling vendor access » de manière démontrable à un auditeur.

### Gaps
- Aucune RFP utility / DNSP 2026 mentionnant « signed control commands » trouvée.
- Texte SA TS 5573 et CSIP-AUS non consultés : vérifier si une exigence d'authentification des commandes
  existe (au-delà de TLS/PKI 2030.5).
- Rule 21 / CSIP Californie : aucune mise à jour 2026 trouvée.
- Aucun appel DOE CESER 2026 spécifiquement onduleurs/BESS trouvé (voir Q6 pour le RMUC 100 M$).

---

## Q3 — Mines (AU, CL, CA, ZA) et pétrole-gaz (Golfe, Texas) : fils sur la sécurisation des écritures Modbus vers des actifs distants

### Takeaway
Aucun fil ni RFI/tender 2026 explicite n'a été trouvé ; les seuls signaux sont des contenus vendeurs et des
appels d'offres Eskom déjà clos. Le segment reste à prospecter directement.

### Cited Findings
- Modbus reste courant dans les mines (coexistence avec EtherNet/IP, PROFINET) — [Avanceon](https://www.avanceon.ae/2026/09/19/mining-automation-australia-plc-scada/)
- « industrial protocols (Modbus, DNP3, EtherNet/IP) lack authentication and encryption by design, allowing
  attackers on the network to read and write process values » — [NFM Consulting](https://nfmconsulting.com/knowledge/ot-cybersecurity-oil-gas/)
- Pare-feux industriels filtrant par protocole « ensures that only approved commands reach controllers » ;
  sites distants (plateformes, stations de compression) comme points d'entrée — [UTSI](https://utsi.com/2026/01/09/cybersecurity-solutions-for-oil-gas-operational-technology/)
- Eskom (ZA) : tender EDR + NIDS pour l'environnement OT NTCSA publié 3 mars 2026, clos 16 mars 2026 ; SOC
  managé Durban clos 2 octobre 2026 — [Tender Bulletins](https://tenderbulletins.co.za/department-company/eskom/) ; [EasyTenders](https://easytenders.co.za/tenders-for/eskom)
- UAE : Cyber Security Council + Dragos lancent un « OT Cyber Centre of Excellence » (Make it in the Emirates
  2026) ; forum « OT Security First » Abu Dhabi 4 février 2026 ; IA Standard v2.0 (2025) couvrirait l'OT
  (source vendeur) — [Economy Middle East](https://economymiddleeast.com/news/make-it-in-the-emirates-2026-uae-cyber-security-council-partners-with-dragos-to-launch-ot-cyber-centre-of-excellence/) ; [DTS Solution](https://www.dts-solution.com/uae-information-assurance-standard-v2-redefining-cyber-resilience-in-the-emirates/)
- Saoudie : OTCC-1:2022 reste la version courante ; contrôles 2-11-1-1 « audit trails on all OT/ICS assets »
  et 2-3-1-10 protection des logs ; NCNICC-1:2025 étend au privé (date contestée déc. 2025 / janv. 2026) —
  [NCA PDF](https://nca.gov.sa/otcc_en.pdf) ; [GRC Vantage](https://www.grcvantage.com/web/compliance/nca-otcc)

### Inferences
- OTCC 2-11-1-1 (« audit trails on all OT/ICS assets ») est le crochet réglementaire le plus direct du Golfe
  pour le journal chaîné d'OASIS ; l'acheteur est l'intégrateur local devant prouver la conformité NCA.

### Gaps
- Pas de fil mining/oil&gas 2026 identifié ; à chercher sur LinkedIn (groupes « Mining Automation », « ISA
  Oil & Gas »), AusIMM, forum Dragos community, et sur les portails Aramco/ADNOC (inaccessibles ici).
- Chili (Codelco, SONAMI) et Canada (mines) : aucune requête n'a rien donné.

---

## Q4 — Drones/robotique après PX4 CVE-2026-1579 : demande de liens de commande authentifiés (discuss.px4.io, ArduPilot, Dronecode, Blue UAS, NDAA, JP/KR)

### Takeaway
La CVE (CVSS 9.8, CWE-306, avis CISA ICSA-26-090-02) a rendu officiel que MAVLink non signé = shell distant ;
la mitigation PX4 (clé symétrique unique partagée, pas de rotation) est exactement la faiblesse qu'un
mécanisme par origine Ed25519 corrige. Aucun fil de forum 2026 ni réponse OEM n'a été trouvé ; Blue UAS exige
des « authentication mechanisms » sans texte public précis.

### Cited Findings
- CVE-2026-1579 : « Successful exploitation … could allow an attacker with access to the MAVLink interface to
  execute arbitrary shell commands without cryptographic authentication » ; SERIAL_CONTROL accepté non signé ;
  v1.16.0 (SITL listé par CISA) ; CVSS 9.8 (9.3 chez OpenCVE) ; EPSS <1 %, pas dans le KEV — [NVD](https://nvd.nist.gov/vuln/detail/CVE-2026-1579) ; [CISA ICSA-26-090-02](https://www.cisa.gov/news-events/ics-advisories/icsa-26-090-02) ; [OpenCVE](https://app.opencve.io/cve/CVE-2026-1579)
- Mitigation PX4 : activer la signature MAVLink 2.0 sur tous les liens non-USB — [Security Online](https://securityonline.info/px4-autopilot-mavlink-vulnerability-cve-2026-1579/)
- Modèle de signature PX4 : « a single secret key, shared by all MAVLink systems and components » ; « no
  automatic key rotation. Keys must be reprovisioned manually via a signed SETUP_SIGNING message » ; perte de
  clé = accès physique à la SD — [PX4 docs signing](https://docs.px4.io/main/en/mavlink/message_signing) ; [PX4 hardening](https://docs.px4.io/main/en/mavlink/security_hardening)
- ArduPilot issue #28736 : demande de signature par canal ; « enabling signing activates all the mavlink
  ports, including the mavlink port that communicates with my companion computer » (ouvert ~2024, statut à
  vérifier) — [GitHub](https://github.com/ArduPilot/ardupilot/issues/28736)
- CVE reprise dans le bulletin CSA Singapour du 1er avril 2026 — [CSA SG bulletin](https://isomer-user-content.by.gov.sg/36/22a6b6d1-5f2f-40bd-b5bd-7e5f79d94a31/Security%20Bulletin%2001%20Apr%202026%20[PDF,%201604KB].pdf)
- Blue UAS 2026 : Framework géré par DCMA US-X ; évaluation DCMA d'un composant « covered encryption,
  authentication mechanisms, and data-at-rest and data-in-transit protections » ; exemption FCC Covered List
  jusqu'à janvier 2027 ; Lantronix SOM compagnon listé 1er octobre 2026 — [Mobilicom](https://mobilicom.com/insight/blue_uas_framework/) ; [FlightBrief](https://flightbrief.news/guides/blue-uas-cleared-list/) ; [GlobeNewswire](https://www.globenewswire.com/news-release/2026/10/01/3372751/0/en/lantronix-secures-blue-uas-companion-computer-som-listing-expanding-defense-drone-opportunity.html)
- SENTRY (CESER) comportait un thème « Counter-Drone Capabilities for Critical Infrastructure » (clos) — [ConnectWerx](https://www.connectwerx.org/portfolio-items/cwx-024-ceser-securing-energy-technology-resiliency-sentry/)
- Parrot + SEALSQ (13 mars 2026) : PQC et firmware signé dans la prochaine génération de drones sécurisés
  (concurrence/benchmark, pas une demande) — [Barchart/GlobeNewswire](https://www.barchart.com/story/news/734042/sealsq-and-parrot-expand-their-strategic-partnership-parrot-to-integrate-sealsq-post-quantum-cryptography-into-its-next-generation-of-secure-drones)
- Japon : extension zone d'interdiction 300 m → ~1 000 m autour des installations importantes à partir du
  14 juillet 2026 ; Remote ID ; aucune exigence de lien de commande authentifié trouvée — [Fly Eye](https://www.flyeye.io/japan-drone-laws/)

### Inferences
- Message vendeur le plus crédible : « MAVLink signing = une clé partagée pour toute la flotte ; une fuite
  compromet tout ; OASIS v0B/Ed25519 par origine + révocation signée + journal donne la non-répudiation par
  opérateur » — à poster sur discuss.px4.io et discuss.ardupilot.org en réponse aux fils post-CVE (à localiser).
- Blue UAS/NDAA : voie indirecte via un intégrateur US (OASIS étant belge, pas listable directement).

### Gaps
- Aucun fil discuss.px4.io / discuss.ardupilot.org / Dronecode 2026 remonté par la recherche (2 requêtes) ;
  ouvrir manuellement les catégories « Security » de ces forums et le GitHub Security Advisory PX4.
- Pas de réponse OEM publique (Auterion, ModalAI, Skydio…) à la CVE trouvée.
- Korea KISA, Japon METI drones : rien de 2026 sur l'authentification des commandes.

### Réponse-type (3 lignes)
EN : « Enabling MAVLink signing closes CVE-2026-1579 but leaves one symmetric key shared by every GCS and
companion — one leak, whole fleet. We maintain an open-source Rust layer (PX4/MAVLink carrier) with per-origin
Ed25519 signed orders, a persisted anti-replay window, signed revocation and a hash-chained journal, with Kani
proofs; glad to share the spec or help wire it on a SITL. »
FR : « Activer la signature MAVLink corrige CVE-2026-1579 mais laisse une seule clé symétrique partagée par
toutes les GCS et compagnons — une fuite, toute la flotte. Nous maintenons une couche Rust open source
(porteuse PX4/MAVLink) avec ordres signés Ed25519 par origine, fenêtre anti-rejeu persistée, révocation
signée et journal chaîné, prouvés avec Kani ; spec disponible, aide possible sur SITL. »

---

## Q5 — Régulateurs (Japon METI/IPA, Corée KISA, Singapour CSA CCoP, Inde CERT-In/MNRE, Israël INCD, Saoudie NCA OTCC, UAE) : exigence 2026 d'authentification/journalisation des commandes ?

### Takeaway
Le signal le plus fort et le plus daté est l'**Inde (MNRE)** : TLS + IMEI obligatoires (mémo 27 mars 2026),
localisation des serveurs de contrôle sous 30 jours (fin août 2026), éoliennes : preuve de conformité au
31 août 2026 et « mesures empêchant le contrôle distant externe ». Singapour met à jour la CCoP (annoncée pour
fin 2026, « Release 1 » du 29 juillet 2026 selon une source vendeur non confirmée). Ailleurs, rien de 2026
n'exige nommément la signature des commandes.

### Cited Findings
- MNRE, directive fin août 2026 (PM Surya Ghar) : logiciels de monitoring, serveurs de contrôle et données
  temps réel sur des installations chiffrées en Inde ; « 30 days from the date of the order to comply »,
  confirmation écrite à REC Ltd, sinon interdiction d'installer — [SolarQuarter](https://solarquarter.com/2026/08/21/mnre-mandates-domestic-data-storage-for-solar-inverters-under-pm-surya-ghar-scheme/)
- MNRE, mémo 27 mars 2026 : « mandatory IMEI-based device identification and TLS certificates for
  authentication » ; règles juillet 2025 : connexion uniquement aux serveurs nationaux — [Qbits Energy](https://qbitsenergy.com/blog/mnre-inverter-data-localization-rules/) ; [SolarQuarter RMS](https://solarquarter.com/2026/04/02/mnre-issues-new-inverter-rms-and-datalogger-testing-guidelines-for-pm-surya-ghar-rooftop-solar-scheme/)
- MNRE éolien : conformité à prouver au 31 août 2026 ; « Operational control of wind turbines must be managed
  exclusively from facilities located in India » ; OEM doivent rapporter les « measures implemented to prevent
  external remote control of turbines » — [WindInsider](https://windinsider.com/2026/08/26/mnre-mandates-cybersecurity-compliance-reporting-for-wind-turbine-oems/)
- MNRE : auto-certification onduleurs >200 kW prolongée au 31 décembre 2026 — [SolarQuarter](https://solarquarter.com/2026/07/13/mnre-extends-self-certification-deadline-for-spv-inverters-above-200-kw-until-december-31-2026/)
- Singapour CSA : CCoP CII mise à jour « in the later part of this year » + Cloud CCoP, motivée par APT et IA
  ; Zavior affirme une « 2026 Release 1 » du 29 juillet 2026 remplaçant CCoP 2.0 Rev.1 (non confirmé par CSA)
  — [CSA press](https://www.csa.gov.sg/news-events/press-releases/cybersecurity-code-of-practice-for-critical-information-infrastructure-to-be-updated-to-address-apt-and-ai-enabled-threats/) ; [Zavior](https://zavior.ai/cii-details)
- Singapour : OT Cybersecurity Masterplan mis à jour (CSA) — [CSA](https://www.csa.gov.sg/news-events/press-releases/singapore-updates-operational-technology-cybersecurity-masterplan/)
- PSA Singapore tender « Cyber security risk assessment for IT and OT systems » 2026/ICP/CS/PSAC/4018828, clos
  4 mai 2026 — [PSA](https://www.singaporepsa.com/tender_uri/2-2-years-term-contract-for-provision-of-service-for-cyber-security-risk-assessment-for-it-and-ot-systems/)
- Japon : METI « OT Security Guidelines for Semiconductor Device Factories » (24 oct. 2025, EN/JP) avec
  « ⑥ Logical access restrictions (ID management, authentication, and access control) » ; MIC : ordonnance
  IoT terminaux (codes d'identification non devinables) effective avril 2027 — [METI PDF](https://www.meti.go.jp/policy/netsecurity/wg1/semiconductor_systems_guideline_ver1.0_eng.pdf) ; [Obsidian RI](https://obsidianri.com/blog/newsroom-jp-20260925-terminal-equipment-iot-security-ordinance)
- NIST SP 800-82 Rev.4 draft publié 21 septembre 2026, **commentaires jusqu'au 30 novembre 2026** — [NIST CSRC](https://csrc.nist.gov/pubs/sp/800/82/r4/ipd) ; [NIST news](https://www.nist.gov/news-events/news/2026/09/guide-operational-technology-ot-security-nist-requests-comments-draft-sp)
- Saoudie / UAE : voir Q3.

### Inferences
- Inde = marché à urgence réglementaire explicite (30 jours, 31 août, 31 déc.) et budget forcé chez les OEM
  onduleurs/éoliens (Chine exclue de fait) ; OASIS peut s'adresser aux OEM indiens/japonais/coréens vendant en
  Inde comme brique « contrôle distant authentifié et journalisé localement ».
- Soumettre un commentaire public sur SP 800-82r4 avant le 30/11/2026 (section authentification des commandes
  / journaux) est un levier de visibilité gratuit auprès des acheteurs US.

### Gaps
- Textes MNRE (OM 27 mars 2026, directive août 2026) non lus : vérifier s'ils exigent une *signature* des
  commandes ou seulement TLS/IMEI.
- CCoP « Release 1 » 2026 : non confirmée côté CSA ; contenu OT inconnu.
- Corée KISA, Israël INCD : aucune publication 2026 trouvée.

---

## Q6 — Programmes non dilutifs hors UE ouverts à une société belge, Oct–Déc 2026

### Takeaway
Deux fenêtres **confirmées** dans la période : **Alpha-Omega Q4** (candidatures 1–31 octobre 2026, décision
en décembre) et **DOE CESER RMUC via ConnectWerx** (100 M$, dépôt 20 octobre 2026 14h ET — mais réservé à des
intermédiaires à but non lucratif partenaires d'≥6 utilities : OASIS ne peut y aller qu'en sous-traitant d'un
intermédiaire). GitHub Secure Open Source Fund est « rolling ». La plupart des autres (SVIP, SENTRY, Innovate
UK, Cyber Runway, NCC Canada, CyberCall SG, AU) sont clos ou sans appel 2026.

### Cited Findings
- **Alpha-Omega Seasonal Grant Program** : cycles trimestriels (soumission / revue-co-design / décision) ;
  calendrier 2026–2027 « Q4 Cycle · October 1- October 31 · November 1- November 30 · December 1- December 31 »
  ; OpenJS WG : « next application window is October 1–31, with review in November and funding decisions in
  December » — [Alpha-Omega blog](https://alpha-omega.dev/blog/announcing-the-new-alpha-omega-seasonal-grant-program/) ; [How to apply](https://alpha-omega.dev/grants/how-to-apply/) ; [OpenJS issue #349](https://github.com/openjs-foundation/security-wg/issues/349) ; AWS +2,5 M$ (mars 2026) — [OpenSSF](https://openssf.org/press-release/2026/03/17/linux-foundation-announces-12-5-million-in-grant-funding-from-leading-organizations-to-advance-open-source-security/)
- **DOE CESER RMUC (Rural & Municipal Utility Cybersecurity)** : 100 M$ ; intermédiaires à but non lucratif
  fournissant outils/services/formation à ≥6 utilities éligibles ; administré par ConnectWerx (PIA) ;
  **deadline 20 octobre 2026, 14h ET** — [Industrial Cyber](https://industrialcyber.co/news/doe-launches-100-million-cybersecurity-assistance-initiative-for-rural-municipal-and-small-electric-utilities/) ; [Shale Mag](https://shalemag.com/small-utility-cybersecurity/)
- **GitHub Secure Open Source Fund** : 10 000 $/projet via GitHub Sponsors, « applications are accepted on a
  rolling basis and considered for all program sessions » ; une fenêtre « 10 août – 5 octobre 2026 » apparaît
  sur un agrégateur sans lien certain — [GitHub blog](https://github.blog/news-insights/company-news/announcing-github-secure-open-source-fund/) ; [Curioss](https://curioss.org/resources/funding-opportunities/) ; [formulaire](https://docs.google.com/forms/d/e/1FAIpQLScDBalom0XhmJrvyI3kwD7dZ-dD4_uhmLNysVXtA8fH_WUKoA/viewform)
- **NIST SP 800-82r4** : commentaires jusqu'au 30 novembre 2026 (pas un financement, mais une fenêtre
  d'influence gratuite) — [NIST](https://csrc.nist.gov/pubs/sp/800/82/r4/ipd)
- **DHS SVIP** : OTS 70RSAT21R00000006 clos 20 mai 2026 ; « if there are no open topic calls, we cannot accept
  applications » ; liste de diffusion DHS-Silicon-Valley@hq.dhs.gov ; non-traditional contractors — [HigherGov](https://www.highergov.com/contract-opportunity/silicon-valley-innovation-program-5-year-other-tra-70rsat21r00000006-k-955e6/) ; [GovTribe FAQ](https://govtribe.com/file/government-file/faq-svip-ots-20220526-dot-pdf)
- **CESER SENTRY** (3,3 M$) : clos 1er déc. 2025, lauréats 14 sept. 2026 — [ConnectWerx](https://www.connectwerx.org/portfolio-items/cwx-024-ceser-securing-energy-technology-resiliency-sentry/)
- **INL** : Industry Days 11–12 novembre 2026, Idaho Falls, « at Capacity » ; inscription Vendor Registration
  Portal requise ; CyTRICS = accord vendeur standardisé, pas d'appel 2026 — [INL](https://inl.gov/procurement/industry-engagement/) ; [CyTRICS one-pager](https://inl.gov/content/uploads/2024/04/CyTRICS_One-Pager_PUBLIC-approved.pdf)
- **UK** : Innovate UK « Secure Software for Resilient Growth » (5 M£) clos 29 avril 2026 ; « Contracts for
  Innovation: Cyber scale in critical sectors » clos 10 juin 2026 ; Cyber Runway (Plexal) « applications … now
  closed » — [IFS 2421](https://apply-for-innovation-funding.service.gov.uk/competition/2421/overview/3d6991fa-73b2-48c0-93eb-cc5393b5cf3d) ; [IFS 2452](https://apply-for-innovation-funding.service.gov.uk/competition/2452/overview/df0addbf-9146-4b7f-b8b5-e78470917675) ; [Plexal](https://www.plexal.com/our-work/cyber-runway/)
- **Singapour CyberCall** (CCDF jusqu'à 1 M S$) : dernière clôture 13 janvier 2026 ; pas d'appel ultérieur
  trouvé ; OT-ISAC Summit 8–9 octobre 2026 avec « Call for Presentations » — [CSA Annex B](https://isomer-user-content.by.gov.sg/36/86dd1eeb-792d-4c2f-9616-7f1106e051f2/Annex%20B.pdf) ; [CyberCall](https://cybercall.sg/) ; [OT-ISAC Summit](https://otisacsummit2026.netlify.app/)
- **Australie** : CI-UP (ASD) = service sur invitation, pas une subvention ; Cyber Invest NT clos 30 juin 2026
  ; « Growing and Professionalising the Cyber Security Industry » clos — [cyber.gov.au CI-UP](https://www.cyber.gov.au/business-government/protecting-devices-systems/assessment-evaluation-programs/critical-infrastructure-uplift-program) ; [business.gov.au](https://business.gov.au/grants-and-programs/growing-and-professionalising-the-cyber-security-industry-program)
- **Canada** : « The NCC is unable to run a 2026 Call for CSIN … Call in 2027 » ; Cyber Security Cooperation
  Program clos 25 sept. 2025 — [NCC](https://ncc-cnc.ca/call-for-proposals/) ; [HelloDarwin](https://hellodarwin.com/business-aid/programs/cyber-security-cooperation-program-2025)
- **Sovereign Tech Agency** (DE, hors périmètre géographique mais ouvert aux projets) : Fellowship 2026 clos
  6 avril 2026 ; Rust Foundation investie 2025–2027 — [STA Fellowship](https://www.sovereign.tech/news/fellowship) ; [STA Rust](https://www.sovereign.tech/tech/rust-foundation)
- **Israël INCD / CyberSpark** : aucun appel 2026 trouvé — [GlobalDeal](https://globaldeal.io/finder/articles/japan-israel-cybersecurity-market-entry)
- **EPA** : ~9 M$ (août 2025) pour systèmes ≥10 000 personnes ; rien pour <10 000 ; SLCGP prolongé au 30
  sept. 2026 — [Tenable](https://www.tenable.com/blog/water-utilities-cybersecurity-regulatory-compliance)

### Inferences
- Priorité 1 (fenêtre ouverte, éligibilité open source quel que soit le pays) : **Alpha-Omega Q4 — dossier
  avant le 31 octobre 2026** (angle : durcissement d'un gateway OT Rust `no_std` critique, preuves Kani,
  audit crypto externe manquant — explicitement listé « none » dans CLAUDE.md, ce qui est un bon objet de
  grant). Priorité 2 : GitHub SOSF (rolling, 10 k$). Priorité 3 : se greffer avant le 20/10 à un intermédiaire
  RMUC (APPA, NRECA, WaterISAC-like) comme fournisseur d'outil pour petites utilities. Priorité 4 : commentaire
  SP 800-82r4 (30/11) et mailing-list SVIP.

### Gaps
- Pages alpha-omega.dev, connectwerx.org, cybercall.sg non lues (fetch bloqué) : confirmer éligibilité d'une
  société belge (Alpha-Omega finance des *projets* OSS ; RMUC exige un intermédiaire US).
- Deadline 2026 GitHub SOSF non confirmée ; fenêtre « 10 août – 5 octobre » non attribuée avec certitude.
- Aucun programme Israël, Corée, Japon, Brésil, Afrique du Sud, UAE/KSA avec deadline Oct–Déc 2026 trouvé.

---

## Classement final des opportunités (par urgence × accessibilité pour un vendeur belge)

| # | Opportunité | Date / deadline | Plateforme / organisation | Rôle acheteur | Besoin (verbatim ou résumé) | Urgence / budget | Action OASIS |
|---|---|---|---|---|---|---|---|
| 1 | Alpha-Omega Q4 seasonal grant | Candidatures **1–31 oct. 2026**, décision déc. | alpha-omega.dev / OpenSSF (USA) | Bailleur OSS | « predictable four-quarter seasonal grant framework » | Fenêtre ouverte 21 jours restants ; montants historiques 100 k–500 k$ (Rust Foundation 460 k$ en 2024) | Dossier « audit + durcissement gateway OT Rust no_std » |
| 2 | DOE CESER RMUC 100 M$ (ConnectWerx) | **20 oct. 2026 14h ET** | energy.gov/ceser, connectwerx.org (USA) | DOE → intermédiaires non-profit, ≥6 utilities | outils, services, formation, partage de menaces pour petites utilities | 100 M$ ; 10 jours | Approcher APPA/NRECA/un ISAC pour être « outil » du dossier |
| 3 | Eau USA post-AA26-097A (MN, MI, GA, SD, AL, NJ…) + NY règle OT 1/1/2027 | juil.–déc. 2026 | WaterISAC, AWWA, primacy agencies, CRS IF13298 | Utilities <10 000 hab., intégrateurs Rockwell | « review and validate project files… » ; « shared credentials make audit trails meaningless » | >100 systèmes ciblés ; capex migration MicroLogix ; Congrès saisi | Kit pilote Modbus + journal ; poster réponse-type Q1 sur r/PLC, plctalk, LinkedIn |
| 4 | Inde MNRE (onduleurs PM Surya Ghar, éolien) | 30 jours (fin sept. 2026), 31 août 2026, 31 déc. 2026 | mnre.gov.in, REC Ltd (IN) | OEM onduleurs/éoliennes, régulateur | « measures implemented to prevent external remote control of turbines » ; TLS + IMEI obligatoires | Interdiction d'installer en cas de non-conformité | Offre « contrôle distant authentifié + journal local » aux OEM non-chinois |
| 5 | PX4 CVE-2026-1579 aftermath | avis 31 mars 2026, toujours ouvert | discuss.px4.io, ardupilot #28736, CISA ICSA-26-090-02 | OEM drones, intégrateurs Blue UAS | « single secret key, shared by all MAVLink systems » ; « no automatic key rotation » | CVSS 9.8 ; Blue UAS exemption FCC jusqu'à janv. 2027 | Réponse-type Q4 ; démo SITL |
| 6 | NIST SP 800-82r4 commentaires | **30 nov. 2026** | csrc.nist.gov (USA) | Régulateur | guide OT, authentification/intégrité des communications | Influence gratuite | Soumettre commentaire sur signature des ordres + journaux |
| 7 | Australie backstop NSW / CSIP-AUS | fin 2026 (Ausgrid « late‑2026 ») | Ausgrid, AEMO, Essential, United Energy (AU) | DNSP, fournisseurs serveurs CSIP-AUS | « capable of receiving and acting on signals communicated by the DNSP » | Déploiement obligatoire ; pass-through AER | Partenariat avec un fournisseur de serveur CSIP-AUS |
| 8 | Brésil ANEEL PRODIST Module 3 (consultation) | consultation ouverte sept. 2026 | aneel.gov.br (BR) | Régulateur | « resposta a comandos remotos » | Projet, pas de budget | Contribution à la consultation |
| 9 | Saoudie NCA OTCC 2-11-1-1 / UAE CSC + Dragos CoE | continu | nca.gov.sa, CSC UAE | Intégrateurs locaux | « audit trails on all OT/ICS assets » | Conformité obligatoire | Via intégrateur local |
| 10 | GitHub Secure Open Source Fund | rolling | github.com (USA) | Bailleur OSS | 10 k$/projet | Faible montant, rapide | Candidater |

Les éléments suivants ont été vérifiés **clos** et ne doivent pas être poursuivis en 2026 : DHS SVIP OTS
(20 mai 2026), CESER SENTRY (1er déc. 2025), Innovate UK Secure Software (29 avr. 2026) et Cyber scale
(10 juin 2026), Cyber Runway, NCC Canada (pas d'appel 2026), CyberCall SG (13 janv. 2026), Cyber Invest NT
(30 juin 2026), Eskom OT EDR/NIDS (16 mars 2026), PSA Singapore (4 mai 2026), STA Fellowship (6 avr. 2026).

### Réponse-type générique régulateur / RFI (3 lignes)
EN : « Our open-source OT command-authorisation gateway makes every PLC/RTU/inverter write carry a per-origin
Ed25519 signature, pass a 9-condition gate (authorised, unrevoked, unexpired, replay-free, within limits…)
and land in a hash-chained 5-year journal; it is no_std Rust with Kani proofs, speaks Modbus TCP/RTU today
and sits in front of existing controllers. We would welcome a pilot or a comment slot in your consultation. »
FR : « Notre passerelle open source d'autorisation de commandes OT fait porter à chaque écriture
PLC/RTU/onduleur une signature Ed25519 par origine, un passage par 9 conditions (autorisé, non révoqué, non
expiré, sans rejeu, dans les limites…) et une inscription dans un journal chaîné sur 5 ans ; Rust no_std avec
preuves Kani, Modbus TCP/RTU aujourd'hui, en coupure devant les automates existants. Nous proposons un pilote
ou une contribution à votre consultation. »
