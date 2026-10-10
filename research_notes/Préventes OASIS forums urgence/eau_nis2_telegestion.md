# Eau / assainissement / infrastructures critiques (BE, FR, NL, DE, LU) — besoins exprimés 2026 d'authentification des commandes OT, journal inaltérable, remédiation NIS2 / CyFun / ANSSI / BSI

Recherche effectuée le 2026-10-10. **Limites méthodologiques importantes** : (1) la récupération directe des pages (WebFetch / curl) était bloquée par le proxy pour ted.europa.eu, boamp.fr, tenderned.nl, marchesonline.com, cloudsecurityalliance.org, dutchitchannel.nl — toutes les citations ci-dessous proviennent donc des extraits de moteur de recherche, pas du texte intégral ; les CCTP / cahiers des charges n'ont pas pu être lus. (2) LinkedIn n'est pas indexé par la recherche utilisée : **aucun post LinkedIn ni fil de forum (Ovarro, Lacroix, forums.automation.fr, tweakers, r/wastewater) où un acheteur demande explicitement une solution n'a été trouvé**. Les « opportunités » exploitables sont donc quasi exclusivement des **marchés publics 2026** et des **cadres réglementaires** créant l'urgence. (3) La demande « verbatim du besoin » est satisfaite seulement quand l'extrait d'avis le permettait.

---

## Q0 — Liste classée des opportunités concrètes 2026 (acheteur identifié, texte public)

### Takeaway
Les seules demandes explicites et datées 2026 trouvées sont des **avis de marché** (FR, BE, NL, DE) qui lient renouvellement de télégestion (fin RTC/2G/3G) et « mise en conformité cybersécurité NIS2 », plus un marché belge wallon d'audits/réponse à incident sur systèmes industriels (Digit'Eaux). Aucun avis trouvé ne nomme textuellement « authentification des commandes » ou « journal de commandes inaltérable » ; c'est un angle que le vendeur doit *introduire* en réponse. Les délais : plusieurs avis sont déjà clos (à marquer « veille attribution / avenant »), les relances de Rives et Eaux (lot 4 télégestion) et d'Agen (3 M€) étaient ouvertes à l'été 2026.

### Cited Findings (classement par pertinence × urgence × ouverture)

**#1 — Rives et Eaux du Sud-Ouest (ex-CACG, Tarbes, FR) — barrages et stations de pompage — TED 130265-2026 (24/02/2026) + relance TED 428741-2026 (24/06/2026)**
- Verbatim (extrait TED) : « La fin programmée du Réseau Téléphonique Commuté (RTC) et de la 2G entre 2025 et fin 2026 rend obsolètes les solutions actuelles de télégestion » ; « Entre 2026 et 2029, un gros travail de rafraichissement des installations va être réalisé pour l'ensemble des sites de Rives et Eaux » ; « en mettant en place de nouveaux standards conforme à la directive NIS2 ». Procédure ouverte, fournitures ; objet : équipements de télégestion, transmission, automates industriels, onduleurs. — [TED 130265-2026](https://ted.europa.eu/fr/notice/130265-2026/pdf)
- Relance des lots 2 (IHM écrans tactiles) et 4 (matériel de télégestion) publiée au JOUE le 24/06/2026, après déclaration sans suite du 19/05/2026 ; pour le lot 4 « erreurs et imprécisions dans les pièces du marché ». Lots 1 (automates), 3 (modems industriels et communication), 6 (onduleurs) attribués ; lot 5 non relancé. Contact indiqué : m.lorinet@riveseteaux.fr. — [TED 428741-2026](https://ted.europa.eu/fi/notice/428741-2026/pdf) ; agrégateur [Patterno](https://www.patterno.de/en/ausschreibungen/tender/lieferung-von-geraeten-fuer-fernsteuerung-und-tarbes-cedex-jhfnp3tveybg3doxvtvg9ht6)
- Rôle acheteur : exploitant régional (SEM) de barrages/canaux/stations de pompage ; pays : France. Date limite de la relance : **non lue** (texte tronqué dans les extraits). Ce qu'ils ont tenté : première consultation lot 4 abandonnée pour pièces imprécises → besoin encore mal cadré → fenêtre pour proposer des exigences (authentification des ordres, journal signé).
- Réponse FR en 3 lignes (à poster / adresser à l'acheteur) :
  1. « Votre lot 4 est relancé parce que les pièces ne permettaient pas de départager les offres : proposez-nous d'y inscrire une exigence mesurable — chaque ordre vers un automate de pompage doit être signé et vérifié à la réception, le relais ne pouvant ni le forger ni le rejouer. »
  2. « OASIS fournit cette couche au-dessus de vos automates Sofrel/TBox/SCADAPack existants : passerelle Modbus RTU/TCP qui n'émet une trame que si l'ordre est authentifié, non révoqué, non expiré et dans les limites configurées, avec journal de décisions. »
  3. « Preuves reproductibles (tests, preuves formelles Kani, essais sur RP2040) disponibles ; nous pouvons les joindre au mémoire technique NIS2 du lot 4. »

**#2 — Digit'Eaux (Verviers, BE — intercommunale IT des opérateurs d'eau wallons, siège IPALLE au Comité de Stratégie Technologique) — marché 2026 « cyber-sécurité » 4 lots dont audits & réponse à incident sur systèmes industriels**
- Verbatim (extrait Patterno, en anglais) : « Digit'Eaux is seeking experienced partners for comprehensive cybersecurity audits, testing, and incident response support to safeguard its IT and industrial systems. » Lots : LOT-0001 audits/tests IT ; **LOT-0002 audits/tests cyber industriels** ; LOT-0003 incident response IT ; **LOT-0004 incident response industriel**. Procédure ouverte, services, CPV 79417000, « EU Funded: Yes », soumission électronique obligatoire. La fiche affichait « 3 days remaining (LOT-0001) » à la date de crawl (date exacte non lisible ; à vérifier sur e-Procurement BOSA / TED). — [Patterno — Digit'Eaux](https://www.patterno.de/en/ausschreibungen/tender/cyber-sicherheitsleistungen-fuer-digiteaux-verviers-e86y0u2ioaskk8szj617fqiy)
- Contexte : Digit'Eaux a « récemment renouvelé » son Comité de Stratégie Technologique « pour guider les choix en matière de technologies et de sécurité », avec un représentant IPALLE. — même source Patterno (extrait) ; aucune page Digit'Eaux primaire récupérable.
- Urgence : NIS2 belge — niveau Basic/Important démontré au 18/04/2026, certification CyFun/ISO 27001 au **18/04/2027** — [Vinçotte CyFun](https://www.vincotte.be/en/pages/cyfun-cyberfundamentals) ; [itdaily — 1 jaar NIS2](https://itdaily.com/blogs/security/1-jaar-nis2-in-belgium/).
- Ce qu'ils ont tenté : mutualisation IT des opérateurs wallons ; achat d'audits (donc pas encore de remédiation produit) → l'angle est de se positionner comme **remédiation** des constats PR.AA que les audits lot 2 vont produire.
- Réponse FR 3 lignes :
  1. « Vos audits industriels (lot 2) vont relever le même constat partout : des ordres Modbus vers les postes de pompage sans authentification ni journal probant — c'est exactement l'écart CyFun PR.AA/PR.PS que les évaluateurs noteront avant avril 2027. »
  2. « OASIS est la brique de remédiation : passerelle qui n'exécute qu'un ordre signé par un opérateur enrôlé, contrôle des bornes par registre, refus de rejeu même après coupure, trace de chaque décision — déployable devant les automates existants des intercommunales. »
  3. « Nous proposons un pilote sur un poste de relevage d'un membre (IPALLE/AIDE/inBW) avec rapport d'évidence réutilisable dans votre dossier de conformité. »

**#3 — Agglomération d'Agen (FR) — remplacement des automates de télégestion (eau potable, assainissement, eaux pluviales) — avis 26-34899 (2026DEA02, abandonné) → relance 26-63234 (2026DEA03), 3 000 000 € HT**
- Verbatim : « RELANCE » ; « la consultation précédente a été déclarée sans suite (redéfinition du besoin) » ; valeur estimée 3 000 000 € HT ; prix unitaires révisables ; visite de site obligatoire (« L'offre d'un candidat qui n'a pas effectué cette visite sera déclarée irrégulière »). Aucun extrait ne mentionne la cybersécurité (à vérifier dans le CCTP sur agglo-agen.net). — [Marchés Online 26-34899](https://www.marchesonline.com/appels-offres/avis/remplacement-des-automates-de-telegestion-sur-les-infra/ao-9584994-1) ; [Marchés Online 26-63234](https://www.marchesonline.com/appels-offres/avis/remplacement-des-automates-de-telegestion-sur-les-infra/ao-9626388-1) ; avis TED de la première consultation daté 08/04/2026 — [TED 240044-2026](https://ted.europa.eu/en/notice/240044-2026/pdf)
- Date limite relance : non lue. Rôle : collectivité (EPCI), France.
- Réponse FR 3 lignes :
  1. « Votre relance après « redéfinition du besoin » est le moment d'ajouter un critère que la plupart des automates ne couvrent pas : un ordre de marche/arrêt de pompe refusé s'il n'est pas signé, dans les bornes, et inédit. »
  2. « OASIS s'insère entre la supervision et les automates (Modbus RTU/TCP) sans changer de marque : ordres authentifiés, horodatés par l'actionneur, journal de décision. »
  3. « Nous pouvons fournir un paragraphe d'exigences prêt pour le CCTP et une démonstration sur un poste de relevage pilote. »

**#4 — Communauté de communes « CCRG » (FR, identité non résolue) — remise à niveau télégestion, 27 postes de relevage + 19 déversoirs d'orage — BOAMP 26-4741 (janvier 2026), DLRO 26/01/2026 (clos)**
- Verbatim : « Le présent marché a pour but la remise à niveau du matériel de télégestion en prévision de la suppression des lignes RTC, la 2G et la 3G » ; inclut automates, enregistreurs de données, communication 4G/5G « ainsi qu'une mise en conformité cybersécurité » ; 27 postes de relevage et 19 déversoirs d'orage. — [BOAMP 26-4741](https://www.boamp.fr/telechargements/FILES/PDF/2026/01/26-4741.pdf)
- Statut : clos ; opportunité = suivi de l'attributaire (intégrateur) et proposition d'avenant « authentification des commandes ». Nom complet de la collectivité non confirmé (gap).

**#5 — CC Maurienne-Galibier (FR) — « Modernisation de la télégestion et des ouvrages d'assainissement », station Calypso, Saint-Michel-de-Maurienne — procédure adaptée, travaux 5 mois, DLRO 09/10/2026**
- Objet et date limite d'après l'agrégateur. Aucune mention cyber dans l'extrait. — [Marchés Online ao-4319388-1](https://www.marchesonline.com/appels-offres/avis/modernisation-de-la-telegestion-et-des-ouvrages-d-ass/ao-4319388-1)
- Statut : clos la veille de la recherche → veille attribution.

**#6 — Hoogheemraadschap Hollands Noorderkwartier (HHNK, NL) — remplacement du poste central (« hoofdpost ») de procesautomatisering des watersystemen (gemalen, stuwen, meetpunten) — publication prévue 12/12/2025, en cours 2026**
- Mémo du conseil (nov. 2025) : financement accordé en octobre 2025 **à condition** que le projet montre comment la solution respecte les normes et la loi en cybersécurité ; les gemalen sont décrits comme objets vitaux pilotés par de l'OT de plus en plus connecté à l'IT. — [HHNK bestuurlijke informatie](https://hhnk.bestuurlijkeinformatie.nl/Document/View/2f4ab130-ef4b-4b15-bfcd-61f232bc6a9b)
- Antécédent : ICT Group a remporté le remplacement de l'automatisation de 15 STEP de HHNK, la cybersécurité étant citée parmi les motifs (date non visible). — [ICT Group](https://www.ict.eu/en/newsroom/news/ict-group-modernizes-15-wwtps-hhnk)
- Résultat 2026 / attributaire : non trouvé (gap). Réponse NL/FR à adapter : positionner OASIS comme exigence « commando-authenticatie en onweerlegbare logging » dans le hoofdpost.

**#7 — Waterschap Hunze en Aa's (NL) — SOC-dienstverlening TN-572646 (18/03/2026, nouvelle version mai 2026) + raamovereenkomst procesautomatisering & elektrotechniek 24 M€ / 10 ans, 3 attributaires (TED 307262-2026)**
- Nota van inlichtingen du 15/04/2026 : des soumissionnaires demandent comment lire « OT » ; réponse de l'acheteur : il gère des objets industriels tels que stuwen, sluizen, **rioolgemalen** et rwzi, l'OT couvrant PLC, SCADA, capteurs, variateurs et réseaux de communication. Critère d'attribution : 15 points pour l'expérience en procesautomatisering ; « Het is denkbaar dat dit in de toekomst wordt toegevoegd aan de SOC-dienstverlening ». Plan d'implémentation : début septembre, fin au plus tard 31/12/2026. — [TenderNed doc 14070552](https://www.tenderned.nl/papi/tenderned-rs-tns/v2/publicaties/424647/documenten/14070552/content) ; [TenderNed doc 14209908](https://www.tenderned.nl/papi/tenderned-rs-tns/v2/publicaties/424647/documenten/14209908/content)
- Raamovereenkomst PA/ET : « raamovereenkomst met drie opdrachtnemers voor procesautomatisering en elektrotechniek, geraamde omvang €24.000.000 excl. btw, maximale looptijd 10 jaar ». — [TED 307262-2026](https://ted.europa.eu/de/notice/307262-2026/pdf)
- Angle : l'OT n'est pas (encore) dans le SOC → un journal de commandes signé côté actionneur est précisément ce que le SOC ne verra pas.

**#8 — Allemagne — avis 2026 Fernwirk-/Automatisierungs-/Prozessleittechnik d'une Kläranlage (dtvp.de), exécution oct. 2026 → avr. 2028 ; Gemeinde Kaltenkirchen « Modernisierung wasserwirtschaftlicher Infrastrukturen » (dossier dispo à partir du 02/06/2026, BI-Medien D461802843) ; Stadtwerke Rosenheim (Netzleitsystem + Fernwirktechnik + IT-Sicherheit, EU-Ausschreibung prévue)**
- dtvp : « Sanierung der Fernwirk-, Automatisierungs- und Prozessleittechnik einer Kläranlage » ; rien sur Protokollierung/NIS2 dans l'extrait. — [dtvp Bekanntmachung](https://www.dtvp.de/Satellite/public/company/project/CXP4DNWMKP4/de/announcements/12617958/Bekanntmachung.pdf)
- Kaltenkirchen : « Ablösung bestehender Systeme und Integration in ein neues Fernüberwachungskonzept ». — [Kaltenkirchen Veröffentlichungstext](https://www.kaltenkirchen.de/de/rathaus-politik/Veroeffentlichungstext-Modernisierung-wasserwirtschaftlicher-Infrastrukturen.pdf)
- Rosenheim (présentation Vivavis 07/10/2025). — [Vivavis PDF](https://www.vivavis.com/wp-content/uploads/2025/10/2025-10-07_Niederl_Echoloten-Niederspannung.pdf)
- KPMG Law : « Pump- und Verteilerstationen sind oftmals rein IT-gesteuert über Fernwirktechnik » (contexte NIS2 énergie/eau). — [KPMG Law](https://kpmg-law.de/nis2-so-muessen-sich-energieversorger-vor-cyberangriffen-schuetzen/)

**#9 — Belgique — Cyber Security Coalition, OT/ICS Security Focus Group : réunions 17/06/2026 (Howest Bruges) et 16/09/2026 (BDO Zaventem) — passées ; prochaine session = canal pour poser la question « qui remédie PR.AA sur la télémétrie ? »**
- [CSC event 4](https://cybersecuritycoalition.be/event/ot-ics-security-focus-group-4) ; [CSC event meeting](https://cybersecuritycoalition.be/event/ot-ics-security-focus-group-meeting/). Aucun compte rendu public ni focus eau trouvé.

**#10 — Agglomération Le Mans Métropole (FR) — AMO exigeant compétence « cybersécurité au regard des directives REC et NIS2 » + procédé STEP (avis 24-72794, 2024 — ANCIEN mais signale une maîtrise d'ouvrage sensibilisée ; marché d'exploitation STEP 108 M€ HT 25-78701)**
- [BOAMP 24-72794](https://www.boamp.fr/telechargements/FILES/PDF/2024/06/24-72794.pdf) ; [BOAMP 25-78701](https://www.boamp.fr/telechargements/FILES/PDF/2025/07/25-78701.pdf)

### Inferences
- Le déclencheur d'achat dominant en 2026 (FR surtout) n'est pas la cyber mais la **fin du RTC/2G/3G** : la cyber NIS2 est ajoutée en clause. Le vendeur a plus de chances en se greffant sur ces renouvellements (clause CCTP, avenant à l'attributaire) qu'en attendant un appel d'offres « authentification des commandes ».
- Les relances (Rives et Eaux lot 4, Agen) montrent des acheteurs qui **ne savent pas spécifier** le lot télégestion : offrir un texte d'exigence vérifiable est le levier le plus concret.
- Pour la Belgique, la fenêtre « audits (Digit'Eaux) → constats → remédiation avant 18/04/2027 » est la plus alignée avec OASIS.

### Gaps
- Aucune date limite lue pour les relances Rives et Eaux (428741-2026) et Agen (26-63234) ; CCTP non consultés.
- Identité complète de « CCRG » (26-4741) non résolue.
- Aucun avis belge publicprocurement.be/TED 2026 trouvé pour SWDE, Vivaqua, CILE, inBW, IPALLE, IDEA, IDELUX, IGRETEC, AIDE, Aquafin (seul un marché Aquafin de génie civil, juin 2026, DL 28/08/2026), De Watergroep, Farys, Pidpa, Water-link portant sur la cyber OT.
- Aucun avis trouvé pour Vitens, Evides, Waternet, Rijkswaterstaat, Veolia, Suez, SAUR en 2026 sur ce sujet.
- Luxembourg : rien sur SIDEN/SEBES/SIDERO.

---

## Q1 — Communauté Ovarro TBox / TWinSoft / Club utilisateurs Lacroix Sofrel : fils 2026 sur la sécurisation des commandes Modbus/TBox après « TWinSoft 12.8.4 (déc. 2025) »

### Takeaway
**Aucune source publique ne confirme l'existence d'une version TWinSoft 12.8.4 (déc. 2025) ni de correctifs « access level » associés** ; aucun fil de forum Ovarro/Lacroix 2026 n'a été trouvé. Les seules références publiques sont les avis CISA/INCIBE 2021 et 2023 sur TBox/TWinSoft.

### Cited Findings
- CISA ICSA-21-054-04 : toutes les versions antérieures à TWinSoft 12.4 et firmware TBox 1.46 affectées ; CVE-2021-22646 (injection de code, CVSS 8.8, PR:L) ; CVE-2021-22644 utilisateur « TWinSoft » codé en dur (CVSS 9.8) ; CVE-2021-22648 fonctions d'accès fichier Modbus propriétaires permettant d'altérer/supprimer la configuration (CVSS 8.8). — [CISA ICSA-21-054-04](https://us-cert.cisa.gov/ics/advisories/icsa-21-054-04) ; [INCIBE CVE-2021-22648](https://www.incibe.es/en/incibe-cert/early-warning/vulnerabilities/cve-2021-22648) ; [INCIBE CVE-2021-22646](https://www.incibe.es/en/incibe-cert/early-warning/vulnerabilities/cve-2021-22646)
- CVE-2023-36611 : les RTU TBox affectés permettent à des utilisateurs à faible privilège d'accéder aux jetons de sécurité logiciels de privilège supérieur ; avis CISA du 29/06/2023 relayé par le Centre canadien (AV23-369). — [cyber.gc.ca AV23-369](https://cyber.gc.ca/en/alerts-advisories/control-systems-ovarro-security-advisory-av23-369) ; [CVE list Ovarro](https://www.cvelogic.com/vendor/ovarro)
- Recommandation saoudienne de passer à TWinSoft 12.5+ (ancienne). — [CERT SA](https://cert.gov.sa/en/security-warnings/ovarro-update342/)
- Sofrel S4W : famille de postes locaux « conçu pour répondre aux nouveaux besoins des exploitants de réseaux d'eau », « une des premières solutions de cybersécurité spécialement dédiée aux acteurs de l'eau » ; LX CONNECT « Automatisation de la cybersécurité » / mise à jour de masse. — [Lacroix S4W DC68](https://www.lacroix-environment.com/wp-content/uploads/3/2025/02/25/dc68-gamme-sofrel-s4w-fr-2024-04-1.pdf) (brochure fabricant)
- Lacroix a subi une cyberattaque ciblée en mai 2023 (sites Electronics FR/DE/TN). — [DILA/AMF 2023](https://echanges.dila.gouv.fr/OPENDATA/AMF/MKW/2023/05/FCMKW139981_20230515.pdf)
- Guide SCADAPack Cybersecurity (Schneider, 47x/47xi/57x), v1.0 du 07/01/2025. — [SE SCADAPack guide](https://www.se.com/us/en/download/document/SCADAPack_Cybersecurity_Guide/)

### Inferences
- L'historique public TBox (privilèges faibles → jetons supérieurs ; fonctions fichier Modbus propriétaires) rend crédible l'argument « le niveau d'accès de l'automate ne protège pas l'ordre lui-même » — à utiliser sans affirmer de faits sur 12.8.4.

### Gaps
- Pas de confirmation de « TWinSoft 12.8.4 » ni de ses notes de version (chercher sur le portail support Ovarro, non indexé).
- Aucun fil « Club utilisateurs Sofrel » ou forum Ovarro 2026 trouvé ; rien sur Perax ou WIT.

---

## Q2 — Belgique : Cyber Security Coalition (focus group OT/ICS), CCB CyFun OT, Agoria — qui demande une remédiation PR.AA sur la télémétrie ?

### Takeaway
Le cadre existe (CyFun 2025 « more focus on OT », jalons 18/04/2026 et 18/04/2027) et le focus group OT/ICS s'est réuni deux fois en 2026, mais **aucune question publique nominative sur la remédiation PR.AA en télémétrie eau** n'a été trouvée ; les seuls signaux acheteurs belges sont Digit'Eaux (marché audits, Q0 #2) et la posture ISO 27001 des flamands.

### Cited Findings
- CCB : « CyFun® 2025 is now officially available » ; améliorations clés dont « more focus on OT », chaîne d'approvisionnement, contrôles plus clairs pour l'audit ; CyFun 2023 et 2025 coexistent « for a while », puis seule la 2025 sera acceptée (page mise à jour 29/10/2025). — [CCB — CyFun 2025](https://ccb.belgium.be/news/cyfunr-2025-here)
- Jalons belges : enregistrement avant 18/03/2025 ; niveau Basic/Important démontré au plus tard 18/04/2026 ; certification CyFun ou ISO 27001 au plus tard 18/04/2027. — [Vinçotte CyFun](https://www.vincotte.be/en/pages/cyfun-cyberfundamentals) ; [itdaily](https://itdaily.com/blogs/security/1-jaar-nis2-in-belgium/)
- « from 18 April 2026 essential organisations in the most critical sectors must demonstrably comply with the NIS2 obligations, at least at the so-called basic level » (blog fournisseur). — [D3 Security](https://d3security.com/blog/nis2-soc-audit-readiness-2026/)
- Focus group OT/ICS : 17/06/2026 Howest Bruges ; 16/09/2026 BDO Zaventem ; sessions 2025 le 24/03 (Bruxelles) et 25/06 (Bruges). — [CSC](https://cybersecuritycoalition.be/event/ot-ics-security-focus-group-4) ; [CSC](https://cybersecuritycoalition.be/event/ot-ics-security-focus-group-meeting/)
- Farys : « Farys and all Flemish water utility companies chose to meet the ISO 27001 standard » ; le CIO indique que Farys relève de NIS2 et coopère avec Pidpa et De Watergroep sur la plateforme de compteurs numériques. — [Orange Cyberdefense — Farys](https://orangecyberdefense.com/be/customer-stories/farys) ; [itdaily — CIO Farys](https://itdaily.com/cio/cio-farys/)
- Vinçotte : les sociétés d'eau potable « need to comply to IEC 62443 to be demonstrably in control under NIS2 » (source fournisseur, accompagnement d'une société d'eau potable non nommée). — [Vinçotte blog](https://www.vincotte.be/en/blog/vincotte-provides-support-to-drinking-water-company-to-comply-with-the-international-cybersecurity-standard-for-ot)
- Wallonie : 20 % des entités publiques wallonnes ont subi au moins un incident de sécurité informatique en deux ans (AdN, cité dans la fiche CyberWal). — [CyberWal fiche](https://s3.wallonie.be/files/Documents/Fiches%20IIS%202024%20VF/CyberWal_fiche_VF.pdf)
- IDELUX : centrale d'achat cybersécurité pour pouvoirs publics (2023, RHEA Group + EASI), hors eau. — [IDELUX CP 2023](https://www.idelux.be/sites/default/files/2023-09/20230929-communique_de_presse-cybersecurite_centrale_achat_vfinale.pdf)
- Non corroboré : un tracker affirme une première amende NIS2 belge (185 000 €). — [Legiscope](https://www.legiscope.com/blog/nis2-enforcement-tracker-2026.html) (fiabilité incertaine)

### Inferences
- La voie ISO 27001 choisie par les flamands (Farys, De Watergroep, Pidpa, Water-link) déporte la question OT vers IEC 62443 : l'argument OASIS doit être formulé en termes IEC 62443-3-3 SR 1.x/3.x (authentification, intégrité des commandes, non-répudiation) plutôt qu'en « PR.AA » seul.

### Gaps
- Aucune publication Agoria 2026 sur la télémétrie eau trouvée ; aucun Q&A CCB OT public ; comptes rendus du focus group non publics.

---

## Q3 — France : « ANSSI PA-110 (avr. 2026) », réactions LinkedIn/forums, « télégestion cybersécurité NIS2 station de pompage » 2026, FNCCR, Astee, SIAAP, Eau de Paris

### Takeaway
**Aucune trace d'un document « ANSSI PA-110 » (avril 2026)** : les références ANSSI retrouvées sont PA-107 (méthode de classification v2, mars 2025) et **PA-108 (« Mesures détaillées » v2, 27/11/2025)**. La transposition française de NIS2 (loi « Résilience ») n'était toujours pas achevée au 02/09/2026 ; l'urgence FR est portée par les 46 incidents eau notifiés à l'ANSSI (2021-2024) et par les renouvellements télégestion fin RTC/2G.

### Cited Findings
- ANSSI « Mesures détaillées » v2.0 publiée le 27/11/2025, référence ANSSI-PA-108 ; traduit les classes en mesures concrètes OT ; méthode de classification v2 (PA-107) mars 2025. — [cyber.gouv.fr — Mesures détaillées v2](https://messervices.cyber.gouv.fr/documents-guides/Guide_Systemes_industriels__Mesures_detaillees_v2.pdf) ; [Wavestone — refonte du guide](https://www.riskinsight-wavestone.com/en/?p=29433)
- CERT-FR (28/11/2024) : entre janvier 2021 et août 2024, 46 incidents concernant le secteur de l'eau, dont les trois quarts émanant de collectivités. — [Banque des Territoires](https://www.banquedesterritoires.fr/des-services-deau-et-dassainissement-tres-vulnerables-face-aux-cyberattaques)
- Astee : groupe de travail et guide pour « mettre les collectivités en ordre de marche en matière de cybersécurité » dans l'eau. — [Banque des Territoires — guide](https://www.banquedesterritoires.fr/services-deau-et-dassainissement-un-guide-pour-aider-les-collectivites-parer-les-cybermenaces)
- Transposition : « au 2 septembre 2026, le processus législatif français n'est pas encore achevé » (projet de loi Résilience). — [CCI Lyon](https://www.lyon-metropole.cci.fr/actualite/directive-nis2-quelles-obligations-de-cybersecurite-pour-les-entreprises) ; mises en demeure de la Commission nov. 2024 et mai 2025 ; Sénat mars 2025, commission spéciale AN sept. 2025. — [revue EIN 13528](https://www.revue-ein.com/download-article/13528)
- > 1 400 collectivités concernées par NIS2 (IDATE). — [IDATE](https://idate.fr/wp-content/uploads/2024/11/IDATE-Transposition-de-la-directive-NIS2-en-France.pdf)
- Journée NIS2 (Les Interconnectés, Bordeaux, oct. 2025) ; aucune journée FNCCR/Astee 2026 trouvée. — [Maire-info](https://www.maire-info.com/cybermalveillance/cybersecurite-et-directive-nis-2-ou-en-est-on-art26-30090)
- Délégataires : Veolia (Lerne) et Saur (IODA) développent leur supervision en interne, avec Sofrel/Paratronic en télégestion ; « Les réseaux OT […] sont désormais interconnectés aux systèmes IT et, dans certains cas, exposés à Internet ». — [revue EIN 13986](https://www.revue-ein.com/download-article/13986)
- Fiche ANSSI (cible CSPN 2021) décrivant le poste local de télégestion « installé sur des sites isolés tels que station de pompage, poste de relèvement » (contexte produit Sofrel, ancien). — [ANSSI cible CSPN 2021_15](https://cyber.gouv.fr/sites/default/files/2021/08/anssi-cible-cspn-2021_15fr.pdf)
- Clubic : une commune française a vu son SI chiffré, perturbant la gestion de l'eau et la facturation (date non précisée dans l'extrait). — [Clubic](https://www.clubic.com/actualite-545489-l-eau-est-precieuse-pourtant-elle-devient-aussi-source-de-nombreuses-cyberattaques-en-france.html)

### Inferences
- Le terme « PA-110 » de la demande est probablement une confusion avec PA-108 (nov. 2025) ou un document non publié/non indexé ; à ne pas citer sans vérification sur cyber.gouv.fr.
- Les réactions LinkedIn (Clusif, CESIN) n'étant pas indexées, les cibles FR exploitables restent les marchés (Q0) et les relais presse (revue EIN, Maire-info).

### Gaps
- Aucun post SIAAP, Eau de Paris, FNCCR 2026 trouvé ; forum-ics.fr non trouvé ; aucune réaction LinkedIn capturée.

---

## Q4 — Pays-Bas : Waterschappen / Unie van Waterschappen, Rijkswaterstaat, Vewin, tweakers 2026

### Takeaway
Le cadre néerlandais a basculé en 2026 (Cyberbeveiligingswet en vigueur le 15/08/2026, CERT-WM reconnu CSIRT sectoriel des waterschappen) ; les signaux acheteurs sont des marchés (HHNK hoofdpost, Hunze en Aa's SOC + PA/ET 24 M€) et un constat presse récurrent : les PLC des sluizen/gemalen ne reçoivent pas de mises à jour. Aucun fil tweakers/security.nl 2026 trouvé.

### Cited Findings
- CERT-WM « officieel aangewezen als sectorale CSIRT » des waterschappen, suite à la Cyberbeveiligingswet (transposition NIS2) entrée en vigueur le 15/08/2026 ; « digitale systemen besturen gemalen, monitoren waterstanden en beheren afvalwaterzuivering ». — [Dutch IT Channel, 24/08/2026](https://www.dutchitchannel.nl/news/758262/cert-wm-als-sectorale-csirt-wettelijk-erkend-onder-cyberbeveiligingswet)
- Binnenlands Bestuur : « waterschappen hebben de beveiliging van sluizen en gemalen niet op orde » ; PLC supportés ≈ 5 ans pour une durée de vie ≈ 25 ans ; incident gemaal Veere (2010). — [Binnenlands Bestuur](https://www.binnenlandsbestuur.nl/digitaal/ravijnjaar/waterschappen-worstelen-met-beveiligingsupdates-sluizen)
- WUR/NOS : « De bedieningssystemen van sluizen en pompen gaan vaak tientallen jaren mee, maar krijgen niet op alle locaties tijdig een beveiligings-update ». — [edepot WUR](https://edepot.wur.nl/443104)
- CSIR (Rijkswaterstaat) adapté aux waterschappen pour sécuriser rwzi et gemalen ; BIACS reprend la structure CSIR ; Hudson Cybertec collabore avec RWS et waterschappen à la mise à jour du CSIR ; Kiwa + Hudson Cybertec certifient l'OT des sociétés d'eau potable. — [Hudson Cybertec / WaterForum 2024](https://hudsoncybertec.com/app/uploads/2024/10/Hudson-Cybertec-pag-57-59-WaterForum-5-2024.pdf) ; [Kiwa](https://www.kiwa.com/en/insights/stories/uniting-forces-for-cybersecurity-in-the-water-sector/)
- Waterschap Limburg : doit respecter NIS2 et CSIR ; OT monitoring + SOC managé Siemens (référence fournisseur). — [Siemens reference](https://references.siemens.com/en/reference/waterschapsbedrijf-limburg?id=43478)
- Spécification « Cybersecurity OT eisen » v1.1 (2022) hébergée sur TenderNed pour l'achat de systèmes OT. — [TenderNed doc 9349465](https://www.tenderned.nl/papi/tenderned-rs-tns/v2/publicaties/302944/documenten/9349465/content)
- Unie van Waterschappen (2023) : de plus en plus d'actifs (gemalen, rwzi, sluizen, stuwen, bruggen) sont automatisés et pilotés à distance. — [Onderzoeksraad — réaction UvW](https://onderzoeksraad.nl/wp-content/uploads/2023/11/reactie_unie_van_waterschappen.pdf)
- Marchés : voir Q0 #6 (HHNK) et #7 (Hunze en Aa's).

### Inferences
- L'argument « PLC non patchable pendant 20 ans » plaide pour une couche externe d'authentification des ordres (passerelle) plutôt que la mise à niveau des automates — exactement le positionnement « brownfield » d'OASIS.

### Gaps
- Aucun post Vewin / UvW 2026 ; aucune discussion tweakers/security.nl 2026 ; aucun marché Vitens/Evides/Waternet/RWS 2026 trouvé.

---

## Q5 — Allemagne : BSI ICS-Kompendium (authentification niveau 0), DVGW W 1060 / DWA M 1060, forums Wasserwirtschaft 2026

### Takeaway
Aucun contenu 2026 du BSI ou de la DVGW spécifique à l'authentification des commandes de télécontrôle n'a été trouvé ; le B3S Wasser/Abwasser (W 1060 / M 1060, édition 2024-09) reste le cadre, à réviser tous les deux ans. NIS2 est applicable en Allemagne depuis le 06/12/2025 (ou 05/12 selon la source), seuils 50 salariés / 10 M€.

### Cited Findings
- B3S Wasser/Abwasser construit sur DVGW W 1060 + DWA M 1060 ; approuvé par le BSI ; à adapter tous les deux ans (BSIG) ; DIN liste l'édition 2024-09. — [DVGW B3S](https://docshare.dvgw.de/medien/dvgw/sicherheit/1708-ewp-brachenstandard-it-sicherheit-wasser-abwasser.pdf) ; [DVGW standard petit opérateur](https://ssl.dvgw.de/medien/dvgw/sicherheit/it-sicherheitsstandard-wasser-kleinunternehmen-2201.pdf) ; [ZfK — update Leitfaden](https://www.zfk.de/wasser-abwasser/wasser/update-des-it-sicherheitsleitfadens-veroeffentlicht)
- Mapping B3S → IT-Grundschutz-Kompendium via 27 cas d'usage (version 2021) ; aucun détail sur télécontrôle/authentification dans l'extrait. — [DVGW 1708 aktualisiert](https://docshare.dvgw.de/medien/dvgw/sicherheit/1708industry-specific-security-standard_aktualisiert.pdf)
- NIS2 DE : « seit dem 6. Dezember 2025 gültig » (Rödl, 04/03/2026) ; seuil ≥ 50 salariés ou 10 M€ (ZfK) ; déclaration d'incident sous 24 h ; systèmes de détection d'attaques (KPMG). — [Rödl](https://www.roedl.com/insights/nis2-deutschland-einordnung-stadtwerke-versorger/) ; [ZfK Stadtwerke](https://www.zfk.de/digitalisierung/it/cybersecurity-fuer-stadtwerke-zuverlaessig-leistbar-und-nis2-konform) ; [KPMG Law](https://kpmg-law.de/nis2-so-muessen-sich-energieversorger-vor-cyberangriffen-schuetzen/)
- Siemens whitepaper Telecontrol Security (IEC 62443-3) — source fournisseur. — [Siemens](https://assets.new.siemens.com/siemens/assets/api/uuid:f43cc254-a4e8-4e86-8fc9-a2d924105563/difa-b10092-00whitepapertelecontrolsecurityde-144.pdf)
- Marchés DE 2026 : voir Q0 #8.

### Gaps
- Aucun fil de forum Wasserwirtschaft 2026 ; aucune publication BSI 2026 sur le « level-0 authentication » ; pas de texte W 1060 2026.

---

## Q6 — Urgence par incidents : Norvège (Bremanger 2025), Danemark (Køge/Tureby 2024, attribution déc. 2025), Pologne (rapport ABW mai 2026), Minnesota/Braham (juil.–août 2026) — réactions européennes « comment éviter cela ? »

### Takeaway
Tous les incidents convergent sur le même mode opératoire : **IHM/PLC exposés à Internet + mots de passe faibles/par défaut → manipulation directe des commandes (vannes, pompes, filtres)**. Les réactions institutionnelles européennes trouvées sont danoises (lettre de l'Agence de l'environnement à tous les services d'eau) et polonaises (budget 80 M€ eau) ; aucune réaction communautaire BE/FR/NL/DE 2026 explicite « how do we prevent this ? » n'a été capturée (LinkedIn non indexé).

### Cited Findings
- **Minnesota (26–27/07/2026)** : > 30 réseaux d'eau touchés ; Braham, Plymouth, South St. Paul, Maple Plain nommés ; à Braham les attaquants ont « shut down the plant's operating controls », usine hors ligne 2–3 h (durée variable selon source ; une source WaterWorld date Braham au 13/07) ; PLC Rockwell MicroLogix 1100/1400 exposés directement sur Internet ; pas de malware. — [SecurityAffairs](https://securityaffairs.com/196246/hacking/hackers-strike-minnesota-water-utilities-one-plant-briefly-offline.html) ; [SecureWorld](https://www.secureworld.io/industry-news/cyberattack-taps-minnesota-water) ; [WaterWorld](https://waterworld.com/55394026) ; [Tenable](https://it.tenable.com/blog/coordinated-cyberattack-on-minnesota-water-utilities-what-you-need-to-know) ; [CSA research note 31/07/2026](https://labs.cloudsecurityalliance.org/research/csa-research-note-minnesota-water-utilities-ot-attack-202607/)
- PSA FBI/EPA I-073026-PSA (30/07/2026) : incidents dans ≥ 7 États depuis le 27/07/2026 ; attaquants ont changé adresses IP et mots de passe des PLC ; effets : perte de pression, inondation ; recommandations : retirer les PLC d'Internet, mots de passe uniques, restreindre l'accès réseau, maintenir le mode manuel. — [Nozomi](https://www.nozominetworks.com/blog/what-the-fbi-epa-plc-warning-means-for-water-utilities) ; [Smart Water Magazine](https://smartwatermagazine.com/news/newsroom/fbi-and-epa-warn-of-cyberattacks-on-water-sector-plcs-after-string-of-us-incidents) ; [WaterTech](https://www.watertechonline.com/news-reports/news/55395665/fbi-issues-water-infrastructure-cyber-attack-warning)
- Attribution : non vérifiée (« possibly Iranian-affiliated » ; alignement avec CISA AA26-097A). — [Tenable](https://it.tenable.com/blog/coordinated-cyberattack-on-minnesota-water-utilities-what-you-need-to-know)
- Relais FR (blog secondaire, non recoupé) : Minnesota, Colorado (août 2026 : cycles de pompage modifiés, alarmes désactivées sur deux petites stations), 12 États. — [ayinedjimi-consultants](https://ayinedjimi-consultants.fr/news/minnesota-scada-eau-cyberattaque-30-systemes-ot-2026) (fiabilité faible)
- INCIBE/ES-ISAC (août 2026) : attaques dirigées contre les PLC du secteur de l'eau. — [INCIBE](https://www.incibe.es/empresas/blog/ataques-dirigidos-plc-en-el-sector-del-agua)
- **Pologne** : rapport ABW (couvert le 08/05/2026) : 5 stations de traitement compromises en 2025 (Jabłonna Lacka, Szczytno, Małdyty, Tolkmicko, Sierakowo) ; mots de passe usine inchangés, ICS exposés ; capacité « to modify operational parameters of equipment in real time » ; attribution APT28/APT29/UNC1151 vs « hacktivist fronts » selon sources ; deux ressortissants russes inculpés pour 17 attaques dont 7 stations eau/assainissement. — [SecurityAffairs](https://securityaffairs.com/191868/security/cyberattacks-on-polands-water-plants-a-blueprint-for-hybrid-warfare.html) ; [TNW](https://thenextweb.com/news/poland-water-treatment-cyberattack-russia-us) ; [ThreatBeat](https://threatbeat.com/critical-infrastructure/polands-water-hack-prosecution-names-russians-but-cant-reach-them-default-passwords-opened-plants/) ; Pologne : 300 attaques russes/jour, budget cyber 2025 1 Md€ dont ~80 M€ eau. — [GovInfoSecurity](https://www.govinfosecurity.com/russian-hackers-accused-in-wave-water-sector-cyberattacks-a-29264) ; analyse juridique [Lieber Institute](https://lieber.westpoint.edu/when-red-lines-cross-blue-lines-cyber-attacks-polands-water-infrastructure-part-i/)
- **Norvège (Bremanger, avril 2025)** : vanne du barrage ouverte ~500 l/s pendant ~4 h ; attribution russe par le contre-espionnage norvégien (une base de données dit avril 2024 — conflit). — [Dark Reading](https://www.darkreading.com/ics-ot-security/water-systems-attack-norway-poland-russia-actors) ; [CSO Online](https://www.csoonline.com/article/4042449/russia-linked-european-attacks-renew-concerns-over-water-cybersecurity.html)
- **Danemark (Tureby Alkestrup, Køge, fin 2024 ; attribution 19/12/2025 à Z-Pentest)** : VNC exposé + mot de passe faible ; ~50 foyers sans eau 7 h, ~450 ~1 h, 3 canalisations éclatées ; **l'Agence de l'environnement a écrit à tous les services d'eau/assainissement danois** pour exiger des contrôles quotidiens des équipements critiques et un durcissement des accès ; 12 % des 691 vandværker sondés ont déjà subi une cyberattaque. — [TV2 Kosmopol](https://www.tv2kosmopol.dk/koege/russiske-hackere-angreb-dansk-vandvaerk-nu-strammer-myndighederne-kravene-cad31) ; [Euronews](https://euronews.com/2025/12/19/denmark-blames-russia-for-cyberattacks-on-water-utility-and-election-websites) ; [RISI](https://www.risidata.com/Database/Detail/denmark-tureby-waterworks-z-pentest-2024) ; [sn.dk](https://www.sn.dk/art6650638/gribskov-kommune/indland/hvert-ottende-vandvaerk-har-vaeret-udsat-for-cyberangreb/) ; [ing.dk — manque de sécurité de base](https://ing.dk/artikel/aabne-porte-og-manglende-opdateringer-danske-vandvaerker-mangler-basal-it-sikkerhed)
- ENISA : des milliers de petites entités rendent le progrès difficile dans l'eau. — [CSO Online](https://www.csoonline.com/article/4042449/russia-linked-european-attacks-renew-concerns-over-water-cybersecurity.html)
- Rapport nation-state 2024–2026 sur l'eau. — [DomainTools](https://dti.domaintools.com/research/threat-intelligence-report-nation-state-targeting-of-water-systems-2024-2026)

### Inferences
- Dans tous les cas, la défense recommandée officiellement reste périmétrique (retirer d'Internet, mots de passe). Aucun avis officiel ne pousse l'**authentification cryptographique de chaque ordre au niveau de l'actionneur** : c'est un espace argumentatif libre pour OASIS (« même si l'IHM tombe, la pompe refuse l'ordre non signé »).
- Réplique FR générique 3 lignes pour tout fil « comment éviter Braham/Køge/Bremanger chez nous ? » :
  1. « Dans les trois cas, l'attaquant n'a eu qu'à atteindre l'IHM ou l'automate : l'ordre lui-même n'était ni authentifié ni borné. Retirer le PLC d'Internet est nécessaire mais ne suffit pas (prestataire, VPN volé, insider). »
  2. « Rendre l'actionneur sceptique : il n'exécute qu'un ordre signé par un opérateur enrôlé, dans les limites, non rejoué, et journalise la décision — c'est ce qu'OASIS fait devant un Modbus RTU/TCP existant, sans changer l'automate. »
  3. « Preuves publiques et reproductibles (tests, preuves formelles, essais sur microcontrôleur) ; heureux d'en discuter avec un exploitant prêt à tester sur un poste de relevage. »

### Gaps
- Aucune réaction nominative d'opérateur BE/FR/NL/DE/LU 2026 aux incidents ; pas de post waterisac public 2026 trouvé ; aucun incident 2026 confirmé en IT/ES/FR/BE/NL/DE.

---

## Q7 — Luxembourg

### Takeaway
Loi NIS2 adoptée le 05/05/2026, en vigueur le 10/05/2026, auto-enregistrement avant le 10/07/2026 (ILR autorité) ; aucune trace de SIDEN/SEBES/SIDERO ni de marché télégestion 2026.

### Cited Findings
- Loi adoptée 05/05/2026, en vigueur 10/05/2026 ; seuils 50 salariés / 10 M€ ; 18 secteurs ; enregistrement avant 10/07/2026, manquement sanctionnable. — [PwC Luxembourg](https://www.pwc.lu/en/newsletter/2026/nis2-luxembourg.html) ; [Simmons & Simmons](https://www.simmons-simmons.com/en/publications/cmqhtf6pm00dov6occ2svi2uz/luxembourg-transposes-nis2-strengthening-cybersecurity-rules-) ; [Paperjam — jusqu'à 2 000 entités](https://en.paperjam.lu/article/up-to-2-000-entities-face-nis2-cyber-deadline) ; [ILR NIS2](https://www.ilr.lu/en/sectors/niss/nis-2/)

### Gaps
- Rien sur les syndicats d'eau luxembourgeois ni sur des marchés 2026.
